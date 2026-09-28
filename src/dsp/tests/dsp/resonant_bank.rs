//! External audio excites a bounded Rings-role bus resonator.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, StoreKind};
use crate::dsp::bus::{BusGraph, BusTemplate};
use crate::dsp::cells::Mirror;
use crate::dsp::effects::catalog::spec;
use crate::dsp::effects::resonant_bank;
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

fn effect(values: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let named: Vec<_> = values
        .iter()
        .map(|(name, value)| (*name, Ctl::Const(*value)))
        .collect();
    bus_def(3, vec![spec(EffectKind::ResonantBank, &named).unwrap()])
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

fn native(
    sr: f32,
    block: usize,
    values: &[(&'static str, f32)],
    source: bool,
) -> (Vec<f32>, Vec<f32>) {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.sample_rate = sr;
    cfg.max_block = block;
    let mut rig = NativeRig::native_with(cfg);
    if source {
        rig.install(&chain(1, vec![UGenSpec::Saw]));
    }
    rig.install_bus(&effect(values), false);
    rig.install_bus(&bus_def(0, vec![]), true);
    let _ = rig.step();
    if source {
        rig.send(event(
            1,
            rig.engine.now(),
            &[(ctl::BUS, 3.0), (ctl::PAN, 0.0), (ctl::LEGATO, 2.0)],
        ));
    }
    rig.run(24)
}

#[test]
fn external_audio_reaches_distinct_main_aux_at_all_rates_and_blocks() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let (l, r) = native(sr, block, &[("model", 2.0)], true);
            assert!(l.iter().chain(&r).all(|v| v.is_finite()));
            assert!(rms(&l) > 1.0e-5, "main {sr}/{block}");
            assert!(rms(&r) > 1.0e-6, "aux {sr}/{block}");
            assert!(
                difference(&l, &r) > 1.0e-5,
                "independent outputs {sr}/{block}"
            );
            let (silent_l, silent_r) = native(sr, block, &[("model", 2.0)], false);
            assert!(rms(&silent_l) + rms(&silent_r) < 1.0e-8);
        }
    }
}

#[test]
fn six_modes_and_key_controls_change_external_response() {
    let baseline = &[("model", 4.0), ("polyphony", 4.0)];
    let base = native(48_000.0, 256, baseline, true);
    for mode in 0..=5 {
        let (l, r) = native(48_000.0, 256, &[("model", mode as f32)], true);
        assert!(l.iter().chain(&r).all(|v| v.is_finite()), "model {mode}");
        assert!(rms(&l) + rms(&r) > 1.0e-5, "model {mode}");
    }
    for (name, value) in [
        ("structure", 0.9),
        ("brightness", 0.9),
        ("damping", 0.9),
        ("position", 0.9),
        ("note", 12.0),
        ("tonic", 12.0),
        ("fm", 12.0),
        ("chord", 5.0),
        ("polyphony", 2.0),
        ("external-mix", 0.3),
        ("gate", 0.2),
        ("mix", 0.3),
    ] {
        let mut controls = baseline.to_vec();
        controls.push((name, value));
        let output = native(48_000.0, 256, &controls, true);
        assert!(
            difference(&base.0, &output.0) + difference(&base.1, &output.1) > 1.0e-6,
            "{name}"
        );
    }
}

#[test]
fn internal_exciter_strum_flags_and_note_have_audible_roles() {
    let active = &[
        ("model", 3.0),
        ("internal-exciter", 1.0),
        ("external-mix", 0.0),
        ("strum", 1.0),
        ("note", 12.0),
    ];
    let voiced = native(48_000.0, 256, active, false);
    assert!(rms(&voiced.0) + rms(&voiced.1) > 1.0e-5);
    for (name, value) in [
        ("strum", 0.0),
        ("internal-exciter", 0.0),
        ("internal-strum", 0.0),
        ("internal-note", 0.0),
        ("note", 24.0),
        ("gate", 0.0),
    ] {
        let mut values = active.to_vec();
        values.push((name, value));
        let alternate = native(48_000.0, 256, &values, false);
        assert!(
            difference(&voiced.0, &alternate.0) + difference(&voiced.1, &alternate.1) > 1.0e-6,
            "{name}"
        );
    }
}

