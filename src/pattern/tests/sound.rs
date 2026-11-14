//! SOUND FIRST and the first-structure rule (design 10.1 goldens), with
//! `kit:` resolution through the stub `QueryVm`.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::*;
use crate::pattern::combinators::sound::sound;
use crate::pattern::combinators::structure::euclid;
use crate::pattern::combinators::time::fast;
use crate::pattern::pat::PParam;
use crate::value::intern::intern_kw;
use crate::value::key::Key;
use crate::value::value::{PathVal, Sound};
use crate::vm::fail::FailCode;

fn notes(events: &[Event]) -> Vec<(Ratio64, String)> {
    let mut v: Vec<(Ratio64, String)> = events
        .iter()
        .map(|e| (e.anchor(), name(ctl_of(e, "note").unwrap())))
        .collect();
    v.sort();
    v
}

fn sample(path: &str) -> Value {
    Value::Sound(Rc::new(Sound::Sample(PathVal {
        text: Rc::from(path),
        file: None,
    })))
}

fn kit(pairs: &[(&str, Value)]) -> Value {
    let map: BTreeMap<Key, Value> = pairs
        .iter()
        .map(|(k, v)| (Key::Kw(intern_kw(k)), v.clone()))
        .collect();
    Value::dict(map)
}

#[test]
fn first_list_step_gives_the_structure() {
    // s :pluck > note [:e2 :g2 :b2]: three events per cycle.
    let p = ctl(
        s(kw("pluck")),
        "note",
        list(vec![kw("e2"), kw("g2"), kw("b2")]),
    );
    assert!(p.structured);
    let ev = run(&p, cycle(0)).events;
    assert_eq!(
        notes(&ev),
        vec![
            (r(0, 1), "e2".into()),
            (r(1, 3), "g2".into()),
            (r(2, 3), "b2".into())
        ]
    );
    assert!(ev.iter().all(|e| name(&e.value) == "pluck"));
}

#[test]
fn structured_subject_samples_later_lists() {
    // s [:bd :sn] > n [0 1 2 3]: two events with n 0 and 2.
    let p = ctl(
        s(list(vec![kw("bd"), kw("sn")])),
        "n",
        list(vec![int(0), int(1), int(2), int(3)]),
    );
    let ev = sorted(run(&p, cycle(0)).events);
    assert_eq!(ev.len(), 2);
    assert_eq!(show(ctl_of(&ev[0], "n").unwrap()), show(&int(0)));
    assert_eq!(show(ctl_of(&ev[1], "n").unwrap()), show(&int(2)));
    assert_eq!(name(&ev[1].value), "sn");
}

#[test]
fn the_first_list_wins_even_for_gain() {
    // s :pluck > gain [0.5 1] > note [:c :e :g]: gain gave the structure.
    let g = ctl(
        s(kw("pluck")),
        "gain",
        list(vec![Value::Float(0.5), int(1)]),
    );
    let p = ctl(g, "note", list(vec![kw("c"), kw("e"), kw("g")]));
    let ev = run(&p, cycle(0)).events;
    assert_eq!(
        notes(&ev),
        vec![(r(0, 1), "c".into()), (r(1, 2), "e".into())]
    );
    let ev = sorted(ev);
    assert_eq!(
        show(ctl_of(&ev[0], "gain").unwrap()),
        show(&Value::Float(0.5))
    );
}

#[test]
fn euclid_and_scalar_controls_on_one_sound() {
    // s :bd > euclid 3 8: three onsets.
    let e = euclid(
        s(kw("bd")),
        PParam::int(3),
        PParam::int(8),
        PParam::int(0),
        None,
    );
    assert_eq!(
        onsets(&run(&e, cycle(0)).events),
        vec![r(0, 1), r(3, 8), r(3, 4)]
    );
    // s :bd > gain 0.5: still unstructured; one event per cycle at a sink.
    let g = ctl(s(kw("bd")), "gain", Value::Float(0.5));
    assert!(!g.structured);
    let ev = run(&g, span(r(0, 1), r(2, 1))).events;
    assert_eq!(timing(&ev), vec![(r(0, 1), r(1, 1)), (r(1, 1), r(1, 1))]);
    // Any other operator realizes the single sound per cycle first (M4).
    let f = fast(s(kw("bd")), PParam::int(2), None);
    assert_eq!(onsets(&run(&f, cycle(0)).events), vec![r(0, 1), r(1, 2)]);
    // s [..] is structured from the start.
    assert!(s(list(vec![kw("bd")])).structured);
    assert!(!s(kw("bd")).structured);
}

