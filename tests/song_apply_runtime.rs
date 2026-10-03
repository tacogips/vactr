//! Actual atomic replacement audio and correlated receipt witnesses.
pub use vactr::{dsp, reader, sched, song, types, value, vm};
#[path = "../src/host/wasm/abi.rs"]
pub mod browser_abi;
#[path = "../src/host/wasm/messages.rs"]
pub mod browser_messages;
pub mod host {
    pub use vactr::host::{caps, wire};
    pub mod wasm {
        pub use crate::browser_abi as abi;
        pub use crate::browser_messages as messages;
    }
}
use browser_messages::{HostState, WasmAudioHost};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};
use vactr::dsp::{
    arena::StoreKind,
    caps::CapabilitySet,
    cells::AtomicCells,
    engine::{Engine, EngineConfig, SongStagingConfig},
    ring::{self, ByteInbox, EngineIo, EventRing, Garbage, SpscRing},
};
use vactr::host::{
    caps::{
        AudioHost, SampleData, SongHostPreparation, SongPreparationLimits, SongPreparationProgress,
        SongReadyBundle,
    },
    native::audio::{AudioSide, NativeAudioHost},
    wire::HostMsg,
};
use vactr::song::{
    assets::{DecodedSongAssetFactory, SongAssetLimits},
    routing::*,
    PreparedSong, SnapshotEpoch, SongLimits,
};
use vactr::value::intern::intern_kw;

