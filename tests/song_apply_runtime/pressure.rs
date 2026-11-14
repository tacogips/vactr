//! Shared real host pressure/partition/lifecycle fixtures; no synthetic receipts.
use super::*;

pub(super) fn drain(rig: &mut Rig) -> Vec<SongHostAck> {
    if let Backend::Arena {
        state,
        received,
        returned,
        ..
    } = &mut rig.backend
    {
        while let Some(message) = received.pop() {
            state.borrow_mut().on_msg(message);
        }
        while let Some(owner) = returned.pop() {
            drop(owner);
        }
    }
    let mut messages = Vec::new();
    rig.host.drain(&mut messages);
    let mut acknowledgments = Vec::new();
    for message in messages {
        if let HostMsg::Song(ack) = message {
            rig.receipts.push(ack);
            acknowledgments.push(ack);
        }
    }
    acknowledgments
}
pub(super) fn render_block(rig: &mut Rig, frames: usize, collect: bool) -> Vec<f32> {
    assert!((1..=16).contains(&frames));
    rig.capture_outbox();
    let mut output = vec![0.; frames * 2];
    match &mut rig.backend {
        Backend::Native(side) => callback(|| side.render(&mut output, 2)),
        Backend::Arena {
            engine,
            inbox,
            pending,
            events,
            acks,
            cells,
            garbage,
            ..
        } => {
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
                    &mut output,
                    frames,
                )
            });
        }
    }
    rig.frame += u64::try_from(frames).unwrap();
    if collect {
        let _ = drain(rig);
    }
    output
}
fn render_until(rig: &mut Rig, frame: u64, collect: bool) {
    while rig.frame < frame {
        let n = usize::try_from((frame - rig.frame).min(16)).unwrap();
        let _ = render_block(rig, n, collect);
    }
}
fn ready_replacement(
    rig: &mut Rig,
    old: &mut SongTransport,
    epoch: u64,
    offset: u64,
) -> (SongReadyBundle, SongReplacement) {
    let mut ready = next_ready(rig, epoch);
    let source = old.replacement_source().unwrap();
    let replacement = intent(&ready, &source, rig.frame + offset, 101, 0);
    ready
        .prepare_replacement(replacement, source, Vec::new())
        .ok()
        .unwrap();
    submit_all(rig, &mut ready, replacement);
    (ready, replacement)
}

fn fill_actual_critical_ring(rig: &mut Rig, bytes: bool, marker_epoch: SnapshotEpoch) -> usize {
    let count = if bytes {
        128
    } else {
        vactr::host::native::audio::ACK_CAPACITY
    };
    let mut sent = 0;
    while sent < count {
        let batch = (count - sent).min(if bytes { 64 } else { 1024 });
        for _ in 0..batch {
            let command = if bytes {
                SongCommand::RequestCapacity(marker_epoch)
            } else {
                SongCommand::RequestClock(SongClockRequest {
                    epoch: marker_epoch,
                    request: sent as u64 + 1,
                })
            };
            rig.host.try_song_command(command).unwrap();
            sent += 1;
        }
        let _ = render_block(rig, 1, false);
    }
    if bytes {
        // Actual byte ingress holds sixteen records per quantum. Accepted
        // outbox records remain queued until later callbacks admit them.
        for _ in 0..count.div_ceil(ring::INBOX_SLOTS) {
            if matches!(&rig.backend, Backend::Arena { acks, .. } if acks.len() == acks.capacity())
            {
                break;
            }
            let _ = render_block(rig, 1, false);
        }
    }
    if let Backend::Arena { acks, .. } = &rig.backend {
        assert_eq!(
            acks.len(),
            acks.capacity(),
            "real Engine reports must fill critical ring"
        );
    }
    count
}

