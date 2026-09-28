//! Separate Elements blow/strike bus inputs and bounded two-output effect.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::elements_bank;
use crate::dsp::effects::{FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, Node, RawGraph};
use crate::host::wire::Ctl;

fn effect(values: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let named: Vec<_> = values
        .iter()
        .map(|(name, value)| (*name, Ctl::Const(*value)))
        .collect();
    bus_def(3, vec![spec(EffectKind::ElementsBank, &named).unwrap()])
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

fn direct(values: &[(&str, f32)], left_on: bool, right_on: bool) -> (Vec<f32>, Vec<f32>) {
    let sr = 48_000.0;
    let mut p: Vec<_> = elements_bank::PARAMS.iter().map(|d| d.default).collect();
    p[1] = 0.3;
    p[3] = 0.5;
    p[6] = 0.5;
    p[22] = 0.4;
    p[25] = 0.5;
    for &(name, value) in values {
        let index = elements_bank::PARAMS
            .iter()
            .position(|d| d.name == name)
            .unwrap();
        p[index] = value;
    }
    let mut state = FxState::default();
    let mut mem = vec![0.0; elements_bank::mem_len(sr)];
    elements_bank::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut left = Vec::new();
    let mut right = Vec::new();
    for block in 0..32 {
        let mut l = [0.0; 256];
        let mut r = [0.0; 256];
        for frame in 0..256 {
            let t = (block * 256 + frame) as f32 / sr;
            if left_on {
                l[frame] = (t * 319.0 * std::f32::consts::TAU).sin() * 0.4;
            }
            if right_on {
                r[frame] = (t * 701.0 * std::f32::consts::TAU).sin() * 0.4;
            }
        }
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        elements_bank::process(&p, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
        left.extend(l);
        right.extend(r);
    }
    (left, right)
}

#[test]
fn independent_blow_and_strike_inputs_and_three_models() {
    let silence = direct(
        &[("external-blend", 1.0), ("el-bow-level", 0.0)],
        false,
        false,
    );
    assert!(rms(&silence.0) + rms(&silence.1) < 1.0e-8);
    let blow = direct(
        &[("external-blend", 1.0), ("el-bow-level", 0.0)],
        true,
        false,
    );
    let strike = direct(
        &[("external-blend", 1.0), ("el-bow-level", 0.0)],
        false,
        true,
    );
    assert!(rms(&blow.0) > 1.0e-5 && rms(&strike.0) > 1.0e-5);
    assert!(difference(&blow.0, &strike.0) > 1.0e-4);
    assert!(difference(&blow.0, &blow.1) > 1.0e-5);
    for model in 0..=2 {
        let (main, aux) = direct(&[("el-model", model as f32)], true, true);
        assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
        assert!(rms(&main) > 1.0e-5 && rms(&aux) > 1.0e-5);
        assert!(difference(&main, &aux) > 1.0e-5);
    }
}

#[test]
fn alternate_has_distinct_spatial_outputs_and_independent_external_inputs() {
    let opts = [("el-alternate", 1.0), ("external-blend", 1.0)];
    let silent = direct(&opts, false, false);
    let blow = direct(&opts, true, false);
    let strike = direct(&opts, false, true);
    assert!(difference(&silent.0, &blow.0) > 1.0e-5);
    assert!(difference(&silent.0, &strike.0) > 1.0e-5);
    assert!(rms(&blow.0) > 1.0e-6 && rms(&strike.0) > 1.0e-6);
    assert!(difference(&blow.0, &strike.0) > 1.0e-5);
    assert!(difference(&blow.0, &blow.1) > 1.0e-5);
    let normal = direct(&[("el-alternate", 0.0)], true, true);
    let alternate = direct(&opts, true, true);
    assert!(difference(&normal.0, &alternate.0) > 1.0e-5);
    assert!(alternate
        .0
        .iter()
        .chain(&alternate.1)
        .all(|x| x.is_finite()));
}

#[test]
fn alternate_patch_and_performance_controls_reach_audio() {
    let base = direct(&[("el-alternate", 1.0)], true, true);
    for param in elements_bank::PARAMS.iter().take(24) {
        let value = match param.name {
            "el-note" => 12.0,
            "el-modulation" => 0.9,
            "el-gate" => 0.0,
            _ => 0.9,
        };
        let rendered = direct(&[("el-alternate", 1.0), (param.name, value)], true, true);
        let delta = difference(&base.0, &rendered.0) + difference(&base.1, &rendered.1);
        assert!(delta > 1.0e-8, "{}: {delta}", param.name);
    }
}

#[test]
fn normal_space_freezes_at_exact_boundary_and_writes_below_it() {
    let sr = 48_000.0;
    let mut p: Vec<_> = elements_bank::PARAMS
        .iter()
        .map(|def| def.default)
        .collect();
    p[18] = 1.75;
    p[25] = 0.0;
    let mut state = FxState::default();
    let mut mem = vec![0.0; elements_bank::mem_len(sr)];
    elements_bank::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut left = [0.0; 256];
    let mut right = [0.0; 256];
    let mut ctx = FxCtx {
        sr,
        store: &store,
        fft: &fft,
        caps: &cap,
        scratch: &mut scratch,
        analysis: &mut analysis,
        stats: &mut stats,
    };
    elements_bank::process(&p, &mut state, &mut mem, &mut left, &mut right, &mut ctx);
    let echo_start = 4 * elements_bank::line_len(sr) + 8;
    assert!(mem[echo_start..].iter().all(|value| *value == 0.0));
    p[18] = 1.749;
    elements_bank::process(&p, &mut state, &mut mem, &mut left, &mut right, &mut ctx);
    assert!(mem[echo_start..].iter().any(|value| value.abs() > 1.0e-8));
}

#[test]
fn single_string_tracks_twenty_hertz_period_at_ninety_six_kilohertz() {
    let sr = 96_000.0;
    let period = (sr / 20.0) as usize;
    assert!(elements_bank::line_len(sr) >= period + 2);
    let mut p: Vec<_> = elements_bank::PARAMS.iter().map(|d| d.default).collect();
    p[1] = 0.0;
    p[3] = 1.0;
    p[21] = -48.0;
    p[24] = 1.0;
    p[25] = 1.0;
    let mut state = FxState::default();
    let mut mem = vec![0.0; elements_bank::mem_len(sr)];
    elements_bank::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut rendered = Vec::new();
    for block in 0..42 {
        let mut left = [0.0; 256];
        let mut right = [0.0; 256];
        if block == 0 {
            left[0] = 1.0;
        }
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        elements_bank::process(&p, &mut state, &mut mem, &mut left, &mut right, &mut ctx);
        rendered.extend(left);
    }
    let first = rendered[period - 400..period + 400]
        .iter()
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max);
    let second = rendered[2 * period - 800..2 * period + 800]
        .iter()
        .map(|v| v.abs())
        .fold(0.0_f32, f32::max);
    assert!(
        first > 1.0e-5 && second > 1.0e-5,
        "20 Hz recurrence: {first}, {second}"
    );
}

#[test]
fn all_patch_and_performance_controls_change_wet_audio() {
    let base = direct(&[], true, true);
    for name in elements_bank::PARAMS.iter().take(26).map(|p| p.name) {
        let index = elements_bank::PARAMS
            .iter()
            .position(|p| p.name == name)
            .unwrap();
        let value = if index == 24 {
            2.0
        } else if index == 21 {
            12.0
        } else {
            0.9
        };
        let (l, r) = direct(&[(name, value)], true, true);
        let delta = difference(&base.0, &l) + difference(&base.1, &r);
        assert!(delta > 1.0e-8, "{name}: {delta}");
    }
}

#[test]
fn final_parameter_codec_editor_and_memory_preflight() {
    assert_eq!(elements_bank::PARAMS.len(), 28);
    assert_eq!(
        catalog::port_count(&Node::Effect {
            kind: EffectKind::ElementsBank,
            fx: 0
        }),
        29
    );
    let decl = crate::types::manifest::HostManifest::spec_default()
        .editor_decl("elements-bank")
        .unwrap();
    for param in elements_bank::PARAMS {
        assert!(
            decl.params.iter().any(|p| p.name == param.name),
            "{}",
            param.name
        );
    }
    let bus = effect(&[("mix", 0.125)]);
    let mut bytes = Vec::new();
    encode_bus(&bus, false, &mut bytes).unwrap();
    let mut decoded = BusTemplate::new();
    let mut raw = RawGraph::boxed();
    assert_eq!(
        decode_graph(&bytes, &mut raw, &mut decoded).unwrap(),
        GraphKind::Bus(BusId::new(3))
    );
    assert_eq!(decoded.params[0][0].1, Ctl::Const(0.125));
    assert_eq!(
        decoded.params[0][0].0.get(),
        crate::dsp::effects::EFFECT_PARAM_BASE + 26
    );
    let mut last_bytes = Vec::new();
    encode_bus(&effect(&[("el-alternate", 1.0)]), false, &mut last_bytes).unwrap();
    let mut last = BusTemplate::new();
    decode_graph(&last_bytes, &mut raw, &mut last).unwrap();
    assert_eq!(last.params[0][0].1, Ctl::Const(1.0));
    assert_eq!(
        last.params[0][0].0.get(),
        crate::dsp::effects::EFFECT_PARAM_BASE + 27
    );

    let mut no_capture = caps();
    no_capture.max_capture_seconds = 0.0;
    let cells = Mirror::new(64);
    let mut enough = BusGraph::new(3, 64, 30_000, &cells, 96_000.0, &no_capture);
    let template = BusTemplate::from_def(&effect(&[])).unwrap();
    assert!(enough.install(&template, false, 11, 1, &cells, 96_000.0, &no_capture));
    let mut limited = BusGraph::new(3, 64, 2000, &cells, 96_000.0, &no_capture);
    let mut old = BusTemplate::new();
    old.bus = BusId::new(3);
    old.push(EffectKind::Gain);
    assert!(limited.install(&old, false, 10, 1, &cells, 96_000.0, &no_capture));
    let slot = limited.find(BusId::new(3)).unwrap();
    assert!(!limited.install(&template, false, 12, 1, &cells, 96_000.0, &no_capture));
    assert_eq!(limited.find(BusId::new(3)), Some(slot));
    assert_eq!(limited.slots[slot].resource, 10);
}

#[test]
fn native_browser_render_at_supported_rates_and_blocks_without_allocation() {
    for alternate in [0.0, 1.0] {
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let mut no_capture = caps();
                no_capture.max_capture_seconds = 0.0;
                let mut cfg = config(&no_capture, StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut native = NativeRig::native_with(cfg);
                native.install(&chain(1, vec![UGenSpec::Saw]));
                native.install_bus(
                    &effect(&[("el-model", 0.0), ("el-alternate", alternate)]),
                    false,
                );
                native.install_bus(&bus_def(0, vec![]), true);
                let _ = native.step();
                native.send(event(
                    1,
                    native.engine.now(),
                    &[(ctl::BUS, 3.0), (ctl::LEGATO, 2.0)],
                ));
                let (main, aux) = native.run(32);
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(
                    rms(&main) > 1.0e-6 && rms(&aux) > 1.0e-6,
                    "native {sr}/{block}"
                );

                let mut cfg = config(&no_capture, StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut browser = BrowserRig::browser_with(cfg);
                let source = chain(1, vec![UGenSpec::Saw]);
                let mut payload = Vec::new();
                let mut record = Vec::new();
                encode_inst(&source, &mut payload).unwrap();
                encode_graph_record(10, 1, &payload, &mut record);
                browser.push(&record);
                for (resource, bus, master) in [
                    (11, effect(&[("el-alternate", alternate)]), false),
                    (12, bus_def(0, vec![]), true),
                ] {
                    payload.clear();
                    record.clear();
                    encode_bus(&bus, master, &mut payload).unwrap();
                    encode_graph_record(resource, 1, &payload, &mut record);
                    browser.push(&record);
                }
                let _ = browser.run(6);
                browser.send(event(
                    1,
                    browser.engine.now(),
                    &[(ctl::BUS, 3.0), (ctl::LEGATO, 2.0)],
                ));
                let (main, aux) = browser.run(32);
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(
                    rms(&main) > 1.0e-6 && rms(&aux) > 1.0e-6,
                    "browser {sr}/{block}"
                );
            }
        }
    }
}
