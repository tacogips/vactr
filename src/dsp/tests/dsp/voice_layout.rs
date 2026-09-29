//! Native voice routing and dense channel-slice capacity for MOD004-22.

use super::{caps, config, ctl, event, rms, NativeRig};
use crate::dsp::arena::StoreKind;
use crate::dsp::effects::catalog::spec as effect_spec;
use crate::dsp::effects::prim::{balance_gains, pan_gains};
use crate::dsp::graph::{BankRef, Edge, EffectKind, InstDef, InstId, UGenSpec};
use crate::dsp::ugen::{BuildError, Template};
use crate::host::wire::Ctl;

fn graph(nodes: Vec<UGenSpec>, edges: Vec<Edge>) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        node_params: Box::new([]),
    }
}

fn graph_with_params(
    nodes: Vec<UGenSpec>,
    edges: Vec<Edge>,
    params: Vec<(crate::sched::slots::CtlId, Ctl)>,
) -> InstDef {
    let mut def = graph(nodes, edges);
    def.params = params.into_boxed_slice();
    def
}

fn edge(from: u16, to: u16, port: u8, output: u8) -> Edge {
    Edge {
        from,
        to,
        port,
        output,
    }
}

fn render(def: &InstDef, pan: f32) -> (Vec<f32>, Vec<f32>) {
    let mut rig = NativeRig::native();
    rig.install(def);
    let _ = rig.step();
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::PAN, pan), (ctl::LEGATO, 10.0)],
    ));
    rig.run(1)
}

#[test]
fn balance_pan_is_unity_center_and_mono_keeps_equal_power() {
    assert_eq!(balance_gains(0.5), (1.0, 1.0));
    assert_eq!(balance_gains(0.0), (1.0, 0.0));
    assert_eq!(balance_gains(1.0), (0.0, 1.0));
    assert_eq!(balance_gains(f32::NAN), pan_gains(f32::NAN));

    let def = graph(vec![UGenSpec::Const(1.0)], vec![]);
    let (left, right) = render(&def, 0.2);
    let gains = pan_gains(0.2);
    assert!(left.iter().all(|sample| *sample == gains.0));
    assert!(right.iter().all(|sample| *sample == gains.1));
}

fn aux_graph() -> InstDef {
    graph(
        vec![
            UGenSpec::Const(0.5),
            UGenSpec::Const(0.25),
            UGenSpec::AuxOut,
        ],
        vec![edge(1, 2, 0, 0)],
    )
}

#[test]
fn main_aux_voice_uses_unity_center_balance() {
    let (left, right) = render(&aux_graph(), 0.5);
    assert!(left.iter().all(|sample| *sample == 0.5));
    assert!(right.iter().all(|sample| *sample == 0.25));

    let (left, right) = render(&aux_graph(), 0.2);
    assert!(left.iter().all(|sample| *sample == 0.5));
    assert!(right.iter().all(|sample| *sample == 0.1));

    let (left, right) = render(&aux_graph(), 0.8);
    let expected_left = 0.5 * balance_gains(0.8).0;
    assert!(left.iter().all(|sample| *sample == expected_left));
    assert!(right.iter().all(|sample| *sample == 0.25));
}

fn stereo_sample_graph(sink: UGenSpec, sink_port: u8) -> InstDef {
    graph(
        vec![UGenSpec::SamplePlay(BankRef::new(41)), sink],
        vec![edge(0, 1, sink_port, 1)],
    )
}

fn render_sample_graph(def: &InstDef) -> (Vec<f32>, Vec<f32>) {
    let mut rig = NativeRig::native();
    let frame = [0.2_f32, 0.7_f32];
    rig.sample(41, frame.repeat(256), 2);
    rig.install(def);
    let _ = rig.step();
    rig.send(event(1, rig.engine.now(), &[(ctl::LEGATO, 10.0)]));
    rig.run(1)
}