#[test]
fn all_seventeen_controls_reach_editor_and_last_wire_slot() {
    let decl = crate::types::manifest::HostManifest::spec_default()
        .editor_decl("resonant-bank")
        .unwrap();
    assert_eq!(decl.params.len(), resonant_bank::PARAMS.len());
    assert_eq!(resonant_bank::PARAMS.len(), 17);
    for param in resonant_bank::PARAMS {
        assert!(
            decl.params.iter().any(|p| p.name == param.name),
            "{}",
            param.name
        );
    }
    let bus = effect(&[("mix", 0.125)]);
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
    assert_eq!(decoded.n_params[0], 1);
    assert_eq!(decoded.params[0][0].1, Ctl::Const(0.125));
}

#[test]
fn insufficient_memory_preserves_live_bus() {
    let cells = Mirror::new(64);
    let mut buses = BusGraph::new(3, 64, 2000, &cells, 96_000.0, &caps());
    let mut old = BusTemplate::new();
    old.bus = BusId::new(3);
    old.push(EffectKind::Gain);
    assert!(buses.install(&old, false, 10, 1, &cells, 96_000.0, &caps()));
    let slot = buses.find(BusId::new(3)).unwrap();
    let candidate = BusTemplate::from_def(&effect(&[])).unwrap();
    assert!(!buses.install(&candidate, false, 11, 1, &cells, 96_000.0, &caps()));
    assert_eq!(buses.find(BusId::new(3)), Some(slot));
    assert_eq!(buses.slots[slot].resource, 10);
    assert!(resonant_bank::mem_len(96_000.0) > 2000);
}

#[test]
fn zero_capture_allowance_still_installs_and_renders_resonator() {
    let mut no_capture = caps();
    no_capture.max_capture_seconds = 0.0;
    let cells = Mirror::new(64);
    let mut buses = BusGraph::new(3, 64, 30_000, &cells, 96_000.0, &no_capture);
    let candidate = BusTemplate::from_def(&effect(&[("model", 2.0)])).unwrap();
    assert!(buses.install(&candidate, false, 11, 1, &cells, 96_000.0, &no_capture));
    assert_eq!(
        buses
            .find(BusId::new(3))
            .map(|slot| buses.slots[slot].resource),
        Some(11)
    );

    let mut cfg = config(&no_capture, StoreKind::NativeArc);
    cfg.sample_rate = 96_000.0;
    cfg.max_block = 256;
    let mut rig = NativeRig::native_with(cfg);
    rig.install(&chain(1, vec![UGenSpec::Saw]));
    rig.install_bus(&effect(&[("model", 2.0)]), false);
    rig.install_bus(&bus_def(0, vec![]), true);
    let _ = rig.step();
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::BUS, 3.0), (ctl::LEGATO, 2.0)],
    ));
    let (left, right) = rig.run(24);
    assert!(rms(&left) > 1.0e-5 && rms(&right) > 1.0e-6);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
}

#[test]
fn browser_codec_installs_resonator_with_last_control() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut rig = BrowserRig::browser_with(cfg);
            let inst = chain(1, vec![UGenSpec::Saw]);
            let mut payload = Vec::new();
            encode_inst(&inst, &mut payload).unwrap();
            let mut record = Vec::new();
            encode_graph_record(10, 1, &payload, &mut record);
            rig.push(&record);
            for (resource, bus, master) in [
                (11, effect(&[("mix", 1.0)]), false),
                (12, bus_def(0, vec![]), true),
            ] {
                payload.clear();
                record.clear();
                encode_bus(&bus, master, &mut payload).unwrap();
                encode_graph_record(resource, 1, &payload, &mut record);
                rig.push(&record);
            }
            let _ = rig.step();
            rig.send(event(
                1,
                rig.engine.now(),
                &[(ctl::BUS, 3.0), (ctl::LEGATO, 2.0)],
            ));
            let (l, r) = rig.run(24);
            assert!(l.iter().chain(&r).all(|v| v.is_finite()));
            assert!(rms(&l) > 1.0e-5 && rms(&r) > 1.0e-6, "{sr}/{block}");
        }
    }
}
