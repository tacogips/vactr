//! Procedural speech models, upper-range tokens, and graph host contracts.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, speech_original, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::SpeechOriginal,
            UGenSpec::SpeechOriginal,
            UGenSpec::AuxOut,
        ]
        .into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![(
            1,
            catalog::port_ctl(&Node::SpeechOriginal, 6).unwrap(),
            Ctl::Const(1.0),
        )]
        .into_boxed_slice(),
    }
}

fn render(controls: [f32; 7]) -> [f32; 8192] {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 8192,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    for (input, value) in inputs.iter_mut().zip(controls) {
        *input = Inp::Val(value);
    }
    let mut node = NodeState::default();
    let mut memory = [0.0; speech_original::STATE_FLOATS];
    let mut output = [0.0; 8192];
    for block in output.chunks_exact_mut(256) {
        speech_original::render(&inputs, &mut node, &mut memory, block, &kx);
    }
    output
}

#[test]
fn lower_models_and_all_four_authored_upper_tokens_are_selectable() {
    assert_eq!(speech_original::model_and_word(0.0), (0, 0));
    assert_eq!(speech_original::model_and_word(1.0 / 6.0), (1, 0));
    assert_eq!(speech_original::model_and_word(1.0 / 3.0), (2, 0));
    assert_eq!(speech_original::model_and_word(1.0), (3, 3));
    let mut signals = Vec::new();
    for group in [0.0, 0.22, 1.0 / 3.0, 0.38, 0.55, 0.72, 0.95] {
        let controls = [220.0, group, 0.5, 0.4, 0.8, 0.0, 0.0];
        let signal = render(controls);
        assert_eq!(signal, render(controls));
        assert!(signal.iter().all(|sample| sample.is_finite()));
        assert!(rms(&signal) > 1.0e-5, "group {group}");
        for earlier in &signals {
            assert_ne!(
                earlier, &signal,
                "group {group} aliases an earlier model/token"
            );
        }
        signals.push(signal);
    }
}

#[test]
fn each_sound_control_and_excitation_output_change_audio() {
    for group in [0.05, 0.22, 0.33, 0.7] {
        let base = [220.0, group, 0.4, 0.35, 0.8, 0.0, 0.0];
        let original = render(base);
        for (port, value) in [
            (0, 330.0),
            (1, 0.85),
            (2, 0.85),
            (3, 0.85),
            (4, 0.2),
            (5, 1.0),
            (6, 1.0),
        ] {
            let mut changed = base;
            changed[port] = value;
            let delta: f32 = original
                .iter()
                .zip(render(changed))
                .map(|(a, b)| (a - b).abs())
                .sum();
            assert!(delta > 0.01, "group {group}, port {port}: {delta}");
        }
    }
}

#[test]
fn native_and_browser_codec_render_distinct_main_and_excitation() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert_eq!(native.mem_total, 2 * speech_original::STATE_FLOATS);
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
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = NativeRig::native_with(cfg);
            rig.install(&def);
            let _ = rig.step();
            rig.send(event(1, rig.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (left, right) = rig.run(20);
            assert!(left.iter().chain(&right).all(|x| x.is_finite()));
            assert!(rms(&left) > 1.0e-5 && rms(&right) > 1.0e-5);
            assert_ne!(left, right);

            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (left, right) = browser.run(20);
            assert!(left.iter().chain(&right).all(|x| x.is_finite()));
            assert!(rms(&left) > 1.0e-5 && rms(&right) > 1.0e-5);
            assert_ne!(left, right);
        }
    }
}
