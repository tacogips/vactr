//! Real admitted neutral graph materialization and Native/Arena activation.
use std::sync::Arc;
use vactr::dsp::arena::{encode_bus, encode_inst, StoreKind};
use vactr::dsp::bus::BusTemplate;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::cells::AtomicCells;
use vactr::dsp::engine::{Engine, EngineConfig, SongStagingConfig};
use vactr::dsp::graph::{BusDef, BusId, InstDef};
use vactr::dsp::ring::{
    self, ByteInbox, EngineIo, EventRing, Garbage, NativeInstall, NativeRecord, NativeSongInstall,
    SpscRing,
};
use vactr::dsp::ugen::Template;
use vactr::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg};
use vactr::sched::slots::SlotId;
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::routing::*;
use vactr::song::snapshot::{FrozenControl, FrozenSongEvent};
use vactr::song::{PreparedSong, ResolvedNote, SnapshotEpoch, SongLimits};
use vactr::value::{intern::intern_kw, Ratio64};
use vactr::vm::fail::FailCode;

const PLAIN: &str = "inst tone:\n\tsin-osc 440\nsong {part [tone: {s :tone > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";
fn candidate(code: &str) -> PreparedSong {
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let context = vactr::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 32,
            max_pcm_bytes: 1_000_000,
            max_source_files: 16,
            max_source_bytes: 100_000,
            max_banks: 16,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    vactr::song::prepare_song(
        vactr::session::song::evaluate_song_candidate(
            code,
            "neutral-stages.vact",
            1,
            SnapshotEpoch(700),
            &context,
        )
        .unwrap(),
    )
    .unwrap()
}
struct Rig {
    engine: Engine,
    caps: CapabilitySet,
    tx: ring::Producer<NativeRecord>,
    controls: ring::Consumer<NativeRecord>,
    inbox: ByteInbox,
    events: ring::EventConsumer,
    acks: ring::AckProducer,
    received: ring::AckConsumer,
    garbage: ring::Producer<Garbage>,
    returned: ring::Consumer<Garbage>,
    cells: AtomicCells,
    bytes: bool,
    frame: u64,
    receipts: Vec<HostMsg>,
}
impl Rig {
    fn new(bytes: bool) -> Self {
        Self::with_bus_slots(bytes, 5)
    }
    fn with_bus_slots(bytes: bool, slots: usize) -> Self {
        let mut caps = CapabilitySet::browser();
        caps.max_voices = 1;
        let store = if bytes {
            StoreKind::Arena { bytes: 1_000_000 }
        } else {
            StoreKind::NativeArc
        };
        let mut config = EngineConfig::new(&caps, 48_000., 16, store);
        config.template_slots = 4;
        config.bus_slots = slots;
        config.voice_seconds = 1.;
        config.bus_seconds = 10.;
        config.event_capacity = 32;
        let mut engine = Engine::with_config(config);
        let (acks, received) = SpscRing::split(32);
        engine
            .configure_song_staging_for_transport(
                SongStagingConfig {
                    preparations: 2,
                    leases: 16,
                    branches: 2,
                    control_slots: 64,
                    analysis_slots: 64,
                    native_pcm_bytes: 1_000_000,
                    critical_receipts: 8,
                },
                &acks,
            )
            .unwrap();
        let (tx, controls) = SpscRing::split(32);
        let (_, events) = EventRing::split(16);
        let (garbage, returned) = SpscRing::split(16);
        Self {
            engine,
            caps,
            tx,
            controls,
            inbox: ByteInbox::new(),
            events,
            acks,
            received,
            garbage,
            returned,
            cells: AtomicCells::new(64),
            bytes,
            frame: 0,
            receipts: Vec::new(),
        }
    }
    fn command(&mut self, command: SongCommand) {
        if self.bytes {
            let mut encoded = [0; CtlMsg::MAX_LEN];
            let length = CtlMsg::Song(command).encode(&mut encoded);
            assert!(length > 0 && self.inbox.push(&encoded[..length]));
        } else {
            self.tx
                .push(NativeRecord::Msg(CtlMsg::Song(command)))
                .ok()
                .unwrap();
        }
    }
    fn render(&mut self, frames: usize) -> Vec<f32> {
        let mut output = vec![0.; frames * 2];
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
        self.frame += u64::try_from(frames).unwrap();
        while let Some(message) = self.received.pop() {
            self.receipts.push(message);
        }
        while let Some(owner) = self.returned.pop() {
            drop(owner);
        }
        assert!(self.engine.pop_fault().is_none(), "real callback fault");
        output
    }
    fn upload(&mut self, lease: SongLeaseKey, instrument: Option<&InstDef>, bus: Option<&BusDef>) {
        if self.bytes {
            let mut graph = Vec::new();
            if let Some(definition) = instrument {
                encode_inst(definition, &mut graph).unwrap();
            } else {
                encode_bus(
                    bus.unwrap(),
                    lease.kind == SongResourceKind::Master,
                    &mut graph,
                )
                .unwrap();
            }
            let mut encoded = vec![0; graph.len() + 64];
            let length = ring::encode_song_graph_record(lease, &graph, &mut encoded);
            assert!(length > 0 && self.inbox.push(&encoded[..length]));
        } else {
            let payload = if let Some(definition) = instrument {
                NativeInstall::Inst {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    template: Template::from_inst(definition, &self.engine.build_env()).unwrap(),
                }
            } else {
                NativeInstall::Bus {
                    resource: lease.resource.id,
                    gen: lease.resource.generation,
                    master: lease.kind == SongResourceKind::Master,
                    template: Box::new(BusTemplate::from_def(bus.unwrap()).unwrap()),
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
        for _ in 0..4 {
            assert!(self.render(16).iter().all(|v| *v == 0.));
        }
        assert!(self
            .receipts
            .contains(&HostMsg::Song(SongHostAck::ResourceReady {
                epoch: lease.epoch,
                resource: lease.resource
            })));
    }
}
fn route_plan(song: &PreparedSong, rig: &Rig) -> SongRoutePlan {
    prepare_routes(
        song.snapshot(),
        &rig.caps,
        &rig.engine.song_remaining_capacities().unwrap(),
    )
    .unwrap()
}
fn assert_tone_families(plan: &SongRoutePlan, tracks: usize) {
    use vactr::song::snapshot::FrozenSound;
    assert_eq!(plan.tracks.len(), tracks);
    assert_eq!(plan.branches.len(), 2 * tracks);
    assert_eq!(
        plan.max_live_generations,
        u32::try_from(2 * tracks).unwrap()
    );
    assert_eq!(
        plan.required.bus_slots,
        u32::try_from(3 * tracks + 1).unwrap()
    );
    for track in &plan.tracks {
        let branches: Vec<_> = plan
            .branches
            .iter()
            .filter(|b| b.track == track.track)
            .collect();
        assert_eq!(branches.len(), 2);
        let id = branches[0].resolved_instrument;
        assert!(branches
            .iter()
            .all(|b| b.resolved_instrument == id && b.reserved_generations == 1));
        assert_eq!(
            branches
                .iter()
                .filter(|b| b.instrument == FrozenSound::Builtin(intern_kw("tone")))
                .count(),
            1
        );
        assert_eq!(
            branches
                .iter()
                .filter(|b| b.instrument == FrozenSound::Instrument(id))
                .count(),
            1
        );
        assert_ne!(branches[0].id, branches[1].id);
    }
}
fn named_candidate() -> PreparedSong {
    candidate("inst tone:\n\tsin-osc 440\nbus :private:\n\tgain 0.75\nbus :tone:\n\tgain 0.5\nmaster:\n\tgain 0.9\nlet base {part [tone: {s :tone}] duration: 1}\nsong {instrument-fx base :tone :tone :private} tail-seconds: 0 > play-song")
}
#[test]
fn neutral_stage_graphs_compile_without_changing_admission() {
    let song = candidate(PLAIN);
    let rig = Rig::new(false);
    let plan = route_plan(&song, &rig);
    let before = (
        plan.required,
        plan.required_bytes,
        plan.branch_delay_frames,
        plan.max_live_generations,
    );
    let stages = materialize_route_buses(&plan, SongLimits::default()).unwrap();
    assert_eq!(stages.len(), plan.branches.len() + plan.tracks.len() + 1);
    assert_eq!(
        stages[0].target,
        SongDetectorTarget::Branch(plan.branches[0].id)
    );
    assert_tone_families(&plan, 1);
    assert_eq!(
        stages[1].target,
        SongDetectorTarget::Branch(plan.branches[1].id)
    );
    assert_eq!(
        stages[2].target,
        SongDetectorTarget::Track(plan.tracks[0].track)
    );
    assert_eq!(stages[3].target, SongDetectorTarget::Master);
    assert!(plan.branches.iter().all(|b| b.effect_template.is_none()));
    assert!(plan.tracks.iter().all(|t| t.template.is_none()));
    assert!(plan.master.is_none());
    assert!(plan.topology.buses.is_empty());
    for (index, stage) in stages.iter().enumerate() {
        assert!(stage.template.chain.is_empty());
        assert_eq!(BusTemplate::from_def(&stage.template).unwrap().n, 0);
        assert!(stages[..index]
            .iter()
            .all(|earlier| earlier.template.id != stage.template.id));
    }
    assert_eq!(
        before,
        (
            plan.required,
            plan.required_bytes,
            plan.branch_delay_frames,
            plan.max_live_generations
        )
    );
    assert!(
        plan.branch_delay_frames > 0,
        "existing private delay admission is preserved"
    );
    let room = vactr::dsp::effects::mem_len(vactr::dsp::graph::EffectKind::Room, 48_000., &rig.caps)
        as u64;
    assert_eq!(
        plan.required.bus_frames,
        u64::from(plan.required.bus_slots) * room * 4 + plan.branch_delay_frames
    );
}
#[test]
fn explicit_graph_arcs_and_distinct_target_ownership_are_preserved() {
    let song = named_candidate();
    let rig = Rig::new(false);
    let mut plan = route_plan(&song, &rig);
    let explicit = materialize_route_buses(&plan, SongLimits::default()).unwrap();
    assert_tone_families(&plan, 1);
    for stage in &explicit {
        let original = match stage.target {
            SongDetectorTarget::Branch(id) => plan
                .branches
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .effect_template
                .and_then(|name| {
                    plan.topology
                        .buses
                        .iter()
                        .find_map(|(key, graph)| (*key == Some(name)).then_some(graph))
                }),
            SongDetectorTarget::Track(track) => plan
                .tracks
                .iter()
                .find(|t| t.track == track)
                .unwrap()
                .template
                .as_ref(),
            SongDetectorTarget::Master => plan.master.as_ref(),
        };
        if let Some(original) = original {
            assert!(!stage.template.chain.is_empty());
            assert!(Arc::ptr_eq(original, &stage.template));
        } else {
            assert!(stage.template.chain.is_empty());
        }
    }
    let shared_song = candidate("inst tone:\n\tsin-osc 440\nbus :private:\n\tgain 0.75\nbus :left:\n\tgain 0.5\nbus :right:\n\tgain 0.6\nmaster:\n\tgain 0.9\nlet base {part [left: {s :tone} right: {s :tone}] duration: 1}\nlet left {instrument-fx base :left :tone :private}\nsong {instrument-fx left :right :tone :private} tail-seconds: 0 > play-song");
    let shared_rig = Rig::with_bus_slots(false, 8);
    let mut shared_plan = route_plan(&shared_song, &shared_rig);
    assert_tone_families(&shared_plan, 2);
    let shared = materialize_route_buses(&shared_plan, SongLimits::default()).unwrap();
    let named: Vec<_> = shared
        .iter()
        .filter(|stage| match stage.target {
            SongDetectorTarget::Branch(id) => shared_plan
                .branches
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .effect_template
                .is_some(),
            _ => false,
        })
        .collect();
    assert_eq!(named.len(), 2);
    assert_ne!(named[0].target, named[1].target);
    let named_tracks: Vec<_> = named
        .iter()
        .map(|stage| {
            let SongDetectorTarget::Branch(id) = stage.target else {
                unreachable!()
            };
            shared_plan
                .branches
                .iter()
                .find(|branch| branch.id == id)
                .unwrap()
                .track
        })
        .collect();
    assert_ne!(
        named_tracks[0], named_tracks[1],
        "same explicit graph belongs to distinct admitted tracks"
    );
    assert!(Arc::ptr_eq(&named[0].template, &named[1].template));
    // Malformed public copied-plan negatives, distinct from genuine admission.
    shared_plan.branches[1].id = shared_plan.branches[0].id;
    assert_eq!(
        materialize_route_buses(&shared_plan, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
    plan.branches[0].effect_template = Some(intern_kw("not-declared"));
    assert_eq!(
        materialize_route_buses(&plan, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
}
#[test]
fn stage_materialization_has_checked_ids_and_cumulative_work() {
    let rig = Rig::new(false);
    let mut plan = route_plan(&named_candidate(), &rig);
    assert_tone_families(&plan, 1);
    // Public copied DTO boundary: assign an explicit named graph to every
    // branch for all-explicit MAX, preserving actual candidate separately.
    for branch in &mut plan.branches {
        branch.effect_template = Some(intern_kw("private"));
    }
    // Graph contents/typed targets remain; candidate-local IDs exercise limits.
    for (_, graph) in &mut plan.topology.buses {
        Arc::make_mut(graph).id = BusId::new(u32::MAX);
    }
    assert!(
        materialize_route_buses(&plan, SongLimits::default()).is_ok(),
        "all explicit needs no successor"
    );
    plan.master = None;
    assert_eq!(
        materialize_route_buses(&plan, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Overflow
    );
    for (_, graph) in &mut plan.topology.buses {
        Arc::make_mut(graph).id = BusId::new(u32::MAX - 1);
    }
    let materialized = materialize_route_buses(&plan, SongLimits::default()).unwrap();
    assert_eq!(materialized.last().unwrap().template.id.get(), u32::MAX);
    let minimum = (1..=4096)
        .find(|&nodes| {
            match materialize_route_buses(
                &plan,
                SongLimits {
                    max_nodes: nodes,
                    ..SongLimits::default()
                },
            ) {
                Ok(stages) => {
                    assert_eq!(stages.len(), materialized.len());
                    true
                }
                Err(error) => {
                    assert_eq!(error.code, FailCode::FuelExhausted);
                    false
                }
            }
        })
        .unwrap();
    let first_lookup: usize = plan
        .branches
        .iter()
        .map(|branch| {
            plan.topology
                .buses
                .iter()
                .position(|(name, _)| *name == branch.effect_template)
                .unwrap()
                + 1
        })
        .sum();
    let branches = plan.branches.len();
    let tracks = plan.tracks.len();
    let expected_work = 4 * materialized.len()
        + branches
        + branches * (branches - 1) / 2
        + first_lookup
        + tracks
        + tracks * (tracks - 1) / 2
        + plan.topology.buses.len()
        + tracks
        + 3
        + branches * plan.topology.buses.len();
    assert_eq!(
        minimum,
        u32::try_from(expected_work).unwrap(),
        "all actual scans plus preadmitted final lookup/storage"
    );
    let mut extra = plan.clone();
    extra.topology.buses.insert(
        0,
        (
            Some(intern_kw("extra-before")),
            Arc::new(BusDef {
                id: BusId::new(1),
                chain: Box::new([]),
            }),
        ),
    );
    assert_eq!(
        materialize_route_buses(
            &extra,
            SongLimits {
                max_nodes: minimum + u32::try_from(2 * branches).unwrap(),
                ..SongLimits::default()
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    assert!(
        materialize_route_buses(
            &extra,
            SongLimits {
                max_nodes: minimum + u32::try_from(2 * branches + 1).unwrap(),
                ..SongLimits::default()
            }
        )
        .is_ok(),
        "extra graph charges each branch validation/final lookup plus highwater"
    );
    let mut missing_late = plan.clone();
    let mut late = missing_late.branches[0].clone();
    late.id = SongBranchId(999);
    late.effect_template = Some(intern_kw("late-missing"));
    missing_late.branches.push(late);
    assert_eq!(
        materialize_route_buses(&missing_late, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert_eq!(
        materialize_route_buses(
            &missing_late,
            SongLimits {
                max_nodes: 1,
                ..SongLimits::default()
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    assert_eq!(
        materialize_route_buses(
            &plan,
            SongLimits {
                max_nodes: minimum - 1,
                ..SongLimits::default()
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    let mut duplicate = plan.clone();
    duplicate.tracks.push(duplicate.tracks[0].clone());
    assert_eq!(
        materialize_route_buses(&duplicate, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
}
fn lease(id: u32, kind: SongResourceKind) -> SongLeaseKey {
    SongLeaseKey {
        epoch: SnapshotEpoch(700),
        resource: SongResourceRef { id, generation: 9 },
        kind,
    }
}
fn number(value: ResolvedNote) -> f32 {
    match value {
        ResolvedNote::Int(n) => n as f32,
        ResolvedNote::Ratio(n) => n.to_f64() as f32,
        ResolvedNote::Float32(n) => n,
        ResolvedNote::Float64(n) => n as f32,
    }
}
fn event_from_row(row: &FrozenSongEvent, instrument: &InstDef) -> AudioEvent {
    let mut event = AudioEvent::new(0., SlotId::new(0), 1, instrument.id);
    for &(name, ref value) in &row.controls {
        let FrozenControl::Number(n) = value else {
            panic!("fixture control is not a frozen numeric constant")
        };
        event
            .push_ctl(
                vactr::dsp::controls::row(vactr::value::intern::name_of_kw(name).as_ref())
                    .unwrap()
                    .ctl,
                Ctl::Const(number(*n)),
            )
            .unwrap();
    }
    event
}
#[test]
fn materialized_neutral_stages_activate_real_native_and_arena_audio() {
    let mut native_output: Option<Vec<f32>> = None;
    for bytes in [false, true] {
        let mut rig = Rig::new(bytes);
        let mut song = candidate(PLAIN);
        let available = rig.engine.song_remaining_capacities().unwrap();
        let plan = route_plan(&song, &rig);
        assert_tone_families(&plan, 1);
        let stages = materialize_route_buses(&plan, SongLimits::default()).unwrap();
        let regions: Vec<_> = rig
            .engine
            .song_bus_regions()
            .iter()
            .filter(|r| r.frames > 0)
            .collect();
        assert_eq!(regions.len(), available.bus_slots as usize);
        assert_eq!(
            regions.iter().map(|r| r.frames).sum::<u64>(),
            available.bus_frames
        );
        let room =
            vactr::dsp::effects::mem_len(vactr::dsp::graph::EffectKind::Room, 48_000., &rig.caps)
                as u64;
        assert!(regions.iter().all(|r| r.frames >= room * 4));
        assert_eq!(plan.max_live_generations, 2);
        assert_eq!(
            plan.branch_delay_frames % u64::from(plan.max_live_generations),
            0
        );
        let private_need =
            room * 4 + plan.branch_delay_frames / u64::from(plan.max_live_generations);
        assert!(
            regions.iter().any(|r| r.frames >= private_need),
            "measured single private slot must fit room plus unchanged priced delay"
        );
        for stage in &stages {
            let compiled = BusTemplate::from_def(&stage.template).unwrap();
            assert_eq!(compiled.n, 0, "actual empty chain remains neutral");
        }
        // The real physical regions fit aggregate admitted room+delay demand.
        // This is no assertion that today's runtime implements nonzero delay.
        assert!(available.bus_frames >= plan.required.bus_frames);
        assert_eq!(available.bus_frames % u64::from(available.bus_slots), 0);
        let graph = &plan
            .topology
            .instruments
            .iter()
            .find(|i| i.graph.id == plan.branches[0].resolved_instrument)
            .unwrap()
            .graph;
        let compiled = Template::from_inst(graph, &rig.engine.build_env()).unwrap();
        assert!(rig
            .engine
            .song_voice_regions()
            .iter()
            .all(|r| r.frames >= compiled.mem_total as u64));
        let instruments: Vec<_> = plan
            .branches
            .iter()
            .enumerate()
            .map(|(index, _)| {
                lease(
                    101 + u32::try_from(index).unwrap(),
                    SongResourceKind::Instrument,
                )
            })
            .collect();
        let buses: Vec<_> = stages
            .iter()
            .enumerate()
            .map(|(index, stage)| {
                let kind = match stage.target {
                    SongDetectorTarget::Branch(_) => SongResourceKind::PrivateFx,
                    SongDetectorTarget::Track(_) => SongResourceKind::Track,
                    SongDetectorTarget::Master => SongResourceKind::Master,
                };
                lease(
                    101 + u32::try_from(instruments.len() + index).unwrap(),
                    kind,
                )
            })
            .collect();
        assert_eq!(instruments.len(), plan.required.template_slots as usize);
        assert_eq!(buses.len(), plan.required.bus_slots as usize);
        rig.command(SongCommand::BeginStaging(SongStagePreparation {
            preparation: SongPreparation {
                epoch: song.snapshot().epoch(),
                branches: plan.branches.len() as u32,
                resources: (instruments.len() + buses.len()) as u32,
                required: plan.required,
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        }));
        for owner in instruments.iter().chain(&buses).copied() {
            rig.command(SongCommand::ReserveResource(SongResourceReservation {
                epoch: owner.epoch,
                resource: owner.resource,
                kind: owner.kind,
            }));
        }
        assert!(rig.render(16).iter().all(|v| *v == 0.));
        for owner in &instruments {
            rig.upload(*owner, Some(graph), None);
        }
        for (stage, owner) in stages.iter().zip(&buses) {
            rig.upload(*owner, None, Some(&stage.template));
        }
        let track = buses[plan.branches.len()];
        let master = *buses.last().unwrap();
        for (index, branch) in plan.branches.iter().enumerate() {
            assert_ne!(instruments[index], buses[index]);
            rig.command(SongCommand::ConfigureBranch(SongBranchConfig {
                epoch: song.snapshot().epoch(),
                branch: branch.id,
                generation: 1,
                family: u32::try_from(index).unwrap(),
                track: track.resource.id,
                instrument: instruments[index].resource,
                private_fx: Some(buses[index].resource),
                track_template: Some(track.resource),
                master: Some(master.resource),
                transition_frame: 0,
                tail_deadline: u64::MAX,
            }));
        }
        rig.command(SongCommand::SealPreparation(song.snapshot().epoch()));
        assert!(rig.render(16).iter().all(|v| *v == 0.));
        assert!(rig
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Ready(song.snapshot().epoch()))));
        let row = song
            .query(
                vactr::pattern::TimeSpan::cycle(0).unwrap(),
                &SongLimits::default(),
            )
            .unwrap()
            .remove(0);
        let routed = resolve_route(&plan, &row, SongLimits::default()).unwrap();
        let chosen = plan
            .branches
            .iter()
            .find(|branch| branch.id == routed.branch)
            .unwrap();
        assert_eq!(chosen.instrument, row.instrument);
        assert_eq!(chosen.resolved_instrument, graph.id);
        assert_eq!(row.handle.occurrence().onset, Ratio64::ZERO);
        let activation = SongActivation {
            epoch: song.snapshot().epoch(),
            frame: rig.frame,
        };
        rig.command(SongCommand::Activate(activation));
        rig.command(SongCommand::Event(SongAudioEvent {
            epoch: activation.epoch,
            branch: routed.branch,
            generation: 1,
            frame: activation.frame,
            event: event_from_row(&row, graph),
        }));
        let output = if bytes {
            let mut output = rig.render(7);
            output.extend(rig.render(121));
            output
        } else {
            rig.render(128)
        };
        if let Some(native) = &native_output {
            assert_eq!(native.len(), output.len());
            assert!(
                native
                    .iter()
                    .zip(&output)
                    .all(|(left, right)| (left - right).abs() < 1e-6),
                "actual Native/Arena callback partition parity"
            );
        } else {
            native_output = Some(output.clone());
        }
        assert!(output.iter().all(|value| value.is_finite()));
        assert!(
            output.iter().filter(|&&value| value.abs() > 1e-5).count() > 50,
            "bytes={bytes}, actual silent neutral stage output"
        );
        assert!(rig
            .receipts
            .contains(&HostMsg::Song(SongHostAck::Applied(activation))));
        assert!(rig
            .receipts
            .iter()
            .all(|message| !matches!(message, HostMsg::Song(SongHostAck::Rejected { .. }))));
    }
}
