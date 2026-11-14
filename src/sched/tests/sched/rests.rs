//! Rest source intervals are visible, but never produce notes or hit signals.

use super::{Rig, DT};
use crate::sched::telemetry::PlayingEvent;
use crate::value::intern::intern_kw;

fn collect(source: &str, end: f64) -> (Rig, Vec<PlayingEvent>, Vec<PlayingEvent>) {
    let mut rig = Rig::new();
    rig.run(source);
    let mut committed = Vec::new();
    let mut ahead = Vec::new();
    let mut now = 0.0;
    while now <= end + 1e-9 {
        let report = rig.tick_at(now);
        assert!(report.faults.is_empty(), "{:?}", report.faults);
        committed.extend(rig.rt.telemetry());
        ahead.extend(rig.rt.announced().ahead);
        now += DT;
    }
    (rig, committed, ahead)
}

#[test]
fn nil_steps_publish_their_own_spans_and_never_make_sound_or_hits() {
    let source = "s [nil nil] > d1";
    let (rig, committed, ahead) = collect(source, 1.1);
    assert!(rig.sent().is_empty());
    assert!(rig.rt.playback_active());
    assert_eq!(rig.rt.input.hits(intern_kw("d1")), 0.0);
    assert_eq!(committed.len(), 2);
    for (event, start) in committed.iter().zip([0.0, 1.0]) {
        assert!(event.rest);
        assert!((event.time - start).abs() < 1e-9);
        assert!((event.dur - 1.0).abs() < 1e-9);
        let span = event.src.expect("nil source").span;
        assert_eq!(&source[span.start as usize..span.end as usize], "nil");
    }
    assert!(ahead
        .iter()
        .any(|preview| preview.rest && preview.id == committed[1].id));
}

#[test]
fn nested_fast_reversed_rests_follow_the_same_timing_as_notes() {
    // One cycle is two seconds; fast doubles it. Reversal puts the nested
    // final nil at zero and the first nil at the end of each one-second cycle.
    let source = "s [nil [:bd nil]] > rev > fast 2 > d1";
    let (rig, committed, _) = collect(source, 0.9);
    let rests: Vec<_> = committed.iter().filter(|event| event.rest).collect();
    assert_eq!(rests.len(), 2);
    assert!((rests[0].time - 0.0).abs() < 1e-9);
    assert!((rests[0].dur - 0.25).abs() < 1e-9);
    assert!((rests[1].time - 0.5).abs() < 1e-9);
    assert!((rests[1].dur - 0.5).abs() < 1e-9);
    assert_eq!(rig.sent().len(), 1);
    assert!((rig.sent()[0].1.time - 0.25).abs() < 1e-9);
}

#[test]
fn reused_source_at_different_speeds_preserves_both_rest_durations() {
    let (rig, committed, _) = collect("let p s [nil :bd]\nstack [p {slow p 2}] > d1", 1.1);
    let mut rests: Vec<_> = committed.iter().filter(|event| event.rest).collect();
    rests.sort_by(|a, b| a.dur.total_cmp(&b.dur));
    assert_eq!(rests.len(), 2, "{committed:?}");
    assert!(rests.iter().all(|event| event.time.abs() < 1e-9));
    assert!((rests[0].dur - 1.0).abs() < 1e-9);
    assert!((rests[1].dur - 2.0).abs() < 1e-9);
    assert_eq!(rig.sent().len(), 1);
}

#[test]
fn control_rests_remain_absent_from_computational_samples() {
    // The structured gain pattern supplies timing. Its nil suppresses the
    // second note, and must not be converted into a control or sound value.
    let (rig, committed, _) = collect("s :bd > gain [1 nil] > d1", 1.1);
    assert_eq!(rig.sent().len(), 1);
    assert_eq!(committed.iter().filter(|event| !event.rest).count(), 1);
    assert_eq!(committed.iter().filter(|event| event.rest).count(), 1);
    // The note at zero has left the one-second window; the nil at one
    // must not add another hit.
    assert_eq!(rig.rt.input.hits(intern_kw("d1")), 0.0);
}

