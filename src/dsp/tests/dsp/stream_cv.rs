//! Streams filter-controller and Lorenz CV roles placed on original stereo DSP.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::stream_cv;
use crate::dsp::effects::{FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

fn effect(kind: EffectKind, named: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let values: Vec<_> = named.iter().map(|(n, v)| (*n, Ctl::Const(*v))).collect();
    bus_def(3, vec![spec(kind, &values).unwrap()])
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

fn direct(kind: EffectKind, values: &[(&str, f32)], right_level: f32) -> (Vec<f32>, Vec<f32>) {
    let sr = 48_000.0;
    let defs = if kind == EffectKind::StreamFilter {
        stream_cv::FILTER_PARAMS
    } else {
        stream_cv::LORENZ_PARAMS
    };
    let mut p: Vec<_> = defs.iter().map(|def| def.default).collect();
    for &(name, value) in values {
        let index = defs.iter().position(|def| def.name == name).unwrap();
        p[index] = value;
    }
    let mut st = FxState::default();
    let mut mem = [0.0; stream_cv::MEM_LEN];
    stream_cv::init(kind, &mut st, &mut mem);
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
            l[frame] = (t * 3200.0 * std::f32::consts::TAU).sin() * 0.45;
            r[frame] = (t * 740.0 * std::f32::consts::TAU).sin() * right_level;
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
        stream_cv::process(kind, &p, &mut st, &mut mem, &mut l, &mut r, &mut ctx);
        left.extend(l);
        right.extend(r);
    }
    (left, right)
}

#[test]
fn signed_filter_amount_offset_sidechain_and_neutral_gain() {
    let kind = EffectKind::StreamFilter;
    let low = direct(
        kind,
        &[("offset", 0.5), ("amount", 0.0), ("excite", 1.0)],
        0.3,
    );
    let high = direct(
        kind,
        &[("offset", 0.5), ("amount", 1.0), ("excite", 1.0)],
        0.3,
    );
    assert!(
        rms(&high.0) > rms(&low.0) * 1.5,
        "amount sign moves cutoff: low={} high={}",
        rms(&low.0),
        rms(&high.0)
    );
    let offset = direct(kind, &[("offset", 0.9), ("amount", 0.5)], 0.3);
    let center = direct(kind, &[("offset", 0.5), ("amount", 0.5)], 0.3);
    assert!(delta(&offset.0, &center.0) > 1.0e-4);
    let side = direct(kind, &[("excite-source", 1.0), ("amount", 1.0)], 0.1);
    let self_detect = direct(kind, &[("excite-source", 0.0), ("amount", 1.0)], 0.1);
    assert!(delta(&side.0, &self_detect.0) > 1.0e-5);
    assert!(rms(&side.1) > 1.0e-6, "right audio is preserved");
    let fallback = direct(kind, &[("excite-source", 1.0), ("amount", 1.0)], 0.0);
    let self_detect = direct(kind, &[("excite-source", 0.0), ("amount", 1.0)], 0.0);
    assert_eq!(fallback.0, self_detect.0);
    let constant_cutoff_a = direct(
        kind,
        &[
            ("amount", 0.0),
            ("cutoff-min", 12_000.0),
            ("cutoff-max", 12_000.0),
        ],
        0.3,
    );
    let constant_cutoff_b = direct(
        kind,
        &[
            ("amount", 1.0),
            ("cutoff-min", 12_000.0),
            ("cutoff-max", 12_000.0),
        ],
        0.3,
    );
    assert_eq!(constant_cutoff_a, constant_cutoff_b, "no gain CV path");
    assert!(low.0.iter().chain(&high.0).all(|v| v.is_finite()));
}

#[test]
fn lorenz_is_bounded_deterministic_and_swaps_stereo_cv_roles() {
    let kind = EffectKind::StreamLorenz;
    let base = direct(kind, &[("rate", 0.7), ("balance", 0.5)], 0.45);
    assert_eq!(base, direct(kind, &[("rate", 0.7), ("balance", 0.5)], 0.45));
    assert!(base.0.iter().chain(&base.1).all(|v| v.is_finite()));
    assert!(rms(&base.0) > 1.0e-5 && rms(&base.1) > 1.0e-5);
    assert!(delta(&base.0, &base.1) > 1.0e-4);
    for (name, value) in [
        ("rate", 0.1),
        ("balance", 0.0),
        ("excite", 0.7),
        ("cutoff-min", 4000.0),
        ("cutoff-max", 500.0),
    ] {
        let changed = direct(
            kind,
            &[("rate", 0.7), ("balance", 0.5), (name, value)],
            0.45,
        );
        assert!(delta(&base.0, &changed.0) > 1.0e-6, "{name}");
    }
    let side = direct(kind, &[("excite-source", 1.0)], 0.1);
    let self_detect = direct(kind, &[("excite-source", 0.0)], 0.1);
    assert!(delta(&side.0, &self_detect.0) > 1.0e-6);
    let fallback = direct(kind, &[("excite-source", 1.0)], 0.0);
    let local = direct(kind, &[("excite-source", 0.0)], 0.0);
    assert_eq!(fallback.0, local.0);
}

#[test]
fn final_streams_effects_native_browser_codec_and_live_bus_preflight() {
    for kind in [EffectKind::StreamFilter, EffectKind::StreamLorenz] {
        let bus = effect(kind, &[("mix", 0.2)]);
        let template = BusTemplate::from_def(&bus).unwrap();
        let mut bytes = Vec::new();
        encode_bus(&bus, false, &mut bytes).unwrap();
        let mut decoded = BusTemplate::new();
        let mut raw = RawGraph::boxed();
        assert_eq!(
            decode_graph(&bytes, &mut raw, &mut decoded).unwrap(),
            GraphKind::Bus(BusId::new(3))
        );
        assert_eq!(decoded.params[0][0].1, Ctl::Const(0.2));
        let cells = Mirror::new(64);
        let mut buses = BusGraph::new(3, 64, 12, &cells, 96_000.0, &caps());
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
                assert!(l.iter().chain(&r).all(|v| v.is_finite()));
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
                assert!(l.iter().chain(&r).all(|v| v.is_finite()));
                assert!(rms(&l) + rms(&r) > 1.0e-6);
            }
        }
    }
}
