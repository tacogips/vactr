//! `midi-notes` after `s` and the bind-time input-lane walk (design 11.7).

use std::rc::Rc;

use super::*;
use crate::pattern::combinators::input::{
    input_lane_walk, midi_notes, realize_note, LaneOp, LiveNote,
};
use crate::pattern::combinators::music::scale;
use crate::pattern::combinators::random::{degrade_by, maybe, sometimes_by};
use crate::pattern::combinators::region::chop;
use crate::pattern::combinators::structure::stack;
use crate::pattern::combinators::time::{every, fast, rev};
use crate::pattern::eval::{InputCells, QueryCtx};
use crate::pattern::pat::PParam;
use crate::types::diag::DiagCode;
use crate::value::intern::intern_kw;

fn lane(channel: Option<u8>) -> Rc<Pat> {
    Rc::new(midi_notes(s(kw("pluck")), channel, None))
}

fn note(seq: u64, pitch: u8) -> LiveNote {
    LiveNote {
        channel: 1,
        note: pitch,
        velocity: 100,
        seq,
        at: r(seq as i64, 4),
    }
}

fn realize(p: &Pat, n: &LiveNote, vm: &mut StubVm) -> Option<Event> {
    let plan = input_lane_walk(p).expect("supported lane");
    assert_eq!(plan.lanes.len(), 1);
    let cells = InputCells::new();
    let mut cx = QueryCtx::new(vm, &cells, 42);
    realize_note(&plan.lanes[0], n, &mut cx).expect("no fault")
}

#[test]
fn midi_notes_yields_no_events_under_query() {
    let p = lane(Some(1));
    let res = run(&p, span(r(0, 1), r(4, 1)));
    assert!(res.events.is_empty());
    assert!(res.faults.is_empty());
}

#[test]
fn degrade_by_1_drops_every_realized_note() {
    // s :pluck > midi-notes > degrade-by 1
    let p = degrade_by(lane(None), PParam::int(1), None);
    let mut vm = StubVm::new();
    for seq in 0..32 {
        assert!(realize(&p, &note(seq, 60), &mut vm).is_none());
    }
    // degrade-by 0 keeps them all; maybe 1 keeps, maybe 0 drops.
    let keep = degrade_by(lane(None), PParam::int(0), None);
    assert!(realize(&keep, &note(3, 60), &mut vm).is_some());
    let m1 = maybe(lane(None), Some(PParam::int(1)), None);
    assert!(realize(&m1, &note(3, 60), &mut vm).is_some());
    let m0 = maybe(lane(None), Some(PParam::int(0)), None);
    assert!(realize(&m0, &note(3, 60), &mut vm).is_none());
}

#[test]
fn controls_and_scales_decorate_per_note_in_tree_order() {
    // s :pluck > gain 0.8 > midi-notes channel: 1 > n 2 > scale :c :major > lpf 800
    let subject = ctl(s(kw("pluck")), "gain", Value::Float(0.8));
    let ln = Rc::new(midi_notes(subject, Some(1), None));
    let with_n = ctl(ln, "n", int(2));
    let scaled = Rc::new(scale(with_n, intern_kw("c"), intern_kw("major"), None).unwrap());
    let p = ctl(scaled, "lpf", int(800));
    let plan = input_lane_walk(&p).unwrap();
    let ops = &plan.lanes[0].ops;
    assert!(matches!(ops[0], LaneOp::Control(k, _) if k == intern_kw("n")));
    assert!(matches!(ops[1], LaneOp::Scale(_, _)));
    assert!(matches!(ops[2], LaneOp::Control(k, _) if k == intern_kw("lpf")));
    let mut vm = StubVm::new();
    let e = realize(&p, &note(1, 61), &mut vm).unwrap();
    assert_eq!(name(&e.value), "pluck");
    assert_eq!(show(ctl_of(&e, "gain").unwrap()), show(&Value::Float(0.8)));
    assert_eq!(
        show(ctl_of(&e, "note").unwrap()),
        show(&int(64)),
        "scale after n"
    );
    assert_eq!(show(ctl_of(&e, "lpf").unwrap()), show(&int(800)));
    assert_eq!(e.part, TimeSpan::point(r(1, 4)));
    assert!(e.whole.is_none(), "an open-duration live note");
    // Reversed order: the scale sees no n yet, so the live pitch stays.
    let rev_order = ctl(
        Rc::new(scale(lane(Some(1)), intern_kw("c"), intern_kw("major"), None).unwrap()),
        "n",
        int(2),
    );
    let e = realize(&rev_order, &note(1, 61), &mut vm).unwrap();
    assert_eq!(show(ctl_of(&e, "note").unwrap()), show(&int(61)));
    // A note on another channel is not this lane's.
    let mut other = note(2, 60);
    other.channel = 2;
    assert!(realize(&p, &other, &mut vm).is_none());
}

#[test]
fn sometimes_by_passes_kept_notes_through_the_transform() {
    let mut vm = StubVm::new();
    let [_, _, _, hurry2] = transforms(&mut vm);
    let p = sometimes_by(lane(None), PParam::int(1), hurry2, None);
    let e = realize(&p, &note(0, 60), &mut vm).unwrap();
    assert_eq!(show(ctl_of(&e, "speed").unwrap()), show(&int(2)));
    assert_eq!(show(ctl_of(&e, "note").unwrap()), show(&int(60)));
}

#[test]
fn a_stack_of_a_lane_and_a_queried_branch_realize_independently() {
    let queried = (*s(list(vec![kw("bd"), kw("sd")]))).clone();
    let p = stack(vec![(*lane(Some(1))).clone(), queried], None);
    let res = run(&p, cycle(0));
    assert_eq!(names(&res.events).len(), 2, "the queried branch plays");
    let plan = input_lane_walk(&p).unwrap();
    assert_eq!(plan.lanes.len(), 1, "the lane is realized live");
    let mut vm = StubVm::new();
    assert!(realize(&p, &note(0, 60), &mut vm).is_some());
}

#[test]
fn retiming_operators_over_the_lane_are_bind_time_diagnostics() {
    let mut vm = StubVm::new();
    let [fast2, _, _, _] = transforms(&mut vm);
    let bad: Vec<Pat> = vec![
        fast(lane(None), PParam::int(2), None),
        rev(lane(None), None),
        every(lane(None), PParam::int(4), fast2, None),
        chop(lane(None), PParam::int(4), None),
        // A structured subject would re-time live input.
        midi_notes(s(list(vec![kw("a"), kw("b")])), None, None),
    ];
    for p in &bad {
        let err = input_lane_walk(p).unwrap_err();
        assert_eq!(err.code, DiagCode::InputLaneOperator);
    }
    // A pattern without lanes has an empty plan.
    assert!(input_lane_walk(&s(kw("bd"))).unwrap().lanes.is_empty());
}
