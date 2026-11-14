//! Original coupled-FM percussion across the native/browser graph contract.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, feedback_metal, Inp, Kx, Node, NodeState, RawGraph, MAX_PORTS};

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: Box::new([UGenSpec::FeedbackMetal]),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

#[test]
fn codec_and_native_browser_rate_block_matrix() {
    let def = voice();
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    assert_eq!(raw.nodes[0], Node::FeedbackMetal);

    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut native_cfg = config(&caps(), StoreKind::NativeArc);
            native_cfg.sample_rate = sr;
            native_cfg.max_block = block;
            let mut native = NativeRig::native_with(native_cfg);
            native.install(&def);
            let _ = native.step();
            native.send(event(
                1,
                native.engine.now(),
                &[(ctl::FREQ, 55.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = native.run(12);
            assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()));
            assert!(rms(&l) > 1.0e-4, "native {sr}/{block}");

            let mut browser_cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            browser_cfg.sample_rate = sr;
            browser_cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(browser_cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.step();
            browser.send(event(
                1,
                browser.engine.now(),
                &[(ctl::FREQ, 55.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = browser.run(12);
            assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()));
            assert!(rms(&l) > 1.0e-4, "browser {sr}/{block}");
        }
    }
}

fn signal(sr: f32, overrides: &[(usize, f32)], seed: u32) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: 0,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::FeedbackMetal).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    for &(port, value) in overrides {
        ins[port] = Inp::Val(value);
    }
    let mut state = NodeState::default();
    let mut out = vec![0.0; 4096];
    feedback_metal::render(&ins, &mut state, &mut out, &kx);
    out
}

#[test]
fn deterministic_restart_extremes_and_low_notes_are_finite() {
    let a = signal(48_000.0, &[], 55);
    assert_eq!(
        a,
        signal(48_000.0, &[], 55),
        "fresh hit resets deterministically"
    );
    assert_ne!(a, signal(48_000.0, &[], 56), "noise follows the event seed");
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for freq in [20.0, 35.0, 55.0] {
            let low = signal(sr, &[(0, freq)], 55);
            assert!(low.iter().all(|v| v.is_finite()), "{sr} Hz / {freq} Hz");
            assert!(rms(&low) > 1.0e-4, "{sr} Hz / {freq} Hz");
        }
        let extreme = signal(
            sr,
            &[
                (0, f32::NAN),
                (2, f32::INFINITY),
                (3, 1000.0),
                (4, -1.0),
                (10, f32::NEG_INFINITY),
                (12, 1.0e9),
                (14, 9.0),
            ],
            55,
        );
        assert!(extreme.iter().all(|v| v.is_finite()), "extremes at {sr} Hz");
    }
}

#[test]
fn a_second_scheduled_hit_restarts_the_percussion_network() {
    let render = |second: bool| {
        let mut rig = NativeRig::native();
        rig.install(&voice());
        let _ = rig.step();
        rig.send(event(
            1,
            rig.engine.now(),
            &[(ctl::FREQ, 180.0), (ctl::LEGATO, 1.0)],
        ));
        let _ = rig.run(8);
        if second {
            rig.send(event(
                1,
                rig.engine.now(),
                &[(ctl::FREQ, 180.0), (ctl::LEGATO, 1.0)],
            ));
        }
        rig.run(8).0
    };
    let one = render(false);
    let two = render(true);
    assert!(one.iter().chain(two.iter()).all(|x| x.is_finite()));
    let difference = one
        .iter()
        .zip(&two)
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / one.len() as f32;
    assert!(difference > 0.01, "second hit must restart: {difference}");
}
