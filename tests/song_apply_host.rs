//! Real prepared owners and queue refusals; no fabricated replacement Applied.
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

use vactr::host::caps::{SongReplacementProgress, SongReplacementSource, SongSubmitError};
use vactr::pattern::TimeSpan;
use vactr::sched::song::{SongTransport, SongTransportState};
use vactr::value::ratio::Ratio64;
fn start(rig: &mut Rig, epoch: u64) -> SongTransport {
    let mut owner =
        SongHostPreparation::begin(candidate(PLAIN, SnapshotEpoch(epoch)), limits(rig.caps))
            .ok()
            .unwrap();
    let ready = rig.complete(&mut owner);
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
    core
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
#[test]
fn actual_initial_activation_issues_one_immutable_old_owner_ticket() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 700);
        let source = old.replacement_source().unwrap();
        assert_eq!(source.activation(), old.applied_activation().unwrap());
        assert_eq!(source.endpoints(), old.endpoints());
        assert!(old.replacement_source().is_err());
        assert!(old.take_replacement_commit().is_none());
        assert!(rig.last.iter().any(|s| s.abs() > 1e-6));
    }
}
#[test]
fn real_ready_refuses_foreign_duplicate_and_missing_overlay_without_losing_originals() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 710);
        let mut source = old.replacement_source().unwrap();
        let mut ready = next_ready(&mut rig, 711);
        let keys = ready.resources().to_vec();
        let graph = Arc::as_ptr(&ready.prepared().snapshot().routing().instruments[0].graph);
        let original = overlay(&ready, 9);
        for case in 0..4 {
            let mut records = original.clone();
            match case {
                0 => records[0].epoch = SnapshotEpoch(999),
                1 => records.push(records[0]),
                2 => records[0].instrument = u32::MAX,
                _ => records[0].overlay_nonce = 10,
            }
            let pointer = records.as_ptr();
            let command = intent(&ready, &source, rig.frame + 512, 9, records.len());
            let refusal = ready
                .prepare_replacement(command, source, records)
                .err()
                .unwrap();
            assert_eq!(refusal.replacement, command);
            assert_eq!(refusal.overlay.as_ptr(), pointer);
            assert_eq!(refusal.source.endpoints(), old.endpoints());
            source = refusal.source;
            assert_eq!(ready.resources(), keys);
            assert_eq!(
                Arc::as_ptr(&ready.prepared().snapshot().routing().instruments[0].graph),
                graph
            );
            assert!(ready.applied_activation().is_none());
            assert!(ready.take_replacement_commit().is_none());
        }
        let records = original;
        let mut command = intent(&ready, &source, rig.frame + 512, 9, records.len());
        command.overlay_count += 1;
        let refusal = ready
            .prepare_replacement(command, source, records)
            .err()
            .unwrap();
        assert_eq!(refusal.replacement, command);
        assert_eq!(
            refusal.overlay.len() + 1,
            usize::try_from(command.overlay_count).unwrap()
        );
        assert_eq!(old.state(), SongTransportState::Playing);
        let mut cleanup = SongHostPreparation::retire(ready);
        rig.cleanup(&mut cleanup);
        assert!(cleanup.take_cancelled().is_ok());
        assert!(cleanup.take_replacement_commit().is_none());
    }
}
#[test]
fn real_queue_pressure_retains_exact_first_prime_and_prime_only_invalidation_cancels_new_owner() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut old = start(&mut rig, 720);
        let source = old.replacement_source().unwrap();
        let ready = next_ready(&mut rig, 721);
        let records = overlay(&ready, 11);
        let first = records[0];
        let mut new = SongTransport::new(ready, rig.frame + 8192, SongLimits::default())
            .ok()
            .unwrap();
        new.prepare_replacement(source, 11, records)
            .unwrap_or_else(|r| panic!("{}", r.failure));
        if bytes {
            browser_abi::fix_outbox(4096);
        }
        let mut full = false;
        for _ in 0..32768 {
            if rig
                .host
                .try_song_command(SongCommand::RequestCapacity(SnapshotEpoch(999)))
                .is_err()
            {
                full = true;
                break;
            }
        }
        assert!(full, "actual sender queue must have a finite bound");
        for _ in 0..2 {
            let refusal = new.submit_replacement(rig.host.as_mut()).err().unwrap();
            assert_eq!(refusal.command, SongCommand::PrimeMute(first));
            assert!(matches!(refusal.error, SongSubmitError::Backpressure));
            assert!(new.take_replacement_commit().is_none());
        }
        // Drain genuine capacity reports, not fabricated completion receipts.
        let mut accepted = false;
        for _ in 0..2048 {
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    if ack.epoch() == old.epoch() {
                        old.receive(ack).unwrap();
                    } else {
                        assert_eq!(ack.epoch(), SnapshotEpoch(999));
                    }
                }
            }
            match new.submit_replacement(rig.host.as_mut()) {
                Ok(progress) => {
                    assert_eq!(progress, SongReplacementProgress::Priming);
                    accepted = true;
                    break;
                }
                Err(refusal) => assert!(matches!(refusal.error, SongSubmitError::Backpressure)),
            }
        }
        assert!(accepted);
        assert!(new.applied_activation().is_none());
        assert!(new.take_replacement_commit().is_none());
        // No Replace was submitted: valid priming has no success receipt.
        // Invalidation retains original new cleanup and does not issue old cutoff.
        new.invalidate_replacement().unwrap();
        assert_eq!(new.state(), SongTransportState::Failed);
        let original_endpoint = old.endpoints();
        let mut rejected = false;
        let mut cancelled = false;
        for _ in 0..2048 {
            new.advance(
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
                    if ack.epoch() == new.epoch() {
                        rejected |= matches!(
                            ack,
                            SongHostAck::Rejected { .. } | SongHostAck::ActivationRejected { .. }
                        );
                        assert!(
                            !matches!(ack, SongHostAck::Applied(_)),
                            "prime-only invalidation cannot apply: {ack:?}"
                        );
                        new.receive(ack).unwrap();
                    } else if ack.epoch() == old.epoch() {
                        old.receive(ack).unwrap();
                    } else {
                        assert_eq!(ack.epoch(), SnapshotEpoch(999));
                    }
                }
            }
            if new.take_cancelled().is_ok() {
                cancelled = true;
                break;
            }
        }
        assert!(cancelled, "actual prime-only owner cleanup must finish");
        assert!(!rejected, "valid prime-only staging must not be rejected");
        assert!(new.take_replacement_commit().is_none());
        assert_eq!(old.endpoints(), original_endpoint);
        assert_eq!(old.state(), SongTransportState::Playing);
    }
}

#[path = "song_apply_host/recovery.rs"]
mod recovery;
