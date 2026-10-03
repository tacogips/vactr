//! Checked command admission uses the real bounded native and browser transports.
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
    cells::CellId,
    graph::InstId,
    ring::{Budget, ByteInbox, ControlSource, Record},
};
use vactr::host::{
    caps::{AudioHost, SampleData, SongSubmitError},
    wire::{AudioEvent, CtlMsg, HostMsg},
};
use vactr::sched::slots::SlotId;
use vactr::song::{routing::*, SnapshotEpoch};
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
fn commands() -> Vec<SongCommand> {
    let epoch = SnapshotEpoch(u64::MAX);
    let resource = lease(SongResourceKind::Instrument).resource;
    let mut event = AudioEvent::new(
        0.125,
        SlotId::new(u32::MAX),
        u32::MAX,
        InstId::new(u32::MAX),
    );
    event
        .push_ctl(
            crate::sched::slots::CtlId::new(u16::MAX),
            crate::host::wire::Ctl::Const(-0.0),
        )
        .unwrap();
    event
        .push_ctl(
            crate::sched::slots::CtlId::new(1),
            crate::host::wire::Ctl::Cell(CellId::new(u32::MAX)),
        )
        .unwrap();
    let preparation = SongPreparation {
        epoch,
        branches: 1,
        resources: 1,
        required: SongHostCapacities {
            sample_rate: 48000,
            ..SongHostCapacities::default()
        },
    };
    vec![
        SongCommand::RequestCapacity(epoch),
        SongCommand::BeginStaging(SongStagePreparation {
            preparation,
            analysis_required: SongAnalysisCapacity { slots: 1 },
        }),
        SongCommand::InitCells(SongCellInit {
            lease: lease(SongResourceKind::ControlCells),
            cell: CellId::new(0),
            value: 0.25,
        }),
        SongCommand::ReserveAnalysis(SongAnalysisReservation {
            lease: lease(SongResourceKind::AnalysisBank),
            slots: 1,
        }),
        SongCommand::CancelPreparation(epoch),
        SongCommand::CancelLease(lease(SongResourceKind::Sample)),
        SongCommand::BeginPreparation(preparation),
        SongCommand::ReserveResource(SongResourceReservation {
            epoch,
            resource,
            kind: SongResourceKind::Instrument,
        }),
        SongCommand::ConfigureBranch(SongBranchConfig {
            epoch,
            branch: SongBranchId(2),
            generation: 3,
            family: 4,
            track: 5,
            instrument: resource,
            private_fx: Some(resource),
            track_template: None,
            master: Some(resource),
            transition_frame: u64::MAX - 1,
            tail_deadline: u64::MAX,
        }),
        SongCommand::SealPreparation(epoch),
        SongCommand::Prepare(epoch),
        SongCommand::Activate(SongActivation {
            epoch,
            frame: u64::MAX,
        }),
        SongCommand::Mute(SongMute {
            epoch,
            instrument: 5,
            muted: true,
            frame: u64::MAX,
        }),
        SongCommand::Endpoints(SongEndpoints {
            epoch,
            arrangement: u64::MAX - 1,
            tail_deadline: u64::MAX,
        }),
        SongCommand::Release(SongBranchRelease {
            epoch,
            branch: SongBranchId(2),
            generation: 3,
            frame: u64::MAX - 1,
            tail_deadline: u64::MAX,
        }),
        SongCommand::Event(SongAudioEvent {
            epoch,
            branch: SongBranchId(2),
            generation: 3,
            frame: u64::MAX,
            event,
        }),
        SongCommand::BindGraphBanks(SongGraphBanks {
            graph: lease(SongResourceKind::Instrument),
            controls: Some(lease(SongResourceKind::ControlCells)),
            analysis: Some(lease(SongResourceKind::AnalysisBank)),
        }),
    ]
}
fn take_records() -> Vec<Vec<u8>> {
    let n = browser_abi::outbox_len() as usize;
    // The actual TLS outbox remains unchanged while this test copies it.
    let bytes = unsafe { std::slice::from_raw_parts(browser_abi::outbox_ptr(), n) }.to_vec();
    browser_abi::outbox_clear();
    let mut records = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let len = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        records.push(bytes[at..at + len].to_vec());
        at += len;
    }
    records
}
#[test]
fn browser_all_commands_refuse_complete_records_and_retry_once() {
    let mut host = WasmAudioHost(Rc::new(RefCell::new(HostState::new(100000))));
    let all = commands();
    assert_eq!(all.len(), 17);
    for command in all {
        let mut encoded = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::Song(command).encode(&mut encoded);
        assert!(n > 0);
        browser_abi::fix_outbox(n + 4);
        assert!(browser_abi::push_record(&encoded[..n]));
        let refusal = host.try_song_command(command).unwrap_err();
        assert_eq!(refusal.command, command);
        assert!(matches!(refusal.error, SongSubmitError::Backpressure));
        assert_eq!(take_records(), vec![encoded[..n].to_vec()]);
        host.try_song_command(refusal.command).unwrap();
        let records = take_records();
        assert_eq!(records, vec![encoded[..n].to_vec()]);
        let mut inbox = ByteInbox::new();
        assert!(inbox.push(&records[0]));
        let budget = Budget {
            install_bytes: usize::MAX,
            batch: true,
        };
        match inbox.next(budget) {
            Some(Record::Msg(message)) => {
                let mut actual = [0; CtlMsg::MAX_LEN];
                let len = message.encode(&mut actual);
                assert_eq!(&actual[..len], &encoded[..n]);
            }
            _ => panic!("complete command"),
        }
        assert!(inbox.next(budget).is_none());
    }
    browser_abi::fix_outbox(0);
}
#[test]
fn unsupported_and_invalid_commands_consume_no_transport() {
    let mut noop = vactr::host::noop::NoopHost;
    for command in commands() {
        let refusal = noop.try_song_command(command).unwrap_err();
        assert_eq!(refusal.command, command);
        assert!(matches!(refusal.error, SongSubmitError::Unavailable));
    }
    browser_abi::fix_outbox(1024);
    let mut host = WasmAudioHost(Rc::new(RefCell::new(HostState::new(100000))));
    let command = SongCommand::Endpoints(SongEndpoints {
        epoch: SnapshotEpoch(1),
        arrangement: 2,
        tail_deadline: 1,
    });
    let refusal = host.try_song_command(command).unwrap_err();
    assert_eq!(refusal.command, command);
    assert!(matches!(refusal.error, SongSubmitError::Invalid(_)));
    assert!(take_records().is_empty());
    browser_abi::fix_outbox(0);
}
#[test]
fn browser_partial_sample_pressure_preserves_full_key_progress() {
    let state = Rc::new(RefCell::new(HostState::new(1000000)));
    let mut host = WasmAudioHost(state.clone());
    let key = lease(SongResourceKind::Sample);
    let data = Arc::new(SampleData {
        rate: 48000,
        channels: 1,
        frames: vec![0.25; browser_messages::SLICE_FLOATS + 1].into_boxed_slice(),
    });
    browser_abi::fix_outbox(31); // Begin fits; the first slice cannot be partially written.
    host.submit_song_sample(key, data).unwrap();
    let begin = take_records();
    assert_eq!(begin.len(), 1);
    assert_eq!(begin[0][0], 0x1d);
    host.poll_msg().unwrap();
    assert!(take_records().is_empty());
    browser_abi::fix_outbox(100000);
    host.poll_msg().unwrap();
    let first = take_records();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0][0], 0x1e);
    browser_abi::fix_outbox(1);
    let mut foreign = key;
    foreign.epoch = SnapshotEpoch(1);
    for ack in [
        HostMsg::SliceOk {
            resource: key.resource.id,
            offset: 0,
        },
        HostMsg::Song(SongHostAck::SliceAccepted {
            lease: foreign,
            offset: 0,
        }),
        HostMsg::Song(SongHostAck::SliceAccepted {
            lease: key,
            offset: 1,
        }),
        HostMsg::Song(SongHostAck::LeaseReturned(foreign)),
    ] {
        state.borrow_mut().on_msg(ack);
        assert!(take_records().is_empty());
    }
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::SliceAccepted {
            lease: key,
            offset: 0,
        }));
    assert!(take_records().is_empty());
    browser_abi::fix_outbox(100000);
    host.poll_msg().unwrap();
    let final_slice = take_records();
    assert_eq!(final_slice.len(), 1);
    assert_eq!(
        u32::from_le_bytes(final_slice[0][18..22].try_into().unwrap()),
        u32::try_from(browser_messages::SLICE_FLOATS).unwrap()
    );
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::SliceAccepted {
            lease: key,
            offset: 0,
        }));
    assert!(take_records().is_empty());
    state
        .borrow_mut()
        .on_msg(HostMsg::Song(SongHostAck::LeaseReturned(key)));
    let replacement = Arc::new(SampleData {
        rate: 48000,
        channels: 1,
        frames: vec![0.5].into_boxed_slice(),
    });
    host.submit_song_sample(key, replacement).unwrap();
    assert_eq!(take_records().len(), 2);
    browser_abi::fix_outbox(0);
}

