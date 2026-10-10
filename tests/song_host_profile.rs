//! Production profile through actual Native/Arena bootstrap, ownership and callbacks.
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
use std::{cell::RefCell, rc::Rc, sync::Arc};
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
fn generated_original(epoch: SnapshotEpoch) -> PreparedSong {
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let context = vactr::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 4_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    vactr::song::prepare_song(
        vactr::session::song::evaluate_song_candidate(
            include_str!("../examples/song-mode/generated-parts.vact"),
            "generated-parts.vact",
            1,
            epoch,
            &context,
        )
        .unwrap(),
    )
    .unwrap()
}
fn limits(caps: CapabilitySet) -> SongPreparationLimits {
    SongPreparationLimits {
        capabilities: caps,
        song: SongLimits::default(),
        max_resources: 256,
        max_pending_records: 4096,
        max_graph_bytes: 1_000_000,
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

struct SharedAudio(Rc<RefCell<Box<dyn AudioHost>>>);
impl AudioHost for SharedAudio {
    fn song_clock(&self) -> Result<SongHostClock, vm::fail::Failure> {
        self.0.borrow().song_clock()
    }
    fn try_song_command(
        &mut self,
        command: SongCommand,
    ) -> Result<(), host::caps::SongCommandRefusal> {
        self.0.borrow_mut().try_song_command(command)
    }
    fn submit_song_native(
        &mut self,
        install: ring::NativeSongInstall,
    ) -> Result<(), ring::NativeSongInstall> {
        self.0.borrow_mut().submit_song_native(install)
    }
    fn materialize_song_native(
        &self,
        lease: SongLeaseKey,
        graph: &host::caps::GraphHandle,
    ) -> Result<Option<ring::NativeSongInstall>, vm::fail::Failure> {
        self.0.borrow().materialize_song_native(lease, graph)
    }
    fn try_song_graph(
        &mut self,
        lease: SongLeaseKey,
        graph: &host::caps::GraphHandle,
    ) -> Result<(), host::caps::SongSubmitError> {
        self.0.borrow_mut().try_song_graph(lease, graph)
    }
    fn try_song_sample(
        &mut self,
        lease: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), host::caps::SongSampleRefusal> {
        self.0.borrow_mut().try_song_sample(lease, data)
    }
    fn song_sample_sender_capacity(
        &self,
    ) -> Result<host::caps::SongSampleSenderCapacity, vm::fail::Failure> {
        self.0.borrow().song_sample_sender_capacity()
    }
    fn submit_song_sample(
        &mut self,
        lease: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        self.0.borrow_mut().submit_song_sample(lease, data)
    }
    fn send(&mut self, event: host::wire::AudioEvent) {
        self.0.borrow_mut().send(event);
    }
    fn control(&mut self, control: host::wire::SlotControl) {
        self.0.borrow_mut().control(control);
    }
    fn post(&mut self, message: host::wire::CtlMsg) {
        self.0.borrow_mut().post(message);
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        self.0.borrow_mut().drain(out);
    }
    fn now(&self) -> f64 {
        self.0.borrow().now()
    }
    fn swap_graph(&mut self, graph: host::caps::GraphHandle) {
        self.0.borrow_mut().swap_graph(graph);
    }
    fn install_sample(&mut self, id: u32, data: Arc<SampleData>) {
        self.0.borrow_mut().install_sample(id, data);
    }
    fn retire_sample(&mut self, id: u32) {
        self.0.borrow_mut().retire_sample(id);
    }
    fn analysis(&self) -> host::caps::HostSigs {
        self.0.borrow().analysis()
    }
}
impl Rig {
    fn bootstrap(&mut self) -> vactr::session::Session {
        let original = std::mem::replace(&mut self.host, Box::new(vactr::host::noop::NoopHost));
        let shared = Rc::new(RefCell::new(original));
        self.host = Box::new(SharedAudio(Rc::clone(&shared)));
        let mut hosts = host::caps::Hosts::noop();
        hosts.audio = Box::new(SharedAudio(shared));
        let mut config = vactr::session::SessionConfig::new(self.caps);
        config.runtime.sample_rate = 8000;
        vactr::session::Session::new(config, hosts)
    }
    fn capacity(&mut self, epoch: SnapshotEpoch) -> SongHostCapacities {
        self.host
            .try_song_command(SongCommand::RequestCapacity(epoch))
            .unwrap();
        for _ in 0..256 {
            for message in self.tick() {
                if let HostMsg::Song(SongHostAck::CapacityReport(report)) = message {
                    if report.epoch == epoch {
                        return report.available;
                    }
                }
            }
        }
        panic!("actual capacity report absent");
    }
}
fn profile_config(sr: f32, caps: CapabilitySet) -> EngineConfig {
    vactr::host::song_profile::song_engine_config(sr, 64, caps, StoreKind::NativeArc, 2).unwrap()
}
fn key(id: u32, kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(17),
        resource: SongResourceRef { id, generation: 9 },
        kind,
    }
}
fn bus_chain(id: u32, delays: usize) -> Box<dsp::bus::BusTemplate> {
    let mut template = dsp::bus::BusTemplate::new();
    template.bus = dsp::graph::BusId::new(id);
    template.n = delays;
    for kind in &mut template.kinds[..delays] {
        *kind = dsp::graph::EffectKind::Delay;
    }
    Box::new(template)
}
#[test]
fn profile_preserves_default_memory_and_validates_before_construction() {
    for caps in [CapabilitySet::native(), CapabilitySet::browser()] {
        for rate in [8000., 48000., 192000.] {
            let plain = EngineConfig::new(&caps, rate, 64, StoreKind::NativeArc);
            assert!(plain.song_bus_memory.is_none());
            assert_eq!((plain.bus_slots, plain.template_slots), (6, 102));
            let full =
                (plain.bus_seconds * rate) as usize + dsp::granular::effect_mem_len(rate, &caps);
            let room = dsp::effects::mem_len(dsp::graph::EffectKind::Room, rate, &caps);
            let expected = room * 4 + 2 * (4 * rate as usize + 4) + rate as usize;
            let mut config = profile_config(rate, caps);
            assert!(config.validate().is_ok());
            config.bus_slots = 8;
            let engine = Engine::with_config(config);
            let regions = engine.song_bus_regions();
            assert_eq!(regions.len(), 8);
            assert_eq!(regions[0].frames, 0);
            assert!(regions[1..6].iter().all(|r| r.frames == full as u64));
            assert!(regions[6..].iter().all(|r| r.frames == expected as u64));
            if rate == 48000. {
                assert_eq!(expected, 460480);
            }
        }
        for profile in [
            ring::SongBusMemoryProfile {
                full_slots: 1,
                small_chain_seconds: 1.,
            },
            ring::SongBusMemoryProfile {
                full_slots: 52,
                small_chain_seconds: 1.,
            },
            ring::SongBusMemoryProfile {
                full_slots: 6,
                small_chain_seconds: f32::NAN,
            },
            ring::SongBusMemoryProfile {
                full_slots: 6,
                small_chain_seconds: 0.,
            },
            ring::SongBusMemoryProfile {
                full_slots: 6,
                small_chain_seconds: f32::MAX,
            },
        ] {
            let mut config = profile_config(8000., caps);
            config.song_bus_memory = Some(profile);
            assert!(config.validate().is_err());
            assert!(Engine::try_with_config(config).is_err());
        }
        let mut fractional = profile_config(8000., caps);
        fractional.sample_rate = 8000.5;
        assert!(fractional.validate().is_err());
        fractional.sample_rate = 8000.;
        fractional.bus_slots = usize::MAX;
        assert!(fractional.validate().is_err());
    }
}
#[test]
fn profile_small_stage_preserves_full_region_and_exact_box_owner() {
    let cells = AtomicCells::new(1);
    for caps in [CapabilitySet::native(), CapabilitySet::browser()] {
        let full = 80000 + dsp::granular::effect_mem_len(8000., &caps);
        let mut buses = dsp::bus::BusGraph::new_with_memory_profile(
            8,
            64,
            full,
            Some(ring::SongBusMemoryProfile {
                full_slots: 6,
                small_chain_seconds: 1.,
            }),
            &cells,
            8000.,
            &caps,
        )
        .unwrap();
        let mut empty = bus_chain(101, 0);
        let pointer = std::ptr::from_ref(empty.as_ref());
        let private = key(101, SongResourceKind::PrivateFx);
        callback(|| {
            buses
                .stage_song_bus(private, empty, &cells, 8000., &caps)
                .unwrap()
        });
        assert_eq!(buses.slots[6].resource, 101);
        assert!(buses.song_regions()[1..6]
            .iter()
            .all(|r| r.frames == full as u64));
        empty = bus_chain(101, 0);
        let refused_pointer = std::ptr::from_ref(empty.as_ref());
        let mut refused = None;
        callback(|| {
            refused = Some(
                buses
                    .stage_song_bus(private, empty, &cells, 8000., &caps)
                    .unwrap_err(),
            );
        });
        assert_eq!(
            std::ptr::from_ref(refused.as_ref().unwrap().as_ref()),
            refused_pointer
        );
        assert_ne!(pointer, refused_pointer);
        let large = bus_chain(102, 3);
        callback(|| {
            buses
                .stage_song_bus(
                    key(102, SongResourceKind::Track),
                    large,
                    &cells,
                    8000.,
                    &caps,
                )
                .unwrap()
        });
        assert_eq!(buses.slots[1].resource, 102);
        assert_ne!(buses.song_regions()[7].frames, 0);
        for id in 1..5 {
            assert!(buses.install(
                &bus_chain(200 + id, 0),
                false,
                200 + id,
                1,
                &cells,
                8000.,
                &caps
            ));
        }
        assert!(!buses.install(&bus_chain(250, 0), false, 250, 1, &cells, 8000., &caps));
        assert_ne!(
            buses.song_regions()[7].frames,
            0,
            "legacy must leave small slots untouched"
        );
        let large = bus_chain(260, 3);
        let address = std::ptr::from_ref(large.as_ref());
        let before = buses.song_regions().to_vec();
        let mut returned = None;
        callback(|| {
            returned = Some(
                buses
                    .stage_song_bus(
                        key(260, SongResourceKind::Track),
                        large,
                        &cells,
                        8000.,
                        &caps,
                    )
                    .unwrap_err(),
            );
        });
        assert_eq!(
            std::ptr::from_ref(returned.as_ref().unwrap().as_ref()),
            address
        );
        assert_eq!(buses.song_regions(), before);
    }
}
#[test]
fn measure_original_generated_parts_ready_peak_bus_demand() {
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes, 51);
        let _session = rig.bootstrap();
        let baseline = rig.capacity(SnapshotEpoch(900));
        assert_eq!((baseline.bus_slots, baseline.template_slots), (50, 55));
        let mut first =
            SongHostPreparation::begin(generated_original(SnapshotEpoch(901)), limits(rig.caps))
                .ok()
                .unwrap();
        let first = rig.complete(&mut first);
        assert_eq!(
            first.prepared().snapshot().duration(),
            value::Ratio64::from_int(24)
        );
        println!(
            "PROFILE bytes={bytes} baseline={baseline:?} required={:?}",
            first.routes().required
        );
        assert_eq!(
            (
                first.routes().required.bus_slots,
                first.routes().required.template_slots
            ),
            (25, 22)
        );
        let after_first = rig.capacity(SnapshotEpoch(902));
        assert_eq!(
            (after_first.bus_slots, after_first.template_slots),
            (25, 33)
        );
        let small = dsp::effects::mem_len(dsp::graph::EffectKind::Room, 8000., &rig.caps) as u64
            * 4
            + 64008
            + 8000;
        assert_eq!(
            baseline.bus_frames - after_first.bus_frames,
            25 * small,
            "both Native/Arena paths must preserve full regions while smaller slots fit"
        );
        let mut second =
            SongHostPreparation::begin(generated_original(SnapshotEpoch(903)), limits(rig.caps))
                .ok()
                .unwrap();
        let second = rig.complete(&mut second);
        assert_eq!(
            (
                second.routes().required.bus_slots,
                second.routes().required.template_slots
            ),
            (25, 22)
        );
        let after_second = rig.capacity(SnapshotEpoch(904));
        assert_eq!(
            (after_second.bus_slots, after_second.template_slots),
            (0, 11)
        );
        assert_eq!(after_second.bus_frames, 0);
        let keys = first
            .resources()
            .iter()
            .chain(second.resources())
            .copied()
            .collect::<Vec<_>>();
        let mut first = SongHostPreparation::retire(first);
        rig.cleanup(&mut first);
        assert!(first.take_cancelled().is_ok());
        let mut second = SongHostPreparation::retire(second);
        rig.cleanup(&mut second);
        assert!(second.take_cancelled().is_ok());
        let restored = rig.capacity(SnapshotEpoch(905));
        assert_eq!(restored, baseline);
        for key in keys {
            assert_eq!(
                rig.receipts
                    .iter()
                    .filter(|ack| **ack == SongHostAck::LeaseReturned(key))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn profile_one_below_refuses_next_and_preserves_old_audio() {
    use vactr::{
        pattern::TimeSpan,
        sched::song::{SongTransport, SongTransportState},
        value::Ratio64,
    };
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes, 50);
        let _session = rig.bootstrap();
        let baseline = rig.capacity(SnapshotEpoch(1000));
        assert_eq!((baseline.bus_slots, baseline.template_slots), (49, 55));
        let mut owner =
            SongHostPreparation::begin(generated_original(SnapshotEpoch(1001)), limits(rig.caps))
                .ok()
                .unwrap();
        let ready = rig.complete(&mut owner);
        let activation = rig.frame + 256;
        let mut old = SongTransport::new(ready, activation, SongLimits::default())
            .ok()
            .unwrap();
        old.submit_activation(rig.host.as_mut()).unwrap();
        let window = TimeSpan::new(Ratio64::ZERO, Ratio64::new(1, 4).unwrap()).unwrap();
        let mut audible = false;
        for _ in 0..128 {
            old.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 8000,
                },
                window,
            )
            .unwrap();
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    old.receive(ack).unwrap();
                }
            }
            audible |= rig.last.iter().any(|sample| sample.abs() > 0.00001);
        }
        assert!(audible);
        assert_eq!(old.state(), SongTransportState::Playing);
        let after_old = rig.capacity(SnapshotEpoch(1002));
        assert_eq!(after_old.bus_slots, 24);
        let mut next =
            SongHostPreparation::begin(generated_original(SnapshotEpoch(1003)), limits(rig.caps))
                .ok()
                .unwrap();
        let mut rejected = false;
        let mut still_audible = false;
        for _ in 0..256 {
            if next.submit(rig.host.as_mut()).is_err() {
                rejected = true;
            }
            old.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 8000,
                },
                window,
            )
            .unwrap();
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    if ack.epoch() == SnapshotEpoch(1003) {
                        let _ = next.receive(ack);
                    } else {
                        old.receive(ack).unwrap();
                    }
                }
            }
            still_audible |= rig.last.iter().any(|sample| sample.abs() > 0.00001);
            if next.progress() == SongPreparationProgress::Failed {
                rejected = true;
                break;
            }
        }
        assert!(rejected, "one physical bus short must not become Ready");
        assert!(next.take_ready().is_err());
        assert!(
            next.take_cancelled().is_ok(),
            "original refused authority must remain recoverable"
        );
        assert_eq!(old.state(), SongTransportState::Playing);
        assert!(
            still_audible,
            "failed preparation must not silence old voices"
        );
        assert_eq!(rig.capacity(SnapshotEpoch(1004)), after_old);
        let boundary = rig.frame + 256;
        old.cutoff(
            rig.host.as_mut(),
            SongEndpoints {
                epoch: SnapshotEpoch(1001),
                arrangement: boundary,
                tail_deadline: boundary,
            },
        )
        .unwrap();
        for _ in 0..4096 {
            old.advance(
                rig.host.as_mut(),
                SongHostClock {
                    frame: rig.frame,
                    sample_rate: 8000,
                },
                window,
            )
            .unwrap();
            for message in rig.tick() {
                if let HostMsg::Song(ack) = message {
                    old.receive(ack).unwrap();
                }
            }
            if old.state() == SongTransportState::Ended {
                break;
            }
        }
        assert_eq!(old.state(), SongTransportState::Ended);
        assert!(old.take_retired().is_ok());
        assert_eq!(rig.capacity(SnapshotEpoch(1005)), baseline);
    }
}

