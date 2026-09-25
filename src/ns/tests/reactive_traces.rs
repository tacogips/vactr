//! The normative reactive traces of design 5.6 (TASK-005 reactive
//! criterion): CHANGING-EDGE, a genuine cycle, FAILED-DIAMOND, and the three
//! abort-policy traces (ABORT/RETRY-FAILURE, REACHABLE PROVISIONAL-ROLLBACK,
//! LATE-FAILURE). Each asserts the exact deterministic schedule.

use super::reactive_basic::Harness;
use crate::ns::depgraph::FormState;
use crate::ns::evaluator::PassEvent;
use crate::ns::stage::SlotKey;
use crate::types::diag::DiagCode;
use crate::value::intern::name_of_sym;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn changing_edge_dirty_read_abort_then_both_end_at_twenty() {
    let mut h = Harness::new();
    h.ok("var switch false\nvar root 10\nlet a d1 {if switch b root}\nlet b d2 {if switch root {+ a 1}}");
    assert_eq!((h.get("a"), h.get("b")), ("10".into(), "11".into()));
    h.take();
    h.ev.queue_upd("switch", Value::Bool(true)).expect("switch");
    h.ev.queue_upd("root", Value::Int(20)).expect("root");
    h.ev.run_pass();
    // A's deref of the scheduled B aborts: A never commits a stale value.
    assert_eq!(h.trace(), strs(&["dirty a<-b", "commit b", "commit a"]));
    assert_eq!(
        h.provisional("a"),
        strs(&["20"]),
        "no intermediate A commit"
    );
    assert_eq!((h.get("a"), h.get("b")), ("20".into(), "20".into()));
    assert_eq!(h.ev.edges("a"), strs(&["b", "switch"]));
    assert_eq!(h.ev.edges("b"), strs(&["root", "switch"]));
    // Only the final slot intent is staged.
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![
            ("d2".to_string(), "20".to_string()),
            ("d1".to_string(), "20".to_string())
        ]
    );
    assert!(!Harness::visible(&fx).contains(&"11".to_string()));
}

#[test]
fn genuinely_cyclic_edges_stop_with_a_cycle_diagnostic_keeping_values() {
    let mut h = Harness::new();
    h.ok("var switch false\nvar root 1\nlet a if switch b root\nlet b if switch {+ a 1} root");
    h.take();
    let out = h.run("upd switch true");
    assert!(
        out[0]
            .diags
            .iter()
            .any(|d| d.code == DiagCode::DependencyCycle),
        "{:?}",
        out[0].diags
    );
    assert_eq!(h.trace(), strs(&["dirty a<-b", "dirty b<-a", "cycle a,b"]));
    assert_eq!((h.get("a"), h.get("b")), ("1".into(), "1".into()));
    assert!(h.failed("a").is_some() && h.failed("b").is_some());
    let fx = h.take();
    assert!(Harness::binds(&fx).is_empty());
}

#[test]
fn failed_diamond_blocks_total_then_repair_recovers_it() {
    let mut h = Harness::new();
    h.ok("var root 1\nlet left / 1 root\nlet right + root 1\nlet total + left right\nd1 total");
    assert_eq!(h.get("total"), "3");
    h.take();
    h.ok("upd root 0");
    assert_eq!(h.failed("left"), Some(FailCode::DivisionByZero));
    assert_eq!(h.get("left"), "1", "a failed transaction commits nothing");
    assert_eq!(h.get("right"), "1");
    assert_eq!(h.blocked_on("total").as_deref(), Some("left"));
    assert_eq!(h.get("total"), "3", "previous committed total retained");
    let fx = h.take();
    assert!(Harness::binds(&fx).is_empty(), "no slot intent for total");
    let d1 = h.ev.form_of_bind(SlotKey::D(1)).expect("d1");
    assert!(matches!(
        h.ev.graph().get(d1).map(|r| r.state.clone()),
        Some(FormState::Blocked { on }) if name_of_sym(on).as_ref() == "total"
    ));
    // Repair: left recovers, total unblocks and recomputes, d1 re-binds.
    h.ok("upd root 1");
    assert_eq!(h.state("left"), FormState::Recomputed);
    assert_eq!(h.state("total"), FormState::Recomputed);
    assert_eq!(h.get("total"), "3");
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![("d1".to_string(), "3".to_string())]
    );
}

