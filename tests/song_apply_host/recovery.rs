//! Return the existing opaque ticket only through real cancellation receipts.
use super::*;

fn horizon() -> TimeSpan {
    TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap()
}
fn receive_old_or_new(
    rig: &mut Rig,
    old: &mut SongTransport,
    new: &mut SongTransport,
) -> Vec<SongHostAck> {
    let mut received = Vec::new();
    for message in rig.tick() {
        if let HostMsg::Song(ack) = message {
            received.push(ack);
            if ack.epoch() == old.epoch() {
                old.receive(ack).unwrap();
            } else if ack.epoch() == new.epoch() {
                new.receive(ack).unwrap();
            } else {
                assert_eq!(ack.epoch(), SnapshotEpoch(999));
                assert_eq!(old.receive(ack), Err(ack));
                assert_eq!(new.receive(ack), Err(ack));
            }
        }
    }
    received
}
fn render_retaining_receipts(rig: &mut Rig) {
    let mut out = [0.; 32];
    match &mut rig.backend {
        Backend::Native(side) => callback(|| side.render(&mut out, 2)),
        Backend::Arena {
            engine,
            inbox,
            pending,
            events,
            acks,
            cells,
            garbage,
            returned,
            ..
        } => {
            let length = usize::try_from(browser_abi::outbox_len()).unwrap();
            let bytes =
                unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), length) }.to_vec();
            browser_abi::outbox_clear();
            let mut at = 0;
            while at < bytes.len() {
                let n = usize::try_from(u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()))
                    .unwrap();
                at += 4;
                pending.push_back(bytes[at..at + n].to_vec());
                at += n;
            }
            while let Some(record) = pending.front() {
                if !inbox.push(record) {
                    break;
                }
                pending.pop_front();
            }
            callback(|| {
                engine.process(
                    &mut EngineIo {
                        events,
                        controls: inbox.as_mut(),
                        acks,
                        cells,
                        garbage: Some(garbage),
                    },
                    &mut out,
                    16,
                )
            });
            while let Some(owner) = returned.pop() {
                drop(owner);
            }
        }
    }
    rig.frame += 16;
    rig.last = out;
}
fn drive_cancel(
    rig: &mut Rig,
    old: &mut SongTransport,
    new: &mut SongTransport,
) -> (Vec<SongHostAck>, Vec<(u64, [f32; 32])>) {
    let mut acks = Vec::new();
    let mut pcm = Vec::new();
    for _ in 0..2048 {
        new.advance(
            rig.host.as_mut(),
            SongHostClock {
                frame: rig.frame,
                sample_rate: 32768,
            },
            horizon(),
        )
        .unwrap();
        acks.extend(receive_old_or_new(rig, old, new));
        pcm.push((rig.frame, rig.last));
        if let Ok(prepared) = new.take_cancelled() {
            assert_eq!(prepared.epoch(), new.epoch());
            assert_eq!(prepared.revision(), 17);
            assert!(new.take_cancelled().is_err());
            assert!(acks.contains(&SongHostAck::PreparationCancelled(new.epoch())));
            for lease in prepared.resources() {
                let resource = SongResourceRef {
                    id: lease.resource,
                    generation: lease.generation,
                };
                assert_eq!(acks.iter().filter(|ack| matches!(ack,
                    SongHostAck::LeaseReturned(key) if key.epoch == new.epoch() && key.resource == resource
                )).count(), 1);
            }
            assert_eq!(
                acks.iter()
                    .filter(
                        |ack| matches!(ack, SongHostAck::LeaseReturned(k) if k.epoch == new.epoch())
                    )
                    .count(),
                prepared.resources().len(),
            );
            return (acks, pcm);
        }
        assert!(new.take_cancelled_replacement_source().unwrap().is_none());
    }
    panic!("real cancellation stalled: {:?}", new.failure());
}
fn assert_old_pcm(bytes: bool, epoch: u64, actual: &[(u64, [f32; 32])]) {
    assert!(!actual.is_empty());
    assert!(actual
        .iter()
        .any(|(_, pcm)| pcm.iter().any(|x| x.abs() > 1e-6)));
    // Construct the reference after the Arena run: the ABI outbox is global.
    let mut reference = Rig::new(bytes);
    let mut old = start(&mut reference, epoch);
    for (frame, pcm) in actual {
        while reference.frame < *frame {
            for message in reference.tick() {
                if let HostMsg::Song(ack) = message {
                    old.receive(ack).unwrap();
                }
            }
        }
        assert_eq!(reference.frame, *frame);
        assert_eq!(
            reference.last.map(f32::to_bits),
            pcm.map(f32::to_bits),
            "old PCM at {frame}"
        );
    }
}
fn make_replacement(
    rig: &mut Rig,
    old: &mut SongTransport,
    epoch: u64,
    ahead: u64,
) -> SongTransport {
    let source = old.replacement_source().unwrap();
    let ready = next_ready(rig, epoch);
    let mut records = overlay(&ready, epoch);
    for m in &mut records {
        m.muted = false;
    }
    let mut new = SongTransport::new(
        ready,
        rig.frame.checked_add(ahead).unwrap(),
        SongLimits::default(),
    )
    .ok()
    .unwrap();
    new.prepare_replacement(source, epoch, records)
        .unwrap_or_else(|r| panic!("{}", r.failure));
    new
}
fn submit_all(rig: &mut Rig, new: &mut SongTransport) {
    for _ in 0..128 {
        match new.submit_replacement(rig.host.as_mut()).unwrap() {
            SongReplacementProgress::Priming => {}
            SongReplacementProgress::Submitted => return,
        }
    }
    panic!("bounded priming did not submit");
}
fn apply_and_retire(rig: &mut Rig, old: &mut SongTransport, new: &mut SongTransport) {
    let mut committed = false;
    let mut audible = false;
    for _ in 0..8192 {
        for core in [&mut *old, &mut *new] {
            core.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 32768,
                },
                horizon(),
            )
            .unwrap();
        }
        receive_old_or_new(rig, old, new);
        if let Some(commit) = new.take_replacement_commit() {
            assert!(!committed);
            assert_eq!(
                commit.replacement().activation,
                new.applied_activation().unwrap()
            );
            old.observe_replacement(commit)
                .unwrap_or_else(|r| panic!("{}", r.failure));
            committed = true;
        }
        if committed {
            audible |= rig.last.iter().any(|x| x.abs() > 1e-6);
        }
        assert!(new.take_cancelled_replacement_source().unwrap().is_none());
        if old.state() == SongTransportState::Ended && new.state() == SongTransportState::Ended {
            assert!(committed && audible);
            assert_eq!(old.take_retired().unwrap().epoch(), old.epoch());
            assert_eq!(new.take_retired().unwrap().epoch(), new.epoch());
            assert!(old.take_retired().is_err());
            assert!(new.take_retired().is_err());
            return;
        }
    }
    panic!(
        "replacement finite retirement stalled: {:?} {:?}",
        old.failure(),
        new.failure()
    );
}

