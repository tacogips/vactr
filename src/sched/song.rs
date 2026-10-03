//! Owned finite song progression shared by playback and streaming export.
use crate::host::caps::{
    AudioHost, SongCommandRefusal, SongHostPreparation, SongPreparationProgress, SongReadyBundle,
    SongReplacementCommit, SongReplacementCommitRefusal, SongReplacementPreparationRefusal,
    SongReplacementProgress, SongReplacementSource, SongReplacementSourceRefusal, SongSubmitError,
};
use crate::pattern::TimeSpan;
use crate::song::routing::{
    self, SongActivation, SongCommand, SongEndpoints, SongHostAck, SongHostClock, SongInitialMute,
    SongMute, SongReplacement,
};
use crate::song::{PreparedSong, SnapshotEpoch, SongLimits, SongSettings};
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};
use std::collections::VecDeque;
mod encode;
mod frames;
mod pools;
mod receipts;

/// Submission and guarded retirement are distinct from audio-side application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongTransportState {
    Prepared,
    Playing,
    Draining,
    Ended,
    Failed,
}
/// Construction failure retains the sole original prepared authority.
pub struct SongTransportRefusal {
    pub ready: SongReadyBundle,
    pub failure: Failure,
}
/// Progress reports accepted command count, never inferred audio commitment.
pub struct SongTransportProgress {
    pub state: SongTransportState,
    pub committed: usize,
}
/// Bounded control-thread finite playback owner.
pub struct SongTransport {
    ready: Option<SongReadyBundle>,
    cleanup: Option<SongHostPreparation>,
    pools: pools::PoolBook,
    pending: VecDeque<Box<SongCommand>>,
    limits: SongLimits,
    sample_rate: u32,
    map: frames::FrameMap,
    settings: SongSettings,
    epoch: SnapshotEpoch,
    activation: SongActivation,
    applied: Option<SongActivation>,
    endpoints: SongEndpoints,
    duration: Ratio64,
    cursor: Ratio64,
    state: SongTransportState,
    failure: Option<Failure>,
    activation_submitted: bool,
    endpoints_submitted: bool,
    replacement: Option<SongReplacement>,
    source_issued: bool,
}
fn failure(message: &str) -> Failure {
    Failure::new(FailCode::BeyondCapability, message)
}
impl SongTransport {
    /// Consumes the original Ready bundle without copying its evaluator or leases.
    #[allow(clippy::result_large_err)]
    pub fn new(
        ready: SongReadyBundle,
        activation_frame: u64,
        limits: SongLimits,
    ) -> Result<Self, SongTransportRefusal> {
        let settings = ready.prepared().snapshot().settings();
        let duration = ready.prepared().snapshot().duration();
        let epoch = ready.prepared().epoch();
        let map = match frames::FrameMap::new(
            settings,
            duration,
            activation_frame,
            ready.clock().sample_rate,
            limits,
        ) {
            Ok(map) => map,
            Err(failure) => return Err(SongTransportRefusal { ready, failure }),
        };
        let endpoints = match map.endpoints(epoch) {
            Ok(endpoints) => endpoints,
            Err(failure) => return Err(SongTransportRefusal { ready, failure }),
        };
        let sample_rate = ready.clock().sample_rate;
        let pools = pools::PoolBook::new(ready.pools(), limits.max_cached_events as usize);
        Ok(Self {
            ready: Some(ready),
            cleanup: None,
            pools,
            pending: VecDeque::new(),
            limits,
            sample_rate,
            map,
            settings,
            epoch,
            activation: SongActivation {
                epoch,
                frame: activation_frame,
            },
            applied: None,
            endpoints,
            duration,
            cursor: Ratio64::ZERO,
            state: SongTransportState::Prepared,
            failure: None,
            activation_submitted: false,
            endpoints_submitted: false,
            replacement: None,
            source_issued: false,
        })
    }
    /// Posts the exact activation intent; refusal preserves its complete POD.
    #[allow(clippy::result_large_err)]
    pub fn submit_activation(
        &mut self,
        host: &mut dyn AudioHost,
    ) -> Result<(), SongCommandRefusal> {
        let Some(ready) = &mut self.ready else {
            return Err(SongCommandRefusal {
                command: SongCommand::Activate(self.activation),
                error: SongSubmitError::Invalid(failure("transport no longer owns Ready")),
            });
        };
        ready.submit_activation(host, self.activation)?;
        self.activation_submitted = true;
        Ok(())
    }
    /// Issues the actual old endpoint/activation once, including natural Ended.
    pub fn replacement_source(&mut self) -> Result<SongReplacementSource, Failure> {
        if self.source_issued
            || !self.endpoints_submitted
            || !matches!(
                self.state,
                SongTransportState::Playing
                    | SongTransportState::Draining
                    | SongTransportState::Ended
            )
        {
            return Err(failure("old transport cannot issue replacement authority"));
        }
        let activation = self
            .applied_activation()
            .ok_or_else(|| failure("old activation not acknowledged"))?;
        self.source_issued = true;
        Ok(SongReplacementSource::issued(activation, self.endpoints))
    }
    /// Restores one-shot issuance by consuming this owner's exact returned ticket.
    pub fn reclaim_replacement_source(
        &mut self,
        source: SongReplacementSource,
    ) -> Result<(), SongReplacementSourceRefusal> {
        if !self.source_issued
            || !self.endpoints_submitted
            || source.activation().epoch != self.epoch
            || self.applied_activation() != Some(source.activation())
            || self.endpoints != source.endpoints()
            || !matches!(
                self.state,
                SongTransportState::Playing
                    | SongTransportState::Draining
                    | SongTransportState::Ended
            )
        {
            return Err(SongReplacementSourceRefusal {
                source,
                failure: failure("foreign or changed old replacement authority"),
            });
        }
        self.source_issued = false;
        Ok(())
    }
    /// Recovery does not infer cancellation from a local transport failure.
    pub fn take_cancelled_replacement_source(
        &mut self,
    ) -> Result<Option<SongReplacementSource>, Failure> {
        match &mut self.cleanup {
            Some(cleanup) => cleanup.take_cancelled_replacement_source(),
            None => Ok(None),
        }
    }
    #[allow(clippy::result_large_err)] // Retains the original old ticket and overlay.
    pub fn prepare_replacement(
        &mut self,
        source: SongReplacementSource,
        overlay_nonce: u64,
        overlay: Vec<SongInitialMute>,
    ) -> Result<(), SongReplacementPreparationRefusal> {
        let count = u32::try_from(overlay.len());
        let overflow = count.is_err();
        let replacement = SongReplacement {
            activation: self.activation,
            previous: source.activation().epoch,
            overlay_nonce,
            overlay_count: count.unwrap_or(0),
        };
        if overflow
            || self.activation_submitted
            || self.replacement.is_some()
            || self.ready.is_none()
        {
            return Err(SongReplacementPreparationRefusal {
                replacement,
                source,
                overlay,
                failure: failure("invalid replacement transport extent or state"),
            });
        }
        // The actual Ready owner proves family/key adoption before any host command.
        let Some(ready) = &mut self.ready else {
            return Err(SongReplacementPreparationRefusal {
                replacement,
                source,
                overlay,
                failure: failure("replacement Ready absent"),
            });
        };
        ready.prepare_replacement(replacement, source, overlay)?;
        self.replacement = Some(replacement);
        Ok(())
    }
    #[allow(clippy::result_large_err)]
    pub fn submit_replacement(
        &mut self,
        host: &mut dyn AudioHost,
    ) -> Result<SongReplacementProgress, SongCommandRefusal> {
        let Some(ready) = &mut self.ready else {
            return Err(SongCommandRefusal {
                command: self
                    .replacement
                    .map_or(SongCommand::Activate(self.activation), SongCommand::Replace),
                error: SongSubmitError::Invalid(failure(
                    "transport no longer owns replacement Ready",
                )),
            });
        };
        let progress = ready.submit_replacement(host)?;
        if progress == SongReplacementProgress::Submitted {
            self.activation_submitted = true;
        }
        Ok(progress)
    }
    pub fn take_replacement_commit(&mut self) -> Option<SongReplacementCommit> {
        if let Some(ready) = &mut self.ready {
            ready.take_replacement_commit()
        } else {
            self.cleanup.as_mut()?.take_replacement_commit()
        }
    }
    /// Document invalidation requests cancellation; admission is not arm evidence.
    pub fn invalidate_replacement(&mut self) -> Result<(), Failure> {
        if let Some(ready) = &mut self.ready {
            ready.invalidate_replacement()?;
        } else {
            self.cleanup
                .as_mut()
                .ok_or_else(|| failure("replacement owner absent"))?
                .invalidate_replacement()?;
        }
        self.fail_and_retire(failure("replacement document invalidated"));
        Ok(())
    }
    /// Observes only a genuine compound outcome; never queues another Endpoints.
    #[allow(clippy::result_large_err)] // Refusal preserves the actual one-shot proof.
    pub fn observe_replacement(
        &mut self,
        commit: SongReplacementCommit,
    ) -> Result<(), SongReplacementCommitRefusal> {
        if commit.replacement().previous != self.epoch
            || commit.previous_endpoints() != self.endpoints
            || self.applied_activation() != Some(commit.previous_activation())
        {
            return Err(SongReplacementCommitRefusal {
                commit,
                failure: failure("foreign compound replacement outcome"),
            });
        }
        let endpoints = commit.cutoff();
        if endpoints.arrangement < self.activation.frame {
            return Err(SongReplacementCommitRefusal {
                commit,
                failure: failure("replacement precedes old activation"),
            });
        }
        self.pending.retain(|pending| match **pending {
            SongCommand::Endpoints(_) => false,
            SongCommand::Event(e) => e.frame < endpoints.arrangement,
            SongCommand::Release(e) => e.frame < endpoints.arrangement,
            SongCommand::RebindBranch(e) => e.transition_frame < endpoints.arrangement,
            _ => true,
        });
        self.endpoints = endpoints;
        self.endpoints_submitted = true;
        self.cursor = self.duration;
        self.begin_retirement();
        Ok(())
    }
    /// Receipts not owned by this transport are returned unchanged to the dispatcher.
    pub fn receive(&mut self, ack: SongHostAck) -> Result<(), SongHostAck> {
        receipts::receive(self, ack)
    }
    /// Advances one bounded canonical query, retaining exact refused submissions.
    pub fn advance(
        &mut self,
        host: &mut dyn AudioHost,
        clock: SongHostClock,
        through: TimeSpan,
    ) -> Result<SongTransportProgress, Failure> {
        if clock.sample_rate != self.sample_rate
            || through.begin < Ratio64::ZERO
            || through.end < through.begin
        {
            return Err(failure("invalid finite transport horizon or host rate"));
        }
        if let Some(cleanup) = &mut self.cleanup {
            if self.failure.is_none() && clock.frame >= self.endpoints.arrangement {
                self.state = SongTransportState::Draining;
            }
            let progress = cleanup.submit(host)?;
            if progress == SongPreparationProgress::Retired && self.failure.is_none() {
                self.state = SongTransportState::Ended;
            }
            return Ok(SongTransportProgress {
                state: self.state,
                committed: 0,
            });
        }
        if !self.activation_submitted {
            return Err(failure("activation was not submitted"));
        }
        let mut committed = self.flush(host)?;
        if !self.pending.is_empty() {
            return Ok(SongTransportProgress {
                state: self.state,
                committed,
            });
        }
        if !self.endpoints_submitted {
            self.pending
                .push_back(Box::new(SongCommand::Endpoints(self.endpoints)));
            committed += self.flush(host)?;
            if !self.pending.is_empty() {
                return Ok(SongTransportProgress {
                    state: self.state,
                    committed,
                });
            }
            self.endpoints_submitted = true;
        }
        if self.failure.is_none() && self.cursor < through.end.min(self.duration) {
            let next_integer = self
                .cursor
                .num()
                .div_euclid(self.cursor.den())
                .checked_add(1)
                .ok_or_else(|| failure("canonical cycle overflow"))?;
            let end = through
                .end
                .min(self.duration)
                .min(Ratio64::from_int(next_integer));
            if let Err(error) = self.realize(end) {
                self.fail_and_retire(error.clone());
                return Err(error);
            }
            committed += self.flush(host)?;
        }
        if self.pending.is_empty() && (self.cursor == self.duration || self.failure.is_some()) {
            self.begin_retirement();
        }
        Ok(SongTransportProgress {
            state: self.state,
            committed,
        })
    }
    fn flush(&mut self, host: &mut dyn AudioHost) -> Result<usize, Failure> {
        let mut accepted = 0;
        while let Some(command) = self.pending.front().map(|p| **p) {
            let initial = match command {
                SongCommand::Event(e) => self.ready.as_ref().is_some_and(|r| {
                    r.pools().iter().any(|p| {
                        p.initial.config.branch == e.branch
                            && p.initial.config.generation == e.generation
                    })
                }),
                SongCommand::Release(e) => self.ready.as_ref().is_some_and(|r| {
                    r.pools().iter().any(|p| {
                        p.initial.config.branch == e.branch
                            && p.initial.config.generation == e.generation
                    })
                }),
                SongCommand::Endpoints(_) => true,
                _ => false,
            };
            let result = if initial {
                self.ready
                    .as_mut()
                    .ok_or_else(|| failure("initial Ready authority missing"))?
                    .submit_initial_command(host, command)
            } else {
                host.try_song_command(command)
            };
            match result {
                Ok(()) => {
                    self.pending.pop_front();
                    if matches!(command, SongCommand::Endpoints(_)) {
                        self.endpoints_submitted = true;
                    }
                    accepted += 1;
                }
                Err(refusal) => {
                    if refusal.command != command {
                        return Err(failure("host changed refused command"));
                    }
                    match refusal.error {
                        SongSubmitError::Backpressure => break,
                        SongSubmitError::Unavailable => {
                            let error = failure("song host unavailable");
                            self.fail_and_retire(error.clone());
                            return Err(error);
                        }
                        SongSubmitError::Invalid(error) => {
                            self.fail_and_retire(error.clone());
                            return Err(error);
                        }
                    }
                }
            }
        }
        Ok(accepted)
    }
    fn realize(&mut self, end: Ratio64) -> Result<(), Failure> {
        let ready = self
            .ready
            .as_mut()
            .ok_or_else(|| failure("Ready query authority missing"))?;
        let mut rows = ready.query(TimeSpan::new(self.cursor, end)?, &self.limits)?;
        self.limits.check_events(rows.len())?;
        rows.sort_by(|a, b| {
            a.whole
                .unwrap_or(a.part)
                .begin
                .cmp(&b.whole.unwrap_or(b.part).begin)
                .then_with(|| a.handle.cmp(&b.handle))
        });
        rows.dedup_by(|a, b| a.handle == b.handle);
        let count = u32::try_from(rows.len()).map_err(|_| failure("event work count overflow"))?;
        let allowance = self.limits.max_nodes / count.max(1);
        if allowance == 0 {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "route batch work exhausted",
            ));
        }
        for row in rows {
            let onset = row.whole.unwrap_or(row.part).begin;
            if onset < self.cursor || onset >= end || onset >= self.duration {
                continue;
            }
            let mut route_limits = self.limits;
            route_limits.max_nodes = allowance;
            let route = routing::resolve_route(ready.routes(), &row, route_limits)?;
            let branch = ready
                .routes()
                .branches
                .iter()
                .find(|b| b.id == route.branch)
                .ok_or_else(|| failure("logical song route missing"))?;
            let frame = self.map.at(onset)?;
            let component_end = route.configuration.end.min(self.duration);
            let assignment = self.pools.assign(
                route,
                frame,
                self.map.at(component_end)?,
                self.map.deadline(component_end)?,
            )?;
            let event = encode::encode(
                ready,
                &row,
                branch,
                assignment.physical,
                assignment.generation,
                frame,
                self.settings.seconds_per_cycle()?,
            )?;
            if let Some(rebind) = assignment.rebind {
                self.pending
                    .push_back(Box::new(SongCommand::RebindBranch(rebind)));
            }
            if let Some(release) = assignment.release {
                self.pending
                    .push_back(Box::new(SongCommand::Release(release)));
            }
            self.pending.push_back(Box::new(SongCommand::Event(event)));
        }
        self.cursor = end;
        Ok(())
    }
    fn fail_and_retire(&mut self, error: Failure) {
        self.pending.clear();
        self.failure = Some(error);
        self.state = SongTransportState::Failed;
        if self.activation_submitted && !self.endpoints_submitted && self.ready.is_some() {
            self.pending
                .push_back(Box::new(SongCommand::Endpoints(self.endpoints)));
        } else {
            self.begin_retirement();
        }
    }
    /// Stops new realization while retaining actual cancellation or retirement work.
    pub fn abort(&mut self, error: Failure) {
        self.fail_and_retire(error);
    }
    fn begin_retirement(&mut self) {
        if let Some(ready) = self.ready.take() {
            self.applied = ready.applied_activation();
            self.cleanup = Some(SongHostPreparation::retire(ready));
        }
    }
    /// Stops new onsets at an exact admitted boundary; refusal leaves ownership intact.
    #[allow(clippy::result_large_err)]
    pub fn cutoff(
        &mut self,
        host: &mut dyn AudioHost,
        endpoints: SongEndpoints,
    ) -> Result<(), SongCommandRefusal> {
        let command = SongCommand::Endpoints(endpoints);
        if endpoints.epoch != self.epoch
            || endpoints.arrangement < self.activation.frame
            || endpoints.arrangement > self.endpoints.arrangement
            || endpoints.tail_deadline < endpoints.arrangement
            || endpoints.tail_deadline > self.endpoints.tail_deadline
        {
            return Err(SongCommandRefusal {
                command,
                error: SongSubmitError::Invalid(failure("invalid finite transport cutoff")),
            });
        }
        if let Some(ready) = &mut self.ready {
            ready.submit_initial_command(host, command)?;
        } else {
            host.try_song_command(command)?;
        }
        self.pending.retain(|pending| match **pending {
            SongCommand::Endpoints(_) => false,
            SongCommand::Event(event) => event.frame < endpoints.arrangement,
            SongCommand::Release(release) => release.frame < endpoints.arrangement,
            SongCommand::RebindBranch(rebind) => rebind.transition_frame < endpoints.arrangement,
            _ => true,
        });
        self.endpoints = endpoints;
        self.endpoints_submitted = true;
        self.cursor = self.duration;
        if self.pending.is_empty() {
            self.begin_retirement();
        }
        Ok(())
    }
    /// Schedules a full epoch-qualified mute without changing query authority.
    #[allow(clippy::result_large_err)]
    pub fn mute(
        &mut self,
        host: &mut dyn AudioHost,
        mute: SongMute,
    ) -> Result<(), SongCommandRefusal> {
        if mute.epoch != self.epoch {
            return Err(SongCommandRefusal {
                command: SongCommand::Mute(mute),
                error: SongSubmitError::Invalid(failure("foreign transport mute")),
            });
        }
        host.try_song_command(SongCommand::Mute(mute))
    }
    pub fn state(&self) -> SongTransportState {
        self.state
    }
    pub fn epoch(&self) -> SnapshotEpoch {
        self.epoch
    }
    pub fn settings(&self) -> SongSettings {
        self.settings
    }
    pub fn endpoints(&self) -> SongEndpoints {
        self.endpoints
    }
    pub fn applied_activation(&self) -> Option<SongActivation> {
        self.ready
            .as_ref()
            .and_then(|r| r.applied_activation())
            .or(self.applied)
    }
    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }
    /// Returns the original rejected/unactivated owner after real cancellation.
    pub fn take_cancelled(&mut self) -> Result<PreparedSong, Failure> {
        self.cleanup
            .as_mut()
            .ok_or_else(|| failure("transport has not entered cleanup"))?
            .take_cancelled()
    }
    /// Returns the original prepared owner only after actual guarded retirement.
    pub fn take_retired(&mut self) -> Result<PreparedSong, Failure> {
        self.cleanup
            .as_mut()
            .ok_or_else(|| failure("transport has not entered retirement"))?
            .take_retired()
    }
}
