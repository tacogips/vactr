//! Genuine command and acknowledgment pressure through original controller.
use super::*;

#[test]
fn pending_mute_and_real_host_pressure_serialize_repeated_apply() {
    for bytes in [false, true] {
        let mut pair = Controller::new(bytes);
        pair.apply(11, tone(440, "8", false), 1, 0);
        let old = pair.wait_applied(11);
        // Capacity changes happen only after the actual accepted records are pumped.
        if bytes {
            while browser_abi::outbox_len() != 0 {
                pair.step();
            }
            browser_abi::fix_outbox(4096);
        }
        let command = SongCommand::Mute(vactr::song::routing::SongMute {
            epoch: SnapshotEpoch(9999),
            instrument: 1,
            muted: true,
            frame: 0,
        });
        let mut filled = 0;
        for _ in 0..8192 {
            match pair.host.borrow_mut().try_song_command(command) {
                Ok(()) => filled += 1,
                Err(refusal) => {
                    assert_eq!(refusal.command, command);
                    assert!(matches!(refusal.error, SongSubmitError::Backpressure));
                    break;
                }
            }
        }
        assert!(
            filled > 0 && filled < 8192,
            "actual finite command queue must refuse"
        );
        pair.mute(12, old.epoch, true);
        pair.apply(22, tone(440, "4", true), 2, 0);
        pair.apply(23, tone(440, "4", true), 2, 0);
        assert!(pair.messages.iter().any(
            |m| m.env.re == Some(23) && matches!(m.env.body, ServerMsg::SongCandidateFailed(_))
        ));
        let before = pair.commands.borrow().len();
        let replacement = pair.wait_replacement();
        assert!(
            pair.messages
                .iter()
                .any(|m| m.env.re == Some(12)
                    && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))),
            "actual mute receipt must precede frozen overlay"
        );
        let rejected=pair.receipts.borrow().iter().filter(|m|matches!(m,
            HostMsg::Song(vactr::song::routing::SongHostAck::Rejected {epoch,..}) if *epoch==SnapshotEpoch(9999))).count();
        assert_eq!(
            rejected, filled,
            "all real foreign pressure outcomes remain accounted"
        );
        assert!(pair.commands.borrow()[before..].iter().any(
            |c| matches!(c,SongCommand::PrimeMute(m) if m.epoch==replacement.activation.epoch)
        ));
        let old_mutes = pair
            .commands
            .borrow()
            .iter()
            .filter(|c| matches!(c,SongCommand::Mute(m) if m.epoch==old.epoch))
            .count();
        pair.mute(24, old.epoch, false);
        assert!(pair.messages.iter().any(
            |m| m.env.re == Some(24) && matches!(m.env.body, ServerMsg::SongCandidateFailed(_))
        ));
        assert_eq!(
            pair.commands
                .borrow()
                .iter()
                .filter(|c| matches!(c,SongCommand::Mute(m) if m.epoch==old.epoch))
                .count(),
            old_mutes,
            "exclusive overlay refuses before any new old gate POD"
        );
        let next = pair.wait_applied(22);
        pair.apply(33, tone(440, "1", true), 3, 0);
        let repeated = pair.wait_applied(33);
        assert_eq!(
            (repeated.application_frame - next.application_frame) % 16000,
            0
        );
        assert_eq!(
            pair.session.applied_song_catalog().unwrap().0,
            repeated.epoch
        );
    }
}

