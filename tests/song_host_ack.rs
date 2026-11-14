//! Real one-message host polling preserves ownership acknowledgments under pressure.
use std::sync::Arc;
use vactr::host::caps::{AudioHost, GraphHandle, HostSigs, SampleData};
use vactr::host::noop::NoopHost;
use vactr::host::wire::{AudioEvent, CtlMsg, HostMsg, SlotControl};
use vactr::song::{
    routing::{SongActivation, SongCommand, SongHostAck, SongRejectCode},
    SnapshotEpoch,
};
use vactr::vm::fail::FailCode;
struct Unsupported {
    replies: Vec<HostMsg>,
    drains: usize,
}
impl AudioHost for Unsupported {
    fn send(&mut self, _: AudioEvent) {}
    fn control(&mut self, _: SlotControl) {}
    fn post(&mut self, _: CtlMsg) {}
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        self.drains += 1;
        out.append(&mut self.replies);
    }
    fn now(&self) -> f64 {
        0.
    }
    fn swap_graph(&mut self, _: GraphHandle) {}
    fn install_sample(&mut self, _: u32, _: Arc<SampleData>) {}
    fn retire_sample(&mut self, _: u32) {}
    fn analysis(&self) -> HostSigs {
        HostSigs::default()
    }
}
fn ack(epoch: u64) -> HostMsg {
    HostMsg::Song(SongHostAck::Rejected {
        epoch: SnapshotEpoch(epoch),
        reason: SongRejectCode::NotReady,
    })
}
#[test]
fn unsupported_poll_consumes_nothing_and_does_not_call_legacy_drain() {
    let mut host = Unsupported {
        replies: vec![ack(1), ack(2)],
        drains: 0,
    };
    for _ in 0..3 {
        assert_eq!(host.poll_msg().unwrap_err().code, FailCode::HostUnavailable);
    }
    assert_eq!(host.drains, 0);
    assert_eq!(host.replies, vec![ack(1), ack(2)]);
    let mut out = vec![ack(0)];
    host.drain(&mut out);
    assert_eq!(out, vec![ack(0), ack(1), ack(2)]);
}
#[test]
fn noop_poll_is_explicit_supported_empty_and_legacy_drain_preserves_output() {
    let mut host = NoopHost;
    for _ in 0..100 {
        assert_eq!(host.poll_msg().unwrap(), None);
    }
    let mut out = vec![ack(1)];
    host.drain(&mut out);
    assert_eq!(out, vec![ack(1)]);
}
#[cfg(feature = "host-native")]
fn activation_ack(epoch: u64, frame: u64) -> HostMsg {
    HostMsg::Song(SongHostAck::ActivationRejected {
        activation: SongActivation {
            epoch: SnapshotEpoch(epoch),
            frame,
        },
        reason: SongRejectCode::NotReady,
    })
}
#[cfg(feature = "host-native")]
fn native() -> (
    vactr::host::native::NativeAudioHost,
    vactr::host::native::audio::AudioSide,
) {
    let mut caps = vactr::dsp::caps::CapabilitySet::native();
    caps.max_voices = 1;
    vactr::host::native::NativeAudioHost::headless(48000, caps, 1)
}
#[cfg(feature = "host-native")]
#[test]
fn native_headless_poll_retains_more_than64_critical_identities_exactly_once() {
    let (mut host, mut side) = native();
    for epoch in 1..=130 {
        host.post_song(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(epoch),
            frame: u64::MAX,
        }));
    }
    let mut out = [0.; 32];
    side.render(&mut out, 2);
    assert_eq!(
        host.poll_msg().unwrap(),
        None,
        "one internal startup receipt"
    );
    for epoch in 1..=70 {
        assert_eq!(
            host.poll_msg().unwrap(),
            Some(activation_ack(epoch, u64::MAX))
        );
    }
    let mut rest = Vec::new();
    host.drain(&mut rest);
    let critical: Vec<_> = rest
        .into_iter()
        .filter(|m| matches!(m, HostMsg::Song(_)))
        .collect();
    assert_eq!(
        critical,
        (71..=130)
            .map(|epoch| activation_ack(epoch, u64::MAX))
            .collect::<Vec<_>>()
    );
    assert_eq!(host.poll_msg().unwrap(), None);
    assert!(out.iter().all(|v| *v == 0.));
}
#[cfg(feature = "host-native")]
#[test]
fn native_poll_preserves_queued_ack_before_drop_delta_and_reports_delta_once() {
    let (mut host, mut side) = native();
    host.post_song(SongCommand::Activate(SongActivation {
        epoch: SnapshotEpoch(1),
        frame: 0,
    }));
    let mut out = [0.; 32];
    side.render(&mut out, 2);
    for _ in 0..vactr::dsp::ring::EVENT_CAPACITY + 3 {
        host.send(AudioEvent::new(
            100.,
            vactr::sched::slots::SlotId::new(1),
            0,
            vactr::dsp::graph::InstId::new(0),
        ));
    }
    assert_eq!(host.dropped(), 3);
    assert_eq!(
        host.poll_msg().unwrap(),
        None,
        "one internal startup receipt"
    );
    assert_eq!(host.poll_msg().unwrap(), Some(activation_ack(1, 0)));
    let mut drops = Vec::new();
    while let Some(message) = host.poll_msg().unwrap() {
        if let HostMsg::Counters { dropped, .. } = message {
            if dropped > 0 {
                drops.push(dropped);
            }
        }
    }
    assert_eq!(drops, vec![3]);
    assert_eq!(host.poll_msg().unwrap(), None);
    let mut old = Vec::new();
    host.drain(&mut old);
    assert!(old.is_empty());
    host.send(AudioEvent::new(
        100.,
        vactr::sched::slots::SlotId::new(1),
        0,
        vactr::dsp::graph::InstId::new(0),
    ));
    assert_eq!(
        host.poll_msg().unwrap(),
        Some(HostMsg::Counters {
            late: 0,
            dropped: 1,
            stolen: 0,
            skipped: 0
        })
    );
}

#[cfg(feature = "host-native")]
#[test]
fn native_poll_collects_retired_garbage_without_releasing_live_sample() {
    let (mut host, mut side) = native();
    let data = Arc::new(SampleData {
        rate: 48000,
        channels: 1,
        frames: vec![0.; 8].into_boxed_slice(),
    });
    host.install_sample(7, data.clone());
    let mut out = [0.; 32];
    side.render(&mut out, 2);
    assert_eq!(Arc::strong_count(&data), 2);
    assert_eq!(
        host.poll_msg().unwrap(),
        None,
        "one internal startup receipt"
    );
    assert_eq!(
        host.poll_msg().unwrap(),
        Some(HostMsg::Installed {
            resource: 7,
            gen: 1
        })
    );
    assert_eq!(Arc::strong_count(&data), 2);
    host.retire_sample(7);
    side.render(&mut out, 2);
    assert_eq!(Arc::strong_count(&data), 2);
    let mut receipts = Vec::new();
    host.drain(&mut receipts);
    assert_eq!(
        receipts
            .iter()
            .filter(|m| **m == HostMsg::Retired { resource: 7 })
            .count(),
        1
    );
    assert_eq!(Arc::strong_count(&data), 1);
}
