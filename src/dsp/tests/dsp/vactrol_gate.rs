//! Contract tests for the shared Plaits gate and decay-mod nodes.

use super::{caps, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::build::{lower_inst, Lowering};
use crate::dsp::bus::BusTemplate;
use crate::dsp::controls::{self, CtlDomain, CtlRoute};
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenInput, UGenKind, UGenNode, UGenSpec};
use crate::dsp::ugen::{
    catalog, vactrol_gate, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::reader::span::{FileId, Span};
use crate::value::intern::intern_kw;
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
fn render_gate(
    input: &[f32],
    block: usize,
    rate: f32,
    gate_samples: usize,
    mode: f32,
    slot: f32,
    lane: f32,
    clocked: f32,
    decay: f32,
    color: f32,
    velocity: f32,
) -> (Vec<f32>, NodeState) {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let mut state = NodeState::default();
    let mut mem = [0.0; vactrol_gate::GATE_STATE_FLOATS];
    let mut result = Vec::with_capacity(input.len());
    let mut offset = 0;
    while offset < input.len() {
        let count = block.min(input.len() - offset);
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        ins[0] = Inp::Buf(&input[offset..offset + count]);
        ins[1] = Inp::Val(440.0);
        ins[2] = Inp::Val(velocity);
        ins[3] = Inp::Val(mode);
        ins[4] = Inp::Val(decay);
        ins[5] = Inp::Val(color);
        ins[6] = Inp::Val(slot);
        ins[7] = Inp::Val(lane);
        ins[8] = Inp::Val(clocked);
        let mut out = vec![0.0; count];
        let kx = Kx {
            sr: rate,
            gate: gate_samples.saturating_sub(offset).min(count),
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 17,
        };
        vactrol_gate::render_gate(&ins, &mut state, &mut mem, &mut out, &kx);
        result.extend_from_slice(&out);
        offset += count;
    }
    (result, state)
}

fn render_decay_mod(input_len: usize, target: f32, amount: f32, decay: f32) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let mut state = NodeState::default();
    let mut mem = [0.0; vactrol_gate::DECAY_MOD_STATE_FLOATS];
    let mut result = Vec::with_capacity(input_len);
    let mut offset = 0;
    while offset < input_len {
        let count = 64.min(input_len - offset);
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        ins[0] = Inp::Val(decay);
        ins[1] = Inp::Val(amount);
        ins[2] = Inp::Val(target);
        let mut out = vec![0.0; count];
        let kx = Kx {
            sr: 48_000.0,
            gate: count,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 0,
        };
        vactrol_gate::render_decay_mod(&ins, &mut state, &mut mem, &mut out, &kx);
        result.extend_from_slice(&out);
        offset += count;
    }
    result
}

fn deterministic_noise(count: usize) -> Vec<f32> {
    #[allow(clippy::cast_precision_loss)]
    (0..count)
        .map(|i| ((i * 73 % 257) as f32 - 128.0) / 128.0)
        .collect()
}

fn high_band_ratio(signal: &[f32]) -> f32 {
    let high = signal
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).powi(2))
        .sum::<f32>();
    let total = signal.iter().map(|sample| sample * sample).sum::<f32>();
    high / total.max(1.0e-12)
}