struct Probe;
thread_local! { static MEMORY: std::cell::Cell<Option<(usize,usize)>> = const { std::cell::Cell::new(None) }; }
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a + 1, d)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a, d + 1)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a + 1, d + 1)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::realloc(&std::alloc::System, ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;
fn callback(run: impl FnOnce()) {
    MEMORY.with(|c| c.set(Some((0, 0))));
    run();
    let counts = MEMORY.with(|c| c.replace(None).unwrap());
    assert_eq!(counts, (0, 0), "actual configured callback memory activity");
}
const PLAIN: &str = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
fn candidate(code: &str, epoch: SnapshotEpoch) -> PreparedSong {
    candidate_with_pcm(code, epoch, 4)
}
fn candidate_with_pcm(code: &str, epoch: SnapshotEpoch, floats: usize) -> PreparedSong {
    let samples = BTreeMap::from([(
        "a.wav".into(),
        Arc::new(SampleData {
            rate: 32768,
            channels: 2,
            frames: vec![0.25; floats].into(),
        }),
    )]);
    let banks = BTreeMap::from([
        (intern_kw("fixed-a"), vec!["a.wav".into()]),
        (intern_kw("event-bank"), vec!["a.wav".into()]),
    ]);
    let factory = DecodedSongAssetFactory::new(samples, banks, Default::default());
    let context = vactr::session::song::CandidateBuildCtx {
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
    vactr::song::prepare_song(
        vactr::session::song::evaluate_song_candidate(code, "owner.vact", 17, epoch, &context)
            .unwrap(),
    )
    .unwrap()
}
fn limits(capabilities: CapabilitySet) -> SongPreparationLimits {
    SongPreparationLimits {
        capabilities,
        song: SongLimits::default(),
        max_resources: 128,
        max_pending_records: 4096,
        max_graph_bytes: 65536,
        max_work: 8_000_000,
    }
}
enum Backend {
    Native(Box<AudioSide>),
    Arena {
        state: Rc<RefCell<HostState>>,
        engine: Box<Engine>,
        inbox: Box<ByteInbox>,
        pending: std::collections::VecDeque<Vec<u8>>,
        events: ring::EventConsumer,
        acks: ring::AckProducer,
        received: ring::AckConsumer,
        cells: AtomicCells,
        garbage: ring::Producer<Garbage>,
        returned: ring::Consumer<Garbage>,
    },
}
struct Rig {
    host: Box<dyn AudioHost>,
    backend: Backend,
    caps: CapabilitySet,
    frame: u64,
    receipts: Vec<SongHostAck>,
    last: [f32; 32],
}
impl Rig {
    fn new(bytes: bool) -> Self {
        let mut caps = if bytes {
            CapabilitySet::browser()
        } else {
            CapabilitySet::native()
        };
        caps.max_voices = 2;
        let (host, backend): (Box<dyn AudioHost>, Backend) = if !bytes {
            let mut cfg = EngineConfig::new(
                &caps,
                32768.,
                vactr::host::native::audio::MAX_BLOCK,
                StoreKind::NativeArc,
            );
            cfg.bus_slots = 12;
            let (host, side) = NativeAudioHost::headless_with_config(cfg, 64).unwrap();
            (Box::new(host), Backend::Native(Box::new(side)))
        } else {
            browser_abi::outbox_clear();
            browser_abi::fix_outbox(0);
            let mut cfg =
                EngineConfig::new(&caps, 32768., 16, StoreKind::Arena { bytes: 1_000_000 });
            cfg.template_slots = 16;
            cfg.bus_slots = 12;
            cfg.bus_seconds = 20.;
            cfg.analysis_cells = 128;
            cfg.voice_seconds = 4.;
            let mut engine = Engine::with_config(cfg);
            let (acks, received) = SpscRing::split(128);
            engine
                .configure_song_staging_for_transport(
                    SongStagingConfig {
                        preparations: 4,
                        leases: 128,
                        branches: 16,
                        control_slots: 64,
                        analysis_slots: 128,
                        native_pcm_bytes: 1_000_000,
                        critical_receipts: 2,
                    },
                    &acks,
                )
                .unwrap();
            let (_, events) = EventRing::split(32);
            let (garbage, returned) = SpscRing::split(128);
            let state = Rc::new(RefCell::new(HostState::new(1_000_000)));
            (
                Box::new(WasmAudioHost(Rc::clone(&state))),
                Backend::Arena {
                    state,
                    engine: Box::new(engine),
                    inbox: Box::new(ByteInbox::new()),
                    pending: std::collections::VecDeque::new(),
                    events,
                    acks,
                    received,
                    cells: AtomicCells::new(64),
                    garbage,
                    returned,
                },
            )
        };
        Self {
            host,
            backend,
            caps,
            frame: 0,
            receipts: Vec::new(),
            last: [0.; 32],
        }
    }
    // Capture this host's actual stream before another Arena frontend writes
    // the process-global test ABI. This does not render or advance its clock.
    fn capture_outbox(&mut self) {
        let Backend::Arena { pending, .. } = &mut self.backend else {
            return;
        };
        let length = usize::try_from(browser_abi::outbox_len()).unwrap();
        let bytes =
            unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), length) }.to_vec();
        browser_abi::outbox_clear();
        let mut at = 0;
        while at < bytes.len() {
            let n =
                usize::try_from(u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())).unwrap();
            at += 4;
            pending.push_back(bytes[at..at + n].to_vec());
            at += n;
        }
    }
    fn tick(&mut self) -> Vec<HostMsg> {
        self.capture_outbox();
        let mut out = [0.; 32];
        match &mut self.backend {
            Backend::Native(side) => callback(|| side.render(&mut out, 2)),
            Backend::Arena {
                state,
                engine,
                inbox,
                pending,
                events,
                acks,
                received,
                cells,
                garbage,
                returned,
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
                        &mut out,
                        16,
                    )
                });
                while let Some(message) = received.pop() {
                    state.borrow_mut().on_msg(message);
                }
                while let Some(owner) = returned.pop() {
                    drop(owner);
                }
            }
        }
        self.last = out;
        self.frame += 16;
        let mut messages = Vec::new();
        // Native poll None can consume an internal report before later public ACKs.
        self.host.drain(&mut messages);
        for message in &messages {
            if let HostMsg::Song(ack) = message {
                self.receipts.push(*ack);
            }
        }
        messages
    }
    fn complete(&mut self, owner: &mut SongHostPreparation) -> SongReadyBundle {
        for _ in 0..2048 {
            owner.submit(self.host.as_mut()).unwrap();
            for message in self.tick() {
                if let HostMsg::Song(ack) = message {
                    if let Err(unhandled) = owner.receive(ack) {
                        assert!(matches!(unhandled, SongHostAck::SliceAccepted { .. }));
                    }
                }
            }
            if owner.progress() == SongPreparationProgress::Ready {
                return owner.take_ready().unwrap();
            }
        }
        panic!("actual owner did not reach Ready: {:?}", owner.progress());
    }
    fn cleanup(&mut self, owner: &mut SongHostPreparation, old: &mut SongTransport) {
        for _ in 0..2048 {
            let _ = owner.submit(self.host.as_mut());
            for message in self.tick() {
                if let HostMsg::Song(ack) = message {
                    if ack.epoch() == old.epoch() {
                        old.receive(ack).unwrap_or_else(|unhandled| {
                            panic!("old cleanup receipt not handled: {unhandled:?}")
                        });
                    } else if let Err(unhandled) = owner.receive(ack) {
                        assert!(
                            matches!(unhandled, SongHostAck::SliceAccepted { .. }),
                            "new cleanup receipt not handled: {unhandled:?}"
                        );
                    }
                }
            }
            if matches!(
                owner.progress(),
                SongPreparationProgress::Cancelled
                    | SongPreparationProgress::Retired
                    | SongPreparationProgress::Failed
            ) {
                return;
            }
        }
        panic!("actual cleanup stalled: {:?}", owner.progress());
    }
}

