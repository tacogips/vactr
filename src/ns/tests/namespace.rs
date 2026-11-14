//! The 5.6 scope model at run time: a read-only prelude, the session scope,
//! fn/block child scopes, compile-time slot resolution, `VarRef` demand
//! and `let` snapshots.

use crate::ns::namespace::{FormGen, Resolved, SlotKind};
use crate::value::intern::intern_sym;
use crate::value::value::Value;
use crate::vm::fail::FailCode;
use crate::vm::tests::Sess;

#[test]
fn a_session_binding_shadows_a_prelude_native_at_run_time() {
    let mut s = Sess::new();
    assert_eq!(s.show("len [1 2 3]"), "3");
    s.eval("let len 42").expect("shadow");
    assert_eq!(s.show("len"), "42");
    assert_eq!(s.show("len [1 2 3]"), "fail: not-callable");
    assert!(matches!(
        s.ns.lookup(intern_sym("len")),
        Some(Resolved::Session(_))
    ));
}

#[test]
fn forms_compiled_before_the_shadowing_binding_keep_the_prelude_slot() {
    let mut s = Sess::new();
    s.eval("fn count xs:\n\tlen xs").expect("count");
    s.eval("let len 42").expect("shadow");
    assert_eq!(s.show("count [1 2 3]"), "3");
}

#[test]
fn the_prelude_value_is_unchanged_by_a_session_binding() {
    let mut s = Sess::new();
    let prelude = s.ns.prelude().slot(intern_sym("len")).expect("prelude len");
    let before = prelude.version();
    s.eval("let len 42").expect("shadow");
    assert!(matches!(prelude.get(), Value::Native(_)));
    assert_eq!(prelude.version(), before);
    assert_eq!(prelude.kind(), SlotKind::Prelude);
    let e =
        s.ns.write_var(&prelude, Value::Int(1))
            .expect_err("read-only");
    assert_eq!(e.code, FailCode::UpdImmutable);
}

#[test]
fn fn_local_bindings_shadow_session_names() {
    let mut s = Sess::new();
    s.eval("let x 1\nfn f x:\n\t+ x 10\nfn g k:\n\tlet x 2\n\t+ x k")
        .expect("setup");
    assert_eq!(s.show("f 5"), "15");
    assert_eq!(s.show("g 1"), "3");
    assert_eq!(s.show("x"), "1");
    // A lambda parameter and a match binding are child scopes too.
    assert_eq!(s.show("map [5] {x -> + x 1}"), "[6]");
    assert_eq!(s.show("match 7:\n\tx -> x"), "7");
    assert_eq!(s.show("x"), "1");
}

#[test]
fn upd_is_observed_through_var_ref_demand() {
    let mut s = Sess::new();
    s.eval("var cutoff 800\nfn get k:\n\t+ cutoff k")
        .expect("setup");
    assert_eq!(s.show("get 0"), "800");
    let slot = s.ns.session_slot(intern_sym("cutoff")).expect("slot");
    let v0 = slot.version();
    assert_eq!(s.show("upd cutoff 400"), "400");
    assert_eq!(s.show("get 0"), "400");
    assert!(slot.version() > v0);
    // A var captured in a list stays late; a let captured is a snapshot.
    s.eval("let late [cutoff]\nlet a 1\nlet early [a]")
        .expect("lists");
    s.eval("upd cutoff 200\nlet a 2").expect("writes");
    assert_eq!(s.show("+ {first late} 0"), "200");
    assert_eq!(s.show("first early"), "1");
}

#[test]
fn redefinition_is_observed_by_the_next_call() {
    let mut s = Sess::new();
    s.eval("fn kick-sound k:\n\t:bd\nfn play k:\n\tkick-sound k")
        .expect("setup");
    assert_eq!(s.show("play 0"), ":bd");
    s.eval("fn kick-sound k:\n\t:bd-tek").expect("redefine");
    assert_eq!(s.show("play 0"), ":bd-tek");
}

#[test]
fn a_top_level_let_is_a_snapshot() {
    let mut s = Sess::new();
    s.eval("let base 10\nlet derived + base 1").expect("setup");
    s.eval("let base 20").expect("redefine base");
    assert_eq!(s.show("derived"), "11");
}

#[test]
fn re_running_a_let_replaces_the_slot_for_later_evaluations() {
    let mut s = Sess::new();
    s.eval("let a 1\nfn get k:\n\t+ a k").expect("setup");
    let slot = s.ns.session_slot(intern_sym("a")).expect("slot");
    let v0 = slot.version();
    s.eval("let a 2").expect("redefine");
    let same = s.ns.session_slot(intern_sym("a")).expect("slot");
    assert!(slot.same(&same), "redefinition reuses the slot");
    assert!(same.version() > v0);
    assert_eq!(s.show("get 0"), "2");
    assert_eq!(same.owner(), Some(FormGen::new(s.gen - 1)));
}

#[test]
fn upd_of_a_non_var_is_upd_immutable() {
    let mut s = Sess::new();
    for src in ["let a 1\nupd a 2", "upd len 3", "fn f k:\n\tk\nupd f 1"] {
        assert_eq!(s.show(src), "fail: upd-immutable", "{src}");
    }
    assert_eq!(s.show("upd nope 1"), "fail: undefined-name");
}

#[test]
fn lookup_order_is_session_then_prelude_with_reserved_names_last() {
    let s = Sess::new();
    let name = intern_sym("later-defined");
    let reserved = s.ns.reserve(name);
    assert!(!reserved.is_bound());
    assert!(matches!(s.ns.lookup(name), Some(Resolved::Session(_))));
    // A reservation never hides a prelude name.
    s.ns.reserve(intern_sym("len"));
    assert!(matches!(
        s.ns.lookup(intern_sym("len")),
        Some(Resolved::Prelude(_))
    ));
}

#[test]
fn a_forward_reference_resolves_once_defined() {
    let mut s = Sess::new();
    s.eval("fn f k:\n\tg k").expect("f before g");
    assert_eq!(s.show("f 1"), "fail: undefined-name");
    s.eval("fn g k:\n\t+ k 1").expect("g");
    assert_eq!(s.show("f 1"), "2");
}

#[test]
fn a_recursive_fn_that_shadows_a_prelude_name_calls_itself() {
    let mut s = Sess::new();
    s.eval("fn len xs:\n\tmatch xs:\n\t\t[] -> 0\n\t\t[x & rest] -> + 1 {len rest}")
        .expect("define");
    assert_eq!(s.show("len [4 5 6]"), "3");
}
