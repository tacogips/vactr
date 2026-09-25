//! `Evaluator::eval_form` (design 7.1.3 "Top-level driver").

use super::reactive_basic::{Harness, FILE};
use crate::ns::depgraph::FormState;
use crate::ns::load::read_forms;
use crate::ns::stage::StagedEffect;
use crate::vm::fail::FailCode;

#[test]
fn eval_form_returns_the_value_and_form_gens_increase() {
    let mut h = Harness::new();
    let forms = read_forms("+ 1 2\nlet a 5\na", FILE).expect("read");
    let outs: Vec<_> = forms.iter().map(|f| h.ev.eval_form(f)).collect();
    assert_eq!(
        outs[0]
            .value
            .as_ref()
            .map(ToString::to_string)
            .ok()
            .as_deref(),
        Some("3")
    );
    assert_eq!(
        outs[2]
            .value
            .as_ref()
            .map(ToString::to_string)
            .ok()
            .as_deref(),
        Some("5")
    );
    assert!(outs[0].form_gen < outs[1].form_gen && outs[1].form_gen < outs[2].form_gen);
}

#[test]
fn a_failing_form_leaves_the_session_usable() {
    let mut h = Harness::new();
    h.ok("let a 1");
    for (src, code) in [
        ("/ 1 0", FailCode::DivisionByZero),
        ("+ 1 \"a\"", FailCode::Type),
        ("for x 0..:\n\tprint x", FailCode::FuelExhausted),
        (
            "fn down n:\n\t+ 1 {down n}\ndown 1",
            FailCode::DepthExceeded,
        ),
    ] {
        let out = h.run(src);
        let e = out.last().expect("o").value.clone().expect_err(src);
        assert_eq!(e.code, code, "{src}");
        assert!(e.origin.span.is_some(), "{src}: origin");
        assert_eq!(h.show("+ a 1"), "2", "{src}: usable");
    }
    assert!(
        h.take()
            .iter()
            .all(|e| !matches!(e, StagedEffect::Console(_))),
        "a failed form prints nothing"
    );
}

#[test]
fn a_redefinition_triggers_the_reactive_pass() {
    let mut h = Harness::new();
    h.ok("let a 1\nlet b + a 1\nd1 b");
    h.take();
    h.ok("let a 10");
    assert_eq!(h.get("b"), "11");
    assert_eq!(h.state("b"), FormState::Recomputed);
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![("d1".to_string(), "11".to_string())]
    );
    assert_eq!(
        Harness::batches(&fx),
        vec![vec![
            ("a".to_string(), "10".to_string()),
            ("b".to_string(), "11".to_string())
        ]]
    );
    // The superseded `let a 1` form is dead; the new one owns `a`.
    assert_eq!(h.ev.runs("a"), 1);
}

#[test]
fn a_standalone_read_of_a_failed_form_is_blocked() {
    let mut h = Harness::new();
    h.ok("var root 1\nlet left / 1 root");
    h.ok("upd root 0");
    assert!(matches!(h.state("left"), FormState::Failed(_)));
    assert_eq!(h.show("+ left 1"), "fail: blocked");
    // Its retained value is still displayed and the session is usable.
    assert_eq!(h.get("left"), "1");
    // Re-typing the failed form repairs it and wakes its dependents.
    h.ok("upd root 2");
    assert_eq!(h.show("+ left 1"), "3/2");
}

#[test]
fn top_level_print_and_upd_forms_publish_their_effects_once() {
    let mut h = Harness::new();
    h.ok("var v 1\nprint \"hi\"");
    let fx = h.take();
    assert_eq!(
        fx.iter()
            .filter(|e| matches!(e, StagedEffect::Console(_)))
            .count(),
        1
    );
    h.ok("upd v 2");
    let fx = h.take();
    assert!(
        matches!(
            fx.as_slice(),
            [StagedEffect::CellUpdate { .. }, StagedEffect::Bindings(_)]
        ),
        "{fx:?}"
    );
}
