//! Genuine Session controller replacements through Native and Arena callbacks.
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
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use vactr::dsp::{
    arena::StoreKind,
    caps::CapabilitySet,
    cells::AtomicCells,
    engine::{Engine, SongStagingConfig},
    ring::{self, ByteInbox, EngineIo, EventRing, Garbage, SpscRing},
};
use vactr::host::{
    caps::{
        AudioHost, GraphHandle, HostSigs, Hosts, SampleData, SongCommandRefusal,
        SongPreparationLimits, SongSubmitError,
    },
    native::audio::{AudioSide, NativeAudioHost},
    wire::{AudioEvent, CtlMsg, HostMsg, SlotControl},
};
use vactr::song::{
    assets::{DecodedSongAssetFactory, SongAssetLimits},
    routing::{SongCommand, SongHostClock, SongLeaseKey, SongReplacement},
    SnapshotEpoch, SongLimits,
};

struct Probe;
thread_local! { static MEMORY: std::cell::Cell<Option<(usize,usize)>> = const { std::cell::Cell::new(None) }; }
// SAFETY: forwards each unchanged request to System; only thread-local counters are touched.
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
    last: [f32; 32],
}
impl Rig {
    fn new(bytes: bool, bus_slots: usize) -> Self {
        let mut caps = if bytes {
            CapabilitySet::browser()
        } else {
            CapabilitySet::native()
        };
        caps.max_voices = 16;
        let (host, backend): (Box<dyn AudioHost>, Backend) = if !bytes {
            let mut cfg = vactr::host::song_profile::song_engine_config(
                8000.,
                vactr::host::native::audio::MAX_BLOCK,
                caps,
                StoreKind::NativeArc,
                2,
            )
            .unwrap();
            cfg.bus_slots = bus_slots;

            let (host, side) = NativeAudioHost::headless_with_config(cfg, 1024).unwrap();
            (Box::new(host), Backend::Native(Box::new(side)))
        } else {
            browser_abi::outbox_clear();
            browser_abi::fix_outbox(0);
            let mut cfg = vactr::host::song_profile::song_engine_config(
                8000.,
                16,
                caps,
                StoreKind::Arena { bytes: 1_000_000 },
                2,
            )
            .unwrap();
            cfg.bus_slots = bus_slots;

            cfg.bus_seconds = 10.;
            cfg.analysis_cells = 128;
            cfg.voice_seconds = 4.;
            let mut engine = Engine::with_config(cfg);
            let (acks, received) = SpscRing::split(8192);
            engine
                .configure_song_staging_for_transport(
                    SongStagingConfig {
                        preparations: 4,
                        leases: 256,
                        branches: 128,
                        control_slots: 1024,
                        analysis_slots: 128,
                        native_pcm_bytes: 1_000_000,
                        critical_receipts: 2,
                    },
                    &acks,
                )
                .unwrap();
            let (_, events) = EventRing::split(32);
            let (garbage, returned) = SpscRing::split(8192);
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
                    cells: AtomicCells::new(1024),
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
            last: [0.; 32],
        }
    }
    fn tick(&mut self) {
        self.tick_with_delivery(true);
    }
    fn tick_with_delivery(&mut self, deliver: bool) {
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
                if deliver {
                    while let Some(message) = received.pop() {
                        state.borrow_mut().on_msg(message);
                    }
                }
                while let Some(owner) = returned.pop() {
                    drop(owner);
                }
            }
        }
        self.last = out;
        self.frame += 16;
    }
}

use vactr::dsp::ring::NativeSongInstall;
use vactr::session::protocol::{ApplySongBody, DocChangedBody, SongInstrumentMuteBody, Topic};
use vactr::session::session::Outgoing;
use vactr::session::{ClientMsg, Dest, Envelope, ServerMsg, Session, SessionConfig};
use vactr::vm::fail::Failure;

