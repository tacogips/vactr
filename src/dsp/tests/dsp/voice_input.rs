//! Validated host-input source nodes, wire tags and event-relative indexing.

use super::{caps, config, event, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_inst, StoreKind};
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{catalog, Node};
use crate::dsp::ugen::{BuildEnv, BuildError, Template};
use crate::host::wire::Ctl;

fn source_voice(quad: bool) -> InstDef {
    let mut nodes = vec![UGenSpec::HostInputL, UGenSpec::HostInputR, UGenSpec::AuxOut];
    let mut edges = vec![Edge {
        from: 1,
        to: 2,
        port: 0,
    }];
    if quad {
        nodes.extend([
            UGenSpec::HostInputL,
            UGenSpec::Out3,
            UGenSpec::HostInputR,
            UGenSpec::Out4,
        ]);
        edges.extend([
            Edge {
                from: 3,
                to: 4,
                port: 0,
            },
            Edge {
                from: 5,
                to: 6,
                port: 0,
            },
        ]);
    }
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        node_params: Box::new([]),
    }
}

#[test]
fn host_source_tags_align_at_mid_block_on_native_and_browser() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            for quad in [false, true] {
                let def = source_voice(quad);
                let mut bytes = Vec::new();
                encode_inst(&def, &mut bytes).unwrap();
                for browser in [false, true] {
                    let mut cfg = config(
                        &caps(),
                        if browser {
                            StoreKind::Arena { bytes: 4 << 20 }
                        } else {
                            StoreKind::NativeArc
                        },
                    );
                    cfg.sample_rate = rate;
                    cfg.max_block = block;
                    cfg.output_channels = if quad { 4 } else { 2 };
                    let mut native = NativeRig::native_with(cfg);
                    let mut web = BrowserRig::browser_with(cfg);
                    if browser {
                        let mut record = Vec::new();
                        encode_graph_record(1, 1, &bytes, &mut record);
                        web.push(&record);
                    } else {
                        native.install(&def);
                    }
                    let rig = if browser {
                        &mut web as &mut dyn SourceRig
                    } else {
                        &mut native as &mut dyn SourceRig
                    };
                    for _ in 0..6 {
                        let _ = rig.step();
                    }
                    assert_eq!(rig.fault(), None, "install {browser}/{quad}");
                    rig.send(event(1, rig.now() + 17.0 / f64::from(rate), &[]));
                    let mut input = vec![0.0; block * 2];
                    for (frame, pair) in input.chunks_exact_mut(2).enumerate() {
                        pair[0] = if frame % 2 == 0 { 0.15 } else { 0.65 };
                        pair[1] = if frame % 3 == 0 { 0.8 } else { 0.25 };
                    }
                    let output = rig.step_with_input(&input);
                    let channels = if quad { 4 } else { 2 };
                    for frame in 0..17 {
                        let out = &output[frame * channels..(frame + 1) * channels];
                        assert_eq!(out[0], input[2 * frame]);
                        assert_eq!(out[1], input[2 * frame + 1]);
                        if quad {
                            assert_eq!(&out[2..], &[0.0, 0.0]);
                        }
                    }
                    let a = &output[17 * channels..18 * channels];
                    let b = &output[18 * channels..19 * channels];
                    let gain_l_a = (a[0] - input[34]) / input[34];
                    let gain_l_b = (b[0] - input[36]) / input[36];
                    assert!(
                        gain_l_a > 0.1,
                        "{rate}/{block}/{browser}/{quad}: {gain_l_a}"
                    );
                    assert!((gain_l_a - gain_l_b).abs() < 1.0e-4);
                    let gain_r_a = (a[1] - input[35]) / input[35];
                    let gain_r_b = (b[1] - input[37]) / input[37];
                    assert!((gain_r_a - gain_r_b).abs() < 1.0e-4);
                    if quad {
                        assert!((a[2] / input[34] - b[2] / input[36]).abs() < 1.0e-4);
                        assert!((a[3] / input[35] - b[3] / input[37]).abs() < 1.0e-4);
                    }
                    assert!(rig.step().iter().all(|sample| *sample == 0.0));
                    assert!(rig
                        .step_with_input(&vec![f32::NAN; 2 * block])
                        .iter()
                        .all(|sample| *sample == 0.0));
                    assert_eq!(rig.fault(), Some(crate::dsp::arena::FaultCode::InputBuffer));
                }
            }
        }
    }
}

trait SourceRig {
    fn step(&mut self) -> &[f32];
    fn step_with_input(&mut self, input: &[f32]) -> &[f32];
    fn send(&mut self, event: crate::host::wire::AudioEvent);
    fn now(&self) -> f64;
    fn fault(&mut self) -> Option<crate::dsp::arena::FaultCode>;
}

impl<C: super::CellStore, S: super::ControlSource> SourceRig for super::Rig<C, S> {
    fn step(&mut self) -> &[f32] {
        self.step()
    }
    fn step_with_input(&mut self, input: &[f32]) -> &[f32] {
        self.step_with_input(input)
    }
    fn send(&mut self, event: crate::host::wire::AudioEvent) {
        self.send(event);
    }
    fn now(&self) -> f64 {
        self.engine.now()
    }
    fn fault(&mut self) -> Option<crate::dsp::arena::FaultCode> {
        self.engine.pop_fault().map(|f| f.code)
    }
}

