//! Seed ordinals preserve kernel seeds when voice-level gates are appended.

use super::{caps, config, ctl, event, NativeRig};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::StoreKind;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ugen::{Node, RawGraph, Template};

fn edge(from: u16, to: u16, port: u8) -> Edge {
    Edge {
        from,
        to,
        port,
        output: 0,
    }
}

fn def(nodes: Vec<UGenSpec>, edges: Vec<Edge>) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: nodes.into_boxed_slice(),
        edges: edges.into_boxed_slice(),
        node_params: Box::new([]),
    }
}

fn gated_clock(gate_mode: f32) -> InstDef {
    let mut nodes = vec![
        UGenSpec::Param(ctl::FREQ),
        UGenSpec::ClockNoisePair,
        UGenSpec::Const(0.5),
        UGenSpec::Mul,
    ];
    let mut edges = vec![edge(0, 1, 0), edge(1, 3, 0), edge(2, 3, 1)];
    let values = [gate_mode, 0.5, 0.5, 0.0, 0.0, 0.0];
    let gate = u16::try_from(4 + values.len()).expect("small test graph index");
    for (offset, value) in values.into_iter().enumerate() {
        let raw = 4 + offset;
        nodes.push(UGenSpec::Const(value));
        edges.push(edge(
            u16::try_from(raw).expect("small test graph index"),
            gate,
            u8::try_from(offset + 3).expect("gate port fits"),
        ));
    }
    nodes.push(UGenSpec::VactrolGate);
    edges.push(edge(3, gate, 0));
    def(nodes, edges)
}

fn plain_clock() -> InstDef {
    def(
        vec![
            UGenSpec::Param(ctl::FREQ),
            UGenSpec::ClockNoisePair,
            UGenSpec::Const(0.5),
            UGenSpec::Mul,
        ],
        vec![edge(0, 1, 0), edge(1, 3, 0), edge(2, 3, 1)],
    )
}

fn template(definition: &InstDef) -> Box<Template> {
    Template::from_inst(
        definition,
        &crate::dsp::ugen::BuildEnv {
            sr: 48_000.0,
            caps: caps(),
            voice_mem: 24_000,
        },
    )
    .expect("template compiles")
}

fn kernel_index(template: &Template) -> usize {
    template
        .nodes()
        .iter()
        .position(|spec| matches!(spec.node, Node::ClockNoisePair))
        .expect("clock-noise kernel exists")
}

fn render(definition: &InstDef) -> Vec<f32> {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.max_block = 256;
    let mut rig = NativeRig::native_with(cfg);
    rig.install(definition);
    let _ = rig.step();
    rig.send(event(
        1,
        rig.engine.now(),
        &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
    ));
    rig.run(188).0
}

#[test]
fn gate_free_graph_uses_compiled_indices_as_ordinals() {
    let graph = def(
        vec![UGenSpec::WhiteNoise, UGenSpec::Const(0.5), UGenSpec::Mul],
        vec![edge(0, 2, 0), edge(1, 2, 1)],
    );
    let t = template(&graph);
    for i in 0..t.n_nodes {
        assert_eq!(usize::from(t.seed_ordinal(i)), i);
    }
}