#[test]
fn kit_argument_resolves_without_the_session_kit() {
    let tr909 = kit(&[
        ("bd909", sample("./tr909/bd.wav")),
        ("sd909", sample("./tr909/sd.wav")),
    ]);
    let src = PParam::Pat(pat(list(vec![kw("bd909"), kw("sd909")])));
    let p = sound(src, Some(PParam::Const(tr909)), None);
    let mut vm = StubVm::new();
    let res = run_with(&p, cycle(0), &mut vm);
    assert!(res.faults.is_empty());
    assert_eq!(vm.kit_calls, 0, "kit: never reads sound_kit()");
    let ev = sorted(res.events);
    assert_eq!(show(&ev[0].value), show(&sample("./tr909/bd.wav")));
    assert_eq!(show(&ev[1].value), show(&sample("./tr909/sd.wav")));
}

#[test]
fn the_session_kit_is_read_per_query_and_late_bound() {
    let p = s(kw("bd"));
    let mut vm = StubVm::new();
    vm.kit = kit(&[("bd", builtin("bd"))]);
    let a = run_with(&p, cycle(0), &mut vm);
    assert_eq!(vm.kit_calls, 1);
    assert_eq!(name(&a.events[0].value), "bd");
    // A kit swap between two queries changes the resolved sound.
    vm.kit = kit(&[("bd", sample("./bd/909.wav"))]);
    let b = run_with(&p, cycle(0), &mut vm);
    assert_eq!(vm.kit_calls, 2);
    assert_eq!(show(&b.events[0].value), show(&sample("./bd/909.wav")));
}

#[test]
fn a_missing_key_is_an_event_local_unknown_sound() {
    let p = s(list(vec![kw("bd"), kw("nope"), kw("sd")]));
    let res = run(&p, cycle(0));
    assert_eq!(names(&res.events).len(), 2);
    assert_eq!(res.faults.len(), 1);
    assert_eq!(res.faults[0].code, FailCode::UnknownSound);
    assert_eq!(res.faults[0].origin.beat, Some(r(4, 3)));
}

#[test]
fn a_sound_value_is_used_as_is() {
    // let kick sample ./kick.wav; s kick
    let kick = sample("./kick.wav");
    let p = sound(PParam::Const(kick.clone()), None, None);
    let mut vm = StubVm::new();
    let res = run_with(&p, cycle(0), &mut vm);
    assert_eq!(show(&res.events[0].value), show(&kick));
    // s {midi 1}: a MIDI-out instrument.
    let midi = Value::Sound(Rc::new(Sound::MidiOut(1)));
    let m = ctl(
        Rc::new(sound(PParam::Const(midi.clone()), None, None)),
        "note",
        list(vec![kw("c"), kw("e"), kw("g")]),
    );
    let ev = run(&m, cycle(0)).events;
    assert_eq!(ev.len(), 3);
    assert!(ev.iter().all(|e| show(&e.value) == show(&midi)));
}

#[test]
fn a_bank_with_n_picks_the_ith_sound() {
    let bank = Value::list(vec![sample("./bd/1.wav"), sample("./bd/2.wav")]);
    let mut vm = StubVm::new();
    vm.kit = kit(&[("bd", bank)]);
    let p = ctl(s(kw("bd")), "n", int(1));
    let res = run_with(&p, cycle(0), &mut vm);
    assert_eq!(show(&res.events[0].value), show(&sample("./bd/2.wav")));
    // No n: the first sound. An index outside the bank: slice-index.
    let plain = run_with(&s(kw("bd")), cycle(0), &mut vm);
    assert_eq!(show(&plain.events[0].value), show(&sample("./bd/1.wav")));
    let out = ctl(s(kw("bd")), "n", list(vec![int(0), int(5)]));
    let res = run_with(&out, cycle(0), &mut vm);
    assert_eq!(res.events.len(), 1);
    assert_eq!(res.faults.len(), 1);
    assert_eq!(res.faults[0].code, FailCode::SliceIndex);
}
