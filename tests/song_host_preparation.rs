//! Consuming original candidate ownership through real Native/Arena staging.
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
    snapshot::SongPreparationState,
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
const CLOSED: &str = "inst closed bank: keyword = :event-bank freq: float = 440:\n\tsample-play bank n: 0 rate: 1 > convolution ir: :fixed-a\nbus :drums:\n\tconvolution ir: :fixed-a > level\nmaster:\n\tlevel\nsong {part [drums: {s closed}] duration: 1} tail-seconds: 0 > play-song";
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
    fn tick(&mut self) -> Vec<HostMsg> {
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
                let length = usize::try_from(browser_abi::outbox_len()).unwrap();
                let bytes =
                    unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), length) }
                        .to_vec();
                browser_abi::outbox_clear();
                let mut at = 0;
                while at < bytes.len() {
                    let n =
                        usize::try_from(u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()))
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
    fn cleanup(&mut self, owner: &mut SongHostPreparation) {
        for _ in 0..2048 {
            let _ = owner.submit(self.host.as_mut());
            for message in self.tick() {
                if let HostMsg::Song(ack) = message {
                    if let Err(unhandled) = owner.receive(ack) {
                        assert!(matches!(unhandled, SongHostAck::SliceAccepted { .. }));
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
#[test]
fn original_candidate_becomes_actual_native_and_arena_ready_once() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let prepared = candidate(PLAIN, SnapshotEpoch(400));
        let pointer = Arc::as_ptr(&prepared.snapshot().routing().instruments[0].graph);
        let mut owner = SongHostPreparation::begin(prepared, limits(rig.caps))
            .ok()
            .unwrap();
        assert!(owner.take_ready().is_err());
        let mut ready = rig.complete(&mut owner);
        assert_eq!(ready.prepared().state(), SongPreparationState::Ready);
        assert!(rig.last.iter().all(|sample| *sample == 0.));
        assert_eq!(ready.prepared().revision(), 17);
        assert_eq!(
            Arc::as_ptr(&ready.prepared().snapshot().routing().instruments[0].graph),
            pointer
        );
        let physical = ready
            .routes()
            .branches
            .iter()
            .map(|branch| branch.reserved_generations as usize)
            .sum::<usize>();
        assert!(physical > 0);
        assert_eq!(ready.pools().len(), physical);
        assert_eq!(
            ready.stages().len(),
            physical + ready.routes().tracks.len() + 1
        );
        for branch in &ready.routes().branches {
            assert_eq!(
                ready
                    .pools()
                    .iter()
                    .filter(|pool| pool.logical == branch.id)
                    .count(),
                branch.reserved_generations as usize
            );
        }
        assert!(!ready
            .query(
                vactr::pattern::TimeSpan::cycle(0).unwrap(),
                &SongLimits::default()
            )
            .unwrap()
            .is_empty());
        assert!(owner.take_ready().is_err());
        let mut retirement = SongHostPreparation::retire(ready);
        rig.cleanup(&mut retirement);
        assert_eq!(retirement.take_cancelled().unwrap().revision(), 17);
        assert!(retirement.take_cancelled().is_err());
    }
}
#[test]
fn closed_pcm_union_and_physical_graph_banks_are_actually_adopted() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut owner =
            SongHostPreparation::begin(candidate(CLOSED, SnapshotEpoch(401)), limits(rig.caps))
                .ok()
                .unwrap();
        let ready = rig.complete(&mut owner);
        assert_eq!(
            ready
                .resources()
                .iter()
                .filter(|r| r.kind == SongResourceKind::Sample)
                .count(),
            1
        );
        assert!(ready
            .resources()
            .iter()
            .any(|r| r.kind == SongResourceKind::ControlCells));
        assert_eq!(
            ready
                .resources()
                .iter()
                .filter(|r| r.kind == SongResourceKind::AnalysisBank)
                .count(),
            2
        );
        let uploaded = ready
            .resources()
            .iter()
            .filter(|r| {
                matches!(
                    r.kind,
                    SongResourceKind::Instrument
                        | SongResourceKind::PrivateFx
                        | SongResourceKind::Track
                        | SongResourceKind::Master
                        | SongResourceKind::Sample
                )
            })
            .count();
        assert_eq!(
            rig.receipts
                .iter()
                .filter(|a| matches!(a, SongHostAck::ResourceReady { .. }))
                .count(),
            uploaded
        );
        let keys = ready.resources().to_vec();
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup);
        for key in keys {
            assert_eq!(
                rig.receipts
                    .iter()
                    .filter(|a| **a == SongHostAck::LeaseReturned(key))
                    .count(),
                1
            );
        }
        assert!(cleanup.take_cancelled().is_ok());
    }
}
#[test]
fn exclusive_activation_and_normal_retirement_preserve_original_authority() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut owner =
            SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(402)), limits(rig.caps))
                .ok()
                .unwrap();
        let mut ready = rig.complete(&mut owner);
        let keys = ready.resources().to_vec();
        let requested = rig.frame;
        ready
            .submit_activation(
                rig.host.as_mut(),
                SongActivation {
                    epoch: SnapshotEpoch(402),
                    frame: requested,
                },
            )
            .unwrap();
        assert!(ready
            .submit_activation(
                rig.host.as_mut(),
                SongActivation {
                    epoch: SnapshotEpoch(402),
                    frame: requested
                }
            )
            .is_err());
        let mut applied = None;
        for _ in 0..16 {
            for msg in rig.tick() {
                if let HostMsg::Song(ack) = msg {
                    if let SongHostAck::Applied(actual) = ack {
                        applied = Some(actual);
                        ready.receive_activation(ack).unwrap();
                    }
                }
            }
            if applied.is_some() {
                break;
            }
        }
        assert!(applied.unwrap().frame >= requested);
        assert_eq!(ready.prepared().state(), SongPreparationState::Applied);
        let end = rig.frame + 16;
        rig.host
            .try_song_command(SongCommand::Endpoints(SongEndpoints {
                epoch: SnapshotEpoch(402),
                arrangement: end,
                tail_deadline: end,
            }))
            .unwrap();
        let mut retirement = SongHostPreparation::retire(ready);
        assert!(retirement.cancel().is_err());
        rig.cleanup(&mut retirement);
        assert_eq!(retirement.progress(), SongPreparationProgress::Retired);
        assert_eq!(retirement.take_retired().unwrap().revision(), 17);
        assert!(retirement.take_retired().is_err());
        for key in keys {
            assert_eq!(
                rig.receipts
                    .iter()
                    .filter(|a| **a == SongHostAck::LeaseReturned(key))
                    .count(),
                1
            );
        }
        assert!(!rig
            .receipts
            .iter()
            .any(|a| matches!(a, SongHostAck::PreparationCancelled(SnapshotEpoch(402)))));
    }
}
#[test]
fn never_activated_ready_cancels_only_after_exact_actual_returns() {
    let mut rig = Rig::new(false);
    let mut owner =
        SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(403)), limits(rig.caps))
            .ok()
            .unwrap();
    let ready = rig.complete(&mut owner);
    let keys = ready.resources().to_vec();
    let mut cleanup = SongHostPreparation::retire(ready);
    assert!(cleanup.take_cancelled().is_err());
    rig.cleanup(&mut cleanup);
    for key in keys {
        assert_eq!(
            rig.receipts
                .iter()
                .filter(|a| **a == SongHostAck::LeaseReturned(key))
                .count(),
            1
        );
    }
    assert_eq!(
        rig.receipts
            .iter()
            .filter(|a| **a == SongHostAck::PreparationCancelled(SnapshotEpoch(403)))
            .count(),
        1
    );
    assert!(cleanup.take_cancelled().is_ok());
}
#[test]
fn foreign_receipts_and_wrong_clock_nonce_are_preserved() {
    let mut rig = Rig::new(false);
    let mut owner =
        SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(404)), limits(rig.caps))
            .ok()
            .unwrap();
    owner.submit(rig.host.as_mut()).unwrap();
    owner.submit(rig.host.as_mut()).unwrap();
    let wrong = SongHostAck::ClockRejected(SongClockFailure {
        request: SongClockRequest {
            epoch: SnapshotEpoch(404),
            request: 99,
        },
        reason: SongRejectCode::NotReady,
    });
    assert_eq!(owner.receive(wrong), Err(wrong));
    let foreign = SongHostAck::Ready(SnapshotEpoch(999));
    assert_eq!(owner.receive(foreign), Err(foreign));
    let ready = rig.complete(&mut owner);
    let mut cleanup = SongHostPreparation::retire(ready);
    rig.cleanup(&mut cleanup);
}
#[test]
fn local_work_and_resource_failure_preserve_original_candidate() {
    for work in [1, 8_000_000] {
        let mut rig = Rig::new(false);
        let prepared = candidate(PLAIN, SnapshotEpoch(405));
        let pointer = Arc::as_ptr(&prepared.snapshot().routing().instruments[0].graph);
        let mut bounds = limits(rig.caps);
        bounds.max_work = work;
        if work != 1 {
            bounds.max_resources = 1;
        }
        let mut owner = SongHostPreparation::begin(prepared, bounds).ok().unwrap();
        let mut failed = false;
        for _ in 0..16 {
            failed |= owner.submit(rig.host.as_mut()).is_err();
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    if let Err(unhandled) = owner.receive(ack) {
                        assert!(matches!(unhandled, SongHostAck::SliceAccepted { .. }));
                    }
                }
            }
            if failed {
                break;
            }
        }
        assert!(failed);
        let returned = owner.take_cancelled().unwrap();
        assert_eq!(
            Arc::as_ptr(&returned.snapshot().routing().instruments[0].graph),
            pointer
        );
        assert!(!rig.receipts.iter().any(|ack| matches!(
            ack,
            SongHostAck::Ready(_) | SongHostAck::ResourceReady { .. }
        )));
    }
}
#[path = "song_host_preparation/pressure.rs"]
mod pressure;

