//! Actual audible legacy playback and callback ownership safety.
use super::*;
use vactr::dsp::arena::{encode_bus, encode_inst};
use vactr::dsp::graph::{BankRef, BusDef, BusId, Edge, EffectSpec, InstDef, InstId, UGenSpec};
use vactr::dsp::ugen::Template;
use vactr::host::wire::AudioEvent;
use vactr::sched::slots::{CtlId, SlotId};

struct Probe;
thread_local! {
    static CALLBACK_MEMORY: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}
// SAFETY: allocation ownership and all contracts are forwarded to System.
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let _ = CALLBACK_MEMORY.try_with(|counter| {
            if let Some((a, d)) = counter.get() {
                counter.set(Some((a + 1, d)));
            }
        });
        // SAFETY: caller layout is forwarded unchanged.
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        let _ = CALLBACK_MEMORY.try_with(|counter| {
            if let Some((a, d)) = counter.get() {
                counter.set(Some((a, d + 1)));
            }
        });
        // SAFETY: original pointer and allocation layout are forwarded unchanged.
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        let _ = CALLBACK_MEMORY.try_with(|counter| {
            if let Some((a, d)) = counter.get() {
                counter.set(Some((a + 1, d + 1)));
            }
        });
        // SAFETY: original ownership/layout and requested size are forwarded unchanged.
        unsafe { std::alloc::GlobalAlloc::realloc(&std::alloc::System, pointer, layout, size) }
    }
}
#[global_allocator]
static GLOBAL: Probe = Probe;

