//! Production clock refresh state under cached, refused and actual native observations.
use super::*;
use crate::host::caps::{GraphHandle, HostSigs, SampleData, SongCommandRefusal};
use crate::host::wire::{AudioEvent, CtlMsg, SlotControl};
use crate::song::routing::{SongClockFailure, SongClockReport, SongRejectCode};
use std::sync::Arc;

struct CachedHost {
    clock: SongHostClock,
    refused: bool,
    commands: Vec<SongCommand>,
}
impl AudioHost for CachedHost {
    fn song_clock(&self) -> Result<SongHostClock, Failure> {
        Ok(self.clock)
    }
    fn try_song_command(&mut self, command: SongCommand) -> Result<(), SongCommandRefusal> {
        if self.refused {
            Err(SongCommandRefusal {
                command,
                error: SongSubmitError::Backpressure,
            })
        } else {
            self.commands.push(command);
            Ok(())
        }
    }
    fn send(&mut self, _: AudioEvent) {}
    fn control(&mut self, _: SlotControl) {}
    fn post(&mut self, _: CtlMsg) {}
    fn drain(&mut self, _: &mut Vec<HostMsg>) {}
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
fn host() -> CachedHost {
    CachedHost {
        clock: SongHostClock {
            frame: 1000,
            sample_rate: 8000,
        },
        refused: false,
        commands: Vec::new(),
    }
}
fn report(request: SongClockRequest, frame: u64) -> SongHostAck {
    SongHostAck::ClockReport(SongClockReport {
        request,
        clock: SongHostClock {
            frame,
            sample_rate: 8000,
        },
    })
}
#[test]
fn cached_observation_refreshes_with_exact_nonce_and_retained_pending_clock() {
    let mut state = SongRuntime {
        nonce: 7,
        ..SongRuntime::default()
    };
    let mut host = host();
    let epoch = SnapshotEpoch(4);
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    let request = SongClockRequest { epoch, request: 8 };
    assert_eq!(host.commands, [SongCommand::RequestClock(request)]);
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    assert_eq!(host.commands.len(), 1);
    assert_eq!(state.clock, Some(host.clock));
    assert!(state.receive_clock(report(request, 1128)));
    // The provider still returns the earlier cached observation until another report.
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    assert_eq!(state.clock.unwrap().frame, 1128);
    assert_eq!(state.request.unwrap().request, 9);
    assert_eq!(host.commands.len(), 2);
}
#[test]
fn refused_refresh_retries_same_nonce_and_preserves_foreign_reports() {
    let mut state = SongRuntime::default();
    let mut host = host();
    host.refused = true;
    let epoch = SnapshotEpoch(2);
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    assert_eq!(state.nonce, 0);
    assert_eq!(state.request, None);
    host.refused = false;
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    let request = state.request.unwrap();
    assert_eq!(request.request, 1);
    let foreign = SongClockRequest {
        epoch: SnapshotEpoch(3),
        ..request
    };
    assert!(!state.receive_clock(report(foreign, 9000)));
    assert!(!state.receive_clock(report(
        SongClockRequest {
            request: 0,
            ..request
        },
        9000
    )));
    assert_eq!(state.request, Some(request));
    assert_eq!(state.clock, Some(host.clock));
    assert!(state.receive_clock(report(request, 1128)));
    // Preparation barriers own the sender while their owner is present.
    state.refresh_clock(&mut host, None).unwrap();
    assert_eq!(host.commands.len(), 1);
}
#[test]
fn rejected_refresh_is_correlated_and_nonce_exhaustion_is_explicit() {
    let mut state = SongRuntime::default();
    let mut host = host();
    let epoch = SnapshotEpoch(2);
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    let request = state.request.unwrap();
    assert!(
        state.receive_clock(SongHostAck::ClockRejected(SongClockFailure {
            request,
            reason: SongRejectCode::HostFault,
        }))
    );
    assert_eq!(state.request, None);
    assert!(state.refresh_clock(&mut host, Some(epoch)).is_err());
    assert_eq!(state.clock, Some(host.clock));
    state.nonce = u64::MAX;
    assert!(state.refresh_clock(&mut host, Some(epoch)).is_err());
    assert_eq!(host.commands.len(), 1);
}
#[cfg(feature = "host-native")]
#[test]
fn actual_native_clock_advances_while_a_report_is_pending() {
    let (mut host, mut side) =
        crate::host::native::audio::NativeAudioHost::headless(48000, CapabilitySet::native(), 8);
    let mut state = SongRuntime::default();
    let epoch = SnapshotEpoch(3);
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    let request = state.request.unwrap();
    let mut pcm = [0.; 256];
    side.render(&mut pcm, 2);
    state.refresh_clock(&mut host, Some(epoch)).unwrap();
    assert_eq!(state.clock.unwrap().frame, 128);
    assert_eq!(state.request, Some(request));
    let mut messages = Vec::new();
    host.drain(&mut messages);
    let actual = messages
        .into_iter()
        .find_map(|message| match message {
            HostMsg::Song(SongHostAck::ClockReport(report)) if report.request == request => {
                Some(SongHostAck::ClockReport(report))
            }
            _ => None,
        })
        .expect("actual Engine-produced clock receipt");
    assert!(state.receive_clock(actual));
    assert_eq!(state.clock.unwrap().frame, 128);
    assert_eq!(state.request, None);
}

#[cfg(feature = "host-native")]
fn genuine_ready(
    audio: &mut crate::host::native::audio::NativeAudioHost,
    side: &mut crate::host::native::audio::AudioSide,
    caps: CapabilitySet,
    epoch: SnapshotEpoch,
) -> SongReadyBundle {
    use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let context = crate::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 16,
            max_source_bytes: 100_000,
            max_banks: 16,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    let candidate = crate::session::song::evaluate_song_candidate(
        "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone}] duration: 4} tail-seconds: 0 > play-song",
        "clock.vact", 1, epoch, &context,
    ).unwrap();
    let prepared = crate::song::prepare_song(candidate).unwrap();
    let mut owner = SongHostPreparation::begin(
        prepared,
        SongPreparationLimits {
            capabilities: caps,
            song: SongLimits::default(),
            max_resources: 128,
            max_pending_records: 4096,
            max_graph_bytes: 65536,
            max_work: 8_000_000,
        },
    )
    .map_err(|r| r.failure)
    .unwrap();
    let mut pcm = [0.; 256];
    for _ in 0..512 {
        owner.submit(audio).unwrap();
        side.render(&mut pcm, 2);
        let mut messages = Vec::new();
        audio.drain(&mut messages);
        for message in messages {
            if let HostMsg::Song(ack) = message {
                owner.receive(ack).unwrap();
            }
        }
        if owner.progress() == SongPreparationProgress::Ready {
            break;
        }
    }
    owner.take_ready().unwrap()
}

#[cfg(feature = "host-native")]
#[test]
fn exact_invalid_clock_aborts_original_runtime_owner_and_retains_cleanup() {
    use crate::dsp::{arena::StoreKind, engine::EngineConfig};
    use crate::host::{caps::Hosts, native::audio::NativeAudioHost, noop::NoopHost};
    let mut caps = CapabilitySet::native();
    caps.max_voices = 2;
    let mut config = EngineConfig::new(
        &caps,
        8000.,
        crate::host::native::audio::MAX_BLOCK,
        StoreKind::NativeArc,
    );
    config.bus_slots = 12;
    let (mut audio, mut side) = NativeAudioHost::headless_with_config(config, 64).unwrap();
    let ready = genuine_ready(&mut audio, &mut side, caps, SnapshotEpoch(4));
    let floor = ready.clock_request_floor();
    assert!(floor > 1, "actual accepted preparation barriers");
    let activation = audio.song_clock().unwrap().frame + 960;
    let mut hosts = Hosts::noop();
    hosts.audio = Box::new(audio);
    let (mut runtime, _) = Runtime::new(hosts, Rc::new(NoopHost), caps, RuntimeConfig::default());
    runtime
        .start_song(ready, activation)
        .map_err(|r| r.failure)
        .unwrap();
    assert_eq!(runtime.song.nonce, floor);
    runtime
        .song
        .refresh_clock(runtime.hosts.audio.as_mut(), Some(SnapshotEpoch(4)))
        .unwrap();
    let request = runtime.song.request.unwrap();
    assert_eq!(request.request, floor + 1);
    // Foreign malformed reports cannot fail or unblock the original owner.
    let malformed = |request| {
        SongHostAck::ClockReport(SongClockReport {
            request,
            clock: SongHostClock {
                frame: 0,
                sample_rate: 0,
            },
        })
    };
    let foreign = malformed(SongClockRequest {
        epoch: SnapshotEpoch(99),
        ..request
    });
    assert_eq!(runtime.dispatch_owned_song_ack(foreign), Err(foreign));
    assert_eq!(runtime.song.request, Some(request));
    runtime.dispatch_owned_song_ack(malformed(request)).unwrap();
    assert_eq!(runtime.song.request, None);
    let mut report = TickReport::default();
    runtime.tick_song(&mut report);
    assert_eq!(runtime.song_state(), Some(SongTransportState::Failed));
    assert_eq!(report.faults.len(), 1);
    let transport = &mut runtime.song.owners[0].transport;
    assert_eq!(transport.epoch(), SnapshotEpoch(4));
    assert_eq!(transport.state(), SongTransportState::Failed);
    assert!(
        transport.take_cancelled().is_err(),
        "original authority stays retained until actual returns"
    );
}

#[cfg(feature = "host-native")]
#[test]
fn rejected_new_epoch_clock_preserves_other_genuine_ready_owner() {
    use crate::dsp::{arena::StoreKind, engine::EngineConfig};
    use crate::host::{caps::Hosts, native::audio::NativeAudioHost, noop::NoopHost};
    let mut caps = CapabilitySet::native();
    caps.max_voices = 2;
    let mut config = EngineConfig::new(
        &caps,
        8000.,
        crate::host::native::audio::MAX_BLOCK,
        StoreKind::NativeArc,
    );
    config.bus_slots = 24;
    let (mut audio, mut side) = NativeAudioHost::headless_with_config(config, 64).unwrap();
    let old = genuine_ready(&mut audio, &mut side, caps, SnapshotEpoch(4));
    let next = genuine_ready(&mut audio, &mut side, caps, SnapshotEpoch(5));
    let activation = audio.song_clock().unwrap().frame + 960;
    let mut hosts = Hosts::noop();
    hosts.audio = Box::new(audio);
    let (mut runtime, _) = Runtime::new(hosts, Rc::new(NoopHost), caps, RuntimeConfig::default());
    runtime
        .start_song(old, activation)
        .map_err(|r| r.failure)
        .unwrap();
    let mut pcm = [0.; 256];
    for _ in 0..32 {
        let mut report = TickReport::default();
        runtime.take_host_msgs(&mut report);
        runtime.tick_song(&mut report);
        assert!(report.faults.is_empty(), "{:?}", report.faults);
        side.render(&mut pcm, 2);
        if runtime.song.owners[0]
            .transport
            .applied_activation()
            .is_some()
        {
            break;
        }
    }
    assert_eq!(
        runtime.song.owners[0].transport.state(),
        SongTransportState::Playing
    );
    runtime
        .start_song(next, activation + 4096)
        .map_err(|r| r.failure)
        .unwrap();
    // Drain the old owner's accepted observation before posting a new one.
    let mut prior = TickReport::default();
    runtime.take_host_msgs(&mut prior);
    assert!(prior.faults.is_empty(), "{:?}", prior.faults);
    assert_eq!(runtime.song.request, None);
    runtime
        .song
        .refresh_clock(runtime.hosts.audio.as_mut(), Some(SnapshotEpoch(5)))
        .unwrap();
    let request = runtime.song.request.unwrap();
    assert_eq!(request.epoch, SnapshotEpoch(5));
    // The exact new request failure must not poison the valid old timebase owner.
    assert!(runtime
        .song
        .receive_clock(SongHostAck::ClockRejected(SongClockFailure {
            request,
            reason: SongRejectCode::HostFault,
        })));
    let mut report = TickReport::default();
    runtime.tick_song(&mut report);
    assert_eq!(report.faults.len(), 1);
    assert_eq!(runtime.song.owners[0].transport.epoch(), SnapshotEpoch(4));
    assert_eq!(
        runtime.song.owners[0].transport.state(),
        SongTransportState::Playing
    );
    assert!(runtime.song.owners[0].transport.failure().is_none());
    assert_eq!(runtime.song.owners[1].transport.epoch(), SnapshotEpoch(5));
    assert_eq!(
        runtime.song.owners[1].transport.state(),
        SongTransportState::Failed
    );
    assert!(runtime.song.owners[1].transport.take_cancelled().is_err());
}
