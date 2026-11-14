//! Checked borrowed graph admission, unchanged framing and actual Engine adoption.
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
    arena::{encode_bus, encode_inst, StoreKind, SLICE_BYTES},
    caps::CapabilitySet,
    cells::AtomicCells,
    engine::{Engine, EngineConfig, SongStagingConfig},
    graph::{BusDef, BusId, EffectKind, EffectSpec, InstDef, InstId, UGenSpec},
    ring::{self, ByteInbox, EngineIo, EventRing, Garbage, SpscRing},
};
use vactr::host::{
    caps::{AudioHost, GraphHandle, SongSubmitError},
    wire::{Ctl, CtlMsg, HostMsg},
};
use vactr::sched::slots::CtlId;
use vactr::song::{routing::*, SnapshotEpoch};
struct Probe;
thread_local! { static CALLBACK_MEMORY: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) }; }
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a + 1, d)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a, d + 1)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        CALLBACK_MEMORY.with(|c| {
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
    CALLBACK_MEMORY.with(|c| c.set(Some((0, 0))));
    run();
    let observed = CALLBACK_MEMORY.with(|c| c.replace(None).unwrap());
    assert_eq!(
        observed,
        (0, 0),
        "actual callback must not allocate or destroy uploads"
    );
}
struct BrowserRig {
    engine: Engine,
    inbox: ByteInbox,
    events: ring::EventConsumer,
    acks: ring::AckProducer,
    received: ring::AckConsumer,
    cells: AtomicCells,
    garbage: ring::Producer<Garbage>,
    returned: ring::Consumer<Garbage>,
}
impl BrowserRig {
    fn new() -> Self {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 1;
        let mut cfg = EngineConfig::new(&caps, 32_768., 16, StoreKind::Arena { bytes: 1_000_000 });
        cfg.template_slots = 3;
        cfg.bus_slots = 4;
        cfg.bus_seconds = 40.;
        cfg.voice_seconds = 0.1;
        let mut engine = Engine::with_config(cfg);
        let (acks, received) = SpscRing::split(8);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 2,
                    leases: 8,
                    branches: 2,
                    control_slots: 8,
                    analysis_slots: 8,
                    native_pcm_bytes: 4096,
                    critical_receipts: 2,
                },
                &acks,
            )
            .unwrap();
        let (_, events) = EventRing::split(2);
        let (garbage, returned) = SpscRing::split(8);
        Self {
            engine,
            inbox: ByteInbox::new(),
            events,
            acks,
            received,
            cells: AtomicCells::new(8),
            garbage,
            returned,
        }
    }
    fn push(&mut self, command: SongCommand) {
        let mut bytes = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut bytes);
        assert_ne!(n, 0);
        assert!(self.inbox.push(&bytes[..n]));
    }
    fn step(&mut self) -> Vec<SongHostAck> {
        let mut out = [0.; 32];
        callback(|| {
            self.engine.process(
                &mut EngineIo {
                    events: &mut self.events,
                    controls: &mut self.inbox,
                    acks: &mut self.acks,
                    cells: &mut self.cells,
                    garbage: Some(&mut self.garbage),
                },
                &mut out,
                16,
            )
        });
        assert!(out.iter().all(|x| *x == 0.));
        std::iter::from_fn(|| self.received.pop())
            .filter_map(|m| {
                if let HostMsg::Song(a) = m {
                    Some(a)
                } else {
                    None
                }
            })
            .collect()
    }
}

fn host() -> WasmAudioHost {
    WasmAudioHost(Rc::new(RefCell::new(HostState::new(1_000_000))))
}
fn lease(id: u32, kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(u64::MAX),
        resource: SongResourceRef {
            id,
            generation: u32::MAX,
        },
        kind,
    }
}
fn graph(id: u32, kind: SongResourceKind) -> GraphHandle {
    if kind == SongResourceKind::Instrument {
        return GraphHandle::Inst {
            id: InstId::new(id),
            def: Arc::new(InstDef {
                id: InstId::new(id),
                params: Box::new([]),
                nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
                edges: Box::new([]),
                node_params: Box::new([]),
            }),
        };
    }
    let def = Arc::new(BusDef {
        id: BusId::new(id),
        chain: Box::new([]),
    });
    if kind == SongResourceKind::Master {
        GraphHandle::Master(def)
    } else {
        GraphHandle::Bus {
            id: BusId::new(id),
            def,
        }
    }
}
fn take_records() -> Vec<Vec<u8>> {
    let length = usize::try_from(browser_abi::outbox_len()).unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), length) }.to_vec();
    browser_abi::outbox_clear();
    let mut result = Vec::new();
    browser_abi::for_each_record(&bytes, |record| result.push(record.to_vec()));
    result
}
fn encoded(key: SongLeaseKey, graph: &GraphHandle) -> Vec<u8> {
    let mut bytes = Vec::new();
    match graph {
        GraphHandle::Inst { def, .. } => encode_inst(def, &mut bytes),
        GraphHandle::Bus { def, .. } => encode_bus(def, false, &mut bytes),
        GraphHandle::Master(def) => encode_bus(def, true, &mut bytes),
    }
    .unwrap();
    let mut result = vec![0; 22 + bytes.len()];
    let length = ring::encode_song_graph_record(key, &bytes, &mut result);
    assert_eq!(length, result.len());
    result
}