#[test]
fn real_full_critical_ack_pressure_preserves_failure_mute_applied_order() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let (mut old, old_keys, families) = start_with_authority(&mut rig, 900);
        let (mut ready, replacement) = ready_replacement(&mut rig, &mut old, 901, 256);
        let a = replacement.activation.frame;
        let f = a - 64;
        let event = initial_event(&ready, a);
        ready
            .submit_initial_command(rig.host.as_mut(), event)
            .unwrap();
        let mute = SongMute {
            epoch: old.epoch(),
            instrument: families[0],
            muted: false,
            frame: f + 20,
        };
        rig.host.try_song_command(SongCommand::Mute(mute)).unwrap();
        render_until(&mut rig, f + 1, true);
        let marker_epoch = SnapshotEpoch(909);
        let count = fill_actual_critical_ring(&mut rig, bytes, marker_epoch);
        let cancellation_frame = rig.frame;
        assert!(cancellation_frame < f + 20);
        rig.host
            .try_song_command(SongCommand::CancelPreparation(SnapshotEpoch(901)))
            .unwrap();
        let later_marker = SongClockRequest {
            epoch: SnapshotEpoch(910),
            request: u64::MAX,
        };
        rig.host
            .try_song_command(SongCommand::RequestClock(later_marker))
            .unwrap();
        let _ = render_block(&mut rig, 1, false);
        render_until(&mut rig, a + 1, false);
        let first = drain(&mut rig);
        assert_eq!(
            first
                .iter()
                .filter(|ack| match ack {
                    SongHostAck::CapacityReport(report) => bytes && report.epoch == marker_epoch,
                    SongHostAck::ClockReport(report) =>
                        !bytes && report.request.epoch == marker_epoch,
                    _ => false,
                })
                .count(),
            count
        );
        assert!(
            !first.iter().any(|ack| matches!(
                ack,
                SongHostAck::Applied(_) | SongHostAck::Muted(_) | SongHostAck::Rejected { .. }
            )),
            "actual outcomes must be retained beyond A while critical ring is full: {first:?}"
        );
        let mut outcomes = Vec::new();
        // Each real key return may occupy a staging quantum before later control
        // intake. Three retained outcomes, the automatic capacity retry and the
        // exact clock marker add five quanta to the original key-count bound.
        let release_quanta = old_keys.len() + ready.resources().len() + 5;
        for _ in 0..release_quanta {
            let _ = render_block(&mut rig, 1, false);
            outcomes.extend(drain(&mut rig));
            if outcomes.iter().any(|ack| {
                matches!(ack,
                SongHostAck::ClockReport(report) if report.request == later_marker)
            }) {
                break;
            }
        }
        let relevant: Vec<_> = outcomes.iter().copied().filter(|ack| matches!(ack,
            SongHostAck::Rejected { epoch, .. } if *epoch == SnapshotEpoch(901))
            || matches!(ack, SongHostAck::Muted(m) if m.epoch == old.epoch())
            || matches!(ack, SongHostAck::Applied(applied) if applied.epoch == SnapshotEpoch(901))).collect();
        assert_eq!(
            relevant,
            vec![
                SongHostAck::Rejected {
                    epoch: SnapshotEpoch(901),
                    reason: SongRejectCode::HostFault
                },
                SongHostAck::Muted(mute),
                SongHostAck::Applied(replacement.activation),
            ]
        );
        let later_reports: Vec<_> = outcomes
            .iter()
            .enumerate()
            .filter_map(|(index, ack)| match ack {
                SongHostAck::ClockReport(report) if report.request == later_marker => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(
            later_reports.len(),
            1,
            "later real control report missing: {outcomes:?}"
        );
        let applied_position = outcomes
            .iter()
            .position(|ack| *ack == SongHostAck::Applied(replacement.activation))
            .unwrap();
        assert!(
            later_reports[0] > applied_position,
            "later immediate control ACK overtook retained outcomes: {outcomes:?}"
        );
        let mut delayed_returns = Vec::new();
        let mut unclaimed = Vec::new();
        for ack in outcomes {
            if ack.epoch() == old.epoch() {
                if let Err(receipt) = old.receive(ack) {
                    assert_eq!(receipt, SongHostAck::Muted(mute));
                    unclaimed.push(receipt);
                }
            } else if ack == SongHostAck::Applied(replacement.activation) {
                ready.receive_activation(ack).unwrap();
            } else if ack.epoch() == SnapshotEpoch(901) {
                assert!(
                    matches!(
                        ack,
                        SongHostAck::Rejected {
                            reason: SongRejectCode::HostFault,
                            ..
                        } | SongHostAck::LeaseReturned(_)
                    ),
                    "unexpected new pressure receipt {ack:?}"
                );
                if matches!(ack, SongHostAck::LeaseReturned(_)) {
                    delayed_returns.push(ack);
                }
            }
        }
        // This FIFO probe submitted Mute through the real host directly, rather
        // than Transport's pending selector table; keep its exact unclaimed ACK.
        assert_eq!(unclaimed, vec![SongHostAck::Muted(mute)]);
        assert_eq!(ready.applied_activation(), Some(replacement.activation));
        old.observe_replacement(ready.take_replacement_commit().unwrap())
            .ok()
            .unwrap();
        assert_eq!(render_block(&mut rig, 16, false), vec![0.; 32]);
        delayed_returns.extend(drain(&mut rig).into_iter().filter(|ack| {
            if ack.epoch() == old.epoch() {
                old.receive(*ack).unwrap();
                false
            } else {
                assert!(matches!(ack, SongHostAck::LeaseReturned(_)));
                true
            }
        }));
        let mut cleanup = SongHostPreparation::retire(ready);
        for ack in delayed_returns {
            cleanup.receive(ack).unwrap();
        }
        rig.cleanup(&mut cleanup, &mut old);
        assert_eq!(cleanup.progress(), SongPreparationProgress::Retired);
    }
}