struct RecordedHost {
    inner: Rc<RefCell<Box<dyn AudioHost>>>,
    commands: Rc<RefCell<Vec<SongCommand>>>,
    receipts: Rc<RefCell<Vec<HostMsg>>>,
    held: Rc<Cell<bool>>,
    replay: Rc<RefCell<std::collections::VecDeque<HostMsg>>>,
    permanent_mute: Rc<Cell<u8>>,
    prime_pressure: Rc<Cell<bool>>,
    pressure_count: Rc<Cell<usize>>,
    refused: Rc<RefCell<Vec<SongCommand>>>,
}
impl AudioHost for RecordedHost {
    fn try_song_command(&mut self, c: SongCommand) -> Result<(), SongCommandRefusal> {
        self.admit_controller_command(c)
    }
    fn song_clock(&self) -> Result<SongHostClock, Failure> {
        self.inner.borrow().song_clock()
    }
    fn materialize_song_native(
        &self,
        k: SongLeaseKey,
        g: &GraphHandle,
    ) -> Result<Option<NativeSongInstall>, Failure> {
        self.inner.borrow().materialize_song_native(k, g)
    }
    fn submit_song_native(&mut self, i: NativeSongInstall) -> Result<(), NativeSongInstall> {
        self.inner.borrow_mut().submit_song_native(i)
    }
    fn try_song_graph(&mut self, k: SongLeaseKey, g: &GraphHandle) -> Result<(), SongSubmitError> {
        self.inner.borrow_mut().try_song_graph(k, g)
    }
    fn try_song_sample(
        &mut self,
        k: SongLeaseKey,
        d: Arc<SampleData>,
    ) -> Result<(), vactr::host::caps::SongSampleRefusal> {
        self.inner.borrow_mut().try_song_sample(k, d)
    }
    fn song_sample_sender_capacity(
        &self,
    ) -> Result<vactr::host::caps::SongSampleSenderCapacity, Failure> {
        self.inner.borrow().song_sample_sender_capacity()
    }
    fn submit_song_sample(
        &mut self,
        k: SongLeaseKey,
        d: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        self.inner.borrow_mut().submit_song_sample(k, d)
    }
    fn send(&mut self, e: AudioEvent) {
        self.inner.borrow_mut().send(e)
    }
    fn control(&mut self, c: SlotControl) {
        self.inner.borrow_mut().control(c)
    }
    fn post(&mut self, c: CtlMsg) {
        self.inner.borrow_mut().post(c)
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        if self.held.get() {
            return;
        }
        let start = out.len();
        out.extend(self.replay.borrow_mut().drain(..));
        self.inner.borrow_mut().drain(out);
        let actual = out.split_off(start);
        self.receipts.borrow_mut().extend_from_slice(&actual);
        for message in actual {
            // Epoch 9999 belongs exclusively to this fixture's pressure producer.
            let owned_foreign = match message {
                HostMsg::Song(vactr::song::routing::SongHostAck::Rejected { epoch, .. }) => {
                    epoch == SnapshotEpoch(9999)
                }
                HostMsg::Song(vactr::song::routing::SongHostAck::CapacityReport(report)) => {
                    report.epoch == SnapshotEpoch(9999)
                }
                _ => false,
            };
            if !owned_foreign {
                out.push(message);
            }
        }
    }
    fn now(&self) -> f64 {
        self.inner.borrow().now()
    }
    fn swap_graph(&mut self, g: GraphHandle) {
        self.inner.borrow_mut().swap_graph(g)
    }
    fn install_sample(&mut self, id: u32, d: Arc<SampleData>) {
        self.inner.borrow_mut().install_sample(id, d)
    }
    fn retire_sample(&mut self, id: u32) {
        self.inner.borrow_mut().retire_sample(id)
    }
    fn analysis(&self) -> HostSigs {
        self.inner.borrow().analysis()
    }
}
struct Controller {
    session: Session,
    rig: Rig,
    messages: Vec<Outgoing>,
    host: Rc<RefCell<Box<dyn AudioHost>>>,
    commands: Rc<RefCell<Vec<SongCommand>>>,
    receipts: Rc<RefCell<Vec<HostMsg>>>,
    held: Rc<Cell<bool>>,
    replay: Rc<RefCell<std::collections::VecDeque<HostMsg>>>,
    permanent_mute: Rc<Cell<u8>>,
    prime_pressure: Rc<Cell<bool>>,
    pressure_count: Rc<Cell<usize>>,
    refused: Rc<RefCell<Vec<SongCommand>>>,
}
impl Controller {
    fn new(bytes: bool) -> Self {
        let mut rig = Rig::new(bytes, 51);
        let mut hosts = Hosts::noop();
        let host = Rc::new(RefCell::new(std::mem::replace(
            &mut rig.host,
            Box::new(vactr::host::noop::NoopHost),
        )));
        let commands = Rc::new(RefCell::new(Vec::new()));
        let receipts = Rc::new(RefCell::new(Vec::new()));
        let held = Rc::new(Cell::new(false));
        let permanent_mute = Rc::new(Cell::new(0));
        let prime_pressure = Rc::new(Cell::new(false));
        let pressure_count = Rc::new(Cell::new(0));
        let refused = Rc::new(RefCell::new(Vec::new()));
        let replay = Rc::new(RefCell::new(std::collections::VecDeque::new()));
        hosts.audio = Box::new(RecordedHost {
            inner: Rc::clone(&host),
            commands: Rc::clone(&commands),
            receipts: Rc::clone(&receipts),
            held: Rc::clone(&held),
            replay: Rc::clone(&replay),
            permanent_mute: Rc::clone(&permanent_mute),
            prime_pressure: Rc::clone(&prime_pressure),
            pressure_count: Rc::clone(&pressure_count),
            refused: Rc::clone(&refused),
        });
        let mut config = SessionConfig::new(rig.caps);
        config.song_assets = Some(Rc::new(DecodedSongAssetFactory::new(
            Default::default(),
            Default::default(),
            Default::default(),
        )));
        let mut session = Session::new(config, hosts);
        session
            .set_song_asset_limits(SongAssetLimits {
                max_resources: 256,
                max_pcm_bytes: 1_000_000,
                max_source_files: 16,
                max_source_bytes: 100_000,
                max_banks: 16,
                max_walk_nodes: 100_000,
                max_walk_depth: 256,
            })
            .unwrap();
        session
            .set_song_preparation_limits(SongPreparationLimits {
                capabilities: rig.caps,
                song: SongLimits::default(),
                max_resources: 256,
                max_pending_records: 4096,
                max_graph_bytes: 1_000_000,
                max_work: 8_000_000,
            })
            .unwrap();
        Self {
            session,
            rig,
            messages: Vec::new(),
            host,
            commands,
            receipts,
            held,
            replay,
            permanent_mute,
            prime_pressure,
            pressure_count,
            refused,
        }
    }
    fn apply(&mut self, seq: u64, code: String, revision: u64, edit_epoch: u64) {
        self.messages.extend(self.session.apply_from(
            1,
            Envelope::new(
                seq,
                None,
                ClientMsg::ApplySong(ApplySongBody {
                    file: "score.vact".into(),
                    code,
                    doc_revision: revision,
                    edit_epoch,
                }),
            ),
        ));
    }
    fn step(&mut self) -> [f32; 32] {
        self.messages
            .extend(self.session.tick_routed(self.rig.frame as f64 / 8000.));
        self.rig.tick_with_delivery(!self.held.get());
        self.rig.last
    }
    fn snapshot_full_ring(&mut self) -> usize {
        assert!(self.held.get());
        if let Backend::Arena {
            state,
            received,
            acks,
            ..
        } = &mut self.rig.backend
        {
            assert_eq!(
                received.len(),
                acks.capacity(),
                "actual Arena critical ring full"
            );
            while let Some(message) = received.pop() {
                state.borrow_mut().on_msg(message);
            }
        }
        let mut actual = Vec::new();
        self.host.borrow_mut().drain(&mut actual);
        let count = actual.len();
        assert_eq!(
            count,
            vactr::host::native::audio::ACK_CAPACITY,
            "actual measured Native/Arena ring snapshot"
        );
        self.replay.borrow_mut().extend(actual);
        count
    }
    fn change(&mut self, revision: u64, base: u64, edit_epoch: u64) {
        self.messages.extend(self.session.apply_from(
            1,
            Envelope::new(
                90,
                None,
                ClientMsg::DocChanged(DocChangedBody {
                    file: "score.vact".into(),
                    doc_revision: revision,
                    base_revision: base,
                    changes: Vec::new(),
                    dirty: Vec::new(),
                    edit_epoch,
                }),
            ),
        ));
    }
    fn replacement(&self) -> Option<SongReplacement> {
        self.commands.borrow().iter().rev().find_map(|c| {
            if let SongCommand::Replace(r) = c {
                Some(*r)
            } else {
                None
            }
        })
    }
    fn wait_replacement(&mut self) -> SongReplacement {
        for _ in 0..2048 {
            self.step();
            if let Some(r) = self.replacement() {
                return r;
            }
        }
        panic!("genuine Replace was not admitted: {:?}", self.messages);
    }
    fn mute(&mut self, seq: u64, epoch: SnapshotEpoch, muted: bool) {
        let selector = self
            .messages
            .iter()
            .rev()
            .find_map(|m| match &m.env.body {
                ServerMsg::SongTransportState(body) if body.epoch == epoch => {
                    body.instruments.first().cloned()
                }
                _ => None,
            })
            .unwrap();
        self.messages.extend(self.session.apply_from(
            1,
            Envelope::new(
                seq,
                None,
                ClientMsg::MuteInstrument(SongInstrumentMuteBody {
                    epoch,
                    selector,
                    muted,
                }),
            ),
        ));
    }
    fn wait_failed(&mut self, seq: u64) {
        for _ in 0..4096 {
            self.step();
            if self.messages.iter().any(|m| {
                m.env.re == Some(seq) && matches!(m.env.body, ServerMsg::SongCandidateFailed(_))
            }) {
                return;
            }
        }
        panic!("real request {seq} did not fail: {:?}", self.messages);
    }
    fn wait_applied(&mut self, seq: u64) -> vactr::song::SongApplyAck {
        for _ in 0..12000 {
            self.step();
            if let Some(ack) = self.messages.iter().find_map(|message| {
                if message.env.re == Some(seq) {
                    if let ServerMsg::SongCandidateApplied(ack) = message.env.body {
                        return Some(ack);
                    }
                    if let ServerMsg::SongCandidateFailed(ref failure) = message.env.body {
                        panic!("actual candidate {seq} failed: {failure:?}");
                    }
                }
                None
            }) {
                return ack;
            }
        }
        panic!("actual candidate {seq} never Applied: {:?}", self.messages);
    }
}
fn tone(frequency: u32, duration: &str, reordered: bool) -> String {
    let extra = if reordered {
        "inst unused:\n\tsin-osc 330 > * amp\n"
    } else {
        ""
    };
    format!("{extra}inst tone freq: float = {frequency}:\n\tsin-osc freq > * amp\nsong {{part [tone: {{s :tone > gain 0.1}}] duration: {duration}}} tail-seconds: 0 > play-song")
}
#[test]
fn playing_replacement_uses_old_cycle_and_actual_new_audio() {
    for bytes in [false, true] {
        let mut pair = Controller::new(bytes);
        pair.apply(11, tone(220, "8", false), 1, 0);
        let old = pair.wait_applied(11);
        assert!((0..256).any(|_| pair.step().iter().any(|x| x.abs() > 1e-6)));
        pair.apply(22, tone(880, "1", false), 2, 0);
        let new = pair.wait_applied(22);
        assert!(new.application_frame > old.application_frame);
        assert_eq!((new.application_frame - old.application_frame) % 16000, 0);
        assert_eq!(pair.session.applied_song_catalog().unwrap().0, new.epoch);
        let mut audible = false;
        for _ in 0..12000 {
            audible |= pair.step().iter().any(|x| x.abs() > 1e-6);
            if pair.session.runtime().song_state() == Some(sched::song::SongTransportState::Ended) {
                break;
            }
        }
        assert!(audible);
        assert_eq!(
            pair.session.runtime().song_state(),
            Some(sched::song::SongTransportState::Ended)
        );
        assert!(pair.step().iter().all(|x| x.abs() < 1e-7));
    }
}
#[test]
fn acknowledged_alias_mute_is_broadcast_after_real_reordered_apply() {
    for bytes in [false, true] {
        let mut pair = Controller::new(bytes);
        pair.apply(11, tone(440, "8", false), 1, 0);
        let old = pair.wait_applied(11);
        let selector = pair
            .messages
            .iter()
            .find_map(|message| match &message.env.body {
                ServerMsg::SongTransportState(body) if body.epoch == old.epoch => {
                    body.instruments.first().cloned()
                }
                _ => None,
            })
            .expect("actual active family selector");
        pair.messages.extend(pair.session.apply_from(
            1,
            Envelope::new(
                12,
                None,
                ClientMsg::MuteInstrument(SongInstrumentMuteBody {
                    epoch: old.epoch,
                    selector,
                    muted: true,
                }),
            ),
        ));
        for _ in 0..512 {
            pair.step();
            if pair.messages.iter().any(|m| {
                m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
            }) {
                break;
            }
        }
        assert!(pair.messages.iter().any(
            |m| m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
        ));
        pair.apply(22, tone(440, "1", true), 2, 0);
        let new = pair.wait_applied(22);
        let applied = pair
            .messages
            .iter()
            .position(|m| {
                m.env.re == Some(22) && matches!(m.env.body, ServerMsg::SongCandidateApplied(_))
            })
            .unwrap();
        let (carried, message) = pair
            .messages
            .iter()
            .enumerate()
            .find(|(_, m)| {
                matches!(&m.env.body,
            ServerMsg::SongInstrumentMuted(body) if body.epoch==new.epoch&&body.muted)
            })
            .unwrap();
        assert!(carried > applied);
        assert_eq!(message.to, Dest::Topic(Topic::Telemetry));
        assert_eq!(message.env.re, None);
        if let ServerMsg::SongInstrumentMuted(body) = &message.env.body {
            assert_eq!(body.application_frame, new.application_frame);
        }
        for _ in 0..128 {
            assert!(pair.step().iter().all(|x| x.abs() < 1e-7));
        }
    }
}

