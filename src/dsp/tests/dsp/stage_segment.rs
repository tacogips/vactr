//! Stages adaptation source inventory, rendering and fixed-memory contract.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{stages_cells, StageCoverage, STAGES_OTHER};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, stage_segment, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(function: f32) -> InstDef {
    let node = Node::StageSegment;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::StageSegment,
            UGenSpec::StageSegment,
            UGenSpec::AuxOut,
        ]
        .into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![
            (
                0,
                catalog::port_ctl(&node, 8).unwrap(),
                Ctl::Const(function),
            ),
            (
                1,
                catalog::port_ctl(&node, 8).unwrap(),
                Ctl::Const(function),
            ),
            (1, catalog::port_ctl(&node, 9).unwrap(), Ctl::Const(1.0)),
            (0, catalog::port_ctl(&node, 3).unwrap(), Ctl::Const(0.0)),
            (1, catalog::port_ctl(&node, 3).unwrap(), Ctl::Const(0.0)),
        ]
        .into_boxed_slice(),
    }
}

fn signal(function: f32, typ: f32, changes: &[(usize, f32)]) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 2048,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 42,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::StageSegment).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[0] = Inp::Val(220.0);
    ins[1] = Inp::Val(typ);
    ins[8] = Inp::Val(function);
    for &(index, value) in changes {
        ins[index] = Inp::Val(value);
    }
    let mut state = NodeState::default();
    let mut mem = vec![0.0; stage_segment::mem_len(48_000.0)];
    let mut out = vec![0.0; 2048];
    stage_segment::render(&ins, &mut state, &mut mem, &mut out, &kx);
    out
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

fn delayed_cv(
    sr: f32,
    primary: f32,
    secondary: f32,
    gate: f32,
    looping: f32,
    channel: f32,
) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: (sr * 0.2) as usize,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 42,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[2] = Inp::Val(primary);
    ins[3] = Inp::Val(secondary);
    ins[4] = Inp::Val(looping);
    ins[5] = Inp::Val(gate);
    ins[8] = Inp::Val(9.0);
    ins[9] = Inp::Val(channel);
    let mut st = NodeState::default();
    let mut mem = vec![0.0; stage_segment::mem_len(sr)];
    let mut out = vec![0.0; (sr * 0.2) as usize];
    stage_segment::render(&ins, &mut st, &mut mem, &mut out, &kx);
    out
}

#[test]
fn cv_delay_has_rate_scaled_time_and_no_feedback_or_gate_dependency() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let a = delayed_cv(sr, 0.8, 0.0, 1.0, 1.0, 0.0);
        let b = delayed_cv(sr, 0.8, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(a, b, "source delay ignores gate/loop at {sr}");
        let first = a.iter().position(|&v| v > 0.01).unwrap();
        let seconds = first as f32 / sr;
        assert!((seconds - 0.0625).abs() < 0.005, "{sr} Hz: {seconds}");
        assert!(a.iter().all(|v| v.is_finite()));
        assert!(rms(&a) > 0.01);
        let lower = delayed_cv(sr, 0.3, 0.0, 1.0, 1.0, 0.0);
        assert!(delta(&a, &lower) > 0.05);
        let slower = delayed_cv(sr, 0.8, 0.2, 1.0, 1.0, 0.0);
        assert!(slower.iter().position(|&v| v > 0.01).unwrap() > first);
        let phase = delayed_cv(sr, 0.8, 0.0, 1.0, 1.0, 1.0);
        assert_ne!(a, phase);
        assert!(phase
            .iter()
            .all(|v| v.is_finite() && *v >= -1.0 && *v <= 1.0));
    }
}

#[test]
fn source_table_has_truthful_cell_coverage() {
    let rows = stages_cells();
    assert_eq!(rows.len(), 16);
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(usize::from(row.cell), index);
    }
    assert_eq!(
        rows.iter()
            .filter(|r| r.coverage == StageCoverage::Adaptation)
            .count(),
        14
    );
    for index in [0, 12] {
        assert_eq!(rows[index].coverage, StageCoverage::Excluded);
    }
    for index in [8, 9] {
        assert_eq!(rows[index].coverage, StageCoverage::Adaptation);
    }
    assert_eq!(STAGES_OTHER[3].coverage, StageCoverage::Excluded);
    assert_eq!(STAGES_OTHER[1].coverage, StageCoverage::Adaptation);
    assert_eq!(STAGES_OTHER[2].coverage, StageCoverage::Pending);
}

#[test]
fn segment_types_functions_and_controls_are_distinct() {
    let base = signal(0.0, 0.0, &[]);
    for typ in 1..=3 {
        assert!(
            delta(&base, &signal(0.0, typ as f32, &[])) > 1.0e-4,
            "type {typ}"
        );
    }
    for (function, typ) in [
        (1, 0),
        (2, 2),
        (3, 2),
        (4, 1),
        (5, 0),
        (6, 0),
        (7, 3),
        (8, 3),
    ] {
        let got = signal(function as f32, typ as f32, &[]);
        assert!(got.iter().all(|v| v.is_finite()));
        assert!(delta(&base, &got) > 1.0e-5, "function {function}");
    }
    for (port, value) in [(2, 0.9), (3, 0.9)] {
        assert!(
            delta(&base, &signal(0.0, 0.0, &[(port, value)])) > 1.0e-5,
            "port {port}"
        );
    }
    assert!(
        delta(
            &signal(0.0, 0.0, &[(2, 0.0), (4, 0.0)]),
            &signal(0.0, 0.0, &[(2, 0.0), (4, 1.0)])
        ) > 1.0e-5
    );
    assert!(
        delta(
            &signal(0.0, 0.0, &[(4, 0.0), (5, 0.0)]),
            &signal(0.0, 0.0, &[(4, 0.0), (5, 1.0)])
        ) > 1.0e-5
    );
    assert!(
        delta(
            &signal(1.0, 0.0, &[(5, 0.0), (6, 0.0)]),
            &signal(1.0, 0.0, &[(5, 0.0), (6, 1.0)])
        ) > 1.0e-5
    );
    let osc = signal(7.0, 3.0, &[]);
    for (port, value) in [(0, 440.0), (2, 0.9), (3, 0.9), (9, 1.0)] {
        assert!(
            delta(&osc, &signal(7.0, 3.0, &[(port, value)])) > 1.0e-5,
            "osc port {port}"
        );
    }
}

#[test]
fn graph_codec_budget_and_native_browser_matrix() {
    assert_eq!(catalog::ports(&Node::StageSegment).len(), 10);
    let last = catalog::port_ctl(&Node::StageSegment, 9).unwrap();
    assert_eq!(catalog::port_of(&Node::StageSegment, last), Some(9));
    let def = voice(9.0);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    let budget = 2 * stage_segment::mem_len(96_000.0);
    let env = BuildEnv {
        sr: 96_000.0,
        caps: caps(),
        voice_mem: budget,
    };
    let mut built = Template::boxed();
    built.build(&raw, &env).unwrap();
    assert_eq!(built.mem_total, budget);
    assert!(built.has_aux);
    assert!(Template::from_inst(
        &def,
        &BuildEnv {
            voice_mem: budget - 1,
            ..env
        }
    )
    .is_err());
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut native = NativeRig::native_with(cfg);
            native.install(&def);
            let _ = native.step();
            native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (main, aux) = native.run(128);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7 && main != aux);
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (main, aux) = browser.run(128);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7 && main != aux);
        }
    }
}
