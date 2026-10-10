use std::rc::Rc;

use crate::pattern::combinators::control::control;
use crate::pattern::combinators::strum::{harp, inversion, strum};
use crate::pattern::pat::{PParam, Pat};
use crate::pattern::tests::{ctl_of, cycle, int, kw, list, run, s, show};
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

fn chord(notes: Vec<Value>) -> Rc<Pat> {
    Rc::new(control(
        intern_kw("note"),
        Rc::new(crate::pattern::build::pure(Value::list(notes), None)),
        s(kw("bd")),
        None,
    ))
}
fn notes(events: &[crate::pattern::query::Event]) -> Vec<String> {
    let value = ctl_of(&events[0], "note").unwrap();
    match value {
        Value::List(list) => list.items.iter().map(show).collect(),
        other => vec![show(other)],
    }
}
fn tone(events: &[crate::pattern::query::Event], i: usize) -> String {
    show(ctl_of(&events[i], "note").unwrap())
}
fn base_chord() -> Rc<Pat> {
    chord(vec![int(60), int(64), int(67)])
}
fn ratio(n: i64, d: i64) -> PParam {
    PParam::Const(Value::Ratio(Ratio64::new(n, d).unwrap()))
}
fn keyword(s: &str) -> PParam {
    PParam::Const(kw(s))
}

#[test]
fn strum_up_and_down_spread_chord_tones() {
    let p = strum(
        base_chord(),
        ratio(1, 32),
        keyword("up"),
        keyword("flat"),
        None,
    );
    let events = run(&p, cycle(0)).events;
    assert_eq!(
        events.iter().map(|e| e.anchor()).collect::<Vec<_>>(),
        vec![
            Ratio64::ZERO,
            Ratio64::new(1, 32).unwrap(),
            Ratio64::new(1, 16).unwrap()
        ]
    );
    assert_eq!(
        (tone(&events, 0), tone(&events, 1), tone(&events, 2)),
        ("Int(60)".into(), "Int(64)".into(), "Int(67)".into())
    );
    assert!(events
        .iter()
        .all(|e| e.whole.unwrap().end == Ratio64::from_int(1)));
    let p = strum(
        base_chord(),
        ratio(1, 32),
        keyword("down"),
        keyword("flat"),
        None,
    );
    let events = run(&p, cycle(0)).events;
    assert_eq!(
        (tone(&events, 0), tone(&events, 1), tone(&events, 2)),
        ("Int(67)".into(), "Int(64)".into(), "Int(60)".into())
    );
}

