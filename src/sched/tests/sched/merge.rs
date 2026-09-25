//! TASK-007 criterion 1: exact positions, boundary-split queries, region
//! partition invariance through staging, and the occurrence merge (11.3
//! layer 2): emission uniqueness and onset preservation, asserted as
//! EMITTED MULTIPLICITY at the recording audio host. Tempo 120 bpm, 4
//! beats per cycle: one cycle is 2 s, so cycle position `c` sounds at
//! `2c` seconds.

use super::{cval, pat_of, r, span, Rig, DT};
use crate::ns::stage::SlotKey;
use crate::pattern::combinators::structure::stack;
use crate::pattern::eval::{QueryCtx, QueryVm};
use crate::pattern::query::{query, TimeSpan};
use crate::value::value::Value;
use crate::vm::query_vm::VmQuery;
use std::collections::BTreeMap;

const D1: SlotKey = SlotKey::D(1);

/// Advances the clock to `end` committing only (no new queries).
fn commit_through(rig: &mut Rig, end: f64) {
    let mut t = rig.clock.now();
    loop {
        rig.clock.set(t);
        let rep = rig.rt.commit_only(t);
        rig.ticks.push(rep);
        if t + DT > end + 1e-9 {
            break;
        }
        t += DT;
    }
}

/// How many events were sent for each onset time (ms).
fn multiplicity(rig: &Rig) -> BTreeMap<i64, usize> {
    let mut m = BTreeMap::new();
    for (_, e) in rig.sent() {
        #[allow(clippy::cast_possible_truncation)]
        let ms = (e.time * 1000.0).round() as i64;
        *m.entry(ms).or_insert(0) += 1;
    }
    m
}

fn ms(pairs: &[(i64, usize)]) -> BTreeMap<i64, usize> {
    pairs.iter().copied().collect()
}

/// Binds `src` at time 0 without ticking.
fn bound(src: &str) -> Rig {
    let mut rig = Rig::new();
    let rep = rig.run(src);
    assert!(rep.faults.is_empty(), "{:?}", rep.faults);
    rig
}

#[test]
fn onsets_sit_at_exact_ratio_positions_and_queries_split_at_boundaries() {
    let mut rig = bound("s [:bd :sd :hh :cp] > d1");
    rig.run_to(4.2);
    // Two full cycles plus the first onset of the third: each once.
    let want: Vec<(i64, usize)> = (0..9).map(|i| (i * 500, 1)).collect();
    assert_eq!(multiplicity(&rig), ms(&want));
}

#[test]
fn a_whole_crossing_a_boundary_is_queried_in_pieces_and_emitted_once() {
    // `slow 2`: the whole [0,2) is queried as [0,1) and [1,2) pieces (and
    // many tick-sized ones); the record merges them into one emission.
    let mut rig = bound("s :bd > slow 2 > d1");
    rig.run_to(4.2);
    assert_eq!(multiplicity(&rig), ms(&[(0, 1), (4000, 1)]));
    let slot = rig.rt.slots().get(D1).expect("d1");
    let covered: Vec<_> = slot
        .lanes()
        .iter()
        .flat_map(|l| l.staging.records().values())
        .map(|rec| rec.covered.spans().to_vec())
        .collect();
    assert!(covered.iter().all(|c| !c.is_empty()));
}

/// `(onset ms, begin, end)` of every committed event.
fn regions(rig: &Rig) -> Vec<(i64, i64, i64)> {
    let mut v: Vec<(i64, i64, i64)> = rig
        .sent()
        .iter()
        .map(|(_, e)| {
            #[allow(clippy::cast_possible_truncation)]
            (
                (e.time * 1000.0).round() as i64,
                (cval(e, "begin").unwrap_or(0.0) * 1000.0).round() as i64,
                (cval(e, "end").unwrap_or(1.0) * 1000.0).round() as i64,
            )
        })
        .collect();
    v.sort_unstable();
    v
}

