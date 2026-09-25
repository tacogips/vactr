//! `is_kernel`, kernel shapes and match guards.

use super::*;

use crate::expand::kernel::check_shape;

#[test]
fn outputs_are_kernel_and_sugar_is_not() {
    let r = rd("print -x");
    assert!(!is_kernel(&r.nodes[0]), "a tree with Neg is not kernel");
    for src in [
        "d :gain ? 1",
        "\"a {b}\"",
        "if a b",
        "for x arr:\n\tprint x",
        "if a b -> c",
        "if a:\n\tx\nelse:\n\ty",
    ] {
        let r = rd(src);
        assert!(!is_kernel(&r.nodes[0]), "{src:?} is sugar");
    }
    let r = rd("import github.com/someone/vactrol-pads");
    assert!(is_kernel(&r.nodes[0]), "import at the root");
}

#[test]
fn builtin_only_as_a_call_head() {
    let (_, out) = expand_src("print -x");
    let n = out.into_iter().next().expect("form").expect("clean");
    assert!(is_kernel(&n));
    let neg_call = &n.children[1];
    let builtin = neg_call.children[0].clone();
    let misplaced = super::super::expander::rebuild(&n, vec![n.children[0].clone(), builtin]);
    assert!(!is_kernel(&misplaced));
}

#[test]
fn kernel_shapes_are_valid() {
    for src in [
        "match x:\n\t:kick -> :bd-haus\n\t:snare -> :sn-dub\n\t_ -> nil",
        "fn f a b:\n\t* a 12",
        "fn add a: int b: int -> int:\n\t+ a b",
        "let a 12",
        "let a: int 12",
        "let [a b] arr",
        "let [amp: a pan: p] d",
        "var n 0",
        "upd n 1",
        "enum shape:\n\tcircle r\n\trect w h\n\tnone",
        "enum shape:\n\tcircle r: float",
        "map pairs {[k v] -> v}",
    ] {
        let r = rd(src);
        assert!(r.diags.is_empty(), "{src:?}: {:?}", r.diags);
        for n in &r.nodes {
            assert!(check_shape(n).is_ok(), "{src:?}");
        }
        let (_, out) = expand_src(src);
        for o in out {
            let n = o.unwrap_or_else(|d| panic!("{src:?}: {d}"));
            assert!(is_kernel(&n), "{src:?}");
        }
    }
}

#[test]
fn match_is_kept_and_its_bodies_expand() {
    expands_as(
        "match x:\n\t:kick -> -y\n\t_ -> nil",
        "(match x {(-> (:kick) (neg y)) (-> (_) nil)})",
    );
}

#[test]
fn match_guard_expands_the_guard_only() {
    expands_as(
        "match v:\n\tx if {> x 100} -> \"big\"\n\tcircle r if {> r -y} -> \"big circle\"",
        "(match v {(-> (x if {(> x 100)}) \"big\") (-> (circle r if {(> r (neg y))}) \"big circle\")})",
    );
}

#[test]
fn lambda_patterns_are_not_expanded() {
    expands_as("map arr {x -> * x 2}", "(map arr (-> (x) (* x 2)))");
    expands_as(
        "reduce arr 0 {acc x -> + acc -x}",
        "(reduce arr 0 (-> (acc x) (+ acc (neg x))))",
    );
}

#[test]
fn fn_header_is_kept_and_body_expands() {
    expands_as(
        "fn fact k:\n\tif {<= k 1} 1 {* k {fact {- k 1}}}",
        "(fn fact k {(match {(<= k 1)} {(-> (false | nil) {(* k {(fact {(- k 1)})})}) (-> (_) 1)})})",
    );
}

#[test]
fn enum_is_unchanged() {
    expands_as(
        "enum shape:\n\tcircle r\n\trect w h\n\tnone",
        "(enum shape {(circle r) (rect w h) none})",
    );
}

#[test]
fn synthesized_ids_are_fresh_and_unique_across_forms() {
    let src = "if a b c\nfor x arr:\n\tprint -x\nprint \"{a} {b}\"";
    let (r, out) = expand_src(src);
    let nodes: Vec<Node> = out.into_iter().map(|o| o.expect("clean")).collect();
    assert_fresh_ids(&r, &nodes);
}

#[test]
fn synthesized_nodes_carry_the_origin_span() {
    let src = "print -x";
    let (r, out) = expand_src(src);
    let n = out.into_iter().next().expect("form").expect("clean");
    let neg_src = &r.nodes[0].children[1];
    let neg_call = &n.children[1];
    assert_eq!(neg_call.span, neg_src.span);
    assert_eq!(neg_call.children[0].span, neg_src.span);
    assert!(neg_call.id >= r.next_node_id());
    assert_eq!(
        neg_call.children[1].id, neg_src.children[0].id,
        "the operand keeps its id"
    );
}
