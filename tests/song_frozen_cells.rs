//! Public frozen metadata admission; no arbitrary certificate constructor.
use std::collections::BTreeMap;
use std::sync::Arc;
use vactr::dsp::cells::{AtomicCells, CellId};
use vactr::dsp::effects;
use vactr::dsp::graph::{AnalyzerKind, BusDef, BusId, EffectKind, EffectSpec};
use vactr::host::wire::Ctl;
use vactr::sched::slots::CtlId;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{
    FrozenCellInventory, FrozenCellOwner, FrozenCellSite, FrozenRoutingInventory,
};
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch};
use vactr::value::intern::intern_kw;
use vactr::vm::fail::FailCode;

const CELL_VOICE: &str = "inst cellvoice freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [drums: {s :cellvoice}] duration: 1} > play-song";
fn candidate(code: &str) -> PreparedSong {
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let context = CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 4_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    prepare_song(
        evaluate_song_candidate(code, "cells.vact", 1, SnapshotEpoch(77), &context).unwrap(),
    )
    .unwrap()
}
#[test]
fn empty_certificate_only_validates_empty_graphs() {
    let empty = FrozenRoutingInventory::default();
    empty.cells.validate_graphs(&empty, &mut 0).unwrap();
    let mut hostile = empty;
    hostile.buses.push((
        None,
        Arc::new(BusDef {
            id: BusId::new(0),
            chain: vec![].into(),
        }),
    ));
    assert_eq!(
        hostile
            .cells
            .validate_graphs(&hostile, &mut 1000)
            .unwrap_err()
            .code,
        FailCode::BeyondCapability
    );
}
#[test]
fn genuine_inst_default_cells_are_complete_and_snapshot_private() {
    let song = candidate(CELL_VOICE);
    let routing = song.snapshot().routing();
    let cells = &routing.cells;
    assert!(cells.consumed_work() > 0);
    assert!(!cells.values().is_empty());
    assert!(cells
        .references()
        .iter()
        .any(|r| matches!(r.site, FrozenCellSite::Header { .. })));
    assert!(cells.values().iter().any(|v| v.value == 440.0));
    cells.validate_graphs(routing, &mut 100_000).unwrap();
    let native = AtomicCells::new(4096);
    for value in cells.values() {
        assert!(native.set(value.cell, -777.0));
        assert_eq!(cells.value(value.cell), Some(value.value));
    }
}
#[test]
fn legacy_default_analysis_writers_overlap_in_original_chain_order() {
    let song=candidate("master:\n\tlevel > spectrum > pitch-meter\nsong {part [drums: {s :analog}] duration: 1} > play-song");
    let cells = &song.snapshot().routing().cells;
    let writers: Vec<_> = cells
        .analysis_ranges()
        .iter()
        .filter(|r| matches!(r.owner, FrozenCellOwner::Master(_)))
        .collect();
    assert_eq!(writers.len(), 3);
    assert_eq!(
        writers.iter().map(|r| r.logical_start).collect::<Vec<_>>(),
        vec![0, 0, 0]
    );
    assert_eq!(
        writers.iter().map(|r| r.writer_order).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        writers.iter().map(|r| r.kind).collect::<Vec<_>>(),
        vec![
            AnalyzerKind::Level,
            AnalyzerKind::Spectrum,
            AnalyzerKind::PitchMeter
        ]
    );
    assert_eq!(
        cells
            .analysis_banks()
            .iter()
            .find(|b| matches!(b.owner, FrozenCellOwner::Master(_)))
            .unwrap()
            .slots,
        u32::try_from(effects::analyzer::cells(AnalyzerKind::Spectrum)).unwrap()
    );
}
#[test]
fn finite_fractional_analysis_address_keeps_runtime_truncation() {
    let song = candidate(
        "master:\n\tlevel id: 1.75\nsong {part [drums: {s :analog}] duration: 1} > play-song",
    );
    let cells = &song.snapshot().routing().cells;
    let writer = cells
        .analysis_ranges()
        .iter()
        .find(|r| matches!(r.owner, FrozenCellOwner::Master(_)))
        .unwrap();
    assert_eq!(writer.logical_start, 1);
    assert_eq!(writer.width, 2);
    assert_eq!(cells.logical_analysis_slots(), 3);
}
#[test]
fn missing_node_bus_and_embedded_cell_references_reject_without_zero() {
    let song = candidate(CELL_VOICE);
    let original = song.snapshot().routing();
    let chosen = original
        .instruments
        .iter()
        .position(|i| i.name == intern_kw("cellvoice"))
        .unwrap();
    for variant in 0..3 {
        let mut hostile = original.clone();
        let missing = CellId::new(u32::MAX);
        let id = effects::param_ctl(EffectKind::Analyzer(AnalyzerKind::Level), "id").unwrap();
        match variant {
            0 => {
                Arc::make_mut(&mut hostile.instruments[chosen].graph).node_params =
                    vec![(0, CtlId::new(0), Ctl::Cell(missing))].into()
            }
            1 => hostile.buses.push((
                Some(intern_kw("foreign")),
                Arc::new(BusDef {
                    id: BusId::new(99),
                    chain: vec![EffectSpec {
                        kind: EffectKind::Analyzer(AnalyzerKind::Level),
                        params: vec![(id, Ctl::Cell(missing))].into(),
                    }]
                    .into(),
                }),
            )),
            _ => {
                let graph = Arc::make_mut(&mut hostile.instruments[chosen].graph);
                let mut nodes = graph.nodes.to_vec();
                nodes.push(vactr::dsp::graph::UGenSpec::Effect(EffectSpec {
                    kind: EffectKind::Analyzer(AnalyzerKind::Level),
                    params: vec![(id, Ctl::Cell(missing))].into(),
                }));
                graph.nodes = nodes.into();
            }
        }
        assert_eq!(
            hostile
                .cells
                .validate_graphs(&hostile, &mut 100_000)
                .unwrap_err()
                .code,
            FailCode::BeyondCapability
        );
    }
}
#[test]
fn conflicting_nonfinite_and_ambiguous_owner_metadata_reject() {
    let song = candidate(CELL_VOICE);
    let original = song.snapshot().routing();
    let index = original
        .instruments
        .iter()
        .position(|i| !i.defaults.is_empty())
        .unwrap();
    let (cell, value) = original.instruments[index].defaults[0];
    for wrong in [value + 1.0, f32::NAN, f32::INFINITY] {
        let mut bad = original.clone();
        bad.instruments[index].defaults.push((cell, wrong));
        assert!(bad.cells.validate_graphs(&bad, &mut 100_000).is_err());
    }
    let mut bad = original.clone();
    let mut alias = bad.instruments[index].clone();
    Arc::make_mut(&mut alias.graph).params = vec![].into();
    bad.instruments.push(alias);
    assert!(bad.cells.validate_graphs(&bad, &mut 100_000).is_err());
}
#[test]
fn only_exact_owner_header_cells_are_admitted_to_host_events() {
    let song = candidate(CELL_VOICE);
    let cells = &song.snapshot().routing().cells;
    let r = cells
        .references()
        .iter()
        .find(|r| matches!(r.site, FrozenCellSite::Header { .. }))
        .unwrap();
    let FrozenCellSite::Header { parameter } = r.site else {
        unreachable!()
    };
    cells
        .validate_event_controls(r.owner, &[(parameter, Ctl::Cell(r.cell))], &mut 1000)
        .unwrap();
    for ctl in [Ctl::Cell(CellId::new(u32::MAX)), Ctl::Const(f32::NAN)] {
        assert!(cells
            .validate_event_controls(r.owner, &[(parameter, ctl)], &mut 1000)
            .is_err());
    }
    assert!(cells
        .validate_event_controls(
            FrozenCellOwner::Instrument(intern_kw("foreign")),
            &[],
            &mut 1000
        )
        .is_err());
}
#[test]
fn public_default_cannot_forge_nonempty_certification() {
    let song = candidate(CELL_VOICE);
    let mut hostile = song.snapshot().routing().clone();
    hostile.cells = FrozenCellInventory::default();
    assert!(hostile
        .cells
        .validate_graphs(&hostile, &mut 100_000)
        .is_err());
    song.snapshot()
        .routing()
        .cells
        .validate_graphs(song.snapshot().routing(), &mut 100_000)
        .unwrap();
}
#[test]
fn cumulative_validation_and_clone_costs_remain_bounded() {
    let song = candidate(CELL_VOICE);
    let routing = song.snapshot().routing();
    let cells = &routing.cells;
    let mut baseline = 100_000;
    cells.validate_graphs(routing, &mut baseline).unwrap();
    let cost = 100_000 - baseline;
    assert!(cost >= cells.consumed_work());
    let mut shared = cost * 2;
    cells.clone().validate_graphs(routing, &mut shared).unwrap();
    cells.validate_graphs(routing, &mut shared).unwrap();
    assert_eq!(shared, 0);
    assert_eq!(
        cells
            .validate_graphs(routing, &mut (cost - 1))
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    assert_eq!(cells.clone().consumed_work(), cells.consumed_work());
}
#[test]
fn topology_cycle_and_out_of_range_edges_are_rejected_on_revalidation() {
    let song = candidate(CELL_VOICE);
    let original = song.snapshot().routing();
    let index = original
        .instruments
        .iter()
        .position(|i| i.name == intern_kw("cellvoice"))
        .unwrap();
    for target in [0, u16::MAX] {
        let mut hostile = original.clone();
        Arc::make_mut(&mut hostile.instruments[index].graph).edges =
            vec![vactr::dsp::graph::Edge {
                from: 0,
                to: target,
                port: 0,
                output: 0,
            }]
            .into();
        assert!(hostile
            .cells
            .validate_graphs(&hostile, &mut 100_000)
            .is_err());
    }
}
