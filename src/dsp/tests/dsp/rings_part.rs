//! Six bounded Rings Part adaptations across graph codecs and host tiers.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{resonator_models, rings_coverage_summary, CoverageState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, rings_part, BuildEnv, Node, RawGraph, Template};
use crate::host::wire::Ctl;

fn voice(model: f32) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::RingsPart, UGenSpec::RingsPart, UGenSpec::AuxOut].into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![
            (
                0,
                catalog::port_ctl(&Node::RingsPart, 1).unwrap(),
                Ctl::Const(model),
            ),
            (
                1,
                catalog::port_ctl(&Node::RingsPart, 1).unwrap(),
                Ctl::Const(model),
            ),
            (
                1,
                catalog::port_ctl(&Node::RingsPart, 15).unwrap(),
                Ctl::Const(1.0),
            ),
        ]
        .into_boxed_slice(),
    }
}

#[test]
fn six_position_inventory_is_ordered_and_candid() {
    for (index, row) in resonator_models().iter().enumerate() {
        assert_eq!(usize::from(row.model), index);
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.voice_template, Some("resonator-voice"));
        assert_eq!((row.main_outputs, row.aux_outputs), (1, 1));
        assert!(row.external_excitation);
        assert_eq!(
            row.external_effect,
            Some(crate::dsp::graph::EffectKind::ResonantBank)
        );
    }
    assert!(rings_coverage_summary().contains("external-audio resonant-bank bus adaptation"));
}

#[test]
fn port_fifteen_survives_codec_and_both_tiers_render_at_supported_rates() {
    assert_eq!(catalog::ports(&Node::RingsPart).len(), 17);
    let mode_ctl = catalog::port_ctl(&Node::RingsPart, 15).unwrap();
    assert_eq!(catalog::port_of(&Node::RingsPart, mode_ctl), Some(15));
    for model in 0..=5 {
        let def = voice(model as f32);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        let mut bus = BusTemplate::new();
        decode_graph(&bytes, &mut raw, &mut bus).unwrap();
        let env = BuildEnv {
            sr: 48_000.0,
            caps: caps(),
            voice_mem: 24_000,
        };
        let mut decoded = Template::boxed();
        decoded.build(&raw, &env).unwrap();
        assert_eq!(decoded.mem_total, 2 * rings_part::mem_len(48_000.0));
        assert!(decoded.has_aux);
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
                let (left, right) = native.run(20);
                assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
                assert!(
                    rms(&left) > 1.0e-7 && rms(&right) > 1.0e-7,
                    "native {model}, {rate}, {block}"
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
                let (left, right) = browser.run(20);
                assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
                assert!(
                    rms(&left) > 1.0e-7 && rms(&right) > 1.0e-7,
                    "browser {model}, {rate}, {block}"
                );
                assert_ne!(left, right);
            }
        }
    }
}

#[test]
fn string_models_recur_at_the_requested_20_to_100_hz_period() {
    use crate::dsp::arena::{SampleStore, StoreKind};
    use crate::dsp::caps::CapabilitySet;
    use crate::dsp::effects::FxStats;
    use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};

    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    for rate in [44_100.0_f32, 48_000.0, 96_000.0] {
        for frequency in [20.0_f32, 35.0, 50.0, 100.0] {
            let expected = (rate / frequency).round() as usize;
            let line_len = rings_part::line_len(rate);
            assert!(expected <= line_len - 2);
            for model in [1.0, 2.0, 4.0, 5.0] {
                let mut inputs = [Inp::Val(0.0); MAX_PORTS];
                inputs[0] = Inp::Val(frequency);
                inputs[1] = Inp::Val(model);
                inputs[7] = Inp::Val(0.0); // no excitation after the seeded impulse
                inputs[14] = Inp::Val(2.0); // main output is the root line
                let mut state = NodeState::default();
                state.u[0] = 1;
                let mut memory = vec![0.0; rings_part::mem_len(rate)];
                memory[line_len - expected] = 1.0;
                let mut signal = vec![0.0; expected * 2 + 1];
                let kx = Kx {
                    sr: rate,
                    gate: signal.len(),
                    bank: None,
                    store: &store,
                    caps: &caps,
                    stats: &mut stats,
                    seed: 1,
                };
                for block in signal.chunks_mut(64) {
                    rings_part::render(&inputs, &mut state, &mut memory, block, &kx);
                }
                let observed = signal[1..]
                    .iter()
                    .position(|value| value.abs() > 0.02)
                    .map(|offset| offset + 1)
                    .expect("recurring string impulse");
                assert_eq!(
                    observed, expected,
                    "model {model}, {rate} Hz, pitch {frequency}"
                );
                assert!(
                    signal[expected].abs() > 0.01,
                    "model {model}, rate {rate}, pitch {frequency}"
                );
                assert!(signal.iter().all(|value| value.is_finite()));
            }
        }
    }
}
