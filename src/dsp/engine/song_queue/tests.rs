//! Admission uses genuinely staged graphs, sealed preparation and registered slots.
use super::*;
use crate::dsp::arena::StoreKind;
use crate::dsp::bus::BusTemplate;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::graph::{BusDef, InstDef, UGenSpec};
use crate::dsp::ring::{NativeSongInstall, SpscRing};
use crate::song::routing::*;
use crate::song::SnapshotEpoch;

fn prepared(last_generation: u32) -> Engine {
    let mut caps = CapabilitySet::native();
    caps.max_voices = 2;
    let mut cfg = EngineConfig::new(&caps, 8000., 16, StoreKind::NativeArc);
    cfg.bus_slots = 5;
    cfg.template_slots = 2;
    cfg.bus_seconds = 12.;
    cfg.event_capacity = 32;
    cfg.voice_seconds = 0.25;
    let mut engine = Engine::with_config(cfg);
    let (acks, _received) = SpscRing::split(8);
    engine
        .configure_song_staging_for_transport(
            super::super::SongStagingConfig {
                preparations: 2,
                leases: 8,
                branches: 2,
                control_slots: 0,
                analysis_slots: 0,
                native_pcm_bytes: 4096,
                critical_receipts: 2,
            },
            &acks,
        )
        .unwrap();
    engine
        .begin_song_preparation(SongStagePreparation {
            preparation: SongPreparation {
                epoch: SnapshotEpoch(71),
                branches: 1,
                resources: 4,
                required: SongHostCapacities {
                    sample_rate: 8000,
                    ..Default::default()
                },
            },
            analysis_required: SongAnalysisCapacity { slots: 0 },
        })
        .unwrap();
    for (id, kind) in [
        (1, SongResourceKind::Instrument),
        (2, SongResourceKind::PrivateFx),
        (3, SongResourceKind::Track),
        (4, SongResourceKind::Master),
    ] {
        let resource = SongResourceRef { id, generation: 5 };
        engine
            .reserve_song_resource(SongResourceReservation {
                epoch: SnapshotEpoch(71),
                resource,
                kind,
            })
            .unwrap();
        let payload = if kind == SongResourceKind::Instrument {
            let graph = InstDef {
                id: InstId::new(id),
                params: vec![].into(),
                nodes: vec![UGenSpec::SinOsc].into(),
                edges: vec![].into(),
                node_params: vec![].into(),
            };
            NativeInstall::Inst {
                resource: id,
                gen: 5,
                template: Template::from_inst(&graph, &engine.build_env()).unwrap(),
            }
        } else {
            let graph = BusDef {
                id: BusId::new(id),
                chain: vec![].into(),
            };
            NativeInstall::Bus {
                resource: id,
                gen: 5,
                master: kind == SongResourceKind::Master,
                template: Box::new(BusTemplate::from_def(&graph).unwrap()),
            }
        };
        engine
            .stage_song_native(NativeSongInstall {
                lease: SongLeaseKey {
                    epoch: SnapshotEpoch(71),
                    resource,
                    kind,
                },
                payload,
            })
            .ok()
            .unwrap();
    }
    let config = SongBranchConfig {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(9),
        generation: 1,
        family: 2,
        track: 3,
        instrument: SongResourceRef {
            id: 1,
            generation: 5,
        },
        private_fx: Some(SongResourceRef {
            id: 2,
            generation: 5,
        }),
        track_template: Some(SongResourceRef {
            id: 3,
            generation: 5,
        }),
        master: Some(SongResourceRef {
            id: 4,
            generation: 5,
        }),
        transition_frame: 0,
        tail_deadline: 20,
    };
    if last_generation == 0 {
        engine.stage_song_branch(config).unwrap();
    } else {
        engine
            .stage_song_reusable_branch(SongReusableBranch {
                config,
                last_generation,
            })
            .unwrap();
    }
    engine.seal_song_preparation(SnapshotEpoch(71)).unwrap();
    engine
}
fn event(generation: u32, frame: u64) -> SongCommand {
    SongCommand::Event(SongAudioEvent {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(9),
        generation,
        frame,
        event: AudioEvent::new(0., SlotId::new(1), 1, InstId::new(1)),
    })
}
fn release(generation: u32, frame: u64, deadline: u64) -> SongCommand {
    SongCommand::Release(SongBranchRelease {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(9),
        generation,
        frame,
        tail_deadline: deadline,
    })
}
fn rebind(expected: u32, frame: u64) -> SongCommand {
    SongCommand::RebindBranch(SongBranchRebind {
        epoch: SnapshotEpoch(71),
        branch: SongBranchId(9),
        expected_generation: expected,
        generation: expected + 1,
        transition_frame: frame,
        tail_deadline: frame + 10,
    })
}
#[test]
fn ranked_reversed_ingress_proves_future_event_without_mutating_owners() {
    let mut engine = prepared(3);
    let before = engine.song_stager.as_ref().unwrap().branches[0];
    let capacity = engine.song_remaining_capacities().unwrap();
    let keys: Vec<_> = engine
        .song_stager
        .as_ref()
        .unwrap()
        .leases
        .iter()
        .map(|l| l.key)
        .collect();
    engine.queue_song_runtime(event(1, 5)).unwrap();
    engine.queue_song_runtime(rebind(1, 20)).unwrap();
    engine.queue_song_runtime(release(1, 10, 20)).unwrap();
    engine.queue_song_runtime(event(2, 20)).unwrap();
    let runtime = engine.song_runtime.as_ref().unwrap();
    assert!(matches!(
        runtime.commands[1].command,
        SongCommand::Release(_)
    ));
    assert!(matches!(
        runtime.commands[2].command,
        SongCommand::RebindBranch(_)
    ));
    assert!(matches!(runtime.commands[3].command, SongCommand::Event(_)));
    assert_eq!(engine.song_stager.as_ref().unwrap().branches[0], before);
    assert_eq!(engine.song_remaining_capacities().unwrap(), capacity);
    assert_eq!(
        engine
            .song_stager
            .as_ref()
            .unwrap()
            .leases
            .iter()
            .map(|l| l.key)
            .collect::<Vec<_>>(),
        keys
    );
    assert!(engine.song_runtime.as_ref().unwrap().branches.is_empty());
}
#[test]
fn missing_transition_release_and_early_event_never_prove_generation() {
    let mut engine = prepared(3);
    assert_eq!(
        engine.queue_song_runtime(event(2, 20)),
        Err(SongRejectCode::StaleEpoch)
    );
    engine.queue_song_runtime(rebind(1, 20)).unwrap();
    assert_eq!(
        engine.queue_song_runtime(event(2, 20)),
        Err(SongRejectCode::NotReady)
    );
    engine.queue_song_runtime(release(1, 10, 20)).unwrap();
    assert_eq!(
        engine.queue_song_runtime(event(2, 19)),
        Err(SongRejectCode::StaleEpoch)
    );
    engine.queue_song_runtime(event(2, 20)).unwrap();
    assert_eq!(
        engine.queue_song_runtime(event(1, 20)),
        Err(SongRejectCode::StaleEpoch)
    );
}
#[test]
fn successive_births_obey_keyed_ceiling_and_each_deadline() {
    let mut engine = prepared(3);
    engine.queue_song_runtime(release(1, 10, 20)).unwrap();
    engine.queue_song_runtime(rebind(1, 20)).unwrap();
    engine.queue_song_runtime(release(2, 25, 30)).unwrap();
    engine.queue_song_runtime(rebind(2, 30)).unwrap();
    engine.queue_song_runtime(event(3, 30)).unwrap();
    engine.queue_song_runtime(release(3, 35, 40)).unwrap();
    engine.queue_song_runtime(rebind(3, 40)).unwrap();
    assert_eq!(
        engine.queue_song_runtime(event(4, 40)),
        Err(SongRejectCode::Capacity)
    );
    assert_eq!(
        engine.song_stager.as_ref().unwrap().branches[0].generation,
        1
    );
}
#[test]
fn foreign_failed_duplicate_and_skipped_transitions_do_not_authorize() {
    for case in 0..4 {
        let mut engine = prepared(4);
        engine.queue_song_runtime(release(1, 10, 20)).unwrap();
        let mut transition = rebind(1, 20);
        if let SongCommand::RebindBranch(ref mut r) = transition {
            if case == 0 {
                r.branch = SongBranchId(10);
            }
            if case == 1 {
                r.expected_generation = 2;
                r.generation = 3;
            }
        }
        engine.queue_song_runtime(transition).unwrap();
        if case == 2 {
            engine.song_runtime.as_mut().unwrap().commands[1].failure =
                Some(SongRejectCode::Capacity);
        }
        if case == 3 {
            engine.queue_song_runtime(transition).unwrap();
        }
        assert!(engine.queue_song_runtime(event(2, 20)).is_err());
        assert_eq!(
            engine.song_stager.as_ref().unwrap().branches[0].generation,
            1
        );
    }
    let mut engine = prepared(3);
    let SongCommand::Event(mut e) = event(1, 1) else {
        unreachable!()
    };
    e.epoch = SnapshotEpoch(72);
    assert_eq!(
        engine.queue_song_runtime(SongCommand::Event(e)),
        Err(SongRejectCode::StaleEpoch)
    );
}
#[test]
fn unreached_tail_deadline_and_final_endpoint_forbid_reopen() {
    let mut engine = prepared(3);
    engine.queue_song_runtime(release(1, 10, 21)).unwrap();
    engine.queue_song_runtime(rebind(1, 20)).unwrap();
    assert_eq!(
        engine.queue_song_runtime(event(2, 20)),
        Err(SongRejectCode::NotReady)
    );
    let mut engine = prepared(3);
    engine.queue_song_runtime(release(1, 10, 20)).unwrap();
    engine.queue_song_runtime(rebind(1, 20)).unwrap();
    engine
        .queue_song_runtime(SongCommand::Endpoints(SongEndpoints {
            epoch: SnapshotEpoch(71),
            arrangement: 20,
            tail_deadline: 40,
        }))
        .unwrap();
    assert_eq!(
        engine.queue_song_runtime(event(2, 20)),
        Err(SongRejectCode::NotReady)
    );
}
#[test]
fn ordinary_generation_and_original_refusal_reasons_are_preserved() {
    let mut engine = prepared(0);
    engine.queue_song_runtime(event(1, 1)).unwrap();
    assert_eq!(
        engine.queue_song_runtime(event(2, 1)),
        Err(SongRejectCode::StaleEpoch)
    );
    let SongCommand::Event(mut e) = event(1, 1) else {
        unreachable!()
    };
    e.branch = SongBranchId(10);
    assert_eq!(
        engine.queue_song_runtime(SongCommand::Event(e)),
        Err(SongRejectCode::StaleEpoch)
    );
    let mut engine = prepared(3);
    let SongCommand::RebindBranch(mut r) = rebind(1, 20) else {
        unreachable!()
    };
    r.expected_generation = u32::MAX;
    r.generation = 0;
    assert_eq!(
        engine.queue_song_runtime(SongCommand::RebindBranch(r)),
        Err(SongRejectCode::Malformed)
    );
    assert!(engine.song_runtime.as_ref().unwrap().commands.is_empty());
}

