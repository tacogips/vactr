//! Captures only copied isolated graph/default metadata, never a live cell table.
use crate::song::snapshot::{FrozenCellInventory, FrozenRoutingInventory};
use crate::vm::fail::Failure;

pub(super) fn capture_frozen_cells(
    routing: &FrozenRoutingInventory,
    remaining: &mut u32,
) -> Result<FrozenCellInventory, Failure> {
    FrozenCellInventory::capture(routing, remaining)
}

#[cfg(test)]
mod tests {
    use crate::dsp::graph::InstId;
    use crate::dsp::graph::{
        AnalyzerKind, BusDef, BusId, EffectKind, EffectSpec, InstDef, UGenSpec,
    };
    use crate::dsp::{cells::CellId, effects};
    use crate::host::wire::Ctl;
    use crate::sched::slots::CtlId;
    use crate::song::snapshot::FrozenInstrument;
    use crate::song::snapshot::*;
    use crate::value::intern::intern_kw;
    use crate::vm::fail::FailCode;
    use std::sync::Arc;

    fn model() -> FrozenRoutingInventory {
        let cell = CellId::new(7);
        let header = CtlId::new(3);
        let id = effects::param_ctl(EffectKind::Analyzer(AnalyzerKind::Level), "id").unwrap();
        let fx = EffectSpec {
            kind: EffectKind::Analyzer(AnalyzerKind::Level),
            params: vec![(id, Ctl::Cell(cell))].into(),
        };
        FrozenRoutingInventory {
            instruments: vec![FrozenInstrument {
                name: intern_kw("cell-model"),
                graph: Arc::new(InstDef {
                    id: InstId::new(10),
                    params: vec![(header, Ctl::Cell(cell))].into(),
                    nodes: vec![UGenSpec::SinOsc, UGenSpec::Effect(fx)].into(),
                    edges: vec![].into(),
                    node_params: vec![
                        (0, CtlId::new(0), Ctl::Cell(cell)),
                        (1, id, Ctl::Cell(cell)),
                    ]
                    .into(),
                }),
                parameters: vec![],
                defaults: vec![(cell, 1.75)],
                resource: None,
            }],
            buses: vec![(
                Some(intern_kw("drums")),
                Arc::new(BusDef {
                    id: BusId::new(2),
                    chain: vec![EffectSpec {
                        kind: EffectKind::Analyzer(AnalyzerKind::Spectrum),
                        params: vec![(id, Ctl::Cell(cell))].into(),
                    }]
                    .into(),
                }),
            )],
            ..Default::default()
        }
    }
    fn capture(model: &FrozenRoutingInventory) -> FrozenCellInventory {
        FrozenCellInventory::capture(model, &mut 100_000).unwrap()
    }
    #[test]
    fn every_header_node_embedded_and_bus_cell_site_is_copied() {
        let model = model();
        let cells = capture(&model);
        assert_eq!(cells.references().len(), 5);
        assert!(cells
            .references()
            .iter()
            .any(|r| matches!(r.site, FrozenCellSite::Header { .. })));
        assert!(cells
            .references()
            .iter()
            .any(|r| matches!(r.site, FrozenCellSite::NodeParameter { .. })));
        assert!(cells
            .references()
            .iter()
            .any(|r| matches!(r.site, FrozenCellSite::EmbeddedEffect { .. })));
        assert!(cells
            .references()
            .iter()
            .any(|r| matches!(r.site, FrozenCellSite::BusEffect { .. })));
        assert_eq!(cells.value(CellId::new(7)), Some(1.75));
        assert_eq!(cells.logical_control_slots(), 1);
        assert_eq!(cells.analysis_ranges()[0].logical_start, 1);
        cells.validate_graphs(&model, &mut 100_000).unwrap();
        cells
            .validate_event_controls(
                FrozenCellOwner::Instrument(intern_kw("cell-model")),
                &[(CtlId::new(3), Ctl::Cell(CellId::new(7)))],
                &mut 1000,
            )
            .unwrap();
    }
    #[test]
    fn missing_conflicting_nonfinite_and_invalid_node_cells_never_fallback() {
        let original = model();
        let mut missing = original.clone();
        missing.instruments[0].defaults.clear();
        assert_eq!(
            FrozenCellInventory::capture(&missing, &mut 100_000)
                .unwrap_err()
                .code,
            FailCode::BeyondCapability
        );
        for value in [f32::NAN, f32::INFINITY, 2.0] {
            let mut bad = original.clone();
            bad.instruments[0].defaults.push((CellId::new(7), value));
            assert!(FrozenCellInventory::capture(&bad, &mut 100_000).is_err());
        }
        let mut bad = original;
        Arc::make_mut(&mut bad.instruments[0].graph).node_params =
            vec![(99, CtlId::new(0), Ctl::Cell(CellId::new(7)))].into();
        assert!(FrozenCellInventory::capture(&bad, &mut 100_000).is_err());
    }
    #[test]
    fn ordered_overlapping_writers_normalize_ids_and_preserve_holes() {
        let level = EffectKind::Analyzer(AnalyzerKind::Level);
        let id = effects::param_ctl(level, "id").unwrap();
        let mut model = FrozenRoutingInventory::default();
        model.buses.push((
            None,
            Arc::new(BusDef {
                id: BusId::new(0),
                chain: vec![
                    EffectSpec {
                        kind: level,
                        params: vec![].into(),
                    },
                    EffectSpec {
                        kind: EffectKind::Analyzer(AnalyzerKind::Spectrum),
                        params: vec![].into(),
                    },
                    EffectSpec {
                        kind: level,
                        params: vec![(id, Ctl::Const(100.75))].into(),
                    },
                    EffectSpec {
                        kind: level,
                        params: vec![(id, Ctl::Const(-2.0))].into(),
                    },
                    EffectSpec {
                        kind: level,
                        params: vec![(id, Ctl::Const(70_000.0))].into(),
                    },
                ]
                .into(),
            }),
        ));
        let cells = capture(&model);
        assert_eq!(
            cells
                .analysis_ranges()
                .iter()
                .map(|r| (r.writer_order, r.logical_start))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 0), (2, 100), (3, 0), (4, 65535)]
        );
        assert_eq!(cells.analysis_banks()[0].slots, 65537);
        assert_eq!(cells.logical_analysis_slots(), 65537);
        assert!(
            cells
                .analysis_ranges()
                .iter()
                .map(|r| u64::from(r.width))
                .sum::<u64>()
                < cells.logical_analysis_slots()
        );
    }
    #[test]
    fn explicit_node_override_follows_embedded_effect_last_value_order() {
        let mut model = model();
        let id = effects::param_ctl(EffectKind::Analyzer(AnalyzerKind::Level), "id").unwrap();
        Arc::make_mut(&mut model.instruments[0].graph).node_params =
            vec![(1, id, Ctl::Const(3.25)), (1, id, Ctl::Const(8.75))].into();
        assert_eq!(capture(&model).analysis_ranges()[0].logical_start, 8);
    }
    #[test]
    fn shared_aliases_keep_sites_once_and_foreign_event_headers_fail() {
        let mut model = model();
        model.instruments.push(model.instruments[0].clone());
        let cells = capture(&model);
        assert_eq!(cells.references().len(), 5);
        assert_eq!(cells.analysis_ranges().len(), 2);
        let owner = FrozenCellOwner::Instrument(intern_kw("cell-model"));
        for control in [
            (CtlId::new(4), Ctl::Cell(CellId::new(7))),
            (CtlId::new(3), Ctl::Cell(CellId::new(99))),
        ] {
            assert!(cells
                .validate_event_controls(owner, &[control], &mut 1000)
                .is_err());
        }
        assert!(cells
            .validate_event_controls(
                FrozenCellOwner::Instrument(intern_kw("foreign")),
                &[],
                &mut 1000
            )
            .is_err());
        Arc::make_mut(&mut model.instruments[1].graph).params = vec![].into();
        assert!(FrozenCellInventory::capture(&model, &mut 100_000).is_err());
    }
    #[test]
    fn reverse_declared_analyzer_chain_matches_compiled_execution_order() {
        use crate::dsp::graph::Edge;
        use crate::dsp::ugen::{BuildEnv, Node, RawGraph, Template};
        let level = EffectKind::Analyzer(AnalyzerKind::Level);
        let spectrum = EffectKind::Analyzer(AnalyzerKind::Spectrum);
        let graph = InstDef {
            id: InstId::new(20),
            params: vec![].into(),
            nodes: vec![
                UGenSpec::Effect(EffectSpec {
                    kind: level,
                    params: vec![].into(),
                }),
                UGenSpec::Effect(EffectSpec {
                    kind: spectrum,
                    params: vec![].into(),
                }),
                UGenSpec::SinOsc,
            ]
            .into(),
            edges: vec![
                Edge {
                    from: 2,
                    to: 1,
                    port: 0,
                    output: 0,
                },
                Edge {
                    from: 1,
                    to: 0,
                    port: 0,
                    output: 0,
                },
            ]
            .into(),
            node_params: vec![].into(),
        };
        let mut model = FrozenRoutingInventory::default();
        model.instruments.push(FrozenInstrument {
            name: intern_kw("reverse"),
            graph: Arc::new(graph),
            parameters: vec![],
            defaults: vec![],
            resource: None,
        });
        let cells = capture(&model);
        assert_eq!(
            cells
                .analysis_ranges()
                .iter()
                .map(|r| (r.effect, r.writer_order, r.kind))
                .collect::<Vec<_>>(),
            vec![(1, 0, AnalyzerKind::Spectrum), (0, 1, AnalyzerKind::Level)]
        );
        let mut raw = RawGraph::boxed();
        raw.load(&model.instruments[0].graph).unwrap();
        let mut template = Template::boxed();
        template
            .build(
                &raw,
                &BuildEnv {
                    sr: 48000.0,
                    caps: crate::dsp::caps::CapabilitySet::native(),
                    voice_mem: 48000,
                },
            )
            .unwrap();
        let actual: Vec<_> = template
            .nodes()
            .iter()
            .filter_map(|n| match n.node {
                Node::Effect {
                    kind: EffectKind::Analyzer(kind),
                    ..
                } => Some(kind),
                _ => None,
            })
            .collect();
        assert_eq!(actual, vec![AnalyzerKind::Spectrum, AnalyzerKind::Level]);
    }
    #[test]
    fn capture_cost_is_exact_cumulative_and_retained_by_clone() {
        let model = model();
        let cells = capture(&model);
        let cost = cells.consumed_work();
        assert!(cost > 0);
        let mut exact = cost;
        let copy = FrozenCellInventory::capture(&model, &mut exact).unwrap();
        assert_eq!(exact, 0);
        assert_eq!(copy.clone().consumed_work(), cost);
        let mut short = cost - 1;
        assert_eq!(
            FrozenCellInventory::capture(&model, &mut short)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        let mut shared = cost * 2;
        capture_pair(&model, &mut shared);
        assert_eq!(shared, 0);
        let mut wide = model.clone();
        Arc::make_mut(&mut wide.instruments[0].graph).params =
            vec![(CtlId::new(3), Ctl::Cell(CellId::new(7))); 1000].into();
        assert_eq!(
            FrozenCellInventory::capture(&wide, &mut 32)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
    }
    fn capture_pair(model: &FrozenRoutingInventory, remaining: &mut u32) {
        FrozenCellInventory::capture(model, remaining).unwrap();
        FrozenCellInventory::capture(model, remaining).unwrap();
    }
}