#[test]
fn actual_new_lease_validation_failure_before_f_preserves_old_pcm_bit_identically() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut twin = Rig::new(bytes);
        let mut old = start(&mut rig, 920);
        let mut reference = start(&mut twin, 920);
        let mut ready = next_ready(&mut rig, 921);
        let _reference_ready = next_ready(&mut twin, 921);
        let key = *ready
            .resources()
            .iter()
            .find(|key| key.kind == SongResourceKind::Instrument)
            .unwrap();
        let source = old.replacement_source().unwrap();
        let replacement = intent(&ready, &source, rig.frame + 128, 103, 0);
        ready
            .prepare_replacement(replacement, source, Vec::new())
            .ok()
            .unwrap();
        rig.host
            .try_song_command(SongCommand::CancelLease(key))
            .unwrap();
        submit_all(&mut rig, &mut ready, replacement);
        let mut delayed = Vec::new();
        while rig.frame < replacement.activation.frame + 80 {
            let target = render_block(&mut rig, 16, false);
            let expected = render_block(&mut twin, 16, false);
            assert_eq!(
                target, expected,
                "failed actual lease validation changed old PCM, backend {bytes}"
            );
            for ack in drain(&mut rig) {
                if ack.epoch() == old.epoch() {
                    old.receive(ack).unwrap();
                } else if matches!(ack, SongHostAck::ActivationRejected { .. }) {
                    ready.receive_activation(ack).unwrap();
                } else {
                    delayed.push(ack);
                }
            }
            for ack in drain(&mut twin) {
                reference.receive(ack).unwrap();
            }
        }
        assert_eq!(ready.activation_rejection(), Some(SongRejectCode::NotReady));
        assert_eq!(ready.applied_activation(), None);
        assert!(ready.take_replacement_commit().is_none());
        let mut cleanup = SongHostPreparation::retire(ready);
        for ack in delayed {
            cleanup.receive(ack).unwrap();
        }
        rig.cleanup(&mut cleanup, &mut old);
        assert_eq!(cleanup.progress(), SongPreparationProgress::Cancelled);
    }
}

