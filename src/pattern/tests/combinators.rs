//! Combinator goldens from design-music section 3.

use std::rc::Rc;

use super::*;
use crate::pattern::build::{alt, signal};
use crate::pattern::combinators::music::{arp, chord, chord_pattern_of, range, scale, voicing};
use crate::pattern::combinators::structure::{
    cat, fastcat, grid, jux, off, ply, repeat, stack, superimpose,
};
use crate::pattern::combinators::time::{chunk, every, fast, iter, rev, segment, slow, whenmod};
use crate::pattern::pat::PParam;
use crate::pattern::signal::Sig;
use crate::value::intern::intern_kw;

fn abcd() -> Rc<Pat> {
    pat(list(vec![kw("a"), kw("b"), kw("c"), kw("d")]))
}

fn seq(events: &[Event]) -> Vec<String> {
    names(events).into_iter().map(|(_, n)| n).collect()
}

fn run_vm(p: &Pat, sp: TimeSpan, vm: &mut StubVm) -> Vec<Event> {
    let res = run_with(p, sp, vm);
    assert!(res.faults.is_empty(), "{:?}", res.faults);
    res.events
}

#[test]
fn fast_and_slow() {
    let bdsd = pat(list(vec![kw("bd"), kw("sd")]));
    let f = fast(Rc::clone(&bdsd), PParam::int(2), None);
    assert_eq!(
        onsets(&run(&f, cycle(0)).events),
        vec![r(0, 1), r(1, 4), r(1, 2), r(3, 4)]
    );
    let sl = slow(bdsd, PParam::int(2), None);
    assert_eq!(
        names(&run(&sl, cycle(0)).events),
        vec![(r(0, 1), "bd".into())]
    );
    assert_eq!(
        names(&run(&sl, cycle(1)).events),
        vec![(r(1, 1), "sd".into())]
    );
    assert_eq!(timing(&run(&sl, cycle(1)).events), vec![(r(1, 1), r(1, 1))]);
    // fast 0 is silence.
    let z = fast(abcd(), PParam::int(0), None);
    assert!(run(&z, cycle(0)).events.is_empty());
}

#[test]
fn rev_reverses_each_cycle() {
    let p = rev(pat(list(vec![kw("a"), kw("b"), kw("c")])), None);
    assert_eq!(
        names(&run(&p, cycle(3)).events),
        vec![
            (r(3, 1), "c".into()),
            (r(10, 3), "b".into()),
            (r(11, 3), "a".into())
        ]
    );
    // Sampling through rev at a point.
    let pt = run(&p, TimeSpan::point(r(0, 1)));
    assert_eq!(name(&pt.events[0].value), "c");
}

#[test]
fn every_and_whenmod() {
    let mut vm = StubVm::new();
    let [fast2, rev_f, _, _] = transforms(&mut vm);
    // s [:bd-haus :sn-dub] > every 4 {p -> fast p 2} > whenmod 8 6 rev
    let base = pat(list(vec![kw("bd"), kw("sd")]));
    let ev = Rc::new(every(base, PParam::int(4), fast2, None));
    for (c, n) in [(0, 4), (1, 2), (3, 2), (4, 4), (8, 4)] {
        assert_eq!(run_vm(&ev, cycle(c), &mut vm).len(), n, "cycle {c}");
    }
    let wm = whenmod(ev, PParam::int(8), PParam::int(6), rev_f, None);
    assert_eq!(seq(&run_vm(&wm, cycle(5), &mut vm)), vec!["bd", "sd"]);
    assert_eq!(seq(&run_vm(&wm, cycle(6), &mut vm)), vec!["sd", "bd"]);
    assert_eq!(seq(&run_vm(&wm, cycle(7), &mut vm)), vec!["sd", "bd"]);
    // Cycle 8 is both an every-4 cycle (fast) and outside 6..7 (not reversed).
    assert_eq!(
        seq(&run_vm(&wm, cycle(8), &mut vm)),
        vec!["bd", "sd", "bd", "sd"]
    );
}

#[test]
fn stack_cat_fastcat() {
    let hh4 = repeat(konst(kw("hh")), PParam::int(4), None);
    let bdsd = (*pat(list(vec![kw("bd"), kw("sd")]))).clone();
    let c = cat(vec![bdsd.clone(), hh4.clone()], None);
    assert_eq!(seq(&run(&c, cycle(0)).events), vec!["bd", "sd"]);
    assert_eq!(seq(&run(&c, cycle(1)).events), vec!["hh"; 4]);
    assert_eq!(seq(&run(&c, cycle(2)).events), vec!["bd", "sd"]);
    let fc = fastcat(vec![bdsd.clone(), hh4.clone()], None);
    assert_eq!(
        onsets(&run(&fc, cycle(0)).events),
        vec![r(0, 1), r(1, 4), r(1, 2), r(5, 8), r(3, 4), r(7, 8)]
    );
    let st = stack(vec![bdsd, hh4], None);
    assert_eq!(run(&st, cycle(0)).events.len(), 6);
}

