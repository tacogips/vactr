//! Staging transport preserves full keys and allocation ownership, never Ready.
// Compile the actual portable browser sender on native test threads.
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
use std::sync::Arc;
use vactr::dsp::cells::{AtomicCells, CellId};
use vactr::dsp::ring::{
    self, Budget, ByteInbox, ControlSource, EngineIo, EventRing, Garbage, NativeInstall,
    NativeRecord, NativeSongInstall, Record, SpscRing,
};
use vactr::host::caps::{AudioHost, SampleData};
use vactr::host::wire::{CtlMsg, HostMsg};
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;
fn lease(kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(u64::MAX),
        resource: SongResourceRef {
            id: u32::MAX,
            generation: u32::MAX,
        },
        kind,
    }
}
fn data(n: usize) -> Arc<SampleData> {
    Arc::new(SampleData {
        rate: 48000,
        channels: 1,
        frames: vec![0.25; n].into_boxed_slice(),
    })
}
fn upload(key: SongLeaseKey, data: Arc<SampleData>) -> NativeSongInstall {
    NativeSongInstall {
        lease: key,
        payload: NativeInstall::Sample {
            resource: key.resource.id,
            gen: key.resource.generation,
            data,
        },
    }
}
fn commands() -> Vec<(SongCommand, usize)> {
    vec![
        (
            SongCommand::BeginStaging(SongStagePreparation {
                preparation: SongPreparation {
                    epoch: SnapshotEpoch(u64::MAX),
                    branches: u32::MAX,
                    resources: u32::MAX,
                    required: SongHostCapacities {
                        sample_rate: 48000,
                        ..SongHostCapacities::default()
                    },
                },
                analysis_required: SongAnalysisCapacity { slots: u32::MAX },
            }),
            74,
        ),
        (
            SongCommand::InitCells(SongCellInit {
                lease: lease(SongResourceKind::ControlCells),
                cell: CellId::new(u32::MAX),
                value: -1.25,
            }),
            27,
        ),
        (
            SongCommand::ReserveAnalysis(SongAnalysisReservation {
                lease: lease(SongResourceKind::AnalysisBank),
                slots: 0,
            }),
            23,
        ),
        (SongCommand::CancelPreparation(SnapshotEpoch(u64::MAX)), 10),
        (
            SongCommand::CancelLease(lease(SongResourceKind::Sample)),
            19,
        ),
    ]
}
#[test]
fn appended_commands_and_native_byte_pods_preserve_exact_sizes_and_keys() {
    let (mut tx, mut native) = SpscRing::split(8);
    let mut inbox = ByteInbox::new();
    for (c, len) in commands() {
        let msg = CtlMsg::Song(c);
        let mut b = [0; CtlMsg::MAX_LEN];
        let n = msg.encode(&mut b);
        assert_eq!(n, len);
        assert_eq!(CtlMsg::decode(&b[..n]).unwrap(), (msg, n));
        for end in 0..n {
            assert!(CtlMsg::decode(&b[..end]).is_err());
        }
        assert!(tx.push(NativeRecord::Msg(msg)).is_ok());
        assert!(inbox.push(&b[..n]));
        let budget = Budget {
            install_bytes: usize::MAX,
            batch: true,
        };
        match (native.next(budget), inbox.next(budget)) {
            (Some(Record::Msg(a)), Some(Record::Msg(b))) => assert_eq!(a, b),
            _ => panic!("full-key POD"),
        }
    }
}
#[test]
fn all_kinds_append_only_holes_invalid_and_scalar_rules_match() {
    for kind in [
        SongResourceKind::Instrument,
        SongResourceKind::PrivateFx,
        SongResourceKind::Track,
        SongResourceKind::Master,
        SongResourceKind::Sample,
        SongResourceKind::ControlCells,
        SongResourceKind::AnalysisBank,
    ] {
        let c = CtlMsg::Song(SongCommand::CancelLease(lease(kind)));
        let mut b = [0; CtlMsg::MAX_LEN];
        let n = c.encode(&mut b);
        assert_eq!(b[18], kind as u8);
        assert_eq!(CtlMsg::decode(&b[..n]).unwrap().0, c);
        for invalid in [5, 6, 9, 255] {
            b[18] = invalid;
            assert!(CtlMsg::decode(&b[..n]).is_err());
        }
    }
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let c = SongCommand::InitCells(SongCellInit {
            lease: lease(SongResourceKind::ControlCells),
            cell: CellId::new(0),
            value,
        });
        assert!(!c.valid());
        assert_eq!(CtlMsg::Song(c).encode(&mut [0; CtlMsg::MAX_LEN]), 0);
    }
    assert!(!SongCommand::InitCells(SongCellInit {
        lease: lease(SongResourceKind::Sample),
        cell: CellId::new(0),
        value: 0.
    })
    .valid());
    assert!(!SongCommand::ReserveAnalysis(SongAnalysisReservation {
        lease: lease(SongResourceKind::ControlCells),
        slots: 0
    })
    .valid());
}
#[test]
fn appended_receipts_preserve_kind_and_full_width_offset() {
    for (ack, len) in [
        (
            SongHostAck::LeaseReturned(lease(SongResourceKind::AnalysisBank)),
            19,
        ),
        (
            SongHostAck::PreparationCancelled(SnapshotEpoch(u64::MAX)),
            10,
        ),
        (
            SongHostAck::SliceAccepted {
                lease: lease(SongResourceKind::Sample),
                offset: u32::MAX,
            },
            23,
        ),
    ] {
        let msg = HostMsg::Song(ack);
        let mut b = [0; HostMsg::MAX_LEN];
        let n = msg.encode(&mut b);
        assert_eq!(n, len);
        assert_eq!(HostMsg::decode(&b[..n]).unwrap(), (msg, n));
        for end in 0..n {
            assert!(HostMsg::decode(&b[..end]).is_err());
        }
    }
}
#[test]
fn graph_begin_and_slices_roundtrip_borrowed_exact_framing() {
    let mut inbox = ByteInbox::new();
    let mut b = vec![0; ring::INBOX_SLOT_BYTES];
    let key = lease(SongResourceKind::Instrument);
    let n = ring::encode_song_graph_record(key, &[3, 7, 11], &mut b);
    assert_eq!(n, 25);
    assert!(inbox.push(&b[..n]));
    match inbox.next(Budget {
        install_bytes: 3,
        batch: true,
    }) {
        Some(Record::SongGraph { lease, bytes }) => {
            assert_eq!(lease, key);
            assert_eq!(bytes, [3, 7, 11]);
        }
        _ => panic!("graph"),
    }
    let key = lease(SongResourceKind::Sample);
    let n = ring::encode_song_sample_begin(key, 2, 2, 48000, &mut b);
    assert_eq!(n, 27);
    assert!(inbox.push(&b[..n]));
    match inbox.next(Budget {
        install_bytes: 0,
        batch: true,
    }) {
        Some(Record::SongSampleBegin {
            lease,
            frames,
            channels,
            rate,
        }) => assert_eq!((lease, frames, channels, rate), (key, 2, 2, 48000)),
        _ => panic!("begin"),
    }
    let n = ring::encode_song_slice(key, 100, &[0.25, -0.5], &mut b);
    assert_eq!(n, 34);
    assert!(inbox.push(&b[..n]));
    assert!(inbox
        .next(Budget {
            install_bytes: 7,
            batch: true
        })
        .is_none());
    match inbox.next(Budget {
        install_bytes: 8,
        batch: true,
    }) {
        Some(Record::SongSlice {
            lease,
            offset,
            data,
        }) => {
            assert_eq!((lease, offset), (key, 100));
            assert_eq!(data, [0, 0, 128, 62, 0, 0, 0, 191]);
        }
        _ => panic!("slice"),
    }
}
#[test]
fn malformed_upload_consumes_only_its_slot_and_preserves_following_identity() {
    let key = lease(SongResourceKind::Sample);
    let mut b = [0; 64];
    let n = ring::encode_song_slice(key, 0, &[1.], &mut b);
    let budget = Budget {
        install_bytes: usize::MAX,
        batch: true,
    };
    for end in 1..n {
        let mut inbox = ByteInbox::new();
        assert!(inbox.push(&b[..end]));
        assert!(inbox.push(&b[..n]));
        assert!(inbox.next(budget).is_none());
        assert!(matches!(inbox.next(budget),Some(Record::SongSlice{lease,..}) if lease==key));
    }
    for at in [17, 22] {
        let mut hostile = b;
        hostile[at] = 255;
        let mut inbox = ByteInbox::new();
        assert!(inbox.push(&hostile[..n]));
        assert!(inbox.next(budget).is_none());
    }
    assert_eq!(ring::encode_song_slice(key, u32::MAX, &[1.], &mut b), 0);
    assert_eq!(ring::encode_song_slice(key, 0, &[f32::NAN], &mut b), 0);
    assert_eq!(
        ring::encode_song_sample_begin(key, u32::MAX, 2, 48000, &mut b),
        0
    );
    assert_eq!(ring::encode_song_sample_begin(key, 1, 3, 48000, &mut b), 0);
    assert_eq!(ring::encode_song_graph_record(key, &[1], &mut b), 0);
}
fn engine() -> vactr::dsp::engine::Engine {
    use vactr::dsp::{arena::StoreKind, caps::CapabilitySet, ring::EngineConfig};
    let mut caps = CapabilitySet::browser();
    caps.max_voices = 1;
    let mut cfg = EngineConfig::new(&caps, 48000., 8, StoreKind::NativeArc);
    cfg.bus_seconds = 0.;
    cfg.voice_seconds = 0.;
    cfg.orbit_delay_seconds = 0.;
    cfg.bus_slots = 1;
    cfg.template_slots = 1;
    vactr::dsp::engine::Engine::with_config(cfg)
}
#[test]
fn full_garbage_and_ack_pressure_retains_exact_native_owner_until_return() {
    let mut engine = engine();
    let sample = data(1);
    let pointer = Arc::as_ptr(&sample);
    let key = lease(SongResourceKind::Sample);
    let (mut tx, mut controls) = SpscRing::split(3);
    assert!(tx
        .push(NativeRecord::SongInstall(upload(key, sample.clone())))
        .is_ok());
    assert!(tx
        .push(NativeRecord::Msg(CtlMsg::Song(SongCommand::Prepare(
            SnapshotEpoch(2)
        ))))
        .is_ok());
    let (_events, mut events) = EventRing::split(1);
    let (mut acks, mut rx) = SpscRing::split(1);
    let (mut garbage, mut returned) = SpscRing::split(1);
    let blocker = data(1);
    assert!(garbage.push(Garbage::Sample(blocker.clone())).is_ok());
    acks.push(HostMsg::CellBatchAck { seq: 77 }).unwrap();
    let mut cells = AtomicCells::new(1);
    let mut out = [0.; 16];
    let mut step = |engine: &mut vactr::dsp::engine::Engine,
                    controls: &mut ring::Consumer<NativeRecord>,
                    acks: &mut ring::AckProducer,
                    garbage: Option<&mut ring::Producer<Garbage>>| {
        engine.process(
            &mut EngineIo {
                events: &mut events,
                controls,
                acks,
                cells: &mut cells,
                garbage,
            },
            &mut out,
            8,
        )
    };
    step(&mut engine, &mut controls, &mut acks, Some(&mut garbage));
    assert_eq!(controls.len(), 1);
    assert_eq!(Arc::strong_count(&sample), 2);
    step(&mut engine, &mut controls, &mut acks, None);
    assert_eq!(controls.len(), 1);
    assert_eq!(Arc::strong_count(&sample), 2);
    assert!(matches!(returned.pop(), Some(Garbage::Sample(_))));
    assert_eq!(rx.pop(), Some(HostMsg::CellBatchAck { seq: 77 }));
    step(&mut engine, &mut controls, &mut acks, Some(&mut garbage));
    match returned.pop() {
        Some(Garbage::SongInstall(NativeSongInstall {
            lease,
            payload: NativeInstall::Sample { data, .. },
        })) => {
            assert_eq!(lease, key);
            assert_eq!(Arc::as_ptr(&data), pointer);
        }
        _ => panic!("exact retained owner"),
    }
    assert_eq!(
        rx.pop(),
        Some(HostMsg::Song(SongHostAck::Rejected {
            epoch: key.epoch,
            reason: SongRejectCode::NotReady
        }))
    );
    step(&mut engine, &mut controls, &mut acks, Some(&mut garbage));
    assert_eq!(
        rx.pop(),
        Some(HostMsg::Song(SongHostAck::Rejected {
            epoch: SnapshotEpoch(2),
            reason: SongRejectCode::NotReady
        }))
    );
    assert_eq!(engine.active_voices(), 0);
}
#[test]
fn wrong_native_kind_is_returned_as_malformed_without_adoption() {
    let mut key = lease(SongResourceKind::Instrument);
    let sample = data(1);
    assert!(!upload(key, sample.clone()).valid());
    key.kind = SongResourceKind::Sample;
    let mut wrong = upload(key, sample);
    wrong.lease.resource.generation = 0;
    assert!(!wrong.valid());
}
#[test]
fn native_full_control_queue_returns_exact_original_arc() {
    use vactr::host::native::audio::NativeAudioHost;
    let (mut host, _side) =
        NativeAudioHost::headless(48000, vactr::dsp::caps::CapabilitySet::browser(), 1);
    for _ in 0..vactr::host::native::audio::CONTROL_CAPACITY {
        host.post(CtlMsg::Song(SongCommand::Prepare(SnapshotEpoch(1))));
    }
    let sample = data(1);
    let pointer = Arc::as_ptr(&sample);
    let returned = host
        .submit_song_sample(lease(SongResourceKind::Sample), sample)
        .unwrap_err();
    assert_eq!(Arc::as_ptr(&returned), pointer);
}
fn outbox() -> Vec<Vec<u8>> {
    use crate::host::wasm::abi;
    let n = abi::outbox_len() as usize;
    // SAFETY: this thread owns the main-half outbox, which remains unchanged while copied.
    let bytes = unsafe { std::slice::from_raw_parts(abi::outbox_ptr(), n) }.to_vec();
    abi::outbox_clear();
    let mut at = 0;
    let mut records = Vec::new();
    while at < bytes.len() {
        let n = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        records.push(bytes[at..at + n].to_vec());
        at += n;
    }
    records
}
#[test]
fn browser_sender_paces_only_exact_full_key_and_rejection_unblocks_following_upload() {
    use crate::host::wasm::{
        abi,
        messages::{HostState, WasmAudioHost},
    };
    use std::{cell::RefCell, rc::Rc};
    abi::outbox_clear();
    let state = Rc::new(RefCell::new(HostState::new(1_000_000)));
    let mut host = WasmAudioHost(state.clone());
    let key = lease(SongResourceKind::Sample);
    host.submit_song_sample(key, data(crate::host::wasm::messages::SLICE_FLOATS + 1))
        .unwrap();
    let records = outbox();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0][0], 0x1d);
    assert_eq!(records[1][0], 0x1e);
    let mut inbox = ByteInbox::new();
    for b in records {
        assert!(inbox.push(&b));
    }
    let budget = Budget {
        install_bytes: usize::MAX,
        batch: true,
    };
    assert!(matches!(inbox.next(budget),Some(Record::SongSampleBegin{lease,..}) if lease==key));
    assert!(matches!(inbox.next(budget),Some(Record::SongSlice{lease,offset:0,..}) if lease==key));
    state.borrow_mut().on_msg(HostMsg::SliceOk {
        resource: key.resource.id,
        offset: 0,
    });
    assert!(outbox().is_empty());
    let mut foreign = key;
    foreign.epoch = SnapshotEpoch(3);
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::SliceAccepted {
            lease: foreign,
            offset: 0,
        }));
    assert!(outbox().is_empty());
    foreign = key;
    foreign.kind = SongResourceKind::Instrument;
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::SliceAccepted {
            lease: foreign,
            offset: 0,
        }));
    assert!(outbox().is_empty());
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::SliceAccepted {
            lease: key,
            offset: 0,
        }));
    let records = outbox();
    assert_eq!(records.len(), 1);
    assert_eq!(
        u32::from_le_bytes(records[0][18..22].try_into().unwrap()),
        crate::host::wasm::messages::SLICE_FLOATS as u32
    );
    let mut next = key;
    next.epoch = SnapshotEpoch(42);
    host.submit_song_sample(next, data(1)).unwrap();
    assert!(outbox().is_empty());
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::Rejected {
            epoch: key.epoch,
            reason: SongRejectCode::NotReady,
        }));
    let records = outbox();
    assert_eq!(records.len(), 2);
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&records[0]));
    assert!(matches!(inbox.next(budget),Some(Record::SongSampleBegin{lease,..}) if lease==next));
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::LeaseReturned(key)));
    assert!(outbox().is_empty());
}
#[test]
fn browser_duplicate_triple_queue_pressure_returns_original_sample_and_kind_receipts_are_distinct()
{
    use crate::host::wasm::messages::{HostState, WasmAudioHost};
    use std::{cell::RefCell, rc::Rc};
    let state = Rc::new(RefCell::new(HostState::new(1_000_000)));
    let mut host = WasmAudioHost(state.clone());
    let key = lease(SongResourceKind::Sample);
    let remaining = || {
        let limits = vactr::song::assets::SongAssetLimits {
            max_resources: 32,
            max_pcm_bytes: 1_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        };
        let actual = state.borrow().remaining_song_sample_limits(limits).unwrap();
        (actual.max_resources, actual.max_pcm_bytes)
    };
    assert_eq!(remaining(), (32, 1_000_000));
    host.install_sample(3, data(2));
    assert_eq!(remaining(), (31, 999_992));
    host.submit_song_sample(key, data(1)).unwrap();
    assert_eq!(remaining(), (30, 999_988));
    host.install_sample(9, data(249_998));
    assert_eq!(
        remaining(),
        (30, 999_988),
        "legacy install cannot consume reserved song PCM"
    );
    let sample = data(1);
    let pointer = Arc::as_ptr(&sample);
    let original = host.submit_song_sample(key, sample).unwrap_err();
    assert_eq!(Arc::as_ptr(&original), pointer);
    let mut wrong = key;
    wrong.kind = SongResourceKind::AnalysisBank;
    assert!(host.submit_song_sample(wrong, data(1)).is_err());
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::SliceAccepted {
            lease: key,
            offset: 0,
        }));
    assert_eq!(
        remaining(),
        (30, 999_988),
        "final slice retains both reservations"
    );
    assert!(
        host.submit_song_sample(key, data(1)).is_err(),
        "completed chunk still owns its reservation"
    );
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::LeaseReturned(wrong)));
    assert!(host.submit_song_sample(key, data(1)).is_err());
    assert_eq!(remaining(), (30, 999_988), "foreign kind cannot free quota");
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::LeaseReturned(key)));
    assert_eq!(remaining(), (31, 999_992));
    host.install_sample(9, data(249_998));
    assert_eq!(
        remaining(),
        (30, 0),
        "legacy admission succeeds after exact song return"
    );
    host.retire_sample(9);
    assert_eq!(remaining(), (31, 999_992));
    assert!(host.submit_song_sample(key, data(1)).is_ok());
    assert_eq!(remaining(), (30, 999_988));
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::PreparationCancelled(key.epoch)));
    assert_eq!(remaining(), (30, 999_988));
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::LeaseReturned(key)));
    assert_eq!(remaining(), (31, 999_992));
    state.borrow_mut().on_msg(HostMsg::Retired { resource: 3 });
    assert_eq!(remaining(), (32, 1_000_000));
}
struct Probe;
thread_local! {static CALLBACK_MEMORY:std::cell::Cell<Option<(u64,u64)>>=const {std::cell::Cell::new(None)};}
// SAFETY: ownership and allocator contracts are forwarded unchanged to System.
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let _ = CALLBACK_MEMORY.try_with(|p| {
            if let Some((a, d)) = p.get() {
                p.set(Some((a + 1, d)));
            }
        });
        // SAFETY: forwarded caller layout contract.
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        let _ = CALLBACK_MEMORY.try_with(|p| {
            if let Some((a, d)) = p.get() {
                p.set(Some((a, d + 1)));
            }
        });
        // SAFETY: same allocator receives original allocation and layout.
        unsafe { std::alloc::System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        let _ = CALLBACK_MEMORY.try_with(|p| {
            if let Some((a, d)) = p.get() {
                p.set(Some((a + 1, d + 1)));
            }
        });
        // SAFETY: forwarded original pointer/layout and requested size.
        unsafe { std::alloc::System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static GLOBAL: Probe = Probe;
#[test]
fn callback_retained_rejection_allocates_and_destroys_nothing() {
    let mut engine = engine();
    let (_tx, mut events) = EventRing::split(1);
    let (mut tx, mut controls) = SpscRing::split(2);
    let (mut acks, _rx) = SpscRing::split(1);
    let mut cells = AtomicCells::new(1);
    let mut out = [0.; 16];
    assert!(tx
        .push(NativeRecord::SongInstall(upload(
            lease(SongResourceKind::Sample),
            data(1)
        )))
        .is_ok());
    CALLBACK_MEMORY.with(|p| p.set(Some((0, 0))));
    engine.process(
        &mut EngineIo {
            events: &mut events,
            controls: &mut controls,
            acks: &mut acks,
            cells: &mut cells,
            garbage: None,
        },
        &mut out,
        8,
    );
    engine.process(
        &mut EngineIo {
            events: &mut events,
            controls: &mut controls,
            acks: &mut acks,
            cells: &mut cells,
            garbage: None,
        },
        &mut out,
        8,
    );
    let observed = CALLBACK_MEMORY.with(|p| p.replace(None));
    assert_eq!(observed, Some((0, 0)));
}
#[test]
fn all_native_graph_payload_kinds_validate_exact_lease_and_master_flag() {
    use vactr::dsp::{bus::BusTemplate, ugen::Template};
    let key = lease(SongResourceKind::Instrument);
    let inst = NativeSongInstall {
        lease: key,
        payload: NativeInstall::Inst {
            resource: key.resource.id,
            gen: key.resource.generation,
            template: Template::boxed(),
        },
    };
    assert!(inst.valid());
    for kind in [
        SongResourceKind::PrivateFx,
        SongResourceKind::Track,
        SongResourceKind::Master,
    ] {
        let key = lease(kind);
        let master = kind == SongResourceKind::Master;
        let mut bus = NativeSongInstall {
            lease: key,
            payload: NativeInstall::Bus {
                resource: key.resource.id,
                gen: key.resource.generation,
                master,
                template: Box::new(BusTemplate::new()),
            },
        };
        assert!(bus.valid());
        bus.lease.kind = SongResourceKind::Instrument;
        assert!(!bus.valid());
    }
}
#[test]
fn rejected_upload_preserves_actual_installed_legacy_voice_and_samples() {
    use vactr::dsp::{
        graph::{InstDef, InstId, UGenSpec},
        ugen::Template,
    };
    use vactr::host::wire::AudioEvent;
    use vactr::sched::slots::SlotId;
    let mut engine = engine();
    let graph = InstDef {
        id: InstId::new(3),
        params: Box::new([]),
        nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    };
    let template = Template::from_inst(&graph, &engine.build_env()).unwrap();
    let (mut tx, mut controls) = SpscRing::split(3);
    assert!(tx
        .push(NativeRecord::Install(NativeInstall::Inst {
            resource: 3,
            gen: 1,
            template
        }))
        .is_ok());
    let (mut events_tx, mut events) = EventRing::split(2);
    events_tx
        .push(AudioEvent::new(0., SlotId::new(1), 1, InstId::new(3)))
        .unwrap();
    let (mut acks, _rx) = SpscRing::split(16);
    let (mut garbage, _returned) = SpscRing::split(4);
    let mut cells = AtomicCells::new(1);
    let mut out = [0.; 16];
    engine.process(
        &mut EngineIo {
            events: &mut events,
            controls: &mut controls,
            acks: &mut acks,
            cells: &mut cells,
            garbage: Some(&mut garbage),
        },
        &mut out,
        8,
    );
    assert_eq!(engine.active_voices(), 1);
    assert!(out.iter().any(|v| v.abs() > 0.));
    assert!(tx
        .push(NativeRecord::SongInstall(upload(
            lease(SongResourceKind::Sample),
            data(1)
        )))
        .is_ok());
    engine.process(
        &mut EngineIo {
            events: &mut events,
            controls: &mut controls,
            acks: &mut acks,
            cells: &mut cells,
            garbage: None,
        },
        &mut out,
        8,
    );
    assert_eq!(engine.active_voices(), 1);
    assert!(out.iter().any(|v| v.abs() > 0.));
    assert!(engine.template(InstId::new(3)).is_some());
}
#[test]
fn browser_real_graph_upload_and_queue_capacity_preserve_full_keys() {
    use crate::host::wasm::{
        abi,
        messages::{HostState, WasmAudioHost},
    };
    use std::{cell::RefCell, rc::Rc};
    use vactr::dsp::graph::{BusDef, BusId};
    use vactr::host::caps::GraphHandle;
    abi::outbox_clear();
    let state = Rc::new(RefCell::new(HostState::new(1_000_000)));
    let mut host = WasmAudioHost(state.clone());
    let key = lease(SongResourceKind::Master);
    host.submit_song_graph(
        key,
        &GraphHandle::Master(Arc::new(BusDef {
            id: BusId::new(8),
            chain: Box::new([]),
        })),
    )
    .unwrap();
    let records = outbox();
    assert_eq!(records.len(), 1);
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&records[0]));
    assert!(
        matches!(inbox.next(Budget{install_bytes:usize::MAX,batch:true}),Some(Record::SongGraph{lease,..}) if lease==key)
    );
    let mut key = lease(SongResourceKind::Sample);
    for id in 0..ring::INBOX_SLOTS {
        key.resource.id = u32::try_from(id).unwrap();
        host.submit_song_sample(key, data(1)).unwrap();
    }
    key.resource.id = 99;
    let sample = data(1);
    let pointer = Arc::as_ptr(&sample);
    let returned = host.submit_song_sample(key, sample).unwrap_err();
    assert_eq!(Arc::as_ptr(&returned), pointer);
}
#[test]
fn graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind() {
    let mut b = [0; 64];
    let mut inbox = ByteInbox::new();
    let budget = Budget {
        install_bytes: usize::MAX,
        batch: true,
    };
    let n = ring::encode_song_graph_record(lease(SongResourceKind::Track), &[0, 1, 2], &mut b);
    for end in 1..n {
        assert!(inbox.push(&b[..end]));
        assert!(inbox.next(budget).is_none());
    }
    b[n] = 0;
    assert!(inbox.push(&b[..n + 1]));
    assert!(inbox.next(budget).is_none());
    b[17] = 7;
    assert!(inbox.push(&b[..n]));
    assert!(inbox.next(budget).is_none());
    let n = ring::encode_song_sample_begin(lease(SongResourceKind::Sample), 1, 1, 48000, &mut b);
    for end in 1..n {
        assert!(inbox.push(&b[..end]));
        assert!(inbox.next(budget).is_none());
    }
    b[n] = 0;
    assert!(inbox.push(&b[..n + 1]));
    assert!(inbox.next(budget).is_none());
    b[22] = 0;
    assert!(inbox.push(&b[..n]));
    assert!(inbox.next(budget).is_none());
}
#[test]
fn browser_full_outbox_retains_upload_and_retries_on_control_poll() {
    use crate::host::wasm::{
        abi,
        messages::{HostState, WasmAudioHost},
    };
    use std::{cell::RefCell, rc::Rc};
    abi::fix_outbox(4);
    let state = Rc::new(RefCell::new(HostState::new(1024)));
    let mut host = WasmAudioHost(state);
    let key = lease(SongResourceKind::Sample);
    let sample = data(1);
    host.submit_song_sample(key, sample.clone()).unwrap();
    assert_eq!(Arc::strong_count(&sample), 2);
    assert!(outbox().is_empty());
    abi::fix_outbox(1024);
    host.poll_msg().unwrap();
    let records = outbox();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0][0], 0x1d);
    assert_eq!(records[1][0], 0x1e);
    assert_eq!(Arc::strong_count(&sample), 2);
    abi::fix_outbox(0);
}
#[test]
fn capacity_request_and_report_preserve_full_serial_every_dimension_and_exact_lengths() {
    let epoch = SnapshotEpoch(u64::MAX);
    let request = CtlMsg::Song(SongCommand::RequestCapacity(epoch));
    let mut command = [0; CtlMsg::MAX_LEN];
    let n = request.encode(&mut command);
    assert_eq!(n, 10);
    assert_eq!(command[1], 15);
    let (mut tx, mut native) = SpscRing::split(2);
    assert!(tx.push(NativeRecord::Msg(request)).is_ok());
    let mut browser = ByteInbox::new();
    assert!(browser.push(&command[..n]));
    let budget = Budget {
        install_bytes: 0,
        batch: true,
    };
    match (native.next(budget), browser.next(budget)) {
        (Some(Record::Msg(a)), Some(Record::Msg(b))) => assert_eq!(a, b),
        _ => panic!("capacity request parity"),
    }
    for end in 0..n {
        assert!(CtlMsg::decode(&command[..end]).is_err());
    }
    let report = SongCapacityReport {
        epoch,
        serial: u64::MAX,
        available: SongHostCapacities {
            sample_rate: 48000,
            cell_slots: u32::MAX,
            voice_slots: u32::MAX - 1,
            template_slots: u32::MAX - 2,
            bus_slots: u32::MAX - 3,
            sample_resources: u32::MAX - 4,
            ack_slots: u32::MAX - 5,
            pcm_bytes: u64::MAX,
            voice_frames: u64::MAX - 1,
            bus_frames: u64::MAX - 2,
        },
        analysis: SongAnalysisCapacity { slots: u32::MAX },
    };
    let message = HostMsg::Song(SongHostAck::CapacityReport(report));
    let mut bytes = [0; HostMsg::MAX_LEN];
    let n = message.encode(&mut bytes);
    assert_eq!(n, 74);
    assert_eq!(bytes[1], 9);
    assert_eq!(SONG_ACK_MAX_LEN, 74);
    assert_eq!(HostMsg::decode(&bytes[..n]).unwrap(), (message, n));
    for end in 0..n {
        assert!(HostMsg::decode(&bytes[..end]).is_err());
    }
    assert_eq!(message.encode(&mut bytes[..73]), 0);
    let (mut ack_tx, mut ack_rx) = SpscRing::split(1);
    ack_tx.push(message).unwrap();
    assert_eq!(ack_rx.pop(), Some(HostMsg::decode(&bytes[..74]).unwrap().0));
    let mut receipts = SongReceipts::default();
    receipts.expect(epoch).unwrap();
    assert!(receipts
        .record(SongHostAck::CapacityReport(report))
        .unwrap());
    assert_eq!(receipts.pop(), Some(SongHostAck::CapacityReport(report)));
}
#[test]
fn capacity_request_is_rejected_not_ready_and_bad_frame_keeps_following_request() {
    let request = CtlMsg::Song(SongCommand::RequestCapacity(SnapshotEpoch(u64::MAX)));
    let mut b = [0; CtlMsg::MAX_LEN];
    let n = request.encode(&mut b);
    b[n] = 0;
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&b[..n + 1]));
    assert!(inbox.push(&b[..n]));
    assert!(inbox
        .next(Budget {
            install_bytes: 0,
            batch: true
        })
        .is_none());
    let mut engine = engine();
    let (_tx, mut events) = EventRing::split(1);
    let (mut acks, mut rx) = SpscRing::split(16);
    let mut cells = AtomicCells::new(1);
    let mut out = [0.; 16];
    engine.process(
        &mut EngineIo {
            events: &mut events,
            controls: &mut inbox,
            acks: &mut acks,
            cells: &mut cells,
            garbage: None,
        },
        &mut out,
        8,
    );
    assert_eq!(
        rx.pop(),
        Some(HostMsg::Song(SongHostAck::CapacityRejected {
            epoch: SnapshotEpoch(u64::MAX),
            reason: SongRejectCode::NotReady
        }))
    );
    assert_eq!(engine.active_voices(), 0);
}

