//! Actual measured resource adoption remains silent until full09 activation.
use std::sync::Arc;
use vactr::dsp::arena::{FaultCode, InstallRequest, ResourceKind, SampleStore, StoreKind};
use vactr::dsp::bus::{BusTemplate, SlotState};
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::cells::{AtomicCells, CellId};
use vactr::dsp::engine::{Engine, EngineConfig, SongStagingConfig};
use vactr::dsp::graph::{AnalyzerKind, EffectKind};
use vactr::dsp::ring::{
    self, ByteInbox, EngineIo, EventRing, Garbage, NativeInstall, NativeRecord, NativeSongInstall,
    SpscRing,
};
use vactr::host::caps::{AudioHost, SampleData};
use vactr::host::wire::{Ctl, CtlMsg, HostMsg};
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;

fn key(epoch: u64, id: u32, kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(epoch),
        resource: SongResourceRef {
            id,
            generation: u32::MAX,
        },
        kind,
    }
}
fn sample(n: usize) -> Arc<SampleData> {
    Arc::new(SampleData {
        rate: 48_000,
        channels: 1,
        frames: vec![0.5; n].into_boxed_slice(),
    })
}
fn request(k: SongLeaseKey, n: usize) -> InstallRequest {
    InstallRequest {
        resource: k.resource.id,
        kind: ResourceKind::Sample,
        frames: n,
        channels: 1,
        rate: 48_000,
        bytes: 0,
        origin: None,
    }
}
fn preparation(engine: &Engine, epoch: u64, resources: u32) -> SongStagePreparation {
    let rate = engine.song_remaining_capacities().unwrap().sample_rate;
    SongStagePreparation {
        preparation: SongPreparation {
            epoch: SnapshotEpoch(epoch),
            branches: 0,
            resources,
            required: SongHostCapacities {
                sample_rate: rate,
                ..SongHostCapacities::default()
            },
        },
        analysis_required: SongAnalysisCapacity { slots: 0 },
    }
}
fn reserve(engine: &mut Engine, k: SongLeaseKey) {
    engine
        .reserve_song_resource(SongResourceReservation {
            epoch: k.epoch,
            resource: k.resource,
            kind: k.kind,
        })
        .unwrap();
}
struct Rig {
    engine: Engine,
    tx: ring::Producer<NativeRecord>,
    controls: ring::Consumer<NativeRecord>,
    events: ring::EventConsumer,
    events_tx: ring::EventProducer,
    acks: ring::AckProducer,
    received: ring::AckConsumer,
    garbage: ring::Producer<Garbage>,
    returned: ring::Consumer<Garbage>,
    cells: AtomicCells,
}
impl Rig {
    fn new(store: StoreKind) -> Self {
        Self::with_bus_slots(store, 4)
    }
    fn with_bus_slots(store: StoreKind, bus_slots: usize) -> Self {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 1;
        let mut cfg = EngineConfig::new(&caps, 48_000., 16, store);
        cfg.template_slots = 3;
        cfg.bus_slots = bus_slots;
        cfg.voice_seconds = 0.1;
        cfg.bus_seconds = 0.1;
        let mut engine = Engine::with_config(cfg);
        let (acks, received) = SpscRing::split(4);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 4,
                    leases: 16,
                    branches: 4,
                    control_slots: 8,
                    analysis_slots: 131_072,
                    native_pcm_bytes: 4096,
                    critical_receipts: 2,
                },
                &acks,
            )
            .unwrap();
        let (tx, controls) = SpscRing::split(32);
        let (events_tx, events) = EventRing::split(2);
        let (garbage, returned) = SpscRing::split(1);
        Self {
            engine,
            tx,
            controls,
            events,
            events_tx,
            acks,
            received,
            garbage,
            returned,
            cells: AtomicCells::new(8),
        }
    }
    fn command(&mut self, command: SongCommand) {
        self.tx
            .push(NativeRecord::Msg(CtlMsg::Song(command)))
            .ok()
            .unwrap();
    }
    fn step(&mut self) -> [f32; 32] {
        let mut output = [0.; 32];
        self.engine.process(
            &mut EngineIo {
                events: &mut self.events,
                controls: &mut self.controls,
                acks: &mut self.acks,
                cells: &mut self.cells,
                garbage: Some(&mut self.garbage),
            },
            &mut output,
            16,
        );
        output
    }
    fn acks(&mut self) -> Vec<SongHostAck> {
        std::iter::from_fn(|| self.received.pop())
            .filter_map(|msg| {
                if let HostMsg::Song(ack) = msg {
                    Some(ack)
                } else {
                    None
                }
            })
            .collect()
    }
}
#[test]
fn measured_regions_and_unconfigured_rejection_are_real() {
    let rig = Rig::new(StoreKind::Arena { bytes: 64 });
    let actual = rig.engine.song_remaining_capacities().unwrap();
    assert_eq!(actual.bus_slots, 3); // default master occupies one slot
    assert_eq!(actual.ack_slots, 4);
    assert_eq!(actual.pcm_bytes, 64);
    assert_eq!(actual.voice_frames, 4800);
    assert_eq!(rig.engine.song_voice_regions()[0].frames, 4800);
    assert_eq!(rig.engine.song_pcm_extents()[0].frames, 16);
    let caps = CapabilitySet::browser();
    let mut engine = Engine::new(&caps, 48_000., 16, StoreKind::Arena { bytes: 64 });
    assert_eq!(
        engine.song_remaining_capacities(),
        Err(SongRejectCode::NotReady)
    );
    assert_eq!(
        engine.configure_song_staging(SongStagingConfig {
            preparations: 1,
            leases: 1,
            branches: 1,
            control_slots: 1,
            analysis_slots: 1,
            native_pcm_bytes: 0,
            critical_receipts: 1
        }),
        Err(SongRejectCode::NotReady)
    );
}
#[test]
fn native_sample_validation_is_incremental_and_completion_stays_silent() {
    let mut rig = Rig::new(StoreKind::NativeArc);
    let k = key(u64::MAX, u32::MAX, SongResourceKind::Sample);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, k.epoch.0, 1))
        .unwrap();
    reserve(&mut rig.engine, k);
    let data = sample(32);
    let pointer = Arc::as_ptr(&data);
    rig.tx
        .push(NativeRecord::SongInstall(NativeSongInstall {
            lease: k,
            payload: NativeInstall::Sample {
                resource: k.resource.id,
                gen: k.resource.generation,
                data: Arc::clone(&data),
            },
        }))
        .ok()
        .unwrap();
    assert!(rig.step().iter().all(|x| *x == 0.));
    assert!(rig.acks().is_empty());
    assert!(rig.step().iter().all(|x| *x == 0.));
    assert!(rig.acks().contains(&SongHostAck::ResourceReady {
        epoch: k.epoch,
        resource: k.resource
    }));
    assert_eq!(
        rig.engine.song_remaining_capacities().unwrap().pcm_bytes,
        4096 - 128
    );
    assert!(rig.engine.store().get(k.resource.id).is_none());
    rig.engine.seal_song_preparation(k.epoch).unwrap();
    assert!(
        rig.engine.now() > 0.,
        "two real callbacks advanced the clock"
    );
    rig.command(SongCommand::Activate(SongActivation {
        epoch: k.epoch,
        frame: 0,
    }));
    assert!(rig.step().iter().all(|x| *x == 0.));
    let receipts = rig.acks();
    assert!(receipts.contains(&SongHostAck::ActivationRejected {
        activation: SongActivation {
            epoch: k.epoch,
            frame: 0
        },
        reason: SongRejectCode::Malformed
    }));
    assert!(!receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::Applied(_))));
    rig.engine.cancel_song_preparation(k.epoch).unwrap();
    rig.step();
    let Some(Garbage::SongInstall(returned)) = rig.returned.pop() else {
        panic!("owned sample must return off-thread")
    };
    let NativeInstall::Sample { data: returned, .. } = returned.payload else {
        panic!("sample owner")
    };
    assert_eq!(Arc::as_ptr(&returned), pointer);
    rig.acks();
    rig.step();
    assert_eq!(
        rig.engine.song_remaining_capacities().unwrap().pcm_bytes,
        4096
    );
}
#[test]
fn sample_extents_remain_exact_under_legacy_fragmentation_and_foreign_cancel() {
    let mut store = SampleStore::new(StoreKind::Arena { bytes: 64 });
    store.begin(1, 1, 4, 1, 48_000).unwrap();
    store.begin(2, 1, 4, 1, 48_000).unwrap();
    assert!(store.retire(1));
    store.release(1).unwrap();
    assert_eq!(
        store
            .song_free_extents()
            .iter()
            .map(|r| r.frames)
            .collect::<Vec<_>>(),
        vec![4, 8]
    );
    let k = key(3, 2, SongResourceKind::Sample);
    store.song_reserve(k, request(k, 8)).unwrap();
    assert_eq!(store.song_free_extents()[0].frames, 4);
    let mut wrong = request(k, 8);
    wrong.kind = ResourceKind::Table;
    assert_eq!(store.song_begin(k, wrong), Err(FaultCode::BadResource));
    store.song_begin(k, request(k, 8)).unwrap();
    let bytes: Vec<_> = [0.5_f32; 8]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    assert!(store.song_write_slice(k, 0, &bytes).unwrap());
    assert!(store.get(k.resource.id).is_none()); // legacy id2 is still Installing, no song activation
    let foreign = key(4, 2, SongResourceKind::Sample);
    assert!(store.song_cancel(foreign).is_err());
    assert_eq!(store.song_free_extents()[0].frames, 4);
    store.song_cancel(k).unwrap();
    assert_eq!(
        store
            .song_free_extents()
            .iter()
            .map(|r| r.frames)
            .collect::<Vec<_>>(),
        vec![4, 8]
    );
}
#[test]
fn sequential_upload_rejects_gaps_overlap_nan_and_same_id_epochs_coexist() {
    let mut store = SampleStore::new(StoreKind::Arena { bytes: 64 });
    let a = key(1, 7, SongResourceKind::Sample);
    let b = key(2, 7, SongResourceKind::Sample);
    store.song_begin(a, request(a, 4)).unwrap();
    store.song_begin(b, request(b, 4)).unwrap();
    let bytes = 1_f32.to_le_bytes();
    assert_eq!(
        store.song_write_slice(a, 1, &bytes),
        Err(FaultCode::BadRecord)
    );
    assert_eq!(
        store.song_write_slice(a, 0, &f32::NAN.to_le_bytes()),
        Err(FaultCode::BadRecord)
    );
    assert!(!store.song_write_slice(a, 0, &bytes).unwrap());
    assert_eq!(
        store.song_write_slice(a, 0, &bytes),
        Err(FaultCode::BadRecord)
    );
    let rest: Vec<_> = [1_f32; 3].into_iter().flat_map(f32::to_le_bytes).collect();
    assert!(store.song_write_slice(a, 1, &rest).unwrap());
    store.song_cancel(a).unwrap();
    assert!(!store.song_write_slice(b, 0, &bytes).unwrap());
}
#[test]
fn explicitly_bound_control_banks_ignore_late_legacy_writes() {
    let mut rig = Rig::new(StoreKind::NativeArc);
    for epoch in [1, 2] {
        let control = key(epoch, 10, SongResourceKind::ControlCells);
        let graph = key(epoch, 11, SongResourceKind::Track);
        rig.engine
            .begin_song_preparation(preparation(&rig.engine, epoch, 2))
            .unwrap();
        reserve(&mut rig.engine, control);
        reserve(&mut rig.engine, graph);
        rig.command(SongCommand::InitCells(SongCellInit {
            lease: control,
            cell: CellId::new(42),
            value: epoch as f32 * 0.25,
        }));
        rig.command(SongCommand::BindGraphBanks(SongGraphBanks {
            graph,
            controls: Some(control),
            analysis: None,
        }));
        rig.step();
        rig.acks();
        let mut template = Box::new(BusTemplate::new());
        let index = template.push(EffectKind::Gain).unwrap();
        template.push_param(
            index,
            vactr::dsp::effects::param_ctl(EffectKind::Gain, "gain").unwrap(),
            Ctl::Cell(CellId::new(42)),
        );
        assert!(rig
            .engine
            .stage_song_native(NativeSongInstall {
                lease: graph,
                payload: NativeInstall::Bus {
                    resource: graph.resource.id,
                    gen: graph.resource.generation,
                    master: false,
                    template
                }
            })
            .is_ok());
        assert!(rig
            .engine
            .buses()
            .find(vactr::dsp::graph::BusId::new(0))
            .is_none());
    }
    let first = key(1, 10, SongResourceKind::ControlCells);
    let second = key(2, 10, SongResourceKind::ControlCells);
    let a = rig
        .engine
        .song_control_cell(first, CellId::new(42))
        .unwrap();
    let b = rig
        .engine
        .song_control_cell(second, CellId::new(42))
        .unwrap();
    assert_ne!(a, b);
    rig.cells.set(a, 99.);
    rig.cells.set(b, -99.);
    assert_eq!(
        rig.engine
            .song_control_value(first, CellId::new(42))
            .unwrap(),
        0.25
    );
    assert_eq!(
        rig.engine
            .song_control_value(second, CellId::new(42))
            .unwrap(),
        0.5
    );
    assert_eq!(rig.engine.buses().master(), 0);
    assert_eq!(
        rig.engine
            .buses()
            .slots
            .iter()
            .filter(|s| s.state == SlotState::Staged)
            .count(),
        2
    );
    assert!(rig.step().iter().all(|x| *x == 0.));
}
#[test]
fn analyzer_banks_preserve_overlapping_logical_zero_in_separate_large_banks() {
    let mut rig = Rig::new(StoreKind::NativeArc);
    for epoch in [1, 2] {
        let bank = key(epoch, 20, SongResourceKind::AnalysisBank);
        let graph = key(epoch, 21, SongResourceKind::Track);
        rig.engine
            .begin_song_preparation(preparation(&rig.engine, epoch, 2))
            .unwrap();
        reserve(&mut rig.engine, bank);
        reserve(&mut rig.engine, graph);
        rig.command(SongCommand::ReserveAnalysis(SongAnalysisReservation {
            lease: bank,
            slots: 65_536,
        }));
        rig.command(SongCommand::BindGraphBanks(SongGraphBanks {
            graph,
            controls: None,
            analysis: Some(bank),
        }));
        rig.step();
        rig.acks();
        let mut template = Box::new(BusTemplate::new());
        template
            .push(EffectKind::Analyzer(AnalyzerKind::Level))
            .unwrap();
        template
            .push(EffectKind::Analyzer(AnalyzerKind::Level))
            .unwrap();
        assert!(rig
            .engine
            .stage_song_native(NativeSongInstall {
                lease: graph,
                payload: NativeInstall::Bus {
                    resource: graph.resource.id,
                    gen: graph.resource.generation,
                    master: false,
                    template
                }
            })
            .is_ok());
    }
    let first = rig
        .engine
        .song_analysis_region(key(1, 20, SongResourceKind::AnalysisBank))
        .unwrap();
    let second = rig
        .engine
        .song_analysis_region(key(2, 20, SongResourceKind::AnalysisBank))
        .unwrap();
    assert_eq!(first.frames, 65_536);
    assert_eq!(second.frames, 65_536);
    assert_eq!(second.offset, 65_536);
    assert_eq!(rig.engine.song_analysis_capacity().unwrap().slots, 0);
}

