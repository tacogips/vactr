//! Six-particle adaptation: deterministic reset, audio paths, and host codec.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{particle_pair, Inp, Kx, NodeState, RawGraph, Template, MAX_PORTS};

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::ParticlePair,
            UGenSpec::ParticlePair,
            UGenSpec::AuxOut,
        ]
        .into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,

            output: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![(
            1,
            crate::dsp::ugen::catalog::port_ctl(&crate::dsp::ugen::Node::ParticlePair, 4).unwrap(),
            crate::host::wire::Ctl::Const(1.0),
        )]
        .into_boxed_slice(),
    }
}

fn assert_paths(left: &[f32], right: &[f32]) {
    assert!(left.iter().chain(right).all(|sample| sample.is_finite()));
    assert!(rms(left) > 1.0e-5, "main audible");
    assert!(rms(right) > 1.0e-4, "impulse aux audible");
    #[allow(clippy::cast_precision_loss)]
    let difference = left
        .iter()
        .zip(right)
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / left.len() as f32;
    assert!(difference > 1.0e-4, "independent outputs: {difference}");
}

#[test]
fn reset_is_deterministic_and_core_controls_respond() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 256,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 18,
    };
    let render = |values: [f32; 5], kx: &Kx<'_>| {
        let mut inputs = [Inp::Val(0.0); MAX_PORTS];
        for (input, value) in inputs.iter_mut().zip(values) {
            *input = Inp::Val(value);
        }
        let mut state = NodeState::default();
        let mut mem = vec![0.0; particle_pair::STATE_FLOATS];
        let mut output = vec![0.0; 8192];
        for block in output.chunks_exact_mut(256) {
            particle_pair::render(&inputs, &mut state, &mut mem, block, kx);
        }
        output
    };
    for mode in [0.0, 1.0] {
        let base = [220.0, 0.5, 0.7, 0.3, mode];
        let original = render(base, &kx);
        assert_eq!(original, render(base, &kx), "event reset is deterministic");
        for (port, value) in [(0, 330.0), (1, 0.9), (2, 0.95), (3, 0.7)] {
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
            if mode == 0.0 || port == 2 {
                assert!(
                    difference > 1.0e-6,
                    "mode {mode}, port {port}: {difference}"
                );
            }
        }
    }
}

#[test]
fn native_particle_voice_survives_rates_and_blocks() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = NativeRig::native_with(cfg);
            rig.install(&voice());
            let _ = rig.step();
            rig.send(event(1, rig.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (left, right) = rig.run(20);
            assert_paths(&left, &right);
        }
    }
}

#[test]
fn browser_particle_codec_survives_rates_and_blocks() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert!(native.has_aux);
    assert_eq!(native.mem_total, 2 * particle_pair::STATE_FLOATS);
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
            rig.send(event(1, rig.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (left, right) = rig.run(20);
            assert_paths(&left, &right);
        }
    }
}
