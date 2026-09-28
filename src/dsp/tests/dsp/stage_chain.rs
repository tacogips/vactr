//! Six-segment chain: transitions, codec capacity, and host-rate contracts.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{StageCoverage, STAGES_OTHER};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, stage_chain, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(count: f32) -> InstDef {
    let node = Node::StageChain;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::StageChain, UGenSpec::StageChain, UGenSpec::AuxOut]
            .into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![
            (0, catalog::port_ctl(&node, 1).unwrap(), Ctl::Const(count)),
            (1, catalog::port_ctl(&node, 1).unwrap(), Ctl::Const(count)),
            (1, catalog::port_ctl(&node, 4).unwrap(), Ctl::Const(1.0)),
        ]
        .into_boxed_slice(),
    }
}

fn signal(count: usize, changes: &[(usize, f32)], frames: usize) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: frames,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::StageChain).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[1] = Inp::Val(count as f32);
    for i in 0..6 {
        ins[5 + i * 4] = Inp::Val(0.0);
        ins[5 + i * 4 + 2] = Inp::Val(if i % 2 == 1 { 0.1 } else { 0.0 });
        ins[5 + i * 4 + 3] = Inp::Val(0.5);
        ins[5 + i * 4 + 1] = Inp::Val(0.0);
    }
    for &(port, value) in changes {
        ins[port] = Inp::Val(value);
    }
    let mut st = NodeState::default();
    let mut mem = [0.0; stage_chain::STATE_FLOATS];
    let mut out = vec![0.0; frames];
    stage_chain::render(&ins, &mut st, &mut mem, &mut out, &kx);
    out
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn bounded_chain_manifest_does_not_claim_cross_module_parity() {
    assert_eq!(STAGES_OTHER[0].source_function, "MultiSegment");
    assert_eq!(STAGES_OTHER[0].coverage, StageCoverage::Adaptation);
    assert_eq!(STAGES_OTHER[0].vactrol_template, Some("stage-chain-voice"));
    assert_eq!(STAGES_OTHER[1].coverage, StageCoverage::Adaptation);
    assert_eq!(
        STAGES_OTHER[1].vactrol_template,
        Some("stage-sequencer-voice")
    );
    assert_eq!(STAGES_OTHER[2].coverage, StageCoverage::Pending);
}

#[test]
fn one_to_six_segments_and_last_segment_controls_change_audio() {
    let baseline = signal(1, &[], 4096);
    for count in 2..=6 {
        let got = signal(count, &[], 4096);
        assert!(got.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        assert!(delta(&baseline, &got) > 1.0e-4, "count {count}");
    }
    let base = signal(6, &[(25, 3.0)], 4096);
    assert!(delta(&base, &signal(6, &[(25, 3.0), (26, 1.0)], 4096)) > 1.0e-4);
    assert!(delta(&base, &signal(6, &[(25, 3.0), (27, 0.9)], 4096)) > 1.0e-4);
    assert!(delta(&base, &signal(6, &[(25, 3.0), (28, 0.9)], 4096)) > 1.0e-4);
    assert!(delta(&base, &signal(6, &[(25, 3.0), (0, 330.0)], 4096)) > 1.0e-4);
    assert!(delta(&base, &signal(6, &[(25, 3.0), (4, 1.0)], 4096)) > 1.0e-4);
}

#[test]
fn mixed_step_hold_ramp_loop_and_gate_exit_are_bounded() {
    let base = signal(
        3,
        &[(5, 0.0), (9, 2.0), (11, 0.6), (12, 0.0), (13, 0.0)],
        4096,
    );
    let looped = signal(
        3,
        &[
            (5, 0.0),
            (9, 2.0),
            (10, 1.0),
            (11, 0.6),
            (12, 0.0),
            (13, 0.0),
        ],
        4096,
    );
    assert!(delta(&base, &looped) > 1.0e-4);
    let step = signal(3, &[(5, 1.0), (7, 0.8), (9, 0.0)], 4096);
    assert!(delta(&base, &step) > 1.0e-4);
    let held = signal(3, &[(5, 2.0), (7, 0.8), (8, 0.0)], 4096);
    assert!(delta(&base, &held) > 1.0e-4);
    let no_gate = signal(3, &[(2, 0.0), (9, 2.0), (10, 1.0)], 4096);
    assert!(delta(&looped, &no_gate) > 1.0e-4);
}

#[test]
fn gate_fall_exits_loop_and_next_rise_restarts_chain() {
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
    for (i, port) in catalog::ports(&Node::StageChain).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[1] = Inp::Val(3.0);
    ins[7] = Inp::Val(0.0); // fast first ramp
    ins[9] = Inp::Val(2.0); // hold
    ins[10] = Inp::Val(1.0); // loop boundary
    ins[11] = Inp::Val(0.7);
    ins[12] = Inp::Val(0.0); // fast hold
    ins[13] = Inp::Val(0.0); // final ramp
    ins[15] = Inp::Val(0.0);
    let mut st = NodeState::default();
    let mut mem = [0.0; stage_chain::STATE_FLOATS];
    let mut out = [0.0; 2048];
    stage_chain::render(&ins, &mut st, &mut mem, &mut out, &kx);
    assert_eq!(mem[0], 1.0, "loops on the marked hold");
    ins[2] = Inp::Val(0.0);
    stage_chain::render(&ins, &mut st, &mut mem, &mut out, &kx);
    assert_eq!(mem[0], 2.0, "gate fall exits after loop end");
    ins[2] = Inp::Val(1.0);
    stage_chain::render(&ins, &mut st, &mut mem, &mut out[..1], &kx);
    assert_eq!(mem[0], 0.0, "gate rise restarts segment zero");
}

#[test]
fn step_advances_on_next_gate_edge() {
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
        seed: 1,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::StageChain).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[1] = Inp::Val(2.0);
    ins[5] = Inp::Val(1.0);
    ins[7] = Inp::Val(0.8);
    let mut st = NodeState::default();
    let mut mem = [0.0; stage_chain::STATE_FLOATS];
    let mut out = [0.0; 128];
    stage_chain::render(&ins, &mut st, &mut mem, &mut out, &kx);
    assert_eq!(mem[0], 0.0);
    ins[2] = Inp::Val(0.0);
    stage_chain::render(&ins, &mut st, &mut mem, &mut out, &kx);
    ins[2] = Inp::Val(1.0);
    stage_chain::render(&ins, &mut st, &mut mem, &mut out[..1], &kx);
    assert_eq!(mem[0], 1.0);
}

#[test]
fn final_port_codec_fixed_budget_and_native_browser_matrix() {
    assert_eq!(catalog::ports(&Node::StageChain).len(), 29);
    assert!(catalog::ports(&Node::StageChain).len() <= MAX_PORTS);
    let last = catalog::port_ctl(&Node::StageChain, 28).unwrap();
    assert_eq!(catalog::port_of(&Node::StageChain, last), Some(28));
    let def = voice(6.0);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    let budget = 2 * stage_chain::STATE_FLOATS;
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
            let (main, aux) = native.run(32);
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
            let (main, aux) = browser.run(32);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7 && main != aux);
        }
    }
}