#[test]
fn cancelled_replacement_reclaims_old_source_and_second_apply_succeeds() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 800);
        let activation = old.applied_activation().unwrap();
        let endpoints = old.endpoints();
        let mut rejected = make_replacement(&mut rig, &mut old, 801, 4096);
        submit_all(&mut rig, &mut rejected);
        assert!(rejected
            .take_cancelled_replacement_source()
            .unwrap()
            .is_none());
        rejected.invalidate_replacement().unwrap();
        let (acks, pcm) = drive_cancel(&mut rig, &mut old, &mut rejected);
        assert!(acks.iter().any(|ack| matches!(ack,
            SongHostAck::ActivationRejected { activation: a, reason: SongRejectCode::NotReady }
                if a.epoch == rejected.epoch()
        )));
        assert!(!acks
            .iter()
            .any(|ack| matches!(ack, SongHostAck::Applied(a) if a.epoch == rejected.epoch())));
        let source = rejected
            .take_cancelled_replacement_source()
            .unwrap()
            .unwrap();
        assert!(rejected
            .take_cancelled_replacement_source()
            .unwrap()
            .is_none());
        assert_eq!(source.activation(), activation);
        assert_eq!(source.endpoints(), endpoints);
        old.reclaim_replacement_source(source)
            .unwrap_or_else(|r| panic!("{}", r.failure));
        assert_eq!(old.endpoints(), endpoints);
        assert_eq!(old.applied_activation(), Some(activation));
        let mut next = make_replacement(&mut rig, &mut old, 802, 1024);
        assert!(old.replacement_source().is_err());
        submit_all(&mut rig, &mut next);
        apply_and_retire(&mut rig, &mut old, &mut next);
        assert_old_pcm(bytes, 800, &pcm);
    }
}

