//! Actual lifecycle evidence shares the parent Native/Arena upload and callback rig.
use super::*;

fn activate(r: &mut Rig, branches: &[u32]) {
    r.command(SongCommand::Activate(SongActivation {
        epoch: SnapshotEpoch(11),
        frame: r.frame,
    }));
    for &b in branches {
        r.event(b, r.frame);
    }
}
fn mute(r: &mut Rig, muted: bool, frame: u64) {
    r.command(SongCommand::Mute(SongMute {
        epoch: SnapshotEpoch(11),
        instrument: 1,
        muted,
        frame,
    }));
}
fn end(r: &mut Rig, arrangement: u64, tail_deadline: u64) {
    r.command(SongCommand::Endpoints(SongEndpoints {
        epoch: SnapshotEpoch(11),
        arrangement,
        tail_deadline,
    }));
}

#[test]
fn real_reset_oscillator_ramp_mutes_same_family_across_tracks_and_preserves_pcm_sibling() {
    for bytes in [false, true] {
        let mut a = Rig::prepared_family(bytes, 16, true);
        let mut reference = Rig::prepared_family(bytes, 16, true);
        let mut prefix = Rig::prepared_family(bytes, 16, true);
        let mut sibling = Rig::prepared_family(bytes, 16, true);
        activate(&mut a, &[0, 1, 2]);
        activate(&mut reference, &[0]);
        activate(&mut prefix, &[0, 1, 2]);
        activate(&mut sibling, &[0]);
        a.process(20, true);
        reference.process(20, true);
        prefix.process(20, true);
        sibling.process(20, true);
        let f = a.frame + 5;
        mute(&mut a, true, f);
        // Untouched initialized voice/room histories provide a real ungated reset oracle.
        reference.event(1, f);
        reference.event(2, f);
        let actual = a.process(85, true);
        let expected = reference.process(85, true);
        let pcm = sibling.process(85, true);
        let unchanged = prefix.process(85, true);
        assert_eq!(&actual[..10], &unchanged[..10]);
        let mut nonzero = 0;
        for frame in 5..85 {
            let gain = if frame < 69 {
                1. - (frame - 5) as f32 / 64.
            } else {
                0.
            };
            for channel in 0..2 {
                let k = 2 * frame + channel;
                assert!((actual[k] - (pcm[k] + (expected[k] - pcm[k]) * gain)).abs() < 0.00001, "bytes={bytes} frame={frame} channel={channel} actual={} reference={} pcm={} gain={gain}", actual[k],expected[k],pcm[k]);
                if frame < 69 && (actual[k] - pcm[k]).abs() > 0.0001 {
                    nonzero += 1;
                }
            }
        }
        assert!(
            nonzero > 50,
            "actual post-reset oscillator audio through the closing gate"
        );
        assert_eq!(a.engine.active_voices(), 1);
        assert!(a
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Muted(SongMute {
                epoch: SnapshotEpoch(11),
                instrument: 1,
                muted: true,
                frame: f
            }))));
    }
}

#[test]
fn early_unmute_never_resumes_old_notes_and_muted_late_arrivals_are_consumed() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 16);
        activate(&mut r, &[1]);
        r.process(16, true);
        let f = r.frame;
        mute(&mut r, true, f);
        r.event(1, f + 10);
        r.process(8, true);
        r.event(1, r.frame);
        r.process(8, true);
        let frame = r.frame;
        mute(&mut r, false, frame);
        assert!(r.process(64, true).iter().all(|x| *x == 0.));
        assert_eq!(r.engine.active_voices(), 0);
        assert!(!r
            .receipts
            .iter()
            .any(|m| matches!(m, HostMsg::Song(SongHostAck::Rejected { .. }))));
        r.event(1, r.frame);
        assert!(r.process(16, true).iter().any(|x| x.abs() > 0.01));
    }
}

