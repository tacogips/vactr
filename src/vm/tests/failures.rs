//! Failure cases (TASK-005 failure criterion, design 8.4): each unwinds,
//! reports an origin span, and leaves the session usable.

use super::*;
use crate::vm::fail::FailCode;

fn fails(s: &mut Sess, src: &str, code: FailCode) {
    let e = s.eval(src).expect_err(src);
    assert_eq!(e.code, code, "{src}: {e}");
    let span = e.origin.span.unwrap_or_else(|| panic!("{src}: no origin"));
    assert!(
        span.end as usize <= src.len() && span.start <= span.end,
        "{src}: {span:?}"
    );
    assert!(s.vm.frames.is_empty() && s.vm.stack.is_empty() && s.vm.pending.is_empty());
    assert_eq!(s.show("+ 1 2"), "3", "{src}: the session is usable");
}

#[test]
fn division_by_zero() {
    let mut s = Sess::new();
    fails(&mut s, "/ 1 0", FailCode::DivisionByZero);
    let e = s.eval("/ 1 0").unwrap_err();
    assert_eq!(e.origin.span.map(|x| (x.start, x.end)), Some((0, 5)));
}

#[test]
fn type_fault() {
    fails(&mut Sess::new(), "+ 1 \"a\"", FailCode::Type);
}

#[test]
fn no_matching_clause() {
    fails(&mut Sess::new(), "match 5:\n\t0 -> 1", FailCode::NoMatch);
}

#[test]
fn unbounded_source() {
    fails(
        &mut Sess::new(),
        "for x 0..:\n\tprint x",
        FailCode::FuelExhausted,
    );
}

#[test]
fn deep_recursion() {
    let mut s = Sess::new();
    fails(
        &mut s,
        "fn down n:\n\t+ 1 {down n}\ndown 1",
        FailCode::DepthExceeded,
    );
}

#[test]
fn deep_recursion_through_native_re_entry() {
    // `map` calls back into `deep`: the re-entry bound (7.1.5).
    let mut s = Sess::new();
    fails(
        &mut s,
        "fn deep n:\n\tmap [n] deep\ndeep 1",
        FailCode::DepthExceeded,
    );
}

#[test]
fn calling_a_number() {
    fails(&mut Sess::new(), "let x 5\nx 1", FailCode::NotCallable);
}

#[test]
fn wrong_arity() {
    let mut s = Sess::new();
    fails(&mut s, "fn f a:\n\t+ a 1\nf 1 2", FailCode::Arity);
}

#[test]
fn upd_of_a_let_and_of_sound_kit() {
    let mut p = Prelude::core();
    p.register_value("sound-kit", Value::dict(Default::default()));
    let mut s = Sess::with_prelude(p);
    fails(&mut s, "let a 1\nupd a 2", FailCode::UpdImmutable);
    fails(&mut s, "upd sound-kit [bd: 1]", FailCode::UpdImmutable);
    fails(&mut s, "upd len 3", FailCode::UpdImmutable);
    fails(&mut s, "fn f x:\n\tupd x 2\nf 1", FailCode::UpdImmutable);
}

#[test]
fn undefined_name() {
    fails(&mut Sess::new(), "nope 1", FailCode::UndefinedName);
}

#[test]
fn a_failed_form_leaves_earlier_bindings() {
    let mut s = Sess::new();
    s.eval("let a 1").expect("a");
    s.eval("let a {/ 1 0}").expect_err("fails");
    assert_eq!(s.get_str("a"), "1");
}

#[test]
fn a_destructured_binding_is_a_snapshot_and_cannot_form_a_var_ref_cycle() {
    let mut s = Sess::new();
    assert_eq!(s.show("var x 1\nlet [a] [x]\nupd x 5\na"), "1");
    // Rebinding `x` from a list holding its own ref stores the value.
    assert_eq!(s.show("var y 1\nvar [y] [y]\nlet z y\nz"), "1");
    assert_eq!(s.show("y 1"), "fail: not-callable");
}

#[test]
fn a_local_var_destructure_binds_updatable_locals() {
    let mut s = Sess::new();
    assert_eq!(
        s.show("fn f k:\n\tvar [a b] [1 2]\n\tupd a 5\n\t+ a b\nf 0"),
        "7"
    );
}

#[test]
fn min_and_max_read_late_list_items() {
    let mut s = Sess::new();
    assert_eq!(s.show("var x 3\nmin [x 5]"), "3");
    assert_eq!(s.show("max [x 5]"), "5");
}