fn partition_transcript(bytes: bool, chunks: &[usize]) -> (Vec<f32>, Vec<SongHostAck>) {
    let mut rig = Rig::new(bytes);
    let mut old = start(&mut rig, 930);
    let (mut ready, replacement) = ready_replacement(&mut rig, &mut old, 931, 133);
    let event = initial_event(&ready, replacement.activation.frame);
    ready
        .submit_initial_command(rig.host.as_mut(), event)
        .unwrap();
    ready
        .submit_initial_command(
            rig.host.as_mut(),
            SongCommand::Endpoints(SongEndpoints {
                epoch: SnapshotEpoch(931),
                arrangement: replacement.activation.frame + 128,
                tail_deadline: replacement.activation.frame + 128,
            }),
        )
        .unwrap();
    let end = replacement.activation.frame + 160;
    let mut result = Vec::new();
    let mut ordinal = 0;
    while rig.frame < end {
        let frames = chunks[ordinal % chunks.len()].min(usize::try_from(end - rig.frame).unwrap());
        result.extend(render_block(&mut rig, frames, true));
        ordinal += 1;
    }
    let acknowledgments = rig
        .receipts
        .iter()
        .copied()
        .filter(|ack| {
            matches!(ack,
        SongHostAck::Applied(a) if a.epoch == SnapshotEpoch(931))
        })
        .collect();
    assert!(result.iter().filter(|sample| sample.abs() > 1e-6).count() > 50);
    (result, acknowledgments)
}
#[test]
fn actual_non_aligned_f_a_and_endpoint_are_callback_partition_invariant() {
    for bytes in [false, true] {
        let fixed = partition_transcript(bytes, &[16]);
        let irregular = partition_transcript(bytes, &[1, 7, 3, 11, 2, 16]);
        assert_eq!(fixed.0.len(), irregular.0.len());
        for (frame, (a, b)) in fixed.0.iter().zip(&irregular.0).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "backend {bytes}, sample {frame}: {a} vs {b}"
            );
        }
        assert_eq!(fixed.1, irregular.1);
        assert_eq!(fixed.1.len(), 1);
    }
}

#[test]
fn actual_old_natural_deadlines_before_f_between_f_a_and_at_a_are_not_prolonged() {
    for bytes in [false, true] {
        for relative in [-80i64, -32, 0] {
            let mut rig = Rig::new(bytes);
            let mut twin = Rig::new(bytes);
            let (mut old, old_keys, _) = start_with_authority(&mut rig, 940);
            let (mut reference, _, _) = start_with_authority(&mut twin, 940);
            let mut ready = next_ready(&mut rig, 941);
            let _reference_ready = next_ready(&mut twin, 941);
            assert_eq!(rig.frame, twin.frame);
            let a = rig.frame + 256;
            let deadline = a.checked_add_signed(relative).unwrap();
            let endpoint = SongEndpoints {
                epoch: old.epoch(),
                arrangement: deadline - 96,
                tail_deadline: deadline,
            };
            old.cutoff(rig.host.as_mut(), endpoint).unwrap();
            rig.capture_outbox();
            reference.cutoff(twin.host.as_mut(), endpoint).unwrap();
            twin.capture_outbox();
            let source = old.replacement_source().unwrap();
            assert_eq!(source.endpoints(), endpoint);
            let records = overlay(&ready, 105);
            let replacement = intent(&ready, &source, a, 105, records.len());
            ready
                .prepare_replacement(replacement, source, records)
                .ok()
                .unwrap();
            submit_all(&mut rig, &mut ready, replacement);
            let event = initial_event(&ready, a);
            ready
                .submit_initial_command(rig.host.as_mut(), event)
                .unwrap();
            ready
                .submit_initial_command(
                    rig.host.as_mut(),
                    SongCommand::Endpoints(SongEndpoints {
                        epoch: SnapshotEpoch(941),
                        arrangement: a + 256,
                        tail_deadline: a + 256,
                    }),
                )
                .unwrap();
            rig.capture_outbox();
            let mut nonzero = 0;
            let mut old_master_returned = false;
            while rig.frame < a + 80 {
                let frame = rig.frame;
                let actual = render_block(&mut rig, 8, false);
                let expected = render_block(&mut twin, 8, false);
                for (i, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                    let sample_frame = frame + u64::try_from(i / 2).unwrap();
                    let gain = if sample_frame < a - 64 {
                        1.
                    } else if sample_frame >= a {
                        0.
                    } else {
                        (a - sample_frame) as f32 / 64.
                    };
                    assert!(
                        (actual - expected * gain).abs() < 1e-6,
                        "backend {bytes}, natural deadline {relative}, frame {sample_frame}"
                    );
                    if actual.abs() > 1e-6 {
                        nonzero += 1;
                    }
                }
                for ack in drain(&mut rig) {
                    if let SongHostAck::LeaseReturned(key) = ack {
                        if key.kind == SongResourceKind::Master && old_keys.contains(&key) {
                            assert!(
                                frame >= deadline,
                                "old master returned before its actual natural deadline"
                            );
                            old_master_returned = true;
                        }
                    }
                    if ack.epoch() == old.epoch() {
                        old.receive(ack).unwrap();
                    } else {
                        ready.receive_activation(ack).unwrap();
                    }
                }
                for ack in drain(&mut twin) {
                    reference.receive(ack).unwrap();
                }
            }
            assert!(nonzero > 50);
            assert!(old_master_returned);
            assert_eq!(ready.applied_activation(), Some(replacement.activation));
            old.observe_replacement(ready.take_replacement_commit().unwrap())
                .ok()
                .unwrap();
            if let Backend::Arena { engine, .. } = &mut rig.backend {
                assert!(
                    engine.pop_fault().is_none(),
                    "natural close must not access a returned old slot"
                );
            }
        }
    }
}

