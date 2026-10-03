//! Bounded read-only future generation admission before timed queue insertion.
use super::*;
use crate::song::routing::SongRejectCode;

fn timed_order(command: SongCommand) -> Option<(u64, u8)> {
    let frame = match command {
        SongCommand::Release(r) => r.frame,
        SongCommand::Endpoints(e) => e.arrangement,
        SongCommand::RebindBranch(r) => r.transition_frame,
        SongCommand::Activate(a) => a.frame,
        SongCommand::Mute(m) => m.frame,
        SongCommand::Event(e) => e.frame,
        _ => return None,
    };
    Some((frame, crate::dsp::song::SongRuntime::command_rank(command)))
}
impl Engine {
    pub(in crate::dsp) fn song_runtime_record(
        &mut self,
        command: SongCommand,
        acks: &mut AckProducer,
    ) -> bool {
        if let SongCommand::CancelPreparation(epoch) = command {
            let queued_activation = self.song_runtime.as_ref().and_then(|r| {
                if r.owns_epoch(epoch) {
                    return None;
                }
                r.commands.iter().find_map(|c| match c.command {
                    SongCommand::Activate(a) if a.epoch == epoch => Some(a),
                    _ => None,
                })
            });
            if queued_activation.is_some()
                || self.song_runtime.as_ref().is_some_and(|r| {
                    r.replacements
                        .iter()
                        .any(|x| x.command.activation.epoch == epoch)
                        || r.primed_mutes.iter().any(|m| m.epoch == epoch)
                })
            {
                if let Err(reason) = self.cancel_song_replacement(epoch) {
                    self.song_ack = acks
                        .push_critical(HostMsg::Song(SongHostAck::Rejected { epoch, reason }))
                        .err();
                } else if let Some(activation) = queued_activation {
                    self.song_ack = acks
                        .push_critical(HostMsg::Song(SongHostAck::ActivationRejected {
                            activation,
                            reason: SongRejectCode::NotReady,
                        }))
                        .err();
                }
                return true;
            }
        }
        let is_runtime = matches!(
            command,
            SongCommand::Activate(_)
                | SongCommand::Replace(_)
                | SongCommand::PrimeMute(_)
                | SongCommand::Mute(_)
                | SongCommand::RebindBranch(_)
                | SongCommand::Event(_)
                | SongCommand::Release(_)
                | SongCommand::Endpoints(_)
        );
        let protected = self.song_runtime.as_ref().is_some_and(|r| match command {
            SongCommand::CancelPreparation(epoch) => r.owns_epoch(epoch),
            SongCommand::CancelLease(key) => {
                r.owns_epoch(key.epoch)
                    || r.replacements
                        .iter()
                        .any(|x| x.command.activation.epoch == key.epoch)
            }
            _ => false,
        });
        if !is_runtime && !protected {
            return false;
        }
        let result = if protected {
            Err(SongRejectCode::NotReady)
        } else {
            self.queue_song_runtime(command)
        };
        if let Err(reason) = result {
            let receipt = match command {
                SongCommand::Replace(r) => SongHostAck::ActivationRejected {
                    activation: r.activation,
                    reason,
                },
                SongCommand::Activate(activation) => {
                    SongHostAck::ActivationRejected { activation, reason }
                }
                _ => SongHostAck::Rejected {
                    epoch: command.epoch(),
                    reason,
                },
            };
            self.song_ack = acks.push_critical(HostMsg::Song(receipt)).err();
        }
        true
    }
    pub(in crate::dsp) fn queue_song_runtime(
        &mut self,
        command: SongCommand,
    ) -> Result<(), SongRejectCode> {
        match command {
            SongCommand::PrimeMute(m) => return self.prime_song_replacement(m),
            SongCommand::Replace(r) => return self.queue_song_replacement(r),
            _ => {}
        }
        let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let preparation = stager
            .preparations
            .iter()
            .find(|p| p.command.preparation.epoch == command.epoch())
            .ok_or(if matches!(command, SongCommand::Activate(_)) {
                SongRejectCode::NotReady
            } else {
                SongRejectCode::StaleEpoch
            })?;
        if !preparation.ready || preparation.cancelling {
            return Err(SongRejectCode::NotReady);
        }
        let runtime = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        if let SongCommand::Activate(a) = command {
            if preparation.active
                || runtime
                    .commands
                    .iter()
                    .any(|c| matches!(c.command, SongCommand::Activate(b) if a.epoch == b.epoch))
            {
                return Err(SongRejectCode::NotReady);
            }
        }
        if let Some(record) = runtime
            .replacements
            .iter()
            .find(|x| x.command.activation.epoch == command.epoch())
        {
            if matches!(
                record.phase,
                crate::dsp::song::SongReplacementPhase::Rejected
            ) || timed_order(command)
                .is_some_and(|(frame, _)| frame < record.command.activation.frame)
                || matches!(command, SongCommand::Activate(_))
            {
                return Err(SongRejectCode::NotReady);
            }
        }
        self.validate_queued_song_generation(command)?;
        let runtime = self.song_runtime.as_mut().ok_or(SongRejectCode::NotReady)?;
        let reserves_endpoint = matches!(
            command,
            SongCommand::Activate(_) | SongCommand::Endpoints(_)
        );
        let newly_reserved = reserves_endpoint && !runtime.has_endpoint_obligation(command.epoch());
        if reserves_endpoint {
            runtime.reserve_endpoint(command.epoch())?;
        }
        if let Err(reason) = runtime.queue(command, self.frame) {
            if newly_reserved {
                runtime.release_endpoint_reservation(command.epoch());
            }
            return Err(reason);
        }
        if let SongCommand::Endpoints(e) = command {
            if let Some(owner) = runtime
                .last_applied_owner
                .as_mut()
                .filter(|p| p.activation.epoch == e.epoch)
            {
                owner.endpoint = Some(e);
            }
        }
        Ok(())
    }
    fn validate_queued_song_generation(&self, command: SongCommand) -> Result<(), SongRejectCode> {
        let SongCommand::Event(event) = command else {
            return Ok(());
        };
        let (epoch, branch, requested) = (event.epoch, event.branch, event.generation);
        if !command.valid() || timed_order(command).is_none_or(|o| o.0 < self.frame) {
            return Err(SongRejectCode::Malformed);
        }
        let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let runtime = self.song_runtime.as_ref().ok_or(SongRejectCode::NotReady)?;
        let config = stager
            .branches
            .iter()
            .find(|b| b.epoch == epoch && b.branch == branch)
            .ok_or(SongRejectCode::StaleEpoch)?;
        let slot = runtime
            .reusable_slot(epoch, branch)
            .or_else(|| stager.reusable_slot(epoch, branch));
        let Some(slot) = slot else {
            return if config.generation == requested {
                Ok(())
            } else {
                Err(SongRejectCode::StaleEpoch)
            };
        };
        let live = runtime
            .branches
            .iter()
            .find(|b| b.config.epoch == epoch && b.config.branch == branch);
        if live.is_some_and(|b| b.config.generation != config.generation) {
            return Err(SongRejectCode::StaleEpoch);
        }
        if runtime.endpoints.iter().any(|e| e.epoch == epoch) {
            return Err(SongRejectCode::NotReady);
        }
        let config = live.map_or(*config, |branch| branch.config);
        let mut generation = config.generation;
        let mut born = config.transition_frame;
        let mut deadline = config.tail_deadline;
        let mut ended = live.is_some_and(|b| b.ended);
        let target = timed_order(command).ok_or(SongRejectCode::Malformed)?;
        for queued in &runtime.commands {
            let order = timed_order(queued.command).ok_or(SongRejectCode::Malformed)?;
            if order > target {
                break;
            }
            match queued.command {
                SongCommand::Endpoints(e) if e.epoch == epoch => {
                    return Err(queued.failure.unwrap_or(SongRejectCode::NotReady));
                }
                SongCommand::Release(r) if r.epoch == epoch && r.branch == branch => {
                    if let Some(error) = queued.failure {
                        return Err(error);
                    }
                    if !SongCommand::Release(r).valid() {
                        return Err(SongRejectCode::Malformed);
                    }
                    if r.generation != generation {
                        return Err(SongRejectCode::StaleEpoch);
                    }
                    ended = true;
                    deadline = r.tail_deadline;
                }
                SongCommand::RebindBranch(r) if r.epoch == epoch && r.branch == branch => {
                    if let Some(error) = queued.failure {
                        return Err(error);
                    }
                    if !r.valid() {
                        return Err(SongRejectCode::Malformed);
                    }
                    if r.expected_generation != generation {
                        return Err(SongRejectCode::StaleEpoch);
                    }
                    if r.generation > slot.last_generation {
                        return Err(SongRejectCode::Capacity);
                    }
                    if !ended || r.transition_frame < deadline {
                        return Err(SongRejectCode::NotReady);
                    }
                    generation = r.generation;
                    born = r.transition_frame;
                    deadline = r.tail_deadline;
                    ended = false;
                }
                _ => {}
            }
        }
        if requested != generation {
            return Err(SongRejectCode::StaleEpoch);
        }
        if target.0 < born {
            return Err(SongRejectCode::NotReady);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
