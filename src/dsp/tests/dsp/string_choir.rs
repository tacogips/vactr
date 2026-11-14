//! Separate string-synth adaptation, native/browser codec and low-note period.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ported::{string_synth_path, CoverageState, ResourceState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, string_choir, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice(fx: f32) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::StringChoir,
            UGenSpec::StringChoir,
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
            (
                0,
                catalog::port_ctl(&Node::StringChoir, 14).unwrap(),
                Ctl::Const(fx),
            ),
            (
                1,
                catalog::port_ctl(&Node::StringChoir, 14).unwrap(),
                Ctl::Const(fx),
            ),
            (
                1,
                catalog::port_ctl(&Node::StringChoir, 15).unwrap(),
                Ctl::Const(1.0),
            ),
        ]
        .into_boxed_slice(),
    }
}

#[test]
fn separate_coverage_row_reports_six_distinct_fx_roles() {
    let row = string_synth_path();
    assert_eq!(row.voice_template, "string-choir-voice");
    assert_eq!(row.coverage, CoverageState::Adaptation);
    assert_eq!(row.resources, ResourceState::Replacement);
    assert!(!row.external_excitation);
    assert_eq!(row.fx_roles.len(), 6);
    for (i, name) in row.fx_roles.iter().enumerate() {
        assert!(!name.is_empty());
        assert!(!row.fx_roles[..i].contains(name));
    }
}

#[test]
fn codec_port_fifteen_and_both_host_tiers_render_without_callback_allocation() {
    assert_eq!(catalog::ports(&Node::StringChoir).len(), 16);
    let mode_ctl = catalog::port_ctl(&Node::StringChoir, 15).unwrap();
    assert_eq!(catalog::port_of(&Node::StringChoir, mode_ctl), Some(15));
    let def = voice(4.0);
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
    assert_eq!(decoded.mem_total, 2 * string_choir::mem_len(48_000.0));
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
                rms(&left) > 1.0e-6 && rms(&right) > 1.0e-6,
                "native {rate}, {block}"
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
                rms(&left) > 1.0e-6 && rms(&right) > 1.0e-6,
                "browser {rate}, {block}"
            );
            assert_ne!(left, right);
        }
    }
}

#[test]
fn root_comb_line_repeats_at_20_to_100_hz_across_rates() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    for rate in [44_100.0_f32, 48_000.0, 96_000.0] {
        for frequency in [20.0_f32, 35.0, 50.0, 100.0] {
            let period = (rate / frequency).round() as usize;
            let len = string_choir::line_len(rate);
            assert!(period <= len - 2);
            let mut inputs = [Inp::Val(0.0); MAX_PORTS];
            inputs[0] = Inp::Val(frequency);
            inputs[1] = Inp::Val(1.0); // strong comb feedback
            inputs[5] = Inp::Val(0.0); // no analytic oscillator
            inputs[6] = Inp::Val(0.0); // no fresh pluck
            inputs[13] = Inp::Val(1.0);
            inputs[14] = Inp::Val(1.0); // chorus, whose echo is below threshold
            let mut state = NodeState::default();
            state.u[0] = 1;
            let mut mem = vec![0.0; string_choir::mem_len(rate)];
            mem[len - period] = 1.0;
            let mut signal = vec![0.0; period * 2 + 1];
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
                string_choir::render(&inputs, &mut state, &mut mem, block, &kx);
            }
            let observed = signal[1..]
                .iter()
                .position(|sample| sample.abs() > 0.015)
                .map(|i| i + 1)
                .expect("recurring string pulse");
            assert_eq!(observed, period, "rate {rate}, pitch {frequency}");
            assert!(signal.iter().all(|sample| sample.is_finite()));
        }
    }
}
