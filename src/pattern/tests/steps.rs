//! Step lists and the design-music section 3 mini-notation table.

use std::rc::Rc;

use super::*;
use crate::ns::namespace::FormGen;
use crate::pattern::build::alt;
use crate::pattern::combinators::pattern_value;
use crate::pattern::combinators::random::maybe;
use crate::pattern::combinators::structure::{euclid, hold, repeat, stack};
use crate::pattern::combinators::time::fast;
use crate::pattern::pat::PParam;
use crate::reader::span::{FileId, Span};
use crate::value::value::{ListProv, ListVal};

fn values(events: &[Event]) -> Vec<(Ratio64, String)> {
    let mut v: Vec<(Ratio64, String)> = events
        .iter()
        .map(|e| (e.anchor(), name(&e.value)))
        .collect();
    v.sort();
    v
}

#[test]
fn nested_subdivision_lands_on_exact_ratios() {
    let p = pat(list(vec![
        kw("bd"),
        kw("sd"),
        list(vec![kw("hh"), kw("hh")]),
    ]));
    let res = run(&p, cycle(0));
    assert!(res.faults.is_empty());
    assert_eq!(
        timing(&res.events),
        vec![
            (r(0, 1), r(1, 3)),
            (r(1, 3), r(1, 3)),
            (r(2, 3), r(1, 6)),
            (r(5, 6), r(1, 6))
        ]
    );
    assert_eq!(
        values(&res.events),
        vec![
            (r(0, 1), "bd".into()),
            (r(1, 3), "sd".into()),
            (r(2, 3), "hh".into()),
            (r(5, 6), "hh".into())
        ]
    );
    // Every event is an onset over a full-cycle query.
    assert!(res.events.iter().all(Event::is_onset));
    // The next cycle repeats, shifted by one.
    let next = run(&p, cycle(1));
    assert_eq!(next.events.len(), 4);
    assert_eq!(timing(&next.events)[3].0, r(11, 6));
}

#[test]
fn nil_is_a_rest() {
    let with_rests = pat(list(vec![kw("bd"), Value::Nil, kw("sd"), Value::Nil]));
    let res = run(&with_rests, cycle(0));
    assert_eq!(
        timing(&res.events),
        vec![(r(0, 1), r(1, 4)), (r(1, 2), r(1, 4))]
    );
    let plain = pat(list(vec![kw("bd"), kw("sd")]));
    let res2 = run(&plain, cycle(0));
    let onsets = |ev: &[Event]| ev.iter().map(Event::anchor).collect::<Vec<_>>();
    assert_eq!(onsets(&sorted(res.events)), onsets(&sorted(res2.events)));
}

#[test]
fn integer_repeat_is_a_nested_list_and_fast_is_the_same() {
    // hh*2: [:hh :hh] == {fast :hh 2}
    let nested = pat(list(vec![kw("bd"), list(vec![kw("hh"), kw("hh")])]));
    let fast_hh = fast(konst(kw("hh")), PParam::int(2), None);
    let with_fast = pat(list(vec![kw("bd"), pattern_value(fast_hh)]));
    let a = run(&nested, cycle(0));
    let b = run(&with_fast, cycle(0));
    assert_eq!(timing(&a.events), timing(&b.events));
    assert_eq!(values(&a.events), values(&b.events));
}

#[test]
fn non_integer_repeat_uses_fast() {
    // {fast :hh 1.5}: onsets 0 and 2/3; the second event is clipped at 1.
    let p = fast(konst(kw("hh")), PParam::Const(Value::Float(1.5)), None);
    let res = run(&p, cycle(0));
    let ev = sorted(res.events);
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0].whole, Some(span(r(0, 1), r(2, 3))));
    assert_eq!(ev[1].whole, Some(span(r(2, 3), r(4, 3))));
    assert_eq!(ev[1].part, span(r(2, 3), r(1, 1)));
    assert!(ev[1].is_onset());
}

#[test]
fn alt_alternates_per_cycle() {
    let a = alt(&[kw("bd"), kw("sd")], None).unwrap();
    let p = pat(list(vec![kw("hh"), pattern_value(a)]));
    let at = |c: i64| {
        let res = run(&p, cycle(c));
        values(&res.events)
    };
    assert_eq!(at(0), vec![(r(0, 1), "hh".into()), (r(1, 2), "bd".into())]);
    assert_eq!(at(1), vec![(r(1, 1), "hh".into()), (r(3, 2), "sd".into())]);
    assert_eq!(at(2)[1].1, "bd");
}

#[test]
fn euclid_inside_a_step_list() {
    let e = euclid(
        konst(kw("bd")),
        PParam::int(3),
        PParam::int(8),
        PParam::int(0),
        None,
    );
    let res = run(&e, cycle(0));
    assert_eq!(
        timing(&res.events),
        vec![(r(0, 1), r(1, 8)), (r(3, 8), r(1, 8)), (r(3, 4), r(1, 8))]
    );
    // rotation: 2 rotates left by two steps: x..x..x. -> .x..x.x.
    let rot = euclid(
        konst(kw("bd")),
        PParam::int(3),
        PParam::int(8),
        PParam::int(2),
        None,
    );
    let res = run(&rot, cycle(0));
    let onsets: Vec<Ratio64> = timing(&res.events).into_iter().map(|t| t.0).collect();
    assert_eq!(onsets, vec![r(1, 8), r(4, 8), r(6, 8)]);
}

#[test]
fn hold_weights_a_step() {
    let h = hold(konst(kw("bd")), PParam::int(3), None);
    let p = pat(list(vec![pattern_value(h), kw("sd")]));
    let res = run(&p, cycle(0));
    assert_eq!(
        timing(&res.events),
        vec![(r(0, 1), r(3, 4)), (r(3, 4), r(1, 4))]
    );
}

