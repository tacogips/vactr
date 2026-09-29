//! Four explicit output lanes retain their identity across both graph tiers.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, FaultCode, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, EffectKind, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, frame_keyframe::FrameData, tidal_function, BuildEnv, Inp, Kx, Node, NodeState,
    RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn quad(frames: bool) -> InstDef {
    let data = FrameData::from_flat(&[0.0, 0.1, 0.3, 0.5, 0.7, 1.0, 0.2, 0.4, 0.6, 0.8]).unwrap();
    let (core, node, channel_port) = if frames {
        (
            UGenSpec::FrameKeyframe {
                data: Some(Box::new(data)),
            },
            Node::FrameKeyframe { slot: 0 },
            9,
        )
    } else {
        (UGenSpec::TidalPoly, Node::TidalPoly, 10)
    };
    let mut params = Vec::new();
    for i in 0..4 {
        params.push((
            i as u16,
            catalog::port_ctl(&node, channel_port).unwrap(),
            Ctl::Const(i as f32),
        ));
        if frames {
            params.push((
                i as u16,
                catalog::port_ctl(&node, 0).unwrap(),
                Ctl::Const(0.37),
            ));
        } else {
            for (port, value) in [(4, 0.8), (7, 1.0), (8, 2.0), (9, 1.0)] {
                params.push((
                    i as u16,
                    catalog::port_ctl(&node, port).unwrap(),
                    Ctl::Const(value),
                ));
            }
        }
    }
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            core.clone(),
            core.clone(),
            core.clone(),
            core,
            UGenSpec::AuxOut,
            UGenSpec::Out3,
            UGenSpec::Out4,
        ]
        .into_boxed_slice(),
        edges: vec![
            Edge {
                from: 1,
                to: 4,
                port: 0,

                output: 0,
            },
            Edge {
                from: 2,
                to: 5,
                port: 0,

                output: 0,
            },
            Edge {
                from: 3,
                to: 6,
                port: 0,

                output: 0,
            },
        ]
        .into_boxed_slice(),
        node_params: params.into_boxed_slice(),
    }
}

fn check_lanes(lanes: &[Vec<f32>; 4]) {
    for lane in lanes {
        assert!(lane.iter().all(|x| x.is_finite()));
        assert!(rms(lane) > 0.001);
    }
    for i in 0..4 {
        for j in i + 1..4 {
            let delta = lanes[i]
                .iter()
                .zip(&lanes[j])
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / lanes[i].len() as f32;
            assert!(delta > 0.005, "lanes {i}/{j}: {delta}");
        }
    }
}

#[test]
fn four_lanes_native_browser_and_stereo_rejection() {
    for frames in [false, true] {
        let def = quad(frames);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        assert!(raw.nodes[..raw.n_nodes].contains(&Node::Out3));
        assert!(raw.nodes[..raw.n_nodes].contains(&Node::Out4));
        assert!(
            Template::from_inst(
                &def,
                &BuildEnv {
                    sr: 48_000.0,
                    caps: caps(),
                    voice_mem: 48_000
                }
            )
            .unwrap()
            .has_quad
        );
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                cfg.output_channels = 4;
                let mut native = NativeRig::native_with(cfg);
                native.install(&def);
                let _ = native.step();
                native.send(event(
                    1,
                    native.engine.now(),
                    &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
                ));
                check_lanes(&native.run_four(8));
                let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                cfg.output_channels = 4;
                let mut browser = BrowserRig::browser_with(cfg);
                let mut record = Vec::new();
                encode_graph_record(1, 1, &bytes, &mut record);
                browser.push(&record);
                let _ = browser.step();
                browser.send(event(
                    1,
                    browser.engine.now(),
                    &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
                ));
                check_lanes(&browser.run_four(8));
            }
        }
        let mut stereo = NativeRig::native();
        stereo.install(&def);
        let _ = stereo.step();
        assert_eq!(
            stereo.engine.pop_fault().map(|f| f.code),
            Some(FaultCode::OutputChannels)
        );
        let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
        cfg.output_channels = 2;
        let mut browser = BrowserRig::browser_with(cfg);
        let mut record = Vec::new();
        encode_graph_record(1, 1, &bytes, &mut record);
        browser.push(&record);
        let _ = browser.step();
        assert_eq!(
            browser.engine.pop_fault().map(|f| f.code),
            Some(FaultCode::OutputChannels)
        );
    }
}

#[test]
fn direct_stems_ignore_other_voices_and_master_effects() {
    let run = |other: bool, master_gain: bool| {
        let mut cfg = config(&caps(), StoreKind::NativeArc);
        cfg.output_channels = 4;
        let mut rig = NativeRig::native_with(cfg);
        rig.install(&quad(true));
        if other {
            rig.install(&chain(2, vec![UGenSpec::Const(0.25)]));
        }
        if master_gain {
            let gain = spec(EffectKind::Gain, &[("gain", Ctl::Const(-6.0206))]).unwrap();
            rig.install_bus(&bus_def(0, vec![gain]), true);
        }
        let _ = rig.step();
        rig.send(event(1, rig.engine.now(), &[(ctl::LEGATO, 1.0)]));
        if other {
            rig.send(event(2, rig.engine.now(), &[(ctl::LEGATO, 1.0)]));
        }
        rig.run_four(4)
    };
    let base = run(false, false);
    let other = run(true, false);
    let wet = run(false, true);
    for lane in 2..4 {
        assert_eq!(
            base[lane], other[lane],
            "other voice must not cross into direct stem {lane}"
        );
        assert_eq!(
            base[lane], wet[lane],
            "master FX must not alter direct stem {lane}"
        );
    }
    assert!((rms(&base[0]) - rms(&other[0])).abs() > 0.01);
    assert!(rms(&wet[0]) < rms(&base[0]) * 0.8);
}

