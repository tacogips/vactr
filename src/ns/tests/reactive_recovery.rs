//! Deref-time joins and recovery (design 5.6 "Failure at joins", TASK-005
//! reactive criterion): CONDITIONAL-UNBLOCKING, SWITCH-TOWARD through a
//! recovery subscription, STATUS-RECOVERY bypassing the equality cutoff,
//! NEWLY-DISCOVERED-SELECTOR through the attempt edge set, and
//! ORDINARY-FAILURE RECOVERY.

use super::reactive_basic::Harness;
use crate::ns::depgraph::FormState;
use crate::vm::fail::FailCode;

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// `root = 1`, `switch = true`, `broken = 1/root`, `selected = if switch
/// then broken else 7`, then `upd root 0`: broken fails, selected blocks at
/// its actual deref of broken.
fn broken_and_selected() -> Harness {
    let mut h = Harness::new();
    h.ok("var root 1\nvar switch true\nlet broken / 1 root\nlet selected d1 {if switch broken 7}");
    assert_eq!(h.get("selected"), "1");
    h.ok("upd root 0");
    assert_eq!(h.failed("broken"), Some(FailCode::DivisionByZero));
    assert_eq!(h.blocked_on("selected").as_deref(), Some("broken"));
    h.take();
    h
}

#[test]
fn conditional_unblocking_switch_away_from_the_failed_dependency() {
    let mut h = broken_and_selected();
    h.ok("upd switch false");
    assert_eq!(h.get("selected"), "7");
    assert_eq!(h.state("selected"), FormState::Recomputed);
    assert_eq!(
        h.ev.edges("selected"),
        strs(&["switch"]),
        "edge set REPLACED"
    );
    assert!(h.ev.subscriptions("selected").is_empty());
    assert_eq!(
        h.failed("broken"),
        Some(FailCode::DivisionByZero),
        "unrepaired"
    );
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![("d1".to_string(), "7".to_string())]
    );
}

#[test]
fn switch_toward_a_failed_dependency_subscribes_and_repair_wakes_it() {
    let mut h = broken_and_selected();
    h.ok("upd switch false");
    let committed =
        h.ev.graph()
            .get(h.ev.form_of("selected").expect("f"))
            .map(|r| {
                r.edges
                    .iter()
                    .map(|e| (e.slot.id(), e.version))
                    .collect::<Vec<_>>()
            });
    h.ok("upd switch true");
    assert_eq!(h.blocked_on("selected").as_deref(), Some("broken"));
    assert_eq!(
        h.ev.edges("selected"),
        strs(&["switch"]),
        "broken is in no committed edge"
    );
    assert_eq!(h.ev.subscriptions("selected"), strs(&["broken"]));
    assert_eq!(h.get("selected"), "7", "the last successful value survives");
    let after =
        h.ev.graph()
            .get(h.ev.form_of("selected").expect("f"))
            .map(|r| {
                r.edges
                    .iter()
                    .map(|e| (e.slot.id(), e.version))
                    .collect::<Vec<_>>()
            });
    assert_eq!(
        committed, after,
        "edges and read versions survive the abort"
    );
    h.take();
    // Repairing broken wakes selected through the subscription.
    h.ok("upd root 2");
    assert_eq!(h.get("broken"), "1/2");
    assert_eq!(h.state("selected"), FormState::Recomputed);
    assert_eq!(h.get("selected"), "1/2");
    assert_eq!(h.ev.edges("selected"), strs(&["broken", "switch"]));
    assert!(h.ev.subscriptions("selected").is_empty());
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![("d1".to_string(), "1/2".to_string())]
    );
}

#[test]
fn status_recovery_to_an_equal_value_still_reruns_blocked_dependents() {
    let mut h = Harness::new();
    h.ok("var root 1\nvar switch false\nlet broken / 1 root\nlet total d1 {+ broken 1}\nlet selected d2 {if switch broken 7}");
    h.ok("upd root 0");
    h.ok("upd switch true");
    assert_eq!(h.blocked_on("total").as_deref(), Some("broken"));
    assert_eq!(h.blocked_on("selected").as_deref(), Some("broken"));
    h.take();
    // 1 -> 0 -> 1: broken recovers to its retained value 1 (equal), yet the
    // Failed -> Recomputed status event re-runs total and the subscriber.
    h.ok("upd root 1");
    assert_eq!(h.get("broken"), "1");
    assert_eq!(h.state("broken"), FormState::Recomputed);
    assert_eq!(h.state("total"), FormState::Recomputed);
    assert_eq!(h.state("selected"), FormState::Recomputed);
    assert_eq!(
        (h.get("total"), h.get("selected")),
        ("2".into(), "1".into())
    );
    let fx = h.take();
    let mut binds = Harness::binds(&fx);
    binds.sort();
    assert_eq!(
        binds,
        vec![
            ("d1".to_string(), "2".to_string()),
            ("d2".to_string(), "1".to_string())
        ]
    );
}