use vactr::host::caps::{SongReplacementProgress, SongReplacementSource};
use vactr::pattern::TimeSpan;
use vactr::sched::song::{SongTransport, SongTransportState};
use vactr::value::ratio::Ratio64;
fn start(rig: &mut Rig, epoch: u64) -> SongTransport {
    start_with_authority(rig, epoch).0
}
fn start_with_authority(rig: &mut Rig, epoch: u64) -> (SongTransport, Vec<SongLeaseKey>, Vec<u32>) {
    let (transport, keys, families, _) = start_with_event_authority(rig, epoch);
    (transport, keys, families)
}
fn start_with_event_authority(
    rig: &mut Rig,
    epoch: u64,
) -> (SongTransport, Vec<SongLeaseKey>, Vec<u32>, SongAudioEvent) {
    let mut owner =
        SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(epoch)), limits(rig.caps))
            .ok()
            .unwrap();
    let ready = rig.complete(&mut owner);
    let SongCommand::Event(event) = initial_event(&ready, 0) else {
        unreachable!()
    };
    let resources = ready.resources().to_vec();
    let families = ready
        .pools()
        .iter()
        .map(|p| p.initial.config.family)
        .collect();
    let mut core = SongTransport::new(ready, rig.frame + 256, SongLimits::default())
        .ok()
        .unwrap();
    core.submit_activation(rig.host.as_mut()).unwrap();
    core.advance(
        rig.host.as_mut(),
        SongHostClock {
            frame: rig.frame,
            sample_rate: 32768,
        },
        TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(),
    )
    .unwrap();
    let mut audible = false;
    for _ in 0..256 {
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                core.receive(ack).unwrap();
            }
        }
        audible |= rig.last.iter().any(|s| s.abs() > 1e-6);
        if audible && core.applied_activation().is_some() {
            break;
        }
    }
    assert_eq!(core.state(), SongTransportState::Playing);
    assert!(audible);
    (core, resources, families, event)
}
fn next_ready(rig: &mut Rig, epoch: u64) -> SongReadyBundle {
    let mut owner =
        SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(epoch)), limits(rig.caps))
            .ok()
            .unwrap();
    rig.complete(&mut owner)
}
fn overlay(ready: &SongReadyBundle, nonce: u64) -> Vec<SongInitialMute> {
    let mut records = Vec::new();
    for pool in ready.pools() {
        let instrument = pool.initial.config.family;
        if !records
            .iter()
            .any(|m: &SongInitialMute| m.instrument == instrument)
        {
            records.push(SongInitialMute {
                epoch: ready.prepared().epoch(),
                overlay_nonce: nonce,
                instrument,
                muted: true,
            });
        }
    }
    assert!(!records.is_empty());
    records
}
fn intent(
    ready: &SongReadyBundle,
    source: &SongReplacementSource,
    frame: u64,
    nonce: u64,
    count: usize,
) -> SongReplacement {
    SongReplacement {
        activation: SongActivation {
            epoch: ready.prepared().epoch(),
            frame,
        },
        previous: source.activation().epoch,
        overlay_nonce: nonce,
        overlay_count: u32::try_from(count).unwrap(),
    }
}

