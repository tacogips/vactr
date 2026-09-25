//! Randomness is a pure hash of (seed, node, cycle, anchor): reproducible
//! and independent of the query window.

use std::rc::Rc;

use super::*;
use crate::pattern::build::alt;
use crate::pattern::combinators::random::{choose, degrade_by, maybe, sometimes_by};
use crate::pattern::combinators::structure::euclid;
use crate::pattern::combinators::time::fast;
use crate::pattern::eval::{InputCells, QueryCtx};
use crate::pattern::pat::PParam;
use crate::pattern::query::query;

fn hats(n: i32) -> Rc<Pat> {
    Rc::new(fast(konst(kw("hh")), PParam::int(n), None))
}

fn run_seed(p: &Pat, sp: TimeSpan, seed: u64) -> Vec<(Ratio64, String)> {
    let mut vm = StubVm::new();
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(&mut vm, &cells, seed);
    names(&query(p, sp, &mut cx).events)
}

fn halves(p: &Pat) -> Vec<(Ratio64, String)> {
    let mut v = names(&run(p, span(r(0, 1), r(1, 2))).events);
    v.extend(names(&run(p, span(r(1, 2), r(1, 1))).events));
    v.sort();
    v
}

#[test]
fn euclid_3_8_is_reproducible_and_pure() {
    let e = euclid(
        konst(kw("bd")),
        PParam::int(3),
        PParam::int(8),
        PParam::int(0),
        None,
    );
    let a = names(&run(&e, cycle(0)).events);
    assert_eq!(a, names(&run(&e, cycle(0)).events));
    assert_eq!(a, run_seed(&e, cycle(0), 7));
    assert_eq!(
        a.iter().map(|x| x.0).collect::<Vec<_>>(),
        vec![r(0, 1), r(3, 8), r(3, 4)]
    );
    assert_eq!(a, halves(&e));
}

#[test]
fn maybe_is_reproducible_pure_and_window_independent() {
    let m = maybe(hats(16), None, None);
    let a = names(&run(&m, cycle(0)).events);
    assert_eq!(
        a,
        names(&run(&m, cycle(0)).events),
        "same span, same events"
    );
    assert_eq!(a, halves(&m), "window independent");
    assert!(
        !a.is_empty() && a.len() < 16,
        "about half kept: {}",
        a.len()
    );
    assert_ne!(a, run_seed(&m, cycle(0), 1234), "the seed matters");
    let all = maybe(hats(16), Some(PParam::Const(Value::Float64(1.0))), None);
    assert_eq!(run(&all, cycle(0)).events.len(), 16);
    let none = maybe(hats(16), Some(PParam::Const(Value::Float64(0.0))), None);
    assert!(run(&none, cycle(0)).events.is_empty());
}

#[test]
fn degrade_by_is_reproducible_and_pure() {
    let d = degrade_by(hats(16), PParam::Const(Value::Float64(0.5)), None);
    let a = names(&run(&d, cycle(3)).events);
    assert_eq!(a, names(&run(&d, cycle(3)).events));
    assert!(!a.is_empty() && a.len() < 16);
    let zero = degrade_by(hats(16), PParam::Const(Value::Float64(0.0)), None);
    assert_eq!(run(&zero, cycle(0)).events.len(), 16);
    let one = degrade_by(hats(16), PParam::int(1), None);
    assert!(run(&one, cycle(0)).events.is_empty());
    // Overlapping windows report the same survivors.
    let mut left = names(&run(&d, span(r(3, 1), r(15, 4))).events);
    let right = names(&run(&d, span(r(13, 4), r(4, 1))).events);
    left.extend(right);
    left.sort();
    left.dedup();
    assert_eq!(left, a);
}

#[test]
fn choose_picks_one_child_per_cycle() {
    let c = choose(
        vec![(*konst(kw("bd"))).clone(), (*konst(kw("sd"))).clone()],
        None,
    );
    let mut seen = std::collections::BTreeSet::new();
    for cyc in 0..32 {
        let a = names(&run(&c, cycle(cyc)).events);
        assert_eq!(a.len(), 1);
        assert_eq!(a, names(&run(&c, cycle(cyc)).events));
        seen.insert(a[0].1.clone());
    }
    assert_eq!(seen.len(), 2, "both children appear over 32 cycles");
    // Inside a step list the choice is per outer cycle.
    let steps = pat(list(vec![kw("hh"), Value::Pattern(Rc::new(c))]));
    let a = names(&run(&steps, cycle(5)).events);
    assert_eq!(a, halves_at(&steps, 5));
}

fn halves_at(p: &Pat, c: i64) -> Vec<(Ratio64, String)> {
    let base = Ratio64::from_int(c);
    let mid = base.checked_add(r(1, 2)).unwrap();
    let end = base.checked_add(Ratio64::ONE).unwrap();
    let mut v = names(&run(p, span(base, mid)).events);
    v.extend(names(&run(p, span(mid, end)).events));
    v.sort();
    v
}

#[test]
fn sometimes_by_bounds() {
    let mut vm = StubVm::new();
    let [fast2, _, _, _] = transforms(&mut vm);
    let base = pat(list(vec![kw("bd"), kw("sd")]));
    let never = sometimes_by(Rc::clone(&base), PParam::int(0), fast2.clone(), None);
    assert_eq!(run_with(&never, cycle(0), &mut vm).events.len(), 2);
    let always = sometimes_by(base, PParam::int(1), fast2, None);
    assert_eq!(run_with(&always, cycle(0), &mut vm).events.len(), 4);
    // alt stays deterministic under randomness elsewhere.
    let a = alt(&[kw("bd"), kw("sd")], None).unwrap();
    assert_eq!(names(&run(&a, cycle(1)).events)[0].1, "sd");
}