#[test]
fn browser_borrowed_graph_refusal_preserves_allocation() {
    use vactr::dsp::graph::{BusDef, BusId};
    use vactr::host::caps::GraphHandle;
    let def = Arc::new(BusDef {
        id: BusId::new(7),
        chain: Vec::new().into_boxed_slice(),
    });
    let pointer = Arc::as_ptr(&def);
    let graph = GraphHandle::Bus { id: def.id, def };
    let mut host = WasmAudioHost(Rc::new(RefCell::new(HostState::new(100000))));
    browser_abi::fix_outbox(1);
    assert!(host
        .submit_song_graph(lease(SongResourceKind::Track), &graph)
        .is_err());
    assert!(take_records().is_empty());
    match &graph {
        GraphHandle::Bus { def, .. } => assert_eq!(Arc::as_ptr(def), pointer),
        _ => unreachable!(),
    }
    browser_abi::fix_outbox(100000);
    host.submit_song_graph(lease(SongResourceKind::Track), &graph)
        .unwrap();
    let records = take_records();
    assert_eq!(records.len(), 1);
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&records[0]));
    let budget = Budget {
        install_bytes: usize::MAX,
        batch: true,
    };
    assert!(
        matches!(inbox.next(budget), Some(Record::SongGraph { lease: key, .. }) if key == lease(SongResourceKind::Track))
    );
    assert!(inbox.next(budget).is_none());
    browser_abi::fix_outbox(0);
}