#[test]
fn trace_1_abort_retry_failure_commits_nothing_for_a() {
    let mut h = Harness::new();
    h.ok("var switch false\nvar root 2\nlet a d1 {if switch {/ 1 b} root}\nlet b if switch 0 {+ a 1}");
    assert_eq!((h.get("a"), h.get("b")), ("2".into(), "3".into()));
    h.take();
    h.ok("upd switch true");
    assert_eq!(
        h.trace(),
        strs(&["dirty a<-b", "commit b", "fail a"]),
        "no journal restore occurs"
    );
    assert!(h.provisional("a").is_empty(), "A never holds 1/3");
    assert_eq!(h.failed("a"), Some(FailCode::DivisionByZero));
    assert_eq!(h.get("a"), "2", "the previous committed A stands");
    assert_eq!(h.get("b"), "0");
    assert_eq!(h.state("b"), FormState::Recomputed);
    let fx = h.take();
    assert!(Harness::binds(&fx).is_empty(), "no A intent is ever staged");
    assert!(!Harness::visible(&fx).contains(&"1/3".to_string()));
}

#[test]
fn trace_2_reachable_provisional_rollback_restores_x() {
    let mut h = Harness::new();
    h.ok("var n 0\nlet x d1 {if {> n 0} {/ 1 {- 12 y}} 7}\nlet z if {> n 0} 5 3\nlet y + z 7");
    assert_eq!(
        (h.get("x"), h.get("z"), h.get("y")),
        ("7".into(), "3".into(), "10".into())
    );
    let slot =
        h.ev.ns()
            .session_slot(crate::value::intern::intern_sym("x"))
            .expect("x");
    let (v0, owner0) = (slot.version(), slot.owner());
    h.take();
    h.ok("upd n 1");
    assert_eq!(
        h.trace(),
        strs(&[
            "commit x",
            "commit z",
            "commit y",
            "stale x",
            "fail x",
            "restore x"
        ])
    );
    // The provisional 1/2 existed at the transaction layer only.
    assert_eq!(h.provisional("x"), strs(&["1/2"]));
    assert!(h.events().iter().any(|e| matches!(
        e,
        PassEvent::Restore(r) if matches!(r.value, Value::Int(7)) && r.version == v0 && r.owner == owner0
    )));
    assert_eq!(h.get("x"), "7");
    assert_eq!(
        (slot.version(), slot.owner()),
        (v0, owner0),
        "value, version, owner restored"
    );
    assert_eq!(h.failed("x"), Some(FailCode::DivisionByZero));
    assert_eq!((h.get("z"), h.get("y")), ("5".into(), "12".into()));
    let fx = h.take();
    assert!(
        Harness::binds(&fx).is_empty(),
        "no bind of X reaches the host"
    );
    assert!(!Harness::visible(&fx).contains(&"1/2".to_string()));
    let batch = &Harness::batches(&fx)[0];
    assert!(
        batch.contains(&("x".to_string(), "7".to_string())),
        "{batch:?}"
    );
}

#[test]
fn trace_3_late_failure_blocks_x_on_y_and_restores_it() {
    let mut h = Harness::new();
    h.ok("var n 0\nlet x d1 {if {> n 0} {+ y 1} 7}\nlet z if {> n 0} 0 1\nlet y / 1 z");
    assert_eq!(
        (h.get("x"), h.get("z"), h.get("y")),
        ("7".into(), "1".into(), "1".into())
    );
    let y =
        h.ev.ns()
            .session_slot(crate::value::intern::intern_sym("y"))
            .expect("y");
    let yv = y.version();
    h.take();
    h.ok("upd n 1");
    assert_eq!(
        h.trace(),
        strs(&["commit x", "commit z", "fail y", "blocked x@y", "restore x"])
    );
    assert_eq!(h.provisional("x"), strs(&["2"]));
    assert_eq!(y.version(), yv, "the failing producer wrote no version");
    assert_eq!(h.blocked_on("x").as_deref(), Some("y"));
    assert_eq!(h.get("x"), "7");
    assert_eq!(h.failed("y"), Some(FailCode::DivisionByZero));
    assert_eq!(h.get("y"), "1");
    assert_eq!(h.get("z"), "0");
    let fx = h.take();
    assert!(Harness::binds(&fx).is_empty());
    assert!(!Harness::visible(&fx).contains(&"2".to_string()));
}
