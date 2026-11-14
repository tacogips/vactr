//! Tides1 analytic function roles, flags, graph codec and host tiers.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{functions, CoverageState, ResourceState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, tidal_function, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(mode: f32, range: f32) -> InstDef {
    let node = Node::TidalFunction;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::TidalFunction,
            UGenSpec::TidalFunction,
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
            (0, catalog::port_ctl(&node, 9).unwrap(), Ctl::Const(mode)),
            (1, catalog::port_ctl(&node, 9).unwrap(), Ctl::Const(mode)),
            (0, catalog::port_ctl(&node, 10).unwrap(), Ctl::Const(range)),
            (1, catalog::port_ctl(&node, 10).unwrap(), Ctl::Const(range)),
            (1, catalog::port_ctl(&node, 12).unwrap(), Ctl::Const(1.0)),
        ]
        .into_boxed_slice(),
    }
}

#[test]
fn two_generations_have_truthful_mode_inventory() {
    let (first, second) = functions();
    assert_eq!(first.len(), 9);
    assert_eq!(second.len(), 24);
    for (index, row) in first.iter().enumerate() {
        assert_eq!(usize::from(row.mode), index / 3);
        assert_eq!(usize::from(row.range), index % 3);
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
        assert_eq!(row.vactr_template, Some("tidal-voice"));
        assert_eq!(row.opt_in_quad_template, Some("tidal-quad-voice"));
        assert_eq!(row.simultaneous_output_coverage, CoverageState::Adaptation);
    }
    assert!(second
        .iter()
        .all(|row| row.coverage == CoverageState::Adaptation
            && row.source_channels == 4
            && row.simultaneous_channels == 2));
}

#[test]
fn last_port_codec_and_fixed_memory_budget() {
    assert_eq!(catalog::ports(&Node::TidalFunction).len(), 13);
    let last = catalog::port_ctl(&Node::TidalFunction, 12).unwrap();
    assert_eq!(catalog::port_of(&Node::TidalFunction, last), Some(12));
    let def = voice(1.0, 0.0);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    let budget = 2 * tidal_function::STATE_FLOATS;
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
}

#[test]
fn gate_clock_freeze_ratio_and_flags_have_distinct_direct_behavior() {
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
    ins[0] = Inp::Val(240.0);
    ins[1] = Inp::Val(0.5);
    ins[2] = Inp::Val(0.5);
    ins[3] = Inp::Val(0.2);
    ins[4] = Inp::Val(1.0);
    ins[6] = Inp::Val(1.0);
    ins[9] = Inp::Val(1.0);
    let mut st = NodeState::default();
    let mut mem = [0.0; tidal_function::STATE_FLOATS];
    let mut a = [0.0; 256];
    tidal_function::render(&ins, &mut st, &mut mem, &mut a, &kx);
    ins[8] = Inp::Val(1.0);
    let mut frozen = [0.0; 256];
    tidal_function::render(&ins, &mut st, &mut mem, &mut frozen, &kx);
    assert!(frozen.iter().all(|sample| *sample == frozen[0]));
    ins[8] = Inp::Val(0.0);
    ins[4] = Inp::Val(2.0);
    let mut faster = [0.0; 256];
    tidal_function::render(&ins, &mut st, &mut mem, &mut faster, &kx);
    assert_ne!(frozen, faster);
    ins[5] = Inp::Val(1.0);
    ins[7] = Inp::Val(1.0);
    tidal_function::render(&ins, &mut st, &mut mem, &mut a, &kx);
    assert!(mem[0] < 0.02, "clock sync restarts phase");

    for (mode, output_kind) in [(0.0, 2.0), (1.0, 3.0), (2.0, 2.0)] {
        let mut state = NodeState::default();
        let mut memory = [0.0; tidal_function::STATE_FLOATS];
        ins[5] = Inp::Val(0.0);
        ins[7] = Inp::Val(0.0);
        ins[9] = Inp::Val(mode);
        ins[12] = Inp::Val(output_kind);
        let mut pulses = 0;
        for _ in 0..100 {
            tidal_function::render(&ins, &mut state, &mut memory, &mut a, &kx);
            pulses += a.iter().filter(|value| **value > 0.5).count();
        }
        assert!(pulses > 0, "mode {mode} flag {output_kind}");
    }
    let mut state = NodeState::default();
    let mut memory = [0.0; tidal_function::STATE_FLOATS];
    ins[9] = Inp::Val(2.0);
    ins[6] = Inp::Val(1.0);
    ins[12] = Inp::Val(3.0);
    tidal_function::render(&ins, &mut state, &mut memory, &mut a, &kx);
    ins[6] = Inp::Val(0.0);
    tidal_function::render(&ins, &mut state, &mut memory, &mut a, &kx);
    assert!(
        a.iter().any(|pulse| *pulse > 0.5),
        "AR gate fall emits release flag"
    );
}