#[test]
fn browser_sample_pacing_consumes_real_engine_receipts_and_returns_lease() {
    use vactr::dsp::{
        arena::StoreKind,
        caps::CapabilitySet,
        cells::AtomicCells,
        engine::{Engine, EngineConfig, SongStagingConfig},
        ring::{EngineIo, EventRing, SpscRing},
    };
    browser_abi::fix_outbox(100000);
    let state = Rc::new(RefCell::new(HostState::new(1000000)));
    let mut host = WasmAudioHost(state.clone());
    let mut caps = CapabilitySet::browser();
    caps.max_voices = 1;
    let mut config = EngineConfig::new(&caps, 48000., 8, StoreKind::Arena { bytes: 1_000_000 });
    config.voice_seconds = 0.;
    config.bus_seconds = 0.;
    config.orbit_delay_seconds = 0.;
    config.template_slots = 1;
    config.bus_slots = 1;
    let mut engine = Engine::with_config(config);
    let (mut acks, mut received) = SpscRing::split(8);
    engine
        .configure_song_staging_for_transport(
            SongStagingConfig {
                preparations: 2,
                leases: 4,
                branches: 1,
                control_slots: 1,
                analysis_slots: 0,
                native_pcm_bytes: 1000000,
                critical_receipts: 2,
            },
            &acks,
        )
        .unwrap();
    let limits = song::assets::SongAssetLimits {
        max_resources: 256,
        max_pcm_bytes: 1000000,
        max_source_files: 64,
        max_source_bytes: 1000000,
        max_banks: 64,
        max_walk_nodes: 100000,
        max_walk_depth: 256,
    };
    let epoch = SnapshotEpoch(9);
    let key = SongLeaseKey {
        epoch,
        resource: SongResourceRef {
            id: 7,
            generation: u32::MAX,
        },
        kind: SongResourceKind::Sample,
    };
    host.try_song_command(SongCommand::BeginStaging(SongStagePreparation {
        preparation: SongPreparation {
            epoch,
            branches: 0,
            resources: 1,
            required: SongHostCapacities {
                sample_rate: 48000,
                ..SongHostCapacities::default()
            },
        },
        analysis_required: SongAnalysisCapacity { slots: 0 },
    }))
    .unwrap();
    host.try_song_command(SongCommand::ReserveResource(SongResourceReservation {
        epoch,
        resource: key.resource,
        kind: key.kind,
    }))
    .unwrap();
    let data = Arc::new(SampleData {
        channels: 1,
        rate: 48000,
        frames: vec![0.25; browser_messages::SLICE_FLOATS + 1].into_boxed_slice(),
    });
    host.submit_song_sample(key, data).unwrap();
    let mut inbox = ByteInbox::new();
    let (_events, mut events) = EventRing::split(1);
    let mut cells = AtomicCells::new(1);
    let mut seen = Vec::new();
    let mut out = [0.; 16];
    for _ in 0..32 {
        for record in take_records() {
            assert!(inbox.push(&record));
        }
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
        assert!(out.iter().all(|v| *v == 0.));
        while let Some(message) = received.pop() {
            if let HostMsg::Song(ack) = message {
                seen.push(ack);
            }
            state.borrow_mut().on_msg(message);
        }
        if seen.contains(&SongHostAck::ResourceReady {
            epoch,
            resource: key.resource,
        }) {
            break;
        }
    }
    assert_eq!(
        seen.iter()
            .filter_map(|ack| match ack {
                SongHostAck::SliceAccepted { lease, offset } if *lease == key => Some(*offset),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![0, u32::try_from(browser_messages::SLICE_FLOATS).unwrap()]
    );
    assert!(seen.contains(&SongHostAck::ResourceReady {
        epoch,
        resource: key.resource
    }));
    assert_eq!(
        state
            .borrow()
            .remaining_song_sample_limits(limits)
            .unwrap()
            .max_pcm_bytes,
        1000000 - u64::try_from((browser_messages::SLICE_FLOATS + 1) * 4).unwrap()
    );
    host.try_song_command(SongCommand::CancelLease(key))
        .unwrap();
    for _ in 0..8 {
        for record in take_records() {
            assert!(inbox.push(&record));
        }
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
        while let Some(message) = received.pop() {
            if let HostMsg::Song(ack) = message {
                seen.push(ack);
            }
            state.borrow_mut().on_msg(message);
        }
        if seen.contains(&SongHostAck::LeaseReturned(key)) {
            break;
        }
    }
    assert_eq!(
        seen.iter()
            .filter(|ack| **ack == SongHostAck::LeaseReturned(key))
            .count(),
        1
    );
    assert_eq!(
        state
            .borrow()
            .remaining_song_sample_limits(limits)
            .unwrap()
            .max_pcm_bytes,
        1000000
    );
    browser_abi::fix_outbox(0);
}

#[test]
fn configured_native_reports_actual_free_capacity_and_renders_existing_quantum() {
    use vactr::dsp::{arena::StoreKind, caps::CapabilitySet, engine::EngineConfig};
    use vactr::host::native::audio::{NativeAudioHost, MAX_BLOCK};
    let mut caps = CapabilitySet::native();
    caps.max_voices = 2;
    let mut config = EngineConfig::new(&caps, 32768., MAX_BLOCK, StoreKind::NativeArc);
    config.bus_slots = 12;
    let (mut host, mut side) = NativeAudioHost::headless_with_config(config, 64).unwrap();
    let epoch = SnapshotEpoch(420);
    host.try_song_command(SongCommand::RequestCapacity(epoch))
        .unwrap();
    let mut report = None;
    for _ in 0..16 {
        side.render(&mut [0.; 32], 2);
        while let Some(message) = host.poll_msg().unwrap() {
            if let HostMsg::Song(SongHostAck::CapacityReport(actual)) = message {
                assert_eq!(actual.epoch, epoch);
                assert!(actual.serial > 0);
                report = Some(actual);
            }
        }
        if report.is_some() {
            break;
        }
    }
    let actual = report.expect("actual configured Engine observation");
    assert_eq!(actual.available.bus_slots, 11);
    assert_eq!(actual.available.sample_rate, 32768);
    assert_eq!(actual.available.cell_slots, 64);
    assert_eq!(actual.available.voice_slots, 2);
    assert!(
        actual.available.bus_slots >= 8,
        "both complete old/new cohorts fit"
    );
    let mut longer = vec![0.; 4 * MAX_BLOCK];
    side.render(&mut longer, 2);
    assert!(longer.iter().all(|sample| *sample == 0.));
}

#[test]
fn configured_native_rejects_storage_rate_and_incompatible_quantum() {
    use vactr::dsp::{arena::StoreKind, caps::CapabilitySet, engine::EngineConfig};
    use vactr::host::native::audio::{NativeAudioHost, MAX_BLOCK};
    let mut caps = CapabilitySet::native();
    caps.max_voices = 2;
    let base = EngineConfig::new(&caps, 32768., MAX_BLOCK, StoreKind::NativeArc);
    let mut storage = base;
    storage.store = StoreKind::Arena { bytes: 1_000_000 };
    let mut fractional = base;
    fractional.sample_rate = 32768.5;
    let mut unsupported = base;
    unsupported.sample_rate = 7999.;
    let mut quantum = base;
    quantum.max_block = MAX_BLOCK - 1;
    let mut invalid = base;
    invalid.voice_seconds = f32::INFINITY;
    for config in [storage, fractional, unsupported, quantum, invalid] {
        let failure = NativeAudioHost::headless_with_config(config, 64)
            .err()
            .expect("invalid config must not construct a host");
        assert_eq!(failure.code, vactr::vm::fail::FailCode::Type);
    }
}
