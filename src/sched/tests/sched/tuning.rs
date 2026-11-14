//! Scheduler frequency resolution under recognized tuning controls.

use super::{cval, pat_of, with_ctl, Rig};
use crate::host::caps::MidiEvent;
use crate::host::testing::SinkCall;
use crate::ns::stage::SlotKey;
use crate::pattern::tuning::{scala_spec, Mapping, Tuning};
use crate::sched::commit::note_to_freq;
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;

fn bind_tuned(rig: &mut Rig, slot: u8, source: &str, tuning: Tuning) {
    let pattern = with_ctl(pat_of(rig, source), "tuning", tuning.to_control());
    assert!(rig.bind(SlotKey::D(slot), pattern).faults.is_empty());
    rig.run_to(0.1);
}

#[test]
fn untuned_note_resolution_retains_exact_12_tet_bits() {
    let mut rig = Rig::new();
    let pattern = with_ctl(
        pat_of(&mut rig, "s :analog"),
        "note",
        Value::list(vec![
            Value::Int(60),
            Value::Float(60.5),
            Value::Keyword(intern_kw("d")),
            Value::Int(69),
        ]),
    );
    assert!(rig.bind(SlotKey::D(1), pattern).faults.is_empty());
    rig.run_to(0.1);
    let notes = [60.0, 60.5, 62.0, 69.0];
    let events = rig.sent();
    assert_eq!(events.len(), notes.len());
    for ((_, event), note) in events.iter().zip(notes) {
        assert_eq!(
            cval(event, "freq").expect("frequency").to_bits(),
            (note_to_freq(note) as f32).to_bits()
        );
    }
}

#[test]
fn edo_tunes_numeric_and_keyword_notes_and_respects_custom_root() {
    let mut rig = Rig::new();
    let edo = Tuning::edo(19, Ratio64::from_int(2)).expect("EDO");
    bind_tuned(&mut rig, 1, "s :analog > note 66", edo.clone());
    let expected = (note_to_freq(60.0) * (6.0_f64 / 19.0).exp2()) as f32;
    let actual = cval(&rig.sent()[0].1, "freq").expect("frequency");
    assert!(actual.to_bits().abs_diff(expected.to_bits()) <= 1);

    let mut rig = Rig::new();
    bind_tuned(&mut rig, 1, "s :analog > note :d", edo);
    let expected = (note_to_freq(60.0) * (3.0_f64 / 19.0).exp2()) as f32;
    assert!(
        cval(&rig.sent()[0].1, "freq")
            .expect("frequency")
            .to_bits()
            .abs_diff(expected.to_bits())
            <= 1
    );

    let mut rig = Rig::new();
    let rooted = Tuning::edo(19, Ratio64::from_int(2))
        .expect("EDO")
        .with_mapping(&Mapping {
            root: Some(62),
            ..Mapping::default()
        })
        .expect("root mapping");
    bind_tuned(&mut rig, 1, "s :analog > note :d", rooted);
    assert_eq!(
        cval(&rig.sent()[0].1, "freq").expect("frequency").to_bits(),
        (note_to_freq(62.0) as f32).to_bits()
    );
}

#[test]
fn ji_five_limit_chord_uses_tuned_ratios() {
    let mut rig = Rig::new();
    let tuning = Tuning::from_spec(&Value::Keyword(intern_kw("ji-5"))).expect("JI-5");
    let pattern = with_ctl(
        with_ctl(
            pat_of(&mut rig, "s :analog"),
            "note",
            Value::list(vec![Value::Int(60), Value::Int(64), Value::Int(67)]),
        ),
        "tuning",
        tuning.to_control(),
    );
    assert!(rig.bind(SlotKey::D(1), pattern).faults.is_empty());
    rig.run_to(0.1);
    let reference = note_to_freq(60.0) as f32;
    let expected = [reference, reference * 1.25, reference * 1.5];
    let events = rig.sent();
    assert_eq!(events.len(), expected.len());
    for ((_, event), hz) in events.iter().zip(expected) {
        assert!((cval(event, "freq").expect("frequency") - hz).abs() <= f32::EPSILON * hz);
    }
}

