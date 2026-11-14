//! Genuine native capacity receipts preserve public provenance and legacy playback.
#![cfg(feature = "host-native")]
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::cells::CellId;
use vactr::host::caps::AudioHost;
use vactr::host::native::NativeAudioHost;
use vactr::host::wire::{CtlMsg, HostMsg};
use vactr::song::routing::{SongActivation, SongCommand, SongHostAck, SongRejectCode};
use vactr::song::SnapshotEpoch;

fn caps() -> CapabilitySet {
    let mut caps = CapabilitySet::native();
    caps.max_voices = 1;
    caps
}
fn runtime(
    audio: Box<dyn AudioHost>,
) -> (
    vactr::sched::runtime::Runtime,
    vactr::ns::evaluator::Evaluator,
) {
    use vactr::host::{caps::Hosts, noop::NoopHost};
    let mut hosts = Hosts::noop();
    hosts.audio = audio;
    let (runtime, sink) = vactr::sched::runtime::Runtime::new(
        hosts,
        std::rc::Rc::new(NoopHost),
        caps(),
        vactr::sched::runtime::RuntimeConfig::default(),
    );
    let evaluator = vactr::ns::evaluator::Evaluator::new(
        vactr::ns::namespace::Prelude::core(),
        Box::new(NoopHost),
        Box::new(sink),
    );
    (runtime, evaluator)
}
#[test]
fn legacy_constructor_and_ordinary_cell_traffic_emit_no_song_receipts() {
    let (mut host, mut side) = NativeAudioHost::headless(48000, caps(), 2);
    side.render(&mut [0.; 32], 2);
    assert!(host.poll_msg().unwrap().is_none());
    assert_eq!(host.song_remaining_capacities().unwrap().cell_slots, 2);
    host.post(CtlMsg::CellInit {
        cell: CellId::new(0),
        epoch: 1,
        value: 0.25,
    });
    side.render(&mut [0.; 32], 2);
    let mut messages = Vec::new();
    host.drain(&mut messages);
    assert!(messages.iter().all(|m| !matches!(m, HostMsg::Song(_))));
    let (mut rt, mut evaluator) = runtime(Box::new(host));
    assert!(rt.tick(&mut evaluator, 0.).faults.is_empty());
}
#[test]
fn poll_consumes_only_one_ack_and_public_same_epoch_receipt_survives() {
    let (mut host, mut side) = NativeAudioHost::headless(48000, caps(), 1);
    host.post_song(SongCommand::RequestCapacity(SnapshotEpoch(1)));
    host.post_song(SongCommand::Activate(SongActivation {
        epoch: SnapshotEpoch(1),
        frame: 0,
    }));
    side.render(&mut [0.; 32], 2);
    assert!(host.poll_msg().unwrap().is_none());
    let Some(HostMsg::Song(SongHostAck::CapacityReport(report))) = host.poll_msg().unwrap() else {
        panic!("public capacity reply remains queued")
    };
    assert_eq!(report.epoch, SnapshotEpoch(1));
    assert_eq!(report.available.sample_rate, 48000);
    assert_eq!(
        host.poll_msg().unwrap(),
        Some(HostMsg::Song(SongHostAck::ActivationRejected {
            activation: SongActivation {
                epoch: SnapshotEpoch(1),
                frame: 0
            },
            reason: SongRejectCode::NotReady
        }))
    );
}
#[test]
fn unconfigured_quad_has_no_internal_probe_but_forwards_typed_public_failure() {
    let (mut host, mut side) = NativeAudioHost::headless_quad(48000, caps(), 1);
    side.render(&mut [0.; 32], 4);
    let mut messages = Vec::new();
    host.drain(&mut messages);
    assert!(messages.iter().all(|m| !matches!(m, HostMsg::Song(_))));
    assert!(host.song_remaining_capacities().is_err());
    host.post_song(SongCommand::RequestCapacity(SnapshotEpoch(u64::MAX)));
    side.render(&mut [0.; 32], 4);
    host.drain(&mut messages);
    assert!(
        messages.contains(&HostMsg::Song(SongHostAck::CapacityRejected {
            epoch: SnapshotEpoch(u64::MAX),
            reason: SongRejectCode::NotReady
        }))
    );
}
#[test]
fn native_runtime_retains130_rejections_without_internal_capacity_receipts() {
    let (mut host, mut side) = NativeAudioHost::headless(48000, caps(), 1);
    for epoch in 1..=130 {
        host.post_song(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(epoch),
            frame: 0,
        }));
    }
    side.render(&mut [0.; 32], 2);
    let (mut rt, mut evaluator) = runtime(Box::new(host));
    rt.expect_song_receipts(SnapshotEpoch(1)).unwrap();
    let mut receipts = Vec::new();
    for _ in 0..260 {
        rt.tick(&mut evaluator, 0.);
        while let Some(receipt) = rt.pop_song_receipt() {
            receipts.push(receipt);
        }
        if receipts.len() == 130 {
            break;
        }
    }
    assert_eq!(
        receipts,
        (1..=130)
            .map(|epoch| SongHostAck::ActivationRejected {
                activation: SongActivation {
                    epoch: SnapshotEpoch(epoch),
                    frame: 0
                },
                reason: SongRejectCode::NotReady
            })
            .collect::<Vec<_>>()
    );
}
#[test]
fn public_queries_retain_fifo_full_width_identity_while_drain_filters_internal() {
    let (mut host, mut side) = NativeAudioHost::headless(48000, caps(), 1);
    for epoch in [1, u64::MAX, 1] {
        host.post_song(SongCommand::RequestCapacity(SnapshotEpoch(epoch)));
    }
    side.render(&mut [0.; 32], 2);
    let mut messages = Vec::new();
    host.drain(&mut messages);
    let reports: Vec<_> = messages
        .into_iter()
        .filter_map(|message| match message {
            HostMsg::Song(SongHostAck::CapacityReport(report)) => Some(report),
            _ => None,
        })
        .collect();
    assert_eq!(
        reports.iter().map(|r| r.epoch.0).collect::<Vec<_>>(),
        vec![1, u64::MAX, 1]
    );
    assert!(reports
        .windows(2)
        .all(|pair| pair[0].serial < pair[1].serial));
    assert!(host.song_remaining_capacities().is_ok());
}

#[test]
fn legacy_runtime_consumes_internal_startup_reply_without_prior_host_polling() {
    let (host, mut side) = NativeAudioHost::headless(48000, caps(), 1);
    side.render(&mut [0.; 32], 2);
    // Leave the constructor query result queued; no host poll/drain or song expectation.
    let (mut rt, mut evaluator) = runtime(Box::new(host));
    let report = rt.tick(&mut evaluator, 0.);
    assert!(report.faults.is_empty(), "{:?}", report.faults);
    assert!(rt.pop_song_receipt().is_none());
    assert!(rt.pending_song().is_none());
}