#[test]
fn alternate_direction_uses_cycle_parity() {
    let p = strum(
        base_chord(),
        ratio(1, 32),
        keyword("alternate"),
        keyword("flat"),
        None,
    );
    for cycle_no in 0..4 {
        let events = run(&p, cycle(cycle_no)).events;
        assert_eq!(
            tone(&events, 0),
            if cycle_no % 2 == 0 {
                "Int(60)"
            } else {
                "Int(67)"
            }
        );
    }

    let quarter_events = Rc::new(crate::pattern::combinators::time::fast(
        base_chord(),
        PParam::int(4),
        None,
    ));
    let p = strum(
        quarter_events,
        ratio(1, 32),
        keyword("alternate"),
        keyword("flat"),
        None,
    );
    let events = run(&p, cycle(0)).events;
    let first_onsets = [
        Ratio64::ZERO,
        Ratio64::new(1, 4).unwrap(),
        Ratio64::new(1, 2).unwrap(),
        Ratio64::new(3, 4).unwrap(),
    ];
    let first_tones = first_onsets
        .iter()
        .map(|onset| {
            let event = events
                .iter()
                .find(|event| event.anchor() == *onset)
                .expect("each quarter event has a first strum tone");
            tone(std::slice::from_ref(event), 0)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        first_tones,
        vec!["Int(60)", "Int(67)", "Int(60)", "Int(67)"]
    );
}

#[test]
fn random_direction_is_deterministic_permutation() {
    let p = strum(
        base_chord(),
        ratio(1, 32),
        keyword("random"),
        keyword("flat"),
        None,
    );
    let first = run(&p, cycle(0)).events;
    let second = run(&p, cycle(0)).events;
    assert_eq!(
        first
            .iter()
            .map(|e| tone(std::slice::from_ref(e), 0))
            .collect::<Vec<_>>(),
        second
            .iter()
            .map(|e| tone(std::slice::from_ref(e), 0))
            .collect::<Vec<_>>()
    );
    let mut got = first
        .iter()
        .map(|e| tone(std::slice::from_ref(e), 0))
        .collect::<Vec<_>>();
    got.sort();
    assert_eq!(got, vec!["Int(60)", "Int(64)", "Int(67)"]);
}

#[test]
fn strum_clamps_time_to_event_duration() {
    let quarter_notes = Rc::new(crate::pattern::combinators::time::fast(
        base_chord(),
        PParam::int(4),
        None,
    ));
    let p = strum(
        quarter_notes,
        ratio(1, 2),
        keyword("up"),
        keyword("flat"),
        None,
    );
    let events = run(&p, cycle(0)).events;
    assert!(events
        .iter()
        .any(|e| e.anchor() == Ratio64::new(1, 12).unwrap()));
}

#[test]
fn strum_curve_updates_gain_or_velocity() {
    let p = strum(
        base_chord(),
        ratio(1, 32),
        keyword("up"),
        keyword("fade"),
        None,
    );
    let events = run(&p, cycle(0)).events;
    assert_eq!(show(ctl_of(&events[0], "gain").unwrap()), "Float64(1.0)");
    assert_eq!(show(ctl_of(&events[1], "gain").unwrap()), "Float64(0.75)");
    assert_eq!(show(ctl_of(&events[2], "gain").unwrap()), "Float64(0.5)");
    let velocity = Rc::new(control(
        intern_kw("velocity"),
        Rc::new(crate::pattern::build::pure(Value::Float64(0.8), None)),
        base_chord(),
        None,
    ));
    let p = strum(velocity, ratio(1, 32), keyword("up"), keyword("fade"), None);
    let events = run(&p, cycle(0)).events;
    assert_eq!(
        show(ctl_of(&events[1], "velocity").unwrap()),
        "Float64(0.6000000000000001)"
    );
}

#[test]
fn float_time_faults_and_scalar_notes_pass_through() {
    let p = strum(
        base_chord(),
        PParam::Const(Value::Float64(0.1)),
        keyword("up"),
        keyword("flat"),
        None,
    );
    let result = run(&p, cycle(0));
    assert!(result.events.is_empty());
    assert_eq!(result.faults.len(), 1);
    assert!(result.faults[0].message.contains("ratio"));
    let scalar = strum(
        s(kw("bd")),
        ratio(1, 32),
        keyword("up"),
        keyword("flat"),
        None,
    );
    assert_eq!(run(&scalar, cycle(0)).events.len(), 1);
}

#[test]
fn harp_selects_positions_across_twelve_tone_plate() {
    let p = harp(base_chord(), PParam::Const(Value::Int(0)), 12, None, None).unwrap();
    assert_eq!(tone(&run(&p, cycle(0)).events, 0), "Int(48)");
    for (position, expected) in [(0.5, "Int(72)"), (1.0, "Int(91)"), (1.7, "Int(91)")] {
        let p = harp(
            base_chord(),
            PParam::Const(Value::Float64(position)),
            12,
            None,
            None,
        )
        .unwrap();
        assert_eq!(tone(&run(&p, cycle(0)).events, 0), expected);
    }
}

#[test]
fn harp_uses_active_tuning_period() {
    let spec = crate::pattern::tuning::edo_spec(&Value::Int(19), None).unwrap();
    let tuned = Rc::new(crate::pattern::combinators::tune::tune(
        Rc::new(crate::pattern::build::pure(spec, None)),
        base_chord(),
        crate::pattern::tuning::Mapping::default(),
        None,
    ));
    let p = harp(tuned, PParam::Const(Value::Float64(0.0)), 12, None, None).unwrap();
    assert_eq!(tone(&run(&p, cycle(0)).events, 0), "Int(41)");
}

#[test]
fn harp_constructor_rejects_invalid_strip_count() {
    let err = harp(base_chord(), PParam::int(0), 0, None, None).unwrap_err();
    assert_eq!(err.code, FailCode::Type);
}

#[test]
fn inversion_handles_negative_multiple_periods_and_bass() {
    for (n, expected) in [
        (1, vec!["Int(64)", "Int(67)", "Int(72)"]),
        (-1, vec!["Int(55)", "Int(60)", "Int(64)"]),
        (3, vec!["Int(72)", "Int(76)", "Int(79)"]),
    ] {
        let p = inversion(base_chord(), PParam::int(n), false, None);
        assert_eq!(notes(&run(&p, cycle(0)).events), expected);
    }
    let p = inversion(base_chord(), PParam::int(0), true, None);
    assert_eq!(
        notes(&run(&p, cycle(0)).events),
        vec!["Int(48)", "Int(60)", "Int(64)", "Int(67)"]
    );
    let p = inversion(base_chord(), PParam::int(1), true, None);
    assert_eq!(
        notes(&run(&p, cycle(0)).events),
        vec!["Int(48)", "Int(64)", "Int(67)", "Int(72)"]
    );
}

#[test]
fn inversion_parameter_walks_per_step() {
    let n = crate::pattern::build::pattern_of(&list(vec![int(0), int(1), int(2), int(1)]), None)
        .unwrap();
    let four_events = Rc::new(crate::pattern::combinators::time::fast(
        base_chord(),
        PParam::int(4),
        None,
    ));
    let p = inversion(four_events, PParam::Pat(n), false, None);
    let mut events = run(&p, cycle(0)).events;
    events.sort_by_key(|event| event.anchor());
    let note_lists = events
        .iter()
        .map(|event| notes(std::slice::from_ref(event)))
        .collect::<Vec<_>>();
    assert_eq!(
        note_lists,
        vec![
            vec!["Int(60)", "Int(64)", "Int(67)"],
            vec!["Int(64)", "Int(67)", "Int(72)"],
            vec!["Int(67)", "Int(72)", "Int(76)"],
            vec!["Int(64)", "Int(67)", "Int(72)"],
        ]
    );
}