#[test]
fn draining_and_natural_end_keep_original_source_until_new_tempo_commit() {
    for bytes in [false, true] {
        for draining in [false, true] {
            let mut pair = Controller::new(bytes);
            let old_code = if draining {
                tone(220, "1/64", false).replace("tail-seconds: 0", "tail-seconds: 1")
            } else {
                tone(220, "1/256", false)
            };
            pair.apply(11, old_code, 1, 0);
            let old = pair.wait_applied(11);
            if draining {
                for _ in 0..256 {
                    pair.step();
                    if pair.session.runtime().song_state()
                        == Some(sched::song::SongTransportState::Draining)
                    {
                        break;
                    }
                }
                assert_eq!(
                    pair.session.runtime().song_state(),
                    Some(sched::song::SongTransportState::Draining)
                );
            }
            pair.apply(
                22,
                tone(880, "1/8", false).replace("tail-seconds: 0", "bpm: 60 tail-seconds: 0"),
                2,
                0,
            );
            let next = pair.wait_applied(22);
            assert_eq!((next.application_frame - old.application_frame) % 16000, 0);
            let ended = pair.messages.iter().position(|m| matches!(&m.env.body,
                ServerMsg::SongTransportState(body) if body.epoch == old.epoch && body.state == vactr::session::protocol::WireSongTransportState::Ended)).unwrap();
            let applied = pair
                .messages
                .iter()
                .position(|m| {
                    m.env.re == Some(22) && matches!(m.env.body, ServerMsg::SongCandidateApplied(_))
                })
                .unwrap();
            assert!(
                ended < applied,
                "old natural endpoint must precede future replacement"
            );
            let mut audible = false;
            for _ in 0..1024 {
                audible |= pair.step().iter().any(|x| x.abs() > 1e-6);
            }
            assert!(
                audible,
                "new tempo arrangement must play actual original graph"
            );
        }
    }
}