#[test]
fn unsubmitted_refusal_and_wrong_owner_preserve_original_source() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 810);
        let source = old.replacement_source().unwrap();
        let activation = source.activation();
        let endpoints = source.endpoints();
        let mut ready = next_ready(&mut rig, 811);
        let mut records = overlay(&ready, 7);
        records[0].epoch = SnapshotEpoch(999);
        let pointer = records.as_ptr();
        let command = intent(&ready, &source, rig.frame + 1024, 7, records.len());
        let refusal = ready
            .prepare_replacement(command, source, records)
            .err()
            .unwrap();
        assert_eq!(refusal.overlay.as_ptr(), pointer);
        assert_eq!(refusal.replacement, command);
        assert_eq!(refusal.source.activation(), activation);
        assert_eq!(refusal.source.endpoints(), endpoints);
        let mut foreign = SongTransport::new(ready, rig.frame + 1024, SongLimits::default())
            .ok()
            .unwrap();
        let refusal = foreign
            .reclaim_replacement_source(refusal.source)
            .err()
            .unwrap();
        assert_eq!(refusal.source.activation(), activation);
        assert_eq!(refusal.source.endpoints(), endpoints);
        old.reclaim_replacement_source(refusal.source)
            .unwrap_or_else(|r| panic!("{}", r.failure));
        let source = old.replacement_source().unwrap();
        assert_eq!(source.activation(), activation);
        assert_eq!(source.endpoints(), endpoints);
        assert!(old.replacement_source().is_err());
        old.reclaim_replacement_source(source)
            .unwrap_or_else(|r| panic!("{}", r.failure));
        foreign.abort(vm::fail::Failure::new(
            vm::fail::FailCode::HostUnavailable,
            "unsubmitted fixture",
        ));
        let (_, pcm) = drive_cancel(&mut rig, &mut old, &mut foreign);
        assert!(foreign
            .take_cancelled_replacement_source()
            .unwrap()
            .is_none());
        assert_old_pcm(bytes, 810, &pcm);
    }
}

#[test]
fn partial_priming_and_ack_pressure_delay_recovery_until_real_cleanup() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 820);
        let endpoints = old.endpoints();
        let mut new = make_replacement(&mut rig, &mut old, 821, 8192);
        assert_eq!(
            new.submit_replacement(rig.host.as_mut()).unwrap(),
            SongReplacementProgress::Priming
        );
        // Fill actual ACK storage without consuming any produced receipt.
        // Arena uses its measured producer; Native uses its constructor constant.
        let capacity = match &rig.backend {
            Backend::Native(_) => vactr::host::native::audio::ACK_CAPACITY,
            Backend::Arena { acks, .. } => acks.capacity(),
        };
        if bytes {
            receive_old_or_new(&mut rig, &mut old, &mut new);
            browser_abi::fix_outbox(4096);
        }
        let mut pcm = Vec::new();
        for batch in 0..capacity / 16 {
            for offset in 0..16 {
                rig.host
                    .try_song_command(SongCommand::RequestClock(SongClockRequest {
                        epoch: SnapshotEpoch(999),
                        request: u64::try_from(batch * 16 + offset + 1).unwrap(),
                    }))
                    .unwrap();
            }
            render_retaining_receipts(&mut rig);
            pcm.push((rig.frame, rig.last));
        }
        if let Backend::Arena { received, acks, .. } = &rig.backend {
            assert_eq!(received.len(), acks.capacity());
        }
        let mut full = false;
        for extra in 0..32768 {
            if rig
                .host
                .try_song_command(SongCommand::RequestClock(SongClockRequest {
                    epoch: SnapshotEpoch(999),
                    request: u64::try_from(capacity + extra + 1).unwrap(),
                }))
                .is_err()
            {
                full = true;
                break;
            }
        }
        assert!(full);
        new.invalidate_replacement().unwrap();
        assert!(new.take_cancelled_replacement_source().unwrap().is_none());
        // Real pressure refusal retains cancellation and original authority.
        new.advance(
            rig.host.as_mut(),
            SongHostClock {
                frame: rig.frame,
                sample_rate: 32768,
            },
            horizon(),
        )
        .unwrap();
        assert!(new.take_cancelled_replacement_source().unwrap().is_none());
        for _ in 0..3 {
            render_retaining_receipts(&mut rig);
            pcm.push((rig.frame, rig.last));
            assert!(new.take_cancelled().is_err());
            assert!(new.take_cancelled_replacement_source().unwrap().is_none());
        }
        let (acks, after) = drive_cancel(&mut rig, &mut old, &mut new);
        pcm.extend(after);
        assert!(!acks
            .iter()
            .any(|ack| matches!(ack, SongHostAck::Applied(a) if a.epoch == new.epoch())));
        assert!(acks.iter().any(
            |ack| matches!(ack, SongHostAck::ClockReport(r) if r.request.epoch == SnapshotEpoch(999))
        ));
        // Natural Ended fixture separately covers source-first cleanup extraction.
        let source = new.take_cancelled_replacement_source().unwrap().unwrap();
        old.reclaim_replacement_source(source)
            .unwrap_or_else(|r| panic!("{}", r.failure));
        assert_eq!(old.endpoints(), endpoints);
        assert!(new.take_cancelled_replacement_source().unwrap().is_none());
        assert_old_pcm(bytes, 820, &pcm);
    }
}