fn submit_all(rig: &mut Rig, ready: &mut SongReadyBundle, replacement: SongReplacement) {
    let original_frame = rig.frame;
    for ordinal in 0..=replacement.overlay_count {
        let expected = if ordinal == replacement.overlay_count {
            SongReplacementProgress::Submitted
        } else {
            SongReplacementProgress::Priming
        };
        assert_eq!(
            ready.submit_replacement(rig.host.as_mut()).unwrap(),
            expected
        );
    }
    assert_eq!(rig.frame, original_frame);
    rig.capture_outbox();
}
fn initial_event(ready: &SongReadyBundle, frame: u64) -> SongCommand {
    let b = ready.pools()[0].initial.config;
    SongCommand::Event(SongAudioEvent {
        epoch: b.epoch,
        branch: b.branch,
        generation: b.generation,
        frame,
        event: vactr::host::wire::AudioEvent::new(
            0.,
            vactr::sched::slots::SlotId::new(0),
            1,
            vactr::dsp::graph::InstId::new(b.instrument.id),
        ),
    })
}
fn deliver(old: &mut SongTransport, ready: &mut SongReadyBundle, messages: Vec<HostMsg>) {
    for message in messages {
        if let HostMsg::Song(ack) = message {
            if ack.epoch() == old.epoch() {
                let _ = old.receive(ack);
            } else if ack.epoch() == ready.prepared().epoch() {
                assert!(
                    ready.receive_activation(ack).is_ok(),
                    "unexpected new-owner outcome {ack:?}"
                );
            }
        }
    }
}
#[test]
fn real_replacement_commits_at_exact_frame_and_primed_first_onset_is_silent() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut twin = Rig::new(bytes);
        let mut old = start(&mut rig, 800);
        let mut reference = start(&mut twin, 800);
        let mut ready = next_ready(&mut rig, 801);
        let _reference_ready = next_ready(&mut twin, 801);
        assert_eq!(rig.frame, twin.frame);
        let source = old.replacement_source().unwrap();
        let a = rig.frame + 128;
        let records = overlay(&ready, 17);
        let replacement = intent(&ready, &source, a, 17, records.len());
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
                    epoch: SnapshotEpoch(801),
                    arrangement: a + 128,
                    tail_deadline: a + 128,
                }),
            )
            .unwrap();
        let mut fade_nonzero = 0;
        while rig.frame < a + 80 {
            let frame = rig.frame;
            let messages = rig.tick();
            for message in twin.tick() {
                if let HostMsg::Song(ack) = message {
                    let _ = reference.receive(ack);
                }
            }
            deliver(&mut old, &mut ready, messages);
            for sample in 0..16 {
                let absolute = frame + sample as u64;
                let gain = if absolute < a - 64 {
                    1.
                } else if absolute >= a {
                    0.
                } else {
                    (a - absolute) as f32 / 64.
                };
                for channel in 0..2 {
                    let i = sample * 2 + channel;
                    assert!(
                        (rig.last[i] - twin.last[i] * gain).abs() < 1e-6,
                        "backend {bytes}, frame {absolute}: actual {} reference {} gain {gain}",
                        rig.last[i],
                        twin.last[i]
                    );
                    if absolute >= a - 64 && absolute < a && rig.last[i].abs() > 1e-6 {
                        fade_nonzero += 1;
                    }
                }
            }
            if frame < a {
                assert!(ready.applied_activation().is_none());
            }
        }
        assert!(fade_nonzero > 50);
        assert_eq!(ready.applied_activation(), Some(replacement.activation));
        assert_eq!(
            rig.receipts
                .iter()
                .filter(|ack| **ack == SongHostAck::Applied(replacement.activation))
                .count(),
            1
        );
        old.observe_replacement(ready.take_replacement_commit().unwrap())
            .ok()
            .unwrap();
        assert!(ready.take_replacement_commit().is_none());
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup, &mut old);
        assert_eq!(cleanup.progress(), SongPreparationProgress::Retired);
    }
}
#[test]
fn genuine_prearm_cancellation_preserves_old_pcm_and_original_rejection_frame() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut twin = Rig::new(bytes);
        let mut old = start(&mut rig, 810);
        let mut reference = start(&mut twin, 810);
        let mut ready = next_ready(&mut rig, 811);
        let _reference_ready = next_ready(&mut twin, 811);
        let source = old.replacement_source().unwrap();
        let a = rig.frame + 192;
        let replacement = intent(&ready, &source, a, 19, 0);
        ready
            .prepare_replacement(replacement, source, Vec::new())
            .ok()
            .unwrap();
        submit_all(&mut rig, &mut ready, replacement);
        let event = initial_event(&ready, a);
        ready
            .submit_initial_command(rig.host.as_mut(), event)
            .unwrap();
        rig.host
            .try_song_command(SongCommand::CancelPreparation(SnapshotEpoch(811)))
            .unwrap();
        while rig.frame < a + 80 {
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    if ack.epoch() == SnapshotEpoch(811) {
                        let _ = ready.receive_activation(ack);
                    } else {
                        let _ = old.receive(ack);
                    }
                }
            }
            for message in twin.tick() {
                if let HostMsg::Song(ack) = message {
                    let _ = reference.receive(ack);
                }
            }
            assert_eq!(
                rig.last, twin.last,
                "prearm cancellation changed old PCM, backend {bytes}"
            );
        }
        assert_eq!(ready.applied_activation(), None);
        assert_eq!(ready.activation_rejection(), Some(SongRejectCode::NotReady));
        assert_eq!(
            rig.receipts
                .iter()
                .filter(|ack| **ack
                    == SongHostAck::ActivationRejected {
                        activation: replacement.activation,
                        reason: SongRejectCode::NotReady,
                    })
                .count(),
            1
        );
        assert!(!rig
            .receipts
            .iter()
            .any(|ack| *ack == SongHostAck::Applied(replacement.activation)));
    }
}

