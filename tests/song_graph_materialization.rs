//! Materialized Native ownership and borrowed Browser uploads use actual hosts and engines.
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
    graph::{InstDef, InstId, UGenSpec},
    ring::{self, ByteInbox, EngineIo, EventRing, Garbage, NativeInstall, SpscRing},
};
use vactr::host::{
    caps::{AudioHost, GraphHandle},
    native::audio::NativeAudioHost,
    wire::{CtlMsg, HostMsg},
};
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
fn lease() -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(u64::MAX),
        resource: SongResourceRef {
            id: u32::MAX,
            generation: u32::MAX,
        },
        kind: SongResourceKind::Instrument,
    }
}
fn graph() -> GraphHandle {
    GraphHandle::Inst {
        id: InstId::new(7),
        def: Arc::new(InstDef {
            id: InstId::new(7),
            params: Box::new([]),
            nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
            edges: Box::new([]),
            node_params: Box::new([]),
        }),
    }
}
fn begin(key: SongLeaseKey, rate: u32) -> SongCommand {
    SongCommand::BeginStaging(SongStagePreparation {
        preparation: SongPreparation {
            epoch: key.epoch,
            resources: 1,
            branches: 0,
            required: SongHostCapacities {
                sample_rate: rate,
                ..SongHostCapacities::default()
            },
        },
        analysis_required: SongAnalysisCapacity { slots: 0 },
    })
}
fn reserve(key: SongLeaseKey) -> SongCommand {
    SongCommand::ReserveResource(SongResourceReservation {
        epoch: key.epoch,
        resource: key.resource,
        kind: key.kind,
    })
}
fn native_acks(host: &mut NativeAudioHost) -> Vec<SongHostAck> {
    let mut messages = Vec::new();
    host.drain(&mut messages);
    messages
        .into_iter()
        .filter_map(|m| {
            if let HostMsg::Song(ack) = m {
                Some(ack)
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn materialized_graph_cancellation_returns_exact_owner_once() {
    let mut caps = CapabilitySet::native();
    caps.max_voices = 1;
    let (mut host, mut side) = NativeAudioHost::headless(32_768, caps, 8);
    let key = lease();
    let graph = graph();
    let install = host.materialize_song_native(key, &graph).unwrap().unwrap();
    assert_eq!(install.lease, key);
    let pointer = match &install.payload {
        NativeInstall::Inst {
            resource,
            gen,
            template,
        } => {
            assert_eq!((*resource, *gen), (u32::MAX, u32::MAX));
            std::ptr::from_ref(template.as_ref())
        }
        _ => panic!("actual instrument allocation"),
    };
    assert!(!pointer.is_null());
    let mut out = [0.; 32];
    host.try_song_command(begin(key, 32_768)).unwrap();
    host.try_song_command(reserve(key)).unwrap();
    assert!(host.submit_song_native(install).is_ok());
    let mut receipts = Vec::new();
    for _ in 0..8 {
        callback(|| side.render(&mut out, 2));
        receipts.extend(native_acks(&mut host));
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|a| **a
                == SongHostAck::ResourceReady {
                    epoch: key.epoch,
                    resource: key.resource
                })
            .count(),
        1
    );
    assert!(out.iter().all(|x| *x == 0.));
    host.try_song_command(SongCommand::SealPreparation(key.epoch))
        .unwrap();
    callback(|| side.render(&mut out, 2));
    assert!(native_acks(&mut host).contains(&SongHostAck::Ready(key.epoch)));
    host.try_song_command(SongCommand::CancelPreparation(key.epoch))
        .unwrap();
    receipts.clear();
    for _ in 0..12 {
        callback(|| side.render(&mut out, 2));
        receipts.extend(native_acks(&mut host));
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|a| **a == SongHostAck::LeaseReturned(key))
            .count(),
        1
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|a| **a == SongHostAck::PreparationCancelled(key.epoch))
            .count(),
        1
    );
    // Host drain above performs the sole native upload destruction off the callback.
    for _ in 0..4 {
        callback(|| side.render(&mut out, 2));
        assert!(!native_acks(&mut host).contains(&SongHostAck::LeaseReturned(key)));
    }
}
#[test]
fn unsent_materialized_owner_is_dropped_on_control_thread_without_submission() {
    let mut caps = CapabilitySet::native();
    caps.max_voices = 1;
    let (mut host, mut side) = NativeAudioHost::headless(32_768, caps, 8);
    let retained = host
        .materialize_song_native(lease(), &graph())
        .unwrap()
        .unwrap();
    // Destruction is deliberately outside the callback probe; no protocol ownership was transferred.
    drop(retained);
    let mut out = [0.; 32];
    callback(|| side.render(&mut out, 2));
    assert!(native_acks(&mut host).is_empty());
    assert!(out.iter().all(|x| *x == 0.));
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
        cfg.bus_slots = 3;
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
fn take_records() -> Vec<Vec<u8>> {
    let n = usize::try_from(browser_abi::outbox_len()).unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), n) }.to_vec();
    browser_abi::outbox_clear();
    let mut result = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let len =
            usize::try_from(u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())).unwrap();
        at += 4;
        result.push(bytes[at..at + len].to_vec());
        at += len;
    }
    result
}
#[test]
fn browser_materialization_uses_atomic_graph_upload_and_real_resource_ready() {
    browser_abi::fix_outbox(0);
    let mut host = WasmAudioHost(Rc::new(RefCell::new(HostState::new(1_000_000))));
    let graph = graph();
    let key = lease();
    let GraphHandle::Inst { def, .. } = &graph else {
        unreachable!()
    };
    let references = Arc::strong_count(def);
    assert!(host.materialize_song_native(key, &graph).unwrap().is_none());
    assert_eq!(browser_abi::outbox_len(), 0);
    browser_abi::fix_outbox(1);
    assert!(host.submit_song_graph(key, &graph).is_err());
    assert_eq!(
        browser_abi::outbox_len(),
        0,
        "refusal cannot publish partial graph bytes"
    );
    assert_eq!(Arc::strong_count(def), references);
    browser_abi::fix_outbox(0);
    host.submit_song_graph(key, &graph).unwrap();
    let records = take_records();
    assert_eq!(records.len(), 1);
    assert_eq!(Arc::strong_count(def), references);
    let mut rig = BrowserRig::new();
    rig.push(begin(key, 32_768));
    rig.push(reserve(key));
    assert!(rig.inbox.push(&records[0]));
    let mut receipts = Vec::new();
    for _ in 0..8 {
        receipts.extend(rig.step());
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|a| **a
                == SongHostAck::ResourceReady {
                    epoch: key.epoch,
                    resource: key.resource
                })
            .count(),
        1
    );
    rig.push(SongCommand::SealPreparation(key.epoch));
    assert!(rig.step().contains(&SongHostAck::Ready(key.epoch)));
    rig.push(SongCommand::CancelPreparation(key.epoch));
    for _ in 0..12 {
        receipts.extend(rig.step());
        while rig.returned.pop().is_some() {}
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|a| **a == SongHostAck::LeaseReturned(key))
            .count(),
        1
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|a| **a == SongHostAck::PreparationCancelled(key.epoch))
            .count(),
        1
    );
    browser_abi::fix_outbox(0);
}
