//! Peaks function adaptation: source-order coverage, state, and host contract.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{peaks_functions, CoverageState, PeaksRole, ResourceState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, peak_function, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(mode: f32, half: f32) -> InstDef {
    let node = Node::PeakFunction;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::PeakFunction,
            UGenSpec::PeakFunction,
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
            (0, catalog::port_ctl(&node, 15).unwrap(), Ctl::Const(mode)),
            (1, catalog::port_ctl(&node, 15).unwrap(), Ctl::Const(mode)),
            (0, catalog::port_ctl(&node, 10).unwrap(), Ctl::Const(half)),
            (1, catalog::port_ctl(&node, 10).unwrap(), Ctl::Const(half)),
            (0, catalog::port_ctl(&node, 16).unwrap(), Ctl::Const(0.0)),
            (1, catalog::port_ctl(&node, 16).unwrap(), Ctl::Const(1.0)),
        ]
        .into_boxed_slice(),
    }
}

fn direct(mode: f32, half: f32, shape: f32, color: f32, preset: f32) -> Vec<f32> {
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
        seed: 1,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(100.0);
    ins[1] = Inp::Val(0.03);
    ins[2] = Inp::Val(0.05);
    ins[3] = Inp::Val(0.4);
    ins[4] = Inp::Val(0.05);
    ins[5] = Inp::Val(0.3);
    ins[6] = Inp::Val(shape);
    ins[7] = Inp::Val(color);
    ins[9] = Inp::Val(0.8);
    ins[10] = Inp::Val(half);
    ins[11] = Inp::Val(1.0);
    ins[15] = Inp::Val(mode);
    ins[17] = Inp::Val(preset);
    let mut state = NodeState::default();
    let mut mem = [0.0; peak_function::STATE_FLOATS];
    let mut out = vec![0.0; 2048];
    peak_function::render(&ins, &mut state, &mut mem, &mut out, &kx);
    out
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn twelve_rows_are_ordered_and_truthful() {
    let rows = peaks_functions();
    assert_eq!(rows.len(), 12);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(usize::from(row.position), i);
    }
    for row in &rows[..3] {
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.vactr_template, Some("peak-motion-voice"));
    }
    for row in &rows[3..6] {
        assert_eq!(row.coverage, CoverageState::Adaptation);
    }
    assert_eq!(rows[6].coverage, CoverageState::SourceStage);
    assert_eq!(rows[10].role, PeaksRole::Excluded);
    assert_eq!(rows[11].resources, ResourceState::Replacement);
    assert_eq!(rows[11].coverage, CoverageState::Adaptation);
    assert!(rows[11].upstream_digit_asset);
}

#[test]
fn alternate_and_wave_controls_change_audio() {
    let env = direct(0.0, 0.0, 0.0, 0.0, 0.0);
    let ad = direct(0.0, 1.0, 0.0, 0.0, 0.0);
    assert!(rms(&env) > 1.0e-5);
    assert!(delta(&env, &ad) > 1.0e-4);
    let base = direct(1.0, 0.0, 0.0, 0.0, 0.0);
    assert!(rms(&base) > 1.0e-5);
    for shape in 1..=4 {
        assert!(delta(&base, &direct(1.0, 0.0, shape as f32, 0.0, 0.0)) > 1.0e-4);
    }
    assert!(delta(&base, &direct(1.0, 0.0, 0.0, 0.7, 0.0)) > 1.0e-4);
    for preset in 1..=6 {
        assert!(
            delta(
                &direct(1.0, 1.0, 0.0, 0.0, 0.0),
                &direct(1.0, 1.0, 0.0, 0.0, preset as f32)
            ) > 1.0e-4
        );
    }
}

#[test]
fn tap_edges_measure_period_and_trigger_resets_phase() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 1,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(100.0);
    ins[9] = Inp::Val(1.0);
    ins[15] = Inp::Val(2.0);
    let mut st = NodeState::default();
    let mut mem = [0.0; peak_function::STATE_FLOATS];
    let mut out = [0.0; 1];
    ins[13] = Inp::Val(1.0);
    peak_function::render(&ins, &mut st, &mut mem, &mut out, &kx);
    ins[13] = Inp::Val(0.0);
    for _ in 0..2400 {
        peak_function::render(&ins, &mut st, &mut mem, &mut out, &kx);
    }
    ins[13] = Inp::Val(1.0);
    peak_function::render(&ins, &mut st, &mut mem, &mut out, &kx);
    assert!((mem[13] - 2401.0).abs() <= 1.0);
    ins[13] = Inp::Val(0.0);
    for _ in 0..100 {
        peak_function::render(&ins, &mut st, &mut mem, &mut out, &kx);
    }
    ins[12] = Inp::Val(1.0);
    peak_function::render(&ins, &mut st, &mut mem, &mut out, &kx);
    assert!(mem[0] < 0.01);
}

#[test]
fn envelope_attack_decay_sustain_and_release_have_distinct_response() {
    fn trace(index: usize, value: f32) -> Vec<f32> {
        let store = SampleStore::new(StoreKind::NativeArc);
        let caps = caps();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr: 48_000.0,
            gate: 24_000,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        ins[0] = Inp::Val(220.0);
        ins[1] = Inp::Val(0.1);
        ins[2] = Inp::Val(0.1);
        ins[3] = Inp::Val(0.4);
        ins[4] = Inp::Val(0.1);
        ins[11] = Inp::Val(1.0);
        ins[index] = Inp::Val(value);
        let mut st = NodeState::default();
        let mut mem = [0.0; peak_function::STATE_FLOATS];
        let mut out = vec![0.0; 24_000];
        peak_function::render(&ins, &mut st, &mut mem, &mut out, &kx);
        ins[11] = Inp::Val(0.0);
        let mut released = vec![0.0; 24_000];
        peak_function::render(&ins, &mut st, &mut mem, &mut released, &kx);
        out.extend(released);
        out
    }
    let base = trace(1, 0.1);
    for (index, value) in [(1, 0.7), (2, 0.7), (3, 0.8), (4, 0.7)] {
        let signal = trace(index, value);
        assert!(delta(&base, &signal) > 1.0e-3, "envelope port {index}");
    }
    assert!(base[46_000] < base[23_000], "gate fall enters release");
}

#[test]
fn native_browser_codec_budget_and_rates() {
    assert_eq!(catalog::ports(&Node::PeakFunction).len(), 18);
    let last = catalog::port_ctl(&Node::PeakFunction, 17).unwrap();
    assert_eq!(catalog::port_of(&Node::PeakFunction, last), Some(17));
    for mode in 0..3 {
        let def = voice(mode as f32, 0.0);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        let budget = 2 * peak_function::STATE_FLOATS;
        let env = BuildEnv {
            sr: 96_000.0,
            caps: caps(),
            voice_mem: budget,
        };
        let built = Template::from_inst(&def, &env).unwrap();
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
                native.send(event(1, native.engine.now(), &[(ctl::FREQ, 100.0)]));
                let (main, aux) = native.run(8);
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(delta(&main, &aux) > 1.0e-6);
                let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut browser = BrowserRig::browser_with(cfg);
                let mut record = Vec::new();
                encode_graph_record(1, 1, &bytes, &mut record);
                browser.push(&record);
                let _ = browser.run(6);
                browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 100.0)]));
                let (main, aux) = browser.run(8);
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(delta(&main, &aux) > 1.0e-6);
            }
        }
    }
}