#[test]
fn elements_last_external_port_is_encoded_and_next_port_rejects() {
    let def = InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::HostInputR, UGenSpec::ElementsInternal].into_boxed_slice(),
        edges: vec![Edge {
            from: 0,
            to: 1,
            port: 29,
        }]
        .into_boxed_slice(),
        node_params: Box::new([]),
    };
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = crate::dsp::ugen::RawGraph::boxed();
    crate::dsp::arena::decode_graph(&bytes, &mut raw, &mut crate::dsp::bus::BusTemplate::new())
        .unwrap();
    assert_eq!(raw.edges[0].port, 29);
    let env = BuildEnv {
        sr: 48_000.0,
        caps: caps(),
        voice_mem: 20_000,
    };
    assert!(Template::from_inst(&def, &env).is_ok());
    let mut invalid = def;
    invalid.edges = vec![Edge {
        from: 0,
        to: 1,
        port: 30,
    }]
    .into_boxed_slice();
    assert!(matches!(
        Template::from_inst(&invalid, &env),
        Err(BuildError::BadEdge)
    ));
}

fn resonator_input_voice(elements: bool) -> InstDef {
    if elements {
        let node = Node::ElementsInternal;
        InstDef {
            id: InstId::new(1),
            params: Box::new([]),
            nodes: vec![
                UGenSpec::HostInputL,
                UGenSpec::HostInputR,
                UGenSpec::ElementsInternal,
                UGenSpec::ElementsInternal,
                UGenSpec::AuxOut,
            ]
            .into_boxed_slice(),
            edges: vec![
                Edge {
                    from: 0,
                    to: 2,
                    port: 28,
                },
                Edge {
                    from: 1,
                    to: 2,
                    port: 29,
                },
                Edge {
                    from: 0,
                    to: 3,
                    port: 28,
                },
                Edge {
                    from: 1,
                    to: 3,
                    port: 29,
                },
                Edge {
                    from: 3,
                    to: 4,
                    port: 0,
                },
            ]
            .into_boxed_slice(),
            node_params: vec![
                (2, catalog::port_ctl(&node, 4).unwrap(), Ctl::Const(1.0)),
                (3, catalog::port_ctl(&node, 4).unwrap(), Ctl::Const(1.0)),
                (2, catalog::port_ctl(&node, 7).unwrap(), Ctl::Const(1.0)),
                (3, catalog::port_ctl(&node, 7).unwrap(), Ctl::Const(1.0)),
                (3, catalog::port_ctl(&node, 26).unwrap(), Ctl::Const(1.0)),
            ]
            .into_boxed_slice(),
        }
    } else {
        let node = Node::RingsPart;
        InstDef {
            id: InstId::new(1),
            params: Box::new([]),
            nodes: vec![
                UGenSpec::HostInputL,
                UGenSpec::RingsPart,
                UGenSpec::RingsPart,
                UGenSpec::AuxOut,
            ]
            .into_boxed_slice(),
            edges: vec![
                Edge {
                    from: 0,
                    to: 1,
                    port: 16,
                },
                Edge {
                    from: 0,
                    to: 2,
                    port: 16,
                },
                Edge {
                    from: 2,
                    to: 3,
                    port: 0,
                },
            ]
            .into_boxed_slice(),
            node_params: vec![
                (1, catalog::port_ctl(&node, 7).unwrap(), Ctl::Const(0.0)),
                (2, catalog::port_ctl(&node, 7).unwrap(), Ctl::Const(0.0)),
                (2, catalog::port_ctl(&node, 15).unwrap(), Ctl::Const(1.0)),
            ]
            .into_boxed_slice(),
        }
    }
}

#[test]
fn rings_and_elements_external_ports_render_across_rates_and_blocks() {
    for elements in [false, true] {
        let def = resonator_input_voice(elements);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut record = Vec::new();
        encode_graph_record(1, 1, &bytes, &mut record);
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                for browser in [false, true] {
                    let mut cfg = config(
                        &caps(),
                        if browser {
                            StoreKind::Arena { bytes: 4 << 20 }
                        } else {
                            StoreKind::NativeArc
                        },
                    );
                    cfg.sample_rate = rate;
                    cfg.max_block = block;
                    cfg.voice_seconds = 0.5;
                    let mut native = NativeRig::native_with(cfg);
                    let mut web = BrowserRig::browser_with(cfg);
                    let rig = if browser {
                        web.push(&record);
                        &mut web as &mut dyn SourceRig
                    } else {
                        native.install(&def);
                        &mut native as &mut dyn SourceRig
                    };
                    for _ in 0..6 {
                        let _ = rig.step();
                    }
                    assert_eq!(rig.fault(), None);
                    rig.send(event(1, rig.now(), &[]));
                    let mut input = vec![0.0; 2 * block];
                    input[if elements { 1 } else { 0 }] = 0.8;
                    let first = rig.step_with_input(&input).to_vec();
                    let mut tail = 0.0;
                    for _ in 0..4 {
                        tail += rig.step().iter().map(|sample| sample.abs()).sum::<f32>();
                    }
                    assert!(first.iter().all(|sample| sample.is_finite()));
                    assert!(tail > 1.0e-4, "{elements}/{browser}/{rate}/{block}: {tail}");
                    assert_eq!(rig.fault(), None);
                }
            }
        }
    }
}