#[test]
fn euclidean_and_segment_sampling_project_rests_onto_structural_intervals() {
    let (rig, committed, _) = collect("s [nil :bd] > euclid 1 4 > d1", 1.1);
    assert!(rig.sent().is_empty());
    assert_eq!(committed.len(), 1);
    assert!(committed[0].rest);
    assert!((committed[0].time - 0.0).abs() < 1e-9);
    assert!((committed[0].dur - 0.5).abs() < 1e-9);

    let (rig, committed, _) = collect("s [nil :bd] > segment 4 > d1", 1.7);
    let rests: Vec<_> = committed.iter().filter(|event| event.rest).collect();
    assert_eq!(rests.len(), 2);
    assert!((rests[0].time - 0.0).abs() < 1e-9);
    assert!((rests[1].time - 0.5).abs() < 1e-9);
    assert!(rests.iter().all(|event| (event.dur - 0.5).abs() < 1e-9));
    assert_eq!(rig.sent().len(), 2);
}

#[test]
fn rejected_probability_branches_leave_no_rest_highlights() {
    for source in [
        "s [nil :bd] > maybe 0 > d1",
        "s [nil :bd] > degrade-by 1 > d1",
    ] {
        let (rig, committed, ahead) = collect(source, 1.7);
        assert!(rig.sent().is_empty());
        assert!(committed.is_empty(), "{source}: {committed:?}");
        assert!(ahead.is_empty(), "{source}: {ahead:?}");
    }
    let (rig, committed, _) = collect("s [nil :bd] > sometimes-by 0 {p -> fast p 2} > d1", 1.1);
    let rests: Vec<_> = committed.iter().filter(|event| event.rest).collect();
    assert_eq!(rests.len(), 1, "{committed:?}");
    assert!((rests[0].dur - 1.0).abs() < 1e-9);
    assert_eq!(rig.sent().len(), 1);
}

#[test]
fn chunk_keeps_only_rest_intervals_from_the_selected_subregions() {
    let (rig, committed, _) = collect("s [nil :bd nil :sd] > chunk 4 {p -> fast p 2} > d1", 1.6);
    let rests: Vec<_> = committed.iter().filter(|event| event.rest).collect();
    assert_eq!(rests.len(), 2, "{committed:?}");
    assert!((rests[0].time - 0.0).abs() < 1e-9);
    assert!((rests[0].dur - 0.25).abs() < 1e-9);
    assert!((rests[1].time - 1.0).abs() < 1e-9);
    assert!((rests[1].dur - 0.5).abs() < 1e-9);
    assert_eq!(rig.sent().len(), 3);
}

fn playback_query(source: &str) -> crate::pattern::query::QueryResult {
    use crate::pattern::eval::{InputCells, QueryCtx};
    use crate::pattern::query::{query_playback, TimeSpan};
    use crate::vm::query_vm::VmQuery;

    let mut rig = Rig::new();
    let pattern = super::pat_of(&mut rig, source);
    let (vm, ns) = rig.ev.vm_and_ns();
    let mut vm = VmQuery::new(vm, ns);
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, 0);
    query_playback(&pattern, TimeSpan::cycle(0).unwrap(), &mut cx)
}

#[test]
fn failed_off_branch_preserves_only_its_successful_original_sibling() {
    let result = playback_query("s [nil :bd] > off 0.25 {p -> / 1 0}");
    assert!(!result.faults.is_empty());
    // Off preserves its original branch when the added transform fails.
    assert_eq!(result.events.len(), 2, "{:?}", result.events);
    assert_eq!(
        result
            .events
            .iter()
            .filter(|e| matches!(e.value, crate::value::value::Value::Nil))
            .count(),
        1
    );
}

#[test]
fn failed_structural_sample_discards_rests_from_its_discarded_content() {
    let result = playback_query("stack [{s [nil :bd]} {s :not-a-sound}] > segment 4");
    assert!(!result.faults.is_empty());
    assert!(result.events.is_empty(), "{:?}", result.events);
}

#[test]
fn muting_retracts_pending_rest_and_stop_removes_future_intervals() {
    let mut rig = Rig::new();
    rig.run("s [nil nil nil nil] > d1");
    let mut pending = None;
    for step in 0..40 {
        rig.tick_at(f64::from(step) * DT);
        let _ = rig.rt.telemetry();
        for event in rig.rt.announced().ahead {
            if event.time > 0.4 {
                pending = event.id;
            }
        }
    }
    let id = pending.expect("upcoming nil preview");
    rig.rt.slots.iter_mut().next().unwrap().muted = true;
    rig.tick_at(0.4);
    assert!(rig.rt.announced().retract.contains(&id));
    assert!(rig.rt.telemetry().is_empty());
    rig.run("stop :d1");
    rig.run_to(2.0);
    assert!(rig.rt.telemetry().is_empty());
    assert!(rig.rt.announced().ahead.is_empty());
    assert!(rig.sent().is_empty());
}