#[test]
fn configured_native_profile_is_actual_and_headless_compatibility_is_unchanged() {
    use vactr::host::song_profile::{song_engine_config, SONG_BUS_SLOTS, SONG_TEMPLATE_SLOTS};
    let mut caps = CapabilitySet::native();
    caps.max_voices = 2;
    let configured = song_engine_config(
        8000.,
        vactr::host::native::audio::MAX_BLOCK,
        caps,
        StoreKind::NativeArc,
        2,
    )
    .unwrap();
    let generic = EngineConfig::new(&caps, 8000., configured.max_block, StoreKind::NativeArc);
    assert_eq!(
        (configured.bus_slots, configured.template_slots),
        (SONG_BUS_SLOTS, SONG_TEMPLATE_SLOTS)
    );
    assert_eq!((SONG_BUS_SLOTS, SONG_TEMPLATE_SLOTS), (51, 134));
    assert_eq!(
        configured.bus_seconds.to_bits(),
        generic.bus_seconds.to_bits()
    );
    assert_eq!(
        configured.voice_seconds.to_bits(),
        generic.voice_seconds.to_bits()
    );
    assert_eq!(
        configured.orbit_delay_seconds.to_bits(),
        generic.orbit_delay_seconds.to_bits()
    );
    assert_eq!(configured.caps, generic.caps);
    assert_eq!(
        (
            configured.orbits,
            configured.analysis_cells,
            configured.event_capacity
        ),
        (
            generic.orbits,
            generic.analysis_cells,
            generic.event_capacity
        )
    );
    for (rate, block, channels) in [
        (8000.5, 512, 2),
        (7999., 512, 2),
        (192001., 512, 2),
        (f32::NAN, 512, 2),
        (f32::INFINITY, 512, 2),
        (8000., 0, 2),
        (8000., 8193, 2),
        (8000., 512, 1),
    ] {
        assert!(song_engine_config(rate, block, caps, StoreKind::NativeArc, channels).is_err());
    }
    assert!(song_engine_config(
        8000.,
        128,
        CapabilitySet::browser(),
        StoreKind::Arena { bytes: 1_000_000 },
        4
    )
    .is_ok());
    for profile in [false, true] {
        let (mut audio, mut side) = if profile {
            NativeAudioHost::headless_with_config(configured, 64).unwrap()
        } else {
            NativeAudioHost::headless(8000, caps, 64)
        };
        audio
            .try_song_command(SongCommand::RequestCapacity(SnapshotEpoch(1100)))
            .unwrap();
        let mut output = [0.; 32];
        callback(|| side.render(&mut output, 2));
        let mut messages = Vec::new();
        audio.drain(&mut messages);
        let actual = messages
            .iter()
            .find_map(|message| match message {
                HostMsg::Song(SongHostAck::CapacityReport(report))
                    if report.epoch == SnapshotEpoch(1100) =>
                {
                    Some(report.available)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(
            (actual.bus_slots, actual.template_slots),
            if profile { (50, 128) } else { (5, 96) }
        );
        assert!(output.iter().all(|sample| *sample == 0.));
    }
}
