//! Forcing-mask inference, the `Forward` chase and `mixed_forcing`
//! (design 5.5; ME-MASKS required tests).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::expand::{expand, ExpandCx};
use crate::reader::node::{Atom, Node, NodeKind};
use crate::reader::span::{FileId, Span};
use crate::reader::{read, AliasEnv};
use crate::types::masks::{
    chase, header_params, infer_masks, mixed_forcing, static_mask, CalleeRef, EffectiveEntry,
    ForcingMask, MaskEntry,
};
use crate::types::natives::NativeTable;
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, Scheme, Ty};

use MaskEntry::{Fn as F, Late as L, Undetermined as U, Value as V};

/// Reads and expands one clean form.
fn form(src: &str) -> Node {
    let r = read(src, FileId::new(1), &AliasEnv::new());
    assert!(r.diags.is_empty(), "{src:?}: {:?}", r.diags);
    let mut cx = ExpandCx::new(r.next_node_id());
    expand(&r.nodes[0], &mut cx).unwrap_or_else(|d| panic!("{src:?}: {d}"))
}

fn mask_of(src: &str) -> Vec<MaskEntry> {
    infer_masks(&form(src), &CheckEnv::empty()).0.into_vec()
}

fn g(name: &str) -> CalleeRef {
    CalleeRef::Global(Rc::from(name))
}

fn fwd(links: &[(&str, u16)]) -> MaskEntry {
    MaskEntry::Forward {
        links: links.iter().map(|(n, p)| (g(n), *p)).collect(),
    }
}

fn global(mask: Option<Vec<MaskEntry>>, ty: Ty) -> GlobalInfo {
    GlobalInfo {
        kind: BindKind::Fn,
        scheme: Some(Scheme::mono(ty)),
        mask: mask.map(|m| ForcingMask(m.into_boxed_slice())),
        span: None,
    }
}

#[test]
fn arithmetic_use_is_value() {
    assert_eq!(mask_of("fn f x:\n\t+ x 1"), [V]);
}

#[test]
fn call_position_is_fn() {
    assert_eq!(mask_of("fn f g:\n\tg 1"), [F]);
}

#[test]
fn forward_to_at_links_its_body_position() {
    assert_eq!(mask_of("fn later body:\n\tat 4 body"), [fwd(&[("at", 1)])]);
}

#[test]
fn fan_out_keeps_one_link_per_destination() {
    let m = mask_of("fn f body:\n\tat 4 body\n\tmap [1 2] body\n\tat 8 body");
    assert_eq!(m, [fwd(&[("at", 1), ("map", 1)])]);
}

#[test]
fn any_typed_or_unknown_callee_is_undetermined() {
    let mut env = CheckEnv::empty();
    env.globals.insert("dyn".into(), global(None, Ty::Any));
    let m = infer_masks(&form("fn f x:\n\tdyn x"), &env);
    assert_eq!(m.0.into_vec(), [U]);
    assert_eq!(mask_of("fn f h x:\n\th x"), [F, U]);
    assert_eq!(mask_of("fn f x:\n\tnot-defined-anywhere x"), [U]);
    assert_eq!(mask_of("fn f x:\n\tput [1] & x x"), [V]);
    assert_eq!(mask_of("fn f xs x:\n\tat & xs x"), [V, U]);
}

#[test]
fn unused_parameter_is_value() {
    assert_eq!(mask_of("fn f x:\n\t1"), [V]);
    assert_eq!(mask_of("fn f [a b]:\n\t1"), [V]);
}

#[test]
fn session_callee_is_a_link() {
    let mut env = CheckEnv::empty();
    let fn_ty = Ty::func(vec![Ty::Any], Ty::Nil);
    env.globals
        .insert("later".into(), global(Some(vec![fwd(&[("at", 1)])]), fn_ty));
    let m = infer_masks(&form("fn twice body:\n\tlater body"), &env);
    assert_eq!(m.0.into_vec(), [fwd(&[("later", 0)])]);
}

#[test]
fn statement_line_and_annotation_are_fn() {
    // lang-reference section 1: `body` on a line of its own is called.
    let src = "fn maybe-do p body:\n\tif {< {rand} p}:\n\t\tbody";
    assert_eq!(mask_of(src), [V, F]);
    assert_eq!(mask_of("fn f g: fn int -> int:\n\t1"), [F]);
}

#[test]
fn lambda_masks_and_shadowing() {
    assert_eq!(mask_of("map arr {x -> * x 2}"), [] as [MaskEntry; 0]);
    let lam = form("x g -> g x");
    let lam = match lam.kind {
        NodeKind::Arrow => lam,
        _ => panic!("not a lambda"),
    };
    assert_eq!(infer_masks(&lam, &CheckEnv::empty()).0.into_vec(), [U, F]);
    // The inner `x` shadows the parameter, so the outer one is unused.
    assert_eq!(mask_of("fn f x:\n\tmap [1] {x -> x 1}"), [V]);
    // A match clause binding shadows too.
    assert_eq!(
        mask_of("fn f x g:\n\tmatch g:\n\t\tx -> x 1\n\t\t_ -> 0"),
        [V, V]
    );
}

#[test]
fn late_native_positions_link_and_chase_to_late() {
    let m = mask_of("fn wob n:\n\tosc n");
    assert_eq!(m, [fwd(&[("osc", 0)])]);
    let env = CheckEnv::empty();
    let MaskEntry::Forward { links } = &m[0] else {
        panic!("forward");
    };
    assert_eq!(
        chase(links, &|c| static_mask(&env, c)),
        (EffectiveEntry::Late, false)
    );
}