#[test]
fn changed_renamed_and_removed_definitions_do_not_carry_old_mute() {
    for bytes in [false, true] {
        for changed in 0..3 {
            let mut pair = Controller::new(bytes);
            pair.apply(11, tone(440, "8", false), 1, 0);
            let old = pair.wait_applied(11);
            pair.mute(12, old.epoch, true);
            for _ in 0..512 {
                pair.step();
                if pair.messages.iter().any(|m| {
                    m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
                }) {
                    break;
                }
            }
            assert!(pair
                .messages
                .iter()
                .any(|m| m.env.re == Some(12)
                    && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))));
            let next_code = match changed {
                0 => tone(880, "1", false),
                1 => tone(440, "1", false).replace("tone", "renamed"),
                _ => "song {sequence []} tail-seconds: 0 > play-song".into(),
            };
            pair.apply(22, next_code, 2, 0);
            let next = pair.wait_applied(22);
            assert!(!pair.messages.iter().any(|m| matches!(&m.env.body,
                ServerMsg::SongInstrumentMuted(body) if body.epoch == next.epoch && body.muted)));
            assert!(!pair.commands.borrow().iter().any(|c| matches!(c,
                SongCommand::PrimeMute(m) if m.epoch == next.epoch)));
            if changed < 2 {
                assert!((0..512).any(|_| pair.step().iter().any(|x| x.abs() > 1e-6)));
            } else {
                assert!(pair.step().iter().all(|x| x.abs() < 1e-7));
            }
        }
    }
}

