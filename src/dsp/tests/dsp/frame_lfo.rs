//! Frames poly-LFO adaptation: inventory, controls, lanes and host contract.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{frames_paths, FramesCoverage, EASING_OPTIONS, KEYFRAME_CAPACITY};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, frame_lfo, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(main: f32, aux: f32) -> InstDef {
    let node = Node::FrameLfo;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::FrameLfo, UGenSpec::FrameLfo, UGenSpec::AuxOut].into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![
            (0, catalog::port_ctl(&node, 6).unwrap(), Ctl::Const(main)),
            (1, catalog::port_ctl(&node, 6).unwrap(), Ctl::Const(aux)),
        ]
        .into_boxed_slice(),
    }
}

fn signal(changes: &[(usize, f32)]) -> Vec<f32> {
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
    for (i, port) in catalog::ports(&Node::FrameLfo).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[0] = Inp::Val(200.0);
    for &(port, value) in changes {
        ins[port] = Inp::Val(value);
    }
    let mut st = NodeState::default();
    let mut mem = [0.0; frame_lfo::STATE_FLOATS];
    let mut out = vec![0.0; 4096];
    frame_lfo::render(&ins, &mut st, &mut mem, &mut out, &kx);
    out
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn inventory_separates_firmware_control_from_analog_audio() {
    let paths = frames_paths();
    assert_eq!(paths[0].coverage, FramesCoverage::Adaptation);
    assert_eq!(paths[0].simultaneous_vactr_channels, 2);
    assert_eq!(paths[0].opt_in_quad_template, Some("frame-lfo-quad-voice"));
    assert!(paths[0].source_generated_wave_table);
    assert_eq!(paths[1].coverage, FramesCoverage::Adaptation);
    assert_eq!(paths[1].vactr_template, Some("frame-keyframe-voice"));
    assert_eq!(
        paths[1].opt_in_quad_template,
        Some("frame-keyframe-quad-voice")
    );
    assert_eq!(KEYFRAME_CAPACITY, 64);
    assert_eq!(EASING_OPTIONS.len(), 6);
    assert_eq!(paths[2].coverage, FramesCoverage::Excluded);
}

#[test]
fn four_lanes_and_every_sound_control_respond() {
    let base = signal(&[(6, 2.0)]);
    assert!(rms(&base) > 0.01);
    for lane in [0, 1, 3] {
        assert!(
            delta(&base, &signal(&[(6, lane as f32)])) > 1.0e-3,
            "lane {lane}"
        );
    }
    for (port, value) in [(0, 330.0), (1, 0.9), (2, 0.1), (3, 0.9), (4, 0.9), (5, 0.3)] {
        assert!(
            delta(&base, &signal(&[(6, 2.0), (port, value)])) > 1.0e-3,
            "port {port}"
        );
    }
    let negative_spread = signal(&[(2, 0.1)]);
    assert!(negative_spread
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
}

#[test]
fn codec_memory_and_native_browser_matrix() {
    assert_eq!(catalog::ports(&Node::FrameLfo).len(), 7);
    let last = catalog::port_ctl(&Node::FrameLfo, 6).unwrap();
    assert_eq!(catalog::port_of(&Node::FrameLfo, last), Some(6));
    let def = voice(0.0, 3.0);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    let budget = 2 * frame_lfo::STATE_FLOATS;
    let env = BuildEnv {
        sr: 96_000.0,
        caps: caps(),
        voice_mem: budget,
    };
    let mut built = Template::boxed();
    built.build(&raw, &env).unwrap();
    assert_eq!(built.mem_total, budget);
    assert!(built.has_aux);
    assert!(Template::from_inst(
        &def,
        &BuildEnv {
            voice_mem: budget - 1,
            ..env
        }
    )
    .is_err());
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut native = NativeRig::native_with(cfg);
            native.install(&def);
            let _ = native.step();
            native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (main, aux) = native.run(16);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7 && main != aux);
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (main, aux) = browser.run(16);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7 && main != aux);
        }
    }
}

#[test]
fn main_and_aux_selectors_are_independent() {
    let render = |main: f32, aux: f32| {
        let mut rig = NativeRig::native_with(config(&caps(), StoreKind::NativeArc));
        rig.install(&voice(main, aux));
        let _ = rig.step();
        rig.send(event(1, rig.engine.now(), &[(ctl::FREQ, 220.0)]));
        rig.run(16)
    };
    let (main0, aux1) = render(0.0, 1.0);
    let (main3, aux1_again) = render(3.0, 1.0);
    assert!(delta(&main0, &main3) > 1.0e-3);
    assert_eq!(aux1, aux1_again);
    let (main0_again, aux3) = render(0.0, 3.0);
    assert_eq!(main0, main0_again);
    assert!(delta(&aux1, &aux3) > 1.0e-3);
}
