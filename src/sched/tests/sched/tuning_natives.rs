//! Source-level scheduler coverage for the tuning and chord-performance natives.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::{cval, Rig};
use crate::host::caps::MidiEvent;
use crate::host::testing::SinkCall;
use crate::ns::evaluator::Evaluator;
use crate::ns::load::SourceLoader;
use crate::ns::namespace::Prelude;
use crate::ns::stage::RecordingSink;
use crate::ns::stage::SlotKey;
use crate::pattern::tuning::Tuning;
use crate::reader::span::FileId;
use crate::value::value::PathVal;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

fn close(actual: f32, expected: f32) {
    assert!(
        actual.to_bits().abs_diff(expected.to_bits()) <= 1,
        "{actual} != {expected}"
    );
}

fn tuned_events(source: &str) -> Vec<(f64, crate::host::wire::AudioEvent)> {
    tuned_events_until(source, 0.2)
}

fn tuned_events_until(source: &str, end: f64) -> Vec<(f64, crate::host::wire::AudioEvent)> {
    let mut rig = Rig::new();
    rig.run(source);
    rig.run_to(end);
    rig.sent()
}

#[test]
fn source_natives_resolve_edo_non_octave_ratio_and_reference_anchor() {
    let anchor = crate::sched::commit::note_to_freq(60.0) as f32;
    for (source, steps, period) in [
        (
            "s :analog > tune {edo 19} > note [60 66 79] > d1",
            19.0_f64,
            2.0_f64,
        ),
        (
            "s :analog > tune {edo 31} > note [60 70] > d1",
            31.0_f64,
            2.0_f64,
        ),
        (
            "s :analog > tune {edo 13 period: 3} > note [60 66 73] > d1",
            13.0_f64,
            3.0_f64,
        ),
    ] {
        let events = tuned_events_until(source, 2.1);
        let notes: &[i64] = if steps == 19.0 {
            &[60, 66, 79]
        } else if steps == 31.0 {
            &[60, 70]
        } else {
            &[60, 66, 73]
        };
        assert!(events.len() >= notes.len());
        for ((_, event), note) in events.iter().take(notes.len()).zip(notes) {
            let degree = (*note - 60) as f64;
            let expected = (f64::from(anchor) * period.powf(degree / steps)) as f32;
            close(cval(event, "freq").expect("freq"), expected);
        }
    }

    let ji = tuned_events(
        "s :analog > tune {ratios [16/15 9/8 6/5 5/4 4/3 45/32 3/2 8/5 5/3 9/5 15/8 2]} > note 64 > d1",
    );
    assert_eq!(ji.len(), 1);
    close(
        cval(&ji[0].1, "freq").expect("freq"),
        (f64::from(anchor) * 5.0 / 4.0) as f32,
    );

    let mapped = tuned_events("s :analog > tune {edo 19} ref-key: 69 ref-freq: 440 > note 69 > d1");
    assert_eq!(mapped.len(), 1);
    assert_eq!(
        cval(&mapped[0].1, "freq").expect("freq").to_bits(),
        440.0_f32.to_bits()
    );
}

#[test]
fn source_native_note_names_patterned_tunings_and_strum_are_queryable() {
    let events = tuned_events("s :analog > tune {edo 19} > note :d > d1");
    assert_eq!(events.len(), 1);
    close(
        cval(&events[0].1, "freq").expect("freq"),
        (crate::sched::commit::note_to_freq(60.0) * (3.0_f64 / 19.0).exp2()) as f32,
    );

    let rooted = tuned_events("s :analog > tune {edo 19} root: :d > note :d > d1");
    assert_eq!(rooted.len(), 1);
    assert_eq!(
        cval(&rooted[0].1, "freq").expect("freq").to_bits(),
        (crate::sched::commit::note_to_freq(62.0) as f32).to_bits()
    );

    let alternating = tuned_events_until(
        "s :analog > tune {alt {edo 19} {edo 31}} > note 66 > d1",
        2.1,
    );
    assert!(alternating.len() >= 2);
    assert_ne!(
        cval(&alternating[0].1, "freq")
            .expect("first freq")
            .to_bits(),
        cval(&alternating[1].1, "freq")
            .expect("second freq")
            .to_bits()
    );

    let strummed = tuned_events("s :analog > tune {edo 19} > chord [:c :maj] > strum 1/32 > d1");
    assert_eq!(strummed.len(), 3);
    assert_eq!(
        strummed.iter().map(|(_, e)| e.time).collect::<Vec<_>>(),
        vec![0.0, 0.0625, 0.125]
    );
    let anchor = crate::sched::commit::note_to_freq(60.0);
    for ((_, event), step) in strummed.iter().zip([0.0_f64, 6.0, 11.0]) {
        close(
            cval(event, "freq").expect("freq"),
            (anchor * (step / 19.0).exp2()) as f32,
        );
    }
}