#[test]
fn repeat_replicates_a_step() {
    let rp = repeat(konst(kw("bd")), PParam::int(2), None);
    let a = pat(list(vec![pattern_value(rp), kw("sd")]));
    let b = pat(list(vec![kw("bd"), kw("bd"), kw("sd")]));
    let ra = run(&a, cycle(0));
    let rb = run(&b, cycle(0));
    assert_eq!(timing(&ra.events), timing(&rb.events));
    assert_eq!(values(&ra.events), values(&rb.events));
    // On its own, repeat is n steps of the child.
    let alone = repeat(konst(kw("hh")), PParam::int(4), None);
    assert_eq!(run(&alone, cycle(0)).events.len(), 4);
}

#[test]
fn sample_index_is_a_control() {
    // bd:3 == s :bd > n 3
    let p = ctl(s(kw("bd")), "n", int(3));
    let res = run(&p, cycle(0));
    assert_eq!(res.events.len(), 1);
    assert_eq!(ctl_of(&res.events[0], "n").map(show), Some(show(&int(3))));
}

#[test]
fn stack_is_polyphony() {
    // a, b == stack [[..] [..]]
    let p = stack(
        vec![
            (*pat(list(vec![kw("bd"), kw("sd")]))).clone(),
            (*pat(list(vec![kw("hh"), kw("hh"), kw("hh")]))).clone(),
        ],
        None,
    );
    let res = run(&p, cycle(0));
    assert_eq!(res.events.len(), 5);
    let onsets: Vec<Ratio64> = timing(&res.events).into_iter().map(|t| t.0).collect();
    assert_eq!(onsets, vec![r(0, 1), r(0, 1), r(1, 3), r(1, 2), r(2, 3)]);
}

#[test]
fn booleans_are_steps() {
    let p = pat(list(vec![
        Value::Bool(true),
        Value::Bool(false),
        Value::Bool(true),
        Value::Bool(true),
    ]));
    let res = run(&p, cycle(0));
    assert_eq!(res.events.len(), 4);
    assert!(matches!(sorted(res.events)[1].value, Value::Bool(false)));
}

#[test]
fn the_full_mini_notation_line() {
    // s [:bd :sd [:hh :hh] [:cp :cp] {alt :bd :sd} {maybe :bd} {euclid :bd 3 8} nil]
    let a = alt(&[kw("bd"), kw("sd")], None).unwrap();
    let m = maybe(
        konst(kw("bd")),
        Some(PParam::Const(Value::Float64(1.0))),
        None,
    );
    let e = euclid(
        konst(kw("bd")),
        PParam::int(3),
        PParam::int(8),
        PParam::int(0),
        None,
    );
    let p = s(list(vec![
        kw("bd"),
        kw("sd"),
        list(vec![kw("hh"), kw("hh")]),
        list(vec![kw("cp"), kw("cp")]),
        pattern_value(a),
        pattern_value(m),
        pattern_value(e),
        Value::Nil,
    ]));
    let res = run(&p, cycle(0));
    assert!(res.faults.is_empty(), "{:?}", res.faults);
    let got = values(&res.events);
    let want: Vec<(Ratio64, String)> = vec![
        (r(0, 1), "bd"),
        (r(1, 8), "sd"),
        (r(2, 8), "hh"),
        (r(5, 16), "hh"),
        (r(3, 8), "cp"),
        (r(7, 16), "cp"),
        (r(4, 8), "bd"),
        (r(5, 8), "bd"),
        (r(6, 8), "bd"),
        (r(51, 64), "bd"),
        (r(54, 64), "bd"),
    ]
    .into_iter()
    .map(|(t, n)| (t, n.to_string()))
    .collect();
    assert_eq!(got, want);
}

#[test]
fn step_events_carry_list_provenance() {
    let file = FileId::new(3);
    let spans = [Span::new(file, 1, 4), Span::new(file, 5, 8)];
    let inner_spans = [Span::new(file, 10, 13), Span::new(file, 14, 17)];
    let prov = |elems: &[Span]| {
        Some(Rc::new(ListProv {
            form_gen: FormGen::new(7),
            doc_revision: 2,
            elems: elems.to_vec().into_boxed_slice(),
        }))
    };
    let inner = Value::List(Rc::new(ListVal {
        items: vec![kw("hh"), kw("hh")].into_boxed_slice(),
        prov: prov(&inner_spans),
    }));
    let outer = Value::List(Rc::new(ListVal {
        items: vec![kw("bd"), inner].into_boxed_slice(),
        prov: prov(&spans),
    }));
    let res = run(&pat(outer), cycle(0));
    let ev = sorted(res.events);
    let srcs: Vec<Span> = ev.iter().map(|e| e.src.unwrap().span).collect();
    assert_eq!(srcs, vec![spans[0], inner_spans[0], inner_spans[1]]);
    assert_eq!(ev[0].src.unwrap().doc_revision, 2);
    assert_eq!(ev[0].src.unwrap().form_gen, FormGen::new(7));
}

#[test]
fn partial_windows_clip_parts_and_keep_wholes() {
    let p = pat(list(vec![kw("bd"), kw("sd")]));
    let res = run(&p, span(r(1, 4), r(3, 4)));
    let ev = sorted(res.events);
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0].whole, Some(span(r(0, 1), r(1, 2))));
    assert_eq!(ev[0].part, span(r(1, 4), r(1, 2)));
    assert!(!ev[0].is_onset());
    assert!(ev[1].is_onset());
    // A point query samples the step that contains it.
    let pt = run(&p, TimeSpan::point(r(1, 2)));
    assert_eq!(pt.events.len(), 1);
    assert_eq!(name(&pt.events[0].value), "sd");
}
