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
        caps.max_voices = 4;
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
}

use vactr::host::caps::{GraphHandle, HostSigs, SongCommandRefusal, SongSubmitError};
use vactr::host::wire::{AudioEvent, CtlMsg, SlotControl};
use vactr::pattern::TimeSpan;
use vactr::sched::song::{SongTransport, SongTransportState};
use vactr::value::ratio::Ratio64;
use vactr::vm::fail::Failure;
struct RecordedHost<'a> {
    inner: &'a mut dyn AudioHost,
    commands: &'a mut Vec<SongCommand>,
}
impl AudioHost for RecordedHost<'_> {
    fn try_song_command(&mut self, command: SongCommand) -> Result<(), SongCommandRefusal> {
        self.inner.try_song_command(command)?;
        self.commands.push(command);
        Ok(())
    }
    fn send(&mut self, e: AudioEvent) {
        self.inner.send(e);
    }
    fn control(&mut self, c: SlotControl) {
        self.inner.control(c);
    }
    fn post(&mut self, c: CtlMsg) {
        self.inner.post(c);
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        self.inner.drain(out);
    }
    fn now(&self) -> f64 {
        self.inner.now()
    }
    fn swap_graph(&mut self, graph: GraphHandle) {
        self.inner.swap_graph(graph);
    }
    fn install_sample(&mut self, id: u32, data: Arc<SampleData>) {
        self.inner.install_sample(id, data);
    }
    fn retire_sample(&mut self, id: u32) {
        self.inner.retire_sample(id);
    }
    fn analysis(&self) -> HostSigs {
        self.inner.analysis()
    }
}
fn fill_commands(host: &mut dyn AudioHost) -> usize {
    let command = SongCommand::Mute(SongMute {
        epoch: SnapshotEpoch(9999),
        instrument: 1,
        muted: true,
        frame: 0,
    });
    for count in 0..8192 {
        if let Err(refusal) = host.try_song_command(command) {
            assert_eq!(refusal.command, command);
            assert!(matches!(refusal.error, SongSubmitError::Backpressure));
            return count;
        }
    }
    panic!("real command ingress was not bounded");
}
fn run(code: &str, bytes: bool, epoch: u64) {
    run_variant(code, bytes, epoch, false, false);
}
fn run_variant(
    code: &str,
    bytes: bool,
    epoch: u64,
    pressure: bool,
    partitioned: bool,
) -> Vec<SongAudioEvent> {
    let mut rig = Rig::new(bytes);
    let prepared = candidate(code, SnapshotEpoch(epoch));
    let settings = prepared.snapshot().settings();
    let duration = prepared.snapshot().duration();
    let mut owner = SongHostPreparation::begin(prepared, limits(rig.caps))
        .ok()
        .unwrap();
    let ready = rig.complete(&mut owner);
    let activation = rig.frame.checked_add(8192).unwrap();
    let mut transport = SongTransport::new(ready, activation, SongLimits::default())
        .ok()
        .unwrap();
    assert_eq!(transport.settings(), settings);
    if pressure && bytes {
        browser_abi::fix_outbox(4096);
    }
    transport.submit_activation(rig.host.as_mut()).unwrap();
    let filled = if pressure {
        fill_commands(rig.host.as_mut())
    } else {
        0
    };
    let mut foreign = Vec::new();
    let mut commands = Vec::new();
    let mut audible = false;
    for tick in 0..40000 {
        let end = if partitioned {
            duration.min(Ratio64::new(i64::from(tick + 1), 4).unwrap())
        } else {
            duration
        };
        let mut recorded = RecordedHost {
            inner: rig.host.as_mut(),
            commands: &mut commands,
        };
        transport
            .advance(
                &mut recorded,
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 32768,
                },
                TimeSpan::new(Ratio64::ZERO, end).unwrap(),
            )
            .unwrap();
        for message in rig.tick() {
            if let HostMsg::Song(ack) = message {
                if ack.epoch() == SnapshotEpoch(9999) {
                    foreign.push(ack);
                } else {
                    transport.receive(ack).unwrap();
                }
            }
        }
        audible |= rig.last.iter().any(|sample| sample.abs() > 0.00001);
        if transport.state() == SongTransportState::Ended {
            break;
        }
    }
    assert_eq!(transport.state(), SongTransportState::Ended, "bytes={bytes}, pressure={pressure}, partitioned={partitioned}, failure={:?}, receipts={:?}", transport.failure(), rig.receipts);
    assert!(transport.failure().is_none());
    assert_eq!(transport.applied_activation().unwrap().frame, activation);
    assert_eq!(transport.take_retired().unwrap().revision(), 17);
    assert!(transport.take_retired().is_err());
    if duration > Ratio64::ZERO {
        assert!(audible);
    }
    assert!(rig
        .receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::LeaseReturned(_))));
    assert_eq!(foreign.len(), filled);
    assert!(foreign.iter().all(|ack| matches!(
        ack,
        SongHostAck::Rejected {
            epoch: SnapshotEpoch(9999),
            ..
        }
    )));
    assert_eq!(
        commands
            .iter()
            .filter(|c| matches!(c, SongCommand::Endpoints(_)))
            .count(),
        1
    );
    let mut events = commands
        .into_iter()
        .filter_map(|command| {
            if let SongCommand::Event(mut event) = command {
                event.frame -= activation;
                event.event.time -= activation as f64 / 32768.;
                Some(event)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    events.sort_by_key(|event| (event.frame, event.branch.0, event.generation));
    assert!(events.iter().all(|event| event.frame
        < SongLimits::default()
            .frames_at(
                duration
                    .checked_mul(settings.seconds_per_cycle().unwrap())
                    .unwrap(),
                32768
            )
            .unwrap()));
    events
}
#[test]
fn finite_native_and_arena_transport_repeats_then_changes_and_retires() {
    let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nlet a {part [tone: {s :tone > n 0 > gain 0.2}] duration: 1}\nlet b {part [tone: {s :tone > n 7 > gain 0.2}] duration: 1}\nsong {sequence [{part-repeat a 2} b]} tail-seconds: 0 > play-song";
    for bytes in [false, true] {
        run(code, bytes, 601);
    }
}
#[test]
fn empty_native_and_arena_transport_returns_original_owner() {
    for bytes in [false, true] {
        run("song {sequence []} tail-seconds: 0 > play-song", bytes, 602);
    }
}

#[test]
fn canonical_partition_and_real_queue_retry_preserve_exact_events() {
    let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nlet a {part [tone: {s :tone > n 60 > gain 0.2}] duration: 1}\nlet b {part [tone: {s :tone > n 67 > gain 0.2}] duration: 1}\nsong {sequence [{part-repeat a 2} b]} tail-seconds: 0 > play-song";
    for bytes in [false, true] {
        let baseline = run_variant(code, bytes, 603, false, false);
        let retried = run_variant(code, bytes, 603, true, true);
        assert_eq!(baseline, retried);
        assert_eq!(baseline.len(), 3);
        assert_eq!(
            baseline.iter().map(|event| event.frame).collect::<Vec<_>>(),
            vec![0, 65536, 131072]
        );
        let freq = vactr::dsp::controls::row("freq").unwrap().ctl;
        for (event, note) in baseline.iter().zip([60., 60., 67.]) {
            let value = event
                .event
                .controls()
                .iter()
                .find(|(id, _)| *id == freq)
                .unwrap()
                .1;
            assert_eq!(
                value,
                vactr::host::wire::Ctl::Const(vactr::sched::commit::note_to_freq(note) as f32)
            );
        }
    }
}
#[test]
fn frozen_chords_are_encoded_once_against_private_graphs() {
    let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > chord [:c :maj] > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
    for bytes in [false, true] {
        let events = run_variant(code, bytes, 604, false, true);
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(|event| event.frame == 0));
        let freq = vactr::dsp::controls::row("freq").unwrap().ctl;
        let mut bits = events
            .iter()
            .map(|event| {
                match event
                    .event
                    .controls()
                    .iter()
                    .find(|(id, _)| *id == freq)
                    .unwrap()
                    .1
                {
                    vactr::host::wire::Ctl::Const(value) => value.to_bits(),
                    _ => panic!("frozen chord became live cell"),
                }
            })
            .collect::<Vec<_>>();
        bits.sort_unstable();
        let mut expected =
            [60., 64., 67.].map(|note| (vactr::sched::commit::note_to_freq(note) as f32).to_bits());
        expected.sort_unstable();
        assert_eq!(bits, expected);
    }
}

#[test]
fn accepted_cutoff_stops_future_onsets_and_returns_full_epoch() {
    let code = "inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > n 60 > gain 0.2}] duration: 4} tail-seconds: 0 > play-song";
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut owner =
            SongHostPreparation::begin(candidate(code, SnapshotEpoch(605)), limits(rig.caps))
                .ok()
                .unwrap();
        let ready = rig.complete(&mut owner);
        let activation = rig.frame + 256;
        let mut core = SongTransport::new(ready, activation, SongLimits::default())
            .ok()
            .unwrap();
        core.submit_activation(rig.host.as_mut()).unwrap();
        for _ in 0..4 {
            core.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 32768,
                },
                TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(4)).unwrap(),
            )
            .unwrap();
        }
        let boundary = activation + 32768;
        core.cutoff(
            rig.host.as_mut(),
            SongEndpoints {
                epoch: SnapshotEpoch(605),
                arrangement: boundary,
                tail_deadline: boundary,
            },
        )
        .unwrap();
        let mut audible = false;
        for _ in 0..10000 {
            core.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 32768,
                },
                TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(4)).unwrap(),
            )
            .unwrap();
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    core.receive(ack).unwrap();
                }
            }
            if rig.frame <= boundary {
                audible |= rig.last.iter().any(|v| v.abs() > 0.00001);
            }
            if rig.frame > boundary + 128 {
                assert!(rig.last.iter().all(|v| v.abs() < 0.00001));
            }
            if core.state() == SongTransportState::Ended {
                break;
            }
        }
        assert!(audible);
        assert_eq!(core.state(), SongTransportState::Ended);
        assert_eq!(core.endpoints().arrangement, boundary);
        assert_eq!(core.take_retired().unwrap().revision(), 17);
        assert!(!rig
            .receipts
            .iter()
            .any(|ack| matches!(ack, SongHostAck::Rejected { .. })));
    }
}
#[test]
fn abort_before_activation_preserves_original_cancelled_owner() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut owner = SongHostPreparation::begin(
            candidate(
                "song {part [tone: {s :analog}] duration: 1} tail-seconds: 0 > play-song",
                SnapshotEpoch(606),
            ),
            limits(rig.caps),
        )
        .ok()
        .unwrap();
        let ready = rig.complete(&mut owner);
        let mut core = SongTransport::new(ready, rig.frame + 256, SongLimits::default())
            .ok()
            .unwrap();
        core.abort(Failure::new(
            vactr::vm::fail::FailCode::Type,
            "actual controller abort",
        ));
        let mut original = None;
        for _ in 0..2048 {
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
            assert!(rig.last.iter().all(|v| *v == 0.));
            if let Ok(prepared) = core.take_cancelled() {
                original = Some(prepared);
                break;
            }
        }
        assert_eq!(original.unwrap().revision(), 17);
        assert_eq!(core.state(), SongTransportState::Failed);
        assert!(core.take_cancelled().is_err());
    }
}