#[test]
fn stereo_effect_and_mul_preserve_each_sample_channel() {
    let gain = effect_spec(EffectKind::Gain, &[("gain", Ctl::Const(-6.0206))]).unwrap();
    let (left, right) = render_sample_graph(&stereo_sample_graph(UGenSpec::Effect(gain), 0));
    assert!(left.iter().all(|sample| (*sample - 0.1).abs() < 0.001));
    assert!(right.iter().all(|sample| (*sample - 0.35).abs() < 0.001));

    let mul = graph_with_params(
        vec![
            UGenSpec::SamplePlay(BankRef::new(41)),
            UGenSpec::Param(crate::sched::slots::CtlId::new(90)),
            UGenSpec::Mul,
        ],
        vec![edge(0, 2, 0, 1), edge(1, 2, 1, 0)],
        vec![(crate::sched::slots::CtlId::new(90), Ctl::Const(0.5))],
    );
    let (left, right) = render_sample_graph_with_def(&mul);
    assert!(left.iter().all(|sample| *sample == 0.1));
    assert!(right.iter().all(|sample| *sample == 0.35));
}

fn render_sample_graph_with_def(def: &InstDef) -> (Vec<f32>, Vec<f32>) {
    render_sample_graph(def)
}

fn stereo_sink_graph() -> InstDef {
    graph(
        vec![
            UGenSpec::SinOsc,
            UGenSpec::Out3,
            UGenSpec::SamplePlay(BankRef::new(41)),
            UGenSpec::Effect(effect_spec(EffectKind::Gain, &[("gain", Ctl::Const(0.0))]).unwrap()),
        ],
        vec![edge(0, 1, 0, 0), edge(2, 3, 0, 1)],
    )
}

fn out3_reference_graph() -> InstDef {
    graph(
        vec![UGenSpec::SinOsc, UGenSpec::Out3],
        vec![edge(0, 1, 0, 0)],
    )
}

fn render_four_channel_stereo(def: &InstDef) -> [Vec<f32>; 4] {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.output_channels = 4;
    let mut rig = NativeRig::native_with(cfg);
    rig.sample(41, [0.2_f32, 0.7_f32].repeat(256), 2);
    rig.install(def);
    let _ = rig.step();
    rig.send(event(1, rig.engine.now(), &[(ctl::LEGATO, 10.0)]));
    rig.run_four(1)
}

fn render_out3_reference() -> [Vec<f32>; 4] {
    render_four_channel_stereo(&out3_reference_graph())
}

#[test]
fn stereo_voice_ignores_mono_out3_sink_when_mixing_right_channel() {
    let with_tap = render_four_channel_stereo(&stereo_sink_graph());
    let out3_reference = render_out3_reference();
    assert!(with_tap[0].iter().all(|sample| *sample == 0.2));
    assert!(with_tap[1].iter().all(|sample| *sample == 0.7));
    assert_eq!(
        with_tap[2], out3_reference[2],
        "out3 stem matches reference"
    );
    assert!(with_tap[3].iter().all(|sample| *sample == 0.0));
}

#[test]
fn stereo_voice_off_center_pan_silences_the_opposite_channel() {
    let def = stereo_sample_graph(
        UGenSpec::Effect(effect_spec(EffectKind::Gain, &[("gain", Ctl::Const(0.0))]).unwrap()),
        0,
    );
    let render_pan = |pan| {
        let mut rig = NativeRig::native();
        rig.sample(41, [0.2_f32, 0.7_f32].repeat(256), 2);
        rig.install(&def);
        let _ = rig.step();
        rig.send(event(
            1,
            rig.engine.now(),
            &[(ctl::PAN, pan), (ctl::LEGATO, 10.0)],
        ));
        rig.run(1)
    };
    let (left_center, right_center) = render_pan(0.5);
    let (left_zero, right_zero) = render_pan(0.0);
    let (left_one, right_one) = render_pan(1.0);
    assert_eq!(left_zero, left_center);
    assert_eq!(right_one, right_center);
    assert!(rms(&right_zero) < 1.0e-5, "pan 0 silences right");
    assert!(rms(&left_one) < 1.0e-5, "pan 1 silences left");
}

#[test]
fn stereo_add_sums_left_and_right_without_downmix() {
    let def = graph(
        vec![
            UGenSpec::SamplePlay(BankRef::new(41)),
            UGenSpec::SamplePlay(BankRef::new(41)),
            UGenSpec::Add,
        ],
        vec![edge(0, 2, 0, 1), edge(1, 2, 1, 1)],
    );
    let (left, right) = render_sample_graph(&def);
    assert!(left.iter().all(|sample| *sample == 0.4));
    assert!(right.iter().all(|sample| *sample == 1.4));
}

