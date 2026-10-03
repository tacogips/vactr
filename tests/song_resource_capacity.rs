//! Real reserved ownership mixed with legacy physical occupancy.
use std::sync::Arc;
use vactr::dsp::arena::{encode_bus, encode_inst, FaultCode, StoreKind};
use vactr::dsp::bus::BusTemplate;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::cells::AtomicCells;
use vactr::dsp::engine::{Engine, EngineConfig, SongStagingConfig};
use vactr::dsp::graph::{BusDef, BusId, InstDef, InstId, UGenSpec};
use vactr::dsp::ring::{
    self, EngineIo, EventRing, Garbage, NativeInstall, NativeRecord, NativeSongInstall, SpscRing,
};
use vactr::dsp::ugen::Template;
use vactr::host::caps::SampleData;
use vactr::host::wire::{AudioEvent, CtlMsg, HostMsg, VoiceTag};
use vactr::sched::slots::SlotId;
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
        let (_, events) = EventRing::split(2);
        let (garbage, returned) = SpscRing::split(1);
        Self {
            engine,
            tx,
            controls,
            events,
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
        self.step_with_garbage(true)
    }
    fn step_with_garbage(&mut self, present: bool) -> [f32; 32] {
        let mut output = [0.; 32];
        self.engine.process(
            &mut EngineIo {
                events: &mut self.events,
                controls: &mut self.controls,
                acks: &mut self.acks,
                cells: &mut self.cells,
                garbage: present.then_some(&mut self.garbage),
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

fn inst(rig: &Rig, id: u32) -> Box<Template> {
    Template::from_inst(
        &InstDef {
            id: InstId::new(id),
            params: Box::new([]),
            nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
            edges: Box::new([]),
            node_params: Box::new([]),
        },
        &rig.engine.build_env(),
    )
    .unwrap()
}
fn bus(id: u32) -> Box<BusTemplate> {
    let mut t = Box::new(BusTemplate::new());
    t.bus = BusId::new(id);
    t
}
fn legacy(r: &mut Rig, kind: SongResourceKind, id: u32) {
    let n = match kind {
        SongResourceKind::Instrument => NativeInstall::Inst {
            resource: id,
            gen: 1,
            template: inst(r, id),
        },
        SongResourceKind::Track => NativeInstall::Bus {
            resource: id,
            gen: 1,
            master: false,
            template: bus(id),
        },
        _ => unreachable!(),
    };
    r.tx.push(NativeRecord::Install(n)).ok().unwrap();
    r.step();
    r.acks();
    while r.returned.pop().is_some() {}
}
fn remaining(r: &Rig, kind: SongResourceKind) -> u32 {
    let c = r.engine.song_remaining_capacities().unwrap();
    match kind {
        SongResourceKind::Instrument => c.template_slots,
        SongResourceKind::Track => c.bus_slots,
        _ => unreachable!(),
    }
}
#[test]
fn unadopted_bus_and_template_claims_add_to_legacy_occupancy() {
    for kind in [SongResourceKind::Instrument, SongResourceKind::Track] {
        let mut r = Rig::new(StoreKind::NativeArc);
        r.engine
            .begin_song_preparation(preparation(&r.engine, 1, 1))
            .unwrap();
        reserve(&mut r.engine, key(1, 20, kind));
        legacy(&mut r, kind, 40);
        assert_eq!(remaining(&r, kind), 1);
        let mut p = preparation(&r.engine, 2, 2);
        if kind == SongResourceKind::Instrument {
            p.preparation.required.template_slots = 2;
        } else {
            p.preparation.required.bus_slots = 2;
        }
        assert_eq!(
            r.engine.begin_song_preparation(p),
            Err(SongRejectCode::Capacity)
        );
    }
}
#[test]
fn adopted_physical_owners_are_charged_once() {
    for kind in [SongResourceKind::Instrument, SongResourceKind::Track] {
        let mut r = Rig::new(StoreKind::NativeArc);
        r.engine
            .begin_song_preparation(preparation(&r.engine, 1, 1))
            .unwrap();
        let k = key(1, 20, kind);
        reserve(&mut r.engine, k);
        let payload = if kind == SongResourceKind::Instrument {
            NativeInstall::Inst {
                resource: 20,
                gen: u32::MAX,
                template: inst(&r, 20),
            }
        } else {
            NativeInstall::Bus {
                resource: 20,
                gen: u32::MAX,
                master: false,
                template: bus(20),
            }
        };
        assert!(r
            .engine
            .stage_song_native(NativeSongInstall { lease: k, payload })
            .is_ok());
        assert_eq!(remaining(&r, kind), 2);
        legacy(&mut r, kind, 40);
        assert_eq!(remaining(&r, kind), 1);
    }
}
#[test]
fn native_pending_pcm_and_browser_installing_entries_are_charged_once() {
    for store in [StoreKind::NativeArc, StoreKind::Arena { bytes: 4096 }] {
        let mut r = Rig::new(store);
        let initial = r.engine.song_remaining_capacities().unwrap();
        r.engine
            .begin_song_preparation(preparation(&r.engine, 1, 1))
            .unwrap();
        let k = key(1, 20, SongResourceKind::Sample);
        reserve(&mut r.engine, k);
        let claimed = r.engine.song_remaining_capacities().unwrap();
        assert_eq!(claimed.sample_resources, initial.sample_resources - 1);
        assert_eq!(claimed.pcm_bytes, initial.pcm_bytes);
        if matches!(store, StoreKind::NativeArc) {
            assert!(r
                .engine
                .stage_song_native(NativeSongInstall {
                    lease: k,
                    payload: NativeInstall::Sample {
                        resource: 20,
                        gen: u32::MAX,
                        data: sample(64)
                    }
                })
                .is_ok());
        } else {
            let mut bytes = [0u8; 27];
            let n = ring::encode_song_sample_begin(k, 64, 1, 48_000, &mut bytes);
            let mut inbox = ring::ByteInbox::new();
            assert!(inbox.push(&bytes[..n]));
            let mut output = [0.; 32];
            r.engine.process(
                &mut EngineIo {
                    events: &mut r.events,
                    controls: &mut inbox,
                    acks: &mut r.acks,
                    cells: &mut r.cells,
                    garbage: Some(&mut r.garbage),
                },
                &mut output,
                16,
            );
        }
        let occupied = r.engine.song_remaining_capacities().unwrap();
        assert_eq!(occupied.sample_resources, claimed.sample_resources);
        assert_eq!(occupied.pcm_bytes, initial.pcm_bytes - 256);
    }
}
#[test]
fn legacy_ingress_cannot_consume_last_reserved_slot() {
    for kind in [SongResourceKind::Instrument, SongResourceKind::Track] {
        let mut r = Rig::new(StoreKind::NativeArc);
        r.engine
            .begin_song_preparation(preparation(&r.engine, 1, 3))
            .unwrap();
        for id in 20..23 {
            reserve(&mut r.engine, key(1, id, kind));
        }
        assert_eq!(remaining(&r, kind), 0);
        legacy(&mut r, kind, 40);
        assert_eq!(remaining(&r, kind), 0);
        assert!(r.engine.song_remaining_capacities().is_ok());
        assert_eq!(r.engine.pop_fault().unwrap().resource, 40);
    }
}
#[test]
fn cancellation_and_delayed_owner_return_keep_capacity_until_exact_release() {
    let mut r = Rig::new(StoreKind::NativeArc);
    let before = r.engine.song_remaining_capacities().unwrap();
    r.engine
        .begin_song_preparation(preparation(&r.engine, 1, 1))
        .unwrap();
    let k = key(1, 20, SongResourceKind::Instrument);
    reserve(&mut r.engine, k);
    assert!(r
        .engine
        .stage_song_native(NativeSongInstall {
            lease: k,
            payload: NativeInstall::Inst {
                resource: 20,
                gen: u32::MAX,
                template: inst(&r, 20)
            }
        })
        .is_ok());
    r.garbage.push(Garbage::Bus(bus(99))).ok().unwrap();
    r.command(SongCommand::CancelPreparation(k.epoch));
    r.step();
    assert_eq!(
        r.engine.song_remaining_capacities().unwrap().template_slots,
        before.template_slots - 1
    );
    r.returned.pop().unwrap();
    for _ in 0..8 {
        r.step();
        r.acks();
        while r.returned.pop().is_some() {}
    }
    assert_eq!(
        r.engine.song_remaining_capacities().unwrap().template_slots,
        before.template_slots
    );
}
#[test]
fn foreign_full_keys_and_multiple_epochs_never_alias_physical_owners() {
    let mut r = Rig::new(StoreKind::NativeArc);
    for epoch in [1, 2] {
        r.engine
            .begin_song_preparation(preparation(&r.engine, epoch, 1))
            .unwrap();
        reserve(&mut r.engine, key(epoch, 20, SongResourceKind::Instrument));
    }
    let k = key(1, 20, SongResourceKind::Instrument);
    assert!(r
        .engine
        .stage_song_native(NativeSongInstall {
            lease: k,
            payload: NativeInstall::Inst {
                resource: 20,
                gen: u32::MAX,
                template: inst(&r, 20)
            }
        })
        .is_ok());
    assert_eq!(remaining(&r, SongResourceKind::Instrument), 1);
    let mut wrong = key(2, 20, SongResourceKind::Instrument);
    wrong.resource.generation = 1;
    assert!(r
        .engine
        .stage_song_native(NativeSongInstall {
            lease: wrong,
            payload: NativeInstall::Inst {
                resource: 20,
                gen: 1,
                template: inst(&r, 20)
            }
        })
        .is_err());
    assert_eq!(remaining(&r, SongResourceKind::Instrument), 1);
}
#[test]
fn rejected_legacy_native_owners_survive_full_and_missing_garbage() {
    for present in [false, true] {
        let mut r = Rig::new(StoreKind::NativeArc);
        r.engine
            .begin_song_preparation(preparation(&r.engine, 1, 2))
            .unwrap();
        legacy(&mut r, SongResourceKind::Instrument, 3);
        r.tx.push(NativeRecord::Msg(CtlMsg::LiveNoteOn {
            tag: VoiceTag {
                slot: SlotId::new(1),
                channel: 0,
                pitch: 60,
                seq: 1,
            },
            ev: AudioEvent::new(0., SlotId::new(1), 0, InstId::new(3)),
        }))
        .ok()
        .unwrap();
        assert!(r.step().iter().any(|x| x.abs() > 0.));
        r.acks();
        for id in 20..22 {
            reserve(&mut r.engine, key(1, id, SongResourceKind::Instrument));
        }
        let t = inst(&r, 40);
        let ptr = std::ptr::from_ref(t.as_ref());
        if present {
            r.garbage.push(Garbage::Bus(bus(99))).ok().unwrap();
        }
        r.tx.push(NativeRecord::Install(NativeInstall::Inst {
            resource: 40,
            gen: 1,
            template: t,
        }))
        .ok()
        .unwrap();
        r.tx.push(NativeRecord::Msg(CtlMsg::Song(
            SongCommand::RequestCapacity(SnapshotEpoch(55)),
        )))
        .ok()
        .unwrap();
        assert!(r.step_with_garbage(present).iter().any(|x| x.abs() > 0.));
        assert!(r.acks().is_empty());
        let previous = r.engine.now();
        assert!(r.step_with_garbage(present).iter().any(|x| x.abs() > 0.));
        assert!(r.engine.now() > previous);
        assert!(r.acks().is_empty());
        if present {
            r.returned.pop().unwrap();
        }
        r.step();
        let Garbage::Template(t) = r.returned.pop().unwrap() else {
            panic!("original template")
        };
        assert_eq!(std::ptr::from_ref(t.as_ref()), ptr);
        assert_eq!(t.inst, InstId::new(40));
        assert!(r.acks().iter().any(|ack| matches!(ack,SongHostAck::CapacityReport(report) if report.epoch==SnapshotEpoch(55))));
    }
}

#[test]
fn sample_claims_block_legacy_pcm_and_retain_arc_until_garbage_handoff() {
    let mut r = Rig::new(StoreKind::NativeArc);
    let count = r
        .engine
        .song_remaining_capacities()
        .unwrap()
        .sample_resources;
    r.engine
        .configure_song_staging_for_transport(
            SongStagingConfig {
                preparations: 2,
                leases: count + 1,
                branches: 1,
                control_slots: 8,
                analysis_slots: 64,
                native_pcm_bytes: 4096,
                critical_receipts: 2,
            },
            &r.acks,
        )
        .unwrap();
    r.engine
        .begin_song_preparation(preparation(&r.engine, 1, count))
        .unwrap();
    for id in 0..count {
        reserve(&mut r.engine, key(1, id, SongResourceKind::Sample));
    }
    let c = r.engine.song_remaining_capacities().unwrap();
    assert_eq!(c.sample_resources, 0);
    assert_eq!(c.pcm_bytes, 4096);
    let data = sample(1);
    let ptr = Arc::as_ptr(&data);
    r.tx.push(NativeRecord::Install(NativeInstall::Sample {
        resource: 1000,
        gen: 1,
        data,
    }))
    .ok()
    .unwrap();
    assert_eq!(r.step_with_garbage(false), [0.; 32]);
    assert!(r.engine.store().get(1000).is_none());
    assert_eq!(r.engine.song_remaining_capacities().unwrap(), c);
    r.step();
    let Garbage::Sample(data) = r.returned.pop().unwrap() else {
        panic!("same sample Arc")
    };
    assert_eq!(Arc::as_ptr(&data), ptr);
    assert_eq!(&*data.frames, &[0.5]);
}

fn byte_legacy_record(kind: SongResourceKind, resource: u32) -> Vec<u8> {
    let mut graph = Vec::new();
    match kind {
        SongResourceKind::Instrument => {
            let def = InstDef {
                id: InstId::new(77),
                params: Box::new([]),
                nodes: vec![UGenSpec::SinOsc].into_boxed_slice(),
                edges: Box::new([]),
                node_params: Box::new([]),
            };
            encode_inst(&def, &mut graph).unwrap();
        }
        SongResourceKind::Track => {
            let def = BusDef {
                id: BusId::new(77),
                chain: Box::new([]),
            };
            encode_bus(&def, false, &mut graph).unwrap();
        }
        SongResourceKind::Sample => {
            return ring::encode_sample_begin(resource, 7, 1, 1, 48_000).to_vec()
        }
        _ => unreachable!(),
    }
    let mut encoded = Vec::new();
    ring::encode_graph_record(resource, 7, &graph, &mut encoded);
    encoded
}
fn process_byte_capacity(rig: &mut Rig, record: &[u8], epoch: SnapshotEpoch) -> Vec<HostMsg> {
    let mut inbox = ring::ByteInbox::new();
    assert!(inbox.push(record));
    let mut following = [0u8; 32];
    let n = CtlMsg::Song(SongCommand::RequestCapacity(epoch)).encode(&mut following);
    assert!(n > 0);
    assert!(inbox.push(&following[..n]));
    let mut output = [0.; 32];
    rig.engine.process(
        &mut EngineIo {
            events: &mut rig.events,
            controls: &mut inbox,
            acks: &mut rig.acks,
            cells: &mut rig.cells,
            garbage: Some(&mut rig.garbage),
        },
        &mut output,
        16,
    );
    assert_eq!(output, [0.; 32]);
    std::iter::from_fn(|| rig.received.pop()).collect()
}
#[test]
fn byte_legacy_claim_rejections_preserve_fault_identity_and_following_record() {
    for kind in [
        SongResourceKind::Instrument,
        SongResourceKind::Track,
        SongResourceKind::Sample,
    ] {
        let resource = u32::MAX - 1;
        let epoch = SnapshotEpoch(u64::MAX);
        let encoded = byte_legacy_record(kind, resource);
        let mut baseline = Rig::new(StoreKind::Arena { bytes: 4096 });
        let receipts = process_byte_capacity(&mut baseline, &encoded, epoch);
        // SampleBegin legitimately awaits slices; graph installs acknowledge here.
        if kind != SongResourceKind::Sample {
            assert!(receipts.contains(&HostMsg::Installed { resource, gen: 7 }));
        } else {
            assert_eq!(
                baseline.engine.store().state(resource),
                Some(vactr::dsp::arena::ResState::Installing)
            );
        }
        assert!(
            baseline.engine.pop_fault().is_none(),
            "same bytes valid without claims"
        );
        assert!(receipts
            .iter()
            .any(|m| matches!(m,HostMsg::Song(SongHostAck::CapacityReport(c)) if c.epoch==epoch)));
        let mut rig = Rig::new(StoreKind::Arena { bytes: 4096 });
        let actual = rig.engine.song_remaining_capacities().unwrap();
        let count = match kind {
            SongResourceKind::Instrument => actual.template_slots,
            SongResourceKind::Track => actual.bus_slots,
            SongResourceKind::Sample => actual.sample_resources,
            _ => unreachable!(),
        };
        rig.engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 2,
                    leases: count + 1,
                    branches: 1,
                    control_slots: 8,
                    analysis_slots: 64,
                    native_pcm_bytes: 4096,
                    critical_receipts: 2,
                },
                &rig.acks,
            )
            .unwrap();
        rig.engine
            .begin_song_preparation(preparation(&rig.engine, 1, count))
            .unwrap();
        for id in 0..count {
            reserve(&mut rig.engine, key(1, id, kind));
        }
        let before = rig.engine.song_remaining_capacities().unwrap();
        let receipts = process_byte_capacity(&mut rig, &encoded, epoch);
        let fault = rig.engine.pop_fault().unwrap();
        assert_eq!(fault.code, FaultCode::BadResource);
        assert_eq!(fault.resource, resource);
        assert!(rig.engine.pop_fault().is_none());
        assert!(!receipts
            .iter()
            .any(|m| matches!(m, HostMsg::Installed { .. })));
        assert_eq!(rig.engine.song_remaining_capacities().unwrap(), before);
        let reports: Vec<_> = receipts
            .iter()
            .filter_map(|m| {
                if let HostMsg::Song(SongHostAck::CapacityReport(c)) = m {
                    Some(c)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].epoch, epoch);
        assert_eq!(reports[0].available, before);
        if kind == SongResourceKind::Sample {
            assert!(rig.engine.store().state(resource).is_none());
        }
        if kind == SongResourceKind::Instrument {
            assert!(rig.engine.template(InstId::new(77)).is_none());
        }
    }
}
