//! INTEGRATE: TASK-006 criteria that need the VM (design 10.1, 11.7, 13).
//!
//! The tweaked-probability cases assert the RE-QUERY semantics: a pattern
//! whose probability is a tweak site reads the site per query, so after a
//! controller write the next query of the SAME bound pattern gains or loses
//! events. Invalidating already-staged, uncommitted events (11.3) is
//! TASK-007's scheduler and is not exercised here.

use crate::ns::stage::SlotKey;
use crate::types::diag::DiagCode;
use crate::value::value::Value;
use crate::vm::tests::integrate_query::Ev;

/// Writes the only tweak site of the form that binds `name`.
fn tweak(h: &mut Ev, name: &str, v: Value) {
    let sites = h.ev.sites_of(name);
    assert_eq!(sites.len(), 1, "{sites:?}");
    let site = &sites[0];
    h.ev.set_tweak(site.id, site.form_gen, v)
        .expect("a live site");
}

fn count(h: &mut Ev, p: &Value) -> usize {
    (0..4).map(|c| h.query(p, c).events.len()).sum()
}

#[test]
fn maybe_zero_to_one_restores_absent_events() {
    let mut h = Ev::new();
    h.run("let p maybe {s [:bd :sd :hh :cp]} 0");
    let p = h.get("p");
    assert_eq!(count(&mut h, &p), 0);
    tweak(&mut h, "p", Value::Int(1));
    assert_eq!(count(&mut h, &p), 16);
    tweak(&mut h, "p", Value::Int(0));
    assert_eq!(count(&mut h, &p), 0);
}

#[test]
fn degrade_by_zero_to_one_removes_events() {
    let mut h = Ev::new();
    h.run("let p degrade-by {s [:bd :sd :hh :cp]} 0");
    let p = h.get("p");
    assert_eq!(count(&mut h, &p), 16);
    tweak(&mut h, "p", Value::Int(1));
    assert_eq!(count(&mut h, &p), 0);
    tweak(&mut h, "p", Value::Int(0));
    assert_eq!(count(&mut h, &p), 16);
}

#[test]
fn a_retiming_operator_on_live_input_is_rejected_at_bind() {
    let mut h = Ev::new();
    let ok = h.run("s :pluck > midi-notes channel: 1 > d1");
    assert!(ok[0].value.is_ok(), "{:?}", ok[0].value);
    assert!(Ev::codes(&ok).is_empty(), "{:?}", ok[0].diags);
    let before = h.bound(SlotKey::D(1)).expect("d1");
    let binds = h.bind_count();
    let owner = h.ev.form_of_bind(SlotKey::D(1));
    assert!(owner.is_some());
    let out = h.run("s :pluck > midi-notes channel: 1 > fast 2 > d1");
    assert!(out[0].value.is_err());
    assert!(
        out[0]
            .diags
            .iter()
            .any(|d| d.code == DiagCode::InputLaneOperator),
        "{:?}",
        out[0].diags
    );
    assert_eq!(h.bind_count(), binds, "nothing staged");
    let after = h.bound(SlotKey::D(1)).expect("d1");
    let (Value::Pattern(a), Value::Pattern(b)) = (&before, &after) else {
        panic!("patterns");
    };
    assert!(std::rc::Rc::ptr_eq(a, b), "the previous d1 binding stays");
    assert_eq!(
        h.ev.form_of_bind(SlotKey::D(1)),
        owner,
        "the owner is unchanged"
    );
}

#[test]
fn per_note_operators_on_live_input_bind() {
    let mut h = Ev::new();
    let out = h.run("s :pluck > midi-notes channel: 1 > gain 0.5 > scale :c :minor > d1");
    assert!(out[0].value.is_ok(), "{:?}", out[0].value);
    let p = h.bound(SlotKey::D(1)).expect("d1");
    // Live input has no events under `query`.
    assert!(h.query(&p, 0).events.is_empty());
}

#[test]
fn a_first_list_step_gives_structure_to_one_sound() {
    let mut h = Ev::new();
    let p = h.last("s :bd > n [0 3]").expect("pattern");
    assert_eq!(h.values(&p, 0).len(), 2);
    let one = h.last("s :bd").expect("pattern");
    assert_eq!(h.values(&one, 0).len(), 1);
    let steps = h.last("s [:bd :sd :hh] > gain [1 0.5]").expect("pattern");
    assert_eq!(h.values(&steps, 0).len(), 3, "later lists are sampled");
}

#[test]
fn transforms_run_through_the_vm() {
    let mut h = Ev::new();
    let p = h
        .last("s [:bd :sd] > every 2 {p -> fast p 2} > whenmod 8 6 rev")
        .expect("pattern");
    assert_eq!(h.values(&p, 0).len(), 4);
    assert_eq!(h.values(&p, 1).len(), 2);
    let r = h.query(&p, 6);
    assert!(r.faults.is_empty(), "{:?}", r.faults);
}

#[test]
fn a_var_list_inside_a_list_or_cat_is_a_nested_step_list() {
    // Regression (2026-09-27): a late-bound `var` used as a step, or as an
    // element of `cat`, was one event carrying the whole list instead of a
    // subdivision.
    let mut h = Ev::new();
    h.run("var bar [:c5 :d5 :e5 :f5]");
    let two = h.last("s :fm > note [bar bar]").expect("pattern");
    assert_eq!(h.values(&two, 0).len(), 8);
    h.run("let seq cat [bar [:g5 :a5]]");
    let played = h.last("s :fm > note seq").expect("pattern");
    assert_eq!(
        h.values(&played, 0).len(),
        4,
        "cycle 0 plays the var's four notes"
    );
    assert_eq!(
        h.values(&played, 1).len(),
        2,
        "cycle 1 plays the literal pair"
    );
    let r = h.query(&played, 0);
    assert!(r.faults.is_empty(), "{:?}", r.faults);
}
