//! Braids positions 5–8: analytic sub-octave and dual-sync response.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{braids_subsync, Inp, Kx, NodeState, RawGraph, Template, MAX_PORTS};

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::BraidsSubSync].into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

fn render(values: [f32; 6]) -> [f32; 2048] {
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
    for (input, value) in ins.iter_mut().zip(values) {
        *input = Inp::Val(value);
    }
    let mut state = NodeState::default();
    let mut memory = [0.0; braids_subsync::STATE_FLOATS];
    let mut output = [0.0; 2048];
    for block in output.chunks_exact_mut(256) {
        braids_subsync::render(&ins, &mut state, &mut memory, block, &kx);
    }
    output
}

#[test]
fn shapes_and_every_sound_control_are_distinct_and_finite() {
    let mut shapes = Vec::new();
    for position in 5..=8 {
        #[allow(clippy::cast_precision_loss)]
        let base = [220.0, position as f32, 0.4, 0.3, 0.4, 0.35];
        let original = render(base);
        assert_eq!(original, render(base), "event reset");
        assert!(original.iter().all(|sample| sample.is_finite()));
        assert!(rms(&original) > 1.0e-4);
        for (port, value) in [(0, 310.0), (2, 0.85), (3, 0.8), (4, 0.9), (5, 0.8)] {
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
    for a in 0..4 {
        for b in a + 1..4 {
            assert_ne!(shapes[a], shapes[b], "shape {a} vs {b}");
        }
    }
    let base = [220.0, 5.0, 0.4, 0.3, 0.4, 0.35];
    let mut below = base;
    below[1] = 4.0;
    assert_eq!(render(base), render(below), "selector lower endpoint");
    let mut high = base;
    high[1] = 8.0;
    let mut above = high;
    above[1] = 9.0;
    assert_eq!(render(high), render(above), "selector upper endpoint");
}

#[test]
fn sub_octave_switch_and_master_slave_sync_are_explicit() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 210,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let phase_after = |shape: f32, timbre: f32, frames: usize| {
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        for (input, value) in ins.iter_mut().zip([480.0, shape, 0.6, timbre, 0.0, 0.0]) {
            *input = Inp::Val(value);
        }
        let mut memory = [0.0; braids_subsync::STATE_FLOATS];
        let mut out = [0.0; 210];
        braids_subsync::render(
            &ins,
            &mut NodeState::default(),
            &mut memory,
            &mut out[..frames],
            &kx,
        );
        memory[1]
    };
    let two_below = phase_after(5.0, 0.0, 100);
    let one_below = phase_after(5.0, 1.0, 100);
    assert!((two_below - 0.25).abs() < 0.02, "two octaves: {two_below}");
    assert!((one_below - 0.5).abs() < 0.02, "one octave: {one_below}");
    let after_one = phase_after(8.0, 0.7, 110);
    let after_two = phase_after(8.0, 0.7, 210);
    assert!(
        (after_one - after_two).abs() < 0.02,
        "slave resets each master cycle"
    );
}

#[test]
fn native_and_browser_codec_render_rates_and_blocks_without_allocations() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert_eq!(native.mem_total, braids_subsync::STATE_FLOATS);
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
            browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (left, _) = browser.run(20);
            assert!(left.iter().all(|sample| sample.is_finite()));
            assert!(rms(&left) > 1.0e-4);
        }
    }
}
