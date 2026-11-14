//! Streams follower/compressor digital adaptations and routing contracts.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::stream_dynamics;
use crate::dsp::effects::{FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

fn params(kind: EffectKind) -> &'static [crate::dsp::effects::ParamDef] {
    if kind == EffectKind::StreamFollower {
        stream_dynamics::FOLLOWER_PARAMS
    } else {
        stream_dynamics::COMPRESSOR_PARAMS
    }
}

fn effect(kind: EffectKind, named: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let values: Vec<_> = named.iter().map(|(n, v)| (*n, Ctl::Const(*v))).collect();
    bus_def(3, vec![spec(kind, &values).unwrap()])
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

fn direct(kind: EffectKind, values: &[(&str, f32)], right_level: f32) -> (Vec<f32>, Vec<f32>) {
    let sr = 48_000.0;
    let mut p: Vec<_> = params(kind).iter().map(|d| d.default).collect();
    for &(name, value) in values {
        let index = params(kind).iter().position(|d| d.name == name).unwrap();
        p[index] = value;
    }
    let mut st = FxState::default();
    let mut mem = [0.0; stream_dynamics::MEM_LEN];
    stream_dynamics::init(&mut st, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut left = Vec::new();
    let mut right = Vec::new();
    for block in 0..96 {
        let mut l = [0.0; 256];
        let mut r = [0.0; 256];
        for frame in 0..256 {
            let t = (block * 256 + frame) as f32 / sr;
            let amplitude = if block < 32 { 0.45 } else { 0.18 };
            l[frame] = (t * 319.0 * std::f32::consts::TAU).sin() * amplitude;
            r[frame] = (t * 1801.0 * std::f32::consts::TAU).sin() * right_level;
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
        stream_dynamics::process(kind, &p, &mut st, &mut mem, &mut l, &mut r, &mut ctx);
        left.extend(l);
        right.extend(r);
    }
    (left, right)
}

#[test]
fn follower_three_band_alternate_and_controls_respond() {
    let kind = EffectKind::StreamFollower;
    let base = direct(kind, &[("shape", 0.1), ("response", 0.3)], 0.3);
    assert!(base.0.iter().chain(&base.1).all(|x| x.is_finite()));
    assert!(rms(&base.0) > 1.0e-5 && rms(&base.1) > 1.0e-5);
    assert!(difference(&base.0, &base.1) > 1.0e-4);
    for (name, value) in [
        ("shape", 0.9),
        ("response", 0.9),
        ("alternate", 1.0),
        ("excite", 0.6),
        ("cutoff-min", 6000.0),
        ("cutoff-max", 500.0),
    ] {
        let changed = direct(
            kind,
            &[("shape", 0.1), ("response", 0.3), (name, value)],
            0.3,
        );
        assert!(difference(&base.0, &changed.0) > 1.0e-6, "{name}");
    }
    let linked = direct(kind, &[("linked", 1.0), ("global-attack", 0.1)], 0.3);
    let changed = direct(kind, &[("linked", 1.0), ("global-attack", 0.9)], 0.3);
    assert!(difference(&linked.0, &changed.0) > 1.0e-6);
    let changed = direct(kind, &[("linked", 1.0), ("global-decay", 0.9)], 0.3);
    assert!(difference(&linked.0, &changed.0) > 1.0e-6);
    let shape_ignored = direct(
        kind,
        &[("linked", 1.0), ("global-attack", 0.1), ("shape", 0.9)],
        0.3,
    );
    assert_eq!(linked.0, shape_ignored.0);
    let self_detect = direct(kind, &[("excite-source", 0.0)], 0.05);
    let sidechain = direct(kind, &[("excite-source", 1.0)], 0.05);
    assert!(difference(&self_detect.0, &sidechain.0) > 1.0e-5);
    assert!(rms(&sidechain.1) > 1.0e-6, "right audio lane remains live");
    let fallback = direct(kind, &[("excite-source", 1.0)], 0.0);
    let local = direct(kind, &[("excite-source", 0.0)], 0.0);
    assert_eq!(
        fallback.0, local.0,
        "quiet right sidechain falls back to left audio"
    );
}

#[test]
fn follower_filter_only_alternate_changes_cutoff_without_cv_gain() {
    let kind = EffectKind::StreamFollower;
    let low_cv = direct(
        kind,
        &[
            ("alternate", 1.0),
            ("response", 0.0),
            ("cutoff-min", 12_000.0),
            ("cutoff-max", 12_000.0),
        ],
        0.3,
    );
    let high_cv = direct(
        kind,
        &[
            ("alternate", 1.0),
            ("response", 1.0),
            ("cutoff-min", 12_000.0),
            ("cutoff-max", 12_000.0),
        ],
        0.3,
    );
    assert_eq!(low_cv, high_cv, "CV must not modulate alternate gain");

    let low_cutoff = direct(
        kind,
        &[
            ("alternate", 1.0),
            ("response", 0.0),
            ("cutoff-min", 100.0),
            ("cutoff-max", 12_000.0),
        ],
        0.3,
    );
    let high_cutoff = direct(
        kind,
        &[
            ("alternate", 1.0),
            ("response", 1.0),
            ("cutoff-min", 100.0),
            ("cutoff-max", 12_000.0),
        ],
        0.3,
    );
    assert!(difference(&low_cutoff.0, &high_cutoff.0) > 1.0e-3);
    assert!(rms(&high_cutoff.1) > rms(&low_cutoff.1) * 2.0);
}

#[test]
fn compressor_alternate_threshold_amount_and_linked_globals_respond() {
    let kind = EffectKind::StreamCompressor;
    let base = direct(kind, &[("threshold", 0.3), ("amount", 0.4)], 0.2);
    assert!(base.0.iter().chain(&base.1).all(|x| x.is_finite()));
    assert!(rms(&base.0) > 1.0e-5 && rms(&base.1) > 1.0e-5);
    assert!(difference(&base.0, &base.1) > 1.0e-4);
    for (name, value) in [
        ("threshold", 0.8),
        ("amount", 0.9),
        ("alternate", 1.0),
        ("excite", 0.6),
    ] {
        let changed = direct(
            kind,
            &[("threshold", 0.3), ("amount", 0.4), (name, value)],
            0.2,
        );
        assert!(difference(&base.0, &changed.0) > 1.0e-6, "{name}");
    }
    let linked = direct(
        kind,
        &[
            ("linked", 1.0),
            ("global-threshold", 0.3),
            ("global-amount", 0.4),
        ],
        0.2,
    );
    for (name, value) in [
        ("global-attack", 0.9),
        ("global-decay", 0.9),
        ("global-threshold", 0.8),
        ("global-amount", 0.9),
    ] {
        let changed = direct(
            kind,
            &[
                ("linked", 1.0),
                ("global-threshold", 0.3),
                ("global-amount", 0.4),
                (name, value),
            ],
            0.2,
        );
        assert!(difference(&linked.0, &changed.0) > 1.0e-6, "{name}");
    }
    let ignored = direct(
        kind,
        &[
            ("linked", 1.0),
            ("global-threshold", 0.3),
            ("global-amount", 0.4),
            ("threshold", 0.9),
            ("amount", 0.9),
        ],
        0.2,
    );
    assert_eq!(linked.0, ignored.0);
    let self_detect = direct(kind, &[("threshold", 0.3), ("excite-source", 0.0)], 0.02);
    let sidechain = direct(kind, &[("threshold", 0.3), ("excite-source", 1.0)], 0.02);
    assert!(difference(&self_detect.0, &sidechain.0) > 1.0e-5);
    assert!(rms(&sidechain.1) > 1.0e-6);
    let fallback = direct(kind, &[("excite-source", 1.0)], 0.0);
    let local = direct(kind, &[("excite-source", 0.0)], 0.0);
    assert_eq!(
        fallback.0, local.0,
        "quiet right sidechain falls back to left audio"
    );
}

#[test]
fn dynamics_native_browser_codec_rates_and_capacity_preflight() {
    for kind in [EffectKind::StreamFollower, EffectKind::StreamCompressor] {
        let def = effect(kind, &[("mix", 0.2)]);
        let template = BusTemplate::from_def(&def).unwrap();
        let mut bytes = Vec::new();
        encode_bus(&def, false, &mut bytes).unwrap();
        let mut decoded = BusTemplate::new();
        let mut raw = RawGraph::boxed();
        assert_eq!(
            decode_graph(&bytes, &mut raw, &mut decoded).unwrap(),
            GraphKind::Bus(BusId::new(3))
        );
        assert_eq!(decoded.params[0][0].1, Ctl::Const(0.2));
        let cells = Mirror::new(64);
        let mut buses = BusGraph::new(3, 64, 24, &cells, 96_000.0, &caps());
        let mut old = BusTemplate::new();
        old.bus = BusId::new(3);
        old.push(EffectKind::Gain);
        assert!(buses.install(&old, false, 10, 1, &cells, 96_000.0, &caps()));
        let slot = buses.find(BusId::new(3)).unwrap();
        assert!(!buses.install(&template, false, 11, 1, &cells, 96_000.0, &caps()));
        assert_eq!(buses.find(BusId::new(3)), Some(slot));
        assert_eq!(buses.slots[slot].resource, 10);
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let rendered = effect(kind, &[("mix", 1.0)]);
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut native = NativeRig::native_with(cfg);
                native.install(&chain(1, vec![UGenSpec::Saw]));
                native.install_bus(&rendered, false);
                native.install_bus(&bus_def(0, vec![]), true);
                let _ = native.step();
                native.send(event(1, native.engine.now(), &[(ctl::BUS, 3.0)]));
                let (l, r) = native.run(24);
                assert!(l.iter().chain(&r).all(|x| x.is_finite()));
                assert!(rms(&l) + rms(&r) > 1.0e-6);
                let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut browser = BrowserRig::browser_with(cfg);
                let mut record = Vec::new();
                let mut payload = Vec::new();
                encode_inst(&chain(1, vec![UGenSpec::Saw]), &mut payload).unwrap();
                encode_graph_record(10, 1, &payload, &mut record);
                browser.push(&record);
                record.clear();
                encode_graph_record(11, 1, &bytes, &mut record);
                browser.push(&record);
                payload.clear();
                record.clear();
                encode_bus(&bus_def(0, vec![]), true, &mut payload).unwrap();
                encode_graph_record(12, 1, &payload, &mut record);
                browser.push(&record);
                let _ = browser.run(6);
                browser.send(event(1, browser.engine.now(), &[(ctl::BUS, 3.0)]));
                let (l, r) = browser.run(24);
                assert!(l.iter().chain(&r).all(|x| x.is_finite()));
                assert!(rms(&l) + rms(&r) > 1.0e-6);
            }
        }
    }
}