fn sample_add_tree(leaves: usize) -> InstDef {
    let mut nodes = vec![UGenSpec::SamplePlay(BankRef::new(41)); leaves];
    let mut edges = Vec::with_capacity((leaves - 1) * 2);
    let mut level: Vec<u16> = (0..leaves as u16).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if pair.len() == 1 {
                next.push(pair[0]);
                continue;
            }
            let node = nodes.len() as u16;
            nodes.push(UGenSpec::Add);
            edges.push(edge(
                pair[0],
                node,
                0,
                if pair[0] < leaves as u16 { 1 } else { 0 },
            ));
            edges.push(edge(
                pair[1],
                node,
                1,
                if pair[1] < leaves as u16 { 1 } else { 0 },
            ));
            next.push(node);
        }
        level = next;
    }
    graph(nodes, edges)
}

#[test]
fn dense_slice_capacity_accepts_508_rejects_513_and_keeps_256_mono_nodes() {
    let env = NativeRig::native().engine.build_env();
    let too_large = sample_add_tree(103);
    let err = Template::from_inst(&too_large, &env).unwrap_err();
    assert_eq!(err, BuildError::TooManyBuffers);
    assert!(err.message().contains("512"));

    let fits = sample_add_tree(102);
    let template = Template::from_inst(&fits, &env).unwrap();
    assert_eq!(template.n_slices, 508);
    assert!(template.stereo);

    let mono = graph(vec![UGenSpec::Const(0.0); 256], vec![]);
    assert_eq!(Template::from_inst(&mono, &env).unwrap().n_slices, 256);
}

#[test]
fn unconsumed_pair_aux_keeps_legacy_single_output_dispatch() {
    let def = graph(vec![UGenSpec::VaFilter], vec![]);
    let env = NativeRig::native().engine.build_env();
    let template = Template::from_inst(&def, &env).unwrap();
    assert_eq!(template.nodes[0].outs[1], crate::dsp::graph::DISCARD);
    let (left, right) = render(&def, 0.5);
    assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
}

fn va_filter_graph(pair: bool) -> InstDef {
    use crate::dsp::ugen::{catalog, Node};

    let (nodes, edges, filter_indices) = if pair {
        (
            vec![
                UGenSpec::VaSource,
                UGenSpec::VaFilter,
                UGenSpec::Add,
                UGenSpec::AuxOut,
            ],
            vec![edge(0, 1, 0, 0), edge(1, 2, 0, 0), edge(1, 3, 0, 1)],
            vec![1],
        )
    } else {
        (
            vec![
                UGenSpec::VaSource,
                UGenSpec::VaFilter,
                UGenSpec::VaFilter,
                UGenSpec::Add,
                UGenSpec::AuxOut,
            ],
            vec![
                edge(0, 1, 0, 0),
                edge(0, 2, 0, 0),
                edge(1, 3, 0, 0),
                edge(2, 4, 0, 0),
            ],
            vec![1, 2],
        )
    };
    let mut node_params = vec![(0, crate::sched::slots::CtlId::new(79), Ctl::Const(0.5))];
    for (index, node) in filter_indices.into_iter().enumerate() {
        for port in [1, 2] {
            node_params.push((
                node,
                catalog::port_ctl(&Node::VaFilter, port).unwrap(),
                Ctl::Const(0.5),
            ));
        }
        if !pair && index == 1 {
            node_params.push((
                node,
                catalog::port_ctl(&Node::VaFilter, 4).unwrap(),
                Ctl::Const(1.0),
            ));
        }
    }
    let mut def = graph(nodes, edges);
    def.node_params = node_params.into_boxed_slice();
    def
}

#[test]
fn selected_va_filter_pair_matches_two_legacy_nodes_bitwise() {
    let pair = va_filter_graph(true);
    let duplicate = va_filter_graph(false);
    let (pair_l, pair_r) = render(&pair, 0.5);
    let (old_l, old_r) = render(&duplicate, 0.5);
    assert_eq!(pair_l, old_l);
    assert_eq!(pair_r, old_r);
}

#[test]
fn sample_output_zero_keeps_legacy_mono_downmix() {
    let def = graph(vec![UGenSpec::SamplePlay(BankRef::new(41))], vec![]);
    let (left, right) = render_sample_graph(&def);
    let expected = 0.45 * pan_gains(0.5).0;
    assert!(left.iter().all(|sample| *sample == expected));
    assert_eq!(left, right);
}