#[test]
fn finite_epoch_deadlines_truncate_long_pcm_at_zero_short_and_long_caps() {
    for bytes in [false, true] {
        for cap in [0, 17, 96] {
            let mut r = Rig::prepared(bytes, 16);
            activate(&mut r, &[0]);
            r.process(16, true);
            let arrangement = r.frame + 5;
            let deadline = arrangement + cap;
            end(&mut r, arrangement, deadline);
            r.event(0, arrangement); // normal end-exclusive precommitted occurrence
            let output = r.process((cap + 25) as usize, true);
            assert!(output[..10].iter().all(|x| x.abs() > 0.01));
            assert!(output[(2 * (5 + cap)) as usize..].iter().all(|x| *x == 0.));
            assert_eq!(r.engine.active_voices(), 0);
            assert!(!r
                .receipts
                .iter()
                .any(|m| matches!(m, HostMsg::Song(SongHostAck::Rejected { .. }))));
            for _ in 0..40 {
                assert!(r.process(16, true).iter().all(|x| *x == 0.));
            }
            assert_eq!(
                r.receipts
                    .iter()
                    .filter(|m| matches!(m, HostMsg::Song(SongHostAck::LeaseReturned(_))))
                    .count(),
                11
            );
            assert!(r
                .engine
                .begin_song_preparation(SongStagePreparation {
                    preparation: SongPreparation {
                        epoch: SnapshotEpoch(11),
                        branches: 0,
                        resources: 0,
                        required: SongHostCapacities {
                            sample_rate: 48000,
                            ..SongHostCapacities::default()
                        }
                    },
                    analysis_required: SongAnalysisCapacity { slots: 0 }
                })
                .is_ok());
        }
    }
}

#[test]
fn active_endpoint_deadline_survives_real_full_ack_and_garbage_queues() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 2);
        activate(&mut r, &[0]);
        r.process(16, true);
        r.command(SongCommand::RequestCapacity(SnapshotEpoch(98)));
        r.command(SongCommand::RequestCapacity(SnapshotEpoch(99)));
        let prior = r.process(1, false)[0];
        for _ in 0..8 {
            r.garbage
                .push(Garbage::Sample(Arc::new(SampleData {
                    rate: 48000,
                    channels: 1,
                    frames: vec![0.; 1].into_boxed_slice(),
                })))
                .ok()
                .unwrap();
        }
        let f = r.frame;
        mute(&mut r, true, f);
        r.command(SongCommand::Mute(SongMute {
            epoch: SnapshotEpoch(11),
            instrument: 0,
            muted: true,
            frame: f + 1,
        }));
        end(&mut r, f + 5, f + 22);
        let output = r.process(48, false);
        assert!(output[..10].iter().all(|x| *x == prior), "second mute cannot apply while first actual Muted occupies the one internal receipt slot");
        assert!(output[44..].iter().all(|x| *x == 0.));
        assert_eq!(r.engine.active_voices(), 0);
        let capacity = r.engine.song_remaining_capacities().unwrap();
        assert_eq!(
            r.engine.cancel_song_preparation(SnapshotEpoch(11)),
            Err(SongRejectCode::NotReady)
        );
        assert_eq!(capacity.pcm_bytes, 0);
        for _ in 0..60 {
            r.drain();
            assert!(r.process(16, true).iter().all(|x| *x == 0.));
        }
        assert_eq!(
            r.receipts
                .iter()
                .filter(|m| matches!(m, HostMsg::Song(SongHostAck::LeaseReturned(_))))
                .count(),
            11
        );
        assert!(r.engine.song_remaining_capacities().unwrap().pcm_bytes > 0);
        assert_eq!(
            r.receipts
                .iter()
                .filter(|m| matches!(m, HostMsg::Song(SongHostAck::CapacityReport(_))))
                .count(),
            2
        );
        assert!(r
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Muted(SongMute {
                epoch: SnapshotEpoch(11),
                instrument: 1,
                muted: true,
                frame: f
            }))));
    }
}

