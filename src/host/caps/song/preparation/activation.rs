//! Original activation and retained compound replacement ownership.
use super::*;

pub(super) fn receive_activation(
    prepared: &mut PreparedSong,
    cleanup: &mut SongPreparationCleanup,
    ack: SongHostAck,
) -> Result<(), SongHostAck> {
    let ActivationState::PendingExclusive(requested) = cleanup.activation else {
        return Err(ack);
    };
    match ack {
        SongHostAck::Applied(actual)
            if actual.epoch == requested.epoch
                && (if cleanup.replacement.as_ref().is_some_and(|p| p.submitted) {
                    actual.frame == requested.frame
                } else {
                    actual.frame >= requested.frame
                }) =>
        {
            prepared
                .acknowledge_applied(SongApplyAck {
                    epoch: actual.epoch,
                    doc_revision: prepared.revision(),
                    application_frame: actual.frame,
                })
                .map_err(|_| ack)?;
            cleanup.activation = ActivationState::Applied(actual);
            if let Some(p) = &mut cleanup.replacement {
                p.applied = true;
            }
            Ok(())
        }
        SongHostAck::ActivationRejected { activation, reason } if activation == requested => {
            cleanup.activation = ActivationState::RejectedNeverActivated(reason);
            Ok(())
        }
        _ => Err(ack),
    }
}
impl SongReadyBundle {
    pub fn applied_activation(&self) -> Option<SongActivation> {
        match self.cleanup.activation {
            ActivationState::Applied(actual) => Some(actual),
            _ => None,
        }
    }
    pub fn activation_rejection(&self) -> Option<SongRejectCode> {
        match self.cleanup.activation {
            ActivationState::RejectedNeverActivated(reason) => Some(reason),
            _ => None,
        }
    }
    /// Last preparation barrier nonce; later observers must advance it.
    pub fn clock_request_floor(&self) -> u64 {
        self.cleanup.nonce
    }
    #[allow(clippy::result_large_err)]
    pub fn submit_activation(
        &mut self,
        host: &mut dyn AudioHost,
        activation: SongActivation,
    ) -> Result<(), SongCommandRefusal> {
        let command = SongCommand::Activate(activation);
        if activation.epoch != self.prepared.epoch()
            || self.cleanup.replacement.is_some()
            || !matches!(self.cleanup.activation, ActivationState::NeverSubmitted)
        {
            return Err(SongCommandRefusal {
                command,
                error: SongSubmitError::Invalid(fail("invalid or repeated candidate activation")),
            });
        }
        host.try_song_command(command)?;
        self.cleanup.activation = ActivationState::PendingExclusive(activation);
        Ok(())
    }
    /// Enqueues authentic initial transitions without waiting past musical zero.
    #[allow(clippy::result_large_err)] // Refusal preserves the exact submitted POD.
    pub fn submit_initial_command(
        &mut self,
        host: &mut dyn AudioHost,
        command: SongCommand,
    ) -> Result<(), SongCommandRefusal> {
        let activation = match self.cleanup.activation {
            ActivationState::PendingExclusive(a) | ActivationState::Applied(a) => Some(a),
            _ => None,
        };
        let valid = activation.is_some_and(|a| {
            if command.epoch() != a.epoch || !command.valid() {
                return false;
            }
            let (branch, generation, frame, inst) = match command {
                SongCommand::Endpoints(e) => return e.arrangement >= a.frame,
                SongCommand::Event(e) => (e.branch, e.generation, e.frame, Some(e.event.inst)),
                SongCommand::Release(r) => (r.branch, r.generation, r.frame, None),
                _ => return false,
            };
            self.pools.iter().any(|pool| {
                let b = pool.initial.config;
                if branch != b.branch || generation != b.generation || frame < a.frame {
                    return false;
                }
                let keys = [
                    Some((b.instrument, SongResourceKind::Instrument)),
                    b.private_fx.map(|r| (r, SongResourceKind::PrivateFx)),
                    b.track_template.map(|r| (r, SongResourceKind::Track)),
                    b.master.map(|r| (r, SongResourceKind::Master)),
                ];
                keys.iter().all(|key| {
                    key.is_some_and(|(resource, kind)| {
                        self.resources.contains(&SongLeaseKey {
                            epoch: a.epoch,
                            resource,
                            kind,
                        })
                    })
                }) && self.cleanup.assembly.resources.iter().any(|r| {
                    r.key
                        == SongLeaseKey {
                            epoch: a.epoch,
                            resource: b.instrument,
                            kind: SongResourceKind::Instrument,
                        }
                        && r.ready
                        && !r.returned
                        && inst.is_none_or(|inst| {
                            matches!(
                                &r.upload,
                                Some(Upload::Graph { graph: GraphHandle::Inst { id, .. }, .. })
                                    if *id == inst
                            )
                        })
                })
            })
        });
        if !valid {
            return Err(SongCommandRefusal {
                command,
                error: SongSubmitError::Invalid(fail("invalid initial song command authority")),
            });
        }
        host.try_song_command(command)
    }
    pub fn receive_activation(&mut self, ack: SongHostAck) -> Result<(), SongHostAck> {
        receive_activation(&mut self.prepared, &mut self.cleanup, ack)
    }
}

