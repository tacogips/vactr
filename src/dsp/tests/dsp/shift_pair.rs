//! Warps hidden-mode role as an original bounded quadrature bus effect.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::shift_pair;
use crate::dsp::effects::{FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

fn effect(named: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let values: Vec<_> = named.iter().map(|(n, v)| (*n, Ctl::Const(*v))).collect();
    bus_def(3, vec![spec(EffectKind::ShiftPair, &values).unwrap()])
}

fn direct(values: &[(&str, f32)], external_hz: f32) -> (Vec<f32>, Vec<f32>) {
    let sr = 48_000.0;
    let mut p: Vec<_> = shift_pair::PARAMS.iter().map(|d| d.default).collect();
    for &(name, value) in values {
        let index = shift_pair::PARAMS
            .iter()
            .position(|d| d.name == name)
            .unwrap();
        p[index] = value;
    }
    let mut st = FxState::default();
    let mut mem = [0.0; shift_pair::MEM_LEN];
    shift_pair::init(&mut st, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let cap = caps();
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut left = Vec::new();
    let mut right = Vec::new();
    for block in 0..188 {
        let mut l = [0.0; 256];
        let mut r = [0.0; 256];
        for frame in 0..256 {
            let t = (block * 256 + frame) as f32 / sr;
            l[frame] = (t * external_hz * std::f32::consts::TAU).cos() * 0.5;
            r[frame] = (t * 440.0 * std::f32::consts::TAU).cos() * 0.5;
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
        shift_pair::process(&p, &mut st, &mut mem, &mut l, &mut r, &mut ctx);
        left.extend(l);
        right.extend(r);
    }
    (left, right)
}

fn tone(samples: &[f32], hz: f32) -> f32 {
    let start = 12_288;
    let mut i = 0.0;
    let mut q = 0.0;
    for (frame, sample) in samples.iter().enumerate().skip(start) {
        let phase = frame as f32 * hz * std::f32::consts::TAU / 48_000.0;
        i += sample * phase.cos();
        q += sample * phase.sin();
    }
    (i * i + q * q).sqrt() / (samples.len() - start) as f32
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn signed_shift_has_correct_up_down_direction_and_zero_position() {
    let positive = direct(&[("shift-pot", 0.541_666_7)], 0.0);
    assert!(tone(&positive.0, 540.0) > tone(&positive.0, 340.0) * 3.0);
    assert!(tone(&positive.1, 340.0) > tone(&positive.1, 540.0) * 3.0);
    let negative = direct(&[("shift-pot", 0.458_333_3)], 0.0);
    assert!(tone(&negative.0, 340.0) > tone(&negative.0, 540.0) * 3.0);
    assert!(tone(&negative.1, 540.0) > tone(&negative.1, 340.0) * 3.0);
    let cv_positive = direct(&[("shift-pot", 0.5), ("shift-cv", 1.0 / 6.0)], 0.0);
    let cv_negative = direct(&[("shift-pot", 0.5), ("shift-cv", -1.0 / 6.0)], 0.0);
    assert!(tone(&cv_positive.0, 540.0) > tone(&cv_positive.0, 340.0) * 3.0);
    assert!(tone(&cv_negative.0, 340.0) > tone(&cv_negative.0, 540.0) * 3.0);
    let zero = direct(&[("shift-pot", 0.5)], 0.0);
    assert!(tone(&zero.0, 440.0) > 0.05);
    assert!(delta(&zero.0, &zero.1) < 0.001);
}

#[test]
fn external_carrier_phase_and_other_controls_reach_distinct_outputs() {
    let base = direct(&[("carrier-wave", 0.0), ("timbre", 1.0)], 200.0);
    assert!(base.0.iter().chain(&base.1).all(|v| v.is_finite()));
    assert!(
        tone(&base.0, 640.0) > tone(&base.0, 240.0) * 2.0,
        "main up={} down={}",
        tone(&base.0, 640.0),
        tone(&base.0, 240.0)
    );
    assert!(tone(&base.1, 240.0) > tone(&base.1, 640.0) * 2.0);
    for (name, values) in [
        (
            "phase-shift",
            vec![("carrier-wave", 0.0), ("phase-shift", 0.25)],
        ),
        ("carrier-wave", vec![("carrier-wave", 2.0)]),
        ("timbre", vec![("carrier-wave", 0.0), ("timbre", 0.0)]),
        ("feedback", vec![("carrier-wave", 0.0), ("feedback", 0.8)]),
        ("dry-wet", vec![("carrier-wave", 0.0), ("dry-wet", 0.0)]),
    ] {
        let rendered = direct(&values, 200.0);
        assert!(delta(&base.0, &rendered.0) > 1.0e-4, "{name}");
        assert!(rendered.0.iter().chain(&rendered.1).all(|v| v.is_finite()));
    }
    let dry = direct(&[("carrier-wave", 0.0), ("dry-wet", 0.0)], 200.0);
    assert_eq!(dry.0, dry.1, "both dry outputs carry the modulator");
    let feedback = direct(&[("carrier-wave", 0.0), ("feedback", 0.95)], 200.0);
    assert!(feedback.0.iter().chain(&feedback.1).all(|v| v.abs() <= 4.0));
}

#[test]
fn shift_pair_codec_rates_and_memory_preflight_preserve_live_bus() {
    let bus = effect(&[("shift-pot", 0.6), ("mix", 0.4)]);
    let template = BusTemplate::from_def(&bus).unwrap();
    let mut bytes = Vec::new();
    encode_bus(&bus, false, &mut bytes).unwrap();
    let mut decoded = BusTemplate::new();
    let mut raw = RawGraph::boxed();
    assert_eq!(
        decode_graph(&bytes, &mut raw, &mut decoded).unwrap(),
        GraphKind::Bus(BusId::new(3))
    );
    assert_eq!(decoded.params[0][0].1, Ctl::Const(0.6));
    let cells = Mirror::new(64);
    let mut buses = BusGraph::new(3, 64, 100, &cells, 96_000.0, &caps());
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
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut native = NativeRig::native_with(cfg);
            native.install(&chain(1, vec![UGenSpec::SinOsc]));
            native.install(&chain(2, vec![UGenSpec::Saw]));
            native.install_bus(&bus, false);
            native.install_bus(&bus_def(0, vec![]), true);
            let _ = native.step();
            let t = native.engine.now();
            for (id, pan) in [(1, 0.0), (2, 1.0)] {
                native.send(event(
                    id,
                    t,
                    &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 2.0)],
                ));
            }
            let (l, r) = native.run(24);
            assert!(l.iter().chain(&r).all(|v| v.is_finite()));
            assert!(rms(&l) + rms(&r) > 1.0e-6);
            assert!(delta(&l, &r) > 1.0e-5);
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            for (resource, source) in [
                chain(1, vec![UGenSpec::SinOsc]),
                chain(2, vec![UGenSpec::Saw]),
            ]
            .into_iter()
            .enumerate()
            {
                let mut payload = Vec::new();
                let mut record = Vec::new();
                encode_inst(&source, &mut payload).unwrap();
                encode_graph_record(10 + resource as u32, 1, &payload, &mut record);
                browser.push(&record);
            }
            let mut record = Vec::new();
            encode_graph_record(20, 1, &bytes, &mut record);
            browser.push(&record);
            record.clear();
            let mut payload = Vec::new();
            encode_bus(&bus_def(0, vec![]), true, &mut payload).unwrap();
            encode_graph_record(21, 1, &payload, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            let t = browser.engine.now();
            for (id, pan) in [(1, 0.0), (2, 1.0)] {
                browser.send(event(
                    id,
                    t,
                    &[(ctl::PAN, pan), (ctl::BUS, 3.0), (ctl::LEGATO, 2.0)],
                ));
            }
            let (l, r) = browser.run(24);
            assert!(l.iter().chain(&r).all(|v| v.is_finite()));
            assert!(rms(&l) + rms(&r) > 1.0e-6);
        }
    }
}