#[test]
fn posted_document_cancel_recovers_source_and_after_arm_retains_applied_catalog() {
    for bytes in [false, true] {
        for after_arm in [false, true] {
            let mut pair = Controller::new(bytes);
            pair.apply(11, tone(220, "8", false), 1, 0);
            let old = pair.wait_applied(11);
            pair.apply(22, tone(880, "1", false), 2, 0);
            let replacement = pair.wait_replacement();
            if after_arm {
                while pair.rig.frame < replacement.activation.frame - 48 {
                    pair.step();
                }
                assert!(pair.rig.frame < replacement.activation.frame);
            } else {
                assert!(pair.rig.frame < replacement.activation.frame - 64);
            }
            pair.change(3, 2, 1);
            pair.wait_failed(22);
            assert_eq!(pair.session.applied_song_catalog().unwrap().0, old.epoch);
            assert!(!pair.messages.iter().any(|m| m.env.re == Some(22)
                && matches!(m.env.body, ServerMsg::SongCandidateApplied(_))));
            if after_arm {
                for _ in 0..4096 {
                    pair.step();
                }
                assert!(pair.receipts.borrow().iter().any(|m| matches!(m,
                    HostMsg::Song(vactr::song::routing::SongHostAck::Applied(activation)) if activation.epoch == replacement.activation.epoch)));
                assert_eq!(pair.session.applied_song_catalog().unwrap().0, old.epoch);
            } else {
                for _ in 0..512 {
                    pair.step();
                }
                assert!(pair.receipts.borrow().iter().any(|m| matches!(m,
                    HostMsg::Song(vactr::song::routing::SongHostAck::PreparationCancelled(epoch)) if *epoch == replacement.activation.epoch)));
                pair.apply(33, tone(660, "1", false), 3, 1);
                let retried = pair.wait_applied(33);
                assert_ne!(retried.epoch, replacement.activation.epoch);
                assert_eq!(
                    (retried.application_frame - old.application_frame) % 16000,
                    0
                );
                assert_eq!(
                    pair.session.applied_song_catalog().unwrap().0,
                    retried.epoch
                );
            }
        }
    }
}

