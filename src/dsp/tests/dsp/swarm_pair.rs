//! Eight-voice swarm reset, accent response and native/browser codec.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, swarm_pair, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::SwarmPair, UGenSpec::SwarmPair, UGenSpec::AuxOut].into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![(
            1,
            catalog::port_ctl(&Node::SwarmPair, 4).unwrap(),
            Ctl::Const(1.0),
        )]
        .into_boxed_slice(),
    }
}

fn assert_paths(l: &[f32], r: &[f32]) {
    assert!(l.iter().chain(r).all(|x| x.is_finite()));
    assert!(rms(l) > 0.0001 && rms(r) > 0.0001);
    #[allow(clippy::cast_precision_loss)]
    let delta = l.iter().zip(r).map(|(a, b)| (a - b).abs()).sum::<f32>() / l.len() as f32;
    assert!(delta > 0.001, "saw/sine outputs differ: {delta}");
}

#[test]
fn retrigger_resets_both_paths_and_mode_changes_grains() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 64,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 123,
    };
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    inputs[0] = Inp::Val(220.0);
    inputs[1] = Inp::Val(0.5);
    inputs[2] = Inp::Val(0.5);
    inputs[3] = Inp::Val(0.5);
    inputs[4] = Inp::Val(0.0);
    for mode in [0.0, 1.0] {
        inputs[4] = Inp::Val(mode);
        let mut state = NodeState::default();
        let mut mem = [0.0; swarm_pair::STATE_FLOATS];
        let mut first = [0.0; 64];
        let mut continued = [0.0; 64];
        let mut reset = [0.0; 64];
        swarm_pair::render(&inputs, &mut state, &mut mem, &mut first, &kx);
        swarm_pair::render(&inputs, &mut state, &mut mem, &mut continued, &kx);
        state = NodeState::default();
        mem.fill(0.0);
        swarm_pair::render(&inputs, &mut state, &mut mem, &mut reset, &kx);
        assert_eq!(first, reset, "mode {mode} retriggers exactly");
        assert_ne!(continued, reset);
        inputs[5] = Inp::Val(1.0);
        state = NodeState::default();
        mem.fill(0.0);
        swarm_pair::render(&inputs, &mut state, &mut mem, &mut reset, &kx);
        assert_ne!(first, reset, "mode {mode} responds to continuous selection");
        inputs[5] = Inp::Val(0.0);
    }
}

#[test]
fn both_source_paths_respond_to_every_sound_port() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 4096,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 874,
    };
    let render = |values: [f32; 6], kx: &Kx<'_>| {
        let mut inputs = [Inp::Val(0.0); MAX_PORTS];
        for (input, value) in inputs.iter_mut().zip(values) {
            *input = Inp::Val(value);
        }
        let mut state = NodeState::default();
        let mut mem = [0.0; swarm_pair::STATE_FLOATS];
        let mut out = [0.0; 4096];
        for block in out.chunks_exact_mut(256) {
            swarm_pair::render(&inputs, &mut state, &mut mem, block, kx);
        }
        out
    };
    for mode in [0.0, 1.0] {
        let base = [100.0, 0.5, 0.5, 0.5, mode, 0.0];
        let original = render(base, &kx);
        for (port, value) in [(0, 170.0), (1, 0.9), (2, 0.9), (3, 0.9), (5, 1.0)] {
            let mut changed = base;
            changed[port] = value;
            let output = render(changed, &kx);
            #[allow(clippy::cast_precision_loss)]
            let difference = original
                .iter()
                .zip(output)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / original.len() as f32;
            assert!(
                difference > 1.0e-5,
                "mode {mode}, port {port}: {difference}"
            );
        }
    }
}

#[test]
fn native_swarm_pair_outputs_survive_rates_and_blocks() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = NativeRig::native_with(cfg);
            rig.install(&voice());
            let _ = rig.step();
            rig.send(event(
                1,
                rig.engine.now(),
                &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = rig.run(20);
            assert_paths(&l, &r);
        }
    }
}

#[test]
fn browser_swarm_pair_codec_preserves_two_outputs() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert!(native.has_aux);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    decode_graph(&bytes, &mut raw, &mut bus).unwrap();
    let mut decoded = Template::boxed();
    decoded.build(&raw, &env).unwrap();
    assert_eq!(decoded.nodes(), native.nodes());

    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            rig.push(&record);
            let _ = rig.run(6);
            rig.send(event(
                1,
                rig.engine.now(),
                &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = rig.run(20);
            assert_paths(&l, &r);
        }
    }
}
