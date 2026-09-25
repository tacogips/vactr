//! `for` to `map` with the result discarded.

use super::*;

#[test]
fn for_statement_row() {
    expands_as(
        "for x arr:\n\tprint x",
        "(match (map arr (-> (x) {(print x)})) {(-> (_) nil)})",
    );
}

#[test]
fn for_list_pattern_is_kept() {
    expands_as(
        "for [k v] d:\n\tprint k v",
        "(match (map d (-> ([k v]) {(print k v)})) {(-> (_) nil)})",
    );
}

#[test]
fn for_with_index() {
    expands_as(
        "for [i x] {enumerate arr}:\n\tprint i x",
        "(match (map {(enumerate arr)} (-> ([i x]) {(print i x)})) {(-> (_) nil)})",
    );
}

#[test]
fn for_over_a_range() {
    expands_as(
        "for i 0..8:\n\tprint {* i i}",
        "(match (map (.. 0 8) (-> (i) {(print {(* i i)})})) {(-> (_) nil)})",
    );
}

#[test]
fn map_is_a_builtin_head() {
    // Hygiene: a user binding named `map` cannot capture the desugared call.
    let (_, out) = expand_src("for x arr:\n\tprint x");
    let n = out.into_iter().next().expect("one form").expect("clean");
    let mapped = &n.children[1];
    assert!(matches!(
        &mapped.children[0].kind,
        crate::reader::NodeKind::Atom(crate::reader::Atom::Builtin(name)) if &**name == "map"
    ));
}