#[test]
fn real_byte_inbox_adopts_sample_without_legacy_visibility_and_seals_only_after_upload() {
    let mut rig = Rig::new(StoreKind::Arena { bytes: 64 });
    let k = key(u64::MAX, u32::MAX, SongResourceKind::Sample);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, k.epoch.0, 1))
        .unwrap();
    reserve(&mut rig.engine, k);
    assert_eq!(
        rig.engine.seal_song_preparation(k.epoch),
        Err(SongRejectCode::NotReady)
    );
    let mut bytes = [0_u8; 256];
    let mut inbox = ByteInbox::new();
    let n = ring::encode_song_sample_begin(k, 2, 1, 48_000, &mut bytes);
    assert!(inbox.push(&bytes[..n]));
    let n = ring::encode_song_slice(k, 0, &[0.25, 0.5], &mut bytes);
    assert!(inbox.push(&bytes[..n]));
    let mut out = [0.; 32];
    rig.engine.process(
        &mut EngineIo {
            events: &mut rig.events,
            controls: &mut inbox,
            acks: &mut rig.acks,
            cells: &mut rig.cells,
            garbage: Some(&mut rig.garbage),
        },
        &mut out,
        16,
    );
    assert!(out.iter().all(|x| *x == 0.));
    assert!(rig.acks().contains(&SongHostAck::SliceAccepted {
        lease: k,
        offset: 0
    }));
    assert_eq!(
        rig.engine.song_remaining_capacities().unwrap().pcm_bytes,
        56
    );
    assert_eq!(rig.engine.song_pcm_extents()[0].offset, 2);
    assert!(rig.engine.store().get(k.resource.id).is_none());
    rig.engine.seal_song_preparation(k.epoch).unwrap();
    rig.step();
    assert!(rig.acks().contains(&SongHostAck::ResourceReady {
        epoch: k.epoch,
        resource: k.resource
    }));
}
#[test]
fn full_garbage_retains_silent_bus_owner_and_exact_key_until_receipt_queue_accepts() {
    let mut rig = Rig::new(StoreKind::NativeArc);
    let graph = key(1, 3, SongResourceKind::Track);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, 1, 1))
        .unwrap();
    reserve(&mut rig.engine, graph);
    let template = Box::new(BusTemplate::new());
    let pointer = std::ptr::from_ref(template.as_ref());
    assert!(rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: graph,
            payload: NativeInstall::Bus {
                resource: graph.resource.id,
                gen: graph.resource.generation,
                master: false,
                template
            }
        })
        .is_ok());
    rig.garbage.push(Garbage::Sample(sample(1))).ok().unwrap();
    rig.engine.cancel_song_preparation(graph.epoch).unwrap();
    for _ in 0..2 {
        assert!(rig.step().iter().all(|x| *x == 0.));
    }
    assert_eq!(
        rig.engine
            .buses()
            .slots
            .iter()
            .filter(|s| s.state == SlotState::Staged)
            .count(),
        1
    );
    assert!(rig.acks().is_empty());
    assert!(matches!(rig.returned.pop(), Some(Garbage::Sample(_))));
    rig.step();
    assert!(rig.acks().contains(&SongHostAck::LeaseReturned(graph)));
    let Some(Garbage::SongInstall(owner)) = rig.returned.pop() else {
        panic!("owned bus return")
    };
    assert_eq!(owner.lease, graph);
    let NativeInstall::Bus { template, .. } = owner.payload else {
        panic!("bus return")
    };
    assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
    rig.step();
    assert_eq!(rig.engine.song_remaining_capacities().unwrap().bus_slots, 3);
}
#[test]
fn native_host_getters_require_actual_fresh_report_and_recover_after_submission() {
    use vactr::host::native::audio::NativeAudioHost;
    let mut caps = CapabilitySet::browser();
    caps.max_voices = 1;
    let (mut host, mut audio) = NativeAudioHost::headless(48_000, caps, 4);
    assert!(host.song_remaining_capacities().is_err());
    let mut out = [0.; 32];
    audio.render(&mut out, 2);
    let mut messages = Vec::new();
    host.drain(&mut messages);
    let actual = host.song_remaining_capacities().unwrap();
    assert_eq!(actual.sample_rate, 48_000);
    assert_eq!(actual.cell_slots, 4);
    assert_eq!(
        actual.ack_slots,
        u32::try_from(vactr::host::native::audio::ACK_CAPACITY).unwrap()
    );
    assert_eq!(
        actual.pcm_bytes,
        u64::try_from(vactr::dsp::arena::DEFAULT_ARENA_BYTES).unwrap()
    );
    host.post(CtlMsg::Song(SongCommand::BeginStaging(
        SongStagePreparation {
            preparation: SongPreparation {
                epoch: SnapshotEpoch(9),
                branches: 0,
                resources: 0,
                required: SongHostCapacities {
                    sample_rate: 48_000,
                    ..SongHostCapacities::default()
                },
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        },
    )));
    assert!(host.song_remaining_capacities().is_err());
    audio.render(&mut out, 2);
    host.drain(&mut messages);
    assert!(host.song_remaining_capacities().is_ok());
}
#[test]
fn malformed_binding_and_missing_analyzer_bank_preserve_owned_graph() {
    let mut rig = Rig::new(StoreKind::NativeArc);
    let graph = key(1, 1, SongResourceKind::Track);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, 1, 1))
        .unwrap();
    reserve(&mut rig.engine, graph);
    let mut template = Box::new(BusTemplate::new());
    template
        .push(EffectKind::Analyzer(AnalyzerKind::Level))
        .unwrap();
    let pointer = std::ptr::from_ref(template.as_ref());
    let returned = rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: graph,
            payload: NativeInstall::Bus {
                resource: graph.resource.id,
                gen: graph.resource.generation,
                master: false,
                template,
            },
        })
        .err()
        .unwrap();
    let NativeInstall::Bus { template, .. } = returned.payload else {
        panic!("exact rejected owner")
    };
    assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
    assert_eq!(rig.engine.song_remaining_capacities().unwrap().bus_slots, 2); // reserved, not adopted
    let foreign = key(2, 3, SongResourceKind::AnalysisBank);
    rig.command(SongCommand::BindGraphBanks(SongGraphBanks {
        graph,
        controls: None,
        analysis: Some(foreign),
    }));
    rig.step();
    assert!(rig.acks().contains(&SongHostAck::Rejected {
        epoch: graph.epoch,
        reason: SongRejectCode::Malformed
    }));
}

