//! Original four-lane stereo-derived bus mixer, driven by shared poly-LFO lanes.

use super::{bus_def, caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, GraphKind, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::{self, catalog::spec, quad_mixer};
use crate::dsp::graph::{BusId, EffectKind, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::RawGraph;
use crate::host::wire::Ctl;

fn bus(values: &[(&'static str, f32)]) -> crate::dsp::graph::BusDef {
    let named: Vec<_> = values
        .iter()
        .map(|(name, value)| (*name, Ctl::Const(*value)))
        .collect();
    bus_def(3, vec![spec(EffectKind::KeyframeMixer, &named).unwrap()])
}

/// Browser render entry used by the host e2e suite after compiling a public
/// `.vact` bus definition through the evaluator and registry.
pub(crate) fn render_browser_bus(def: &crate::dsp::graph::BusDef) -> (Vec<f32>, Vec<f32>) {
    let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
    cfg.sample_rate = 48_000.0;
    cfg.max_block = 256;
    let mut browser = BrowserRig::browser_with(cfg);
    let mut record = Vec::new();
    let mut payload = Vec::new();
    encode_inst(&chain(1, vec![UGenSpec::Saw]), &mut payload).unwrap();
    encode_graph_record(1, 1, &payload, &mut record);
    browser.push(&record);
    payload.clear();
    record.clear();
    encode_bus(def, false, &mut payload).unwrap();
    encode_graph_record(2, 1, &payload, &mut record);
    browser.push(&record);
    payload.clear();
    record.clear();
    encode_bus(&bus_def(0, vec![]), true, &mut payload).unwrap();
    encode_graph_record(3, 1, &payload, &mut record);
    browser.push(&record);
    let _ = browser.run(6);
    browser.send(event(1, browser.engine.now(), &[(ctl::BUS, 3.0)]));
    browser.run(12)
}

#[test]
fn editor_params_are_codeable_and_capacity_is_bounded() {
    let kind = EffectKind::KeyframeMixer;
    assert_eq!(kind.name(), "keyframe-mixer");
    let editor = crate::dsp::meta::decl_for(kind.name()).unwrap();
    for param in quad_mixer::PARAMS {
        let metadata = editor
            .params
            .iter()
            .find(|meta| meta.name == param.name)
            .unwrap();
        assert_eq!(metadata.default, param.default);
        assert_eq!(metadata.label, crate::dsp::meta::label_of(param.name));
        assert!(metadata.choices.is_empty());
    }
    assert_eq!(editor.params.len(), quad_mixer::PARAMS.len());
    let def = bus(&[
        ("rate", 4.0),
        ("shape", 0.3),
        ("spread", 0.8),
        ("shape-spread", 0.7),
        ("coupling", 0.2),
        ("offset", 0.15),
        ("mix", 0.8),
    ]);
    let mut template = BusTemplate::from_def(&def).unwrap();
    assert_eq!(template.n_params[0] as usize, quad_mixer::PARAMS.len());
    for _ in quad_mixer::PARAMS.len()..crate::dsp::effects::MAX_FX_PARAMS {
        assert!(template.push_param(0, crate::sched::slots::CtlId::new(1), Ctl::Const(0.0)));
    }
    assert!(!template.push_param(0, crate::sched::slots::CtlId::new(1), Ctl::Const(0.0)));
    assert!(spec(kind, &[("unknown-control", Ctl::Const(1.0))]).is_err());
}

#[test]
fn native_browser_bus_codec_rates_blocks_and_last_mix_control() {
    let dry_bus = bus(&[("mix", 0.0)]);
    let wet_bus = bus(&[
        ("rate", 4.0),
        ("shape", 0.3),
        ("spread", 0.8),
        ("shape-spread", 0.7),
        ("coupling", 0.2),
        ("offset", 0.15),
        ("mix", 1.0),
    ]);
    let mut bytes = Vec::new();
    encode_bus(&wet_bus, false, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    let mut decoded = BusTemplate::new();
    assert_eq!(
        decode_graph(&bytes, &mut raw, &mut decoded).unwrap(),
        GraphKind::Bus(BusId::new(3))
    );
    assert_eq!(decoded.n_params[0] as usize, quad_mixer::PARAMS.len());
    let input = chain(1, vec![UGenSpec::Saw]);
    let mut voice_bytes = Vec::new();
    encode_inst(&input, &mut voice_bytes).unwrap();
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [1, 17, 64, 256, 511] {
            let render_native = |chain_def: &crate::dsp::graph::BusDef| {
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut host = NativeRig::native_with(cfg);
                host.install(&input);
                host.install_bus(chain_def, false);
                host.install_bus(&bus_def(0, vec![]), true);
                let _ = host.run(6);
                host.send(event(1, host.engine.now(), &[(ctl::BUS, 3.0)]));
                host.run(12)
            };
            let native = render_native(&wet_bus);
            assert!(native
                .0
                .iter()
                .chain(&native.1)
                .all(|sample| sample.is_finite()));
            assert!(rms(&native.0) + rms(&native.1) > 1.0e-6);
            let dry = render_native(&dry_bus);
            assert!(native
                .0
                .iter()
                .zip(&dry.0)
                .chain(native.1.iter().zip(&dry.1))
                .any(|(a, b)| (a - b).abs() > 1.0e-5));

            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &voice_bytes, &mut record);
            browser.push(&record);
            record.clear();
            encode_graph_record(2, 1, &bytes, &mut record);
            browser.push(&record);
            let mut master = Vec::new();
            encode_bus(&bus_def(0, vec![]), true, &mut master).unwrap();
            record.clear();
            encode_graph_record(3, 1, &master, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(1, browser.engine.now(), &[(ctl::BUS, 3.0)]));
            let browser_output = browser.run(12);
            assert!(browser_output
                .0
                .iter()
                .chain(&browser_output.1)
                .all(|sample| sample.is_finite()));
            assert!(rms(&browser_output.0) + rms(&browser_output.1) > 1.0e-6);
            assert_eq!(native, browser_output);
        }
    }
}

#[test]
fn every_keyframe_mixer_parameter_has_metadata_and_effect_defaults() {
    let kind = EffectKind::KeyframeMixer;
    for (index, def) in effects::params(kind).iter().enumerate() {
        assert_eq!(
            effects::param_ctl(kind, def.name).unwrap().get(),
            effects::EFFECT_PARAM_BASE + index as u16
        );
    }
    assert_eq!(quad_mixer::PARAMS.len(), 7);
}