fn audible_rig(store: StoreKind) -> Rig {
    let mut rig = Rig::with_bus_slots(store, 8);
    rig.engine
        .configure_song_staging_for_transport(
            SongStagingConfig {
                preparations: 4,
                leases: 16,
                branches: 4,
                control_slots: 8,
                analysis_slots: 131_072,
                native_pcm_bytes: 16_384,
                critical_receipts: 2,
            },
            &rig.acks,
        )
        .unwrap();
    rig
}
fn live_block(rig: &mut Rig, inbox: Option<&mut ByteInbox>) -> [f32; 32] {
    let mut output = [0.; 32];
    if let Some(inbox) = inbox {
        rig.engine.process(
            &mut EngineIo {
                events: &mut rig.events,
                controls: inbox,
                acks: &mut rig.acks,
                cells: &mut rig.cells,
                garbage: Some(&mut rig.garbage),
            },
            &mut output,
            16,
        );
    } else {
        rig.engine.process(
            &mut EngineIo {
                events: &mut rig.events,
                controls: &mut rig.controls,
                acks: &mut rig.acks,
                cells: &mut rig.cells,
                garbage: Some(&mut rig.garbage),
            },
            &mut output,
            16,
        );
    }
    output
}
fn probed_block(rig: &mut Rig, inbox: Option<&mut ByteInbox>) -> [f32; 32] {
    CALLBACK_MEMORY.with(|counter| {
        assert!(counter.get().is_none());
        counter.set(Some((0, 0)));
    });
    let output = live_block(rig, inbox);
    let counts = CALLBACK_MEMORY.with(|counter| counter.replace(None).unwrap());
    assert_eq!(
        counts,
        (0, 0),
        "configured callback allocates/destroys nothing"
    );
    output
}
fn live_inst() -> InstDef {
    InstDef {
        id: InstId::new(3),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::SamplePlay(BankRef::new(7)),
            UGenSpec::SinOsc,
            UGenSpec::Add,
        ]
        .into_boxed_slice(),
        edges: vec![
            Edge {
                from: 0,
                to: 2,
                port: 0,
                output: 0,
            },
            Edge {
                from: 1,
                to: 2,
                port: 1,
                output: 0,
            },
        ]
        .into_boxed_slice(),
        node_params: Box::new([]),
    }
}
fn live_bus(master: bool) -> BusDef {
    let kind = if master {
        EffectKind::Compressor
    } else {
        EffectKind::Gain
    };
    let params = if master {
        vec![
            (vactr::dsp::effects::SIDECHAIN_BUS_CTL, Ctl::Const(91.)),
            (
                vactr::dsp::effects::param_ctl(kind, "threshold").unwrap(),
                Ctl::Const(-40.),
            ),
            (
                vactr::dsp::effects::param_ctl(kind, "ratio").unwrap(),
                Ctl::Const(20.),
            ),
        ]
    } else {
        vec![(
            vactr::dsp::effects::param_ctl(kind, "gain").unwrap(),
            Ctl::Const(1.),
        )]
    };
    BusDef {
        id: BusId::new(if master { 0 } else { 91 }),
        chain: vec![EffectSpec {
            kind,
            params: params.into_boxed_slice(),
        }]
        .into_boxed_slice(),
    }
}
fn install_audible_legacy(rig: &mut Rig, bytes: bool) {
    let data = sample(2048);
    let mut inbox = ByteInbox::new();
    let mut encoded = Vec::new();
    if bytes {
        assert!(inbox.push(&ring::encode_sample_begin(7, 1, 2048, 1, 48_000)));
        probed_block(rig, Some(&mut inbox));
        rig.acks();
        for (index, chunk) in data.frames.chunks(128).enumerate() {
            ring::encode_slice(7, u32::try_from(index * 128).unwrap(), chunk, &mut encoded);
            assert!(inbox.push(&encoded));
            probed_block(rig, Some(&mut inbox));
            rig.acks();
            while rig.returned.pop().is_some() {}
        }
    } else {
        rig.tx
            .push(NativeRecord::Install(NativeInstall::Sample {
                resource: 7,
                gen: 1,
                data,
            }))
            .ok()
            .unwrap();
        probed_block(rig, None);
        rig.acks();
        while rig.returned.pop().is_some() {}
    }
    let inst = live_inst();
    if bytes {
        let mut graph = Vec::new();
        encode_inst(&inst, &mut graph).unwrap();
        ring::encode_graph_record(3, 1, &graph, &mut encoded);
        assert!(inbox.push(&encoded));
        probed_block(rig, Some(&mut inbox));
    } else {
        let template = Template::from_inst(&inst, &rig.engine.build_env()).unwrap();
        rig.tx
            .push(NativeRecord::Install(NativeInstall::Inst {
                resource: 3,
                gen: 1,
                template,
            }))
            .ok()
            .unwrap();
        probed_block(rig, None);
    }
    rig.acks();
    while rig.returned.pop().is_some() {}
    for master in [false, true] {
        let def = live_bus(master);
        if bytes {
            let mut graph = Vec::new();
            encode_bus(&def, master, &mut graph).unwrap();
            ring::encode_graph_record(if master { 71 } else { 70 }, 1, &graph, &mut encoded);
            assert!(inbox.push(&encoded));
            probed_block(rig, Some(&mut inbox));
        } else {
            rig.tx
                .push(NativeRecord::Install(NativeInstall::Bus {
                    resource: if master { 71 } else { 70 },
                    gen: 1,
                    master,
                    template: Box::new(BusTemplate::from_def(&def).unwrap()),
                }))
                .ok()
                .unwrap();
            probed_block(rig, None);
        }
        rig.acks();
        while rig.returned.pop().is_some() {}
    }
    assert!(rig.engine.template(InstId::new(3)).is_some());
    assert!(rig.engine.store().get(7).is_some());
    assert!(rig.engine.buses().find(BusId::new(91)).is_some());
    assert!(rig.engine.buses().slots[rig.engine.buses().master()]
        .kinds()
        .any(|kind| kind == EffectKind::Compressor));
    let mut event = AudioEvent::new(0.05, SlotId::new(1), 1, InstId::new(3));
    event
        .push_ctl(vactr::dsp::voice::BUS, Ctl::Const(91.))
        .unwrap();
    event.push_ctl(CtlId::new(49), Ctl::Const(1.)).unwrap();
    rig.events_tx.push(event).unwrap();
}
fn compare_live_block(subject: &mut Rig, reference: &mut Rig, inbox: Option<&mut ByteInbox>) {
    let actual = probed_block(subject, inbox);
    let expected = probed_block(reference, None);
    assert_eq!(
        actual, expected,
        "candidate cannot affect live legacy samples"
    );
    assert!(
        expected.iter().any(|x| x.abs() > 0.),
        "reference is genuinely audible"
    );
    assert_eq!(subject.engine.active_voices(), 1);
    assert_eq!(reference.engine.active_voices(), 1);
    let s = subject.engine.buses().find(BusId::new(91)).unwrap();
    let r = reference.engine.buses().find(BusId::new(91)).unwrap();
    assert_eq!(
        subject.engine.buses().frames(s, 16),
        reference.engine.buses().frames(r, 16)
    );
    assert_eq!(
        subject.engine.buses().master(),
        reference.engine.buses().master()
    );
    assert_eq!(
        subject
            .engine
            .buses()
            .frames(subject.engine.buses().master(), 16),
        reference
            .engine
            .buses()
            .frames(reference.engine.buses().master(), 16)
    );
}
fn self_noise_bus() -> Box<BusTemplate> {
    let mut bus = Box::new(BusTemplate::new());
    bus.bus = BusId::new(91);
    let index = bus.push(EffectKind::NoiseBlend).unwrap();
    for (name, value) in [("level", 0.), ("mix", 1.)] {
        assert!(bus.push_param(
            index,
            vactr::dsp::effects::param_ctl(EffectKind::NoiseBlend, name).unwrap(),
            Ctl::Const(value)
        ));
    }
    bus
}
fn enqueue_candidate(
    rig: &mut Rig,
    inbox: &mut ByteInbox,
    lease: SongLeaseKey,
    bytes: bool,
    malformed: bool,
) -> Option<*const BusTemplate> {
    let template = if malformed {
        let mut template = Box::new(BusTemplate::new());
        template.bus = BusId::new(91);
        let index = template.push(EffectKind::Gain).unwrap();
        assert!(template.push_param(
            index,
            vactr::dsp::effects::param_ctl(EffectKind::Gain, "gain").unwrap(),
            Ctl::Cell(CellId::new(42))
        ));
        template
    } else {
        self_noise_bus()
    };
    if bytes {
        let def = BusDef {
            id: template.bus,
            chain: (0..template.n)
                .map(|i| EffectSpec {
                    kind: template.kinds[i],
                    params: template.params[i][..usize::from(template.n_params[i])]
                        .to_vec()
                        .into_boxed_slice(),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        };
        let mut graph = Vec::new();
        encode_bus(&def, lease.kind == SongResourceKind::Master, &mut graph).unwrap();
        let mut framed = [0_u8; 8192];
        let n = ring::encode_song_graph_record(lease, &graph, &mut framed);
        assert!(n > 0 && inbox.push(&framed[..n]));
        None
    } else {
        let pointer = std::ptr::from_ref(template.as_ref());
        rig.tx
            .push(NativeRecord::SongInstall(NativeSongInstall {
                lease,
                payload: NativeInstall::Bus {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    master: lease.kind == SongResourceKind::Master,
                    template,
                },
            }))
            .ok()
            .unwrap();
        Some(pointer)
    }
}
#[test]
fn native_live_legacy_audio_is_unchanged_by_staged_self_noise() {
    live_twins(false);
}
#[test]
fn arena_live_legacy_audio_is_unchanged_by_staged_self_noise() {
    live_twins(true);
}
fn live_twins(bytes: bool) {
    let store = if bytes {
        StoreKind::Arena { bytes: 32768 }
    } else {
        StoreKind::NativeArc
    };
    let mut subject = audible_rig(store);
    let mut reference = audible_rig(store);
    install_audible_legacy(&mut subject, bytes);
    install_audible_legacy(&mut reference, bytes);
    let mut started = false;
    for _ in 0..200 {
        let actual = probed_block(&mut subject, None);
        let expected = probed_block(&mut reference, None);
        assert_eq!(actual, expected);
        subject.acks();
        reference.acks();
        while subject.returned.pop().is_some() {}
        while reference.returned.pop().is_some() {}
        if reference.engine.active_voices() == 1 {
            assert!(expected.iter().any(|x| x.abs() > 0.));
            started = true;
            break;
        }
    }
    assert!(started, "scheduled real event starts boundedly");
    for _ in 0..4 {
        compare_live_block(&mut subject, &mut reference, None);
    }
    let capacity = subject.engine.song_remaining_capacities().unwrap();
    assert!(capacity.bus_slots >= 2);
    assert!(
        subject
            .engine
            .song_bus_regions()
            .iter()
            .filter(|r| r.frames > 0)
            .count()
            >= 2
    );
    let mut inbox = ByteInbox::new();
    let mut receipts = Vec::new();
    let mut garbage = Vec::new();
    let owners = [
        key(u64::MAX - 5, 81, SongResourceKind::Track),
        key(u64::MAX - 5, 82, SongResourceKind::Master),
    ];
    subject
        .engine
        .begin_song_preparation(preparation(&subject.engine, owners[0].epoch.0, 2))
        .unwrap();
    for owner in owners {
        reserve(&mut subject.engine, owner);
    }
    let pointers =
        owners.map(|owner| enqueue_candidate(&mut subject, &mut inbox, owner, bytes, false));
    for _ in 0..8 {
        compare_live_block(
            &mut subject,
            &mut reference,
            if bytes { Some(&mut inbox) } else { None },
        );
        receipts.extend(subject.acks());
        reference.acks();
        while let Some(owner) = subject.returned.pop() {
            garbage.push(owner);
        }
        while reference.returned.pop().is_some() {}
    }
    for owner in owners {
        assert_eq!(
            receipts
                .iter()
                .filter(|ack| **ack
                    == SongHostAck::ResourceReady {
                        epoch: owner.epoch,
                        resource: owner.resource
                    })
                .count(),
            1
        );
    }
    assert_eq!(
        subject
            .engine
            .buses()
            .slots
            .iter()
            .filter(|s| s.state == SlotState::Staged)
            .count(),
        2
    );
    subject.command(SongCommand::SealPreparation(owners[0].epoch));
    for _ in 0..4 {
        compare_live_block(&mut subject, &mut reference, None);
        receipts.extend(subject.acks());
        reference.acks();
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|ack| **ack == SongHostAck::Ready(owners[0].epoch))
            .count(),
        1
    );
    subject.command(SongCommand::CancelPreparation(owners[0].epoch));
    for _ in 0..12 {
        compare_live_block(&mut subject, &mut reference, None);
        receipts.extend(subject.acks());
        reference.acks();
        while let Some(owner) = subject.returned.pop() {
            garbage.push(owner);
        }
        while reference.returned.pop().is_some() {}
    }
    for owner in owners {
        assert_eq!(
            receipts
                .iter()
                .filter(|ack| **ack == SongHostAck::LeaseReturned(owner))
                .count(),
            1
        );
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|ack| **ack == SongHostAck::PreparationCancelled(owners[0].epoch))
            .count(),
        1
    );
    if !bytes {
        assert_eq!(garbage.len(), 2);
        for returned in garbage.drain(..) {
            let Garbage::SongInstall(returned) = returned else {
                panic!("candidate owner")
            };
            let index = owners
                .iter()
                .position(|key| *key == returned.lease)
                .unwrap();
            let NativeInstall::Bus { template, .. } = returned.payload else {
                panic!("candidate bus")
            };
            assert_eq!(Some(std::ptr::from_ref(template.as_ref())), pointers[index]);
        }
    } else {
        assert!(garbage.is_empty());
    }
    assert_eq!(
        subject.engine.song_remaining_capacities().unwrap(),
        capacity
    );
    let failed = key(u64::MAX - 4, 83, SongResourceKind::Track);
    subject
        .engine
        .begin_song_preparation(preparation(&subject.engine, failed.epoch.0, 1))
        .unwrap();
    reserve(&mut subject.engine, failed);
    let pointer = enqueue_candidate(&mut subject, &mut inbox, failed, bytes, true);
    for _ in 0..6 {
        compare_live_block(
            &mut subject,
            &mut reference,
            if bytes { Some(&mut inbox) } else { None },
        );
        receipts.extend(subject.acks());
        reference.acks();
        while let Some(owner) = subject.returned.pop() {
            garbage.push(owner);
        }
        while reference.returned.pop().is_some() {}
    }
    assert!(receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::Rejected { epoch, .. } if *epoch == failed.epoch)));
    subject.command(SongCommand::CancelPreparation(failed.epoch));
    for _ in 0..12 {
        compare_live_block(&mut subject, &mut reference, None);
        receipts.extend(subject.acks());
        reference.acks();
        while let Some(owner) = subject.returned.pop() {
            garbage.push(owner);
        }
        while reference.returned.pop().is_some() {}
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|ack| **ack == SongHostAck::LeaseReturned(failed))
            .count(),
        1
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|ack| **ack == SongHostAck::PreparationCancelled(failed.epoch))
            .count(),
        1
    );
    assert!(!receipts
        .iter()
        .any(|ack| *ack == SongHostAck::Ready(failed.epoch)));
    if !bytes {
        assert_eq!(garbage.len(), 1);
        let Garbage::SongInstall(returned) = garbage.pop().unwrap() else {
            panic!("failed owner")
        };
        assert_eq!(returned.lease, failed);
        let NativeInstall::Bus { template, .. } = returned.payload else {
            panic!("failed bus")
        };
        assert_eq!(Some(std::ptr::from_ref(template.as_ref())), pointer);
    } else {
        assert!(garbage.is_empty());
    }
    assert_eq!(
        subject.engine.song_remaining_capacities().unwrap(),
        capacity
    );
}

