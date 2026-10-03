//! Real first-increment song audio; full mute/detector/tail acceptance remains separate.
use std::sync::Arc;
use vactr::dsp::arena::{encode_bus, encode_inst, StoreKind};
use vactr::dsp::bus::{BusTemplate, SlotState};
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::cells::{AtomicCells, CellId};
use vactr::dsp::engine::{Engine, EngineConfig, SongStagingConfig};
use vactr::dsp::graph::{
    AnalyzerKind, BankRef, BusDef, BusId, Edge, EffectKind, EffectSpec, InstDef, InstId, UGenSpec,
};
use vactr::dsp::ring::{
    self, ByteInbox, EngineIo, EventRing, Garbage, NativeInstall, NativeRecord, NativeSongInstall,
    SpscRing,
};
use vactr::dsp::ugen::{Template, AMP, BANK};
use vactr::host::caps::SampleData;
use vactr::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg};
use vactr::sched::slots::SlotId;
use vactr::song::routing::*;
use vactr::song::SnapshotEpoch;

#[path = "song_dsp/delay_geometry.rs"]
mod delay_geometry;
#[path = "song_dsp/late_mute.rs"]
mod late_mute;
#[path = "song_dsp/lifecycle.rs"]
mod lifecycle;

struct Probe;
thread_local! { static CALLBACK_MEMORY: std::cell::Cell<Option<(usize,usize)>> = const { std::cell::Cell::new(None) }; }
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a + 1, d)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a, d + 1)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        CALLBACK_MEMORY.with(|c| {
            if let Some((a, d)) = c.get() {
                c.set(Some((a + 1, d + 1)));
            }
        });
        unsafe { std::alloc::GlobalAlloc::realloc(&std::alloc::System, pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;

fn key(id: u32, kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(11),
        resource: SongResourceRef { id, generation: 5 },
        kind,
    }
}
struct Rig {
    engine: Engine,
    tx: ring::Producer<NativeRecord>,
    controls: ring::Consumer<NativeRecord>,
    inbox: ByteInbox,
    bytes: bool,
    event_tx: ring::EventProducer,
    events: ring::EventConsumer,
    acks: ring::AckProducer,
    received: ring::AckConsumer,
    garbage: ring::Producer<Garbage>,
    returned: ring::Consumer<Garbage>,
    cells: AtomicCells,
    master_tail: bool,
    receipts: Vec<HostMsg>,
    frame: u64,
}
impl Rig {
    fn new(bytes: bool, ack_slots: usize) -> Self {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 4;
        let mut config = EngineConfig::new(
            &caps,
            48_000.,
            16,
            if bytes {
                StoreKind::Arena { bytes: 16384 }
            } else {
                StoreKind::NativeArc
            },
        );
        config.template_slots = 5;
        config.bus_slots = 8;
        config.voice_seconds = 0.25;
        delay_geometry::configure_private_fixture_regions(&mut config, &caps, 48_000).unwrap();
        config.event_capacity = 32;
        let mut engine = Engine::with_config(config);
        let (acks, received) = SpscRing::split(ack_slots);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 2,
                    leases: 32,
                    branches: 4,
                    control_slots: 8,
                    analysis_slots: 128,
                    native_pcm_bytes: 16384,
                    critical_receipts: 1,
                },
                &acks,
            )
            .unwrap();
        let (tx, controls) = SpscRing::split(64);
        let (event_tx, events) = EventRing::split(8);
        let (garbage, returned) = SpscRing::split(8);
        let cells = AtomicCells::new(64);
        assert!(cells.set(CellId::new(42), 0.2));
        assert!(cells.set(CellId::new(0), 99.));
        assert!(cells.set(CellId::new(1), 99.));
        Self {
            engine,
            tx,
            controls,
            inbox: ByteInbox::new(),
            bytes,
            event_tx,
            events,
            acks,
            received,
            garbage,
            returned,
            cells,
            master_tail: false,
            receipts: Vec::new(),
            frame: 0,
        }
    }
    fn command(&mut self, command: SongCommand) {
        if self.bytes {
            let mut data = [0; CtlMsg::MAX_LEN];
            let n = CtlMsg::Song(command).encode(&mut data);
            assert!(n > 0 && self.inbox.push(&data[..n]));
        } else {
            self.tx
                .push(NativeRecord::Msg(CtlMsg::Song(command)))
                .ok()
                .unwrap();
        }
    }
    fn process(&mut self, frames: usize, drain: bool) -> Vec<f32> {
        let mut output = vec![0.; frames * 2];
        CALLBACK_MEMORY.with(|c| {
            assert!(c.get().is_none());
            c.set(Some((0, 0)));
        });
        if self.bytes {
            self.engine.process(
                &mut EngineIo {
                    events: &mut self.events,
                    controls: &mut self.inbox,
                    acks: &mut self.acks,
                    cells: &mut self.cells,
                    garbage: Some(&mut self.garbage),
                },
                &mut output,
                frames,
            );
        } else {
            self.engine.process(
                &mut EngineIo {
                    events: &mut self.events,
                    controls: &mut self.controls,
                    acks: &mut self.acks,
                    cells: &mut self.cells,
                    garbage: Some(&mut self.garbage),
                },
                &mut output,
                frames,
            );
        }
        let counts = CALLBACK_MEMORY.with(|c| c.replace(None).unwrap());
        assert_eq!(counts, (0, 0), "actual callback allocation/destruction");
        self.frame += frames as u64;
        if drain {
            self.drain();
        }
        output
    }
    fn drain(&mut self) {
        while let Some(message) = self.received.pop() {
            self.receipts.push(message);
        }
        while let Some(owner) = self.returned.pop() {
            drop(owner);
        }
    }
    fn song_graph(&mut self, lease: SongLeaseKey, inst: Option<&InstDef>) {
        let mut bus = BusDef {
            id: BusId::new(lease.resource.id),
            chain: Box::new([]),
        };
        if self.master_tail && lease.kind == SongResourceKind::Master {
            let kind = EffectKind::SvfFilter;
            bus.chain = vec![EffectSpec {
                kind,
                params: [("cutoff", 100.), ("q", 5.), ("mix", 1.)]
                    .into_iter()
                    .map(|(name, v)| {
                        (
                            vactr::dsp::effects::param_ctl(kind, name).unwrap(),
                            Ctl::Const(v),
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }]
            .into_boxed_slice();
        }
        if self.bytes {
            let mut graph = Vec::new();
            if let Some(def) = inst {
                encode_inst(def, &mut graph).unwrap();
            } else {
                encode_bus(&bus, lease.kind == SongResourceKind::Master, &mut graph).unwrap();
            }
            let mut data = [0; 8192];
            let n = ring::encode_song_graph_record(lease, &graph, &mut data);
            assert!(n > 0 && self.inbox.push(&data[..n]));
        } else {
            let payload = if let Some(def) = inst {
                NativeInstall::Inst {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    template: Template::from_inst(def, &self.engine.build_env()).unwrap(),
                }
            } else {
                NativeInstall::Bus {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    master: lease.kind == SongResourceKind::Master,
                    template: Box::new(BusTemplate::from_def(&bus).unwrap()),
                }
            };
            self.tx
                .push(NativeRecord::SongInstall(NativeSongInstall {
                    lease,
                    payload,
                }))
                .ok()
                .unwrap();
        }
        for _ in 0..3 {
            assert!(self.process(16, true).iter().all(|v| *v == 0.));
        }
        assert!(self
            .receipts
            .contains(&HostMsg::Song(SongHostAck::ResourceReady {
                epoch: lease.epoch,
                resource: lease.resource
            })));
    }
    fn prepared(bytes: bool, ack_slots: usize) -> Self {
        Self::prepared_family(bytes, ack_slots, false)
    }
    fn prepared_family(bytes: bool, ack_slots: usize, three: bool) -> Self {
        Self::prepared_case(bytes, ack_slots, three, false)
    }
    fn prepared_case(bytes: bool, ack_slots: usize, three: bool, master_tail: bool) -> Self {
        let mut r = Self::new(bytes, ack_slots);
        r.master_tail = master_tail;
        let mut owners = vec![
            key(1, SongResourceKind::Instrument),
            key(2, SongResourceKind::Instrument),
            key(3, SongResourceKind::PrivateFx),
            key(4, SongResourceKind::PrivateFx),
            key(5, SongResourceKind::Track),
            key(6, SongResourceKind::Master),
            key(7, SongResourceKind::Sample),
            key(8, SongResourceKind::ControlCells),
            key(9, SongResourceKind::ControlCells),
            key(10, SongResourceKind::AnalysisBank),
            key(11, SongResourceKind::AnalysisBank),
        ];
        if three {
            owners.extend([
                key(12, SongResourceKind::Instrument),
                key(13, SongResourceKind::PrivateFx),
                key(14, SongResourceKind::Track),
            ]);
        }
        r.engine
            .begin_song_preparation(SongStagePreparation {
                preparation: SongPreparation {
                    epoch: SnapshotEpoch(11),
                    branches: if three { 3 } else { 2 },
                    resources: owners.len() as u32,
                    required: SongHostCapacities {
                        sample_rate: 48000,
                        voice_slots: 2,
                        ..SongHostCapacities::default()
                    },
                },
                analysis_required: SongAnalysisCapacity { slots: 128 },
            })
            .unwrap();
        for owner in owners.iter().copied() {
            r.engine
                .reserve_song_resource(SongResourceReservation {
                    epoch: owner.epoch,
                    resource: owner.resource,
                    kind: owner.kind,
                })
                .unwrap();
        }
        for (i, value) in [(0, 0.25), (1, 0.75)] {
            let bank = owners[7 + i];
            r.command(SongCommand::InitCells(SongCellInit {
                lease: bank,
                cell: CellId::new(42),
                value,
            }));
            r.command(SongCommand::InitCells(SongCellInit {
                lease: bank,
                cell: CellId::new(43),
                value: 7.,
            }));
            r.command(SongCommand::ReserveAnalysis(SongAnalysisReservation {
                lease: owners[9 + i],
                slots: 64,
            }));
            r.command(SongCommand::BindGraphBanks(SongGraphBanks {
                graph: owners[i],
                controls: Some(bank),
                analysis: Some(owners[9 + i]),
            }));
        }
        for _ in 0..4 {
            r.process(16, true);
        }
        let pcm_before_upload = r.engine.song_remaining_capacities().unwrap().pcm_bytes;
        let pcm = Arc::new(SampleData {
            rate: 48000,
            channels: 1,
            frames: vec![0.5; 4096].into_boxed_slice(),
        });
        if bytes {
            let mut data = vec![0; 17000];
            let n = ring::encode_song_sample_begin(owners[6], 4096, 1, 48000, &mut data);
            assert!(r.inbox.push(&data[..n]));
            r.process(16, true);
            for (i, slice) in pcm.frames.chunks(128).enumerate() {
                let n = ring::encode_song_slice(owners[6], (i * 128) as u32, slice, &mut data);
                assert!(r.inbox.push(&data[..n]));
                r.process(16, true);
            }
            // Final SliceAccepted precedes the retained ResourceReady followup.
            // Pump real callbacks with a bounded completion allowance, never fabricate readiness.
            for _ in 0..4 {
                if r.receipts
                    .contains(&HostMsg::Song(SongHostAck::ResourceReady {
                        epoch: owners[6].epoch,
                        resource: owners[6].resource,
                    }))
                {
                    break;
                }
                assert!(r.process(16, true).iter().all(|value| *value == 0.));
            }
        } else {
            r.tx.push(NativeRecord::SongInstall(NativeSongInstall {
                lease: owners[6],
                payload: NativeInstall::Sample {
                    resource: 7,
                    gen: 5,
                    data: pcm,
                },
            }))
            .ok()
            .unwrap();
            for _ in 0..4 {
                r.process(16, true);
            }
        }
        assert!(r.receipts.contains(&HostMsg::Song(SongHostAck::ResourceReady {
            epoch: owners[6].epoch, resource: owners[6].resource,
        })), "bytes={bytes}, pcm_before_upload={pcm_before_upload}, receipts={:?}, fault={:?}, remaining={:?}",
            r.receipts, r.engine.pop_fault(), r.engine.song_remaining_capacities());
        for (i, owner) in owners[..2].iter().copied().enumerate() {
            r.song_graph(owner, Some(&instrument(i == 0)));
        }
        for owner in owners[2..6].iter().copied() {
            r.song_graph(owner, None);
        }
        for i in 0..2 {
            r.engine
                .stage_song_branch(SongBranchConfig {
                    epoch: SnapshotEpoch(11),
                    branch: SongBranchId(i as u32),
                    generation: 17,
                    family: i as u32,
                    track: 5,
                    instrument: owners[i].resource,
                    private_fx: Some(owners[2 + i].resource),
                    track_template: Some(owners[4].resource),
                    master: Some(owners[5].resource),
                    transition_frame: 0,
                    tail_deadline: u64::MAX,
                })
                .unwrap();
        }
        if three {
            r.command(SongCommand::BindGraphBanks(SongGraphBanks {
                graph: owners[11],
                controls: Some(owners[8]),
                analysis: Some(owners[10]),
            }));
            r.process(16, true);
            let mut def = instrument(false);
            def.id = InstId::new(303);
            r.song_graph(owners[11], Some(&def));
            r.song_graph(owners[12], None);
            r.song_graph(owners[13], None);
            r.engine
                .stage_song_branch(SongBranchConfig {
                    epoch: SnapshotEpoch(11),
                    branch: SongBranchId(2),
                    generation: 17,
                    family: 1,
                    track: 14,
                    instrument: owners[11].resource,
                    private_fx: Some(owners[12].resource),
                    track_template: Some(owners[13].resource),
                    master: Some(owners[5].resource),
                    transition_frame: 0,
                    tail_deadline: u64::MAX,
                })
                .unwrap();
        }
        r.command(SongCommand::SealPreparation(SnapshotEpoch(11)));
        r.process(16, true);
        assert!(r
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Ready(SnapshotEpoch(11)))));
        r
    }
    fn event(&mut self, branch: u32, frame: u64) {
        let mut event = AudioEvent::new(0., SlotId::new(branch), 1, InstId::new(301 + branch));
        if branch == 0 {
            event.push_ctl(BANK, Ctl::Cell(CellId::new(43))).unwrap();
        }
        if branch != 0 {
            event
                .push_ctl(
                    vactr::dsp::controls::row("room").unwrap().ctl,
                    Ctl::Const(0.25),
                )
                .unwrap();
        }
        event
            .push_ctl(vactr::sched::slots::CtlId::new(49), Ctl::Const(0.05))
            .unwrap();
        self.command(SongCommand::Event(SongAudioEvent {
            epoch: SnapshotEpoch(11),
            branch: SongBranchId(branch),
            generation: 17,
            frame,
            event,
        }));
    }
}
fn instrument(pcm: bool) -> InstDef {
    InstDef {
        id: InstId::new(if pcm { 301 } else { 302 }),
        params: vec![(AMP, Ctl::Cell(CellId::new(42)))].into_boxed_slice(),
        nodes: vec![
            if pcm {
                UGenSpec::SamplePlay(BankRef::new(7))
            } else {
                UGenSpec::SinOsc
            },
            UGenSpec::Effect(EffectSpec {
                kind: EffectKind::Analyzer(AnalyzerKind::Level),
                params: vec![(
                    vactr::dsp::effects::param_ctl(EffectKind::Analyzer(AnalyzerKind::Level), "id")
                        .unwrap(),
                    Ctl::Const(0.),
                )]
                .into_boxed_slice(),
            }),
        ]
        .into_boxed_slice(),
        edges: vec![Edge {
            from: 0,
            to: 1,
            port: 0,
            output: 0,
        }]
        .into_boxed_slice(),
        node_params: Box::new([]),
    }
}