fn function_quad(tides: bool) -> InstDef {
    let (core, node, selector) = if tides {
        (UGenSpec::TidalFunction, Node::TidalFunction, 12)
    } else {
        (UGenSpec::FrameLfo, Node::FrameLfo, 6)
    };
    let mut params = Vec::new();
    for lane in 0..4 {
        params.push((
            lane,
            catalog::port_ctl(&node, selector).unwrap(),
            Ctl::Const(f32::from(lane)),
        ));
        params.push((
            lane,
            catalog::port_ctl(&node, 0).unwrap(),
            Ctl::Const(500.0),
        ));
        if tides {
            params.push((lane, catalog::port_ctl(&node, 9).unwrap(), Ctl::Const(1.0)));
        } else {
            for (port, value) in [(2, 0.75), (3, 0.8), (4, 0.7), (5, 0.2)] {
                params.push((
                    lane,
                    catalog::port_ctl(&node, port).unwrap(),
                    Ctl::Const(value),
                ));
            }
        }
    }
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            core.clone(),
            core.clone(),
            core.clone(),
            core,
            UGenSpec::AuxOut,
            UGenSpec::Out3,
            UGenSpec::Out4,
        ]
        .into_boxed_slice(),
        edges: vec![
            Edge {
                from: 1,
                to: 4,
                port: 0,

                output: 0,
            },
            Edge {
                from: 2,
                to: 5,
                port: 0,

                output: 0,
            },
            Edge {
                from: 3,
                to: 6,
                port: 0,

                output: 0,
            },
        ]
        .into_boxed_slice(),
        node_params: params.into_boxed_slice(),
    }
}

#[test]
fn quad_frame_lfo_and_tides1_survive_codec_rate_blocks_and_stereo_rejection() {
    for tides in [false, true] {
        let def = function_quad(tides);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        assert_eq!(
            raw.nodes[0],
            if tides {
                Node::TidalFunction
            } else {
                Node::FrameLfo
            }
        );
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                cfg.output_channels = 4;
                let mut native = NativeRig::native_with(cfg);
                native.install(&def);
                let _ = native.step();
                native.send(event(
                    1,
                    native.engine.now(),
                    &[(ctl::FREQ, 500.0), (ctl::LEGATO, 1.0)],
                ));
                check_lanes(&native.run_four(8));

                let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                cfg.output_channels = 4;
                let mut browser = BrowserRig::browser_with(cfg);
                let mut record = Vec::new();
                encode_graph_record(1, 1, &bytes, &mut record);
                browser.push(&record);
                let _ = browser.step();
                browser.send(event(
                    1,
                    browser.engine.now(),
                    &[(ctl::FREQ, 500.0), (ctl::LEGATO, 1.0)],
                ));
                check_lanes(&browser.run_four(8));
            }
        }
        let mut native = NativeRig::native();
        native.install(&def);
        let _ = native.step();
        assert_eq!(
            native.engine.pop_fault().map(|f| f.code),
            Some(FaultCode::OutputChannels)
        );
        let mut browser = BrowserRig::browser(4 << 20);
        let mut record = Vec::new();
        encode_graph_record(1, 1, &bytes, &mut record);
        browser.push(&record);
        let _ = browser.step();
        assert_eq!(
            browser.engine.pop_fault().map(|f| f.code),
            Some(FaultCode::OutputChannels)
        );
    }
}

#[test]
fn tides1_quad_flags_are_sample_aligned_with_the_two_wave_outputs() {
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
    let mut outputs = [
        vec![0.0; 4096],
        vec![0.0; 4096],
        vec![0.0; 4096],
        vec![0.0; 4096],
    ];
    for (lane, output) in outputs.iter_mut().enumerate() {
        let mut inputs = [Inp::Val(0.0); MAX_PORTS];
        for (port, spec) in catalog::ports(&Node::TidalFunction).iter().enumerate() {
            inputs[port] = Inp::Val(spec.default);
        }
        inputs[0] = Inp::Val(500.0);
        inputs[9] = Inp::Val(1.0);
        inputs[12] = Inp::Val(lane as f32);
        let mut state = NodeState::default();
        let mut mem = [0.0; tidal_function::STATE_FLOATS];
        tidal_function::render(&inputs, &mut state, &mut mem, output, &kx);
    }
    let [uni, bi, attack, release] = &outputs;
    for (u, b) in uni.iter().zip(bi) {
        assert!((2.0 * u - 1.0 - b).abs() < 1.0e-5);
    }
    let attack_indices: Vec<_> = attack
        .iter()
        .enumerate()
        .filter_map(|(i, &x)| (x > 0.5).then_some(i))
        .collect();
    let release_indices: Vec<_> = release
        .iter()
        .enumerate()
        .filter_map(|(i, &x)| (x > 0.5).then_some(i))
        .collect();
    assert!(attack_indices.len() > 100 && release_indices.len() > 5);
    assert_eq!(attack[0], 0.0);
    assert_eq!(release[0], 0.0);
    assert!(attack.windows(2).any(|pair| pair == [1.0, 1.0]));
    assert_ne!(
        attack, release,
        "EOA level and EOR hold serve different roles"
    );
}
