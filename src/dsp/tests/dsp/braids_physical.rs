//! Original string/wind positions 28–31 and host graph contracts.

use super::{caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    braids_physical, BuildError, Inp, Kx, NodeState, RawGraph, Template, MAX_PORTS,
};

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::BraidsPhysical].into_boxed_slice(),
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
    let mut memory = vec![0.0; braids_physical::mem_len(48_000.0)];
    let mut output = [0.0; 4096];
    for block in output.chunks_exact_mut(256) {
        braids_physical::render(&ins, &mut state, &mut memory, block, &kx);
    }
    output
}

#[test]
fn resonator_capacity_tracks_host_rate() {
    assert!(braids_physical::mem_len(44_100.0) < braids_physical::mem_len(48_000.0));
    assert!(braids_physical::mem_len(48_000.0) < braids_physical::mem_len(96_000.0));
}

#[test]
fn lowest_supported_pitch_fits_the_preallocated_period() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        let store = SampleStore::new(StoreKind::NativeArc);
        let caps = caps();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr: rate,
            gate: 256,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        for (input, value) in ins.iter_mut().zip([20.0, 28.0, 0.5, 0.5, 0.8, 0.0]) {
            *input = Inp::Val(value);
        }
        let mut memory = vec![0.0; braids_physical::mem_len(rate)];
        let mut state = NodeState::default();
        let mut output = [0.0; 256];
        for _ in 0..64 {
            braids_physical::render(&ins, &mut state, &mut memory, &mut output, &kx);
            assert!(output.iter().all(|sample| sample.is_finite()));
        }
        assert!(memory.len() > (rate / 20.0) as usize);
    }
}

#[test]
fn rejected_state_budget_preserves_a_live_native_instrument() {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.voice_seconds = 0.001;
    let mut rig = NativeRig::native_with(cfg);
    rig.install(&chain(1, vec![UGenSpec::SinOsc]));
    let _ = rig.step();
    assert_eq!(
        Template::from_inst(&voice(), &rig.engine.build_env()).unwrap_err(),
        BuildError::MemExceeded
    );
    rig.send(event(1, rig.engine.now(), &[(ctl::FREQ, 220.0)]));
    let (left, _) = rig.run(8);
    assert!(rms(&left) > 1.0e-4);
}

#[test]
fn four_shapes_and_all_sound_controls_respond_without_nans() {
    let mut shapes = Vec::new();
    for position in 28..=31 {
        #[allow(clippy::cast_precision_loss)]
        let base = [220.0, position as f32, 0.4, 0.7, 0.4, 0.35];
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
    for a in 0..4 {
        for b in a + 1..4 {
            assert_ne!(shapes[a], shapes[b], "shape {a} vs {b}");
        }
    }
    let base = [220.0, 28.0, 0.4, 0.7, 0.4, 0.35];
    let mut below = base;
    below[1] = 27.0;
    assert_eq!(render(base), render(below), "lower selector endpoint");
    let mut high = base;
    high[1] = 31.0;
    let mut above = high;
    above[1] = 32.0;
    assert_eq!(render(high), render(above), "upper selector endpoint");
}

#[test]
fn native_and_browser_codec_render_rates_and_blocks_without_allocations() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert_eq!(native.mem_total, braids_physical::mem_len(48_000.0));
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