#[test]
fn superimpose_off_jux() {
    let mut vm = StubVm::new();
    let [fast2, rev_f, ident, _] = transforms(&mut vm);
    let bdsd = pat(list(vec![kw("bd"), kw("sd")]));
    let si = superimpose(Rc::clone(&bdsd), fast2, None);
    assert_eq!(run_vm(&si, cycle(0), &mut vm).len(), 6);
    // off 0.25 on note [:c :e :g] with an identity transform.
    let ceg = pat(list(vec![kw("c"), kw("e"), kw("g")]));
    let o = off(ceg, PParam::Const(Value::Float(0.25)), ident, None);
    let ev = run_vm(&o, cycle(0), &mut vm);
    assert_eq!(
        onsets(&ev),
        vec![r(0, 1), r(1, 4), r(1, 3), r(7, 12), r(2, 3), r(11, 12)]
    );
    // The shifted g of cycle -1 continues into cycle 0 without an onset.
    assert!(ev
        .iter()
        .any(|e| !e.is_onset() && e.whole == Some(span(r(-1, 12), r(1, 4)))));
    let j = jux(bdsd, rev_f, None);
    let ev = run_vm(&j, cycle(0), &mut vm);
    let pan = intern_kw("pan");
    let left: Vec<String> = ev
        .iter()
        .filter(|e| matches!(e.controls.get(&pan), Some(Value::Float64(x)) if *x == 0.0))
        .map(|e| name(&e.value))
        .collect();
    let right: Vec<(Ratio64, String)> = names(
        &ev.iter()
            .filter(|e| matches!(e.controls.get(&pan), Some(Value::Float64(x)) if *x == 1.0))
            .cloned()
            .collect::<Vec<_>>(),
    );
    assert_eq!(left, vec!["bd", "sd"]);
    assert_eq!(right, vec![(r(0, 1), "sd".into()), (r(1, 2), "bd".into())]);
}

#[test]
fn iter_ply_chunk() {
    let it = iter(abcd(), PParam::int(4), None);
    assert_eq!(seq(&run(&it, cycle(0)).events), vec!["a", "b", "c", "d"]);
    assert_eq!(seq(&run(&it, cycle(1)).events), vec!["b", "c", "d", "a"]);
    assert_eq!(seq(&run(&it, cycle(2)).events), vec!["c", "d", "a", "b"]);
    let pl = ply(pat(list(vec![kw("a"), kw("b")])), PParam::int(2), None);
    assert_eq!(
        names(&run(&pl, cycle(0)).events),
        vec![
            (r(0, 1), "a".into()),
            (r(1, 4), "a".into()),
            (r(1, 2), "b".into()),
            (r(3, 4), "b".into())
        ]
    );
    let mut vm = StubVm::new();
    let [fast2, _, _, _] = transforms(&mut vm);
    let ch = chunk(abcd(), PParam::int(4), fast2, None);
    // Cycle 0: the first quarter is fast 2 (a at 0, b at 1/8); the rest plain.
    assert_eq!(
        names(&run_vm(&ch, cycle(0), &mut vm)),
        vec![
            (r(0, 1), "a".into()),
            (r(1, 8), "b".into()),
            (r(1, 4), "b".into()),
            (r(1, 2), "c".into()),
            (r(3, 4), "d".into())
        ]
    );
    // Cycle 1: the second quarter is transformed (c at 1+1/4, d at 1+3/8).
    assert_eq!(
        names(&run_vm(&ch, cycle(1), &mut vm)),
        vec![
            (r(1, 1), "a".into()),
            (r(5, 4), "c".into()),
            (r(11, 8), "d".into()),
            (r(3, 2), "c".into()),
            (r(7, 4), "d".into())
        ]
    );
}

#[test]
fn segment_and_range() {
    let sg = segment(Rc::new(signal(Sig::Sine, None)), PParam::int(8), None);
    let ev = sorted(run(&sg, cycle(0)).events);
    assert_eq!(ev.len(), 8);
    assert_eq!(timing(&ev)[1], (r(1, 8), r(1, 8)));
    assert_eq!(show(&ev[0].value), show(&Value::Float64(0.5)));
    assert_eq!(show(&ev[2].value), show(&Value::Float64(1.0)));
    assert_eq!(show(&ev[6].value), show(&Value::Float64(0.0)));
    // range sine 1 5 (subject first, M2).
    let rg = range(
        Rc::new(signal(Sig::Sine, None)),
        PParam::int(1),
        PParam::int(5),
        None,
    );
    let at = |t: Ratio64| run(&rg, TimeSpan::point(t)).events[0].value.clone();
    assert_eq!(show(&at(r(0, 1))), show(&Value::Float64(3.0)));
    assert_eq!(show(&at(r(1, 4))), show(&Value::Float64(5.0)));
    // Exact inputs stay exact.
    let ex = range(
        pat(list(vec![int(0), Value::Ratio(r(1, 2)), int(1)])),
        PParam::int(10),
        PParam::int(20),
        None,
    );
    let v: Vec<Value> = sorted(run(&ex, cycle(0)).events)
        .into_iter()
        .map(|e| e.value)
        .collect();
    assert_eq!(shows(&v), shows(&[int(10), int(15), int(20)]));
}