#[test]
fn capacity_reports_track_actual_staged_cancelled_and_legacy_bus_regions() {
    fn reported(rig: &mut Rig) -> SongHostCapacities {
        rig.command(SongCommand::RequestCapacity(SnapshotEpoch(99)));
        rig.step();
        let report = rig
            .acks()
            .into_iter()
            .find_map(|ack| match ack {
                SongHostAck::CapacityReport(report) => Some(report),
                _ => None,
            })
            .expect("actual capacity receipt");
        let free = rig
            .engine
            .song_bus_regions()
            .iter()
            .try_fold(0_u64, |sum, region| sum.checked_add(region.frames))
            .unwrap();
        assert_eq!(report.available.bus_frames, free);
        report.available
    }
    let mut rig = Rig::new(StoreKind::NativeArc);
    let original = reported(&mut rig);
    let slot_frames = rig
        .engine
        .song_bus_regions()
        .iter()
        .find(|region| region.frames > 0)
        .unwrap()
        .frames;
    let graph = key(1, 50, SongResourceKind::Track);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, 1, 1))
        .unwrap();
    reserve(&mut rig.engine, graph);
    assert!(rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: graph,
            payload: NativeInstall::Bus {
                resource: graph.resource.id,
                gen: graph.resource.generation,
                master: false,
                template: Box::new(BusTemplate::new())
            },
        })
        .is_ok());
    let staged = reported(&mut rig);
    assert_eq!(staged.bus_frames, original.bus_frames - slot_frames);
    assert_eq!(staged.voice_frames, original.voice_frames);
    rig.engine.cancel_song_preparation(graph.epoch).unwrap();
    rig.step();
    rig.acks();
    assert!(matches!(rig.returned.pop(), Some(Garbage::SongInstall(_))));
    rig.step();
    rig.acks();
    assert_eq!(reported(&mut rig).bus_frames, original.bus_frames);
    rig.tx
        .push(NativeRecord::Install(NativeInstall::Bus {
            resource: 70,
            gen: 1,
            master: false,
            template: Box::new(BusTemplate::new()),
        }))
        .ok()
        .unwrap();
    rig.step();
    rig.acks();
    assert!(matches!(rig.returned.pop(), Some(Garbage::Bus(_))));
    let legacy = reported(&mut rig);
    assert_eq!(legacy.bus_frames, original.bus_frames - slot_frames);
    assert_eq!(legacy.voice_frames, original.voice_frames);
}

