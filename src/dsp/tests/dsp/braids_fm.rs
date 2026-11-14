//! Analytic FM positions 25–27 and host contracts.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{braids_fm, Inp, Kx, NodeState, RawGraph, Template, MAX_PORTS};

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::BraidsFm].into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

fn render(values: [f32; 6]) -> [f32; 4096] {
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
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (input, value) in ins.iter_mut().zip(values) {
        *input = Inp::Val(value);
    }
    let mut state = NodeState::default();
    let mut memory = [0.0; braids_fm::STATE_FLOATS];
    let mut output = [0.0; 4096];
    for block in output.chunks_exact_mut(256) {
        braids_fm::render(&ins, &mut state, &mut memory, block, &kx);
    }
    output
}

#[test]
fn three_shapes_and_all_sound_controls_respond_without_nans() {
    let mut shapes = Vec::new();
    for position in 25..=27 {
        #[allow(clippy::cast_precision_loss)]
        let base = [227.0, position as f32, 0.4, 0.7, 0.4, 0.35];
        let original = render(base);
        assert_eq!(original, render(base), "deterministic event reset");
        assert!(original.iter().all(|sample| sample.is_finite()));
        assert!(rms(&original) > 1.0e-4);
        for (port, value) in [(0, 310.0), (2, 0.85), (3, 0.25), (4, 0.9), (5, 0.8)] {
            let mut changed = base;
            changed[port] = value;
            let diff: f32 = original
                .iter()
                .zip(render(changed))
                .map(|(a, b)| (a - b).abs())
                .sum();
            assert!(diff > 0.01, "shape {position}, port {port}: {diff}");
        }
        shapes.push(original);
    }
    for a in 0..3 {
        for b in a + 1..3 {
            assert_ne!(shapes[a], shapes[b], "shape {a} vs {b}");
        }
    }
    let base = [227.0, 25.0, 0.4, 0.7, 0.4, 0.35];
    let mut below = base;
    below[1] = 24.0;
    assert_eq!(render(base), render(below), "lower selector endpoint");
    let mut high = base;
    high[1] = 27.0;
    let mut above = high;
    above[1] = 28.0;
    assert_eq!(render(high), render(above), "upper selector endpoint");
}

#[test]
fn native_and_browser_codec_render_rates_and_blocks_without_allocations() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert_eq!(native.mem_total, braids_fm::STATE_FLOATS);
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
            rig.send(event(1, rig.engine.now(), &[(ctl::FREQ, 227.0)]));
            let (left, _) = rig.run(20);
            assert!(left.iter().all(|sample| sample.is_finite()));
            assert!(rms(&left) > 1.0e-4);

            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 227.0)]));
            let (left, _) = browser.run(20);
            assert!(left.iter().all(|sample| sample.is_finite()));
            assert!(rms(&left) > 1.0e-4);
        }
    }
}
