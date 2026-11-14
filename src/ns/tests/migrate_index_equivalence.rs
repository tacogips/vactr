//! Differential checks for the namespace lookup indexes added in session 349.

use super::reactive_basic::{Harness, FILE};
use crate::ns::stage::SlotKey;
use crate::reader::span::Span;
use crate::value::value::Value;

fn linear_current_gen_at_span(h: &Harness, span: Span) -> Option<crate::ns::namespace::FormGen> {
    h.ev.graph().ids().find_map(|id| {
        let previous = h.ev.graph().get(id)?;
        (h.ev.graph().is_current(id) && previous.node.span == span).then_some(previous.gen)
    })
}

fn assert_index_matches_linear_scan(h: &Harness) {
    let spans =
        h.ev.graph()
            .ids()
            .filter_map(|id| h.ev.graph().get(id).map(|record| record.node.span))
            .collect::<Vec<_>>();
    for span in spans {
        assert_eq!(
            h.ev.graph().current_gen_at_span(span),
            linear_current_gen_at_span(h, span),
            "lookup mismatch at {span:?}"
        );
    }
}

#[test]
fn indexed_lookups_match_linear_reference_across_evaluation_sequences() {
    let mut h = Harness::new();

    for source in ["let value0 0.5", "let value1 0.5", "let value2 0.5"] {
        assert!(h.run(source)[0].value.is_ok());
        assert_index_matches_linear_scan(&h);
    }
    assert!(h.run("let value1 0.5")[0].value.is_ok());
    assert_index_matches_linear_scan(&h);

    let mut redefinitions = Harness::new();
    assert!(redefinitions.run("let a 1")[0].value.is_ok());
    assert_index_matches_linear_scan(&redefinitions);
    assert!(redefinitions.run("\nlet a 2")[0].value.is_ok());
    assert_index_matches_linear_scan(&redefinitions);
    let superseded = redefinitions
        .ev
        .graph()
        .ids()
        .next()
        .expect("first definition is recorded");
    assert!(!redefinitions.ev.graph().is_current(superseded));

    let empty_write = "{+ 1 2}";
    assert_eq!(h.show(empty_write), "3");
    assert_index_matches_linear_scan(&h);
    assert_eq!(h.show(empty_write), "3");
    assert_index_matches_linear_scan(&h);
    let empty_span = Span::new(
        FILE,
        0,
        u32::try_from(empty_write.len()).expect("short source"),
    );
    assert_eq!(h.ev.graph().current_gen_at_span(empty_span), None);
    assert_eq!(linear_current_gen_at_span(&h, empty_span), None);

    let retry = "let retryvalue latevalue";
    assert!(h.run(retry)[0].value.is_err());
    assert_index_matches_linear_scan(&h);
    assert!(h.run("let latevalue 1")[0].value.is_ok());
    assert_index_matches_linear_scan(&h);
    assert!(h.run(retry)[0].value.is_ok());
    assert_index_matches_linear_scan(&h);

    let source = "d1 {late-probe {* 0.5 2}}";
    h.ok(source);
    assert_index_matches_linear_scan(&h);
    let form = h.ev.form_of_bind(SlotKey::D(1)).expect("bound form");
    let initial_gen = h.ev.graph().get(form).expect("recorded form").gen;
    let initial_site = h.ev.ns().tweaks().borrow().sites_of(initial_gen)[0].clone();
    h.ev.set_tweak(initial_site.id, initial_gen, Value::Float(0.25))
        .expect("set-tweak");
    assert_index_matches_linear_scan(&h);
    let override_form = h.ev.form_of_bind(SlotKey::D(1)).expect("bound form");
    let override_gen = h.ev.graph().get(override_form).expect("recorded form").gen;
    let override_site = h.ev.ns().tweaks().borrow().sites_of(override_gen)[0].clone();

    h.ok(source);
    assert_index_matches_linear_scan(&h);
    let reevaluated_form = h.ev.form_of_bind(SlotKey::D(1)).expect("bound form");
    let reevaluated_gen =
        h.ev.graph()
            .get(reevaluated_form)
            .expect("recorded form")
            .gen;
    let reevaluated_site = h.ev.ns().tweaks().borrow().sites_of(reevaluated_gen)[0].clone();
    assert!(h
        .ev
        .take_migrations()
        .contains(&(override_site.id, reevaluated_site.id)));
}