#[test]
fn pending_initial_commands_preserve_onset_and_exact_activation_correlation() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let epoch = SnapshotEpoch(410);
        let mut owner = SongHostPreparation::begin(candidate(PLAIN, epoch), limits(rig.caps))
            .ok()
            .unwrap();
        let mut ready = rig.complete(&mut owner);
        let activation = SongActivation {
            epoch,
            frame: rig.frame,
        };
        ready
            .submit_activation(rig.host.as_mut(), activation)
            .unwrap();
        let generic = SongHostAck::Rejected {
            epoch,
            reason: SongRejectCode::Capacity,
        };
        assert_eq!(ready.receive_activation(generic), Err(generic));
        let foreign = SongHostAck::ActivationRejected {
            activation: SongActivation {
                frame: activation.frame + 1,
                ..activation
            },
            reason: SongRejectCode::NotReady,
        };
        assert_eq!(ready.receive_activation(foreign), Err(foreign));
        assert_eq!(ready.activation_rejection(), None);
        let b = ready.pools()[0].initial.config;
        let event = vactr::host::wire::AudioEvent::new(
            0.,
            vactr::sched::slots::SlotId::new(0),
            1,
            vactr::dsp::graph::InstId::new(b.instrument.id),
        );
        ready
            .submit_initial_command(
                rig.host.as_mut(),
                SongCommand::Event(SongAudioEvent {
                    epoch,
                    branch: b.branch,
                    generation: b.generation,
                    frame: activation.frame,
                    event,
                }),
            )
            .unwrap();
        ready
            .submit_initial_command(
                rig.host.as_mut(),
                SongCommand::Release(SongBranchRelease {
                    epoch,
                    branch: b.branch,
                    generation: b.generation,
                    frame: activation.frame + 8,
                    tail_deadline: activation.frame + 16,
                }),
            )
            .unwrap();
        ready
            .submit_initial_command(
                rig.host.as_mut(),
                SongCommand::Endpoints(SongEndpoints {
                    epoch,
                    arrangement: activation.frame + 16,
                    tail_deadline: activation.frame + 16,
                }),
            )
            .unwrap();
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                ready.receive_activation(ack).unwrap();
            }
        }
        assert_eq!(
            ready.applied_activation(),
            Some(activation),
            "backend bytes={bytes}; receipts={:?}; rejection={:?}",
            rig.receipts,
            ready.activation_rejection(),
        );
        assert!(
            rig.last.iter().any(|v| v.abs() > 0.001),
            "initial callback contains real audio"
        );
        let mut retirement = SongHostPreparation::retire(ready);
        rig.cleanup(&mut retirement);
        assert_eq!(retirement.progress(), SongPreparationProgress::Retired);
    }
}
#[test]
fn empty_repeat_zero_and_subframe_owners_retire_at_exact_initial_endpoint() {
    for bytes in [false, true] {
        for body in [
            "sequence []",
            "part-repeat {part [drums: {s :analog}] duration: 1} 0",
            "part [drums: {s :analog}] duration: 1/1000000",
        ] {
            for tail in [0, 64] {
                let mut rig = Rig::new(bytes);
                let epoch = SnapshotEpoch(411);
                let code = format!("song {{{body}}} tail-seconds: 0 > play-song");
                let mut owner =
                    SongHostPreparation::begin(candidate(&code, epoch), limits(rig.caps))
                        .ok()
                        .unwrap();
                let mut ready = rig.complete(&mut owner);
                let keys = ready.resources().to_vec();
                assert!(keys.iter().any(|k| k.kind == SongResourceKind::Master));
                let activation = SongActivation {
                    epoch,
                    frame: rig.frame,
                };
                ready
                    .submit_activation(rig.host.as_mut(), activation)
                    .unwrap();
                ready
                    .submit_initial_command(
                        rig.host.as_mut(),
                        SongCommand::Endpoints(SongEndpoints {
                            epoch,
                            arrangement: activation.frame,
                            tail_deadline: activation.frame + tail,
                        }),
                    )
                    .unwrap();
                let mut retirement = SongHostPreparation::retire(ready);
                assert!(retirement.cancel().is_err());
                rig.cleanup(&mut retirement);
                assert_eq!(retirement.progress(), SongPreparationProgress::Retired);
                assert!(rig.last.iter().all(|v| *v == 0.));
                assert_eq!(
                    rig.receipts
                        .iter()
                        .filter(|a| **a == SongHostAck::Applied(activation))
                        .count(),
                    1
                );
                for key in keys {
                    assert_eq!(
                        rig.receipts
                            .iter()
                            .filter(|a| **a == SongHostAck::LeaseReturned(key))
                            .count(),
                        1
                    );
                }
                assert!(!rig.receipts.iter().any(|a| matches!(
                    a,
                    SongHostAck::Rejected { .. } | SongHostAck::ActivationRejected { .. }
                )));
            }
        }
    }
}