#[test]
fn genuine_afterarm_invalidation_keeps_boundary_commit_and_finite_cleanup() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 820);
        let mut ready = next_ready(&mut rig, 821);
        let source = old.replacement_source().unwrap();
        let a = rig.frame + 128;
        let replacement = intent(&ready, &source, a, 23, 0);
        ready
            .prepare_replacement(replacement, source, Vec::new())
            .ok()
            .unwrap();
        submit_all(&mut rig, &mut ready, replacement);
        let event = initial_event(&ready, a);
        ready
            .submit_initial_command(rig.host.as_mut(), event)
            .unwrap();
        while rig.frame <= a - 64 {
            let messages = rig.tick();
            deliver(&mut old, &mut ready, messages);
        }
        rig.host
            .try_song_command(SongCommand::CancelPreparation(SnapshotEpoch(821)))
            .unwrap();
        while rig.frame < a + 32 {
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    if ack.epoch() == SnapshotEpoch(821) {
                        let _ = ready.receive_activation(ack);
                    } else {
                        let _ = old.receive(ack);
                    }
                }
            }
        }
        assert_eq!(ready.applied_activation(), Some(replacement.activation));
        assert!(rig.receipts.contains(&SongHostAck::Rejected {
            epoch: SnapshotEpoch(821),
            reason: SongRejectCode::HostFault
        }));
        assert!(!rig.receipts.iter().any(|ack| matches!(ack, SongHostAck::ActivationRejected { activation, .. } if activation.epoch == SnapshotEpoch(821))));
        assert_eq!(rig.last, [0.; 32]);
        old.observe_replacement(ready.take_replacement_commit().unwrap())
            .ok()
            .unwrap();
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup, &mut old);
        assert_eq!(cleanup.progress(), SongPreparationProgress::Retired);
    }
}

