//! Versioned immutable Frames payload across native and browser graph paths.

use super::{caps, config, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, FaultCode, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, frame_keyframe::FrameData, BuildEnv, Node, RawGraph, Template};
use crate::host::wire::Ctl;

fn voice() -> InstDef {
    let data = FrameData::from_flat(&[0.0, 0.1, 0.8, 0.2, 0.4, 1.0, 0.9, 0.2, 0.7, 0.3]).unwrap();
    let node = Node::FrameKeyframe { slot: 0 };
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::FrameKeyframe {
                data: Some(Box::new(data)),
            },
            UGenSpec::FrameKeyframe {
                data: Some(Box::new(data)),
            },
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
        node_params: vec![
            (0, catalog::port_ctl(&node, 0).unwrap(), Ctl::Const(0.3)),
            (1, catalog::port_ctl(&node, 0).unwrap(), Ctl::Const(0.3)),
            (1, catalog::port_ctl(&node, 9).unwrap(), Ctl::Const(1.0)),
        ]
        .into_boxed_slice(),
    }
}

#[test]
fn versioned_payload_codec_and_capacity() {
    let def = voice();
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    assert_eq!(raw.n_frame_payloads, 2);
    assert_eq!(raw.frame_payloads[0].len, 2);
    let env = BuildEnv {
        sr: 96_000.0,
        caps: caps(),
        voice_mem: 0,
    };
    let built = Template::from_inst(&def, &env).unwrap();
    assert_eq!(built.mem_total, 0);
    assert_eq!(built.n_frame_payloads, 2);

    let mut fifth = def.clone();
    fifth.nodes = vec![def.nodes[0].clone(); 5].into_boxed_slice();
    fifth.edges = Box::new([]);
    fifth.node_params = Box::new([]);
    assert!(Template::from_inst(&fifth, &env).is_err());

    let first_tag = bytes.windows(2).position(|w| w == [85, 1]).unwrap();
    bytes[first_tag + 1] = 2;
    assert_eq!(
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()),
        Err(FaultCode::BadRecord)
    );
}

#[test]
fn native_browser_rate_block_matrix_is_finite_and_distinct() {
    let def = voice();
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut native = NativeRig::native_with(cfg);
            native.install(&def);
            let _ = native.step();
            native.send(event(1, native.engine.now(), &[]));
            let (main, aux) = native.run(8);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 0.01 && rms(&aux) > 0.01 && main != aux);

            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(1, browser.engine.now(), &[]));
            let (main, aux) = browser.run(8);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 0.01 && rms(&aux) > 0.01 && main != aux);
        }
    }
}
