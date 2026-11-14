//! Original six-operator banks, graph codec, and host contracts.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, six_op_original, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(bank: f32) -> InstDef {
    let bank_ctl = catalog::port_ctl(&Node::SixOpOriginal, 6).unwrap();
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::SixOpOriginal,
            UGenSpec::SixOpOriginal,
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
            (0, bank_ctl, Ctl::Const(bank)),
            (1, bank_ctl, Ctl::Const(bank)),
        ]
        .into_boxed_slice(),
    }
}

fn render(controls: [f32; 7]) -> [f32; 4096] {
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
        seed: 1,
    };
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    for (input, value) in inputs.iter_mut().zip(controls) {
        *input = Inp::Val(value);
    }
    let mut node = NodeState::default();
    let mut memory = [0.0; six_op_original::STATE_FLOATS];
    let mut output = [0.0; 4096];
    for block in output.chunks_exact_mut(256) {
        six_op_original::render(&inputs, &mut node, &mut memory, block, &kx);
    }
    output
}

#[test]
fn banks_and_all_sound_controls_produce_distinct_finite_audio() {
    let mut banks = Vec::new();
    for bank in 0..3 {
        let baseline = [220.0, 0.2, 0.45, 0.5, 0.7, 0.0, bank as f32];
        let original = render(baseline);
        assert_eq!(original, render(baseline), "event reset is deterministic");
        assert!(original.iter().all(|sample| sample.is_finite()));
        assert!(rms(&original) > 1.0e-4);
        for (port, value) in [(0, 330.0), (1, 0.8), (2, 0.9), (3, 0.9), (4, 0.2), (5, 1.0)] {
            let mut changed = baseline;
            changed[port] = value;
            let diff: f32 = original
                .iter()
                .zip(render(changed))
                .map(|(a, b)| (a - b).abs())
                .sum();
            assert!(diff > 0.1, "bank {bank}, port {port}: {diff}");
        }
        banks.push(original);
    }
    assert_ne!(banks[0], banks[1]);
    assert_ne!(banks[1], banks[2]);
    assert_ne!(banks[0], banks[2]);
}

#[test]
fn bank_selector_clamps_to_the_three_authored_banks() {
    let base = [220.0, 0.2, 0.45, 0.5, 0.7, 0.0, 0.0];
    let mut below = base;
    below[6] = -1.0;
    assert_eq!(render(base), render(below));
    let mut high = base;
    high[6] = 2.0;
    let mut above = high;
    above[6] = 3.0;
    assert_eq!(render(high), render(above));
}

#[test]
fn native_and_browser_graphs_keep_equal_main_aux_at_all_supported_rates() {
    for bank in 0..3 {
        let def = voice(bank as f32);
        let env = NativeRig::native().engine.build_env();
        let native = Template::from_inst(&def, &env).unwrap();
        assert_eq!(native.mem_total, 2 * six_op_original::STATE_FLOATS);
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
                assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
                assert!(rms(&left) > 1.0e-4 && rms(&right) > 1.0e-4);
                assert_eq!(left, right, "source engine emits equal main/aux");

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
                assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
                assert!(rms(&left) > 1.0e-4 && rms(&right) > 1.0e-4);
                assert_eq!(left, right);
            }
        }
    }
}