fn finish_old(rig: &mut Rig, core: &mut SongTransport) {
    for _ in 0..8192 {
        core.advance(
            rig.host.as_mut(),
            SongHostClock {
                frame: rig.frame,
                sample_rate: 32768,
            },
            TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(),
        )
        .unwrap();
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                core.receive(ack).unwrap();
            }
        }
        if core.state() == SongTransportState::Ended {
            return;
        }
    }
    panic!(
        "actual old owner did not naturally retire: {:?}",
        core.state()
    );
}
#[test]
fn observed_natural_retirement_allows_exact_replacement_and_64_frame_new_opening() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut twin = Rig::new(bytes);
        let mut old = start(&mut rig, 830);
        let mut reference = start(&mut twin, 830);
        finish_old(&mut rig, &mut old);
        finish_old(&mut twin, &mut reference);
        let mut ready = next_ready(&mut rig, 831);
        let mut baseline = next_ready(&mut twin, 831);
        assert_eq!(rig.frame, twin.frame);
        let source = old.replacement_source().unwrap();
        let a = rig.frame + 128;
        assert!(a > source.endpoints().tail_deadline);
        let replacement = intent(&ready, &source, a, 29, 0);
        ready
            .prepare_replacement(replacement, source, Vec::new())
            .ok()
            .unwrap();
        submit_all(&mut rig, &mut ready, replacement);
        baseline
            .submit_activation(twin.host.as_mut(), replacement.activation)
            .unwrap();
        twin.capture_outbox();
        let event = initial_event(&ready, a);
        let reference_event = initial_event(&baseline, a);
        ready
            .submit_initial_command(rig.host.as_mut(), event)
            .unwrap();
        rig.capture_outbox();
        baseline
            .submit_initial_command(twin.host.as_mut(), reference_event)
            .unwrap();
        twin.capture_outbox();
        let endpoint = SongCommand::Endpoints(SongEndpoints {
            epoch: SnapshotEpoch(831),
            arrangement: a + 256,
            tail_deadline: a + 256,
        });
        ready
            .submit_initial_command(rig.host.as_mut(), endpoint)
            .unwrap();
        rig.capture_outbox();
        baseline
            .submit_initial_command(twin.host.as_mut(), endpoint)
            .unwrap();
        twin.capture_outbox();
        let mut nonzero = 0;
        while rig.frame < a + 160 {
            let frame = rig.frame;
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    if ack.epoch() == SnapshotEpoch(831) {
                        ready.receive_activation(ack).unwrap();
                    }
                }
            }
            for message in twin.tick() {
                if let HostMsg::Song(ack) = message {
                    if ack.epoch() == SnapshotEpoch(831) {
                        baseline.receive_activation(ack).unwrap();
                    }
                }
            }
            for sample in 0..16 {
                let absolute = frame + sample as u64;
                let gain = if absolute < a {
                    0.
                } else if absolute >= a + 64 {
                    1.
                } else {
                    (absolute - a) as f32 / 64.
                };
                for channel in 0..2 {
                    let i = sample * 2 + channel;
                    assert!(
                        (rig.last[i] - twin.last[i] * gain).abs() < 1e-6,
                        "new opening backend {bytes} at {absolute}"
                    );
                    if rig.last[i].abs() > 1e-6 {
                        nonzero += 1;
                    }
                }
            }
        }
        assert!(nonzero > 50);
        assert_eq!(ready.applied_activation(), Some(replacement.activation));
        old.observe_replacement(ready.take_replacement_commit().unwrap())
            .ok()
            .unwrap();
    }
}

#[path = "song_apply_runtime/pressure.rs"]
mod pressure;