#[test]
fn malformed_effect_indices_return_original_instrument_without_panicking() {
    use vactr::dsp::ugen::{Node, Template};
    let mut rig = Rig::new(StoreKind::NativeArc);
    let graph = key(1, 60, SongResourceKind::Instrument);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, 1, 1))
        .unwrap();
    reserve(&mut rig.engine, graph);
    for kind in [
        EffectKind::Convolution,
        EffectKind::Analyzer(AnalyzerKind::Level),
        EffectKind::Gain,
    ] {
        for fx in [0, u8::MAX] {
            let mut template = Template::boxed();
            template.n_nodes = 1;
            template.nodes[0].node = Node::Effect { kind, fx };
            let pointer = std::ptr::from_ref(template.as_ref());
            let returned = rig
                .engine
                .stage_song_native(NativeSongInstall {
                    lease: graph,
                    payload: NativeInstall::Inst {
                        resource: graph.resource.id,
                        gen: graph.resource.generation,
                        template,
                    },
                })
                .expect_err("malformed effect rejected");
            let NativeInstall::Inst { template, .. } = returned.payload else {
                panic!("original instrument")
            };
            assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
            assert_eq!(template.n_fx, 0);
            assert_eq!(template.nodes[0].node, Node::Effect { kind, fx });
        }
    }
}