#[test]
fn actual_full_ack_ring_preserves_pending_mute_and_replacement() {
    for bytes in [false, true] {
        let mut pair = Controller::new(bytes);
        pair.apply(11, tone(440, "8", false), 1, 0);
        let old = pair.wait_applied(11);
        // Settle the original internal refresh chain without delivering Session ACKs.
        // At most one query is inflight plus one dirty retry; no new mutations here.
        for _ in 0..16 {
            pair.rig.tick();
            let mut original = Vec::new();
            pair.host.borrow_mut().drain(&mut original);
            pair.replay.borrow_mut().extend(original);
        }
        pair.held.set(true);
        let command = SongCommand::RequestCapacity(SnapshotEpoch(9999));
        let capacity = vactr::host::native::audio::ACK_CAPACITY;
        for _ in 0..capacity {
            let mut accepted = false;
            for _ in 0..capacity {
                let outcome = pair.host.borrow_mut().try_song_command(command);
                match outcome {
                    Ok(()) => {
                        accepted = true;
                        break;
                    }
                    Err(refusal) => {
                        assert_eq!(refusal.command, command);
                        assert!(matches!(refusal.error, SongSubmitError::Backpressure));
                        pair.rig.tick_with_delivery(false);
                    }
                }
            }
            assert!(accepted, "bounded real pressure admission must progress");
        }
        // Actual ByteInbox capacity is 128; this bound also covers Native queue pumping.
        for _ in 0..capacity / 16 {
            pair.rig.tick_with_delivery(false);
        }
        pair.mute(12, old.epoch, true);
        pair.apply(22, tone(440, "1", true), 2, 0);
        for _ in 0..2 {
            pair.step();
        }
        assert!(!pair.messages.iter().any(
            |m| m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
        ));
        assert!(!pair
            .commands
            .borrow()
            .iter()
            .any(|c| matches!(c, SongCommand::PrimeMute(_) | SongCommand::Replace(_))));
        assert!(pair.receipts.borrow().iter().all(|m|!matches!(m,
            HostMsg::Song(vactr::song::routing::SongHostAck::CapacityReport(report)) if report.epoch==SnapshotEpoch(9999))));
        assert_eq!(pair.snapshot_full_ring(), capacity);
        // Snapshot consumes only the real ring; Session still has no receipt access.
        assert!(!pair.messages.iter().any(
            |m| m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
        ));
        pair.held.set(false);
        pair.rig.tick();
        let next = pair.wait_applied(22);
        assert_eq!((next.application_frame - old.application_frame) % 16000, 0);
        let reports=pair.receipts.borrow().iter().filter(|m|matches!(m,
            HostMsg::Song(vactr::song::routing::SongHostAck::CapacityReport(report)) if report.epoch==SnapshotEpoch(9999))).count();
        assert_eq!(
            reports, capacity,
            "every actual foreign capacity result has exactly one explicit consumer"
        );
        let muted = pair
            .messages
            .iter()
            .position(|m| {
                m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
            })
            .unwrap();
        let applied = pair
            .messages
            .iter()
            .position(|m| {
                m.env.re == Some(22) && matches!(m.env.body, ServerMsg::SongCandidateApplied(_))
            })
            .unwrap();
        assert!(muted < applied);
        assert!(pair
            .commands
            .borrow()
            .iter()
            .any(|c| matches!(c,SongCommand::PrimeMute(m) if m.epoch==next.epoch&&m.muted)));
        for _ in 0..4096 {
            assert!(
                pair.step().iter().all(|x| x.abs() < 1e-7),
                "actual carried mute remains silent"
            );
            if pair.session.runtime().song_state() == Some(sched::song::SongTransportState::Ended) {
                break;
            }
        }
        assert_eq!(
            pair.session.runtime().song_state(),
            Some(sched::song::SongTransportState::Ended)
        );
        for _ in 0..512 {
            pair.step();
        }
        let expected: Vec<_> = pair
            .commands
            .borrow()
            .iter()
            .filter_map(|c| match c {
                SongCommand::ReserveResource(r) if r.epoch == next.epoch => {
                    Some(vactr::song::routing::SongLeaseKey {
                        epoch: r.epoch,
                        resource: r.resource,
                        kind: r.kind,
                    })
                }
                _ => None,
            })
            .collect();
        assert!(!expected.is_empty());
        for key in expected {
            assert_eq!(pair.receipts.borrow().iter().filter(|m|matches!(m,
                HostMsg::Song(vactr::song::routing::SongHostAck::LeaseReturned(returned)) if *returned == key)).count(),1,
                "each actual reserved full key returns exactly once");
        }
        pair.apply(33, tone(660, "1", false), 3, 0);
        let further = pair.wait_applied(33);
        assert!(further.application_frame > next.application_frame);
        assert_eq!(
            pair.session.applied_song_catalog().unwrap().0,
            further.epoch
        );
    }
}

impl RecordedHost {
    #[allow(clippy::result_large_err)] // Preserve the host's exact inline refused POD ownership.
    pub(super) fn admit_controller_command(
        &mut self,
        command: SongCommand,
    ) -> Result<(), SongCommandRefusal> {
        if matches!(command,SongCommand::Mute(m) if m.epoch!=SnapshotEpoch(9999))
            && self.permanent_mute.get() != 0
        {
            self.refused.borrow_mut().push(command);
            let error = if self.permanent_mute.get() == 1 {
                SongSubmitError::Unavailable
            } else {
                SongSubmitError::Invalid(Failure::new(
                    vactr::vm::fail::FailCode::Type,
                    "configured permanent mute validation refusal",
                ))
            };
            return Err(SongCommandRefusal { command, error });
        }
        if let Err(refusal) = self.inner.borrow_mut().try_song_command(command) {
            self.refused.borrow_mut().push(command);
            return Err(refusal);
        }
        self.commands.borrow_mut().push(command);
        if matches!(command, SongCommand::PrimeMute(_)) && self.prime_pressure.replace(false) {
            let filler = SongCommand::RequestCapacity(SnapshotEpoch(9999));
            let mut filled = 0;
            for _ in 0..8192 {
                match self.inner.borrow_mut().try_song_command(filler) {
                    Ok(()) => filled += 1,
                    Err(refusal) => {
                        assert_eq!(refusal.command, filler);
                        assert!(matches!(refusal.error, SongSubmitError::Backpressure));
                        break;
                    }
                }
            }
            assert!(
                filled > 0 && filled < 8192,
                "actual provider admission saturates after accepted Prime"
            );
            self.pressure_count.set(filled);
        }
        Ok(())
    }
}

