//! Independent low-pass main and high-pass auxiliary outputs through both tiers.

use super::{ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, Node, RawGraph, Template};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

fn voice() -> InstDef {
    let morph = CtlId::new(79);
    let timbre = CtlId::new(80);
    let harmonics = CtlId::new(81);
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::VaSource,
            UGenSpec::VaFilter,
            UGenSpec::VaSource,
            UGenSpec::VaFilter,
            UGenSpec::AuxOut,
        ]
        .into_boxed_slice(),
        edges: vec![
            Edge {
                from: 0,
                to: 1,
                port: 0,
            },
            Edge {
                from: 2,
                to: 3,
                port: 0,
            },
            Edge {
                from: 3,
                to: 4,
                port: 0,
            },
        ]
        .into_boxed_slice(),
        node_params: vec![
            (0, morph, Ctl::Const(0.5)),
            (2, morph, Ctl::Const(0.5)),
            (1, timbre, Ctl::Const(0.5)),
            (3, timbre, Ctl::Const(0.5)),
            (1, harmonics, Ctl::Const(0.5)),
            (3, harmonics, Ctl::Const(0.5)),
            (
                3,
                catalog::port_ctl(&Node::VaFilter, 4).unwrap(),
                Ctl::Const(1.0),
            ),
        ]
        .into_boxed_slice(),
    }
}

fn assert_paths(left: &[f32], right: &[f32]) {
    assert!(left.iter().chain(right).all(|x| x.is_finite()));
    assert!(rms(left) > 0.001 && rms(right) > 0.001);
    #[allow(clippy::cast_precision_loss)]
    let delta = left
        .iter()
        .zip(right)
        .map(|(l, r)| (l - r).abs())
        .sum::<f32>()
        / left.len() as f32;
    assert!(delta > 0.005, "main and aux are distinct: {delta}");
}

#[test]
fn native_filter_voice_has_independent_outputs() {
    let mut rig = NativeRig::native();
    rig.install(&voice());
    let _ = rig.step();
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
    ));
    let (left, right) = rig.run(4);
    assert_paths(&left, &right);
}

#[test]
fn browser_codec_preserves_filter_voice_paths() {
    let def = voice();
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

    let mut rig = BrowserRig::browser(4 << 20);
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    rig.push(&record);
    let _ = rig.run(6);
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
    ));
    let (left, right) = rig.run(4);
    assert_paths(&left, &right);
}