#[test]
fn shifted_kernel_keeps_its_old_seed_and_off_render() {
    let plain = plain_clock();
    let gated = gated_clock(0.0);
    let plain_template = template(&plain);
    let gated_template = template(&gated);
    let plain_index = kernel_index(&plain_template);
    let gated_index = kernel_index(&gated_template);
    assert_ne!(
        plain_index, gated_index,
        "the gate inputs must shift Kahn order"
    );
    assert_eq!(
        gated_template.seed_ordinal(gated_index),
        u16::try_from(plain_index).expect("small graph index")
    );
    let expected = render(&plain);
    let actual = render(&gated);
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

#[test]
fn gate_only_source_elides_but_subject_source_is_retained() {
    let plain = def(vec![UGenSpec::WhiteNoise], vec![]);
    let gate_only_first = def(
        vec![
            UGenSpec::Const(0.0),
            UGenSpec::WhiteNoise,
            UGenSpec::VactrolGate,
        ],
        vec![edge(0, 2, 3), edge(1, 2, 0)],
    );
    let plain_t = template(&plain);
    let gated_t = template(&gate_only_first);
    let noise_index = gated_t
        .nodes()
        .iter()
        .position(|spec| matches!(spec.node, Node::WhiteNoise))
        .expect("noise exists");
    assert_eq!(noise_index, 1);
    assert_eq!(gated_t.seed_ordinal(noise_index), 0);
    assert_eq!(plain_t.seed_ordinal(0), 0);
    assert_eq!(render(&gate_only_first), render(&plain));

    let mut subject_nodes = vec![UGenSpec::WhiteNoise];
    let mut subject_edges = vec![edge(0, 7, 0)];
    for (offset, value) in [0.0, 0.5, 0.5, 0.0, 0.0, 0.0].into_iter().enumerate() {
        let raw = u16::try_from(subject_nodes.len()).expect("small test graph index");
        subject_nodes.push(UGenSpec::Const(value));
        subject_edges.push(edge(
            raw,
            7,
            u8::try_from(offset + 3).expect("gate port fits"),
        ));
    }
    subject_nodes.push(UGenSpec::VactrolGate);
    let subject_gate = def(subject_nodes, subject_edges);
    let subject_t = template(&subject_gate);
    assert_eq!(subject_t.seed_ordinal(0), plain_t.seed_ordinal(0));
    assert_eq!(render(&subject_gate), render(&plain));
}

#[test]
fn shared_frequency_and_gate_chains_preserve_kernel_ordinal() {
    let plain = template(&plain_clock());
    let mut shared = gated_clock(0.0);
    let gate_index = shared.nodes.len() - 1;
    let mut nodes = shared.nodes.into_vec();
    let mut edges = shared.edges.into_vec();
    // Reuse the frequency source at a gate port: this source is shared with K.
    let gate_index = u16::try_from(gate_index).expect("small graph index");
    edges.push(edge(0, gate_index, 1));
    // Add a second gate whose subject is the first gate's output.
    let second = u16::try_from(nodes.len()).expect("small graph index");
    nodes.push(UGenSpec::VactrolGate);
    edges.push(edge(gate_index, second, 0));
    shared = def(nodes, edges);
    let t = template(&shared);
    assert_eq!(
        t.seed_ordinal(kernel_index(&t)),
        plain.seed_ordinal(kernel_index(&plain))
    );
}

#[test]
fn in_place_rebuild_resets_gate_count_and_seed_identity() {
    let gated = gated_clock(0.0);
    let plain = plain_clock();
    let mut raw = RawGraph::boxed();
    raw.load(&gated).expect("gated graph loads");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: caps(),
        voice_mem: 24_000,
    };
    let mut t = Template::boxed();
    t.build(&raw, &env).expect("gated graph builds");
    assert!(t.gates > 0);
    raw.load(&plain).expect("plain graph loads");
    t.build(&raw, &env).expect("plain rebuild succeeds");
    assert_eq!(t.gates, 0);
    for i in 0..t.n_nodes {
        assert_eq!(usize::from(t.seed_ordinal(i)), i);
    }
}

#[test]
fn gated_template_build_allocates_nothing_in_place() {
    let definition = gated_clock(0.0);
    let mut raw = RawGraph::boxed();
    raw.load(&definition)
        .expect("graph loads before allocation probe");
    let env = crate::dsp::ugen::BuildEnv {
        sr: 48_000.0,
        caps: caps(),
        voice_mem: 24_000,
    };
    let mut t = Template::boxed();
    let (result, allocations) = armed(|| t.build(&raw, &env));
    result.expect("in-place build succeeds");
    assert_eq!(allocations, 0);
}
