//! TASK-007 criterion 8: captured `print` output reaches the console only
//! at its event's commit point (design 10.4, 11.3 step (4)); a restage
//! before commit replaces the held output; a write after commit leaves the
//! emitted output untouched. One cycle is 2 s: steps every 0.5 s.

use super::Rig;

const SRC: &str =
    "var x 0\nfn loud t:\n\tprint \"q\" x\n\t0.5\ns [:bd :sd :hh :cp] > gain loud > d1";

#[test]
fn a_print_reaches_the_console_once_at_commit() {
    let mut rig = Rig::new();
    let rep = rig.run(SRC);
    // The bind's dry run printed into its report only.
    assert!(!rep.dry_output.is_empty());
    assert!(rep.console.is_empty());
    rig.run_to(0.9);
    // Events 0.0 and 0.5 committed; 1.0 is staged (queried at ~0.88 s)
    // with its output held, and many continuation windows re-queried the
    // events without adding lines.
    assert_eq!(rig.console(), vec!["q 0", "q 0"]);
    let n = rig.console().len();
    rig.run_to(0.96);
    assert_eq!(rig.console().len(), n, "held until the 0.97 s commit");
    rig.run_to(0.97);
    assert_eq!(rig.console(), vec!["q 0", "q 0", "q 0"]);
}

#[test]
fn a_restage_before_commit_replaces_held_output_and_a_later_write_does_not() {
    let mut rig = Rig::new();
    rig.run(SRC);
    rig.run_to(0.9);
    // The 1.0 s event is staged with "q 0" held: the write restages it.
    rig.run("upd x 1");
    rig.run_to(0.98);
    // Committed at 0.97 s with the restaged output only.
    assert_eq!(rig.console(), vec!["q 0", "q 0", "q 1"]);
    // A write after commit: the emitted line stays; later events print the
    // new value.
    rig.run("upd x 2");
    rig.run_to(1.6);
    assert_eq!(rig.console(), vec!["q 0", "q 0", "q 1", "q 2"]);
}
