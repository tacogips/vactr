//! Streams CV-role inventory and original stereo digital effect tests.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, SampleStore, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::dynamic_control;
use crate::dsp::effects::{FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ported::{streams_functions, CoverageState, ResourceState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

fn effect(kind: EffectKind, values: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let named: Vec<_> = values.iter().map(|(n, v)| (*n, Ctl::Const(*v))).collect();
    bus_def(3, vec![spec(kind, &named).unwrap()])
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

fn direct(
    kind: EffectKind,
    values: &[(&str, f32)],
    left_on: bool,
    right_on: bool,
) -> (Vec<f32>, Vec<f32>) {
    let sr = 48_000.0;
    let mut p: Vec<_> = dynamic_control::PARAMS.iter().map(|d| d.default).collect();
    for &(name, value) in values {
        let index = dynamic_control::PARAMS
            .iter()
            .position(|d| d.name == name)
            .unwrap();
        p[index] = value;
    }
    let mut state = FxState::default();
    let mut mem = [0.0; dynamic_control::MEM_LEN];
    dynamic_control::init(&mut state, &mut mem);
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
            if left_on {
                l[frame] = (t * 319.0 * std::f32::consts::TAU).sin() * 0.45;
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
        dynamic_control::process(kind, &p, &mut state, &mut mem, &mut l, &mut r, &mut ctx);
        left.extend(l);
        right.extend(r);
    }
    (left, right)
}

#[test]
fn six_cv_rows_separate_firmware_from_digital_audio() {
    let rows = streams_functions();
    assert_eq!(rows.len(), 6);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(usize::from(row.position), i);
        assert!(row.firmware_output.contains("CV"));
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
        assert!(row.digital_effect.is_some());
    }
}

#[test]
fn envelope_ad_ar_and_vactr_damped_plucked_have_distinct_audio() {
    for kind in [EffectKind::StreamEnvelope, EffectKind::StreamVactr] {
        let base = direct(kind, &[("shape", 0.1)], true, true);
        let alt = direct(kind, &[("shape", 0.1), ("alternate", 1.0)], true, true);
        assert!(base.0.iter().chain(&base.1).all(|x| x.is_finite()));
        assert!(rms(&base.0) > 1.0e-6 && rms(&base.1) > 1.0e-6);
        assert!(
            delta(&base.0, &base.1) > 1.0e-5,
            "independent stereo {kind:?}"
        );
        assert!(delta(&base.0, &alt.0) > 1.0e-5, "alternate {kind:?}");
        let silence = direct(kind, &[], false, false);
        assert!(rms(&silence.0) + rms(&silence.1) < 1.0e-8);
    }
}

#[test]
fn local_global_link_and_sidechain_controls_have_meaningful_roles() {
    for kind in [EffectKind::StreamEnvelope, EffectKind::StreamVactr] {
        let baseline = &[("shape", 0.1), ("response", 0.3)];
        let base = direct(kind, baseline, true, true);
        for (name, value) in [
            ("shape", 0.9),
            ("response", 0.9),
            ("threshold", 0.4),
            ("cutoff-min", 4000.0),
            ("cutoff-max", 1000.0),
            ("gate", 1.0),
            ("excite", 0.8),
        ] {
            let changed = direct(kind, &[baseline[0], baseline[1], (name, value)], true, true);
            assert!(delta(&base.0, &changed.0) > 1.0e-6, "{kind:?}/{name}");
        }
        let linked = direct(
            kind,
            &[
                ("linked", 1.0),
                ("global-attack", 0.1),
                ("global-decay", 0.1),
            ],
            true,
            true,
        );
        let linked_changed = direct(
            kind,
            &[
                ("linked", 1.0),
                ("global-attack", 0.9),
                ("global-decay", 0.9),
            ],
            true,
            true,
        );
        assert!(
            delta(&linked.0, &linked_changed.0) > 1.0e-5,
            "linked globals"
        );
        let shape_ignored = direct(
            kind,
            &[
                ("linked", 1.0),
                ("global-attack", 0.1),
                ("global-decay", 0.1),
                ("shape", 0.9),
            ],
            true,
            true,
        );
        assert!(
            delta(&linked.0, &shape_ignored.0) == 0.0,
            "source linked mode substitutes globals"
        );
        let left_self = direct(kind, &[("excite-source", 0.0)], true, false);
        let left_side = direct(kind, &[("excite-source", 1.0)], true, false);
        assert!(
            rms(&left_self.0) > rms(&left_side.0) * 2.0,
            "right sidechain controls left"
        );
        let linked_audio = direct(kind, &[("linked", 1.0)], true, true);
        assert!(
            delta(&linked_audio.0, &linked_audio.1) > 1.0e-5,
            "audio lanes stay independent"
        );
    }
}

#[test]
fn explicit_trigger_starts_an_event_below_the_audio_detector_threshold() {
    for kind in [EffectKind::StreamEnvelope, EffectKind::StreamVactr] {
        let controls = &[("threshold", 1.0), ("alternate", 1.0)];
        let idle = direct(kind, controls, true, false);
        let struck = direct(
            kind,
            &[("threshold", 1.0), ("alternate", 1.0), ("trigger", 1.0)],
            true,
            false,
        );
        assert!(rms(&idle.0) < 1.0e-8, "idle {kind:?}");
        assert!(rms(&struck.0) > 1.0e-6, "trigger {kind:?}");
    }
}

#[test]
fn native_browser_codec_memory_preflight_and_rates() {
    for kind in [EffectKind::StreamEnvelope, EffectKind::StreamVactr] {
        assert_eq!(dynamic_control::PARAMS.len(), 14);
        let bus = effect(kind, &[("mix", 0.2)]);
        let template = BusTemplate::from_def(&bus).unwrap();
        assert_eq!(template.n_params[0], 1);
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
        let mut buses = BusGraph::new(3, 64, 20, &cells, 96_000.0, &caps());
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
                let def = effect(kind, &[("shape", 0.1), ("gate", 1.0)]);
                let mut bytes = Vec::new();
                encode_bus(&def, false, &mut bytes).unwrap();
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut native = NativeRig::native_with(cfg);
                native.install(&chain(1, vec![UGenSpec::Saw]));
                native.install_bus(&def, false);
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
