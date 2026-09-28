//! Peaks pulse and bouncing-ball control, timing, and host tests.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{peaks_functions, CoverageState, ResourceState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, peak_pulse, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(mode: f32, half: f32) -> InstDef {
    let node = Node::PeakPulse;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::PeakPulse, UGenSpec::PeakPulse, UGenSpec::AuxOut].into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![
            (0, catalog::port_ctl(&node, 1).unwrap(), Ctl::Const(mode)),
            (1, catalog::port_ctl(&node, 1).unwrap(), Ctl::Const(mode)),
            (0, catalog::port_ctl(&node, 2).unwrap(), Ctl::Const(half)),
            (1, catalog::port_ctl(&node, 2).unwrap(), Ctl::Const(half)),
            (0, catalog::port_ctl(&node, 16).unwrap(), Ctl::Const(0.0)),
            (1, catalog::port_ctl(&node, 16).unwrap(), Ctl::Const(1.0)),
        ]
        .into_boxed_slice(),
    }
}

fn signal(mode: f32, half: f32, changes: &[(usize, f32)]) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 48_000,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 901,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::PeakPulse).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[1] = Inp::Val(mode);
    ins[2] = Inp::Val(half);
    for &(index, value) in changes {
        ins[index] = Inp::Val(value);
    }
    let mut st = NodeState::default();
    let mut mem = [0.0; peak_pulse::STATE_FLOATS];
    let mut out = vec![0.0; 48_000];
    peak_pulse::render(&ins, &mut st, &mut mem, &mut out, &kx);
    out
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn three_rows_are_runnable_adaptations() {
    for row in &peaks_functions()[7..10] {
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
        assert_eq!(row.vactrol_template, Some("peak-pulse-voice"));
    }
}

#[test]
fn pulse_shaper_width_delay_repeats_and_half_change_edges() {
    let base = signal(0.0, 0.0, &[]);
    assert!(rms(&base) > 1.0e-4);
    for &(index, value) in &[(5, 0.8), (6, 0.8), (7, 0.8), (8, 6.0), (0, 110.0)] {
        assert!(
            delta(&base, &signal(0.0, 0.0, &[(index, value)])) > 1.0e-4,
            "port {index}"
        );
    }
    assert!(delta(&base, &signal(0.0, 1.0, &[])) > 1.0e-4);
    assert!(
        delta(
            &signal(0.0, 1.0, &[(5, 0.0), (7, 0.0)]),
            &signal(0.0, 1.0, &[(5, 1.0), (7, 1.0)])
        ) == 0.0,
        "half shaper derives pre-delay and interval"
    );
    assert!(signal(0.0, 0.0, &[(4, 0.0)]).iter().all(|&x| x == 0.0));
}

#[test]
fn separate_trigger_and_gate_edges_restart_the_train() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 128,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 901,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::PeakPulse).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[3] = Inp::Val(1.0);
    ins[4] = Inp::Val(0.0);
    let mut st = NodeState::default();
    let mut mem = [0.0; peak_pulse::STATE_FLOATS];
    let mut out = [0.0; 128];
    peak_pulse::render(&ins, &mut st, &mut mem, &mut out, &kx);
    let remaining = mem[2];
    ins[4] = Inp::Val(1.0);
    peak_pulse::render(&ins, &mut st, &mut mem, &mut out[..1], &kx);
    assert!(
        mem[2] > remaining,
        "trigger rise retriggers even with held gate"
    );
}

#[test]
fn randomizer_probability_delay_variation_and_half_are_distinct() {
    let base = signal(1.0, 0.0, &[(10, 1.0)]);
    assert!(rms(&base) > 1.0e-5);
    assert!(signal(1.0, 0.0, &[(9, 0.0)]).iter().all(|&x| x == 0.0));
    for &(index, value) in &[(7, 0.8), (10, 0.0), (11, 0.0)] {
        assert!(
            delta(&base, &signal(1.0, 0.0, &[(10, 1.0), (index, value)])) > 1.0e-5,
            "port {index}"
        );
    }
    let half = signal(1.0, 1.0, &[(9, 0.0), (11, 1.0)]);
    let fixed = signal(1.0, 1.0, &[(9, 1.0), (11, 0.0)]);
    assert_eq!(half, fixed, "half mode fixes acceptance and randomness");
}

#[test]
fn bouncing_ball_gravity_loss_height_velocity_and_half_affect_timing() {
    let base = signal(2.0, 0.0, &[]);
    assert!(rms(&base) > 1.0e-4);
    for &(index, value) in &[(12, 0.9), (13, 0.1), (14, 0.2), (15, 0.8)] {
        assert!(
            delta(&base, &signal(2.0, 0.0, &[(index, value)])) > 1.0e-4,
            "port {index}"
        );
    }
    let half = signal(2.0, 1.0, &[(14, 0.1), (15, 0.9)]);
    let fixed = signal(2.0, 1.0, &[(14, 1.0), (15, 0.0)]);
    assert_eq!(half, fixed, "half mode fixes height and velocity");
    assert!(base.iter().all(|x| x.is_finite()));
}

#[test]
fn native_browser_codec_budget_and_rates() {
    assert_eq!(catalog::ports(&Node::PeakPulse).len(), 17);
    let last = catalog::port_ctl(&Node::PeakPulse, 16).unwrap();
    assert_eq!(catalog::port_of(&Node::PeakPulse, last), Some(16));
    for mode in 0..3 {
        let def = voice(mode as f32, 0.0);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        let budget = 2 * peak_pulse::STATE_FLOATS;
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
                native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
                let (main, aux) = native.run(120);
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
                browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
                let (main, aux) = browser.run(120);
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(delta(&main, &aux) > 1.0e-6);
            }
        }
    }
}
