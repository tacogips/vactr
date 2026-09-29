//! Original programmable digital drum kernel: codec/rate-block matrix,
//! deterministic restart, and internal repeat retriggering, for both
//! `digital-drum-core` and `digital-snare-core` (design-music 4.1,
//! DDRUM-001/003).

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, digital_drum, Inp, Kx, Node, NodeState, RawGraph, MAX_PORTS};

fn voice(spec: UGenSpec) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: Box::new([spec]),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

#[test]
fn codec_and_native_browser_rate_block_matrix() {
    for (spec, node) in [
        (UGenSpec::DigitalDrumCore, Node::DigitalDrumCore),
        (UGenSpec::DigitalSnareCore, Node::DigitalSnareCore),
    ] {
        let def = voice(spec.clone());
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        assert_eq!(raw.nodes[0], node);

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
                    &[(ctl::FREQ, 180.0), (ctl::LEGATO, 1.0)],
                ));
                let (l, r) = native.run(16);
                assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()));
                assert!(rms(&l) > 1.0e-4, "{node:?} native {sr}/{block}");

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
                    &[(ctl::FREQ, 180.0), (ctl::LEGATO, 1.0)],
                ));
                let (l, r) = browser.run(16);
                assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()));
                assert!(rms(&l) > 1.0e-4, "{node:?} browser {sr}/{block}");
            }
        }
    }
}

type Render = fn(&[Inp<'_>; MAX_PORTS], &mut NodeState, &mut [f32], &mut [f32], &Kx<'_>);

fn signal(render: Render, node: &Node, sr: f32, overrides: &[(&str, f32)], seed: u32) -> Vec<f32> {
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
    let ports = catalog::ports(node);
    for (i, port) in ports.iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    for &(name, value) in overrides {
        let i = ports
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| {
                panic!("{name}: no port on {node:?}");
            });
        ins[i] = Inp::Val(value);
    }
    let mut state = NodeState::default();
    let mut mem = vec![0.0; digital_drum::STATE_FLOATS];
    let mut out = vec![0.0; 8192];
    render(&ins, &mut state, &mut mem, &mut out, &kx);
    out
}

#[test]
fn deterministic_restart_and_low_notes_are_finite() {
    for (render, node) in [
        (digital_drum::render_drum as Render, Node::DigitalDrumCore),
        (digital_drum::render_snare as Render, Node::DigitalSnareCore),
    ] {
        let a = signal(render, &node, 48_000.0, &[], 7);
        assert_eq!(
            a,
            signal(render, &node, 48_000.0, &[], 7),
            "{node:?}: fresh hit resets deterministically"
        );
        assert_ne!(
            a,
            signal(render, &node, 48_000.0, &[], 8),
            "{node:?}: noise follows the event seed"
        );
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for freq in [30.0, 80.0, 220.0] {
                let low = signal(render, &node, sr, &[("freq", freq)], 7);
                assert!(low.iter().all(|v| v.is_finite()), "{node:?} {sr}/{freq}");
                assert!(rms(&low) > 1.0e-4, "{node:?} {sr}/{freq}");
            }
        }
        let extreme = signal(
            render,
            &node,
            48_000.0,
            &[
                ("freq", f32::NAN),
                ("coarse", f32::INFINITY),
                ("fm-depth", 1.0e9),
                ("lfo-rate", f32::NEG_INFINITY),
                ("velocity-depth", 1.0e6),
                ("decimation", 2.0),
                ("repeat-count", 20.0),
            ],
            7,
        );
        assert!(
            extreme.iter().all(|v| v.is_finite()),
            "{node:?}: extremes stay finite"
        );
    }
}

#[test]
fn repeats_retrigger_the_envelopes_and_the_voice_ends() {
    for (render, node) in [
        (digital_drum::render_drum as Render, Node::DigitalDrumCore),
        (digital_drum::render_snare as Render, Node::DigitalSnareCore),
    ] {
        let sr = 48_000.0;
        let once = signal(render, &node, sr, &[("repeat-count", 0.0)], 3);
        let twice = signal(
            render,
            &node,
            sr,
            &[("repeat-count", 1.0), ("repeat-time", 0.05)],
            3,
        );
        assert!(once.iter().all(|v| v.is_finite()));
        assert!(twice.iter().all(|v| v.is_finite()));
        // The repeat re-attacks around 0.05 s in: a fresh onset there
        // raises the local level well above the single hit's decayed tail.
        let at = (0.06 * sr) as usize;
        assert!(
            twice[at].abs() + twice[at + 1].abs() + twice[at + 2].abs()
                > once[at].abs() + once[at + 1].abs() + once[at + 2].abs() + 1.0e-3,
            "{node:?}: the repeat re-attacks the envelopes"
        );

        // A one-shot voice (`repeat-count: 0`) finishes well inside 8192
        // frames at a short decay; a later block is exact silence.
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
            seed: 3,
        };
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        for (i, port) in catalog::ports(&node).iter().enumerate() {
            ins[i] = Inp::Val(port.default);
        }
        for name in ["amp-decay", "mod-decay", "pitch-decay"] {
            let port = catalog::ports(&node)
                .iter()
                .position(|p| p.name == name)
                .unwrap();
            ins[port] = Inp::Val(0.01);
        }
        let mut state = NodeState::default();
        let mut mem = vec![0.0; digital_drum::STATE_FLOATS];
        let mut out = vec![0.0; sr as usize];
        render(&ins, &mut state, &mut mem, &mut out, &kx);
        assert!(state.done(), "{node:?}: a short one-shot voice finishes");
        assert!(
            out[out.len() - 100..].iter().all(|&v| v == 0.0),
            "{node:?}: a finished voice renders silence"
        );
    }
}
