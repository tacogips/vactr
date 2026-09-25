//! `if`, binding `if` and `if`/`elif`/`else` to `match` (canonical rows 1-6).

use super::*;

#[test]
fn if_value_row() {
    expands_as(
        "if {> a 10} \"big\" \"small\"",
        "(match {(> a 10)} {(-> (false | nil) \"small\") (-> (_) \"big\")})",
    );
}

#[test]
fn if_without_else_in_let_gives_nil() {
    expands_as(
        "let maybe if {> a 100} \"huge\"",
        "(let maybe (match {(> a 100)} {(-> (false | nil) nil) (-> (_) \"huge\")}))",
    );
}

#[test]
fn if_c_x_without_else_gives_nil() {
    expands_as("if c x", "(match c {(-> (false | nil) nil) (-> (_) x)})");
}

#[test]
fn if_inside_let() {
    expands_as(
        "let label if {> a 10} \"big\" \"small\"",
        "(let label (match {(> a 10)} {(-> (false | nil) \"small\") (-> (_) \"big\")}))",
    );
}

#[test]
fn binding_if_inline_row() {
    expands_as(
        "if {d :gain} g -> once {s :bd} gain: g",
        "(match {(d :gain)} {(-> (false | nil) nil) (-> (g) (once {(s :bd)} [:gain g]))})",
    );
}

#[test]
fn binding_if_block_row() {
    expands_as(
        "if {d :gain} g ->:\n\tonce {s :bd} gain: g\nelse:\n\tonce {s :bd}",
        "(match {(d :gain)} {(-> (false | nil) {(once {(s :bd)})}) \
         (-> (g) {(once {(s :bd)} [:gain g])})})",
    );
}

#[test]
fn binding_if_variant_row() {
    expands_as(
        "if {parse s} ok v ->:\n\tprint v\nelse:\n\tprint \"parse failed\"",
        "(match {(parse s)} {(-> (ok v) {(print v)}) (-> (_) {(print \"parse failed\")})})",
    );
}

#[test]
fn binding_if_wildcard_uses_the_falsy_form() {
    expands_as(
        "if {d :gain} _ -> 1",
        "(match {(d :gain)} {(-> (false | nil) nil) (-> (_) 1)})",
    );
}

#[test]
fn bare_variant_binding_if_keeps_the_syntactic_form() {
    // U5: the checker (TASK-004) diagnoses a field-less variant here.
    expands_as(
        "if x none -> 1",
        "(match x {(-> (false | nil) nil) (-> (none) 1)})",
    );
}

#[test]
fn if_elif_else_row() {
    expands_as(
        "if {> a 10}:\n\tprint \"big\"\nelif {> a 5}:\n\tprint \"medium\"\nelse:\n\tprint \"small\"",
        "(match {(> a 10)} {(-> (false | nil) (match {(> a 5)} {(-> (false | nil) \
         {(print \"small\")}) (-> (_) {(print \"medium\")})})) (-> (_) {(print \"big\")})})",
    );
}

#[test]
fn if_elif_without_else_ends_in_nil() {
    expands_as(
        "if a:\n\tx\nelif b:\n\ty",
        "(match a {(-> (false | nil) (match b {(-> (false | nil) nil) (-> (_) {y})})) \
         (-> (_) {x})})",
    );
}

#[test]
fn elif_binding_form() {
    expands_as(
        "if a:\n\tx\nelif {parse s} ok v ->:\n\tprint v\nelse:\n\ty",
        "(match a {(-> (false | nil) (match {(parse s)} {(-> (ok v) {(print v)}) \
         (-> (_) {y})})) (-> (_) {x})})",
    );
}

#[test]
fn chain_inside_a_fn_body() {
    expands_as(
        "fn flatten-notes tree:\n\tif {is-list tree}:\n\t\tmap tree flatten-notes\n\t\t\t> join\n\telse:\n\t\t[tree]",
        "(fn flatten-notes tree {(match {(is-list tree)} {(-> (false | nil) {[tree]}) \
         (-> (_) {(join (map tree flatten-notes))})})})",
    );
}

#[test]
fn chain_match_carries_the_chain_span() {
    let src = "if a:\n\tx\nelse:\n\ty";
    let (r, out) = expand_src(src);
    let n = out.into_iter().next().expect("one form").expect("clean");
    assert_eq!(n.span, r.nodes[0].span);
}
