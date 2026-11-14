//! Three-string resonator reset, dual outputs and host serialization.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, string_pair, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::StringPair, UGenSpec::StringPair, UGenSpec::AuxOut]
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
            catalog::port_ctl(&Node::StringPair, 5).unwrap(),
            Ctl::Const(1.0),
        )]
        .into_boxed_slice(),
    }
}

fn assert_paths(left: &[f32], right: &[f32]) {
    assert!(left.iter().chain(right).all(|sample| sample.is_finite()));
    assert!(rms(left) > 1.0e-6, "string main audible");
    assert!(rms(right) > 1.0e-5, "excitation aux audible");
    #[allow(clippy::cast_precision_loss)]
    let difference = left
        .iter()
        .zip(right)
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / left.len() as f32;
    assert!(difference > 1.0e-5, "separate outputs: {difference}");
}

#[test]
fn string_reset_and_both_modes_are_deterministic() {
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
        seed: 20,
    };
    for mode in [0.0, 1.0] {
        let mut inputs = [Inp::Val(0.0); MAX_PORTS];
        for (input, value) in inputs
            .iter_mut()
            .zip([220.0, 0.5, 0.5, 0.5, 1.0, mode, 0.0])
        {
            *input = Inp::Val(value);
        }
        let mut state = NodeState::default();
        let mut memory = [0.0; string_pair::STATE_FLOATS];
        let mut first = [0.0; 256];
        let mut next = [0.0; 256];
        string_pair::render(&inputs, &mut state, &mut memory, &mut first, &kx);
        string_pair::render(&inputs, &mut state, &mut memory, &mut next, &kx);
        let mut reset = [0.0; 256];
        state = NodeState::default();
        memory.fill(0.0);
        string_pair::render(&inputs, &mut state, &mut memory, &mut reset, &kx);
        assert_eq!(first, reset, "mode {mode}: deterministic reset");
        assert_ne!(first, next, "mode {mode}: state advanced");
    }
}

#[test]
fn native_string_voice_survives_rates_and_blocks() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = rate;
            cfg.max_block = block;
            cfg.voice_seconds = 0.5;
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
fn browser_string_codec_survives_rates_and_blocks() {
    let def = voice();
    let mut native_cfg = config(&caps(), StoreKind::NativeArc);
    native_cfg.voice_seconds = 0.5;
    let env = NativeRig::native_with(native_cfg).engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert!(native.has_aux);
    assert_eq!(native.mem_total, 2 * string_pair::STATE_FLOATS);
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
            cfg.voice_seconds = 0.5;
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
