//! Stereo granular bus adaptation, parameter response and install contracts.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_bus, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::texture;
use crate::dsp::effects::{FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::host::wire::Ctl;

fn effect(values: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let params: Vec<_> = values
        .iter()
        .map(|(name, value)| (*name, Ctl::Const(*value)))
        .collect();
    bus_def(3, vec![spec(EffectKind::TextureGrain, &params).unwrap()])
}

fn diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

pub(super) fn direct(overrides: &[(&str, f32)]) -> (Vec<f32>, Vec<f32>, FxStats) {
    let mut p: Vec<_> = texture::PARAMS.iter().map(|d| d.default).collect();
    p[8] = 1.0;
    for &(name, value) in overrides {
        let i = texture::PARAMS.iter().position(|d| d.name == name).unwrap();
        p[i] = value;
    }
    let sr = 48_000.0;
    let mut state = FxState::default();
    let mut mem = vec![0.0; texture::mem_len(sr)];
    texture::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut left = Vec::new();
    let mut right = Vec::new();
    for block in 0..180 {
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
        texture::process(&p, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
        if block >= 110 {
            left.extend(l);
            right.extend(r);
        }
    }
    (left, right, stats)
}

#[test]
fn every_continuous_control_changes_granular_audio() {
    let (base_l, base_r, stats) = direct(&[]);
    assert!(stats.grains_spawned > 0);
    assert!(rms(&base_l) > 1.0e-4 && rms(&base_r) > 1.0e-4);
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
        let (l, r, _) = direct(&[(name, value)]);
        assert!(l.iter().chain(&r).all(|x| x.is_finite()), "{name}");
        assert!(diff(&base_l, &l) + diff(&base_r, &r) > 1.0e-5, "{name}");
    }
    let (spread_l, spread_r, _) = direct(&[("stereo-spread", 1.0)]);
    assert!(diff(&spread_l, &spread_r) > 1.0e-3);
}

#[test]
fn trigger_edge_spawns_with_zero_density_and_gate_blocks_it() {
    let sr = 48_000.0;
    let mut p: Vec<_> = texture::PARAMS.iter().map(|d| d.default).collect();
    p[3] = 0.0;
    p[11] = 0.0;
    let mut state = FxState::default();
    let mut mem = vec![0.0; texture::mem_len(sr)];
    texture::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut process = |p: &[f32], state: &mut FxState, mem: &mut [f32], stats: &mut FxStats| {
        let mut l = [0.25; 256];
        let mut r = [-0.2; 256];
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats,
        };
        texture::process(p, state, mem, &mut l, &mut r, &mut ctx);
        (l, r)
    };
    for _ in 0..100 {
        process(&p, &mut state, &mut mem, &mut stats);
    }
    p[10] = 1.0;
    process(&p, &mut state, &mut mem, &mut stats);
    assert_eq!(stats.grains_spawned, 0, "closed gate prevents trigger");
    p[10] = 0.0;
    p[11] = 1.0;
    process(&p, &mut state, &mut mem, &mut stats);
    p[10] = 1.0;
    let (l, r) = process(&p, &mut state, &mut mem, &mut stats);
    assert_eq!(stats.grains_spawned, 1);
    assert!(rms(&l) > 1.0e-5 && rms(&r) > 1.0e-5);
}

#[test]
fn freeze_replays_captured_stereo_without_writing_new_input() {
    let sr = 48_000.0;
    let mut p: Vec<_> = texture::PARAMS.iter().map(|d| d.default).collect();
    p[3] = 0.9;
    p[11] = 0.0;
    let mut state = FxState::default();
    let mut mem = vec![0.0; texture::mem_len(sr)];
    texture::init(&mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    for _ in 0..100 {
        let mut l = [0.25; 256];
        let mut r = [-0.3; 256];
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        texture::process(&p, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
    }
    let capture_len = mem.len() - 8 * 7;
    let captured = mem[..capture_len].to_vec();
    p[9] = 1.0;
    p[11] = 1.0;
    let mut energy = 0.0;
    for _ in 0..15 {
        let mut l = [0.0; 256];
        let mut r = [0.0; 256];
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        texture::process(&p, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
        energy += rms(&l) + rms(&r);
    }
    assert_eq!(mem[..capture_len], captured);
    assert!(energy > 0.01, "frozen capture remains playable");
}

#[test]
fn insufficient_capture_rejects_install_without_retiring_live_bus() {
    let cells = Mirror::new(64);
    let mut buses = BusGraph::new(3, 64, 2000, &cells, 48_000.0, &caps());
    let mut old = BusTemplate::new();
    old.bus = BusId::new(3);
    old.push(EffectKind::Gain);
    assert!(buses.install(&old, false, 10, 1, &cells, 48_000.0, &caps()));
    let old_slot = buses.find(BusId::new(3)).unwrap();
    let texture = BusTemplate::from_def(&effect(&[("quality", 3.0)])).unwrap();
    assert!(!buses.install(&texture, false, 11, 1, &cells, 48_000.0, &caps()));
    assert_eq!(buses.find(BusId::new(3)), Some(old_slot));
    assert_eq!(buses.slots[old_slot].resource, 10);
    let mut limited = caps();
    limited.max_capture_seconds = 0.1;
    let mut enough_mem = BusGraph::new(
        2,
        64,
        texture::mem_len(48_000.0) + 10_000,
        &cells,
        48_000.0,
        &limited,
    );
    assert!(!enough_mem.install(&texture, false, 12, 1, &cells, 48_000.0, &limited));
}

#[test]
fn native_and_browser_stereo_bus_render_across_rates_and_blocks() {
    let source_l = chain(1, vec![UGenSpec::SinOsc]);
    let source_r = chain(2, vec![UGenSpec::Saw]);
    let bus = effect(&[("mix", 1.0), ("density", 0.9)]);
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut native = NativeRig::native_with(cfg);
            native.install(&source_l);
            native.install(&source_r);
            let id = native.install_bus(&bus, false);
            let _ = native.step();
            assert!(native.acks().iter().any(|a| matches!(a, crate::host::wire::HostMsg::Installed { resource, .. } if *resource == id)));
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
            for (resource, source) in [(1, &source_l), (2, &source_r)] {
                let mut bytes = Vec::new();
                encode_inst(source, &mut bytes).unwrap();
                let mut record = Vec::new();
                encode_graph_record(resource, 1, &bytes, &mut record);
                browser.push(&record);
            }
            let mut bytes = Vec::new();
            encode_bus(&bus, false, &mut bytes).unwrap();
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
fn editor_exposes_every_texture_control() {
    let decl = crate::types::manifest::HostManifest::spec_default()
        .editor_decl("texture-grain")
        .unwrap();
    for def in texture::PARAMS {
        assert!(
            decl.params.iter().any(|p| p.name == def.name),
            "{}",
            def.name
        );
    }
}