#[test]
fn empty_ready_epoch_remains_active_through_its_silent_tail_then_reaps() {
    for bytes in [false, true] {
        let mut r = Rig::new(bytes, 8);
        let prep = SongStagePreparation {
            preparation: SongPreparation {
                epoch: SnapshotEpoch(11),
                branches: 0,
                resources: 0,
                required: SongHostCapacities {
                    sample_rate: 48000,
                    ..SongHostCapacities::default()
                },
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        };
        r.engine.begin_song_preparation(prep).unwrap();
        r.command(SongCommand::SealPreparation(SnapshotEpoch(11)));
        r.process(16, true);
        activate(&mut r, &[]);
        r.process(16, true);
        assert_eq!(
            r.engine.cancel_song_preparation(SnapshotEpoch(11)),
            Err(SongRejectCode::NotReady)
        );
        let a = r.frame + 5;
        end(&mut r, a, a + 17);
        assert!(r.process(10, true).iter().all(|x| *x == 0.));
        assert!(r.engine.begin_song_preparation(prep).is_err());
        r.process(20, true);
        r.engine.begin_song_preparation(prep).unwrap();
    }
}

#[test]
fn blocked_activation_cannot_overtake_a_past_finite_end_or_remain_active() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 2);
        let prep = SongStagePreparation {
            preparation: SongPreparation {
                epoch: SnapshotEpoch(99),
                branches: 0,
                resources: 0,
                required: SongHostCapacities {
                    sample_rate: 48000,
                    ..SongHostCapacities::default()
                },
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        };
        r.engine.begin_song_preparation(prep).unwrap();
        r.command(SongCommand::SealPreparation(SnapshotEpoch(99)));
        r.process(16, true);
        r.command(SongCommand::RequestCapacity(SnapshotEpoch(97)));
        r.command(SongCommand::RequestCapacity(SnapshotEpoch(98)));
        r.process(1, false);
        let filled = (r.frame, r.controls.len(), r.inbox.len(), r.received.len());
        activate(&mut r, &[]);
        r.process(1, false);
        let first = (r.frame, r.controls.len(), r.inbox.len(), r.received.len());
        let f = r.frame;
        r.command(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(99),
            frame: f,
        }));
        r.command(SongCommand::Endpoints(SongEndpoints {
            epoch: SnapshotEpoch(99),
            arrangement: f + 2,
            tail_deadline: f + 4,
        }));
        end(&mut r, f + 2, f + 4);
        assert!(r.process(16, false).iter().all(|x| *x == 0.));
        let blocked = (r.frame, r.controls.len(), r.inbox.len(), r.received.len());
        for _ in 0..60 {
            r.drain();
            r.process(16, true);
        }
        assert_eq!(r.engine.active_voices(), 0);
        assert_eq!(
            r.receipts
                .iter()
                .filter(|m| matches!(m, HostMsg::Song(SongHostAck::Applied(_))))
                .count(),
            1,
            "bytes={bytes}; (frame,native,arena,ack) filled={filled:?}, first={first:?}, blocked={blocked:?}; actual receipts={:#?}",
            r.receipts
        );
        assert_eq!(
            r.receipts
                .iter()
                .filter(|m| matches!(
                    m,
                    HostMsg::Song(SongHostAck::ActivationRejected {
                        activation: SongActivation { epoch: SnapshotEpoch(99), frame },
                        reason: SongRejectCode::Malformed
                    }) if *frame == f
                ))
                .count(),
            1,
            "bytes={bytes}; requested99={f}; (frame,native,arena,ack) filled={filled:?}, first={first:?}, blocked={blocked:?}; actual receipts={:#?}",
            r.receipts
        );
        assert!(!r.receipts.iter().any(|m| matches!(
            m,
            HostMsg::Song(SongHostAck::Applied(SongActivation {
                epoch: SnapshotEpoch(99),
                ..
            }))
        )));
        // If only the physical ACK ring were full, epoch99 would have applied before its cap.
        // The actual first Applied occupies the constructor-admitted single internal slot.
        assert_eq!(
            r.engine.begin_song_preparation(prep),
            Err(SongRejectCode::StaleEpoch)
        );
        for epoch in [11, 99] {
            assert_eq!(
                r.receipts
                    .iter()
                    .filter(|m| matches!(m,
                HostMsg::Song(SongHostAck::Rejected { epoch: e, reason: SongRejectCode::Malformed })
                if *e == SnapshotEpoch(epoch)))
                    .count(),
                1
            );
        }
        // Rejected late endpoints did not clean either original owner.
        let held = r.engine.song_remaining_capacities().unwrap();
        let deadline = r.frame;
        end(&mut r, deadline, deadline);
        r.command(SongCommand::CancelPreparation(SnapshotEpoch(99)));
        for _ in 0..60 {
            assert!(r.process(16, true).iter().all(|x| *x == 0.));
        }
        assert_eq!(r.engine.active_voices(), 0);
        assert_eq!(
            r.receipts
                .iter()
                .filter(
                    |m| **m == HostMsg::Song(SongHostAck::PreparationCancelled(SnapshotEpoch(99)))
                )
                .count(),
            1
        );
        for (id, kind) in [
            (1, SongResourceKind::Instrument),
            (2, SongResourceKind::Instrument),
            (3, SongResourceKind::PrivateFx),
            (4, SongResourceKind::PrivateFx),
            (5, SongResourceKind::Track),
            (6, SongResourceKind::Master),
            (7, SongResourceKind::Sample),
            (8, SongResourceKind::ControlCells),
            (9, SongResourceKind::ControlCells),
            (10, SongResourceKind::AnalysisBank),
            (11, SongResourceKind::AnalysisBank),
        ] {
            assert_eq!(
                r.receipts
                    .iter()
                    .filter(|m| **m == HostMsg::Song(SongHostAck::LeaseReturned(key(id, kind))))
                    .count(),
                1
            );
        }
        let reclaimed = r.engine.song_remaining_capacities().unwrap();
        assert_eq!(reclaimed.template_slots, held.template_slots + 2);
        assert_eq!(reclaimed.bus_slots, held.bus_slots + 4);
        assert_eq!(reclaimed.sample_resources, held.sample_resources + 1);
        assert_eq!(reclaimed.pcm_bytes, held.pcm_bytes + 4096 * 4);
        assert_eq!(
            r.engine.cancel_song_preparation(SnapshotEpoch(11)),
            Err(SongRejectCode::StaleEpoch)
        );
        let mut fresh = prep;
        fresh.preparation.epoch = SnapshotEpoch(100);
        r.engine.begin_song_preparation(fresh).unwrap();
        r.command(SongCommand::CancelPreparation(SnapshotEpoch(100)));
        for _ in 0..4 {
            r.process(16, true);
        }
        assert_eq!(
            r.receipts
                .iter()
                .filter(
                    |m| **m == HostMsg::Song(SongHostAck::PreparationCancelled(SnapshotEpoch(100)))
                )
                .count(),
            1
        );
    }
}