fn pressure_adopt(
    rig: &mut Rig,
    bytes: bool,
) -> (
    [SongLeaseKey; 4],
    Option<*const Template>,
    Option<*const SampleData>,
) {
    let owners = [
        key(u64::MAX - 9, 41, SongResourceKind::Sample),
        key(u64::MAX - 9, 42, SongResourceKind::Instrument),
        key(u64::MAX - 9, 43, SongResourceKind::ControlCells),
        key(u64::MAX - 9, 44, SongResourceKind::AnalysisBank),
    ];
    let [pcm, inst, control, analysis] = owners;
    rig.engine
        .begin_song_preparation(preparation(&rig.engine, pcm.epoch.0, 4))
        .unwrap();
    for owner in owners {
        reserve(&mut rig.engine, owner);
    }
    rig.command(SongCommand::InitCells(SongCellInit {
        lease: control,
        cell: CellId::new(42),
        value: 0.5,
    }));
    rig.command(SongCommand::ReserveAnalysis(SongAnalysisReservation {
        lease: analysis,
        slots: 64,
    }));
    rig.command(SongCommand::BindGraphBanks(SongGraphBanks {
        graph: inst,
        controls: Some(control),
        analysis: Some(analysis),
    }));
    probed_block(rig, None);
    rig.acks();
    let data = sample(64);
    let sample_pointer = (!bytes).then_some(Arc::as_ptr(&data));
    let mut inbox = ByteInbox::new();
    let mut encoded = [0; 8192];
    if bytes {
        let n = ring::encode_song_sample_begin(pcm, 64, 1, 48_000, &mut encoded);
        assert!(n > 0 && inbox.push(&encoded[..n]));
        let n = ring::encode_song_slice(pcm, 0, &data.frames, &mut encoded);
        assert!(n > 0 && inbox.push(&encoded[..n]));
        probed_block(rig, Some(&mut inbox));
    } else {
        rig.tx
            .push(NativeRecord::SongInstall(NativeSongInstall {
                lease: pcm,
                payload: NativeInstall::Sample {
                    resource: pcm.resource.id,
                    gen: pcm.resource.generation,
                    data,
                },
            }))
            .ok()
            .unwrap();
        probed_block(rig, None);
    }
    let mut receipts = rig.acks();
    for _ in 0..4 {
        probed_block(rig, None);
        receipts.extend(rig.acks());
    }
    let def = super::acceptance::closure_inst(pcm.resource.id, CellId::new(42));
    let inst_pointer;
    if bytes {
        inst_pointer = None;
        let mut graph = Vec::new();
        encode_inst(&def, &mut graph).unwrap();
        let n = ring::encode_song_graph_record(inst, &graph, &mut encoded);
        assert!(n > 0 && inbox.push(&encoded[..n]));
        probed_block(rig, Some(&mut inbox));
    } else {
        let template = Template::from_inst(&def, &rig.engine.build_env()).unwrap();
        assert!(
            u64::try_from(template.mem_total).unwrap() <= rig.engine.song_voice_regions()[0].frames
        );
        inst_pointer = Some(std::ptr::from_ref(template.as_ref()));
        rig.tx
            .push(NativeRecord::SongInstall(NativeSongInstall {
                lease: inst,
                payload: NativeInstall::Inst {
                    resource: inst.resource.id,
                    gen: inst.resource.generation,
                    template,
                },
            }))
            .ok()
            .unwrap();
        probed_block(rig, None);
    }
    receipts.extend(rig.acks());
    for _ in 0..4 {
        probed_block(rig, None);
        receipts.extend(rig.acks());
    }
    for owner in [pcm, inst] {
        assert_eq!(
            receipts
                .iter()
                .filter(|ack| **ack
                    == SongHostAck::ResourceReady {
                        epoch: owner.epoch,
                        resource: owner.resource
                    })
                .count(),
            1
        );
    }
    assert!(!receipts
        .iter()
        .any(|ack| matches!(ack, SongHostAck::Rejected { .. })));
    rig.command(SongCommand::SealPreparation(pcm.epoch));
    probed_block(rig, None);
    assert_eq!(rig.acks(), vec![SongHostAck::Ready(pcm.epoch)]);
    assert!(rig.returned.pop().is_none());
    (owners, inst_pointer, sample_pointer)
}
fn fill_capacity_reports(rig: &mut Rig) -> [SnapshotEpoch; 4] {
    let epochs = [
        SnapshotEpoch(901),
        SnapshotEpoch(902),
        SnapshotEpoch(903),
        SnapshotEpoch(904),
    ];
    for epoch in epochs {
        rig.command(SongCommand::RequestCapacity(epoch));
        probed_block(rig, None);
    }
    assert_eq!(rig.received.len(), rig.acks.capacity());
    assert_eq!(rig.acks.capacity(), 4);
    assert_eq!(rig.engine.song_remaining_capacities().unwrap().ack_slots, 4);
    epochs
}
#[test]
fn native_full_reports_and_garbage_retirement_preserve_every_owner() {
    combined_pressure(false);
}
#[test]
fn arena_full_reports_and_garbage_retirement_preserve_every_owner() {
    combined_pressure(true);
}
fn combined_pressure(bytes: bool) {
    let mut rig = Rig::new(if bytes {
        StoreKind::Arena { bytes: 4096 }
    } else {
        StoreKind::NativeArc
    });
    let before = rig.engine.song_remaining_capacities().unwrap();
    let analysis_before = rig.engine.song_analysis_capacity().unwrap();
    let extents_before = rig.engine.song_pcm_extents().to_vec();
    let (owners, inst_pointer, sample_pointer) = pressure_adopt(&mut rig, bytes);
    let [pcm, inst, control, analysis] = owners;
    let charged = rig.engine.song_remaining_capacities().unwrap();
    let analysis_charged = rig.engine.song_analysis_capacity().unwrap();
    let analysis_region = rig.engine.song_analysis_region(analysis).unwrap();
    assert_eq!(analysis_region.frames, 64);
    let epochs = fill_capacity_reports(&mut rig);
    let marker = sample(1);
    let marker_pointer = Arc::as_ptr(&marker);
    rig.garbage.push(Garbage::Sample(marker)).ok().unwrap();
    rig.engine.cancel_song_preparation(pcm.epoch).unwrap();
    for _ in 0..3 {
        probed_block(&mut rig, None);
        assert_eq!(rig.engine.song_remaining_capacities().unwrap(), charged);
        assert_eq!(
            rig.engine.song_analysis_capacity().unwrap(),
            analysis_charged
        );
        assert_eq!(rig.received.len(), 4);
        assert_eq!(
            rig.engine
                .song_control_value(control, CellId::new(42))
                .unwrap(),
            0.5
        );
        assert_eq!(
            rig.engine.song_analysis_region(analysis),
            Err(SongRejectCode::StaleEpoch)
        );
        assert!(rig
            .engine
            .reserve_song_resource(SongResourceReservation {
                epoch: inst.epoch,
                resource: inst.resource,
                kind: inst.kind
            })
            .is_err());
    }
    let Some(Garbage::Sample(marker)) = rig.returned.pop() else {
        panic!("original owned marker")
    };
    assert_eq!(Arc::as_ptr(&marker), marker_pointer);
    drop(marker);
    probed_block(&mut rig, None);
    let mut returned = Vec::new();
    if let Some(owner) = rig.returned.pop() {
        returned.push(owner);
    }
    assert_eq!(returned.len(), usize::from(!bytes));
    if !bytes {
        let Garbage::SongInstall(owner) = &returned[0] else {
            panic!("instrument owner")
        };
        assert_eq!(owner.lease, inst);
        let NativeInstall::Inst { template, .. } = &owner.payload else {
            panic!("instrument allocation")
        };
        assert_eq!(Some(std::ptr::from_ref(template.as_ref())), inst_pointer);
    }
    for _ in 0..3 {
        probed_block(&mut rig, None);
        assert!(rig.returned.pop().is_none());
        assert_eq!(rig.engine.song_remaining_capacities().unwrap(), charged);
        assert_eq!(rig.received.len(), 4);
    }
    let mut messages = vec![rig.received.pop().expect("oldest capacity report")];
    probed_block(&mut rig, None);
    let mut after_instrument = charged;
    after_instrument.template_slots = after_instrument.template_slots.checked_add(1).unwrap();
    assert_eq!(
        rig.engine.song_remaining_capacities().unwrap(),
        after_instrument
    );
    assert_eq!(
        rig.engine.song_analysis_capacity().unwrap(),
        analysis_charged
    );
    assert_eq!(rig.received.len(), 4); // Instrument receipt was enqueued, not consumed.
    if let Some(owner) = rig.returned.pop() {
        returned.push(owner);
    }
    for _ in 0..2 {
        probed_block(&mut rig, None);
        assert_eq!(
            rig.engine.song_remaining_capacities().unwrap(),
            after_instrument
        );
        assert!(rig.returned.pop().is_none());
    }
    for _ in 0..40 {
        if let Some(message) = rig.received.pop() {
            messages.push(message);
        }
        probed_block(&mut rig, None);
        if let Some(owner) = rig.returned.pop() {
            returned.push(owner);
        }
    }
    assert!(rig.received.pop().is_none());
    assert!(rig.returned.pop().is_none());
    assert_eq!(messages.len(), 9);
    let mut serial = None;
    for (message, epoch) in messages[..4].iter().zip(epochs) {
        let HostMsg::Song(SongHostAck::CapacityReport(report)) = message else {
            panic!("actual capacity FIFO")
        };
        assert_eq!(report.epoch, epoch);
        assert_eq!(report.available, charged);
        assert_eq!(report.analysis, analysis_charged);
        if let Some(previous) = serial {
            assert!(report.serial > previous);
        }
        serial = Some(report.serial);
    }
    let order = [inst, pcm, control, analysis];
    for (message, owner) in messages[4..8].iter().zip(order) {
        assert_eq!(*message, HostMsg::Song(SongHostAck::LeaseReturned(owner)));
    }
    assert_eq!(
        messages[8],
        HostMsg::Song(SongHostAck::PreparationCancelled(pcm.epoch))
    );
    assert_eq!(returned.len(), if bytes { 0 } else { 2 });
    for owner in returned {
        let Garbage::SongInstall(owner) = owner else {
            panic!("original native owner")
        };
        match owner.payload {
            NativeInstall::Inst { template, .. } => {
                assert_eq!(owner.lease, inst);
                assert_eq!(Some(std::ptr::from_ref(template.as_ref())), inst_pointer);
            }
            NativeInstall::Sample { data, .. } => {
                assert_eq!(owner.lease, pcm);
                assert_eq!(Some(Arc::as_ptr(&data)), sample_pointer);
            }
            NativeInstall::Bus { .. } => panic!("no bus owner in this cohort"),
        }
    }
    assert!(rig
        .engine
        .song_control_cell(control, CellId::new(42))
        .is_err());
    assert!(rig.engine.song_analysis_region(analysis).is_err());
    assert!(rig.engine.store().get(pcm.resource.id).is_none());
    assert_eq!(rig.engine.song_remaining_capacities().unwrap(), before);
    assert_eq!(
        rig.engine.song_analysis_capacity().unwrap(),
        analysis_before
    );
    assert_eq!(rig.engine.song_pcm_extents(), extents_before);
}