#[test]
fn partial_prime_pressure_retries_or_cancels_without_premature_advance() {
    for bytes in [false, true] {
        for cancel in [false, true] {
            let mut pair = Controller::new(bytes);
            pair.apply(11, tone(440, "8", false), 1, 0);
            let old = pair.wait_applied(11);
            pair.mute(12, old.epoch, true);
            for _ in 0..512 {
                pair.step();
                if pair.messages.iter().any(|m| {
                    m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
                }) {
                    break;
                }
            }
            assert!(pair
                .messages
                .iter()
                .any(|m| m.env.re == Some(12)
                    && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))));
            if bytes {
                while browser_abi::outbox_len() != 0 {
                    pair.step();
                }
                browser_abi::fix_outbox(4096);
            }
            pair.prime_pressure.set(true);
            pair.apply(22, tone(440, "1", true), 2, 0);
            for _ in 0..2048 {
                pair.step();
                if pair.pressure_count.get() > 0 {
                    break;
                }
            }
            assert!(pair.pressure_count.get() > 0);
            assert!(pair
                .commands
                .borrow()
                .iter()
                .any(|c| matches!(c, SongCommand::PrimeMute(_))));
            assert!(pair
                .refused
                .borrow()
                .iter()
                .any(|c| matches!(c, SongCommand::PrimeMute(_) | SongCommand::Replace(_))));
            assert!(
                pair.replacement().is_none(),
                "partial admission cannot falsely become Submitted"
            );
            assert!(!pair
                .messages
                .iter()
                .any(|m| m.env.re == Some(22)
                    && matches!(m.env.body, ServerMsg::SongCandidateFailed(_))));
            assert_eq!(pair.session.applied_song_catalog().unwrap().0, old.epoch);
            if cancel {
                pair.change(3, 2, 1);
                pair.wait_failed(22);
                for _ in 0..512 {
                    pair.step();
                }
                assert_eq!(pair.session.applied_song_catalog().unwrap().0, old.epoch);
                pair.apply(33, tone(440, "1", true), 3, 1);
                pair.wait_applied(33);
            } else {
                pair.wait_applied(22);
            }
            let reports=pair.receipts.borrow().iter().filter(|m|matches!(m,
                HostMsg::Song(vactr::song::routing::SongHostAck::CapacityReport(report)) if report.epoch==SnapshotEpoch(9999))).count();
            assert_eq!(reports, pair.pressure_count.get());
        }
    }
}

#[test]
fn permanent_mute_refusal_resolves_request_and_allows_later_apply() {
    for bytes in [false, true] {
        for policy in [1, 2] {
            let mut pair = Controller::new(bytes);
            pair.apply(11, tone(440, "8", false), 1, 0);
            let old = pair.wait_applied(11);
            pair.permanent_mute.set(policy);
            pair.mute(12, old.epoch, true);
            pair.wait_failed(12);
            assert_eq!(
                pair.refused
                    .borrow()
                    .iter()
                    .filter(|c| matches!(c,SongCommand::Mute(m) if m.epoch==old.epoch))
                    .count(),
                1,
                "permanent refusal never loops as queue pressure"
            );
            assert!(!pair
                .commands
                .borrow()
                .iter()
                .any(|c| matches!(c,SongCommand::Mute(m) if m.epoch==old.epoch)));
            assert!(!pair.receipts.borrow().iter().any(|m|matches!(m,HostMsg::Song(vactr::song::routing::SongHostAck::Muted(m)) if m.epoch==old.epoch)));
            assert_eq!(pair.session.applied_song_catalog().unwrap().0, old.epoch);
            assert!(
                (0..128).any(|_| pair.step().iter().any(|x| x.abs() > 1e-6)),
                "original actual PCM remains audible after refusal"
            );
            pair.permanent_mute.set(0);
            pair.apply(22, tone(660, "1", false), 2, 0);
            let next = pair.wait_applied(22);
            assert_eq!((next.application_frame - old.application_frame) % 16000, 0);
            assert_eq!(pair.session.applied_song_catalog().unwrap().0, next.epoch);
        }
    }
}