#[test]
fn checked_graph_default_is_unavailable_and_preserves_borrowed_owner() {
    let graph = graph(7, SongResourceKind::Instrument);
    let mut host = vactr::host::noop::NoopHost;
    let GraphHandle::Inst { def, .. } = &graph else {
        unreachable!()
    };
    let pointer = Arc::as_ptr(def);
    let count = Arc::strong_count(def);
    assert!(matches!(
        host.try_song_graph(lease(7, SongResourceKind::Instrument), &graph),
        Err(SongSubmitError::Unavailable)
    ));
    assert_eq!(Arc::as_ptr(def), pointer);
    assert_eq!(Arc::strong_count(def), count);
}

#[test]
fn checked_graph_pressure_retains_wire_and_retry_frames_following_record() {
    let graph = graph(7, SongResourceKind::Instrument);
    let key = lease(7, SongResourceKind::Instrument);
    let wire = encoded(key, &graph);
    let GraphHandle::Inst { def, .. } = &graph else {
        unreachable!()
    };
    let pointer = Arc::as_ptr(def);
    let count = Arc::strong_count(def);
    let framed = 4 + wire.len();
    browser_abi::fix_outbox(framed);
    let filler = vec![0x55; wire.len()];
    assert!(browser_abi::push_record(&filler));
    let before = browser_abi::outbox_len();
    let mut host = host();
    for _ in 0..2 {
        assert!(matches!(
            host.try_song_graph(key, &graph),
            Err(SongSubmitError::Backpressure)
        ));
        assert_eq!(browser_abi::outbox_len(), before);
        assert_eq!(Arc::as_ptr(def), pointer);
        assert_eq!(Arc::strong_count(def), count);
    }
    assert_eq!(take_records(), vec![filler]);
    host.try_song_graph(key, &graph).unwrap();
    assert_eq!(take_records(), vec![wire.clone()]);
    browser_abi::fix_outbox(0);
    host.try_song_graph(key, &graph).unwrap();
    let following = SongCommand::RequestCapacity(key.epoch);
    host.try_song_command(following).unwrap();
    let records = take_records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0], wire);
    let mut expected = [0; CtlMsg::MAX_LEN];
    let n = CtlMsg::Song(following).encode(&mut expected);
    assert_eq!(records[1], expected[..n]);
    assert_eq!(Arc::strong_count(def), count);
}

#[test]
fn checked_graph_permanent_size_refusal_never_becomes_backpressure() {
    let graph = graph(7, SongResourceKind::Instrument);
    let key = lease(7, SongResourceKind::Instrument);
    let framed = 4 + encoded(key, &graph).len();
    let mut host = host();
    browser_abi::fix_outbox(framed - 1);
    let drops = browser_abi::outbox_dropped();
    for _ in 0..2 {
        assert!(matches!(
            host.try_song_graph(key, &graph),
            Err(SongSubmitError::Invalid(_))
        ));
        assert_eq!(browser_abi::outbox_len(), 0);
        assert_eq!(browser_abi::outbox_dropped(), drops);
    }
    browser_abi::fix_outbox(0);
    let oversized = GraphHandle::Inst {
        id: InstId::new(7),
        def: Arc::new(InstDef {
            id: InstId::new(7),
            params: vec![(CtlId::new(0), Ctl::Const(1.)); 10000].into_boxed_slice(),
            nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
            edges: Box::new([]),
            node_params: Box::new([]),
        }),
    };
    let GraphHandle::Inst { def, .. } = &oversized else {
        unreachable!()
    };
    let mut payload = Vec::new();
    encode_inst(def, &mut payload).unwrap();
    assert!(payload.len() > SLICE_BYTES);
    assert!(matches!(
        host.try_song_graph(key, &oversized),
        Err(SongSubmitError::Invalid(_))
    ));
    assert_eq!(browser_abi::outbox_len(), 0);
}

