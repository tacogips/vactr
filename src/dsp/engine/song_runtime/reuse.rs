//! Keyed reusable branch lifecycle and final ownership.
use super::*;
use crate::dsp::song::{SongReusableSlot, SongRuntime};
use crate::song::routing::{
    SongBranchConfig, SongBranchRebind, SongBranchRebound, SongReusableBranch,
};

impl Engine {
    pub(in crate::dsp::engine) fn retire_song_runtime(&mut self, frame: u64) {
        let (Some(runtime), Some(stager)) = (&mut self.song_runtime, &mut self.song_stager) else {
            return;
        };
        for lease in &mut stager.leases {
            let final_due = stager
                .preparations
                .iter()
                .any(|p| p.command.preparation.epoch == lease.key.epoch && p.cancelling)
                || runtime
                    .endpoints
                    .iter()
                    .any(|e| e.epoch == lease.key.epoch && frame >= e.deadline);
            if Self::retains_reusable_song_lease(runtime, lease.key, final_due) {
                continue;
            }
            if !runtime.owns_lease(lease.key) || lease.retiring {
                continue;
            }
            if matches!(
                lease.key.kind,
                SongResourceKind::Track | SongResourceKind::Master
            ) && !runtime
                .endpoints
                .iter()
                .any(|e| e.epoch == lease.key.epoch && frame >= e.deadline)
            {
                continue;
            }
            let related = |branch: &RuntimeBranch| {
                if branch.config.epoch != lease.key.epoch {
                    return false;
                }
                match lease.key.kind {
                    SongResourceKind::Instrument => branch.config.instrument == lease.key.resource,
                    SongResourceKind::PrivateFx => {
                        branch.config.private_fx == Some(lease.key.resource)
                    }
                    SongResourceKind::Track => {
                        branch.config.track_template == Some(lease.key.resource)
                    }
                    SongResourceKind::Master => branch.config.master == Some(lease.key.resource),
                    _ => true,
                }
            };
            if !runtime.branches.iter().any(related)
                && !runtime
                    .endpoints
                    .iter()
                    .any(|e| e.epoch == lease.key.epoch && frame >= e.deadline)
            {
                continue;
            }
            if runtime.branches.iter().filter(|b| related(b)).any(|b| {
                !b.ended
                    || frame < b.config.tail_deadline
                    || runtime
                        .voices
                        .iter()
                        .zip(self.pool.voices.iter())
                        .any(|(owner, voice)| voice.active && *owner == Some(b.owner()))
            }) {
                continue;
            }
            let result = match lease.key.kind {
                SongResourceKind::Instrument => {
                    self.templates.close_song_template_for_return(lease.key)
                }
                SongResourceKind::PrivateFx
                | SongResourceKind::Track
                | SongResourceKind::Master => self.buses.close_song_bus_for_return(lease.key),
                SongResourceKind::Sample => self.store.retire_song_sample(lease.key),
                SongResourceKind::ControlCells | SongResourceKind::AnalysisBank => Ok(()),
            };
            if result.is_ok() {
                lease.retiring = true;
            }
        }
        runtime.branches.retain(|b| {
            stager
                .leases
                .iter()
                .any(|l| l.key == b.owner().instrument && !l.retiring)
        });
    }
    pub(in crate::dsp::engine) fn finish_song_deadlines(&mut self, frame: u64) {
        let (Some(r), Some(s)) = (&mut self.song_runtime, &self.song_stager) else {
            return;
        };
        for b in &mut r.branches {
            if b.closed
                || !((b.muted && frame >= b.gate.end)
                    || (b.ended && frame >= b.config.tail_deadline))
            {
                continue;
            }
            for (v, owner) in self.pool.voices.iter_mut().zip(r.voices.iter_mut()) {
                if *owner != Some(b.owner()) {
                    continue;
                }
                if v.active {
                    self.buses.slots[v.bus].users = self.buses.slots[v.bus].users.saturating_sub(1);
                }
                v.stop_song_voice();
                *owner = None;
            }
            if let Some(fx) = b.fx {
                if let Ok(key) = b.fx_key() {
                    if !self.buses.song_is_live(key, fx) {
                        continue;
                    }
                    let _ = self.buses.reset_song_bus(
                        key,
                        &s.private_cells(key),
                        self.sr,
                        &self.cfg.caps,
                    );
                }
            }
            b.closed = true;
        }
        for e in r
            .endpoints
            .iter_mut()
            .filter(|e| !e.closed && frame >= e.deadline)
        {
            for b in r.tracks.iter().filter(|b| b.epoch == e.epoch) {
                for (i, key) in [(b.track, b.track_key), (b.master, b.master_key)] {
                    if (i == b.track && b.owned_track(&self.buses))
                        || (i == b.master && b.owned_master(&self.buses))
                    {
                        let _ = self.buses.reset_song_bus(
                            key,
                            &s.private_cells(key),
                            self.sr,
                            &self.cfg.caps,
                        );
                    }
                }
            }
            e.closed = true;
            if let Some(owner) = r
                .last_applied_owner
                .as_mut()
                .filter(|p| p.activation.epoch == e.epoch)
            {
                owner.musically_closed = true;
            }
        }
    }
    pub(in crate::dsp::engine) fn reap_returned_song_epochs(&mut self) {
        let (Some(r), Some(s)) = (&mut self.song_runtime, &mut self.song_stager) else {
            return;
        };
        r.leases
            .retain(|key| s.leases.iter().any(|l| l.key == *key));
        r.tracks.retain(|t| {
            r.endpoints
                .iter()
                .all(|e| e.epoch != t.epoch || self.frame < e.deadline)
                || s.leases.iter().any(|l| {
                    l.key.epoch == t.epoch
                        && matches!(
                            l.key.kind,
                            SongResourceKind::Track | SongResourceKind::Master
                        )
                })
        });
        r.active.retain(|epoch| {
            let complete = r.endpoints.iter().any(|e| e.epoch == *epoch && self.frame >= e.deadline);
            let pending_message = |message: &Option<HostMsg>| matches!(message, Some(HostMsg::Song(a)) if a.epoch() == *epoch);
            !complete || r.leases.iter().any(|k| k.epoch == *epoch)
                || r.branches.iter().any(|b| b.config.epoch == *epoch)
                || r.tracks.iter().any(|t| t.epoch == *epoch)
                || r.voices.iter().flatten().any(|o| o.epoch == *epoch)
                || r.commands.iter().any(|c| c.command.epoch() == *epoch)
                || r.receipts.iter().any(|a| a.epoch() == *epoch)
                || r.replacements.iter().any(|x| x.outcomes.iter().flatten().any(|o| o.acknowledgment.epoch() == *epoch))
                || self.pending_song_install.as_ref().is_some_and(|i| i.lease.epoch == *epoch)
                || self.song_validation.as_ref().is_some_and(|(i, _)| i.lease.epoch == *epoch)
                || self.song_return_key.is_some_and(|(k, _)| k.epoch == *epoch)
                || pending_message(&self.song_ack) || pending_message(&self.song_followup)
        });
        s.preparations
            .retain(|p| !p.active || r.active.contains(&p.command.preparation.epoch));
        r.endpoints.retain(|e| r.active.contains(&e.epoch));
        r.replacements.retain(|x| {
            x.outcomes.iter().any(Option::is_some)
                || matches!(
                    x.phase,
                    crate::dsp::song::SongReplacementPhase::Queued
                        | crate::dsp::song::SongReplacementPhase::Armed
                        | crate::dsp::song::SongReplacementPhase::FailedAfterArm
                )
                || r.active.contains(&x.command.activation.epoch)
        });
        r.endpoint_reservations.retain(|epoch| {
            r.active.contains(epoch)
                || r.commands.iter().any(|c| c.command.epoch() == *epoch)
                || r.replacements.iter().any(|x| {
                    x.command.activation.epoch == *epoch
                        && matches!(
                            x.phase,
                            crate::dsp::song::SongReplacementPhase::Queued
                                | crate::dsp::song::SongReplacementPhase::Armed
                                | crate::dsp::song::SongReplacementPhase::FailedAfterArm
                        )
                })
        });
        self.cleanup_song_reusable_slots();
    }
}

