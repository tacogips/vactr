//! The 5.6 scope model in the checker (7.1.4): rebinding, shadowing,
//! shadows-prelude, upd-immutable and Live redefinition (ME-CHECK
//! required tests).

use crate::types::diag::{DiagCode, Severity};
use crate::types::manifest::HostManifest;
use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, Scheme, Ty};

use super::{assert_clean, assert_diags, assert_has, check_env, diag_lines};

#[test]
fn same_scope_rebinding_is_an_error() {
    assert_diags("let a 12\nlet a 13", &["rebinding@2"]);
    assert_has(
        "let a 12\nlet a 13",
        DiagCode::Rebinding,
        2,
        Severity::Error,
    );
    assert_diags("var hits 0\nvar hits 5", &["rebinding@2"]);
    assert_diags("let a 1\nfn a:\n\t2", &["rebinding@2"]);
}

#[test]
fn upd_of_an_immutable_binding() {
    assert_diags("let a 12\nupd a 13", &["upd-immutable@2"]);
    assert_diags(
        "upd sound-kit [bd: {sample ./bd.wav}]",
        &["upd-immutable@1"],
    );
    assert_diags("fn f p:\n\tupd p 1", &["upd-immutable@2"]);
    assert_diags("upd nothing-here 1", &["undefined-name@1"]);
    assert_clean("var hits 0\nupd hits {+ hits 1}");
}

#[test]
fn a_prelude_shadow_is_one_hint() {
    assert_diags("let scale 2", &["shadows-prelude@1"]);
    assert_has("let scale 2", DiagCode::ShadowsPrelude, 1, Severity::Hint);
    assert_diags(
        "let sound-kit put default-sound-kit [bd: {sample ./bd/909.wav}]",
        &["shadows-prelude@1"],
    );
}

#[test]
fn a_user_parent_shadow_is_a_warning() {
    let src = "let x 1\nfn f x:\n\tx";
    assert_diags(src, &["shadowing@2"]);
    assert_has(src, DiagCode::Shadowing, 2, Severity::Warning);
}

#[test]
fn fn_body_statements_share_the_parameter_scope() {
    assert_diags("fn f a:\n\tlet y 1\n\tlet y 2\n\t+ a y", &["rebinding@3"]);
    assert_diags("fn f y:\n\tlet y 2\n\ty", &["rebinding@2"]);
}

#[test]
fn a_nested_block_may_shadow_with_a_warning() {
    assert_diags(
        "fn f y:\n\tif true:\n\t\tlet y 2\n\t\ty\n\ty",
        &["shadowing@3"],
    );
    // Each clause is its own scope: the same name in two clauses is fine.
    assert_clean("match 3:\n\t0 -> 0\n\tk -> k");
    assert_clean("fn f v:\n\tmatch v:\n\t\t[a b] -> + a b\n\t\ta -> a");
}

#[test]
fn live_redefinition_against_the_session_has_no_diagnostic() {
    let mut env = CheckEnv::empty();
    env.globals.insert(
        "a".into(),
        GlobalInfo {
            kind: BindKind::Let,
            scheme: Some(Scheme::mono(Ty::Int)),
            mask: None,
            span: None,
        },
    );
    let m = HostManifest::spec_default();
    let src = "let a 13";
    assert_eq!(
        diag_lines(src, &check_env(src, &env, &m)),
        Vec::<String>::new()
    );
    // Twice in the checked document is still a rebinding.
    let src = "let a 13\nlet a 14";
    assert_eq!(diag_lines(src, &check_env(src, &env, &m)), ["rebinding@2"]);
    // A session name shadowed inside a fn is a warning.
    let src = "fn f a:\n\ta";
    assert_eq!(diag_lines(src, &check_env(src, &env, &m)), ["shadowing@1"]);
}

#[test]
fn inst_header_parameters_are_control_names() {
    assert_clean("inst pluck amp: float = 0.5:\n\tsaw 440\n\t\t> * amp");
    assert_clean("inst pluck freq: float = 440 amp: float = 0.5 cutoff: float = 2000:\n\tsaw freq\n\t\t> lpf cutoff\n\t\t> * amp");
}

#[test]
fn let_destructuring_binds_every_name_once() {
    assert_clean("let [a b] [1 2]\n+ a b");
    assert_clean("let d [amp: 0.5 pan: -1]\nlet [amp: a pan: p] d\n[a p]");
    assert_diags("let [a a] [1 2]", &["rebinding@1"]);
    // Dict-pattern names keep the binding kind of their `var`.
    assert_clean("var [amp: a] [amp: 1]\nupd a 2");
}
