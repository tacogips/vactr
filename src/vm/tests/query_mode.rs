//! Query effect mode (design 10.4, TASK-005 Query criterion).

use super::*;
use crate::vm::fail::FailCode;
use crate::vm::vm::EffectMode;

fn query(s: &mut Sess, f: &str) -> Result<Value, Failure> {
    let fv = s.get(f);
    let Sess { ns, vm, .. } = s;
    vm.with_effect_mode(EffectMode::Query, |vm| {
        vm.call_value(ns, &fv, vec![Value::Int(0)], Vec::new())
    })
}

#[test]
fn global_upd_in_a_queried_closure_fails_with_origin() {
    let mut s = Sess::new();
    let src = "var g 0\nfn bump k:\n\tupd g {+ g 1}";
    s.eval(src).expect("setup");
    let e = query(&mut s, "bump").expect_err("effect-in-query");
    assert_eq!(e.code, FailCode::EffectInQuery);
    let span = e.origin.span.expect("origin span");
    let at = src.find("upd g").expect("upd") as u32;
    assert!(span.start >= at && span.end <= src.len() as u32, "{span:?}");
    assert_eq!(s.get_str("g"), "0");
}

#[test]
fn frame_local_var_is_legal_in_query_mode() {
    let mut s = Sess::new();
    s.eval("fn count k:\n\tvar n 0\n\tupd n {+ n 1}\n\tupd n {+ n 1}\n\tn")
        .expect("setup");
    assert_eq!(
        query(&mut s, "count").expect("local state").to_string(),
        "2"
    );
}

#[test]
fn print_is_captured_not_staged() {
    let mut s = Sess::new();
    s.eval("fn say k:\n\tprint \"hi\" k").expect("setup");
    let before = s.vm.effects().len();
    query(&mut s, "say").expect("print is legal");
    assert_eq!(s.vm.effects().len(), before, "nothing staged");
    let out = s.vm.take_output();
    assert_eq!(out.len(), 1);
    assert_eq!(&*out[0].1, "hi 0");
    assert!(out[0].0.span.is_some());
    assert!(s.sink.console().is_empty());
}

#[test]
fn slot_binds_and_one_shots_fail_in_query_mode() {
    let mut s = Sess::new();
    s.eval("fn bind k:\n\td1 k\nfn shot k:\n\tonce k")
        .expect("setup");
    assert_eq!(
        query(&mut s, "bind").unwrap_err().code,
        FailCode::EffectInQuery
    );
    assert_eq!(
        query(&mut s, "shot").unwrap_err().code,
        FailCode::EffectInQuery
    );
    assert!(s.vm.effects().is_empty());
}

#[test]
fn scope_guard_restores_normal_after_an_unwinding_failure() {
    let mut s = Sess::new();
    s.eval("var g 0\nfn bump k:\n\tupd g {+ g 1}")
        .expect("setup");
    query(&mut s, "bump").expect_err("fails");
    assert_eq!(s.vm.effect_mode(), EffectMode::Normal);
    assert!(s.vm.frames.is_empty() && s.vm.stack.is_empty() && s.vm.pending.is_empty());
    assert_eq!(s.show("bump 0\ng"), "1");
}

#[test]
fn a_host_capability_can_be_installed_and_taken_back() {
    let mut s = Sess::new();
    assert!(s.vm.set_host(Some(Box::new(7_u32))).is_none());
    let host = s.vm.take_host().expect("installed");
    assert_eq!(host.downcast_ref::<u32>(), Some(&7));
    assert!(s.vm.take_host().is_none());
}

#[test]
fn a_var_captured_from_outside_the_query_is_not_writable() {
    let mut s = Sess::new();
    s.eval("fn mk k:\n\tvar n 0\n\tj -> upd n {+ n 1}\nlet counter mk 0")
        .expect("setup");
    let e = query(&mut s, "counter").expect_err("outlives the query");
    assert_eq!(e.code, FailCode::EffectInQuery);
    // Outside a query the closure's own state works as before.
    assert_eq!(s.show("counter 0"), "1");
}