impl Engine {
    /// Register a physical pool slot without activating it.
    /// # Errors
    /// Invalid generation range, aliased private graph or staging capacity.
    pub fn stage_song_reusable_branch(
        &mut self,
        reusable: SongReusableBranch,
    ) -> Result<(), SongRejectCode> {
        if !reusable.valid() {
            return Err(SongRejectCode::Malformed);
        }
        let s = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let r = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        if s.reusable.len() == s.reusable.capacity()
            || s.reusable.len() >= r.reusable.capacity()
            || s.reusable_slot(reusable.config.epoch, reusable.config.branch)
                .is_some()
        {
            return Err(SongRejectCode::Capacity);
        }
        self.stage_song_branch(reusable.config)?;
        if let Some(s) = &mut self.song_stager {
            s.reusable.push(SongReusableSlot {
                epoch: reusable.config.epoch,
                branch: reusable.config.branch,
                last_generation: reusable.last_generation,
            });
        }
        Ok(())
    }
    pub(in crate::dsp::engine) fn validate_song_private_exclusivity(
        &self,
        config: SongBranchConfig,
    ) -> Result<(), SongRejectCode> {
        let Some(private) = config.private_fx else {
            return Ok(());
        };
        let aliases = |b: &SongBranchConfig| {
            b.epoch == config.epoch && b.private_fx == Some(private) && b.branch != config.branch
        };
        if self
            .song_stager
            .as_ref()
            .is_some_and(|s| s.branches.iter().any(aliases))
            || self
                .song_runtime
                .as_ref()
                .is_some_and(|r| r.branches.iter().any(|b| aliases(&b.config)))
        {
            return Err(SongRejectCode::Malformed);
        }
        Ok(())
    }
    pub(in crate::dsp::engine) fn validate_reusable_private_slot(
        &self,
        branch: RuntimeBranch,
    ) -> Result<(), SongRejectCode> {
        self.validate_song_private_exclusivity(branch.config)?;
        let key = branch.fx_key()?;
        let index = branch.fx.ok_or(SongRejectCode::NotReady)?;
        if !self.buses.song_is_live(key, index)
            || !self.buses.can_reset_song_bus(key)
            || self.buses.slots[index].users != 0
        {
            return Err(SongRejectCode::NotReady);
        }
        let r = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        if r.voices
            .iter()
            .zip(self.pool.voices.iter())
            .any(|(owner, v)| v.active && (v.bus == index || *owner == Some(branch.owner())))
        {
            return Err(SongRejectCode::NotReady);
        }
        Ok(())
    }
    pub(in crate::dsp::engine) fn rebind_song_branch(
        &mut self,
        rebind: SongBranchRebind,
        frame: u64,
    ) -> Result<SongBranchRebound, SongRejectCode> {
        if !rebind.valid() || frame != rebind.transition_frame {
            return Err(SongRejectCode::Malformed);
        }
        let r = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        if r.receipts.len() == r.receipts.capacity() {
            return Err(SongRejectCode::Capacity);
        }
        if !r.owns_epoch(rebind.epoch) || r.endpoints.iter().any(|e| e.epoch == rebind.epoch) {
            return Err(SongRejectCode::StaleEpoch);
        }
        let slot = r
            .reusable_slot(rebind.epoch, rebind.branch)
            .ok_or(SongRejectCode::NotReady)?;
        if rebind.generation > slot.last_generation {
            return Err(SongRejectCode::Capacity);
        }
        let index = r
            .branches
            .iter()
            .position(|b| b.config.epoch == rebind.epoch && b.config.branch == rebind.branch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        let b = r.branches[index];
        if b.config.generation != rebind.expected_generation {
            return Err(SongRejectCode::StaleEpoch);
        }
        if !b.ended || !b.closed || frame < b.config.tail_deadline {
            return Err(SongRejectCode::NotReady);
        }
        let s = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let p = s
            .preparations
            .iter()
            .find(|p| p.command.preparation.epoch == rebind.epoch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        if !p.active || p.cancelling {
            return Err(SongRejectCode::NotReady);
        }
        for (kind, resource) in [
            (SongResourceKind::Instrument, Some(b.config.instrument)),
            (SongResourceKind::PrivateFx, b.config.private_fx),
            (SongResourceKind::Track, b.config.track_template),
            (SongResourceKind::Master, b.config.master),
        ] {
            let key = SongLeaseKey {
                epoch: rebind.epoch,
                resource: resource.ok_or(SongRejectCode::NotReady)?,
                kind,
            };
            s.lease(key)?;
            if !r.owns_lease(key) {
                return Err(SongRejectCode::StaleEpoch);
            }
            if kind == SongResourceKind::Instrument {
                if self.templates.slots.get(b.template).is_none_or(|t| {
                    t.song != Some(key)
                        || t.state != crate::dsp::bus::SlotState::Live
                        || t.t.is_none()
                }) {
                    return Err(SongRejectCode::NotReady);
                }
            } else if self
                .buses
                .song_index(key)
                .is_none_or(|i| !self.buses.song_is_live(key, i))
            {
                return Err(SongRejectCode::NotReady);
            }
        }
        let staged_index = s
            .branches
            .iter()
            .position(|c| c.epoch == rebind.epoch && c.branch == rebind.branch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        let mut staged = s.branches[staged_index];
        staged.tail_deadline = b.config.tail_deadline;
        if staged != b.config {
            return Err(SongRejectCode::StaleEpoch);
        }
        self.validate_reusable_private_slot(b)?;
        let key = b.fx_key()?;
        self.buses
            .reset_song_bus(key, &s.private_cells(key), self.sr, &self.cfg.caps)?;
        // All remaining commits are bounded infallible updates. Family mute persists.
        if let Some(r) = &mut self.song_runtime {
            let b = &mut r.branches[index];
            b.config.generation = rebind.generation;
            b.config.transition_frame = frame;
            b.config.tail_deadline = rebind.tail_deadline;
            b.ended = false;
            b.closed = b.muted;
            b.release_frame = None;
            b.gate = SongGainGate {
                from: if b.muted { 0.0 } else { 1.0 },
                to: if b.muted { 0.0 } else { 1.0 },
                start: frame,
                end: frame,
            };
            if let Some(s) = &mut self.song_stager {
                s.branches[staged_index] = b.config;
            }
        }
        Ok(SongBranchRebound {
            epoch: rebind.epoch,
            branch: rebind.branch,
            generation: rebind.generation,
            frame,
        })
    }
    pub(in crate::dsp::engine) fn retains_reusable_song_lease(
        runtime: &SongRuntime,
        key: SongLeaseKey,
        final_retirement_due: bool,
    ) -> bool {
        !final_retirement_due && runtime.reusable.iter().any(|slot| slot.epoch == key.epoch)
    }
    pub(in crate::dsp::engine) fn cleanup_song_reusable_slots(&mut self) {
        if let (Some(r), Some(s)) = (&mut self.song_runtime, &mut self.song_stager) {
            r.reusable.retain(|slot| {
                r.branches
                    .iter()
                    .any(|b| b.config.epoch == slot.epoch && b.config.branch == slot.branch)
            });
            s.reusable.retain(|slot| {
                s.branches
                    .iter()
                    .any(|b| b.epoch == slot.epoch && b.branch == slot.branch)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::arena::StoreKind;
    use crate::dsp::bus::BusTemplate;
    use crate::dsp::caps::CapabilitySet;
    use crate::dsp::engine::{EngineConfig, SongStagingConfig};
    use crate::dsp::graph::{BusDef, BusId, InstDef, InstId, UGenSpec};
    use crate::dsp::ring::{NativeInstall, NativeSongInstall, SpscRing};
    use crate::dsp::ugen::Template;
    use crate::song::routing::*;

    fn physical_reuse_engine() -> Engine {
        let caps = CapabilitySet::browser();
        let mut config = EngineConfig::new(&caps, 48000., 64, StoreKind::NativeArc);
        config.bus_slots = 4;
        config.bus_seconds = 12.;
        config.template_slots = 2;
        let mut e = Engine::with_config(config);
        let (acks, _rx) = SpscRing::split(4);
        e.configure_song_staging_for_transport(
            SongStagingConfig {
                preparations: 1,
                leases: 4,
                branches: 1,
                control_slots: 0,
                analysis_slots: 0,
                native_pcm_bytes: 0,
                critical_receipts: 1,
            },
            &acks,
        )
        .unwrap();
        let epoch = SnapshotEpoch(99);
        e.begin_song_preparation(SongStagePreparation {
            preparation: SongPreparation {
                epoch,
                branches: 1,
                resources: 4,
                required: SongHostCapacities {
                    sample_rate: 48000,
                    ..Default::default()
                },
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        })
        .unwrap();
        for (id, kind) in [
            (1, SongResourceKind::Instrument),
            (2, SongResourceKind::PrivateFx),
            (3, SongResourceKind::Track),
            (4, SongResourceKind::Master),
        ] {
            let resource = SongResourceRef { id, generation: 8 };
            let lease = SongLeaseKey {
                epoch,
                resource,
                kind,
            };
            e.reserve_song_resource(SongResourceReservation {
                epoch,
                resource,
                kind,
            })
            .unwrap();
            let payload = if kind == SongResourceKind::Instrument {
                NativeInstall::Inst {
                    resource: id,
                    gen: 8,
                    template: Template::from_inst(
                        &InstDef {
                            id: InstId::new(501),
                            params: Box::new([]),
                            nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
                            edges: Box::new([]),
                            node_params: Box::new([]),
                        },
                        &e.build_env(),
                    )
                    .unwrap(),
                }
            } else {
                NativeInstall::Bus {
                    resource: id,
                    gen: 8,
                    master: kind == SongResourceKind::Master,
                    template: Box::new(
                        BusTemplate::from_def(&BusDef {
                            id: BusId::new(id),
                            chain: Box::new([]),
                        })
                        .unwrap(),
                    ),
                }
            };
            assert!(e
                .stage_song_native(NativeSongInstall { lease, payload })
                .is_ok());
        }
        e.stage_song_reusable_branch(SongReusableBranch {
            config: SongBranchConfig {
                epoch,
                branch: SongBranchId(7),
                generation: 1,
                family: 5,
                track: 3,
                instrument: SongResourceRef {
                    id: 1,
                    generation: 8,
                },
                private_fx: Some(SongResourceRef {
                    id: 2,
                    generation: 8,
                }),
                track_template: Some(SongResourceRef {
                    id: 3,
                    generation: 8,
                }),
                master: Some(SongResourceRef {
                    id: 4,
                    generation: 8,
                }),
                transition_frame: 0,
                tail_deadline: 0,
            },
            last_generation: 2,
        })
        .unwrap();
        e.seal_song_preparation(epoch).unwrap();
        e.activate_song_epoch(epoch).unwrap();
        e.end_song_branches(epoch, Some((SongBranchId(7), 1)), 0)
            .unwrap();
        e.finish_song_deadlines(0);
        e
    }
    #[test]
    fn real_private_slot_users_block_rebind_without_an_old_voice() {
        let mut e = physical_reuse_engine();
        let b = e.song_runtime.as_ref().unwrap().branches[0];
        let fx = b.fx.unwrap();
        e.buses.slots[fx].users = 1;
        assert!(e
            .song_runtime
            .as_ref()
            .unwrap()
            .voices
            .iter()
            .all(Option::is_none));
        let request = SongBranchRebind {
            epoch: b.config.epoch,
            branch: b.config.branch,
            expected_generation: 1,
            generation: 2,
            transition_frame: 0,
            tail_deadline: 100,
        };
        assert_eq!(
            e.rebind_song_branch(request, 0),
            Err(SongRejectCode::NotReady)
        );
        assert_eq!(e.buses.slots[fx].users, 1);
        assert_eq!(
            e.song_runtime.as_ref().unwrap().branches[0]
                .config
                .generation,
            1
        );
        e.buses.slots[fx].users = 0;
        assert_eq!(e.rebind_song_branch(request, 0).unwrap().generation, 2);
    }
}