fn capacity(rig: &mut Rig) -> SongHostCapacities {
    let epoch = SnapshotEpoch(959);
    rig.host
        .try_song_command(SongCommand::RequestCapacity(epoch))
        .unwrap();
    let _ = render_block(rig, 1, false);
    drain(rig)
        .into_iter()
        .find_map(|ack| match ack {
            SongHostAck::CapacityReport(report) if report.epoch == epoch => Some(report.available),
            _ => None,
        })
        .expect("actual engine capacity report")
}
#[test]
fn actual_guarded_old_returns_force_physical_slot_reuse_without_resetting_new_audio() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let (mut old, old_keys, _) = start_with_authority(&mut rig, 950);
        let original_old_deadline = old.endpoints().tail_deadline;
        let mut ready = next_ready(&mut rig, 951);
        let new_bus_count = ready
            .resources()
            .iter()
            .filter(|key| {
                matches!(
                    key.kind,
                    SongResourceKind::PrivateFx
                        | SongResourceKind::Track
                        | SongResourceKind::Master
                )
            })
            .count();
        let before = capacity(&mut rig);
        assert!(
            usize::try_from(before.bus_slots).unwrap() < new_bus_count,
            "third equal graph cannot fit without reusing returned old physical bus slots"
        );
        let source = old.replacement_source().unwrap();
        let a = rig.frame + 128;
        let records = overlay(&ready, 107);
        let replacement = intent(&ready, &source, a, 107, records.len());
        ready
            .prepare_replacement(replacement, source, records)
            .ok()
            .unwrap();
        submit_all(&mut rig, &mut ready, replacement);
        ready
            .submit_initial_command(
                rig.host.as_mut(),
                SongCommand::Endpoints(SongEndpoints {
                    epoch: SnapshotEpoch(951),
                    arrangement: original_old_deadline + 256,
                    tail_deadline: original_old_deadline + 256,
                }),
            )
            .unwrap();
        render_until(&mut rig, a + 1, false);
        let first = drain(&mut rig);
        let mut returned = Vec::new();
        for ack in first {
            if let SongHostAck::LeaseReturned(key) = ack {
                if old_keys.contains(&key) {
                    returned.push(key);
                }
            }
            if ack.epoch() == old.epoch() {
                old.receive(ack).unwrap();
            } else {
                ready.receive_activation(ack).unwrap();
            }
        }
        old.observe_replacement(ready.take_replacement_commit().unwrap())
            .ok()
            .unwrap();
        for _ in 0..1024 {
            let _ = render_block(&mut rig, 1, false);
            for ack in drain(&mut rig) {
                if let SongHostAck::LeaseReturned(key) = ack {
                    if old_keys.contains(&key) {
                        returned.push(key);
                    }
                }
                if ack.epoch() == old.epoch() {
                    old.receive(ack).unwrap();
                } else {
                    ready.receive_activation(ack).unwrap();
                }
            }
            if old_keys.iter().all(|key| returned.contains(key)) {
                break;
            }
        }
        assert!(
            old_keys.iter().all(|key| returned.contains(key)),
            "all original full keys must return before restaging"
        );
        assert!(capacity(&mut rig).bus_slots > before.bus_slots);
        let mut restaged = next_ready(&mut rig, 952);
        assert_eq!(
            restaged
                .resources()
                .iter()
                .filter(|key| matches!(
                    key.kind,
                    SongResourceKind::PrivateFx
                        | SongResourceKind::Track
                        | SongResourceKind::Master
                ))
                .count(),
            new_bus_count
        );
        let activation = SongActivation {
            epoch: SnapshotEpoch(952),
            frame: rig.frame + 128,
        };
        assert!(activation.frame + 128 < original_old_deadline);
        restaged
            .submit_activation(rig.host.as_mut(), activation)
            .unwrap();
        let event = initial_event(&restaged, activation.frame);
        restaged
            .submit_initial_command(rig.host.as_mut(), event)
            .unwrap();
        let late_event = initial_event(&restaged, original_old_deadline - 32);
        restaged
            .submit_initial_command(rig.host.as_mut(), late_event)
            .unwrap();
        restaged
            .submit_initial_command(
                rig.host.as_mut(),
                SongCommand::Endpoints(SongEndpoints {
                    epoch: SnapshotEpoch(952),
                    arrangement: original_old_deadline + 128,
                    tail_deadline: original_old_deadline + 128,
                }),
            )
            .unwrap();
        let mut nonzero = 0;
        while rig.frame < original_old_deadline + 64 {
            let frame = rig.frame;
            let output = render_block(&mut rig, 16, false);
            if frame >= original_old_deadline - 32 {
                nonzero += output.iter().filter(|sample| sample.abs() > 1e-6).count();
            }
            for ack in drain(&mut rig) {
                if ack.epoch() == SnapshotEpoch(952) {
                    restaged.receive_activation(ack).unwrap();
                } else {
                    assert!(
                        matches!(
                            ack,
                            SongHostAck::CapacityReport(_) | SongHostAck::ClockReport(_)
                        ),
                        "unexpected stale owner work: {ack:?}"
                    );
                }
            }
        }
        assert!(
            nonzero > 50,
            "new graph must remain audible across the old original deadline"
        );
        assert_eq!(restaged.applied_activation(), Some(activation));
        if let Backend::Arena { engine, .. } = &mut rig.backend {
            assert!(
                engine.pop_fault().is_none(),
                "old completion must not reset or fault a reused slot"
            );
        }
    }
}