#[test]
fn native_and_arena_actual_pcm_and_oscillator_use_distinct_private_branches() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 8);
        let frame = r.frame;
        r.command(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(11),
            frame,
        }));
        r.event(0, frame);
        r.event(1, frame + 5);
        let output = r.process(16, true);
        assert!(output.iter().any(|v| v.abs() > 0.01), "real private audio");
        assert_eq!(r.engine.active_voices(), 2);
        assert!(r
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Applied(SongActivation {
                epoch: SnapshotEpoch(11),
                frame
            }))));
        assert!(
            r.engine.template(InstId::new(301)).is_none(),
            "private template never legacy live"
        );
        let fx0 = r
            .engine
            .buses()
            .slots
            .iter()
            .position(|s| s.resource == 3 && s.gen == 5 && s.state == SlotState::Live)
            .unwrap();
        let fx1 = r
            .engine
            .buses()
            .slots
            .iter()
            .position(|s| s.resource == 4 && s.gen == 5 && s.state == SlotState::Live)
            .unwrap();
        assert_ne!(fx0, fx1);
        assert!(r
            .engine
            .buses()
            .frames(fx0, 11)
            .0
            .iter()
            .any(|v| v.abs() > 0.01));
        assert!(r
            .engine
            .buses()
            .frames(fx1, 11)
            .0
            .iter()
            .any(|v| v.abs() > 0.01));
        assert_eq!(
            r.engine.cancel_song_preparation(SnapshotEpoch(11)),
            Err(SongRejectCode::NotReady)
        );
        assert_eq!(
            r.engine.cancel_song_preparation(SnapshotEpoch(99)),
            Err(SongRejectCode::StaleEpoch)
        );
        assert!(r.engine.store().get(7).is_none());
        let logical = r
            .engine
            .song_control_cell(key(8, SongResourceKind::ControlCells), CellId::new(43))
            .unwrap();
        assert_ne!(logical, CellId::new(43));
        assert!(
            r.engine.analysis().iter().all(|v| *v == 0.),
            "song analyzer never writes legacy bank"
        );
    }
}
#[test]
fn current_frame_activation_then_inside_quantum_onset_is_exact() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 8);
        let frame = r.frame;
        r.command(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(11),
            frame,
        }));
        r.event(0, frame + 5);
        let output = r.process(16, true);
        assert!(output[..10].iter().all(|v| *v == 0.));
        assert!(output[10..].iter().all(|v| v.abs() > 0.01));
        assert_eq!(
            r.engine
                .voices()
                .voices
                .iter()
                .find(|v| v.active)
                .unwrap()
                .start,
            (frame + 5) as f64 / 48000.
        );
    }
}
#[test]
fn same_frame_activation_onset_and_later_onset_are_partition_invariant() {
    for bytes in [false, true] {
        let mut a = Rig::prepared(bytes, 8);
        let mut b = Rig::prepared(bytes, 8);
        assert_eq!(a.frame, b.frame);
        let frame = a.frame;
        for r in [&mut a, &mut b] {
            r.command(SongCommand::Activate(SongActivation {
                epoch: SnapshotEpoch(11),
                frame,
            }));
            r.event(0, frame);
            r.event(1, frame + 5);
        }
        let whole = a.process(16, true);
        let mut split = b.process(5, true);
        split.extend(b.process(11, true));
        assert_eq!(whole, split);
    }
}
#[test]
fn actual_frame_applied_receipt_survives_full_ack_ring_once() {
    let mut r = Rig::prepared(false, 2);
    r.drain();
    let frame = r.frame;
    r.acks.push(HostMsg::CellBatchAck { seq: 90 }).unwrap();
    r.acks.push(HostMsg::CellBatchAck { seq: 91 }).unwrap();
    r.command(SongCommand::Activate(SongActivation {
        epoch: SnapshotEpoch(11),
        frame,
    }));
    r.event(0, frame);
    assert!(r.process(16, false).iter().any(|v| v.abs() > 0.01));
    r.drain();
    r.process(16, true);
    let applied = r
        .receipts
        .iter()
        .filter(|m| matches!(m, HostMsg::Song(SongHostAck::Applied(_))))
        .collect::<Vec<_>>();
    assert_eq!(applied.len(), 1);
    assert_eq!(
        *applied[0],
        HostMsg::Song(SongHostAck::Applied(SongActivation {
            epoch: SnapshotEpoch(11),
            frame
        }))
    );
}

