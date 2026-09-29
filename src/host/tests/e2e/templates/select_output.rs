//! MOD004-20 source-form output selection and definition-time lowering tests.

use crate::dsp::graph::UGenSpec;
use crate::types::diag::DiagCode;
use crate::value::intern::name_of_kw;
use crate::vm::fail::FailCode;

use super::super::{E2e, SOURCE};

const SEL_A: &str = "inst sel-a:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p :main} > * amp > + {{p :aux} > * amp > aux-out}";

fn failed_definition(e: &mut E2e, source: &str) -> (FailCode, String, Vec<DiagCode>) {
    let outcomes = e.ev.eval_str(source, SOURCE).expect("source parses");
    let outcome = outcomes.last().expect("one form");
    let failure = outcome.value.as_ref().expect_err("definition fails");
    (
        failure.code,
        failure.message.clone(),
        outcome.diags.iter().map(|diag| diag.code).collect(),
    )
}

fn entry(e: &E2e, name: &str) -> crate::ns::insts::InstEntry {
    e.reg
        .borrow()
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *name)
        .expect("instrument definition is registered")
        .clone()
}

#[test]
fn keyword_and_index_selection_share_one_node_and_unselected_means_zero() {
    let mut e = E2e::new();
    e.eval(SEL_A);
    let selected = entry(&e, "sel-a");
    let filter = selected
        .def
        .nodes
        .iter()
        .position(|node| matches!(node, UGenSpec::VaFilter))
        .expect("one VaFilter");
    assert_eq!(
        selected
            .def
            .nodes
            .iter()
            .filter(|node| matches!(node, UGenSpec::VaFilter))
            .count(),
        1
    );
    assert_eq!(
        selected
            .def
            .nodes
            .iter()
            .filter(|node| matches!(node, UGenSpec::VaSource))
            .count(),
        1
    );
    let mut outputs: Vec<u8> = selected
        .def
        .edges
        .iter()
        .filter(|edge| usize::from(edge.from) == filter)
        .map(|edge| edge.output)
        .collect();
    outputs.sort_unstable();
    assert_eq!(outputs, [0, 1]);

    e.eval("inst sel-index:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p 0} > * amp > + {{p 1} > * amp > aux-out}");
    let indexed = entry(&e, "sel-index");
    assert_eq!(selected.def.nodes, indexed.def.nodes);
    assert_eq!(selected.def.edges, indexed.def.edges);
    assert_eq!(selected.def.node_params, indexed.def.node_params);
    assert_eq!(selected.def.params, indexed.def.params);

    e.eval("inst sel-default:\n\tlet p {va-source freq > va-filter freq: freq}\n\tp > * amp");
    let unselected = entry(&e, "sel-default");
    let filter = unselected
        .def
        .nodes
        .iter()
        .position(|node| matches!(node, UGenSpec::VaFilter))
        .expect("VaFilter");
    assert!(unselected.def.edges.iter().all(|edge| edge.output == 0));
    assert!(unselected
        .def
        .edges
        .iter()
        .filter(|edge| usize::from(edge.from) == filter)
        .all(|edge| edge.output == 0));
}

#[test]
fn invalid_selections_fail_at_instrument_definition() {
    let mut e = E2e::new();
    let cases = [
        ("inst bad-left:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p :left}", FailCode::UnknownField, "unknown output :left; declared outputs: :main, :aux"),
        ("inst bad-index:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p 2}", FailCode::UnknownField, "unknown output 2"),
        ("inst bad-type:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p 1.5}", FailCode::Type, "select an output with a keyword or an int"),
        ("inst bad-arity:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p :main :aux}", FailCode::Arity, "one output name or index"),
        ("inst bad-single:\n\tlet o {sin-osc freq}\n\t{o :main}", FailCode::NotCallable, "a unit generator is not callable"),
        ("inst bad-reselect:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{{p :aux} :main}", FailCode::NotCallable, "a unit generator is not callable"),
    ];
    for (source, expected_code, expected_message) in cases {
        let (code, message, _) = failed_definition(&mut e, source);
        assert_eq!(code, expected_code, "{source}");
        assert!(message.contains(expected_message), "{source}: {message}");
    }
}

#[test]
fn selected_root_and_stereo_layout_are_checked_at_definition() {
    let mut e = E2e::new();
    e.eval("inst sel-root:\n\tlet p {va-source freq > va-filter freq: freq}\n\t{p :aux}");
    let root = entry(&e, "sel-root");
    let last = u16::try_from(root.def.nodes.len() - 1).expect("node index fits");
    assert!(matches!(root.def.nodes.last(), Some(UGenSpec::Add)));
    assert!(root
        .def
        .edges
        .iter()
        .any(|edge| edge.to == last && edge.port == 0 && edge.output == 1));
    assert!(!root
        .def
        .edges
        .iter()
        .any(|edge| edge.to == last && edge.port == 1));

    e.eval(
        "inst stereo-root bank: keyword = :bd:\n\tlet s {sample-play bank}\n\t{s :stereo} > gain 0",
    );
    let stereo = entry(&e, "stereo-root");
    assert!(matches!(stereo.def.nodes.last(), Some(UGenSpec::Effect(_))));

    let (code, message, _) = failed_definition(
        &mut e,
        "inst stereo-mono bank: keyword = :bd:\n\tlet s {sample-play bank}\n\t{s :stereo} > + {sin-osc freq}",
    );
    assert_eq!(code, FailCode::InstFailed);
    assert!(
        message.contains("stereo output connected to mono-only input"),
        "{message}"
    );

    let (code, message, _) = failed_definition(
        &mut e,
        "inst stereo-aux bank: keyword = :bd:\n\tlet s {sample-play bank}\n\t{s :stereo} > + {sin-osc freq > aux-out}",
    );
    assert_eq!(code, FailCode::InstFailed);
    assert!(
        message.contains("stereo voice cannot also use aux-out"),
        "{message}"
    );
}

fn sample_sum(name: &str, count: usize) -> String {
    let mut source = format!("inst {name} bank: keyword = :bd:\n");
    for index in 0..count {
        source.push_str(&format!("\tlet s{index} {{sample-play bank}}\n"));
    }
    let mut sum = "{s0 :stereo}".to_owned();
    for index in 1..count {
        sum = format!("{sum} > + {{s{index} :stereo}}");
    }
    source.push_str(&format!("\t{sum}"));
    source
}

#[test]
fn channel_buffer_cap_is_a_definition_time_graph_too_large_error() {
    let mut e = E2e::new();
    let source = sample_sum("too-many-slices", 103);
    let (code, message, diagnostics) = failed_definition(&mut e, &source);
    assert_eq!(code, FailCode::InstFailed);
    assert!(message.contains("512"), "{message}");
    assert!(
        diagnostics.contains(&DiagCode::GraphTooLarge),
        "{diagnostics:?}"
    );

    let source = sample_sum("fits-slices", 102);
    let outcomes = e.ev.eval_str(&source, SOURCE).expect("source parses");
    assert!(
        outcomes.last().is_some_and(|outcome| outcome.value.is_ok()),
        "{outcomes:?}"
    );
    assert!(entry(&e, "fits-slices").def.nodes.len() < 256);
}