#[test]
fn off_and_invalid_slots_copy_every_input_bit() {
    let mut input = deterministic_noise(512);
    input[2] = 0.0;
    input[3] = -0.0;
    input[4] = 1.0e-30;
    input[5] = 3.0;
    input[6] = f32::NAN;
    for block in [64, 97] {
        let (actual, state) = render_gate(
            &input, block, 48_000.0, 512, 0.0, 10.0, 0.0, 0.0, 0.5, 0.5, 1.0,
        );
        assert_eq!(
            actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            input.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(state.u[2], 0);
        assert!(!state.done());
        for invalid_slot in [-0.1, 24.0, f32::NAN] {
            let (invalid, invalid_state) = render_gate(
                &input,
                block,
                48_000.0,
                512,
                1.0,
                invalid_slot,
                0.0,
                0.0,
                0.5,
                0.5,
                1.0,
            );
            assert_eq!(
                invalid.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                input.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
            );
            assert_eq!(invalid_state.u[2], 0);
            assert!(!invalid_state.done());
        }
    }
}

#[test]
fn ping_shape_and_decay_control_change_the_tail() {
    let input = vec![0.5; 24_000];
    let (slow, state) = render_gate(
        &input,
        97,
        48_000.0,
        input.len(),
        1.0,
        0.0,
        0.0,
        0.0,
        0.8,
        0.5,
        1.0,
    );
    let (fast, _) = render_gate(
        &input,
        97,
        48_000.0,
        input.len(),
        1.0,
        0.0,
        0.0,
        0.0,
        0.2,
        0.5,
        1.0,
    );
    let peak = slow.iter().take(240).copied().fold(0.0_f32, f32::max);
    let tail_slow = slow[4_800..9_600].iter().map(|x| x * x).sum::<f32>();
    let tail_fast = fast[4_800..9_600].iter().map(|x| x * x).sum::<f32>();
    assert!(slow[0].abs() < 0.1);
    assert!(peak > 0.1, "ping peak {peak}");
    assert!(tail_slow > tail_fast, "slow={tail_slow} fast={tail_fast}");
    assert_eq!(state.u[2], 1);
}

#[test]
fn color_controls_high_frequency_bleed() {
    let input = deterministic_noise(12_000);
    let (dark, _) = render_gate(
        &input,
        97,
        48_000.0,
        input.len(),
        1.0,
        0.0,
        0.0,
        0.0,
        0.5,
        0.0,
        1.0,
    );
    let (bright, _) = render_gate(
        &input,
        97,
        48_000.0,
        input.len(),
        1.0,
        0.0,
        0.0,
        0.0,
        0.5,
        1.0,
        1.0,
    );
    assert!(high_band_ratio(&dark) < high_band_ratio(&bright));
}

#[test]
fn level_tracks_velocity_and_finishes_after_gate_closes() {
    let input = vec![0.5; 48_000];
    let (full, full_state) = render_gate(
        &input, 64, 48_000.0, 2_400, 2.0, 0.0, 0.0, 0.0, 0.2, 0.5, 1.0,
    );
    let (half, _) = render_gate(
        &input, 64, 48_000.0, 2_400, 2.0, 0.0, 0.0, 0.0, 0.2, 0.5, 0.5,
    );
    let rms = |samples: &[f32]| {
        (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
    };
    let ratio = rms(&half[1_200..2_300]) / rms(&full[1_200..2_300]);
    assert!(
        (0.8125 * 0.85..=0.8125 * 1.15).contains(&ratio),
        "ratio {ratio}"
    );
    assert_eq!(full_state.u[0], 1);
    assert!(full_state.done());
    assert!(full[2_400..].iter().any(|sample| sample.abs() > 1.0e-5));
}

#[test]
fn bypass_gain_limiter_and_clocked_position_seven_follow_registration() {
    let input = vec![0.25, -0.5, 1.5, -2.0];
    let (out, state) = render_gate(&input, 4, 48_000.0, 4, 1.0, 21.0, 0.0, 0.0, 0.5, 0.5, 1.0);
    for (actual, source) in out.iter().zip(&input) {
        assert!((actual - crate::dsp::ugen::voice_layer::clip_unit(source * 0.8)).abs() <= 1.0e-7);
    }
    assert_eq!(state.u[2], 0);

    let loud = vec![4.0; 48_000];
    let (limited, _) = render_gate(
        &loud,
        256,
        48_000.0,
        loud.len(),
        1.0,
        19.0,
        0.0,
        0.0,
        0.5,
        0.5,
        1.0,
    );
    assert!(limited[24_000..].iter().all(|sample| sample.abs() <= 0.801));
    let (aux, _) = render_gate(
        &loud[..256],
        64,
        48_000.0,
        256,
        1.0,
        19.0,
        1.0,
        0.0,
        0.5,
        0.5,
        1.0,
    );
    assert!(aux
        .iter()
        .all(
            |sample| (sample - crate::dsp::ugen::voice_layer::clip_unit(4.0 * 0.8)).abs() <= 1.0e-6
        ));

    let (clocked, clocked_state) =
        render_gate(&input, 4, 48_000.0, 4, 1.0, 7.0, 0.0, 1.0, 0.5, 0.5, 1.0);
    assert_eq!(
        clocked,
        input
            .iter()
            .map(|x| crate::dsp::ugen::voice_layer::clip_unit(x * 0.5))
            .collect::<Vec<_>>()
    );
    assert_eq!(clocked_state.u[2], 0);
    let (_, unclocked_state) =
        render_gate(&input, 4, 48_000.0, 4, 1.0, 7.0, 0.0, 0.0, 0.5, 0.5, 1.0);
    assert_eq!(unclocked_state.u[2], 1);
}

#[test]
fn control_clock_is_partition_invariant_across_rates() {
    let input = deterministic_noise(30_000);
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        let (a, _) = render_gate(
            &input,
            64,
            rate,
            input.len(),
            1.0,
            0.0,
            0.0,
            0.0,
            0.6,
            0.4,
            1.0,
        );
        let (b, _) = render_gate(
            &input,
            256,
            rate,
            input.len(),
            1.0,
            0.0,
            0.0,
            0.0,
            0.6,
            0.4,
            1.0,
        );
        let (c, _) = render_gate(
            &input,
            97,
            rate,
            input.len(),
            1.0,
            0.0,
            0.0,
            0.0,
            0.6,
            0.4,
            1.0,
        );
        assert_eq!(a, b, "rate {rate}: 64 vs 256");
        assert_eq!(a, c, "rate {rate}: 64 vs 97");
    }
}

#[test]
fn decay_mod_handles_zero_and_triggered_amounts() {
    assert!(render_decay_mod(1_024, 0.0, 0.0, 0.5)
        .iter()
        .all(|x| *x == 0.0));
    assert!(render_decay_mod(1_024, 1.0, 0.0, 0.5)
        .iter()
        .all(|x| *x == 1.0));
    let output = render_decay_mod(12_000, 0.0, 1.0, 0.5);
    let short = crate::dsp::ugen::voice_layer::decay_terms(0.5, 0.5).0;
    let expected = 0.9975 * (1.0 - 2.0 * short);
    assert!((output[0] - expected).abs() <= 1.0e-6);
    assert!(output.windows(2).all(|pair| pair[1] <= pair[0]));
}

#[test]
fn kinds_ports_controls_codec_and_template_build_match_contract() {
    let gate = UGenSpec::VactrolGate;
    let decay = UGenSpec::DecayMod;
    let names = |spec: &UGenSpec| {
        catalog::ports(&Node::from_spec(spec))
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&gate),
        [
            "in",
            "freq",
            "velocity",
            "lpg-mode",
            "lpg-decay",
            "lpg-color",
            "slot",
            "lane",
            "clocked"
        ]
    );
    assert_eq!(names(&decay), ["lpg-decay", "amount", "target"]);
    assert_eq!(crate::dsp::build::ports(&gate), names(&gate));
    assert_eq!(crate::dsp::build::ports(&decay), names(&decay));
    assert!(!Node::VactrolGate.is_env() && !Node::DecayMod.is_env());
    assert_eq!(
        controls::row("lpg-mode").map(|row| (
            row.ctl.get(),
            row.default,
            row.range,
            row.route,
            row.domain
        )),
        Some((
            153,
            0.0,
            (0.0, 2.0),
            CtlRoute::InstParam,
            CtlDomain::Enum(controls::LPG_MODES)
        ))
    );
    assert_eq!(controls::LPG_MODES, &["off", "ping", "level"]);
    assert_eq!(
        controls::row("lpg-decay").map(|row| (row.ctl.get(), row.default)),
        Some((154, 0.5))
    );
    assert_eq!(
        controls::row("lpg-color").map(|row| (row.ctl.get(), row.default)),
        Some((155, 0.5))
    );
    let manifest = crate::types::manifest::HostManifest::spec_default();
    let gate_editor = manifest
        .editor_decls()
        .iter()
        .find(|decl| decl.name == "vactrol-gate")
        .expect("gate editor metadata");
    let mode_meta = gate_editor
        .params
        .iter()
        .find(|param| param.name == "lpg-mode")
        .expect("mode metadata");
    assert_eq!(mode_meta.default, 0.0);
    assert_eq!(mode_meta.choices, controls::LPG_MODES);
    assert_eq!(mode_meta.label, "Lpg mode");

    let def = InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::Const(0.5), gate, decay].into_boxed_slice(),
        edges: vec![
            Edge {
                from: 0,
                to: 1,
                port: 0,
                output: 0,
            },
            Edge {
                from: 1,
                to: 2,
                port: 1,
                output: 0,
            },
        ]
        .into_boxed_slice(),
        node_params: Box::new([]),
    };
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).expect("new kinds encode");
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    decode_graph(&bytes, &mut raw, &mut bus).expect("new kinds decode");
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).expect("native template builds");
    let mut decoded = Template::boxed();
    decoded.build(&raw, &env).expect("decoded template builds");
    assert_eq!(decoded.nodes(), native.nodes());
    assert_eq!(decoded.mem_total, native.mem_total);
    let mut reencoded = Vec::new();
    encode_inst(&def, &mut reencoded).expect("graph re-encodes");
    assert_eq!(reencoded, bytes);
}

