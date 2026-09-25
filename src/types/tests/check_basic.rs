//! Inference basics (ME-CHECK required tests): literals, lists and dicts,
//! fn inference and let-polymorphism, numeric widening, `?T` and `?`,
//! `any` narrowing, and the annotated negatives of lang-reference
//! sections 1-2.

use super::{assert_clean, assert_diags, last_type};

#[test]
fn literal_types() {
    assert_eq!(last_type("1"), "int");
    assert_eq!(last_type("3000000000"), "int64");
    assert_eq!(last_type("2.5"), "float");
    assert_eq!(last_type("1/4"), "ratio");
    assert_eq!(last_type("\"text\""), "string");
    assert_eq!(last_type(":kick"), "keyword");
    assert_eq!(last_type("nil"), "nil");
    assert_eq!(last_type("true"), "bool");
    assert_eq!(last_type("./soundpack/bd/1.wav"), "path");
    assert_eq!(last_type("https://example.org/packs/x.vact"), "url");
}

#[test]
fn lists_and_dicts() {
    assert_eq!(last_type("[1 2 3]"), "[int]");
    assert_eq!(last_type("[:bd :sd]"), "[keyword]");
    assert_eq!(last_type("[amp: 0.5 pan: -1]"), "[keyword: float]");
    assert_eq!(last_type("0..8"), "[int]");
    // A heterogeneous list is legal: its items are `any`, no diagnostic.
    assert_eq!(last_type("[1 \"a\"]"), "[any]");
    assert_clean("[1 \"a\"]");
}

#[test]
fn fn_inference_and_let_polymorphism() {
    assert_eq!(last_type("fn add a b:\n\t+ a b\nadd 1 2"), "int");
    assert_eq!(last_type("let id x -> x\nid 1\nid \"a\""), "string");
    assert_clean("let id x -> x\nid 1\nid \"a\"");
    let twice = "fn twice f x:\n\tf {f x}\ntwice {y -> + y 1} 3\ntwice {t -> concat t \"!\"} \"a\"";
    assert_clean(twice);
    assert_eq!(last_type(twice), "string");
    assert_clean("fn fact k:\n\tif {<= k 1} 1 {* k {fact {- k 1}}}\nfact 5");
}

#[test]
fn numeric_widening() {
    assert_eq!(last_type("+ 1 2"), "int");
    assert_eq!(last_type("+ 1 2.5"), "float");
    assert_eq!(last_type("* 60 1.5"), "float");
    assert_eq!(last_type("+ 1/4 1/8"), "ratio");
    assert_eq!(last_type("/ 1 3"), "ratio");
    assert_eq!(last_type("* 1/3 0.5"), "float");
    assert_eq!(last_type("let big: int64 1\nbig"), "int64");
    assert_clean("let big: int64 1\n+ big 2.5");
}

#[test]
fn optional_flow_and_fallback() {
    let d = "let d [gain: 0.5]\n";
    assert_eq!(last_type(&format!("{d}d :gain")), "?float");
    assert_eq!(last_type(&format!("{d}d :gain ? 1.0")), "float");
    assert_clean(&format!("{d}+ {{d :gain ? 1.0}} 1"));
    // Accessors pun nil; arithmetic does not.
    assert_clean("first nil\nlen nil");
    assert_eq!(last_type("first [1 2]"), "?int");
}

#[test]
fn optional_used_as_value() {
    assert_diags("let d [gain: 0.5]\n+ {d :gain} 1", &["optional-as-value@2"]);
    assert_diags("+ {first [1 2]} 1", &["optional-as-value@1"]);
}

#[test]
fn any_must_be_narrowed() {
    assert_diags("let x: any 5\n+ x 1", &["any-not-narrowed@2"]);
    assert_clean("let x: any 5\nmatch x:\n\tk -> + k 1");
    assert_clean("let x: any 5\nprint x");
    // An unknown annotation type is not a written `any`.
    assert_clean("let x: foo 1\n+ x 1");
}

#[test]
fn annotated_negatives() {
    // lang-reference section 2: `+ 1 "a"  # fails (type)`.
    assert_diags("+ 1 \"a\"", &["type-mismatch@1"]);
    assert_diags("let m: int \"a\"", &["annotation-mismatch@1"]);
    // `amp` is also the prelude signal: a parameter shadowing it is a hint.
    assert_diags(
        "fn pluck pitch: int amp: float = \"x\":\n\tpitch",
        &["shadows-prelude@1", "annotation-mismatch@1"],
    );
    assert_clean("fn pluck pitch: int vol: float = 1.0:\n\t+ pitch 1\npluck 60 vol: 0.5");
}

#[test]
fn calls_arity_and_named_arguments() {
    assert_diags("take [1 2]", &["type-mismatch@1"]);
    assert_diags("fn f a:\n\t+ a 1\nf 1 2", &["type-mismatch@3"]);
    assert_diags(
        "fn f a b: int = 1:\n\t+ a b\nf 1 c: 2",
        &["type-mismatch@3"],
    );
    assert_clean("once {s :crash} at: 4 gain: 0.5");
    assert_diags("\"a\" 1", &["type-mismatch@1"]);
}

#[test]
fn structs_and_enums() {
    let voice = "struct voice:\n\tamp 1.0\n\tpan 0\n\tnote\nlet v voice note: 60 pan: -1\n";
    assert_clean(&format!("{voice}v :amp"));
    assert_diags(&format!("{voice}v :nope"), &["unknown-keyword@6"]);
    assert_diags(&format!("{voice}voice nope: 1"), &["type-mismatch@6"]);
    let shapes = "enum figure:\n\tcircle r\n\trect w h\n\tnone\n";
    assert_clean(&format!(
        "{shapes}fn area sh:\n\tmatch sh:\n\t\tcircle r -> * 3.14 r r\n\t\trect w h -> * w h\n\t\tnone -> 0\narea {{circle 5}}"
    ));
}

#[test]
fn thunk_typing_follows_the_mask_entry_of_each_parameter() {
    // `g` is called on its own line: it receives the closure, even after
    // a keyword parameter or `_`.
    assert_diags("fn f a = 1 g:\n\tg\nf 3", &["type-mismatch@3"]);
    assert_diags("fn f _ g:\n\tg\nf 1 3", &["type-mismatch@3"]);
    assert_clean("fn f _ g:\n\tg\nf 1 {+ 1 2}");
}

#[test]
fn inst_parameters_are_controls_only_at_a_call_head() {
    let inst = "inst pluck cutoff: float = 2000:\n\tsaw 440\n";
    assert_clean(&format!(
        "{inst}s :pluck > cutoff {{range sine 400 2000}} > d1"
    ));
    assert_diags(&format!("{inst}print cutoff"), &["undefined-name@3"]);
}

#[test]
fn overloads_resolve_on_the_subject() {
    assert_clean("s :pluck > n [0 2 4] > scale :c :minor > d1");
    assert_clean("osc 10 > scale 1.5 > out o0");
    assert_clean("shape 3 > out o0");
    assert_clean("s :bd > shape 0.5 > d1");
    // Subject first (M2).
    assert_clean("s :bd > lpf {range sine 200 2000} > d1");
    assert_clean("grid {s :bd} [true false true true]");
}
