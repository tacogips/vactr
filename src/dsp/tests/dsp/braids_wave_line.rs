//! Original line/chord replacements at Braids positions 39–40.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{braids_wave_line, Inp, Kx, NodeState, RawGraph, Template, MAX_PORTS};

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::BraidsWaveLine].into_boxed_slice(),
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
    let mut memory = [0.0; braids_wave_line::STATE_FLOATS];
    let mut output = [0.0; 4096];
    for block in output.chunks_exact_mut(256) {
        braids_wave_line::render(&ins, &mut state, &mut memory, block, &kx);
    }
    output
}

#[test]
fn line_and_chord_roles_and_all_sound_controls_respond_without_nans() {
    let mut shapes = Vec::new();
    for position in 39..=40 {
        #[allow(clippy::cast_precision_loss)]
        let base = [240.0, position as f32, 0.4, 0.7, 0.4, 0.35];
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
    for a in 0..2 {
        for b in a + 1..2 {
            assert_ne!(shapes[a], shapes[b], "shape {a} vs {b}");
        }
    }
    let base = [240.0, 39.0, 0.4, 0.7, 0.4, 0.35];
    let mut below = base;
    below[1] = 38.0;
    assert_eq!(render(base), render(below), "lower selector endpoint");
    let mut high = base;
    high[1] = 40.0;
    let mut above = high;
    above[1] = 41.0;
    assert_eq!(render(high), render(above), "upper selector endpoint");
}

#[test]
fn line_endpoints_and_midpoint_interpolate_computed_nodes() {
    let phase = 0.173;
    let freq = 220.0;
    let sr = 48_000.0;
    for (scan, index) in [(0.0, 0), (1.0, 63)] {
        let actual = braids_wave_line::line_wave(scan, 0.0, phase, freq, sr);
        let expected = braids_wave_line::line_cell(index, phase, freq, sr);
        assert!((actual - expected).abs() < 1.0e-6);
    }
    let mid = braids_wave_line::line_wave(0.5 / 63.0, 0.0, phase, freq, sr);
    let expected = 0.5
        * (braids_wave_line::line_cell(0, phase, freq, sr)
            + braids_wave_line::line_cell(1, phase, freq, sr));
    assert!((mid - expected).abs() < 1.0e-6);
    let rough = braids_wave_line::line_wave(0.5 / 63.0, 1.0, phase, freq, sr);
    assert!((rough - braids_wave_line::line_cell(0, phase, freq, sr)).abs() < 1.0e-6);
}

#[test]
fn chord_family_and_inversion_ranges_are_distinct() {
    let plain = braids_wave_line::chord_offsets(0.1);
    let other = braids_wave_line::chord_offsets(0.3);
    let inverted = braids_wave_line::chord_offsets(0.6);
    assert_ne!(plain, other);
    assert_eq!(inverted[0], plain[0] - 12.0);
    assert_eq!(inverted[1..], plain[1..]);
}

#[test]
fn line_scan_smoothing_has_the_same_time_constant_across_rates() {
    let mut positions = Vec::new();
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        let store = SampleStore::new(StoreKind::NativeArc);
        let caps = caps();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr: rate,
            gate: 1024,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        for (input, value) in ins.iter_mut().zip([220.0, 39.0, 1.0, 0.0, 0.0, 0.0]) {
            *input = Inp::Val(value);
        }
        let mut state = NodeState::default();
        let mut memory = [0.0; braids_wave_line::STATE_FLOATS];
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (rate * 0.002) as usize;
        let mut output = vec![0.0; frames];
        braids_wave_line::render(&ins, &mut state, &mut memory, &mut output, &kx);
        positions.push(memory[6]);
    }
    for pair in positions.windows(2) {
        assert!((pair[0] - pair[1]).abs() < 5.0e-4, "{pair:?}");
    }
}

#[test]
fn native_and_browser_codec_render_rates_and_blocks_without_allocations() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert_eq!(native.mem_total, braids_wave_line::STATE_FLOATS);
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
            rig.send(event(1, rig.engine.now(), &[(ctl::FREQ, 240.0)]));
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
            browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 240.0)]));
            let (left, _) = browser.run(20);
            assert!(left.iter().all(|sample| sample.is_finite()));
            assert!(rms(&left) > 1.0e-4);
        }
    }
}