#[test]
fn applied_or_armed_replacement_never_returns_reclaimable_source() {
    for bytes in [false, true] {
        for after_applied in [false, true] {
            let mut rig = Rig::new(bytes);
            let mut old = start(&mut rig, 830);
            let mut new = make_replacement(&mut rig, &mut old, 831, 512);
            let boundary = rig.frame + 512;
            submit_all(&mut rig, &mut new);
            new.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 32768,
                },
                horizon(),
            )
            .unwrap();
            // Sixteen-frame blocks cross the actual arm point boundary-64;
            // stop before boundary commit for the after-arm cancellation variant.
            let target = if after_applied {
                boundary + 16
            } else {
                boundary - 48
            };
            while rig.frame < target {
                receive_old_or_new(&mut rig, &mut old, &mut new);
                assert!(new.take_cancelled_replacement_source().unwrap().is_none());
            }
            assert_eq!(new.applied_activation().is_some(), after_applied);
            if after_applied {
                assert!(new.invalidate_replacement().is_err());
            } else {
                new.invalidate_replacement().unwrap();
            }
            let mut applied = after_applied;
            let mut host_fault = false;
            for _ in 0..128 {
                new.advance(
                    rig.host.as_mut(),
                    SongHostClock {
                        frame: rig.frame,
                        sample_rate: 32768,
                    },
                    horizon(),
                )
                .unwrap();
                let acks = receive_old_or_new(&mut rig, &mut old, &mut new);
                applied |= acks.contains(&SongHostAck::Applied(SongActivation {
                    epoch: new.epoch(),
                    frame: boundary,
                }));
                host_fault |= acks.iter().any(|ack| matches!(ack,
                    SongHostAck::Rejected { epoch, reason: SongRejectCode::HostFault } if *epoch == new.epoch()
                ));
                assert!(new.take_cancelled_replacement_source().unwrap().is_none());
                if applied {
                    break;
                }
            }
            assert!(applied);
            assert!(after_applied || host_fault);
            let commit = new.take_replacement_commit().unwrap();
            old.observe_replacement(commit)
                .unwrap_or_else(|r| panic!("{}", r.failure));
            assert!(new.take_replacement_commit().is_none());
            assert!(new.take_cancelled_replacement_source().unwrap().is_none());
            assert!(
                old.replacement_source().is_err(),
                "committed source never reissues"
            );
            assert!(new.take_cancelled().is_err());
        }
    }
}

#[test]
fn naturally_ended_old_source_reclaims_without_resource_resurrection() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 840);
        for _ in 0..8192 {
            old.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 32768,
                },
                horizon(),
            )
            .unwrap();
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    old.receive(ack).unwrap();
                }
            }
            if old.state() == SongTransportState::Ended {
                break;
            }
        }
        assert_eq!(old.state(), SongTransportState::Ended);
        old.take_retired().unwrap();
        let source = old.replacement_source().unwrap();
        let activation = source.activation();
        let endpoints = source.endpoints();
        let mut ready = next_ready(&mut rig, 841);
        let records = overlay(&ready, 1);
        let command = intent(&ready, &source, rig.frame + 512, 1, records.len());
        ready
            .prepare_replacement(command, source, records)
            .unwrap_or_else(|r| panic!("{}", r.failure));
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup);
        // Source-first order; the original candidate remains independently once-only.
        let source = cleanup
            .take_cancelled_replacement_source()
            .unwrap()
            .unwrap();
        assert!(cleanup
            .take_cancelled_replacement_source()
            .unwrap()
            .is_none());
        assert_eq!(
            cleanup.take_cancelled().unwrap().epoch(),
            SnapshotEpoch(841)
        );
        assert!(cleanup.take_cancelled().is_err());
        old.reclaim_replacement_source(source)
            .unwrap_or_else(|r| panic!("{}", r.failure));
        assert_eq!(old.state(), SongTransportState::Ended);
        let source = old.replacement_source().unwrap();
        assert_eq!(source.activation(), activation);
        assert_eq!(source.endpoints(), endpoints);
        assert!(old.replacement_source().is_err());
        for _ in 0..4 {
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    assert_ne!(ack.epoch(), old.epoch());
                }
            }
            assert!(rig.last.iter().all(|x| x.abs() <= 1e-6));
        }
    }
}