#[test]
fn failed_event_under_receipt_pressure_retains_original_capacity_reason() {
    let mut r = Rig::prepared(false, 2);
    r.drain();
    let frame = r.frame;
    r.acks.push(HostMsg::CellBatchAck { seq: 92 }).unwrap();
    r.acks.push(HostMsg::CellBatchAck { seq: 93 }).unwrap();
    r.command(SongCommand::Activate(SongActivation {
        epoch: SnapshotEpoch(11),
        frame,
    }));
    for _ in 0..5 {
        r.event(0, frame);
    }
    assert!(r.process(16, false).iter().any(|v| v.abs() > 0.01));
    assert_eq!(r.engine.active_voices(), 4);
    r.drain();
    for _ in 0..3 {
        r.process(16, true);
    }
    let outcomes = r
        .receipts
        .iter()
        .filter(|m| {
            matches!(
                m,
                HostMsg::Song(SongHostAck::Applied(_) | SongHostAck::Rejected { .. })
            )
        })
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes,
        vec![
            HostMsg::Song(SongHostAck::Applied(SongActivation {
                epoch: SnapshotEpoch(11),
                frame
            })),
            HostMsg::Song(SongHostAck::Rejected {
                epoch: SnapshotEpoch(11),
                reason: SongRejectCode::Capacity
            })
        ]
    );
}
#[test]
fn concurrent_legacy_audio_cells_analyzers_and_slot_controls_remain_isolated() {
    for bytes in [false, true] {
        let mut a = Rig::prepared(bytes, 8);
        let mut b = Rig::prepared(bytes, 8);
        for r in [&mut a, &mut b] {
            let mut def = instrument(false);
            def.id = InstId::new(303);
            if bytes {
                let mut graph = Vec::new();
                encode_inst(&def, &mut graph).unwrap();
                let mut record = Vec::new();
                ring::encode_graph_record(99, 1, &graph, &mut record);
                assert!(r.inbox.push(&record));
            } else {
                r.tx.push(NativeRecord::Install(NativeInstall::Inst {
                    resource: 99,
                    gen: 1,
                    template: Template::from_inst(&def, &r.engine.build_env()).unwrap(),
                }))
                .ok()
                .unwrap();
            }
            r.process(16, true);
            assert!(r.engine.template(InstId::new(303)).is_some());
            let mut event =
                AudioEvent::new(r.frame as f64 / 48000., SlotId::new(0), 1, InstId::new(303));
            event
                .push_ctl(vactr::sched::slots::CtlId::new(49), Ctl::Const(0.05))
                .unwrap();
            r.event_tx.push(event).unwrap();
        }
        let frame = a.frame;
        assert_eq!(frame, b.frame);
        a.command(SongCommand::Activate(SongActivation {
            epoch: SnapshotEpoch(11),
            frame,
        }));
        a.event(0, frame);
        let both = a.process(16, true);
        let legacy = b.process(16, true);
        assert!(legacy.iter().any(|v| v.abs() > 0.001));
        assert!(both.iter().zip(&legacy).all(|(mix, old)| mix - old > 0.01));
        assert_eq!(
            a.engine.analysis(),
            b.engine.analysis(),
            "legacy analyzer bank unchanged by private writer"
        );
        assert!(a.engine.analysis().iter().any(|v| *v > 0.));
        let physical = a
            .engine
            .song_control_cell(key(8, SongResourceKind::ControlCells), CellId::new(42))
            .unwrap();
        assert!(a.cells.set(physical, 1000.));
        assert!(a.cells.set(CellId::new(42), 0.));
        assert!(b.cells.set(CellId::new(42), 0.));
        let both = a.process(16, true);
        let legacy = b.process(16, true);
        assert!(
            both.iter().zip(&legacy).all(|(mix, old)| mix - old > 0.01),
            "late legacy cell writes cannot change private amplitude"
        );
        let control = vactr::host::wire::SlotControl {
            slot: SlotId::new(0),
            new_gen: 2,
            effective_time: a.frame as f64 / 48000.,
            release: vactr::host::wire::Release::Panic,
        };
        let mut data = [0; CtlMsg::MAX_LEN];
        let n = CtlMsg::SlotControl(control).encode(&mut data);
        if bytes {
            assert!(a.inbox.push(&data[..n]));
        } else {
            a.tx.push(NativeRecord::Msg(CtlMsg::SlotControl(control)))
                .ok()
                .unwrap();
        }
        a.process(16, true);
        let song = a
            .engine
            .voices()
            .voices
            .iter()
            .find(|v| {
                v.active
                    && v.tmpl
                        != a.engine
                            .voices()
                            .voices
                            .iter()
                            .find(|v| v.active && v.slot == SlotId::new(0) && v.fade.is_some())
                            .unwrap()
                            .tmpl
            })
            .unwrap();
        assert!(
            song.fade.is_none(),
            "legacy slot panic cannot gate private voice"
        );
    }
}