#[path = "song_apply_controller/ack_pressure.rs"]
mod ack_pressure;

fn bank_factory(second: u32) -> Rc<DecodedSongAssetFactory> {
    let samples = std::collections::BTreeMap::from([
        (
            "a.wav".into(),
            Arc::new(SampleData {
                rate: 8000,
                channels: 1,
                frames: (0..8192)
                    .map(|i| (std::f32::consts::TAU * 220. * i as f32 / 8000.).sin() * 0.2)
                    .collect(),
            }),
        ),
        (
            "b.wav".into(),
            Arc::new(SampleData {
                rate: 8000,
                channels: 1,
                frames: (0..8192)
                    .map(|i| (std::f32::consts::TAU * second as f32 * i as f32 / 8000.).sin() * 0.2)
                    .collect(),
            }),
        ),
    ]);
    Rc::new(DecodedSongAssetFactory::new(
        samples,
        std::collections::BTreeMap::from([(
            vactr::value::intern::intern_kw("event-bank"),
            vec!["a.wav".into(), "b.wav".into()],
        )]),
        Default::default(),
    ))
}
#[test]
fn unused_closed_bank_member_change_prevents_mute_carry() {
    for bytes in [false, true] {
        let mut pair = Controller::new(bytes);
        pair.session.set_song_asset_factory(Some(bank_factory(330)));
        let code="inst tone bank: keyword = :event-bank:\n\tsample-play bank n: 0 rate: 1 > * amp\nsong {part [tone: {s tone > gain 0.1}] duration: 8} tail-seconds: 0 > play-song";
        pair.apply(11, code.into(), 1, 0);
        let old = pair.wait_applied(11);
        assert!((0..128).any(|_| pair.step().iter().any(|x| x.abs() > 1e-6)));
        pair.mute(12, old.epoch, true);
        for _ in 0..512 {
            pair.step();
            if pair.messages.iter().any(|m| {
                m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
            }) {
                break;
            }
        }
        assert!(pair.messages.iter().any(
            |m| m.env.re == Some(12) && matches!(m.env.body, ServerMsg::SongInstrumentMuted(_))
        ));
        pair.session.set_song_asset_factory(Some(bank_factory(660)));
        pair.apply(22, code.replace("duration: 8", "duration: 1"), 2, 0);
        let next = pair.wait_applied(22);
        assert!(!pair
            .commands
            .borrow()
            .iter()
            .any(|c| matches!(c,SongCommand::PrimeMute(m) if m.epoch==next.epoch)));
        assert!(!pair.messages.iter().any(|m|matches!(&m.env.body,ServerMsg::SongInstrumentMuted(body) if body.epoch==next.epoch&&body.muted)));
        assert!((0..128).any(|_|pair.step().iter().any(|x|x.abs()>1e-6)),"unchanged played member must resume; changed unused member invalidates whole-bank carry");
    }
}