#[test]
fn node_sample_prevalidation_preserves_original_bound_controls_and_owner() {
    use vactr::dsp::graph::BankRef;
    use vactr::dsp::ugen::{Node, Template, AMP};
    let mut rig = Rig::new(StoreKind::NativeArc);
    let graph = key(1, 61, SongResourceKind::Instrument);
    let bank = key(1, 62, SongResourceKind::ControlCells);
    let pcm = key(1, 63, SongResourceKind::Sample);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, 1, 3))
        .unwrap();
    for lease in [graph, bank, pcm] {
        reserve(&mut rig.engine, lease);
    }
    rig.command(SongCommand::InitCells(SongCellInit {
        lease: bank,
        cell: CellId::new(42),
        value: 0.5,
    }));
    rig.command(SongCommand::BindGraphBanks(SongGraphBanks {
        graph,
        controls: Some(bank),
        analysis: None,
    }));
    rig.step();
    rig.acks();
    assert!(rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: pcm,
            payload: NativeInstall::Sample {
                resource: pcm.resource.id,
                gen: pcm.resource.generation,
                data: sample(1)
            },
        })
        .is_ok());
    rig.step();
    rig.acks();
    for (resource, listed) in [(999, true), (pcm.resource.id, false)] {
        let mut template = Template::boxed();
        template.n_params = 1;
        template.params[0] = (AMP, Ctl::Cell(CellId::new(42)));
        template.n_nodes = 1;
        template.nodes[0].node = Node::SamplePlay(BankRef::new(resource));
        if listed {
            template.n_refs = 1;
            template.refs[0] = resource;
        }
        let pointer = std::ptr::from_ref(template.as_ref());
        let returned = rig
            .engine
            .stage_song_native(NativeSongInstall {
                lease: graph,
                payload: NativeInstall::Inst {
                    resource: graph.resource.id,
                    gen: graph.resource.generation,
                    template,
                },
            })
            .expect_err("unknown or omitted ownership reference rejected");
        let NativeInstall::Inst { template, .. } = returned.payload else {
            panic!("original instrument")
        };
        assert_eq!(std::ptr::from_ref(template.as_ref()), pointer);
        assert_eq!(template.params[0], (AMP, Ctl::Cell(CellId::new(42))));
        assert_eq!(
            template.nodes[0].node,
            Node::SamplePlay(BankRef::new(resource))
        );
        assert_eq!(template.n_refs, usize::from(listed));
    }
}