#[test]
fn perform_composes_each_mode_and_rejects_unknown_mode() {
    for instrument in ["pd", "fm"] {
        for mode in ["block", "strum", "arp"] {
            let source = format!(
                "s :{instrument} > tune {{edo 19}} > perform [:c :maj] mode: :{mode} time: 1/32 > d1"
            );
            let events = if mode == "arp" {
                tuned_events_until(&source, 1.9)
            } else {
                tuned_events(&source)
            };
            let anchor = crate::sched::commit::note_to_freq(60.0);
            if mode == "strum" {
                assert_eq!(events.len(), 3);
                assert_eq!(
                    events.iter().map(|(_, e)| e.time).collect::<Vec<_>>(),
                    vec![0.0, 0.0625, 0.125]
                );
                for ((_, event), step) in events.iter().zip([0.0_f64, 6.0, 11.0]) {
                    close(
                        cval(event, "freq").expect("perform strum frequency"),
                        (anchor * (step / 19.0).exp2()) as f32,
                    );
                }
            } else if mode == "block" {
                assert_eq!(events.len(), 3, "{instrument} block event count");
                assert_eq!(
                    events.iter().map(|(_, e)| e.time).collect::<Vec<_>>(),
                    vec![0.0, 0.0, 0.0],
                    "{instrument} block onsets"
                );
                for ((_, event), step) in events.iter().zip([0.0_f64, 6.0, 11.0]) {
                    close(
                        cval(event, "freq").expect("perform block frequency"),
                        (anchor * (step / 19.0).exp2()) as f32,
                    );
                }
            } else {
                assert_eq!(events.len(), 3, "{instrument} arp event count");
                let onsets = events.iter().map(|(_, e)| e.time).collect::<Vec<_>>();
                for (actual, expected) in onsets.iter().zip([0.0, 2.0 / 3.0, 4.0 / 3.0]) {
                    assert!(
                        (actual - expected).abs() < 1e-6,
                        "{instrument} arp onsets: {onsets:?}"
                    );
                }
                for ((_, event), step) in events.iter().zip([0.0_f64, 6.0, 11.0]) {
                    close(
                        cval(event, "freq").expect("perform arp frequency"),
                        (anchor * (step / 19.0).exp2()) as f32,
                    );
                }
            }
        }

        let harp_source = format!(
            "s :{instrument} > tune {{edo 19}} > perform [:c :maj] mode: :harp pos: 0.5 > d1"
        );
        let harp_events = tuned_events(&harp_source);
        assert_eq!(harp_events.len(), 1, "{instrument} harp event count");
        assert_eq!(harp_events[0].1.time, 0.0, "{instrument} harp onset");
        let anchor = crate::sched::commit::note_to_freq(60.0);
        close(
            cval(&harp_events[0].1, "freq").expect("perform harp frequency"),
            (anchor * 2.0) as f32,
        );
    }

    let mut rig = Rig::new();
    let outcome = rig.eval("s :analog > perform [:c :maj] mode: :nope");
    assert!(matches!(
        outcome
            .last()
            .and_then(|row| row.value.as_ref().err())
            .map(|e| e.code),
        Some(FailCode::Type)
    ));
}

