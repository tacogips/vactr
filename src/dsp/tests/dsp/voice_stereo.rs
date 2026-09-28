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
            },
            Edge {
                from: 0,
                to: 3,
                port: 0,
            },
            Edge {
                from: 2,
                to: 3,
                port: 1,
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
    let mut rig = NativeRig::native();
    rig.install(&def);
    let _ = rig.step();
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::PAN, 0.0), (ctl::LEGATO, 1.0)],
    ));
    let (left, right) = rig.run(4);
    assert_split(&left, &right);

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

    let mut rig = BrowserRig::browser(4 << 20);
    let mut record = Vec::new();
    encode_graph_record(50, 1, &bytes, &mut record);
    rig.push(&record);
    let _ = rig.run(6);
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::PAN, 0.0), (ctl::LEGATO, 1.0)],
    ));
    let (left, right) = rig.run(4);
    assert_split(&left, &right);
}
