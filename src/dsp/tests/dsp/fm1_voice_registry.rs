//! FM1V-30 native/browser graph codec parity and append-only tags.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    gendyn, hurdy_gurdy, kalimba, scanned, tonewheel, vosim, RawGraph, Template,
};

const VOICES: [(UGenSpec, usize, &str); 6] = [
    (UGenSpec::KalimbaCore, kalimba::STATE_FLOATS, "kalimba-core"),
    (
        UGenSpec::TonewheelCore,
        tonewheel::STATE_FLOATS,
        "tonewheel-core",
    ),
    (
        UGenSpec::HurdyGurdyCore,
        hurdy_gurdy::STATE_FLOATS,
        "hurdy-gurdy-core",
    ),
    (UGenSpec::VosimCore, vosim::STATE_FLOATS, "vosim-core"),
    (UGenSpec::GendynCore, gendyn::STATE_FLOATS, "gendyn-core"),
    (UGenSpec::ScannedCore, scanned::STATE_FLOATS, "scanned-core"),
];

fn def(spec: UGenSpec) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![spec].into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

#[test]
fn fm1_voice_codec_tags_are_106_to_111() {
    for (index, (spec, _, name)) in VOICES.iter().enumerate() {
        let mut bytes = Vec::new();
        encode_inst(&def(spec.clone()), &mut bytes).expect("encode voice");
        assert_eq!(bytes.get(9), Some(&(106 + index as u8)), "{name} tag");
    }
}

#[test]
fn fm1_voice_native_and_browser_codec_parity() {
    for (spec, state_floats, name) in VOICES.iter() {
        let definition = def(spec.clone());
        let base = NativeRig::native();
        let mut env = base.engine.build_env();
        env.voice_mem = 24_000;
        let native_template = Template::from_inst(&definition, &env).expect("native template");
        assert_eq!(
            native_template.mem_total, *state_floats,
            "{name} memory contract"
        );

        let mut bytes = Vec::new();
        encode_inst(&definition, &mut bytes).expect("encode graph");
        let mut raw = RawGraph::boxed();
        let mut bus = BusTemplate::new();
        decode_graph(&bytes, &mut raw, &mut bus).expect("decode browser graph");
        let mut decoded = Template::boxed();
        decoded.build(&raw, &env).expect("build decoded template");
        assert_eq!(
            decoded.nodes(),
            native_template.nodes(),
            "{name} node parity"
        );

        for rate in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = rate;
                cfg.max_block = block;
                cfg.voice_seconds = 0.5;
                let mut native = NativeRig::native_with(cfg);
                native.install(&definition);
                let _ = native.step();
                native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
                let (left, _) = native.run(20);
                assert!(
                    left.iter().all(|sample| sample.is_finite()),
                    "{name} native {rate}/{block}"
                );
                assert!(rms(&left) > 1.0e-4, "{name} native audible {rate}/{block}");

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
                let (left, _) = browser.run(20);
                assert!(
                    left.iter().all(|sample| sample.is_finite()),
                    "{name} browser {rate}/{block}"
                );
                assert!(rms(&left) > 1.0e-4, "{name} browser audible {rate}/{block}");
            }
        }
    }
}