#[test]
fn newly_discovered_selector_wakes_through_the_attempt_edge_set() {
    // `broken` fails through a pass: a standalone `/ 1 0` records nothing.
    let mut h = Harness::new();
    h.ok("var root 1\nlet broken / 1 root\nvar a false\nvar b true\nlet selected d1 {if a {if b broken 7} 5}");
    h.ok("upd root 0");
    assert_eq!(h.failed("broken"), Some(FailCode::DivisionByZero));
    assert_eq!(h.get("selected"), "5");
    assert_eq!(h.ev.edges("selected"), strs(&["a"]));
    h.ok("upd a true");
    assert_eq!(h.blocked_on("selected").as_deref(), Some("broken"));
    assert_eq!(h.ev.attempt_edges("selected"), strs(&["a", "b"]));
    assert_eq!(h.ev.subscriptions("selected"), strs(&["broken"]));
    h.take();
    // `b` is in no committed edge set and no subscription.
    h.ok("upd b false");
    assert_eq!(h.get("selected"), "7");
    assert_eq!(h.state("selected"), FormState::Recomputed);
    assert!(
        h.ev.attempt_edges("selected").is_empty(),
        "cleared on commit"
    );
    assert!(h.ev.subscriptions("selected").is_empty());
    assert_eq!(h.ev.edges("selected"), strs(&["a", "b"]));
    assert_eq!(
        h.failed("broken"),
        Some(FailCode::DivisionByZero),
        "unrepaired"
    );
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![("d1".to_string(), "7".to_string())]
    );
}

#[test]
fn ordinary_failure_recovers_through_the_attempt_edge_set() {
    let mut h = Harness::new();
    h.ok("var a false\nvar b 0\nlet selected d1 {if a {/ 1 b} 5}");
    assert_eq!(h.ev.edges("selected"), strs(&["a"]));
    h.take();
    h.ok("upd a true");
    assert_eq!(h.failed("selected"), Some(FailCode::DivisionByZero));
    assert_eq!(h.get("selected"), "5", "retained");
    assert_eq!(h.ev.attempt_edges("selected"), strs(&["a", "b"]));
    assert!(
        h.ev.subscriptions("selected").is_empty(),
        "every read's owner stayed healthy"
    );
    assert_eq!(h.ev.edges("selected"), strs(&["a"]));
    let fx = h.take();
    assert!(
        Harness::binds(&fx).is_empty(),
        "the failed attempt published no write"
    );
    // `b` is in no committed edge set and no subscription.
    h.ok("upd b 1");
    assert_eq!(h.get("selected"), "1");
    assert_eq!(h.state("selected"), FormState::Recomputed);
    assert_eq!(h.ev.edges("selected"), strs(&["a", "b"]));
    assert!(h.ev.attempt_edges("selected").is_empty());
    let fx = h.take();
    assert_eq!(
        Harness::binds(&fx),
        vec![("d1".to_string(), "1".to_string())]
    );
}

#[test]
fn a_repeated_abort_replaces_the_attempt_metadata() {
    let mut h = Harness::new();
    h.ok("var root 1\nlet broken / 1 root\nlet broken2 / 2 root\nvar a false\nvar b true\nlet selected d1 {if a {if b broken broken2} 5}");
    h.ok("upd root 0");
    h.ok("upd a true");
    assert_eq!(h.ev.attempt_edges("selected"), strs(&["a", "b"]));
    assert_eq!(h.ev.subscriptions("selected"), strs(&["broken"]));
    h.ok("upd b false");
    assert_eq!(h.blocked_on("selected").as_deref(), Some("broken2"));
    assert_eq!(
        h.ev.subscriptions("selected"),
        strs(&["broken2"]),
        "replaced"
    );
    assert_eq!(h.get("selected"), "5");
    // Repairing both wakes it; the commit clears the metadata.
    h.ok("upd root 4");
    assert_eq!(h.get("selected"), "1/2");
    assert!(h.ev.attempt_edges("selected").is_empty());
    assert!(h.ev.subscriptions("selected").is_empty());
}
