//! A bounded voice graph with independently routed main/aux outputs.

use super::{chain, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{RawGraph, Template};

fn split_voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::SinOsc,
            UGenSpec::Tri,
            UGenSpec::AuxOut,
            UGenSpec::Add,
        ]
        .into_boxed_slice(),
        edges: vec![
            Edge {
                from: 1,
                to: 2,
                port: 0,

                output: 0,
            },
            Edge {
                from: 0,
                to: 3,
                port: 0,

                output: 0,
            },
            Edge {
                from: 2,
                to: 3,
                port: 1,

                output: 0,
            },
        ]
        .into_boxed_slice(),
        node_params: Box::new([]),
    }
}

fn assert_split(left: &[f32], right: &[f32]) {
    assert!(left.iter().chain(right).all(|x| x.is_finite()));
    assert!(rms(left) > 1.0e-2 && rms(right) > 1.0e-2);
    let delta = left
        .iter()
        .zip(right)
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / left.len() as f32;
    assert!(delta > 0.02, "independent output paths: {delta}");
}

#[test]
fn native_split_voice_and_mono_pan_remain_distinct() {
    let def = split_voice();
    let mut left_pan = NativeRig::native();
    left_pan.install(&def);
    let _ = left_pan.step();
    left_pan.send(event(
        1,
        left_pan.engine.now(),
        &[(ctl::PAN, 0.0), (ctl::LEGATO, 1.0)],
    ));
    let (left_at_zero, right_at_zero) = left_pan.run(4);

    let mut center_pan = NativeRig::native();
    center_pan.install(&def);
    let _ = center_pan.step();
    center_pan.send(event(
        1,
        center_pan.engine.now(),
        &[(ctl::PAN, 0.5), (ctl::LEGATO, 1.0)],
    ));
    let (left_at_center, right_at_center) = center_pan.run(4);

    let mut right_pan = NativeRig::native();
    right_pan.install(&def);
    let _ = right_pan.step();
    right_pan.send(event(
        1,
        right_pan.engine.now(),
        &[(ctl::PAN, 1.0), (ctl::LEGATO, 1.0)],
    ));
    let (left_at_one, right_at_one) = right_pan.run(4);

    // Owner decision: main/aux pairs use unity-center balance; pan 0 keeps
    // main at unity and silences aux, while center leaves both at unity.
    assert_eq!(left_at_zero, left_at_center, "main remains at unity");
    assert_eq!(right_at_center, right_at_one, "aux remains at unity");
    assert!(rms(&left_at_zero) > 1.0e-2);
    assert!(rms(&right_at_zero) < 1.0e-5, "pan 0 silences aux");
    assert!(rms(&left_at_one) < 1.0e-5, "pan 1 silences main");
    assert_split(&left_at_center, &right_at_center);

    let mut mono = NativeRig::native();
    mono.install(&chain(1, vec![UGenSpec::SinOsc]));
    let _ = mono.step();
    mono.send(event(
        1,
        mono.engine.now(),
        &[(ctl::PAN, 0.0), (ctl::LEGATO, 1.0)],
    ));
    let (left, right) = mono.run(4);
    assert!(rms(&left) > 1.0e-2);
    assert!(rms(&right) < 1.0e-5, "legacy mono panning stays intact");
}

#[test]
fn browser_codec_preserves_aux_output_marker_and_audio() {
    let def = split_voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert!(native.has_aux);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    decode_graph(&bytes, &mut raw, &mut bus).unwrap();
    let mut decoded = Template::boxed();
    decoded.build(&raw, &env).unwrap();
    assert_eq!(decoded.nodes(), native.nodes());
    assert!(decoded.has_aux);

    let mut left_pan = BrowserRig::browser(4 << 20);
    let mut record = Vec::new();
    encode_graph_record(50, 1, &bytes, &mut record);
    left_pan.push(&record);
    let _ = left_pan.run(6);
    left_pan.send(event(
        1,
        left_pan.engine.now(),
        &[(ctl::PAN, 0.0), (ctl::LEGATO, 1.0)],
    ));
    let (left_at_zero, right_at_zero) = left_pan.run(4);

    let mut center_pan = BrowserRig::browser(4 << 20);
    center_pan.push(&record);
    let _ = center_pan.run(6);
    center_pan.send(event(
        1,
        center_pan.engine.now(),
        &[(ctl::PAN, 0.5), (ctl::LEGATO, 1.0)],
    ));
    let (left_at_center, right_at_center) = center_pan.run(4);

    let mut right_pan = BrowserRig::browser(4 << 20);
    right_pan.push(&record);
    let _ = right_pan.run(6);
    right_pan.send(event(
        1,
        right_pan.engine.now(),
        &[(ctl::PAN, 1.0), (ctl::LEGATO, 1.0)],
    ));
    let (left_at_one, right_at_one) = right_pan.run(4);

    // Owner decision: main/aux pairs use unity-center balance; the codec must
    // preserve the aux marker so its center-pan audio remains on the right.
    assert_eq!(left_at_zero, left_at_center, "main remains at unity");
    assert_eq!(right_at_center, right_at_one, "aux remains at unity");
    assert!(rms(&left_at_zero) > 1.0e-2);
    assert!(rms(&right_at_zero) < 1.0e-5, "pan 0 silences aux");
    assert!(rms(&left_at_one) < 1.0e-5, "pan 1 silences main");
    assert_split(&left_at_center, &right_at_center);
}