/// The full-cycle query of cycle `c` of the bound pattern, as regions.
fn full_cycle(rig: &mut Rig, src: &str, c: i64) -> Vec<(i64, i64, i64)> {
    let p = pat_of(rig, src);
    let (vm, ns) = rig.ev.vm_and_ns();
    let mut h = VmQuery::new(vm, ns);
    let cells = crate::pattern::eval::InputCells::new();
    let mut cx = QueryCtx::new(&mut h as &mut dyn QueryVm, &cells, 0);
    let res = query(&p, TimeSpan::cycle(c).expect("cycle"), &mut cx);
    let num = |v: Option<&Value>, d: f64| v.and_then(crate::pattern::eval::num_f64).unwrap_or(d);
    let mut v: Vec<(i64, i64, i64)> = res
        .events
        .iter()
        .filter(|e| e.is_onset())
        .map(|e| {
            let b = e.controls.get(&crate::value::intern::intern_kw("begin"));
            let en = e.controls.get(&crate::value::intern::intern_kw("end"));
            #[allow(clippy::cast_possible_truncation)]
            (
                (e.anchor().to_f64() * 2000.0).round() as i64,
                (num(b, 0.0) * 1000.0).round() as i64,
                (num(en, 1.0) * 1000.0).round() as i64,
            )
        })
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn chop_and_striate_staged_incrementally_equal_the_full_cycle_query() {
    for src in ["s :bd > chop 4", "s [:bd :sd] > striate 2"] {
        let mut rig = Rig::new();
        rig.run("var g 0.5");
        let bind = format!("{src} > gain g > d1");
        assert!(rig.run(&bind).faults.is_empty());
        // Cycle 1 is staged across ~200 ticks and boundary splits, and a
        // control write mid-cycle invalidates and re-stages everything
        // uncommitted.
        rig.run_to(2.5);
        rig.run("upd g 0.8");
        rig.run_to(3.99);
        let got: Vec<(i64, i64, i64)> = regions(&rig)
            .into_iter()
            .filter(|(t, _, _)| (2000..4000).contains(t))
            .collect();
        let want = full_cycle(&mut rig, src, 1);
        assert!(!want.is_empty());
        assert_eq!(got, want, "{src}");
    }
}

#[test]
fn overlapping_windows_staged_before_commit_emit_each_onset_once() {
    let mut rig = bound("s [:bd :sd :hh :cp] > d1");
    rig.rt.stage_span(&mut rig.ev, D1, span(r(0, 1), r(3, 4)));
    rig.rt.stage_span(&mut rig.ev, D1, span(r(1, 4), r(1, 1)));
    commit_through(&mut rig, 2.0);
    assert_eq!(
        multiplicity(&rig),
        ms(&[(0, 1), (500, 1), (1000, 1), (1500, 1)])
    );
}

#[test]
fn a_partially_committed_window_never_re_emits_through_the_ledger() {
    let mut rig = bound("s [:bd :sd :hh :cp] > d1");
    rig.rt.stage_span(&mut rig.ev, D1, span(r(0, 1), r(3, 4)));
    // Onsets 0, 1/4 and 1/2 (0 s, 0.5 s, 1 s) commit before the second
    // window arrives.
    commit_through(&mut rig, 1.0);
    assert_eq!(multiplicity(&rig), ms(&[(0, 1), (500, 1), (1000, 1)]));
    rig.rt.stage_span(&mut rig.ev, D1, span(r(1, 4), r(1, 1)));
    commit_through(&mut rig, 2.0);
    assert_eq!(
        multiplicity(&rig),
        ms(&[(0, 1), (500, 1), (1000, 1), (1500, 1)])
    );
}

#[test]
fn onsets_survive_clipping_in_both_query_orders() {
    for order in [[(4, 7), (5, 8)], [(5, 8), (4, 7)]] {
        let mut rig = bound("s :bd > chop 2 > d1");
        // Source whole [1,2): child wholes [1,3/2) and [3/2,2). Windows
        // [1,7/4) and [5/4,2), both staged before any commit.
        for (b, e) in order {
            rig.rt.stage_span(&mut rig.ev, D1, span(r(b, 4), r(e, 4)));
        }
        commit_through(&mut rig, 4.0);
        assert_eq!(multiplicity(&rig), ms(&[(2000, 1), (3000, 1)]), "{order:?}");
    }
}

#[test]
fn boundary_crossing_fragments_merge_into_one_emission_either_way() {
    let onset = span(r(0, 1), r(1, 4));
    let rest = span(r(1, 4), r(1, 1));
    for (first, second) in [(onset, rest), (rest, onset)] {
        let mut rig = bound("s [:bd :sd] > d1");
        // The whole [0,1/2) crosses 1/4: its onset fragment and its
        // continuation arrive in disjoint adjacent spans.
        rig.rt.stage_span(&mut rig.ev, D1, first);
        rig.rt.stage_span(&mut rig.ev, D1, second);
        commit_through(&mut rig, 2.0);
        assert_eq!(multiplicity(&rig), ms(&[(0, 1), (1000, 1)]), "{first:?}");
    }
}

#[test]
fn partial_invalidation_keeps_onsets_outside_the_span() {
    // chop 2: invalidating [5/4,2) clips child 0 (onset 1 outside the
    // span, never dropped) and re-extends child 1.
    let mut rig = bound("s :bd > chop 2 > d1");
    rig.rt.stage_span(&mut rig.ev, D1, span(r(1, 1), r(2, 1)));
    rig.rt.invalidate(D1, span(r(5, 4), r(2, 1)));
    rig.rt.requery_dirty(&mut rig.ev, D1);
    commit_through(&mut rig, 4.0);
    assert_eq!(multiplicity(&rig), ms(&[(2000, 1), (3000, 1)]));

    // Removal is scoped: after `maybe` drops everything, only records whose
    // onset lies in the invalidated span and whose key is gone are dropped.
    let mut rig = Rig::new();
    rig.run("var p 1");
    assert!(rig
        .run("maybe {s [:bd :sd :hh :cp]} p > d1")
        .faults
        .is_empty());
    rig.rt.stage_span(&mut rig.ev, D1, span(r(1, 1), r(2, 1)));
    rig.ev.queue_upd("p", Value::Int(0)).expect("upd");
    rig.ev.run_pass(); // released, not drained: only the scoped span below
    rig.rt.invalidate(D1, span(r(3, 2), r(2, 1)));
    rig.rt.requery_dirty(&mut rig.ev, D1);
    commit_through(&mut rig, 4.0);
    assert_eq!(multiplicity(&rig), ms(&[(2000, 1), (2500, 1)]));
}

#[test]
fn a_repeated_window_commits_nothing_twice() {
    let mut rig = bound("s [:bd :sd :hh :cp] > d1");
    for _ in 0..3 {
        rig.rt.stage_span(&mut rig.ev, D1, span(r(0, 1), r(1, 1)));
    }
    commit_through(&mut rig, 1.0);
    rig.rt.stage_span(&mut rig.ev, D1, span(r(0, 1), r(1, 1)));
    commit_through(&mut rig, 2.0);
    assert_eq!(
        multiplicity(&rig),
        ms(&[(0, 1), (500, 1), (1000, 1), (1500, 1)])
    );
}

#[test]
fn identical_stack_twins_both_commit() {
    let mut rig = Rig::new();
    let a = pat_of(&mut rig, "s :bd");
    let p = stack(vec![a.clone(), a], None);
    assert!(rig.bind(D1, p).faults.is_empty());
    rig.run_to(4.0);
    assert_eq!(multiplicity(&rig), ms(&[(0, 2), (2000, 2), (4000, 2)]));
}

/// The (OccKey, generation, begin, end) of the single record at onset 1.
fn record_at_1(rig: &Rig) -> (crate::pattern::occ::OccKey, u32, Value, Value) {
    let slot = rig.rt.slots().get(D1).expect("d1");
    let lane = slot.lanes().last().expect("a lane");
    let recs: Vec<_> = lane
        .staging
        .records()
        .values()
        .filter(|rec| rec.whole.begin == r(1, 1))
        .collect();
    assert_eq!(recs.len(), 1, "{recs:?}");
    let begin = crate::value::intern::intern_kw("begin");
    let end = crate::value::intern::intern_kw("end");
    (
        recs[0].key.clone(),
        lane.gen,
        recs[0].payload.controls[&begin].clone(),
        recs[0].payload.controls[&end].clone(),
    )
}

#[test]
fn semantic_invalidation_replaces_the_payload_of_the_same_occurrence() {
    let mut rig = Rig::new();
    rig.run("var i 0");
    assert!(rig.run("s :bd > slice 2 i > d1").faults.is_empty());
    rig.rt.stage_span(&mut rig.ev, D1, span(r(1, 1), r(2, 1)));
    let (key0, gen0, b0, _) = record_at_1(&rig);
    assert_eq!(b0.to_string(), "0");
    rig.ev.queue_upd("i", Value::Int(1)).expect("upd");
    rig.ev.run_pass();
    rig.rt.invalidate(D1, span(r(1, 1), r(2, 1)));
    rig.rt.requery_dirty(&mut rig.ev, D1);
    let (key1, gen1, b1, e1) = record_at_1(&rig);
    assert_eq!((key0, gen0), (key1, gen1));
    assert_eq!((b1.to_string(), e1.to_string()), ("1/2".into(), "1".into()));
    commit_through(&mut rig, 4.0);
    let sent: Vec<(i64, i64, i64)> = regions(&rig)
        .into_iter()
        .filter(|(t, _, _)| *t == 2000)
        .collect();
    assert_eq!(sent, vec![(2000, 500, 1000)]);
    // No emission ever carried the old region [0,1/2).
    assert!(regions(&rig).iter().all(|(_, b, _)| *b != 0));
}

#[test]
fn invalidating_only_a_continuation_keeps_the_onset_payload() {
    let mut rig = Rig::new();
    rig.run("var i 0");
    assert!(rig.run("s :bd > slice 2 i > d1").faults.is_empty());
    rig.rt.stage_span(&mut rig.ev, D1, span(r(1, 1), r(2, 1)));
    rig.ev.queue_upd("i", Value::Int(1)).expect("upd");
    rig.ev.run_pass();
    rig.rt.invalidate(D1, span(r(3, 2), r(2, 1)));
    rig.rt.requery_dirty(&mut rig.ev, D1);
    let (_, _, b, e) = record_at_1(&rig);
    assert_eq!((b.to_string(), e.to_string()), ("0".into(), "1/2".into()));
    commit_through(&mut rig, 4.0);
    let sent: Vec<(i64, i64, i64)> = regions(&rig)
        .into_iter()
        .filter(|(t, _, _)| *t == 2000)
        .collect();
    assert_eq!(sent, vec![(2000, 0, 500)]);
}
