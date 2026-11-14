//! Integer and organ spectra, fixed-state budget, and both graph codecs.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, BuildError, Node, RawGraph, Template};
use crate::host::wire::Ctl;

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::SpectrumPair,
            UGenSpec::SpectrumPair,
            UGenSpec::AuxOut,
        ]
        .into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,

            output: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![(
            1,
            catalog::port_ctl(&Node::SpectrumPair, 4).unwrap(),
            Ctl::Const(1.0),
        )]
        .into_boxed_slice(),
    }
}

fn assert_paths(l: &[f32], r: &[f32]) {
    assert!(l.iter().chain(r).all(|x| x.is_finite()));
    assert!(rms(l) > 0.001 && rms(r) > 0.001);
    #[allow(clippy::cast_precision_loss)]
    let delta = l.iter().zip(r).map(|(a, b)| (a - b).abs()).sum::<f32>() / l.len() as f32;
    assert!(delta > 0.01, "integer/organ spectra differ: {delta}");
}

#[test]
fn additive_voice_state_is_budgeted_before_install() {
    let mut env = NativeRig::native().engine.build_env();
    env.voice_mem = 95;
    assert_eq!(
        Template::from_inst(&voice(), &env).unwrap_err(),
        BuildError::MemExceeded
    );
    env.voice_mem = 96;
    let template = Template::from_inst(&voice(), &env).unwrap();
    assert_eq!(template.mem_total, 96);
}

#[test]
fn native_spectrum_pair_keeps_two_series_distinct() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = NativeRig::native_with(cfg);
            rig.install(&voice());
            let _ = rig.step();
            rig.send(event(
                1,
                rig.engine.now(),
                &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = rig.run(20);
            assert_paths(&l, &r);
        }
    }
}

#[test]
fn browser_spectrum_pair_codec_keeps_both_series() {
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

    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            rig.push(&record);
            let _ = rig.run(6);
            rig.send(event(
                1,
                rig.engine.now(),
                &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = rig.run(20);
            assert_paths(&l, &r);
        }
    }
}
