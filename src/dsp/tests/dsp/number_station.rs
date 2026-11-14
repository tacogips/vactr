//! Original Number Station voice/tone adaptation and host behavior.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{peaks_functions, CoverageState, ResourceState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, number_station, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(mode: f32) -> InstDef {
    let node = Node::NumberStation;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::NumberStation,
            UGenSpec::NumberStation,
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
            (0, catalog::port_ctl(&node, 1).unwrap(), Ctl::Const(mode)),
            (1, catalog::port_ctl(&node, 1).unwrap(), Ctl::Const(mode)),
            (0, catalog::port_ctl(&node, 11).unwrap(), Ctl::Const(0.0)),
            (1, catalog::port_ctl(&node, 11).unwrap(), Ctl::Const(1.0)),
        ]
        .into_boxed_slice(),
    }
}

fn signal(mode: f32, changes: &[(usize, f32)], seed: u32) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 18_000,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::NumberStation).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[1] = Inp::Val(mode);
    for &(index, value) in changes {
        ins[index] = Inp::Val(value);
    }
    let mut st = NodeState::default();
    let mut mem = [0.0; number_station::STATE_FLOATS];
    let mut out = vec![0.0; 18_000];
    number_station::render(&ins, &mut st, &mut mem, &mut out, &kx);
    out
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn asset_flag_and_both_roles_are_truthful() {
    let row = &peaks_functions()[11];
    assert!(row.upstream_digit_asset);
    assert_eq!(row.coverage, CoverageState::Adaptation);
    assert_eq!(row.resources, ResourceState::Replacement);
    assert_eq!(row.vactr_template, Some("number-station-voice"));
    let tone = signal(0.0, &[], 34);
    let speech = signal(1.0, &[], 34);
    assert!(rms(&tone) > 1.0e-4 && rms(&speech) > 1.0e-5);
    assert!(delta(&tone, &speech) > 1.0e-4);
}

#[test]
fn all_ten_original_digit_recipes_are_distinct_and_selected() {
    let signals: Vec<_> = (0..10).map(|d| signal(1.0, &[(7, d as f32)], 34)).collect();
    for d in 0..10 {
        assert!(signals[d].iter().all(|x| x.is_finite()));
        assert!(rms(&signals[d]) > 1.0e-5, "digit {d}");
        for other in d + 1..10 {
            assert!(
                delta(&signals[d], &signals[other]) > 1.0e-4,
                "digits {d}/{other}"
            );
        }
    }
}

#[test]
fn tone_mode_four_registers_are_distinct_and_trigger_restarts_syllable() {
    let tones: Vec<_> = (0..4).map(|d| signal(0.0, &[(7, d as f32)], 34)).collect();
    for d in 0..4 {
        for other in d + 1..4 {
            assert!(delta(&tones[d], &tones[other]) > 1.0e-4);
        }
    }
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 128,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 34,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (i, port) in catalog::ports(&Node::NumberStation).iter().enumerate() {
        ins[i] = Inp::Val(port.default);
    }
    ins[1] = Inp::Val(1.0);
    let mut st = NodeState::default();
    let mut mem = [0.0; number_station::STATE_FLOATS];
    let mut out = [0.0; 128];
    number_station::render(&ins, &mut st, &mut mem, &mut out, &kx);
    let elapsed = mem[3];
    ins[10] = Inp::Val(1.0);
    number_station::render(&ins, &mut st, &mut mem, &mut out[..1], &kx);
    assert!(mem[3] < elapsed, "trigger rise restarts while gate is held");
}

#[test]
fn full_half_controls_and_seeded_transition_are_meaningful() {
    let base = signal(1.0, &[(7, 4.0)], 34);
    for &(index, value) in &[(0, 330.0), (3, 0.9), (5, 0.0), (6, 0.9), (7, 8.0)] {
        assert!(
            delta(&base, &signal(1.0, &[(7, 4.0), (index, value)], 34)) > 1.0e-5,
            "port {index}"
        );
    }
    let half_a = signal(1.0, &[(2, 1.0), (5, 0.0), (6, 0.0)], 34);
    let half_b = signal(1.0, &[(2, 1.0), (5, 1.0), (6, 1.0)], 34);
    assert!(
        delta(&half_a, &half_b) == 0.0,
        "half mode fixes noise and drive"
    );
    let auto = &[(8, 1.0), (4, 1.0), (7, 0.0)];
    let picked = signal(1.0, auto, 34);
    assert!(delta(&picked, &signal(1.0, auto, 34)) == 0.0);
    assert!(delta(&picked, &signal(1.0, &[(8, 1.0), (4, 0.0), (7, 0.0)], 34)) > 1.0e-5);
    assert!(signal(1.0, &[(9, 0.0), (10, 0.0)], 34)
        .iter()
        .all(|x| *x == 0.0));
}

#[test]
fn native_browser_codec_budget_and_rates() {
    assert_eq!(catalog::ports(&Node::NumberStation).len(), 12);
    let last = catalog::port_ctl(&Node::NumberStation, 11).unwrap();
    assert_eq!(catalog::port_of(&Node::NumberStation, last), Some(11));
    for mode in 0..2 {
        let def = voice(mode as f32);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        let budget = 2 * number_station::STATE_FLOATS;
        let env = BuildEnv {
            sr: 96_000.0,
            caps: caps(),
            voice_mem: budget,
        };
        let built = Template::from_inst(&def, &env).unwrap();
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