#[test]
fn cached_failed_event_precedes_boundary_outcome_under_real_pressure() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let (mut old, old_keys, families, mut bad) = start_with_event_authority(&mut rig, 970);
        let (mut ready, replacement) = ready_replacement(&mut rig, &mut old, 971, 256);
        let a = replacement.activation.frame;
        let f = a - 64;
        let mute = SongMute {
            epoch: old.epoch(),
            instrument: families[0],
            muted: false,
            frame: f + 20,
        };
        bad.frame = f + 24;
        bad.event.inst = vactr::dsp::graph::InstId::new(u32::MAX);
        assert!(SongCommand::Event(bad).valid());
        rig.host.try_song_command(SongCommand::Mute(mute)).unwrap();
        rig.host.try_song_command(SongCommand::Event(bad)).unwrap();
        render_until(&mut rig, f + 1, true);
        let marker = SnapshotEpoch(979);
        let count = fill_actual_critical_ring(&mut rig, bytes, marker);
        assert!(rig.frame < mute.frame);
        rig.host
            .try_song_command(SongCommand::CancelPreparation(SnapshotEpoch(971)))
            .unwrap();
        render_until(&mut rig, a + 1, false);
        let physical = drain(&mut rig);
        assert_eq!(
            physical
                .iter()
                .filter(|ack| matches!(ack,
            SongHostAck::ClockReport(r) if !bytes && r.request.epoch == marker)
                    || matches!(ack, SongHostAck::CapacityReport(r) if bytes && r.epoch == marker))
                .count(),
            count
        );
        let mut outcomes = Vec::new();
        for _ in 0..old_keys.len() + ready.resources().len() + 5 {
            let _ = render_block(&mut rig, 1, false);
            outcomes.extend(drain(&mut rig));
            if outcomes.contains(&SongHostAck::Applied(replacement.activation)) {
                break;
            }
        }
        let critical: Vec<_> = outcomes
            .iter()
            .copied()
            .filter(|ack| {
                matches!(
                    ack,
                    SongHostAck::Rejected { .. } | SongHostAck::Muted(_) | SongHostAck::Applied(_)
                )
            })
            .collect();
        assert_eq!(
            critical,
            vec![
                SongHostAck::Rejected {
                    epoch: SnapshotEpoch(971),
                    reason: SongRejectCode::HostFault
                },
                SongHostAck::Muted(mute),
                SongHostAck::Rejected {
                    epoch: old.epoch(),
                    reason: SongRejectCode::Malformed
                },
                SongHostAck::Applied(replacement.activation),
            ]
        );
        let mut delayed = Vec::new();
        for ack in outcomes {
            if matches!(ack, SongHostAck::LeaseReturned(k) if k.epoch == old.epoch()) {
                old.receive(ack).unwrap();
            } else if ack == SongHostAck::Applied(replacement.activation) {
                ready.receive_activation(ack).unwrap();
            } else if matches!(ack, SongHostAck::LeaseReturned(_)) {
                delayed.push(ack);
            }
        }
        old.observe_replacement(ready.take_replacement_commit().unwrap())
            .ok()
            .unwrap();
        let mut cleanup = SongHostPreparation::retire(ready);
        for ack in delayed {
            cleanup.receive(ack).unwrap();
        }
        rig.cleanup(&mut cleanup, &mut old);
        assert_eq!(cleanup.progress(), SongPreparationProgress::Retired);
    }
}

