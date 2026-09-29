//! Elements internal model roles, 30-port graph codec and fixed-state tiers.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::EffectKind;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{resonator_modes, CoverageState, ResourceState, ALTERNATE_VOICE};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, elements_internal, BuildEnv, Node, RawGraph, Template};
use crate::host::wire::Ctl;

fn voice(model: f32, alternate: f32) -> InstDef {
    let node = Node::ElementsInternal;
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::ElementsInternal,
            UGenSpec::ElementsInternal,
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
            (0, catalog::port_ctl(&node, 25).unwrap(), Ctl::Const(model)),
            (1, catalog::port_ctl(&node, 25).unwrap(), Ctl::Const(model)),
            (1, catalog::port_ctl(&node, 26).unwrap(), Ctl::Const(1.0)),
            (
                0,
                catalog::port_ctl(&node, 27).unwrap(),
                Ctl::Const(alternate),
            ),
            (
                1,
                catalog::port_ctl(&node, 27).unwrap(),
                Ctl::Const(alternate),
            ),
        ]
        .into_boxed_slice(),
    }
}

#[test]
fn inventory_includes_bounded_alternate() {
    for (index, row) in resonator_modes().iter().enumerate() {
        assert_eq!(usize::from(row.mode), index);
        assert_eq!(row.vactr_template, Some("exciter-voice"));
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
        assert!(row.external_blow && row.external_strike);
        assert_eq!(row.external_effect, Some(EffectKind::ElementsBank));
    }
    assert_eq!(ALTERNATE_VOICE.coverage, CoverageState::Adaptation);
    assert_eq!(ALTERNATE_VOICE.vactr_template, Some("exciter-voice"));
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        assert!((elements_internal::line_len(rate) - 2) as f32 >= rate / 20.0);
    }
}

#[test]
fn final_port_round_trips_and_memory_budget_is_enforced() {
    assert_eq!(catalog::ports(&Node::ElementsInternal).len(), 30);
    let strike_input = catalog::port_ctl(&Node::ElementsInternal, 29).unwrap();
    assert_eq!(
        catalog::port_of(&Node::ElementsInternal, strike_input),
        Some(29)
    );
    let alternate = catalog::port_ctl(&Node::ElementsInternal, 27).unwrap();
    assert_eq!(
        catalog::port_of(&Node::ElementsInternal, alternate),
        Some(27)
    );
    assert_eq!(
        catalog::port_count(&Node::Effect {
            kind: EffectKind::ResonantBank,
            fx: 0
        }),
        18
    );
    let last = catalog::port_ctl(&Node::ElementsInternal, 26).unwrap();
    assert_eq!(catalog::port_of(&Node::ElementsInternal, last), Some(26));
    let def = voice(2.0, 1.0);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    let need = 2 * elements_internal::mem_len(96_000.0);
    let env = BuildEnv {
        sr: 96_000.0,
        caps: caps(),
        voice_mem: need,
    };
    let mut built = Template::boxed();
    built.build(&raw, &env).unwrap();
    assert_eq!(built.mem_total, need);
    assert!(built.has_aux);
    let too_small = BuildEnv {
        voice_mem: need - 1,
        ..env
    };
    assert!(Template::from_inst(&def, &too_small).is_err());
}

#[test]
fn three_roles_render_finite_distinct_main_aux_at_native_browser_rates() {
    for alternate in [0.0, 1.0] {
        let last_model = if alternate == 0.0 { 2 } else { 0 };
        for model in 0..=last_model {
            let def = voice(model as f32, alternate);
            let mut bytes = Vec::new();
            encode_inst(&def, &mut bytes).unwrap();
            for rate in [44_100.0, 48_000.0, 96_000.0] {
                for block in [64, 256] {
                    let mut cfg = config(&caps(), StoreKind::NativeArc);
                    cfg.sample_rate = rate;
                    cfg.max_block = block;
                    cfg.voice_seconds = 0.5;
                    let mut native = NativeRig::native_with(cfg);
                    native.install(&def);
                    let _ = native.step();
                    native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
                    let (left, right) = native.run(40);
                    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
                    assert!(
                        rms(&left) > 1.0e-7 && rms(&right) > 1.0e-7,
                        "native {model}/{rate}/{block}"
                    );
                    assert_ne!(left, right);

                    let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                    cfg.sample_rate = rate;
                    cfg.max_block = block;
                    cfg.voice_seconds = 0.5;
                    let mut browser = BrowserRig::browser_with(cfg);
                    let mut record = Vec::new();
                    encode_graph_record(1, 1, &bytes, &mut record);
                    browser.push(&record);
                    let _ = browser.run(6);
                    browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
                    let (left, right) = browser.run(40);
                    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
                    assert!(
                        rms(&left) > 1.0e-7 && rms(&right) > 1.0e-7,
                        "browser {model}/{rate}/{block}"
                    );
                    assert_ne!(left, right);
                }
            }
        }
    }
}
