//! TASK-007 criterion 2: the deadline-aware rebind (design 11.3), three
//! cases with an explicit cycle duration (2 s: 120 bpm, 4 beats per cycle),
//! commit lead 30 ms, tick 10 ms and a stated control delivery delay `D`.
//! The rebind is applied at `now`; its boundary `B` is cycle 1 (2.0 s).

use super::{pat_of, played, started, Rig, DT};
use crate::ns::stage::SlotKey;
use crate::pattern::combinators::structure::stack;
use crate::types::diag::DiagCode;

const D1: SlotKey = SlotKey::D(1);
const B: f64 = 2.0;
const LEAD: f64 = 0.030;

/// The generation of the first binding of `d1`, and of the rebind.
const OLD: u32 = 1;
const NEW: u32 = 2;

#[test]
fn sufficient_lead_swaps_exactly_at_the_boundary_with_full_lead() {
    // B - now = 0.5 s >= commit_lead + L_ctl (0.03 + 0.01).
    let l_ctl = 0.01;
    let mut rig = Rig::new();
    rig.ack_delay = Some(l_ctl);
    assert!(rig.run("s [:bd :bd :bd :bd] > d1").faults.is_empty());
    rig.run_to(1.5);
    assert!(rig.run("s [:sd :sd :sd :sd] > d1").faults.is_empty());
    rig.run_to(3.0);
    let voices = started(&played(&rig, l_ctl, &[]));
    // Zero old-pattern events at or past the boundary, even committed ones.
    assert!(rig
        .sent()
        .iter()
        .all(|(_, e)| e.gen != OLD || e.time < B - 1e-9));
    assert!(voices
        .iter()
        .all(|v| v.ev.gen == NEW || v.ev.time < B - 1e-9));
    // The old pattern owns the current cycle: its 1.5 s event played.
    assert!(voices
        .iter()
        .any(|v| v.ev.gen == OLD && (v.ev.time - 1.5).abs() < 1e-9));
    // The new boundary event exists once, committed with the full lead.
    let at_b: Vec<_> = rig
        .sent()
        .into_iter()
        .filter(|(_, e)| (e.time - B).abs() < 1e-9)
        .collect();
    assert_eq!(at_b.len(), 1);
    let (arrival, e) = at_b[0];
    assert_eq!(e.gen, NEW);
    assert!(B - arrival >= LEAD - DT - 1e-9, "lead {}", B - arrival);
    let tel = rig.rt.telemetry();
    let first = tel
        .iter()
        .find(|p| (p.time - B).abs() < 1e-9)
        .expect("telemetry");
    assert!(!first.reduced_lead);
    // No missed deadline.
    assert!(rig
        .diags()
        .iter()
        .all(|d| d.code != DiagCode::HostTransport));
}

#[test]
fn insufficient_lead_commits_new_boundary_events_with_reduced_lead() {
    // B - now = 0.02 s < commit_lead 0.03: the old boundary event was
    // already committed; the control (L_ctl = 0.01) still lands before B.
    let l_ctl = 0.01;
    let mut rig = Rig::new();
    rig.ack_delay = Some(l_ctl);
    assert!(rig.run("s [:bd :bd :bd :bd] > d1").faults.is_empty());
    rig.run_to(1.98);
    let committed_old = rig
        .sent()
        .iter()
        .filter(|(_, e)| e.gen == OLD && (e.time - B).abs() < 1e-9)
        .count();
    assert_eq!(committed_old, 1, "the old boundary event was committed");
    assert!(rig.run("s [:sd :sd :sd :sd] > d1").faults.is_empty());
    rig.run_to(2.6);
    let voices = played(&rig, l_ctl, &[]);
    // The committed old boundary event is revoked (control landed first).
    let old_at_b: Vec<_> = voices
        .iter()
        .filter(|v| v.ev.gen == OLD && (v.ev.time - B).abs() < 1e-9)
        .collect();
    assert_eq!(old_at_b.len(), 1);
    assert!(old_at_b[0].start.is_none());
    // The new boundary event was committed (not dropped), flagged.
    let new_at_b: Vec<_> = voices
        .iter()
        .filter(|v| v.ev.gen == NEW && (v.ev.time - B).abs() < 1e-9)
        .collect();
    assert_eq!(new_at_b.len(), 1);
    assert!(new_at_b[0].start.is_some());
    let tel = rig.rt.telemetry();
    let first = tel
        .iter()
        .find(|p| (p.time - B).abs() < 1e-9 && p.reduced_lead)
        .expect("a reduced-lead boundary event");
    assert!(first.time <= B);
}

#[test]
fn control_delivery_failure_is_density_bounded_and_reported() {
    // D = 0.05 s exceeds the 0.02 s left before B: the three simultaneous
    // committed old boundary events all start and are each cut when the
    // control lands (an artifact of at most D).
    let d = 0.05;
    let mut rig = Rig::new();
    rig.ack_delay = Some(d);
    let a = pat_of(&mut rig, "s :bd");
    let b = pat_of(&mut rig, "s :sd");
    let c = pat_of(&mut rig, "s :hh");
    assert!(rig.bind(D1, stack(vec![a, b, c], None)).faults.is_empty());
    rig.run_to(1.98);
    // The new pattern rests at the boundary, so no newer batch arrives
    // before the control does.
    assert!(rig.run("s [nil :sd] > d1").faults.is_empty());
    rig.run_to(2.6);
    let voices = played(&rig, d, &[]);
    let artifacts: Vec<_> = voices
        .iter()
        .filter(|v| v.ev.gen == OLD && (v.ev.time - B).abs() < 1e-9)
        .collect();
    assert_eq!(artifacts.len(), 3);
    for v in &artifacts {
        let start = v.start.expect("stale-started");
        let cut = v.cut.expect("cut on control receipt");
        assert!(cut - start <= d + 1e-9, "artifact {}", cut - start);
    }
    // The missed deadline is reported.
    assert!(rig
        .diags()
        .iter()
        .any(|x| x.code == DiagCode::HostTransport && x.message.contains("missed its deadline")));
}
