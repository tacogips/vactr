use std::rc::Rc;

use super::*;
use crate::pattern::combinators::control::control;
use crate::pattern::tests::{ctl_of, cycle, int, konst, kw, list, pat, run, s, sorted};
use crate::pattern::tuning::Mapping;
use crate::value::intern::intern_kw;

fn edo19() -> Tuning {
    Tuning::edo(19, Ratio64::from_int(2)).expect("valid EDO")
}

fn n_degrees(degrees: &[i32]) -> Rc<Pat> {
    Rc::new(control(
        intern_kw("n"),
        pat(list(degrees.iter().copied().map(int).collect())),
        s(kw("pluck")),
        None,
    ))
}

fn with_tuning(subject: Rc<Pat>, tuning: &Tuning) -> Rc<Pat> {
    Rc::new(control(
        intern_kw("tuning"),
        konst(tuning.to_control()),
        subject,
        None,
    ))
}

fn notes(events: &[Event]) -> Vec<i64> {
    sorted(events.to_vec())
        .iter()
        .flat_map(|event| match ctl_of(event, "note").expect("note control") {
            Value::List(notes) => notes
                .items
                .iter()
                .map(|note| int_of(note).expect("integer chord tone"))
                .collect::<Vec<_>>(),
            note => vec![int_of(note).expect("integer note")],
        })
        .collect()
}

fn chord_subject(tuning: Option<&Tuning>, root: Value, quality: Value) -> Rc<Pat> {
    let chord_pattern = chord_pattern_of(&list(vec![root, quality]), None).expect("chord pattern");
    let subject = s(kw("piano"));
    let subject = tuning.map_or(subject.clone(), |tuning| with_tuning(subject, tuning));
    Rc::new(chord(chord_pattern, subject, None))
}

#[test]
fn legacy_scale_keeps_untuned_notes() {
    let scale = scale(
        n_degrees(&[0, 2, 4]),
        intern_kw("c"),
        intern_kw("major"),
        None,
    )
    .expect("legacy scale");
    let result = run(&scale, cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert_eq!(notes(&result.events), vec![60, 64, 67]);
}

#[test]
fn edo19_scale_preset_and_legacy_scale_map_degrees() {
    let tuned = with_tuning(n_degrees(&[0, 1, 2, 3, 4, 5, 6, 7]), &edo19());
    let preset_scale =
        scale(tuned, intern_kw("c"), intern_kw("edo19-major"), None).expect("microtonal preset");
    let result = run(&preset_scale, cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert_eq!(notes(&result.events), vec![60, 63, 66, 68, 71, 74, 77, 79]);

    let tuned = with_tuning(n_degrees(&[2]), &edo19());
    let scale =
        scale(tuned, intern_kw("c"), intern_kw("major"), None).expect("legacy scale under tuning");
    let result = run(&scale, cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert_eq!(notes(&result.events), vec![66]);
}

#[test]
fn tuning_root_maps_scale_note_names_and_is_not_overwritten() {
    let tuning = edo19()
        .with_mapping(&Mapping {
            root: Some(62),
            ..Mapping::default()
        })
        .expect("mapped root");
    let tuned = with_tuning(n_degrees(&[0]), &tuning);
    let scale =
        scale(tuned, intern_kw("d"), intern_kw("edo19-major"), None).expect("microtonal preset");
    let result = run(&scale, cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert_eq!(notes(&result.events), vec![62]);
    assert!(crate::value::eq::deep_eq(
        ctl_of(&result.events[0], "tuning").expect("tuning control"),
        &tuning.to_control()
    )
    .expect("deep equality"));
}

#[test]
fn untuned_microtonal_presets_insert_their_default_tuning() {
    for (name, size, period) in [("maqam-rast", 24, 2), ("bp-lambda", 13, 3)] {
        let degrees = if name == "maqam-rast" {
            &[0, 1, 2, 3][..]
        } else {
            &[0, 1, 2][..]
        };
        let scale = scale(n_degrees(degrees), intern_kw("c"), intern_kw(name), None)
            .expect("microtonal preset");
        let result = run(&scale, cycle(0));
        assert!(result.faults.is_empty(), "{:?}", result.faults);
        if name == "maqam-rast" {
            assert_eq!(notes(&result.events), vec![60, 64, 67, 70]);
        }
        let tuning_control = ctl_of(&result.events[0], "tuning").expect("inserted tuning");
        let inserted = Tuning::from_control(tuning_control)
            .expect("recognized inserted tuning")
            .expect("valid inserted tuning");
        assert_eq!(inserted.keys_per_period(), size);
        assert_eq!(inserted.period_cents(), 1200.0 * f64::from(period).log2());
    }
}

#[test]
fn twelve_key_tuning_uses_direct_legacy_scale_steps() {
    let tuning = Tuning::from_spec(&Value::kw("ji-5")).expect("JI preset");
    let scale = scale(
        with_tuning(n_degrees(&[0, 1, 2]), &tuning),
        intern_kw("c"),
        intern_kw("major"),
        None,
    )
    .expect("legacy scale under JI");
    let result = run(&scale, cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert_eq!(notes(&result.events), vec![60, 62, 64]);
}

#[test]
fn chords_map_intervals_and_keyword_roots_through_tuning() {
    let result = run(&chord_subject(Some(&edo19()), kw("c"), kw("maj")), cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert!(crate::value::eq::deep_eq(
        ctl_of(&result.events[0], "note").expect("note control"),
        &list(vec![int(60), int(66), int(71)])
    )
    .expect("deep equality"));

    let result = run(&chord_subject(Some(&edo19()), kw("d"), kw("m")), cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    let Value::List(notes) = ctl_of(&result.events[0], "note").expect("chord notes") else {
        panic!("chord note control is not a list");
    };
    assert_eq!(int_of(&notes.items[0]), Some(63));
}

#[test]
fn tuned_chord_voicing_uses_period_pitch_classes() {
    let chord = chord_subject(Some(&edo19()), kw("c"), kw("maj"));
    let voicing = voicing(chord, None);
    let result = run(&voicing, cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    let voiced = notes(&result.events);
    assert_eq!(voiced[0], 60);
    assert!(voiced.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(voiced.iter().all(|note| (60..79).contains(note)));
}

#[test]
fn untuned_chord_voicing_keeps_legacy_values() {
    let voicing = voicing(chord_subject(None, kw("c"), kw("maj7")), None);
    let result = run(&voicing, cycle(0));
    assert!(result.faults.is_empty(), "{:?}", result.faults);
    assert!(crate::value::eq::deep_eq(
        ctl_of(&result.events[0], "note").expect("note control"),
        &list(vec![int(60), int(64), int(67), int(71)])
    )
    .expect("deep equality"));
}

#[test]
fn unknown_scale_error_is_unchanged() {
    let failure = scale(n_degrees(&[0]), intern_kw("c"), intern_kw("nope"), None)
        .expect_err("unknown scale must fail");
    assert_eq!(failure.message, "unknown scale :nope");
}