#[test]
fn inactive_ordinary_cancel_restores_exact_coupled_capacity_and_real_replacement() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut twin = Rig::new(bytes);
        let mut old = start(&mut rig, 990);
        let mut reference = start(&mut twin, 990);
        let activation =
            pressure::cancel_ordinary_ready_twins(&mut rig, &mut old, &mut twin, &mut reference);
        let slots = if bytes { 4 } else { 64 }; // Actual construction in Rig/native bootstrap.
        let begin = |epoch| {
            SongCommand::BeginPreparation(SongPreparation {
                epoch,
                branches: 0,
                resources: 0,
                required: SongHostCapacities {
                    sample_rate: 32768,
                    ..Default::default()
                },
            })
        };
        let far = rig.frame + 8192;
        let epochs: Vec<_> = (0..slots - 1).map(|i| SnapshotEpoch(1100 + i)).collect();
        for epoch in &epochs {
            for r in [&mut rig, &mut twin] {
                for command in [
                    begin(*epoch),
                    SongCommand::SealPreparation(*epoch),
                    SongCommand::Activate(SongActivation {
                        epoch: *epoch,
                        frame: far,
                    }),
                ] {
                    r.host.try_song_command(command).unwrap();
                }
                r.capture_outbox();
            }
            assert_eq!(
                pressure::render_block(&mut rig, 1, true),
                pressure::render_block(&mut twin, 1, true)
            );
        }
        for epoch in &epochs {
            assert!(rig.receipts.contains(&SongHostAck::Ready(*epoch)));
        }
        let extra = SnapshotEpoch(1200);
        let original = begin(extra);
        for r in [&mut rig, &mut twin] {
            r.host.try_song_command(original).unwrap();
            r.capture_outbox();
        }
        assert_eq!(
            pressure::render_block(&mut rig, 1, true),
            pressure::render_block(&mut twin, 1, true)
        );
        assert_eq!(original, begin(extra));
        assert!(rig.receipts.contains(&SongHostAck::Rejected {
            epoch: extra,
            reason: SongRejectCode::Capacity
        }));
        for epoch in &epochs {
            for r in [&mut rig, &mut twin] {
                r.host
                    .try_song_command(SongCommand::CancelPreparation(*epoch))
                    .unwrap();
                r.capture_outbox();
            }
        }
        for _ in 0..epochs.len() * 2 + 4 {
            assert_eq!(
                pressure::render_block(&mut rig, 1, true),
                pressure::render_block(&mut twin, 1, true)
            );
        }
        for epoch in &epochs {
            assert!(rig
                .receipts
                .contains(&SongHostAck::PreparationCancelled(*epoch)));
            assert!(rig.receipts.contains(&SongHostAck::ActivationRejected {
                activation: SongActivation {
                    epoch: *epoch,
                    frame: far
                },
                reason: SongRejectCode::NotReady,
            }));
        }
        let mut next = next_ready(&mut rig, 992);
        let mut next_twin = next_ready(&mut twin, 992);
        let a = rig.frame + 128;
        for (r, o, n) in [
            (&mut rig, &mut old, &mut next),
            (&mut twin, &mut reference, &mut next_twin),
        ] {
            let source = o.replacement_source().unwrap();
            let replacement = intent(n, &source, a, 141, 0);
            n.prepare_replacement(replacement, source, Vec::new())
                .ok()
                .unwrap();
            submit_all(r, n, replacement);
            for frame in [a, far + 8] {
                n.submit_initial_command(r.host.as_mut(), initial_event(n, frame))
                    .unwrap();
            }
            r.capture_outbox();
        }
        while rig.frame < a + 80 {
            assert_eq!(
                pressure::render_block(&mut rig, 16, false),
                pressure::render_block(&mut twin, 16, false)
            );
            for ack in pressure::drain(&mut rig) {
                if ack.epoch() == old.epoch() {
                    old.receive(ack).unwrap();
                } else if matches!(ack, SongHostAck::Applied(_)) {
                    next.receive_activation(ack).unwrap();
                } else {
                    assert!(matches!(ack, SongHostAck::LeaseReturned(_)));
                }
            }
            for ack in pressure::drain(&mut twin) {
                if ack.epoch() == reference.epoch() {
                    reference.receive(ack).unwrap();
                } else if matches!(ack, SongHostAck::Applied(_)) {
                    next_twin.receive_activation(ack).unwrap();
                } else {
                    assert!(matches!(ack, SongHostAck::LeaseReturned(_)));
                }
            }
        }
        assert_eq!(
            next.applied_activation(),
            Some(SongActivation {
                epoch: SnapshotEpoch(992),
                frame: a
            })
        );
        assert!(rig.frame < activation.frame && rig.frame < far);
        old.observe_replacement(next.take_replacement_commit().unwrap())
            .ok()
            .unwrap();
        // Cross every cancelled original activation frame with genuine new
        // audio present; no obsolete activation or endpoint may resurrect.
        let mut audible = false;
        while rig.frame < far + 32 {
            let n = usize::try_from((far + 32 - rig.frame).min(16)).unwrap();
            let actual = pressure::render_block(&mut rig, n, true);
            assert_eq!(actual, pressure::render_block(&mut twin, n, true));
            audible |= actual.iter().any(|value| value.abs() > 1e-6);
        }
        assert!(audible);
        assert!(!rig
            .receipts
            .iter()
            .any(|ack| matches!(ack, SongHostAck::Applied(a)
            if a.epoch == activation.epoch || epochs.contains(&a.epoch))));
    }
}