fn advance(engine: &mut Engine, frames: usize) -> Vec<HostMsg> {
    use crate::dsp::cells::AtomicCells;
    use crate::dsp::ring::{EngineIo, EventRing, Garbage, NativeRecord};
    let (_tx, mut controls) = SpscRing::<NativeRecord>::split(8);
    let (_events_tx, mut events) = EventRing::split(8);
    let (mut acks, mut received) = SpscRing::split(8);
    let (mut garbage, _returned) = SpscRing::<Garbage>::split(8);
    let mut cells = AtomicCells::new(8);
    let mut output = vec![0.; frames * 2];
    engine.process(
        &mut EngineIo {
            controls: &mut controls,
            events: &mut events,
            acks: &mut acks,
            cells: &mut cells,
            garbage: Some(&mut garbage),
        },
        &mut output,
        frames,
    );
    let mut receipts = Vec::new();
    while let Some(message) = received.pop() {
        receipts.push(message);
    }
    assert!(engine.pop_fault().is_none(), "actual callback fault");
    receipts
}
#[test]
fn already_executed_shortened_release_uses_live_deadline() {
    let mut engine = prepared(3);
    engine
        .activate_song(SongActivation {
            epoch: SnapshotEpoch(71),
            frame: 0,
        })
        .unwrap();
    engine.queue_song_runtime(release(1, 5, 10)).unwrap();
    let receipts = advance(&mut engine, 12);
    assert!(receipts.iter().any(|m| matches!(
        m,
        HostMsg::Song(SongHostAck::Applied(SongActivation {
            epoch: SnapshotEpoch(71),
            ..
        }))
    )));
    let live = engine.song_runtime.as_ref().unwrap().branches[0];
    assert!(live.ended && live.closed);
    assert_eq!(live.config.tail_deadline, 10);
    assert_eq!(
        engine.song_stager.as_ref().unwrap().branches[0].tail_deadline,
        20
    );
    engine.queue_song_runtime(rebind(1, 12)).unwrap();
    engine.queue_song_runtime(event(2, 12)).unwrap();
    assert_eq!(
        engine.song_stager.as_ref().unwrap().branches[0].generation,
        1
    );
}