#[test]
fn checked_graph_malformed_identity_controls_and_structure_publish_nothing() {
    browser_abi::fix_outbox(0);
    let mut host = host();
    let key = lease(7, SongResourceKind::Instrument);
    for mode in 0..5 {
        let mut def = InstDef {
            id: InstId::new(7),
            params: Box::new([]),
            nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
            edges: Box::new([]),
            node_params: Box::new([]),
        };
        match mode {
            0 => def.params = vec![(CtlId::new(0), Ctl::Const(f32::NAN))].into_boxed_slice(),
            1 => def.nodes = vec![UGenSpec::Const(f32::INFINITY)].into_boxed_slice(),
            2 => def.node_params = vec![(9, CtlId::new(0), Ctl::Const(0.))].into_boxed_slice(),
            3 => {
                def.edges = vec![dsp::graph::Edge {
                    from: 9,
                    to: 0,
                    port: 0,
                    output: 0,
                }]
                .into_boxed_slice()
            }
            _ => def.id = InstId::new(8),
        }
        let graph = GraphHandle::Inst {
            id: InstId::new(7),
            def: Arc::new(def),
        };
        assert!(matches!(
            host.try_song_graph(key, &graph),
            Err(SongSubmitError::Invalid(_))
        ));
        assert_eq!(browser_abi::outbox_len(), 0);
    }
    let bus = GraphHandle::Bus {
        id: BusId::new(7),
        def: Arc::new(BusDef {
            id: BusId::new(7),
            chain: vec![EffectSpec {
                kind: EffectKind::Gain,
                params: vec![(CtlId::new(0), Ctl::Const(f32::NAN))].into_boxed_slice(),
            }]
            .into_boxed_slice(),
        }),
    };
    assert!(matches!(
        host.try_song_graph(lease(7, SongResourceKind::Track), &bus),
        Err(SongSubmitError::Invalid(_))
    ));
    assert!(matches!(
        host.try_song_graph(key, &bus),
        Err(SongSubmitError::Invalid(_))
    ));
    assert_eq!(browser_abi::outbox_len(), 0);
}

#[test]
fn checked_graph_four_kinds_reach_real_engine_resource_ready_and_cancel() {
    browser_abi::fix_outbox(0);
    let mut host = host();
    let mut rig = BrowserRig::new();
    let kinds = [
        SongResourceKind::Instrument,
        SongResourceKind::PrivateFx,
        SongResourceKind::Track,
        SongResourceKind::Master,
    ];
    let epoch = SnapshotEpoch(u64::MAX);
    rig.push(SongCommand::BeginStaging(SongStagePreparation {
        preparation: SongPreparation {
            epoch,
            branches: 0,
            resources: 4,
            required: SongHostCapacities {
                sample_rate: 32768,
                ..SongHostCapacities::default()
            },
        },
        analysis_required: SongAnalysisCapacity { slots: 0 },
    }));
    for (index, kind) in kinds.into_iter().enumerate() {
        let id = u32::try_from(index + 1).unwrap();
        let key = lease(id, kind);
        let graph = graph(id, kind);
        rig.push(SongCommand::ReserveResource(SongResourceReservation {
            epoch,
            resource: key.resource,
            kind,
        }));
        host.try_song_graph(key, &graph).unwrap();
        let record = take_records();
        assert_eq!(record, vec![encoded(key, &graph)]);
        assert!(rig.inbox.push(&record[0]));
    }
    let mut receipts = Vec::new();
    for _ in 0..8 {
        receipts.extend(rig.step());
    }
    for (index, kind) in kinds.into_iter().enumerate() {
        let key = lease(u32::try_from(index + 1).unwrap(), kind);
        assert_eq!(
            receipts
                .iter()
                .filter(|ack| **ack
                    == SongHostAck::ResourceReady {
                        epoch,
                        resource: key.resource,
                    })
                .count(),
            1
        );
    }
    assert!(!receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::Rejected { .. })));
    assert!(rig.engine.template(InstId::new(1)).is_none());
    host.try_song_command(SongCommand::SealPreparation(epoch))
        .unwrap();
    for record in take_records() {
        assert!(rig.inbox.push(&record));
    }
    assert!(rig.step().contains(&SongHostAck::Ready(epoch)));
    host.try_song_command(SongCommand::CancelPreparation(epoch))
        .unwrap();
    for record in take_records() {
        assert!(rig.inbox.push(&record));
    }
    let mut returned = Vec::new();
    for _ in 0..16 {
        returned.extend(rig.step());
        while rig.returned.pop().is_some() {}
    }
    for (index, kind) in kinds.into_iter().enumerate() {
        let key = lease(u32::try_from(index + 1).unwrap(), kind);
        assert_eq!(
            returned
                .iter()
                .filter(|ack| **ack == SongHostAck::LeaseReturned(key))
                .count(),
            1
        );
    }
    assert_eq!(
        returned
            .iter()
            .filter(|ack| **ack == SongHostAck::PreparationCancelled(epoch))
            .count(),
        1
    );
}

#[test]
fn empty_outbox_size_predicate_is_checked_and_has_no_side_effects() {
    browser_abi::fix_outbox(16);
    assert!(browser_abi::push_record(&[7]));
    let length = browser_abi::outbox_len();
    let drops = browser_abi::outbox_dropped();
    assert!(browser_abi::record_fits_empty_outbox(12));
    assert!(!browser_abi::record_fits_empty_outbox(13));
    assert!(!browser_abi::record_fits_empty_outbox(usize::MAX));
    assert_eq!(browser_abi::outbox_len(), length);
    assert_eq!(browser_abi::outbox_dropped(), drops);
    browser_abi::fix_outbox(0);
    assert!(browser_abi::record_fits_empty_outbox(65558));
}
