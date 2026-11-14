//! `?`, `-x`, interpolation, and pairs and splats passing through.

use super::*;

#[test]
fn fallback_row_binds_tighter_than_pipe() {
    expands_as("d :gain ? 1.0 > * 2", "(* (or (d :gain) 1.0) 2)");
}

#[test]
fn fallback_chain_folds_left() {
    expands_as("a ? b ? c", "(or (or a b) c)");
}

#[test]
fn neg_row() {
    expands_as("print -x", "(print (neg x))");
}

#[test]
fn neg_of_a_group() {
    expands_as("print -{f x}", "(print (neg {(f x)}))");
}

#[test]
fn negative_literal_is_not_sugar() {
    expands_as("print -1", "(print -1)");
}

#[test]
fn interp_row() {
    expands_as(
        "\"note {n} at beat {beat}\"",
        "(concat \"note \" n \" at beat \" beat)",
    );
}

#[test]
fn interp_drops_empty_text() {
    expands_as("print \"{n}\"", "(print (concat n))");
    expands_as("print \"{a}{b}!\"", "(print (concat a b \"!\"))");
}

#[test]
fn interp_expands_nested_sugar() {
    expands_as("print \"v {-x}\"", "(print (concat \"v \" (neg x)))");
}

#[test]
fn splat_passes_through() {
    expands_as("once {s :crash} & p", "(once {(s :crash)} (& p))");
    expands_as("put d & other", "(put d (& other))");
    expands_as("print [a & rest]", "(print [a (& rest)])");
}

#[test]
fn pairs_pass_through_in_order() {
    expands_as(
        "let v voice note: 60 pan: -1",
        "(let v (voice [:note 60] [:pan -1]))",
    );
    expands_as(
        "once {s :crash} at: 4 gain: {- x}",
        "(once {(s :crash)} [:at 4] [:gain {(- x)}])",
    );
}

#[test]
fn pair_values_are_expanded() {
    expands_as("f gain: -x", "(f [:gain (neg x)])");
}