#[test]
fn scala_unmapped_key_drops_only_that_tone() {
    let spec = scala_spec(
        "two key scale\n2\n3/2\n2/1\n",
        Some("2\n60\n61\n60\n60\n261.6255653005986\n2\n0\nx\n"),
    )
    .expect("Scala tuning");
    let tuning = Tuning::from_spec(&spec).expect("valid keymap");
    let mut rig = Rig::new();
    bind_tuned(&mut rig, 1, "s :analog > note [60 61]", tuning);
    let events = rig.sent();
    assert_eq!(events.len(), 1);
    assert_eq!(
        cval(&events[0].1, "freq").expect("mapped tone").to_bits(),
        (note_to_freq(60.0) as f32).to_bits()
    );
    assert!(rig.faults().is_empty());
}

#[test]
fn tuned_var_driven_note_commits_a_constant_frequency() {
    let mut rig = Rig::new();
    rig.run("var n 60");
    let tuning = Tuning::edo(19, Ratio64::from_int(2)).expect("EDO");
    bind_tuned(&mut rig, 1, "s :analog > note n", tuning);
    assert!(matches!(
        crate::sched::tests::sched::ctl(&rig.sent()[0].1, "freq"),
        Some(crate::host::wire::Ctl::Const(_))
    ));
}

#[test]
fn midi_uses_nearest_twelve_tet_note_only_under_tuning() {
    let midi_notes = |rig: &Rig| {
        rig.midi_calls()
            .into_iter()
            .filter_map(|(_, call)| match call {
                SinkCall::Send(MidiEvent::Note { note, .. }) => Some(note),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut tuned_rig = Rig::new();
    let tuning = Tuning::edo(19, Ratio64::from_int(2)).expect("EDO");
    bind_tuned(&mut tuned_rig, 1, "s {midi 1} > note :d", tuning);
    assert_eq!(midi_notes(&tuned_rig), vec![62]);

    let mut tuned_numeric = Rig::new();
    let tuning = Tuning::edo(19, Ratio64::from_int(2)).expect("EDO");
    bind_tuned(&mut tuned_numeric, 1, "s {midi 1} > note 66", tuning);
    let frequency = note_to_freq(60.0) * (6.0_f64 / 19.0).exp2();
    let expected = (69.0 + 12.0 * (frequency / 440.0).log2()).round() as u8;
    assert_eq!(expected, 64);
    assert_eq!(midi_notes(&tuned_numeric), vec![expected]);

    let mut untuned_keyword = Rig::new();
    untuned_keyword.run("s {midi 1} > note :d > d1");
    untuned_keyword.run_to(0.1);
    assert_eq!(midi_notes(&untuned_keyword), vec![62]);

    let mut untuned_number = Rig::new();
    untuned_number.run("s {midi 1} > note 60.6 > d1");
    untuned_number.run_to(0.1);
    assert_eq!(midi_notes(&untuned_number), vec![61]);
}

#[test]
fn unrecognized_tuning_keeps_instrument_control_handling() {
    let mut rig = Rig::new();
    let pattern = with_ctl(
        pat_of(&mut rig, "s :analog > note 60"),
        "tuning",
        Value::Int(3),
    );
    assert!(rig.bind(SlotKey::D(1), pattern).faults.is_empty());
    rig.run_to(0.1);
    assert!(rig.faults().iter().any(|fault| fault
        .message
        .contains("unknown instrument control `tuning`")));
}

#[test]
fn malformed_recognized_tuning_is_an_event_local_fault() {
    let mut rig = Rig::new();
    let pattern = with_ctl(
        pat_of(&mut rig, "s :analog > note 60"),
        "tuning",
        Value::list(vec![Value::Keyword(intern_kw("edo"))]),
    );
    assert!(rig.bind(SlotKey::D(1), pattern).faults.is_empty());
    rig.run_to(0.1);
    assert!(rig.faults().iter().any(|fault| {
        fault.code == crate::vm::fail::FailCode::Type && fault.message.contains("tuning")
    }));
    assert!(rig.sent().is_empty());
}