#[test]
fn stale_issued_previous_owner_refuses_without_changing_current_pcm() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut twin = Rig::new(bytes);
        let (mut expired, keys, _) = start_with_authority(&mut rig, 980);
        let mut expired_twin = start(&mut twin, 980);
        let source = expired.replacement_source().unwrap();
        finish_old(&mut rig, &mut expired);
        finish_old(&mut twin, &mut expired_twin);
        for key in keys {
            assert!(rig.receipts.contains(&SongHostAck::LeaseReturned(key)));
        }
        let mut current = start(&mut rig, 981);
        let mut reference = start(&mut twin, 981);
        let mut ready = next_ready(&mut rig, 982);
        let _baseline = next_ready(&mut twin, 982);
        assert_eq!(rig.frame, twin.frame);
        let replacement = intent(&ready, &source, rig.frame + 128, 131, 0);
        ready
            .prepare_replacement(replacement, source, Vec::new())
            .ok()
            .unwrap();
        submit_all(&mut rig, &mut ready, replacement);
        while rig.frame < replacement.activation.frame + 80 {
            assert_eq!(
                render_block(&mut rig, 16, false),
                render_block(&mut twin, 16, false),
                "stale physical previous owner changed current PCM, backend {bytes}"
            );
            for ack in drain(&mut rig) {
                if ack.epoch() == current.epoch() {
                    current.receive(ack).unwrap();
                } else {
                    assert!(
                        matches!(ack, SongHostAck::ActivationRejected { activation, reason: SongRejectCode::StaleEpoch } if activation == replacement.activation)
                    );
                    ready.receive_activation(ack).unwrap();
                }
            }
            for ack in drain(&mut twin) {
                reference.receive(ack).unwrap();
            }
        }
        assert_eq!(
            ready.activation_rejection(),
            Some(SongRejectCode::StaleEpoch)
        );
        assert_eq!(ready.applied_activation(), None);
        assert!(ready.take_replacement_commit().is_none());
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup, &mut current);
        assert_eq!(cleanup.progress(), SongPreparationProgress::Cancelled);
    }
}