#[test]
fn shared_master_real_tail_survives_last_private_generation_until_epoch_deadline() {
    for bytes in [false, true] {
        let mut r = Rig::prepared_case(bytes, 16, false, true);
        activate(&mut r, &[0]);
        r.process(64, true);
        let start = r.frame;
        for branch in 0..2 {
            r.command(SongCommand::Release(SongBranchRelease {
                epoch: SnapshotEpoch(11),
                branch: SongBranchId(branch),
                generation: 17,
                frame: start,
                tail_deadline: start + 8,
            }));
        }
        end(&mut r, start + 60, start + 100);
        let output = r.process(132, true);
        assert!(
            output[32..100].iter().any(|x| x.abs() > 0.0001),
            "actual filter master history continues after private voice/FX cutoff"
        );
        assert!(output[200..].iter().all(|x| *x == 0.));
        assert_eq!(r.engine.active_voices(), 0);
        assert!(r.engine.pop_fault().is_none());
    }
}

#[test]
fn mute_and_short_tail_are_invariant_under_actual_callback_partitions() {
    for bytes in [false, true] {
        let mut whole = Rig::prepared(bytes, 16);
        let mut pieces = Rig::prepared(bytes, 16);
        for r in [&mut whole, &mut pieces] {
            activate(r, &[0, 1]);
            r.process(16, true);
            let f = r.frame;
            mute(r, true, f + 5);
            end(r, f + 70, f + 87);
        }
        let actual = whole.process(112, true);
        let mut divided = Vec::new();
        for n in [3, 7, 11, 17, 19, 23, 32] {
            divided.extend(pieces.process(n, true));
        }
        assert_eq!(actual.len(), divided.len());
        for (k, (a, b)) in actual.iter().zip(&divided).enumerate() {
            assert!(
                (*a - *b).abs() < 0.00001,
                "bytes={bytes} frame={} channel={} whole={a} partitioned={b}",
                k / 2,
                k % 2
            );
        }
        assert!(actual[174..].iter().all(|x| *x == 0.));
    }
}