#[test]
fn eor_hold_threshold_and_rate_scaling_match_the_adapted_generator_roles() {
    let threshold = (44_739_242.0 / 4_294_967_296.0) * 48_000.0;
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let normal_long = tidal_function::eor_hold_samples(sr, 0, threshold - 1.0);
        let normal_short = tidal_function::eor_hold_samples(sr, 0, threshold + 1.0);
        let low_long = tidal_function::eor_hold_samples(sr, 2, threshold / 4.0 - 1.0);
        let low_short = tidal_function::eor_hold_samples(sr, 2, threshold / 4.0 + 1.0);
        assert_eq!(normal_long, (sr / 1_000.0_f32).round());
        assert_eq!(normal_short, (sr / 48_000.0_f32).round().max(1.0));
        assert_eq!(
            tidal_function::eor_hold_samples(sr, 0, threshold),
            normal_short
        );
        assert_eq!(low_long, (sr / 250.0_f32).round());
        assert_eq!(low_short, (sr / 12_000.0_f32).round().max(1.0));
        assert_eq!(
            tidal_function::eor_hold_samples(sr, 2, threshold / 4.0),
            low_short
        );
    }
}

#[test]
fn eoa_level_eor_hold_idle_retrigger_and_freeze_replay() {
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
    ins[0] = Inp::Val(600.0);
    ins[2] = Inp::Val(0.9);
    ins[4] = Inp::Val(1.0);
    ins[12] = Inp::Val(2.0);
    let mut state = NodeState::default();
    let mut mem = [0.0; tidal_function::STATE_FLOATS];
    let mut sample = [0.0];
    tidal_function::render(&ins, &mut state, &mut mem, &mut sample, &kx);
    assert_eq!(sample[0], 1.0, "idle EOA is a level");
    ins[12] = Inp::Val(3.0);
    tidal_function::render(&ins, &mut state, &mut mem, &mut sample, &kx);
    assert_eq!(sample[0], 1.0, "idle EOR remains high");
    ins[6] = Inp::Val(1.0);
    tidal_function::render(&ins, &mut state, &mut mem, &mut sample, &kx);
    assert_eq!(sample[0], 0.0, "gate rise clears idle EOR");
    ins[8] = Inp::Val(1.0);
    ins[6] = Inp::Val(0.0);
    let mut frozen = [0.0; 32];
    tidal_function::render(&ins, &mut state, &mut mem, &mut frozen, &kx);
    assert!(frozen.iter().all(|&value| value == 0.0));
    ins[8] = Inp::Val(0.0);
    ins[9] = Inp::Val(1.0);
    let mut looped = [0.0; 240];
    tidal_function::render(&ins, &mut state, &mut mem, &mut looped, &kx);
    assert!(looped.iter().any(|&value| value == 1.0));
    assert!(looped.windows(2).any(|pair| pair == [1.0, 0.0]));

    // High-range AR holds at half phase even when slope's EOA split is later.
    ins[9] = Inp::Val(2.0);
    ins[12] = Inp::Val(2.0);
    ins[6] = Inp::Val(1.0);
    let mut ar_state = NodeState::default();
    let mut ar_mem = [0.0; tidal_function::STATE_FLOATS];
    let mut ar = [0.0; 512];
    tidal_function::render(&ins, &mut ar_state, &mut ar_mem, &mut ar, &kx);
    assert!((ar_mem[0] - 0.5).abs() < 1.0e-4);
    assert_eq!(ar[511], 1.0, "AR sustain holds EOA high");
    ins[8] = Inp::Val(1.0);
    ins[6] = Inp::Val(0.0);
    let mut held = [0.0; 32];
    tidal_function::render(&ins, &mut ar_state, &mut ar_mem, &mut held, &kx);
    assert!(
        held.iter().all(|&value| value == 1.0),
        "freeze replays high EOA"
    );
}