#[test]
fn lowering_places_named_gate_arguments_on_contract_ports() {
    let oscillator = Rc::new(UGenNode {
        kind: UGenKind::Ugen(UGenSpec::SinOsc),
        args: vec![(None, UGenInput::Const(220.0))].into_boxed_slice(),
    });
    let gate = UGenNode {
        kind: UGenKind::Ugen(UGenSpec::VactrolGate),
        args: vec![
            (None, UGenInput::Node(oscillator)),
            (Some(intern_kw("freq")), UGenInput::Const(330.0)),
            (Some(intern_kw("velocity")), UGenInput::Const(0.7)),
            (Some(intern_kw("slot")), UGenInput::Const(10.0)),
            (Some(intern_kw("lane")), UGenInput::Const(1.0)),
        ]
        .into_boxed_slice(),
    };
    let caps = caps();
    let mut alloc = || None;
    let (def, _) = lower_inst(
        InstId::new(1),
        &Rc::new(gate),
        &[],
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc,
        },
    )
    .expect("gate lowering succeeds");
    let gate_index = u16::try_from(def.nodes.len() - 1).expect("gate index fits");
    let mut ports = def
        .edges
        .iter()
        .filter(|edge| edge.to == gate_index)
        .map(|edge| edge.port)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    assert_eq!(ports, [0, 1, 2, 6, 7]);
    for (port, value) in [(1, 330.0), (2, 0.7), (6, 10.0), (7, 1.0)] {
        let source = def
            .edges
            .iter()
            .find(|edge| edge.to == gate_index && edge.port == port)
            .map(|edge| usize::from(edge.from))
            .expect("named port edge");
        assert_eq!(def.nodes[source], UGenSpec::Const(value));
    }
    assert_eq!(
        crate::dsp::graph::decl_for_spec(&UGenSpec::VactrolGate),
        crate::dsp::graph::OutputDecl::Fixed {
            shape: crate::dsp::graph::NodeAudioShape::MONO,
            names: &[]
        }
    );
    assert!(catalog::port_default(&Node::VactrolGate, 3) == 0.0);
    assert_eq!(catalog::ugen_name(&UGenSpec::VactrolGate), "vactrol-gate");
    assert_eq!(catalog::ugen_name(&UGenSpec::DecayMod), "decay-mod");
}