#[test]
fn duplicate_activation_rejection_retains_requested_identity_and_active_audio() {
    for bytes in [false, true] {
        let mut r = Rig::prepared(bytes, 8);
        let first = SongActivation {
            epoch: SnapshotEpoch(11),
            frame: r.frame,
        };
        r.command(SongCommand::Activate(first));
        r.event(0, first.frame);
        assert!(r.process(16, true).iter().any(|v| v.abs() > 0.01));
        let duplicate = SongActivation {
            epoch: first.epoch,
            frame: r.frame + 5,
        };
        r.command(SongCommand::Activate(duplicate));
        assert!(r.process(16, true).iter().any(|v| v.abs() > 0.01));
        assert_eq!(
            r.receipts
                .iter()
                .filter(|m| **m
                    == HostMsg::Song(SongHostAck::ActivationRejected {
                        activation: duplicate,
                        reason: SongRejectCode::NotReady,
                    }))
                .count(),
            1
        );
        assert_eq!(
            r.receipts
                .iter()
                .filter(|m| **m == HostMsg::Song(SongHostAck::Applied(first)))
                .count(),
            1
        );
        assert!(!r
            .receipts
            .iter()
            .any(|m| matches!(m, HostMsg::Song(SongHostAck::Rejected { .. }))));
    }
}