pub(super) fn cancel_ordinary_ready_twins(
    rig: &mut Rig,
    old: &mut SongTransport,
    twin: &mut Rig,
    reference: &mut SongTransport,
) -> SongActivation {
    let mut ready = next_ready(rig, 991);
    let mut baseline = next_ready(twin, 991);
    let keys = ready.resources().to_vec();
    let activation = SongActivation {
        epoch: SnapshotEpoch(991),
        frame: rig.frame + 8192,
    };
    ready
        .submit_activation(rig.host.as_mut(), activation)
        .unwrap();
    rig.capture_outbox();
    baseline
        .submit_activation(twin.host.as_mut(), activation)
        .unwrap();
    twin.capture_outbox();
    for r in [&mut *rig, &mut *twin] {
        r.host
            .try_song_command(SongCommand::Endpoints(SongEndpoints {
                epoch: activation.epoch,
                arrangement: activation.frame + 128,
                tail_deadline: activation.frame + 128,
            }))
            .unwrap();
        r.capture_outbox();
    }
    // Direct pending cancellation exercises actual Engine ingress. Keep each
    // original Ready alive until every physical key returns; do not fabricate
    // host-owner cancellation flags or a cleanup completion receipt.
    for r in [&mut *rig, &mut *twin] {
        r.host
            .try_song_command(SongCommand::CancelPreparation(activation.epoch))
            .unwrap();
        r.capture_outbox();
    }
    for _ in 0..keys.len() * 4 + 8 {
        assert_eq!(render_block(rig, 1, false), render_block(twin, 1, false));
        for ack in drain(rig) {
            if ack.epoch() == old.epoch() {
                old.receive(ack).unwrap();
            } else if matches!(ack, SongHostAck::ActivationRejected { .. }) {
                ready.receive_activation(ack).unwrap();
            } else {
                assert!(matches!(
                    ack,
                    SongHostAck::LeaseReturned(_) | SongHostAck::PreparationCancelled(_)
                ));
            }
        }
        for ack in drain(twin) {
            if ack.epoch() == reference.epoch() {
                reference.receive(ack).unwrap();
            } else if matches!(ack, SongHostAck::ActivationRejected { .. }) {
                baseline.receive_activation(ack).unwrap();
            } else {
                assert!(matches!(
                    ack,
                    SongHostAck::LeaseReturned(_) | SongHostAck::PreparationCancelled(_)
                ));
            }
        }
        if rig
            .receipts
            .contains(&SongHostAck::PreparationCancelled(activation.epoch))
            && twin
                .receipts
                .contains(&SongHostAck::PreparationCancelled(activation.epoch))
        {
            break;
        }
    }
    assert_eq!(ready.activation_rejection(), Some(SongRejectCode::NotReady));
    assert_eq!(
        baseline.activation_rejection(),
        Some(SongRejectCode::NotReady)
    );
    assert!(rig
        .receipts
        .contains(&SongHostAck::PreparationCancelled(activation.epoch)));
    assert!(twin
        .receipts
        .contains(&SongHostAck::PreparationCancelled(activation.epoch)));
    assert_eq!(
        rig.receipts
            .iter()
            .filter(|ack| **ack
                == SongHostAck::ActivationRejected {
                    activation,
                    reason: SongRejectCode::NotReady
                })
            .count(),
        1
    );
    for key in keys {
        assert!(rig.receipts.contains(&SongHostAck::LeaseReturned(key)));
    }
    assert!(!rig
        .receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::Applied(a) if a.epoch == activation.epoch)));
    activation
}