#[test]
fn header_params_of_an_annotated_fn() {
    let f = form("fn pluck pitch: int amp: float = 1.0 pan: float = 0 -> voice:\n\tpitch");
    let ps = header_params(&f);
    let names: Vec<_> = ps.iter().map(|p| p.name.as_deref()).collect();
    assert_eq!(names, [Some("pitch"), Some("amp"), Some("pan")]);
    let kw: Vec<_> = ps.iter().map(|p| p.keyword).collect();
    assert_eq!(kw, [false, true, true]);
    assert_eq!(infer_masks(&f, &CheckEnv::empty()).len(), 3);
}

type Masks = BTreeMap<&'static str, Vec<MaskEntry>>;

fn table_lookup(masks: &Masks) -> impl Fn(&CalleeRef) -> Option<ForcingMask> + '_ {
    move |c| match c {
        CalleeRef::Global(n) => masks
            .get(&**n)
            .map(|m| ForcingMask(m.clone().into_boxed_slice())),
        _ => None,
    }
}

#[test]
fn chase_combination_rule() {
    let mut masks = BTreeMap::new();
    masks.insert("v", vec![V]);
    masks.insert("f", vec![F]);
    masks.insert("l", vec![L]);
    masks.insert("u", vec![U]);
    masks.insert("w", vec![fwd(&[("w", 0)])]);
    masks.insert("chain", vec![fwd(&[("l", 0)])]);
    let look = table_lookup(&masks);
    let run = |links: &[(&str, u16)]| {
        let links: Vec<_> = links.iter().map(|(n, p)| (g(n), *p)).collect();
        chase(&links, &look)
    };
    assert_eq!(run(&[("v", 0), ("f", 0)]), (EffectiveEntry::Value, true));
    assert_eq!(run(&[("f", 0), ("l", 0)]), (EffectiveEntry::Fn, false));
    assert_eq!(run(&[("l", 0)]), (EffectiveEntry::Late, false));
    assert_eq!(run(&[("chain", 0)]), (EffectiveEntry::Late, false));
    assert_eq!(run(&[("w", 0)]).0, EffectiveEntry::Undetermined, "cycle");
    assert_eq!(
        run(&[("nope", 0)]).0,
        EffectiveEntry::Undetermined,
        "unknown"
    );
    assert_eq!(run(&[("u", 0), ("f", 0)]).0, EffectiveEntry::Undetermined);
    assert_eq!(run(&[]).0, EffectiveEntry::Undetermined);
}

#[test]
fn chase_follows_current_bindings() {
    // Redefining only the downstream callee changes the wrapper's result.
    let mut masks = BTreeMap::new();
    masks.insert("later", vec![fwd(&[("at", 1)])]);
    for (at, want) in [
        (V, EffectiveEntry::Value),
        (F, EffectiveEntry::Fn),
        (L, EffectiveEntry::Late),
    ] {
        masks.insert("at", vec![V, at]);
        let look = table_lookup(&masks);
        assert_eq!(chase(&[(g("later"), 0)], &look).0, want);
    }
}

#[test]
fn mixed_forcing_positions() {
    let mut env = CheckEnv::empty();
    let fn_ty = Ty::func(vec![Ty::Int], Ty::Int);
    env.globals
        .insert("num".into(), global(Some(vec![V]), fn_ty));
    let m = infer_masks(&form("fn f x y:\n\tat 4 x\n\tnum x\n\tat 1 y"), &env);
    assert_eq!(m.0[0], fwd(&[("at", 1), ("num", 0)]));
    assert_eq!(mixed_forcing(&m, &|c| static_mask(&env, c)), [0]);
}

#[test]
fn builtin_and_named_heads() {
    // `for` expands to a `Builtin` `map`, which a session binding cannot capture.
    let m = mask_of("fn each f xs:\n\tfor x xs:\n\t\tf x");
    assert_eq!(m, [F, V]);
    let (id, _) = NativeTable::global().get("map").expect("map");
    // A named `map` head links by name, so a session redefinition is seen.
    assert_eq!(mask_of("fn g h:\n\tmap [1] h"), [fwd(&[("map", 1)])]);
    assert!(static_mask(&CheckEnv::empty(), &CalleeRef::Native(id)).is_some());
}

/// `(fn f x {(h (h .. (h x)))})`, `depth` calls deep, built without recursion.
fn deep_fn(depth: usize, h: &str) -> Node {
    let at = Span::new(FileId::new(1), 0, 0);
    let sym = |s: &str| Node::atom(Atom::Sym(Rc::from(s)), at);
    let mut inner = sym("x");
    for _ in 0..depth {
        inner = Node::new(NodeKind::Call, at, vec![sym(h), inner]);
    }
    let body = Node::new(NodeKind::Block, at, vec![inner]);
    Node::new(
        NodeKind::Call,
        at,
        vec![sym("fn"), sym("f"), sym("x"), body],
    )
}

#[test]
fn deep_body_does_not_panic() {
    let shallow = deep_fn(10, "neg");
    assert_eq!(infer_masks(&shallow, &CheckEnv::empty()).0.into_vec(), [V]);
    let deep = deep_fn(1100, "neg");
    let m = infer_masks(&deep, &CheckEnv::empty());
    assert_eq!(m.0.into_vec(), [U], "past the depth guard: Undetermined");
}

#[test]
fn non_function_nodes_have_empty_masks() {
    assert!(infer_masks(&form("+ 1 2"), &CheckEnv::empty()).is_empty());
    assert_eq!(ForcingMask(Box::new([V, F])).entry(5), Some(&F));
    assert_eq!(ForcingMask::default().entry(0), None);
}