#[test]
fn midi_and_untuned_paths_keep_their_existing_note_mapping() {
    let mut rig = Rig::new();
    rig.run("s {midi 1} > tune {edo 19} > note :d > d1");
    rig.run_to(0.1);
    let midi = rig
        .midi_calls()
        .into_iter()
        .filter_map(|(_, call)| match call {
            SinkCall::Send(MidiEvent::Note { note, .. }) => Some(note),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(midi, vec![62]);

    let mut untuned = Rig::new();
    let notes = [
        Value::Int(60),
        Value::Float(61.5),
        Value::Keyword(crate::value::intern::intern_kw("d")),
    ];
    let pattern = super::with_ctl(
        super::pat_of(&mut untuned, "s :analog"),
        "note",
        Value::list(notes.to_vec()),
    );
    assert!(untuned.bind(SlotKey::D(1), pattern).faults.is_empty());
    untuned.run_to(0.1);
    let events = untuned.sent();
    assert_eq!(events.len(), 3);
    for ((_, event), note) in events.iter().zip([60.0, 61.5, 62.0]) {
        assert_eq!(
            cval(event, "freq").expect("freq").to_bits(),
            (crate::sched::commit::note_to_freq(note) as f32).to_bits()
        );
    }
}

struct FixtureLoader(BTreeMap<String, Rc<str>>);

impl SourceLoader for FixtureLoader {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        self.0
            .get(path.text.trim_start_matches("./"))
            .cloned()
            .map(|text| (FileId::new(23), text))
            .ok_or_else(|| Failure::new(FailCode::LoadFailed, "missing Scala fixture"))
    }
}

#[test]
fn scala_inline_loaded_and_unmapped_keyboard_entries_work() {
    let source =
        "s :analog > tune {scala \"three\\n3\\n100.0\\n3/2\\n2/1\"} > note [60 61 62] > d1";
    let events = tuned_events_until(source, 1.9);
    assert_eq!(events.len(), 3);
    let reference = crate::sched::commit::note_to_freq(60.0) as f32;
    for ((_, event), expected) in events.iter().zip([
        reference,
        reference * 2.0_f32.powf(100.0 / 1200.0),
        reference * 1.5,
    ]) {
        close(
            cval(event, "freq").expect("inline Scala frequency"),
            expected,
        );
    }

    let mut rig = Rig::new();
    let missing = rig.eval("load-scala ./scale.scl");
    assert!(matches!(
        missing
            .last()
            .and_then(|row| row.value.as_ref().err())
            .map(|e| e.code),
        Some(FailCode::HostUnavailable)
    ));

    let scl = include_str!("../../../../tests/fixtures/tuning/slendro.scl");
    let kbm = include_str!("../../../../tests/fixtures/tuning/slendro.kbm");
    let loader = FixtureLoader(BTreeMap::from([
        ("scale.scl".to_string(), Rc::from(scl)),
        ("scale.kbm".to_string(), Rc::from(kbm)),
    ]));
    let mut evaluator = Evaluator::new(
        Prelude::core(),
        Box::new(loader),
        Box::new(RecordingSink::default()),
    );
    let output = evaluator
        .eval_str(
            "let scale {load-scala ./scale.scl kbm: ./scale.kbm}",
            FileId::new(1),
        )
        .expect("reader");
    assert!(output.last().expect("form").value.is_ok());
    let spec = evaluator
        .ns()
        .session_value("scale")
        .expect("loaded Scala spec");
    let tuning = Tuning::from_spec(&spec).expect("loaded tuning");
    let mut loaded_rig = Rig::new();
    let pattern = super::with_ctl(
        super::with_ctl(
            super::pat_of(&mut loaded_rig, "s :analog"),
            "note",
            Value::list(vec![Value::Int(60), Value::Int(61), Value::Int(62)]),
        ),
        "tuning",
        tuning.to_control(),
    );
    assert!(loaded_rig.bind(SlotKey::D(1), pattern).faults.is_empty());
    loaded_rig.run_to(0.1);
    let loaded_events = loaded_rig.sent();
    assert_eq!(loaded_events.len(), 2);
    let reference = crate::sched::commit::note_to_freq(60.0) as f32;
    close(
        cval(&loaded_events[0].1, "freq").expect("loaded ref"),
        reference,
    );
    close(
        cval(&loaded_events[1].1, "freq").expect("loaded degree"),
        reference * 2.0_f32.powf(240.0 / 1200.0),
    );
    let output = evaluator
        .eval_str("let scale2 {load-scala ./scale.scl}", FileId::new(3))
        .expect("reader");
    assert!(output.last().expect("form").value.is_ok());
    let unmapped = tuned_events_until(
        "s :analog > tune {scala \"two\\n2\\n3/2\\n2/1\" kbm: \"2\\n60\\n61\\n60\\n60\\n261.6255653005986\\n2\\n0\\nx\"} > note [60 61 62] > d1",
        1.9,
    );
    assert_eq!(unmapped.len(), 1);
    let reference = crate::sched::commit::note_to_freq(60.0) as f32;
    close(
        cval(&unmapped[0].1, "freq").expect("mapped reference tone"),
        reference,
    );
}

#[test]
fn invalid_constant_tuning_spec_fails_at_the_call() {
    let mut rig = Rig::new();
    let invalid = rig.eval("s :analog > tune {edo 0}");
    assert!(invalid.last().is_some_and(|row| row.value.is_err()));
}
