//! Tides2 four selectable lanes, 24 modes and native/browser host contract.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{functions, CoverageState, ResourceState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, tidal_poly, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(mode: f32, output_mode: f32, range: f32, main: f32, aux: f32) -> InstDef {
    let node = Node::TidalPoly;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::TidalPoly, UGenSpec::TidalPoly, UGenSpec::AuxOut].into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![
            (0, catalog::port_ctl(&node, 7).unwrap(), Ctl::Const(mode)),
            (1, catalog::port_ctl(&node, 7).unwrap(), Ctl::Const(mode)),
            (
                0,
                catalog::port_ctl(&node, 8).unwrap(),
                Ctl::Const(output_mode),
            ),
            (
                1,
                catalog::port_ctl(&node, 8).unwrap(),
                Ctl::Const(output_mode),
            ),
            (0, catalog::port_ctl(&node, 9).unwrap(), Ctl::Const(range)),
            (1, catalog::port_ctl(&node, 9).unwrap(), Ctl::Const(range)),
            (0, catalog::port_ctl(&node, 10).unwrap(), Ctl::Const(main)),
            (1, catalog::port_ctl(&node, 10).unwrap(), Ctl::Const(aux)),
            (0, catalog::port_ctl(&node, 4).unwrap(), Ctl::Const(0.8)),
            (1, catalog::port_ctl(&node, 4).unwrap(), Ctl::Const(0.8)),
        ]
        .into_boxed_slice(),
    }
}

fn direct(mode: f32, output_mode: f32, channel: f32, shift: f32) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 1024,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(220.0);
    ins[1] = Inp::Val(0.45);
    ins[2] = Inp::Val(0.7);
    ins[3] = Inp::Val(0.8);
    ins[4] = Inp::Val(shift);
    ins[5] = Inp::Val(1.0);
    ins[7] = Inp::Val(mode);
    ins[8] = Inp::Val(output_mode);
    ins[9] = Inp::Val(1.0);
    ins[10] = Inp::Val(channel);
    let mut st = NodeState::default();
    let mut mem = [0.0; tidal_poly::STATE_FLOATS];
    let mut out = vec![0.0; 1024];
    tidal_poly::render(&ins, &mut st, &mut mem, &mut out, &kx);
    out
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn all_twenty_four_roles_and_output_budget_are_truthful() {
    let (_, second) = functions();
    assert_eq!(second.len(), 24);
    for (index, row) in second.iter().enumerate() {
        assert_eq!(usize::from(row.mode), index / 8);
        assert_eq!(usize::from(row.output_mode), (index / 2) % 4);
        assert_eq!(usize::from(row.range), index % 2);
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
        assert_eq!(row.vactrol_template, Some("tides2-voice"));
        assert_eq!((row.source_channels, row.simultaneous_channels), (4, 2));
        assert_eq!(row.opt_in_quad_template, Some("tides2-quad-voice"));
        assert_eq!(row.simultaneous_output_coverage, CoverageState::Adaptation);
    }
}

#[test]
fn every_output_mode_has_four_distinct_selectable_lanes() {
    for mode in 0..4 {
        let signals: Vec<_> = (0..4)
            .map(|channel| direct(1.0, mode as f32, channel as f32, 0.8))
            .collect();
        for channel in 0..4 {
            assert!(signals[channel].iter().all(|x| x.is_finite()));
            for other in channel + 1..4 {
                assert!(
                    delta(&signals[channel], &signals[other]) > 1.0e-4,
                    "output mode {mode}, lanes {channel}/{other}"
                );
            }
        }
    }
    for mode in 0..3 {
        let signal = direct(mode as f32, 2.0, 2.0, 0.8);
        assert!(rms(&signal) > 1.0e-5);
    }
}

#[test]
fn ar_gate_release_and_clock_edge_reset_have_distinct_state_transitions() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 256,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(100.0);
    ins[1] = Inp::Val(0.5);
    ins[2] = Inp::Val(0.5);
    ins[3] = Inp::Val(0.8);
    ins[4] = Inp::Val(0.5);
    ins[5] = Inp::Val(1.0);
    ins[7] = Inp::Val(2.0);
    ins[9] = Inp::Val(1.0);
    let mut state = NodeState::default();
    let mut mem = [0.0; tidal_poly::STATE_FLOATS];
    let mut out = [0.0; 256];
    tidal_poly::render(&ins, &mut state, &mut mem, &mut out, &kx);
    let held = mem[0];
    assert!(held > 0.49 && held < 0.51);
    ins[5] = Inp::Val(0.0);
    tidal_poly::render(&ins, &mut state, &mut mem, &mut out, &kx);
    assert!(mem[0] > held, "gate fall advances release");
    ins[7] = Inp::Val(1.0);
    ins[6] = Inp::Val(1.0);
    let mut one = [0.0; 1];
    tidal_poly::render(&ins, &mut state, &mut mem, &mut one, &kx);
    assert!(mem[0] < 0.01, "clock rising edge resets phase");
    tidal_poly::render(&ins, &mut state, &mut mem, &mut one, &kx);
    assert!(mem[0] > 0.0, "held clock does not repeatedly reset");
}

#[test]
fn selector_codec_and_two_node_budget_are_explicit() {
    assert_eq!(catalog::ports(&Node::TidalPoly).len(), 11);
    let last = catalog::port_ctl(&Node::TidalPoly, 10).unwrap();
    assert_eq!(catalog::port_of(&Node::TidalPoly, last), Some(10));
    let def = voice(1.0, 3.0, 1.0, 0.0, 3.0);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    let budget = 2 * tidal_poly::STATE_FLOATS;
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
    assert!(raw.n_node_params > 0);
}

#[test]
fn native_browser_all_combinations_render_finite_distinct_channels() {
    for mode in 0..3 {
        for output_mode in 0..4 {
            for range in 0..2 {
                let def = voice(mode as f32, output_mode as f32, range as f32, 0.0, 3.0);
                let mut bytes = Vec::new();
                encode_inst(&def, &mut bytes).unwrap();
                for sr in [44_100.0, 48_000.0, 96_000.0] {
                    for block in [64, 256] {
                        let mut cfg = config(&caps(), StoreKind::NativeArc);
                        cfg.sample_rate = sr;
                        cfg.max_block = block;
                        let mut native = NativeRig::native_with(cfg);
                        native.install(&def);
                        let _ = native.step();
                        native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
                        let (main, aux) = native.run(12);
                        assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                        assert!(delta(&main, &aux) > 1.0e-6);

                        let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                        cfg.sample_rate = sr;
                        cfg.max_block = block;
                        let mut browser = BrowserRig::browser_with(cfg);
                        let mut record = Vec::new();
                        encode_graph_record(1, 1, &bytes, &mut record);
                        browser.push(&record);
                        let _ = browser.run(6);
                        browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
                        let (main, aux) = browser.run(12);
                        assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                        assert!(delta(&main, &aux) > 1.0e-6);
                    }
                }
            }
        }
    }
}
