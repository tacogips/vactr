//! Layer-1 cover equivalence (design 10.1): for any cover of a span by
//! adjacent, overlapping or repeated windows, the union of the results
//! deduplicated by `occ` equals the whole-span result occurrence for
//! occurrence.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::region::region;
use super::*;
use crate::pattern::combinators::region::{chop, fit, splice, striate};
use crate::pattern::combinators::structure::stack;
use crate::pattern::occ::OccKey;
use crate::pattern::pat::{PParam, SliceCuts};

/// What must agree between windows for one occurrence.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Occ {
    whole: TimeSpan,
    value: String,
    begin: Option<String>,
    end: Option<String>,
    speed_fit: Option<String>,
}

fn record(e: &Event) -> Occ {
    Occ {
        whole: e.whole.expect("region events have wholes"),
        value: name(&e.value),
        begin: ctl_of(e, "begin").map(show),
        end: ctl_of(e, "end").map(show),
        speed_fit: ctl_of(e, "speed-fit").map(show),
    }
}

/// Merges events by occurrence; a repeated occurrence must agree.
fn merge(into: &mut BTreeMap<OccKey, Occ>, onsets: &mut BTreeMap<OccKey, bool>, events: &[Event]) {
    for e in events {
        let rec = record(e);
        if let Some(prev) = into.get(&e.occ) {
            assert_eq!(prev, &rec, "one occurrence, two different records");
        }
        into.insert(e.occ.clone(), rec);
        let onset = onsets.entry(e.occ.clone()).or_insert(false);
        *onset |= e.is_onset();
    }
}

fn windows(p: &Pat, spans: &[TimeSpan]) -> (BTreeMap<OccKey, Occ>, BTreeMap<OccKey, bool>) {
    let mut occs = BTreeMap::new();
    let mut ons = BTreeMap::new();
    for sp in spans {
        let res = run(p, *sp);
        assert!(res.faults.is_empty(), "{:?}", res.faults);
        merge(&mut occs, &mut ons, &res.events);
    }
    (occs, ons)
}

fn assert_cover_equivalent(p: &Pat) {
    let full = windows(p, &[cycle(0)]);
    assert!(!full.0.is_empty());
    let covers = [
        vec![span(r(0, 1), r(1, 2)), span(r(1, 2), r(1, 1))],
        vec![span(r(0, 1), r(3, 4)), span(r(1, 4), r(1, 1))],
        vec![cycle(0), cycle(0)],
        vec![
            span(r(0, 1), r(1, 3)),
            span(r(1, 5), r(3, 5)),
            span(r(1, 2), r(7, 8)),
            span(r(5, 6), r(1, 1)),
        ],
    ];
    for cover in &covers {
        let got = windows(p, cover);
        assert_eq!(got.0, full.0, "cover {cover:?}");
        // Every occurrence starts exactly where the full query starts it.
        assert_eq!(got.1, full.1, "onsets under cover {cover:?}");
    }
}

fn abc() -> Rc<Pat> {
    s(list(vec![kw("a"), kw("b"), kw("c")]))
}

#[test]
fn cover_equivalence_matrix() {
    assert_cover_equivalent(&chop(abc(), PParam::int(2), None));
    assert_cover_equivalent(&chop(abc(), PParam::int(8), None));
    assert_cover_equivalent(&chop(s(kw("break")), PParam::int(8), None));
    assert_cover_equivalent(&striate(abc(), PParam::int(8), None));
    let idx = pat(list(vec![int(0), int(1), int(2), int(3)]));
    assert_cover_equivalent(&splice(
        abc(),
        SliceCuts::Equal(PParam::int(4)),
        Rc::clone(&idx),
        None,
    ));
    assert_cover_equivalent(&splice(
        s(kw("break")),
        SliceCuts::Equal(PParam::int(8)),
        idx,
        None,
    ));
    assert_cover_equivalent(&fit(abc(), None));
}

#[test]
fn astra_chop_2_counterexample() {
    // One source event with whole [0, 1) under chop 2.
    let p = chop(s(kw("break")), PParam::int(2), None);
    // Adjacent partition: exactly two notes, onsets 0 and 1/2, regions 0, 1.
    let a = run(&p, span(r(0, 1), r(1, 2))).events;
    let b = run(&p, span(r(1, 2), r(1, 1))).events;
    assert_eq!(a.len() + b.len(), 2, "never four");
    assert_eq!(a[0].whole, Some(span(r(0, 1), r(1, 2))));
    assert!(a[0].is_onset());
    assert_eq!(region(&a[0]), (r(0, 1), r(1, 2)));
    assert_eq!(b[0].whole, Some(span(r(1, 2), r(1, 1))));
    assert!(b[0].is_onset());
    assert_eq!(
        region(&b[0]),
        (r(1, 2), r(1, 1)),
        "the region does not restart"
    );
    // Overlapping re-query: the shared child comes back with the SAME key.
    let left = run(&p, span(r(0, 1), r(3, 4))).events;
    let right = run(&p, span(r(1, 4), r(1, 1))).events;
    let second_l = left.iter().find(|e| e.anchor() == r(1, 2)).unwrap();
    let second_r = right.iter().find(|e| e.anchor() == r(1, 2)).unwrap();
    assert_eq!(second_l.occ, second_r.occ);
    assert!(second_l.is_onset() && second_r.is_onset());
    assert_eq!(second_l.part, span(r(1, 2), r(3, 4)));
    // The continuation of the first child carries its onset's region.
    let cont = right.iter().find(|e| e.anchor() == r(0, 1)).unwrap();
    assert!(!cont.is_onset());
    assert_eq!(cont.part.begin, r(1, 4));
    assert_eq!(region(cont), (r(0, 1), r(1, 2)));
    assert_eq!(
        cont.occ,
        left.iter().find(|e| e.anchor() == r(0, 1)).unwrap().occ
    );
}

#[test]
fn identical_stack_twins_have_distinct_occurrences() {
    let twin = (*s(kw("bd"))).clone();
    let p = stack(vec![twin.clone(), twin], None);
    let ev = run(&p, cycle(0)).events;
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0].whole, ev[1].whole);
    assert_ne!(ev[0].occ, ev[1].occ);
    // Under chop, the twins' children stay distinct too.
    let c = chop(Rc::new(p), PParam::int(2), None);
    let ev = run(&c, cycle(0)).events;
    let keys: std::collections::BTreeSet<OccKey> = ev.iter().map(|e| e.occ.clone()).collect();
    assert_eq!(keys.len(), 4);
}

#[test]
fn occurrence_keys_are_equal_across_windows_and_anchored() {
    let p = chop(abc(), PParam::int(2), None);
    let full = run(&p, cycle(0)).events;
    for e in &full {
        assert_eq!(e.occ.anchor, e.whole.unwrap().begin);
        assert_eq!(e.occ.cycle, 0);
    }
    let later = run(&p, cycle(3)).events;
    assert!(later.iter().all(|e| e.occ.cycle == 3));
}