#[test]
fn capacity_rejected_preserves_full_identity_reason_and_following_frame() {
    for reason in [
        SongRejectCode::StaleEpoch,
        SongRejectCode::NotReady,
        SongRejectCode::Capacity,
        SongRejectCode::Malformed,
        SongRejectCode::HostFault,
    ] {
        let ack = SongHostAck::CapacityRejected {
            epoch: SnapshotEpoch(u64::MAX),
            reason,
        };
        assert_eq!(ack.epoch(), SnapshotEpoch(u64::MAX));
        let msg = HostMsg::Song(ack);
        let mut bytes = [0; HostMsg::MAX_LEN];
        let n = msg.encode(&mut bytes);
        assert_eq!(n, 11);
        assert_eq!(&bytes[..2], &[0x49, 10]);
        assert_eq!(&bytes[2..10], &u64::MAX.to_le_bytes());
        assert_eq!(bytes[10], reason as u8);
        assert_eq!(HostMsg::decode(&bytes[..n]).unwrap(), (msg, n));
        for end in 0..n {
            assert!(HostMsg::decode(&bytes[..end]).is_err());
        }
        assert_eq!(msg.encode(&mut [0; 10]), 0);
        for bad in [5, 9, 255] {
            bytes[10] = bad;
            assert!(HostMsg::decode(&bytes[..n]).is_err());
        }
        bytes[10] = reason as u8;
        let next = HostMsg::Song(SongHostAck::Rejected {
            epoch: SnapshotEpoch(u64::MAX),
            reason,
        });
        let tail = next.encode(&mut bytes[n..]);
        assert_eq!(bytes[n + 1], 3);
        let (first, consumed) = HostMsg::decode(&bytes[..n + tail]).unwrap();
        assert_eq!((first, consumed), (msg, n));
        assert_eq!(
            HostMsg::decode(&bytes[consumed..n + tail]).unwrap(),
            (next, tail)
        );
        let (mut tx, mut rx) = SpscRing::split(2);
        tx.push(msg).unwrap();
        tx.push(next).unwrap();
        assert_eq!(rx.pop(), Some(first));
        assert_eq!(rx.pop(), Some(next));
    }
}