#[test]
fn looping_eor_hold_is_applied_at_each_host_rate() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let store = SampleStore::new(StoreKind::NativeArc);
        let caps = caps();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr,
            gate: 2048,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        for hz in [240.0, 600.0] {
            let mut ins = [Inp::Val(0.0); MAX_PORTS];
            ins[0] = Inp::Val(hz);
            ins[4] = Inp::Val(1.0);
            ins[9] = Inp::Val(1.0);
            ins[12] = Inp::Val(3.0);
            let mut state = NodeState::default();
            let mut mem = [0.0; tidal_function::STATE_FLOATS];
            let mut output = [0.0; 2048];
            tidal_function::render(&ins, &mut state, &mut mem, &mut output, &kx);
            let first = output.iter().position(|&value| value > 0.5).unwrap();
            let hold = output[first..]
                .iter()
                .take_while(|&&value| value > 0.5)
                .count();
            assert_eq!(hold as f32, tidal_function::eor_hold_samples(sr, 0, hz));
        }
    }
}

#[test]
fn low_range_eor_hold_uses_divided_generator_rate_after_real_wrap() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let store = SampleStore::new(StoreKind::NativeArc);
        let caps = caps();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr,
            gate: 8192,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        for (base_hz, effective_hz) in [(1_000.0, 25.6), (10_000.0, 256.0)] {
            assert!(base_hz < sr * 0.4, "test frequency must avoid base clamp");
            let mut ins = [Inp::Val(0.0); MAX_PORTS];
            ins[0] = Inp::Val(base_hz);
            ins[4] = Inp::Val(4.0);
            ins[9] = Inp::Val(1.0);
            ins[10] = Inp::Val(2.0);
            ins[11] = Inp::Val(48.0);
            ins[12] = Inp::Val(3.0);
            let mut state = NodeState::default();
            let mut mem = [0.0; tidal_function::STATE_FLOATS];
            let mut output = [0.0; 8192];
            tidal_function::render(&ins, &mut state, &mut mem, &mut output, &kx);
            let first = output.iter().position(|&value| value > 0.5).unwrap();
            assert!(
                ((first + 1) as f32 - sr / effective_hz).abs() <= 2.0,
                "low-range wrap must follow effective pitch: {sr}/{effective_hz}"
            );
            let hold = output[first..]
                .iter()
                .take_while(|&&value| value > 0.5)
                .count();
            assert_eq!(
                hold as f32,
                tidal_function::eor_hold_samples(sr, 2, effective_hz),
                "low-range EOR at {sr} Hz and {effective_hz} Hz"
            );
            assert!(output[first + hold..].iter().any(|&value| value == 0.0));
        }
    }
}

#[test]
fn native_browser_modes_and_ranges_render_finite_distinct_outputs() {
    for mode in 0..=2 {
        for range in 0..=2 {
            let def = voice(mode as f32, range as f32);
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
                    let (main, aux) = native.run(16);
                    assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
                    assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7);
                    assert_ne!(main, aux);
                    assert!(main.iter().all(|sample| (0.0..=1.0).contains(sample)));
                    assert!(aux.iter().any(|sample| *sample < 0.0));

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
                    assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7);
                    assert_ne!(main, aux);
                }
            }
        }
    }
}
