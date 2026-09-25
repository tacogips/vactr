//! Advisory eager/late dependency edges (design 5.6; ME-CHECK required
//! tests).

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::types::deps::deps;
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, Scheme, Ty};

use super::forms;

fn env_with(names: &[(&str, BindKind)]) -> CheckEnv {
    let mut env = CheckEnv::empty();
    for (name, kind) in names {
        env.globals.insert(
            (*name).into(),
            GlobalInfo {
                kind: *kind,
                scheme: Some(Scheme::mono(Ty::Int)),
                mask: None,
                span: None,
            },
        );
    }
    env
}

fn set(names: &[&str]) -> BTreeSet<Rc<str>> {
    names.iter().map(|n| Rc::from(*n)).collect()
}

fn deps_of(src: &str, env: &CheckEnv) -> (BTreeSet<Rc<str>>, BTreeSet<Rc<str>>) {
    let f = forms(src);
    let d = deps(&f[0], env);
    (d.eager, d.late)
}

#[test]
fn arithmetic_on_a_session_name_is_eager() {
    let env = env_with(&[("root", BindKind::Let)]);
    assert_eq!(
        deps_of("let raised + root 7", &env),
        (set(&["root"]), set(&[]))
    );
}

#[test]
fn a_pattern_capturing_a_var_is_late() {
    let env = env_with(&[("cutoff", BindKind::Var)]);
    let (eager, late) = deps_of("s [:bd-haus :sn-dub] > lpf cutoff > d1", &env);
    assert_eq!((eager, late), (set(&[]), set(&["cutoff"])));
}

#[test]
fn bodies_and_lambdas_are_late_and_locals_are_not_edges() {
    let env = env_with(&[
        ("base", BindKind::Let),
        ("xs", BindKind::Let),
        ("k", BindKind::Let),
    ]);
    assert_eq!(
        deps_of("fn f x:\n\t+ x base", &env),
        (set(&[]), set(&["base"]))
    );
    assert_eq!(
        deps_of("map xs {x -> + x k}", &env),
        (set(&["xs"]), set(&["k"]))
    );
    assert_eq!(
        deps_of("let y {+ base 1}", &env),
        (set(&["base"]), set(&[]))
    );
    // A name read both ways is only eager; prelude names are not slots.
    assert_eq!(
        deps_of("s :bd > gain base > n base > d1", &env),
        (set(&[]), set(&["base"]))
    );
    // A dict pattern binds its values, not its keys.
    assert_eq!(
        deps_of("match base:\n\t[amp: a] -> + a k", &env),
        (set(&["base", "k"]), set(&[]))
    );
    assert_eq!(
        deps_of("+ base {first [base]}", &env),
        (set(&["base"]), set(&[]))
    );
}
