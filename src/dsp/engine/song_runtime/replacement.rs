//! Atomic replacement prevalidation and exact-frame ownership transitions.
use super::*;

impl Engine {
    fn validate_song_activation(&self, epoch: SnapshotEpoch) -> Result<(), SongRejectCode> {
        let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let preparation = stager
            .preparations
            .iter()
            .find(|p| p.command.preparation.epoch == epoch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if !preparation.ready || preparation.active || preparation.cancelling {
            return Err(SongRejectCode::NotReady);
        }
        let runtime = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        let branches = stager.branches.iter().filter(|b| b.epoch == epoch).count();
        let leases = stager
            .leases
            .iter()
            .filter(|l| l.key.epoch == epoch)
            .count();
        if runtime.active.len() == runtime.active.capacity()
            || branches > runtime.branches.capacity() - runtime.branches.len()
            || leases > runtime.leases.capacity() - runtime.leases.len()
            || branches > runtime.tracks.capacity() - runtime.tracks.len()
        {
            return Err(SongRejectCode::Capacity);
        }
        let reusable = stager.reusable.iter().filter(|s| s.epoch == epoch).count();
        if reusable > runtime.reusable.capacity() - runtime.reusable.len() {
            return Err(SongRejectCode::Capacity);
        }
        // Complete physical validation precedes every state change.
        for lease in stager.leases.iter().filter(|l| l.key.epoch == epoch) {
            if !lease.staged || lease.retiring {
                return Err(SongRejectCode::NotReady);
            }
            let valid = match lease.key.kind {
                SongResourceKind::Instrument => self.templates.slots.iter().any(|s| {
                    s.song == Some(lease.key)
                        && s.state == crate::dsp::bus::SlotState::Staged
                        && s.t.is_some()
                }),
                SongResourceKind::PrivateFx
                | SongResourceKind::Track
                | SongResourceKind::Master => self.buses.song_index(lease.key).is_some_and(|i| {
                    self.buses.slots[i].state == crate::dsp::bus::SlotState::Staged
                }),
                SongResourceKind::Sample => self.store.song_is_staged(lease.key),
                SongResourceKind::ControlCells | SongResourceKind::AnalysisBank => true,
            };
            if !valid {
                return Err(SongRejectCode::NotReady);
            }
        }
        for branch in stager.branches.iter().filter(|b| b.epoch == epoch) {
            if branch.private_fx.is_none()
                || branch.track_template.is_none()
                || branch.master.is_none()
            {
                return Err(SongRejectCode::NotReady);
            }
            if stager.branches.iter().any(|other| {
                other.epoch == epoch
                    && other.branch != branch.branch
                    && other.private_fx == branch.private_fx
            }) {
                return Err(SongRejectCode::Malformed);
            }
        }
        for config in stager.branches.iter().filter(|b| b.epoch == epoch) {
            let key = |resource, kind| SongLeaseKey {
                epoch,
                resource,
                kind,
            };
            if !self
                .templates
                .slots
                .iter()
                .any(|s| s.song == Some(key(config.instrument, SongResourceKind::Instrument)))
                || config
                    .private_fx
                    .and_then(|r| self.buses.song_index(key(r, SongResourceKind::PrivateFx)))
                    .is_none()
                || config
                    .track_template
                    .and_then(|r| self.buses.song_index(key(r, SongResourceKind::Track)))
                    .is_none()
                || config
                    .master
                    .and_then(|r| self.buses.song_index(key(r, SongResourceKind::Master)))
                    .is_none()
            {
                return Err(SongRejectCode::NotReady);
            }
        }
        Ok(())
    }
    pub(in crate::dsp::engine) fn activate_song_epoch(
        &mut self,
        epoch: SnapshotEpoch,
    ) -> Result<(), SongRejectCode> {
        if self.song_runtime.as_ref().is_some_and(|r| r.commands.iter().any(|c|
            matches!(c.command, SongCommand::Endpoints(e) if e.epoch == epoch && e.tail_deadline < self.frame))) {
            self.cancel_song_preparation(epoch)?;
            return Err(SongRejectCode::NotReady);
        }
        self.validate_song_activation(epoch)?;
        let stager = self.song_stager.as_mut().ok_or(SongRejectCode::NotReady)?;
        for lease in stager.leases.iter().filter(|l| l.key.epoch == epoch) {
            match lease.key.kind {
                SongResourceKind::Instrument => {
                    self.templates.activate_song_template(lease.key)?;
                }
                SongResourceKind::PrivateFx
                | SongResourceKind::Track
                | SongResourceKind::Master => {
                    self.buses.activate_song_bus(lease.key)?;
                }
                SongResourceKind::Sample => {
                    self.store.activate_song_sample(lease.key)?;
                }
                SongResourceKind::ControlCells | SongResourceKind::AnalysisBank => {}
            }
        }
        stager.close_preparation_for_activation(epoch)?;
        let runtime = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        for config in stager.branches.iter().filter(|b| b.epoch == epoch).copied() {
            let key = |resource, kind| SongLeaseKey {
                epoch,
                resource,
                kind,
            };
            let template = self
                .templates
                .slots
                .iter()
                .position(|s| s.song == Some(key(config.instrument, SongResourceKind::Instrument)))
                .ok_or(SongRejectCode::NotReady)?;
            let fx = config
                .private_fx
                .and_then(|r| self.buses.song_index(key(r, SongResourceKind::PrivateFx)));
            let track = config
                .track_template
                .and_then(|r| self.buses.song_index(key(r, SongResourceKind::Track)))
                .ok_or(SongRejectCode::NotReady)?;
            let master = config
                .master
                .and_then(|r| self.buses.song_index(key(r, SongResourceKind::Master)))
                .ok_or(SongRejectCode::NotReady)?;
            if !runtime
                .tracks
                .iter()
                .any(|t| t.epoch == epoch && t.track == track)
            {
                runtime.tracks.push(RuntimeTrack {
                    epoch,
                    track,
                    master,
                    track_key: key(
                        config.track_template.ok_or(SongRejectCode::NotReady)?,
                        SongResourceKind::Track,
                    ),
                    master_key: key(
                        config.master.ok_or(SongRejectCode::NotReady)?,
                        SongResourceKind::Master,
                    ),
                });
            }
            runtime.branches.push(RuntimeBranch {
                config,
                template,
                fx,
                track,
                muted: false,
                ended: false,
                gate: SongGainGate::open(),
                release_frame: None,
                closed: false,
            });
        }
        runtime.leases.extend(
            stager
                .leases
                .iter()
                .filter(|l| l.key.epoch == epoch)
                .map(|l| l.key),
        );
        runtime
            .reusable
            .extend(stager.reusable.iter().filter(|s| s.epoch == epoch).copied());
        runtime.active.push(epoch);
        Ok(())
    }
}

use crate::dsp::song::{
    SongAppliedOwner, SongReplacementOutcome, SongReplacementPhase, SongReplacementRecord,
};
use crate::song::routing::{SongInitialMute, SongReplacement};

impl Engine {
    fn validate_replacement_previous(
        &self,
        record: SongReplacementRecord,
    ) -> Result<(), SongRejectCode> {
        let r = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        let owner = r.last_applied_owner.ok_or(SongRejectCode::StaleEpoch)?;
        if owner.activation != record.previous.activation
            || owner.endpoint != record.previous.endpoint
        {
            return Err(SongRejectCode::StaleEpoch);
        }
        if owner.musically_closed {
            return Ok(());
        }
        if !r.owns_epoch(owner.activation.epoch) {
            return Err(SongRejectCode::NotReady);
        }
        for b in r
            .branches
            .iter()
            .filter(|b| b.config.epoch == owner.activation.epoch && !b.closed)
        {
            let key = b.fx_key()?;
            if !r.owns_lease(b.owner().instrument)
                || self
                    .templates
                    .slots
                    .get(b.template)
                    .is_none_or(|s| s.song != Some(b.owner().instrument))
                || b.fx.is_none_or(|i| !self.buses.song_is_live(key, i))
                || !self.buses.can_reset_song_bus(key)
            {
                return Err(SongRejectCode::NotReady);
            }
        }
        for t in r
            .tracks
            .iter()
            .filter(|t| t.epoch == owner.activation.epoch)
        {
            if !t.owned_track(&self.buses)
                || !t.owned_master(&self.buses)
                || !self.buses.can_reset_song_bus(t.track_key)
                || !self.buses.can_reset_song_bus(t.master_key)
            {
                return Err(SongRejectCode::NotReady);
            }
        }
        Ok(())
    }
    pub(in crate::dsp::engine) fn prime_song_replacement(
        &mut self,
        command: SongInitialMute,
    ) -> Result<(), SongRejectCode> {
        if !SongCommand::PrimeMute(command).valid() {
            return Err(SongRejectCode::Malformed);
        }
        let s = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        if !s.preparations.iter().any(|p| {
            p.command.preparation.epoch == command.epoch && p.ready && !p.active && !p.cancelling
        }) || !s
            .branches
            .iter()
            .any(|b| b.epoch == command.epoch && b.family == command.instrument)
        {
            return Err(SongRejectCode::NotReady);
        }
        let r = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        if r.replacements
            .iter()
            .any(|x| x.command.activation.epoch == command.epoch)
            || r.primed_mutes.iter().any(|m| {
                m.epoch == command.epoch
                    && (m.instrument == command.instrument
                        || m.overlay_nonce != command.overlay_nonce)
            })
        {
            return Err(SongRejectCode::Malformed);
        }
        if r.primed_mutes.len() == r.primed_mutes.capacity() {
            return Err(SongRejectCode::Capacity);
        }
        r.primed_mutes.push(command);
        Ok(())
    }
    pub(in crate::dsp::engine) fn queue_song_replacement(
        &mut self,
        command: SongReplacement,
    ) -> Result<(), SongRejectCode> {
        if !SongCommand::Replace(command).valid() || command.activation.frame - 64 < self.frame {
            return Err(SongRejectCode::Malformed);
        }
        self.validate_song_activation(command.activation.epoch)?;
        let r = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        let previous = r
            .last_applied_owner
            .filter(|p| p.activation.epoch == command.previous)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if previous.endpoint.is_none() || previous.activation.frame > command.activation.frame
            || r.replacements.iter().any(|x| matches!(x.phase, SongReplacementPhase::Queued | SongReplacementPhase::Armed | SongReplacementPhase::FailedAfterArm))
            || r.commands.iter().any(|c| matches!(c.command, SongCommand::Activate(a) if a.epoch == command.activation.epoch))
        { return Err(SongRejectCode::NotReady); }
        if r.replacements.len() == r.replacements.capacity() {
            return Err(SongRejectCode::Capacity);
        }
        let count = r
            .primed_mutes
            .iter()
            .filter(|m| m.epoch == command.activation.epoch)
            .count();
        if count != usize::try_from(command.overlay_count).map_err(|_| SongRejectCode::Capacity)?
            || r.primed_mutes.iter().any(|m| {
                m.epoch == command.activation.epoch && m.overlay_nonce != command.overlay_nonce
            })
        {
            return Err(SongRejectCode::Malformed);
        }
        let r = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        let old_new = !r.has_endpoint_obligation(command.previous);
        if r.owns_epoch(command.previous) {
            r.reserve_endpoint(command.previous)?;
        }
        if let Err(reason) = r.reserve_endpoint(command.activation.epoch) {
            if old_new {
                r.release_endpoint_reservation(command.previous);
            }
            return Err(reason);
        }
        r.replacements.push(SongReplacementRecord {
            command,
            phase: SongReplacementPhase::Queued,
            previous,
            outcomes: [None; 2],
        });
        Ok(())
    }
    pub(in crate::dsp::engine) fn cancel_song_replacement(
        &mut self,
        epoch: SnapshotEpoch,
    ) -> Result<(), SongRejectCode> {
        let r = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        let Some(index) = r
            .replacements
            .iter()
            .position(|x| x.command.activation.epoch == epoch)
        else {
            self.cancel_song_preparation(epoch)?;
            if let Some(r) = &mut self.song_runtime {
                r.primed_mutes.retain(|m| m.epoch != epoch);
                r.commands.retain(|c| c.command.epoch() != epoch);
                r.release_endpoint_reservation(epoch);
            }
            return Ok(());
        };
        let record = &mut r.replacements[index];
        if matches!(
            record.phase,
            SongReplacementPhase::Queued | SongReplacementPhase::Rejected
        ) {
            record.outcomes[0] = Some(SongReplacementOutcome {
                acknowledgment: SongHostAck::ActivationRejected {
                    activation: record.command.activation,
                    reason: SongRejectCode::NotReady,
                },
                frame: self.frame,
                before_boundary_commit: true,
            });
            // Retain the bounded outcome until delivered, but never arm this record.
            record.phase = SongReplacementPhase::Rejected;
            r.commands.retain(|c| c.command.epoch() != epoch);
            r.primed_mutes.retain(|m| m.epoch != epoch);
            r.release_endpoint_reservation(epoch);
            self.cancel_song_preparation(epoch)
        } else {
            let before_boundary_commit = record.phase != SongReplacementPhase::Applied;
            if before_boundary_commit {
                record.phase = SongReplacementPhase::FailedAfterArm;
            }
            if record.outcomes[1].is_none() {
                record.outcomes[1] = Some(SongReplacementOutcome {
                    acknowledgment: SongHostAck::Rejected {
                        epoch,
                        reason: SongRejectCode::HostFault,
                    },
                    frame: self.frame,
                    before_boundary_commit,
                });
            }
            r.commands.retain(|c| {
                c.command.epoch() != epoch || matches!(c.command, SongCommand::Endpoints(_))
            });
            Ok(())
        }
    }
    pub(in crate::dsp::engine) fn advance_song_replacements(&mut self, frame: u64) {
        let count = self
            .song_runtime
            .as_ref()
            .map_or(0, |r| r.replacements.len());
        for index in 0..count {
            let Some(record) = self
                .song_runtime
                .as_ref()
                .and_then(|r| r.replacements.get(index))
                .copied()
            else {
                continue;
            };
            let a = record.command.activation;
            if record.phase == SongReplacementPhase::Queued && frame >= a.frame - 64 {
                let result = if frame != a.frame - 64 {
                    Err(SongRejectCode::Malformed)
                } else {
                    self.validate_replacement_previous(record).and_then(|()| {
                        let r = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
                        if (r.owns_epoch(record.command.previous)
                            && !r.has_endpoint_obligation(record.command.previous))
                            || !r.has_endpoint_obligation(a.epoch)
                        {
                            return Err(SongRejectCode::Capacity);
                        }
                        self.activate_song_epoch(a.epoch)
                    })
                };
                let r = match self.song_runtime.as_mut() {
                    Some(r) => r,
                    None => return,
                };
                match result {
                    Ok(()) => {
                        r.replacements[index].phase = SongReplacementPhase::Armed;
                        for branch in r.branches.iter_mut().filter(|b| b.config.epoch == a.epoch) {
                            if let Some(m) = r.primed_mutes.iter().find(|m| {
                                m.epoch == a.epoch && m.instrument == branch.config.family
                            }) {
                                branch.muted = m.muted;
                                if m.muted {
                                    branch.gate = SongGainGate {
                                        from: 0.0,
                                        to: 0.0,
                                        start: frame,
                                        end: frame,
                                    };
                                }
                            }
                        }
                    }
                    Err(reason) => {
                        r.replacements[index].phase = SongReplacementPhase::Rejected;
                        r.replacements[index].outcomes[0] = Some(SongReplacementOutcome {
                            acknowledgment: SongHostAck::ActivationRejected {
                                activation: a,
                                reason,
                            },
                            frame,
                            before_boundary_commit: true,
                        });
                        r.release_endpoint_reservation(a.epoch);
                        r.commands.retain(|c| c.command.epoch() != a.epoch);
                    }
                }
            }
            let Some(current) = self
                .song_runtime
                .as_ref()
                .and_then(|r| r.replacements.get(index))
                .copied()
            else {
                continue;
            };
            if matches!(
                current.phase,
                SongReplacementPhase::Armed | SongReplacementPhase::FailedAfterArm
            ) && frame >= a.frame
            {
                self.clear_replaced_song_owner(current.command.previous, a.frame);
                let Some(r) = &mut self.song_runtime else {
                    return;
                };
                let endpoint = r.commands.iter().find_map(|c| match c.command {
                    SongCommand::Endpoints(e) if e.epoch == a.epoch => Some(e),
                    _ => None,
                });
                r.last_applied_owner = Some(SongAppliedOwner {
                    activation: a,
                    endpoint,
                    musically_closed: false,
                });
                r.replacements[index].phase = SongReplacementPhase::Applied;
                r.replacements[index].outcomes[0] = Some(SongReplacementOutcome {
                    acknowledgment: SongHostAck::Applied(a),
                    frame: a.frame,
                    before_boundary_commit: false,
                });
                if current.phase == SongReplacementPhase::FailedAfterArm && endpoint.is_none() {
                    r.commit_endpoint(SongEpochEnd {
                        epoch: a.epoch,
                        arrangement: a.frame,
                        deadline: a.frame,
                        closed: false,
                    });
                    for b in r.branches.iter_mut().filter(|b| b.config.epoch == a.epoch) {
                        b.ended = true;
                        b.config.tail_deadline = a.frame;
                    }
                }
                r.primed_mutes.retain(|m| m.epoch != a.epoch);
            }
        }
    }
    fn clear_replaced_song_owner(&mut self, epoch: SnapshotEpoch, frame: u64) {
        let Some(r) = &mut self.song_runtime else {
            return;
        };
        if !r.owns_epoch(epoch) {
            return;
        }
        for b in r.branches.iter_mut().filter(|b| b.config.epoch == epoch) {
            b.ended = true;
            b.config.tail_deadline = b.config.tail_deadline.min(frame);
        }
        if let Some(e) = r.endpoints.iter_mut().find(|e| e.epoch == epoch) {
            e.arrangement = e.arrangement.min(frame);
            e.deadline = e.deadline.min(frame);
        } else {
            r.commit_endpoint(SongEpochEnd {
                epoch,
                arrangement: frame,
                deadline: frame,
                closed: false,
            });
        }
        r.commands
            .retain(|c| c.command.epoch() != epoch || c.frame < frame);
        self.finish_song_deadlines(frame);
    }
}
