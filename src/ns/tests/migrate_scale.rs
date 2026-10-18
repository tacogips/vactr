//! Migration lookup indexing and scaling regressions.

use super::reactive_basic::Harness;
use crate::ns::stage::SlotKey;
use crate::value::value::Value;

fn independent_tweak_forms(count: usize) -> String {
    (0..count)
        .map(|index| format!("let value{index} 0.5"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn migration_lookup_work_scales_with_forms_and_local_tweak_sites() {
    const N: usize = 48;
    let mut n = Harness::new();
    let outcomes = n.run(&independent_tweak_forms(N));
    assert_eq!(outcomes.len(), N);
    assert!(outcomes.iter().all(|outcome| outcome.value.is_ok()));
    assert_eq!(n.ev.ns().tweaks().borrow().iter().count(), N);
    let n_work = n.ev.migration_lookup_work();

    let mut four_n = Harness::new();
    let outcomes = four_n.run(&independent_tweak_forms(N * 4));
    assert_eq!(outcomes.len(), N * 4);
    assert!(outcomes.iter().all(|outcome| outcome.value.is_ok()));
    assert_eq!(four_n.ev.ns().tweaks().borrow().iter().count(), N * 4);
    let four_n_work = four_n.ev.migration_lookup_work();

    assert!(n_work <= N * 4, "N work {n_work} exceeds linear bound");
    assert!(
        four_n_work <= (n_work * 9) / 2,
        "work grew faster than 4.5x: N={n_work}, 4N={four_n_work}"
    );
    assert!(
        four_n_work <= N * 4 * 4,
        "4N work {four_n_work} is not linear"
    );
}

#[test]
fn explicit_same_span_reevaluation_inherits_prior_tweak_override() {
    let mut h = Harness::new();
    let source = "d1 {late-probe {* 0.5 2}}";
    h.ok(source);
    let form = h.ev.form_of_bind(SlotKey::D(1)).expect("bound form");
    let initial_gen =
        h.ev.graph()
            .get(form)
            .map(|record| record.gen)
            .expect("gen");
    let initial_site = h.ev.ns().tweaks().borrow().sites_of(initial_gen)[0].clone();

    h.ev.set_tweak(initial_site.id, initial_gen, Value::Float(0.25))
        .expect("set-tweak");
    let override_gen =
        h.ev.graph()
            .get(form)
            .map(|record| record.gen)
            .expect("gen");
    assert_ne!(override_gen, initial_gen);
    let override_site = h.ev.ns().tweaks().borrow().sites_of(override_gen)[0].clone();
    assert_eq!(override_site.slot.get().to_string(), "0.25");

    h.ok(source);
    let reevaluated_form = h.ev.form_of_bind(SlotKey::D(1)).expect("reevaluated form");
    let reevaluated_gen =
        h.ev.graph()
            .get(reevaluated_form)
            .map(|record| record.gen)
            .expect("gen");
    assert_ne!(reevaluated_gen, override_gen);
    let reevaluated_site = h.ev.ns().tweaks().borrow().sites_of(reevaluated_gen)[0].clone();
    assert_eq!(reevaluated_site.slot.get().to_string(), "0.25");
    assert!(h
        .ev
        .take_migrations()
        .contains(&(override_site.id, reevaluated_site.id)));
}