#[test]
fn rejected_reconfiguration_preserves_pcm_provider_and_later_valid_staging() {
    let mut rig = Rig::new(StoreKind::NativeArc);
    let before = rig.engine.song_remaining_capacities().unwrap();
    let original_analysis = rig.engine.song_analysis_capacity().unwrap();
    let mut config = SongStagingConfig {
        preparations: 4,
        leases: 16,
        branches: 4,
        control_slots: 8,
        analysis_slots: 131_072,
        native_pcm_bytes: 1,
        critical_receipts: 5,
    };
    assert_eq!(
        rig.engine
            .configure_song_staging_for_transport(config, &rig.acks),
        Err(SongRejectCode::Capacity)
    );
    assert_eq!(rig.engine.song_remaining_capacities().unwrap(), before);
    assert_eq!(
        rig.engine.song_analysis_capacity().unwrap(),
        original_analysis
    );
    config.critical_receipts = 2;
    config.native_pcm_bytes = 128;
    rig.engine
        .configure_song_staging_for_transport(config, &rig.acks)
        .unwrap();
    let configured = rig.engine.song_remaining_capacities().unwrap();
    assert_eq!(configured.pcm_bytes, 128);
    assert_eq!(configured.bus_frames, before.bus_frames);
    let pcm = key(1, 90, SongResourceKind::Sample);
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, 1, 1))
        .unwrap();
    reserve(&mut rig.engine, pcm);
    assert!(rig
        .engine
        .stage_song_native(NativeSongInstall {
            lease: pcm,
            payload: NativeInstall::Sample {
                resource: pcm.resource.id,
                gen: pcm.resource.generation,
                data: sample(16)
            },
        })
        .is_ok());
    rig.step();
    assert_eq!(
        rig.engine.song_remaining_capacities().unwrap().pcm_bytes,
        64
    );
    assert!(rig.acks().contains(&SongHostAck::ResourceReady {
        epoch: pcm.epoch,
        resource: pcm.resource
    }));
}

#[path = "song_resource_staging/acceptance.rs"]
mod acceptance;

#[path = "song_resource_staging/live.rs"]
mod live;