/// One accepted priming record or the admitted compound intent, never Applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongReplacementProgress {
    Priming,
    Submitted,
}
/// Issued only from an actually applied old transport; not Clone or Copy.
///
/// ```compile_fail
/// use vactr::host::caps::SongReplacementSource;
/// let _forged = SongReplacementSource {};
/// ```
pub struct SongReplacementSource {
    activation: SongActivation,
    endpoints: SongEndpoints,
}
impl SongReplacementSource {
    pub(crate) fn issued(activation: SongActivation, endpoints: SongEndpoints) -> Self {
        Self {
            activation,
            endpoints,
        }
    }
    pub fn activation(&self) -> SongActivation {
        self.activation
    }
    pub fn endpoints(&self) -> SongEndpoints {
        self.endpoints
    }
}
/// One-shot proof of exact actual replacement application.
///
/// ```compile_fail
/// use vactr::host::caps::SongReplacementCommit;
/// let _forged = SongReplacementCommit {};
/// ```
pub struct SongReplacementCommit {
    replacement: SongReplacement,
    source: SongReplacementSource,
}
impl SongReplacementCommit {
    pub fn replacement(&self) -> SongReplacement {
        self.replacement
    }
    pub fn previous_endpoints(&self) -> SongEndpoints {
        self.source.endpoints
    }
    pub(crate) fn previous_activation(&self) -> SongActivation {
        self.source.activation
    }
    pub fn cutoff(&self) -> SongEndpoints {
        SongEndpoints {
            epoch: self.replacement.previous,
            arrangement: self.replacement.activation.frame,
            tail_deadline: self.replacement.activation.frame,
        }
    }
}
/// Refusal returns the same one-shot outcome for the correct old owner.
pub struct SongReplacementCommitRefusal {
    pub commit: SongReplacementCommit,
    pub failure: Failure,
}
/// Refusal preserves the same opaque source for its authentic old owner.
pub struct SongReplacementSourceRefusal {
    pub source: SongReplacementSource,
    pub failure: Failure,
}
pub struct SongReplacementPreparationRefusal {
    pub replacement: SongReplacement,
    pub source: SongReplacementSource,
    pub overlay: Vec<SongInitialMute>,
    pub failure: Failure,
}
pub(super) struct PendingReplacement {
    replacement: SongReplacement,
    source: Option<SongReplacementSource>,
    overlay: Vec<SongInitialMute>,
    cursor: usize,
    pub(super) submitted: bool,
    pub(super) invalidated: bool,
    pub(super) cancel_posted: bool,
    applied: bool,
}
impl SongPreparationCleanup {
    fn take_replacement_commit(&mut self) -> Option<SongReplacementCommit> {
        let pending = self.replacement.as_mut()?;
        if !pending.applied {
            return None;
        }
        Some(SongReplacementCommit {
            replacement: pending.replacement,
            source: pending.source.take()?,
        })
    }
    fn take_cancelled_replacement_source(
        &mut self,
        completed: bool,
    ) -> Result<Option<SongReplacementSource>, Failure> {
        let Some(pending) = &self.replacement else {
            return Ok(None);
        };
        if pending.source.is_none()
            || pending.applied
            || !completed
            || !self.cancel_posted
            || !self.cancelled
            || self.flight.is_some()
            || self.upload.is_some()
            || !matches!(
                self.activation,
                ActivationState::NeverSubmitted | ActivationState::RejectedNeverActivated(_)
            )
            || (pending.submitted
                && !matches!(self.activation, ActivationState::RejectedNeverActivated(_)))
        {
            return Ok(None);
        }
        charge(&mut self.remaining, self.assembly.resources.len())?;
        if self
            .assembly
            .resources
            .iter()
            .any(|r| r.admitted && !r.returned)
        {
            return Ok(None);
        }
        Ok(self.replacement.as_mut().and_then(|p| p.source.take()))
    }
    fn invalidate_replacement(&mut self) -> Result<(), Failure> {
        let pending = self
            .replacement
            .as_mut()
            .ok_or_else(|| fail("no replacement intent"))?;
        if pending.applied {
            return Err(fail("applied replacement is no longer pending"));
        }
        pending.invalidated = true;
        Ok(())
    }
    pub(super) fn submit_replacement_cancel(
        &mut self,
        host: &mut dyn AudioHost,
    ) -> Result<(), Failure> {
        let Some(pending) = &mut self.replacement else {
            return Ok(());
        };
        if !pending.invalidated || !pending.submitted || pending.cancel_posted || pending.applied {
            return Ok(());
        }
        let command = SongCommand::CancelPreparation(pending.replacement.activation.epoch);
        match host.try_song_command(command) {
            Ok(()) => {
                pending.cancel_posted = true;
                Ok(())
            }
            Err(refusal) if refusal.command != command => {
                Err(fail("host changed cancellation intent"))
            }
            Err(SongCommandRefusal {
                error: SongSubmitError::Backpressure,
                ..
            }) => Ok(()),
            Err(SongCommandRefusal {
                error: SongSubmitError::Invalid(error),
                ..
            }) => Err(error),
            Err(_) => Err(fail("replacement cancellation host unavailable")),
        }
    }
}
impl SongHostPreparation {
    /// Returns the original old ticket only after genuine never-activated cleanup.
    pub fn take_cancelled_replacement_source(
        &mut self,
    ) -> Result<Option<SongReplacementSource>, Failure> {
        self.cleanup
            .take_cancelled_replacement_source(self.progress == SongPreparationProgress::Cancelled)
    }
    pub fn take_replacement_commit(&mut self) -> Option<SongReplacementCommit> {
        self.cleanup.take_replacement_commit()
    }
    pub fn invalidate_replacement(&mut self) -> Result<(), Failure> {
        self.cleanup.invalidate_replacement()
    }
}
impl SongReadyBundle {
    fn validate_replacement(
        &mut self,
        replacement: SongReplacement,
        source: &SongReplacementSource,
        overlay: &[SongInitialMute],
    ) -> Result<(), Failure> {
        if !SongCommand::Replace(replacement).valid()
            || replacement.activation.epoch != self.prepared.epoch()
            || self.cleanup.replacement.is_some()
            || !matches!(self.cleanup.activation, ActivationState::NeverSubmitted)
            || source.activation.epoch != replacement.previous
            || source.endpoints.epoch != replacement.previous
            || source.activation.frame > replacement.activation.frame
            || source.endpoints.arrangement > source.endpoints.tail_deadline
            || source.endpoints.arrangement < source.activation.frame
            || u32::try_from(overlay.len()).ok() != Some(replacement.overlay_count)
            || overlay.len() > self.cleanup.limits.max_resources as usize
            || overlay.len() > self.cleanup.limits.max_pending_records as usize
        {
            return Err(fail("invalid replacement owner or overlay extent"));
        }
        charge(&mut self.cleanup.remaining, overlay.len())?;
        for (index, mute) in overlay.iter().enumerate() {
            if mute.epoch != self.prepared.epoch()
                || mute.overlay_nonce != replacement.overlay_nonce
                || !SongCommand::PrimeMute(*mute).valid()
            {
                return Err(fail("foreign replacement overlay"));
            }
            for previous in &overlay[..index] {
                charge(&mut self.cleanup.remaining, 1)?;
                if previous.instrument == mute.instrument {
                    return Err(fail("duplicate replacement family"));
                }
            }
            let mut found = false;
            for pool in &self.pools {
                charge(&mut self.cleanup.remaining, 1)?;
                let branch = pool.initial.config;
                if branch.epoch != self.prepared.epoch() {
                    return Err(fail("foreign replacement pool"));
                }
                if branch.family != mute.instrument {
                    continue;
                }
                found = true;
                for (resource, kind) in [
                    (Some(branch.instrument), SongResourceKind::Instrument),
                    (branch.private_fx, SongResourceKind::PrivateFx),
                    (branch.track_template, SongResourceKind::Track),
                    (branch.master, SongResourceKind::Master),
                ] {
                    let resource =
                        resource.ok_or_else(|| fail("replacement branch missing graph"))?;
                    let key = SongLeaseKey {
                        epoch: self.prepared.epoch(),
                        resource,
                        kind,
                    };
                    let mut adopted = false;
                    for owner in &self.cleanup.assembly.resources {
                        charge(&mut self.cleanup.remaining, 1)?;
                        if owner.key == key {
                            adopted = owner.admitted && owner.ready && !owner.returned;
                            break;
                        }
                    }
                    if !adopted {
                        return Err(fail("replacement graph not ready"));
                    }
                }
            }
            if !found {
                return Err(fail("replacement family not installed"));
            }
        }
        Ok(())
    }
    #[allow(clippy::result_large_err)] // Refusal retains the original ticket and overlay.
    pub fn prepare_replacement(
        &mut self,
        replacement: SongReplacement,
        source: SongReplacementSource,
        overlay: Vec<SongInitialMute>,
    ) -> Result<(), SongReplacementPreparationRefusal> {
        if let Err(failure) = self.validate_replacement(replacement, &source, &overlay) {
            return Err(SongReplacementPreparationRefusal {
                replacement,
                source,
                overlay,
                failure,
            });
        }
        self.cleanup.replacement = Some(PendingReplacement {
            replacement,
            source: Some(source),
            overlay,
            cursor: 0,
            submitted: false,
            invalidated: false,
            cancel_posted: false,
            applied: false,
        });
        Ok(())
    }
    #[allow(clippy::result_large_err)]
    pub fn submit_replacement(
        &mut self,
        host: &mut dyn AudioHost,
    ) -> Result<SongReplacementProgress, SongCommandRefusal> {
        let Some(pending) = &mut self.cleanup.replacement else {
            return Err(SongCommandRefusal {
                command: SongCommand::Prepare(self.prepared.epoch()),
                error: SongSubmitError::Invalid(fail("replacement was not prepared")),
            });
        };
        if pending.invalidated {
            return Err(SongCommandRefusal {
                command: SongCommand::Replace(pending.replacement),
                error: SongSubmitError::Invalid(fail("replacement invalidated")),
            });
        }
        if pending.submitted {
            return Ok(SongReplacementProgress::Submitted);
        }
        let command = pending
            .overlay
            .get(pending.cursor)
            .map_or(SongCommand::Replace(pending.replacement), |m| {
                SongCommand::PrimeMute(*m)
            });
        if let Err(refusal) = host.try_song_command(command) {
            return if refusal.command == command {
                Err(refusal)
            } else {
                Err(SongCommandRefusal {
                    command,
                    error: SongSubmitError::Invalid(fail("host changed replacement intent")),
                })
            };
        }
        if pending.cursor < pending.overlay.len() {
            pending.cursor += 1;
            Ok(SongReplacementProgress::Priming)
        } else {
            pending.submitted = true;
            self.cleanup.activation =
                ActivationState::PendingExclusive(pending.replacement.activation);
            Ok(SongReplacementProgress::Submitted)
        }
    }
    pub fn take_replacement_commit(&mut self) -> Option<SongReplacementCommit> {
        self.cleanup.take_replacement_commit()
    }
    pub fn invalidate_replacement(&mut self) -> Result<(), Failure> {
        self.cleanup.invalidate_replacement()
    }
}