#[test]
fn scale_chord_voicing_arp() {
    // s :pluck > n [0 2 4] > scale :c :minor
    let p = scale(
        ctl(s(kw("pluck")), "n", list(vec![int(0), int(2), int(4)])),
        intern_kw("c"),
        intern_kw("minor"),
        None,
    )
    .unwrap();
    let ev = sorted(run(&p, cycle(0)).events);
    let notes: Vec<Value> = ev
        .iter()
        .map(|e| ctl_of(e, "note").unwrap().clone())
        .collect();
    assert_eq!(shows(&notes), shows(&[int(60), int(63), int(67)]));
    assert!(scale(s(kw("x")), intern_kw("c"), intern_kw("nope"), None).is_err());
    // Negative and wrapping degrees.
    let q = scale(
        pat(list(vec![int(-1), int(7)])),
        intern_kw("c"),
        intern_kw("major"),
        None,
    )
    .unwrap();
    let v: Vec<Value> = sorted(run(&q, cycle(0)).events)
        .into_iter()
        .map(|e| e.value)
        .collect();
    assert_eq!(shows(&v), shows(&[int(59), int(72)]));

    // s :piano > chord {alt [:c :maj7] [:d :m7] [:g :dom7]} > voicing
    let chords = alt(
        &[
            list(vec![kw("c"), kw("maj7")]),
            list(vec![kw("d"), kw("m7")]),
            list(vec![kw("g"), kw("dom7")]),
        ],
        None,
    )
    .unwrap();
    let chords = chord_pattern_of(&Value::Pattern(Rc::new(chords)), None).unwrap();
    let ch = Rc::new(chord(chords, s(kw("piano")), None));
    let v = Rc::new(voicing(ch, None));
    let notes_at = |c: i64| {
        let res = run(&v, cycle(c));
        assert!(res.faults.is_empty(), "{:?}", res.faults);
        let ev = res.events;
        assert_eq!(ev.len(), 1, "one chord event per cycle");
        ctl_of(&ev[0], "note").unwrap().clone()
    };
    let ints = |xs: &[i32]| list(xs.iter().map(|x| int(*x)).collect());
    assert_eq!(
        format!("{:?}", notes_at(0)),
        format!("{:?}", ints(&[60, 64, 67, 71]))
    );
    assert_eq!(
        format!("{:?}", notes_at(1)),
        format!("{:?}", ints(&[62, 65, 69, 72]))
    );
    assert_eq!(
        format!("{:?}", notes_at(2)),
        format!("{:?}", ints(&[67, 71, 74, 77]))
    );

    // s :piano > chord [:c2 :m7] > voicing: moved to octave 5.
    let low = chord_pattern_of(&list(vec![kw("c2"), kw("m7")]), None).unwrap();
    let lv = voicing(Rc::new(chord(low, s(kw("piano")), None)), None);
    let ev = run(&lv, cycle(0)).events;
    assert_eq!(
        format!("{:?}", ctl_of(&ev[0], "note").unwrap()),
        format!("{:?}", ints(&[60, 63, 67, 70]))
    );

    // s :piano > chord [:c :m7] > arp :up / :down
    let cm7 = chord_pattern_of(&list(vec![kw("c"), kw("m7")]), None).unwrap();
    let base = Rc::new(chord(cm7, s(kw("piano")), None));
    for (mode, want) in [("up", [60, 63, 67, 70]), ("down", [70, 67, 63, 60])] {
        let a = arp(Rc::clone(&base), PParam::Const(kw(mode)), None);
        let ev = sorted(run(&a, cycle(0)).events);
        assert_eq!(
            timing(&ev).iter().map(|t| t.0).collect::<Vec<_>>(),
            vec![r(0, 1), r(1, 4), r(1, 2), r(3, 4)]
        );
        let got: Vec<Value> = ev
            .iter()
            .map(|e| ctl_of(e, "note").unwrap().clone())
            .collect();
        assert_eq!(
            shows(&got),
            shows(&want.iter().map(|x| int(*x)).collect::<Vec<_>>())
        );
    }
    // A dynamic unknown quality is an event-local fault.
    let bad = chord_pattern_of(&list(vec![kw("c"), kw("nine")]), None).unwrap();
    let res = run(&chord(bad, s(kw("piano")), None), cycle(0));
    assert!(res.events.is_empty());
    assert_eq!(res.faults.len(), 1);
}

#[test]
fn grid_takes_structure_from_booleans() {
    // grid {s :bd} [true false true true]
    let g = grid(
        s(kw("bd")),
        pat(list(vec![
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(true),
            Value::Bool(true),
        ])),
        None,
    );
    let ev = run(&g, cycle(0)).events;
    assert_eq!(onsets(&ev), vec![r(0, 1), r(1, 2), r(3, 4)]);
    assert!(ev.iter().all(|e| name(&e.value) == "bd"));
    assert_eq!(timing(&ev)[0].1, r(1, 4));
}
