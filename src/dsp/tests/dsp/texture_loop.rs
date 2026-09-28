//! Variable-delay/frozen-loop stereo effect contracts.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_bus, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::{texture, texture_loop, FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::host::wire::Ctl;

fn bus(values: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let params: Vec<_> = values
        .iter()
        .map(|(name, value)| (*name, Ctl::Const(*value)))
        .collect();
    bus_def(3, vec![spec(EffectKind::TextureLoop, &params).unwrap()])
}

fn diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

fn direct(overrides: &[(&str, f32)], freeze_after_capture: bool) -> (Vec<f32>, Vec<f32>) {
    let mut p: Vec<_> = texture::PARAMS.iter().map(|d| d.default).collect();
    for &(name, value) in overrides {
        let i = texture::PARAMS.iter().position(|d| d.name == name).unwrap();
        p[i] = value;
    }
    let sr = 48_000.0;
    let mut state = FxState::default();
    let mut mem = vec![0.0; texture_loop::mem_len(sr)];
    texture_loop::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut frozen_snapshot = None;
    for block in 0..170 {
        if freeze_after_capture && block == 95 {
            p[9] = 1.0;
            frozen_snapshot = Some(mem.clone());
        }
        let mut l = [0.0; 256];
        let mut r = [0.0; 256];
        for i in 0..256 {
            let t = (block * 256 + i) as f32 / sr;
            l[i] = (t * 317.0 * std::f32::consts::TAU).sin() * 0.45;
            r[i] = (t * 521.0 * std::f32::consts::TAU).sin() * 0.35;
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
        texture_loop::process(&p, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
        if block >= 110 {
            left.extend(l);
            right.extend(r);
        }
    }
    if let Some(snapshot) = frozen_snapshot {
        assert_eq!(mem, snapshot, "freeze preserves both capture channels");
    }
    (left, right)
}

#[test]
fn all_loop_controls_change_finite_stereo_audio() {
    let (base_l, base_r) = direct(&[], false);
    assert!(rms(&base_l) > 1.0e-5 && rms(&base_r) > 1.0e-5);
    for (name, value) in [
        ("position", 0.05),
        ("size", 0.9),
        ("pitch", 7.0),
        ("density", 0.9),
        ("texture", 0.95),
        ("stereo-spread", 1.0),
        ("feedback", 0.8),
        ("reverb", 1.0),
        ("freeze", 1.0),
        ("gate", 0.0),
    ] {
        let (l, r) = direct(&[(name, value)], false);
        assert!(l.iter().chain(&r).all(|x| x.is_finite()), "{name}");
        assert!(diff(&base_l, &l) + diff(&base_r, &r) > 1.0e-6, "{name}");
    }
    let (frozen_l, frozen_r) = direct(&[], true);
    assert!(rms(&frozen_l) > 1.0e-5 && rms(&frozen_r) > 1.0e-5);
    let (grain_l, _, _) = super::texture::direct(&[]);
    assert!(diff(&base_l, &grain_l[..base_l.len()]) > 1.0e-4);
}

#[test]
fn trigger_taps_sync_live_delay_and_reset_frozen_loop() {
    let sr = 48_000.0;
    let mut p: Vec<_> = texture::PARAMS.iter().map(|d| d.default).collect();
    let mut state = FxState::default();
    let mut mem = vec![0.0; texture_loop::mem_len(sr)];
    texture_loop::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut process = |p: &[f32], state: &mut FxState, mem: &mut [f32]| {
        let mut l = [0.25; 256];
        let mut r = [-0.2; 256];
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        texture_loop::process(p, state, mem, &mut l, &mut r, &mut ctx);
    };
    for _ in 0..25 {
        process(&p, &mut state, &mut mem);
    }
    p[10] = 1.0;
    process(&p, &mut state, &mut mem);
    assert!(state.s[6] >= 0.5, "trigger sets tap sync");
    let tap = state.s[5];
    assert!(tap > 128.0);
    p[10] = 0.0;
    p[9] = 1.0;
    process(&p, &mut state, &mut mem);
    assert!(state.s[8] > 0.0, "frozen loop phase advances");
    p[10] = 1.0;
    process(&p, &mut state, &mut mem);
    assert!(state.s[8] < tap, "trigger restarts the synchronized loop");
}

#[test]
fn live_delay_glides_and_frozen_geometry_uses_cubic_size_and_pitch_fade() {
    let sr = 48_000.0;
    let mut p: Vec<_> = texture::PARAMS.iter().map(|d| d.default).collect();
    let mut state = FxState::default();
    let mut mem = vec![0.0; texture_loop::mem_len(sr)];
    texture_loop::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut process = |p: &[f32], state: &mut FxState, mem: &mut [f32], frames: usize| {
        let mut l = vec![0.25; frames];
        let mut r = vec![-0.2; frames];
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        texture_loop::process(p, state, mem, &mut l, &mut r, &mut ctx);
    };
    p[0] = 0.2;
    for _ in 0..32 {
        process(&p, &mut state, &mut mem, 256);
    }
    let before = state.s[9];
    p[0] = 0.8;
    process(&p, &mut state, &mut mem, 1);
    assert!(state.s[9] > before);
    assert!(state.s[9] - before < 1.0, "delay must glide, not jump");

    p[0] = 0.25;
    p[1] = 0.5;
    p[2] = -12.0;
    p[9] = 1.0;
    process(&p, &mut state, &mut mem, 1);
    let fade = 64.0 * sr / 32_000.0;
    let max_delay = (state.s[1] - fade - 8.0).max(16.0);
    let expected_duration = (0.01 + 0.99 * p[1].powi(3)) * max_delay;
    let expected_point = p[0] * max_delay * 15.0 / 16.0 + fade;
    assert!((state.s[18] - expected_duration).abs() < 0.01);
    assert!((state.s[10] - expected_point).abs() < 0.01);
    assert!((state.s[21] - fade * 0.5).abs() < 0.01);
}

#[test]
fn loop_playback_is_identical_across_capture_and_freeze_partitions() {
    const LEN: usize = 8192;
    let render = |block: usize, late: usize| {
        let sr = 48_000.0;
        let mut p: Vec<_> = texture::PARAMS.iter().map(|d| d.default).collect();
        let mut state = FxState::default();
        let mut mem = vec![0.0; texture_loop::mem_len(sr)];
        texture_loop::init(&mut state, &mut mem);
        let store = SampleStore::new(StoreKind::NativeArc);
        let fft = Fft::new(1024);
        let cap = caps();
        let mut scratch = vec![0.0; 4096];
        let mut analysis = vec![0.0; 1024];
        let mut stats = FxStats::default();
        let mut l: Vec<_> = (0..LEN).map(|i| (i as f32 * 0.03).sin() * 0.3).collect();
        let mut r: Vec<_> = (0..LEN).map(|i| (i as f32 * 0.07).sin() * 0.2).collect();
        let mut offset = 0;
        let mut chunk = block - late;
        while offset < LEN {
            p[10] = if offset == 4096 { 1.0 } else { 0.0 };
            p[9] = if offset >= 5120 { 1.0 } else { 0.0 };
            p[2] = if offset >= 7168 { -7.0 } else { 0.0 };
            let next_control = [4096, 4097, 5120, 7168, LEN]
                .into_iter()
                .find(|&at| at > offset)
                .unwrap();
            let end = (offset + chunk).min(next_control);
            let mut ctx = FxCtx {
                sr,
                store: &store,
                fft: &fft,
                caps: &cap,
                scratch: &mut scratch,
                analysis: &mut analysis,
                stats: &mut stats,
            };
            texture_loop::process(
                &p,
                &mut state,
                &mut mem,
                &mut l[offset..end],
                &mut r[offset..end],
                &mut ctx,
            );
            chunk -= end - offset;
            if chunk == 0 {
                chunk = block;
            }
            offset = end;
        }
        (l, r)
    };
    let reference = render(64, 0);
    assert!(rms(&reference.0) > 1.0e-5);
    for (block, late) in [(256, 0), (64, 17), (256, 17), (37, 11)] {
        assert_eq!(reference, render(block, late), "{block}/{late}");
    }
}

#[test]
fn insufficient_loop_memory_keeps_live_bus() {
    let cells = Mirror::new(64);
    let mut buses = BusGraph::new(3, 64, 2000, &cells, 48_000.0, &caps());
    let mut old = BusTemplate::new();
    old.bus = BusId::new(3);
    old.push(EffectKind::Gain);
    assert!(buses.install(&old, false, 10, 1, &cells, 48_000.0, &caps()));
    let prior = buses.find(BusId::new(3));
    let loop_bus = BusTemplate::from_def(&bus(&[("quality", 3.0)])).unwrap();
    assert!(!buses.install(&loop_bus, false, 11, 1, &cells, 48_000.0, &caps()));
    assert_eq!(buses.find(BusId::new(3)), prior);
}

#[test]
fn native_and_browser_loop_survives_rate_and_block_matrix() {
    let sources = [
        chain(1, vec![UGenSpec::SinOsc]),
        chain(2, vec![UGenSpec::Saw]),
    ];
    let loop_bus = bus(&[("mix", 1.0), ("position", 0.3)]);
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut native = NativeRig::native_with(cfg);
            for source in &sources {
                native.install(source);
            }
            native.install_bus(&loop_bus, false);
            let _ = native.step();
            let t = native.engine.now();
            for (inst, pan) in [(1, 0.0), (2, 1.0)] {
                native.send(event(
                    inst,
                    t,
                    &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 1.0)],
                ));
            }
            let blocks = ((sr * 0.5) as usize / block) + 1;
            let (l, r) = native.run(blocks);
            assert!(l.iter().chain(&r).all(|x| x.is_finite()));
            assert!(rms(&l) > 1.0e-5 && rms(&r) > 1.0e-5, "native {sr}/{block}");
            assert!(diff(&l, &r) > 1.0e-4);

            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            for (resource, source) in [(1, &sources[0]), (2, &sources[1])] {
                let mut bytes = Vec::new();
                encode_inst(source, &mut bytes).unwrap();
                let mut record = Vec::new();
                encode_graph_record(resource, 1, &bytes, &mut record);
                browser.push(&record);
            }
            let mut bytes = Vec::new();
            encode_bus(&loop_bus, false, &mut bytes).unwrap();
            let mut record = Vec::new();
            encode_graph_record(3, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            let t = browser.engine.now();
            for (inst, pan) in [(1, 0.0), (2, 1.0)] {
                browser.send(event(
                    inst,
                    t,
                    &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 1.0)],
                ));
            }
            let (l, r) = browser.run(blocks);
            assert!(l.iter().chain(&r).all(|x| x.is_finite()));
            assert!(rms(&l) > 1.0e-5 && rms(&r) > 1.0e-5, "browser {sr}/{block}");
            assert!(diff(&l, &r) > 1.0e-4);
        }
    }
}

#[test]
fn editor_exposes_loop_controls() {
    let decl = crate::types::manifest::HostManifest::spec_default()
        .editor_decl("texture-loop")
        .unwrap();
    for def in texture::PARAMS {
        assert!(
            decl.params.iter().any(|p| p.name == def.name),
            "{}",
            def.name
        );
    }
}
