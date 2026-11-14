//! TASK-007 criterion 9: the dry run (design 11.5, 10.4). A successful and
//! a failed dry run leave the namespace, the slots, staging, the pending
//! queues and every host output unchanged (a snapshot compare); a failed
//! bind keeps the old binding playing; dry-run `print` output appears in
//! the report only.

use std::rc::Rc;

use super::{pat_of, Rig};
use crate::ns::stage::SlotKey;
use crate::pattern::eval::{InputCells, QueryVm};
use crate::sched::dryrun::dry_run;
use crate::sched::slots::Binding;
use crate::value::intern::intern_sym;
use crate::vm::fail::FailCode;
use crate::vm::query_vm::VmQuery;

/// Everything a dry run must not touch, printed.
fn snapshot(rig: &Rig) -> String {
    let mut s = String::new();
    for name in ["x", "noisy"] {
        let v = rig.ev.ns().session_value(name);
        s += &format!("{name}={v:?};");
    }
    for slot in rig.rt.slots().iter() {
        s += &format!(
            "{:?} gen {} pending {} bound {} lanes {};",
            slot.key,
            slot.gen,
            slot.pending.is_some(),
            slot.bound.is_some(),
            slot.lanes().len()
        );
        for l in slot.lanes() {
            s += &format!(
                "lane {} q {:?} recs {} held {} ledger {};",
                l.gen,
                l.queried_to,
                l.staging.records().len(),
                l.staging.fragments().len(),
                l.ledger.len()
            );
        }
    }
    s += &format!(
        "audio {} midi {} osc {} render {} queued {} at {};",
        rig.audio.calls().len(),
        rig.midi.calls().len(),
        rig.osc.calls().len(),
        rig.render.calls().len(),
        rig.rt.queued(),
        rig.rt.at.len()
    );
    s
}

fn dry(rig: &mut Rig, src: &str) -> crate::pattern::query::QueryResult {
    let p = pat_of(rig, src);
    let (vm, ns) = rig.ev.vm_and_ns();
    let before = vm.effects().len();
    let mut h = VmQuery::new(vm, ns);
    let cells = InputCells::new();
    let r = dry_run(
        &Binding::Pattern(Rc::new(p)),
        &mut h as &mut dyn QueryVm,
        &cells,
        0,
    );
    assert_eq!(
        rig.ev.vm_and_ns().0.effects().len(),
        before,
        "nothing staged"
    );
    r
}

#[test]
fn successful_and_failed_dry_runs_leave_everything_unchanged() {
    let mut rig = Rig::new();
    rig.run("var x 0\nfn noisy t:\n\tprint \"dry\" x\n\t0.5\nfn bad t:\n\tupd x 9\n\t0.5");
    rig.run("s [:bd :sd] > d1");
    rig.run_to(0.7);
    let before = snapshot(&rig);
    let ok = dry(&mut rig, "s [:bd :sd] > gain noisy");
    assert!(ok.faults.is_empty());
    assert_eq!(ok.events.len(), 2);
    assert_eq!(ok.output.len(), 2, "print captured into the report");
    assert_eq!(snapshot(&rig), before);
    let failed = dry(&mut rig, "s [:bd :sd] > gain bad");
    assert!(failed
        .faults
        .iter()
        .all(|f| f.code == FailCode::EffectInQuery));
    assert_eq!(failed.faults.len(), 2);
    assert_eq!(snapshot(&rig), before);
    let x = rig.ev.ns().session_slot(intern_sym("x")).expect("x").get();
    assert_eq!(x.to_string(), "0", "the global write failed inside");
    let unknown = dry(&mut rig, "s [:bd :nope]");
    assert_eq!(unknown.faults.len(), 1);
    assert_eq!(snapshot(&rig), before);
}

#[test]
fn a_failed_bind_keeps_the_old_binding_and_its_print_stays_in_the_report() {
    let mut rig = Rig::new();
    rig.run("fn noisy t:\n\tprint \"dry\"\n\t0.5\ns :bd > d1");
    rig.run_to(0.5);
    let gen = rig.rt.slot_gen(SlotKey::D(1));
    let rep = rig.run("s [:bd :nope] > gain noisy > d1");
    assert_eq!(rep.faults.len(), 1);
    assert_eq!(rep.faults[0].code, FailCode::UnknownSound);
    assert!(rep.dry_output.iter().any(|(_, s)| &**s == "dry"));
    assert_eq!(rig.rt.slot_gen(SlotKey::D(1)), gen, "no rebind");
    let slot = rig.rt.slots().get(SlotKey::D(1)).expect("d1");
    assert!(slot.pending.is_none());
    rig.run_to(4.1);
    // The old binding keeps playing: one :bd per cycle, gen 1.
    let sent: Vec<(u32, f64)> = rig.sent().iter().map(|(_, e)| (e.gen, e.time)).collect();
    assert_eq!(sent, vec![(1, 0.0), (1, 2.0), (1, 4.0)]);
    assert!(rig.console().iter().all(|l| l != "dry"));
}
