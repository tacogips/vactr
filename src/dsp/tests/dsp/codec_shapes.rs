//! Instrument codec fixtures for output shapes and edge output indexes.

use crate::dsp::arena::{decode_graph, encode_bus, encode_inst, FaultCode, GraphKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::graph::{
    BankRef, BusDef, BusId, Edge, EffectKind, EffectSpec, InstDef, InstId, UGenSpec,
};
use crate::dsp::ugen::RawGraph;

fn edge(from: u16, to: u16, port: u8, output: u8) -> Edge {
    Edge {
        from,
        to,
        port,
        output,
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

fn mono() -> InstDef {
    def(
        vec![UGenSpec::Const(440.0), UGenSpec::SinOsc],
        vec![edge(0, 1, 0, 0)],
    )
}

fn multi_mono() -> InstDef {
    def(
        vec![UGenSpec::VaSource, UGenSpec::VaFilter, UGenSpec::Add],
        vec![edge(0, 1, 0, 0), edge(1, 2, 0, 0), edge(1, 2, 1, 1)],
    )
}

fn stereo() -> InstDef {
    def(
        vec![
            UGenSpec::SamplePlay(BankRef::new(7)),
            UGenSpec::Effect(EffectSpec {
                kind: EffectKind::Compressor,
                params: Box::new([]),
            }),
        ],
        vec![edge(0, 1, 0, 1)],
    )
}

fn assert_round_trip(def: &InstDef, fixture: &[u8]) {
    let mut encoded = Vec::new();
    encode_inst(def, &mut encoded).expect("valid fixture graph encodes");
    assert_eq!(encoded, fixture);

    let mut decoded = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    assert_eq!(
        decode_graph(fixture, &mut decoded, &mut bus),
        Ok(GraphKind::Inst)
    );
    let mut expected = RawGraph::boxed();
    expected.load(def).expect("fixture graph loads");
    assert_eq!(decoded.inst, expected.inst);
    assert_eq!(decoded.n_nodes, expected.n_nodes);
    assert_eq!(
        &decoded.nodes[..decoded.n_nodes],
        &expected.nodes[..expected.n_nodes]
    );
    assert_eq!(decoded.n_edges, expected.n_edges);
    assert_eq!(
        &decoded.edges[..decoded.n_edges],
        &expected.edges[..expected.n_edges]
    );
    assert_eq!(decoded.n_params, expected.n_params);
    assert_eq!(
        &decoded.params[..decoded.n_params],
        &expected.params[..expected.n_params]
    );
    assert_eq!(decoded.n_node_params, expected.n_node_params);
    assert_eq!(
        &decoded.node_params[..decoded.n_node_params],
        &expected.node_params[..expected.n_node_params]
    );

    let mut reencoded = Vec::new();
    encode_inst(def, &mut reencoded).expect("fixture graph re-encodes");
    assert_eq!(reencoded, fixture);
}

// Hand-authored from the pinned record layout; these are not encoder output.
const MONO_BYTES: &[u8] = &[
    b'V', 1, 0, 0, 0, 0, 0, 2, 0, // tag, id, params, node count
    16, 0, 0, 0xdc, 0x43, 0, // Const(440), mono shape
    0, 0, 0, 0, 0, 0, // SinOsc, mono shape
    1, 0, // edge count
    0, 0, 1, 0, 0, 0, // Const output 0 -> SinOsc
    0, 0, // node params
];

const MULTI_MONO_BYTES: &[u8] = &[
    b'V', 1, 0, 0, 0, 0, 0, 3, 0, // tag, id, params, node count
    37, 0, 0, 0, 0, 0, // VaSource, mono shape
    38, 0, 0, 0, 0, 1, // VaFilter, two mono outputs
    15, 0, 0, 0, 0, 0, // Add, mono shape
    3, 0, // edge count
    0, 0, 1, 0, 0, 0, // source -> filter, output 0
    1, 0, 2, 0, 0, 0, // filter output 0 -> Add port 0
    1, 0, 2, 0, 1, 1, // filter output 1 -> Add port 1
    0, 0, // node params
];

const STEREO_BYTES: &[u8] = &[
    b'V', 1, 0, 0, 0, 0, 0, 2, 0, // tag, id, params, node count
    13, 7, 0, 0, 0, 9, // SamplePlay(7), outputs mono and stereo
    30, 0, 0, 0, 4, // Compressor with no params, stereo output
    1, 0, // edge count
    0, 0, 1, 0, 0, 1, // SamplePlay output 1 -> stereo effect
    0, 0, // node params
];

#[test]
fn exact_mono_fixture_round_trips() {
    assert_round_trip(&mono(), MONO_BYTES);
}

#[test]
fn exact_multi_mono_fixture_round_trips() {
    assert_round_trip(&multi_mono(), MULTI_MONO_BYTES);
}

#[test]
fn exact_stereo_fixture_round_trips() {
    assert_round_trip(&stereo(), STEREO_BYTES);
}

fn rejects(bytes: &[u8]) {
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    assert_eq!(
        decode_graph(bytes, &mut raw, &mut bus),
        Err(FaultCode::BadRecord)
    );
}

#[test]
fn old_unknown_and_malformed_shape_or_output_records_are_rejected() {
    let mut old = MONO_BYTES.to_vec();
    old[0] = b'I';
    rejects(&old);

    let mut unknown = MONO_BYTES.to_vec();
    unknown[0] = b'?';
    rejects(&unknown);

    let mut reserved_shape = MONO_BYTES.to_vec();
    reserved_shape[14] |= 0x40;
    rejects(&reserved_shape);

    let mut mismatched_shape = MULTI_MONO_BYTES.to_vec();
    mismatched_shape[20] = 0;
    rejects(&mismatched_shape);

    let mut bad_output = MULTI_MONO_BYTES.to_vec();
    bad_output[46] = 2;
    rejects(&bad_output);
}

#[test]
fn stereo_output_cannot_feed_a_mono_node() {
    const STEREO_TO_LPF: &[u8] = &[
        b'V', 1, 0, 0, 0, 0, 0, 2, 0, // tag, id, params, node count
        13, 7, 0, 0, 0, 9, // SamplePlay(7), stereo output 1
        5, 0, 0, 0, 0, 0, // Lpf, mono shape
        1, 0, // edge count
        0, 0, 1, 0, 0, 1, // stereo output into mono node
        0, 0, // node params
    ];
    rejects(STEREO_TO_LPF);
}

#[test]
fn bus_and_master_records_keep_their_existing_layout() {
    let def = BusDef {
        id: BusId::new(1),
        chain: vec![EffectSpec {
            kind: EffectKind::Compressor,
            params: Box::new([]),
        }]
        .into_boxed_slice(),
    };
    const BUS_BYTES: &[u8] = &[b'B', 1, 0, 0, 0, 1, 0, 0, 0, 0];
    const MASTER_BYTES: &[u8] = &[b'M', 1, 0, 0, 0, 1, 0, 0, 0, 0];

    let mut bytes = Vec::new();
    encode_bus(&def, false, &mut bytes).expect("bus encodes");
    assert_eq!(bytes, BUS_BYTES);
    encode_bus(&def, true, &mut bytes).expect("master encodes");
    assert_eq!(bytes, MASTER_BYTES);
}
