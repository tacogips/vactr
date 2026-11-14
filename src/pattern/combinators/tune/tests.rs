use std::rc::Rc;

use crate::pattern::build::pure;
use crate::pattern::combinators::tune::tune;
use crate::pattern::tests::{ctl_of, cycle, konst, kw, list, run, s, show};
use crate::pattern::tuning::{edo_spec, Mapping, Tuning};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;

fn edo(steps: i32) -> Value {
    edo_spec(&Value::Int(steps), None).unwrap()
}

#[test]
fn tune_sets_canonical_control() {
    let p = tune(konst(edo(19)), s(kw("bd")), Mapping::default(), None);
    let result = run(&p, cycle(0));
    assert_eq!(result.events.len(), 1);
    assert_eq!(
        show(ctl_of(&result.events[0], "tuning").unwrap()),
        show(&Tuning::edo(19, Ratio64::from_int(2)).unwrap().to_control())
    );
}

#[test]
fn patterned_tunings_alternate_per_step() {
    let specs = crate::pattern::build::pattern_of(&list(vec![edo(19), edo(31)]), None).unwrap();
    let p = tune(specs, s(kw("bd")), Mapping::default(), None);
    let events = run(&p, cycle(0)).events;
    assert_eq!(events.len(), 2);
    assert_eq!(
        show(ctl_of(&events[0], "tuning").unwrap()),
        show(&Tuning::edo(19, Ratio64::from_int(2)).unwrap().to_control())
    );
    assert_eq!(
        show(ctl_of(&events[1], "tuning").unwrap()),
        show(&Tuning::edo(31, Ratio64::from_int(2)).unwrap().to_control())
    );
}

#[test]
fn invalid_tuning_is_an_event_local_fault() {
    let p = tune(konst(Value::Int(3)), s(kw("bd")), Mapping::default(), None);
    let result = run(&p, cycle(0));
    assert!(result.events.is_empty());
    assert_eq!(result.faults.len(), 1);
}

#[test]
fn mapping_is_encoded_in_canonical_control() {
    let mapping = Mapping {
        root: None,
        ref_key: Some(69),
        ref_freq: Some(440.0),
    };
    let p = tune(konst(edo(19)), s(kw("bd")), mapping, None);
    let result = run(&p, cycle(0));
    let value = ctl_of(&result.events[0], "tuning").unwrap();
    let decoded = Tuning::from_control(value).unwrap().unwrap();
    assert_eq!(show(&decoded.to_control()), show(value));
    assert_eq!(
        show(value),
        show(
            &Tuning::edo(19, Ratio64::from_int(2))
                .unwrap()
                .with_mapping(&mapping)
                .unwrap()
                .to_control()
        )
    );
    assert_ne!(
        show(value),
        show(&Tuning::edo(19, Ratio64::from_int(2)).unwrap().to_control())
    );
}

#[test]
fn pure_tuning_does_not_give_structure_to_unstructured_subject() {
    let subject = Rc::new(pure(kw("x"), None));
    let p = tune(konst(edo(19)), subject, Mapping::default(), None);
    assert!(!p.structured);
    let events = run(&p, cycle(0)).events;
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].whole.unwrap().duration().unwrap(),
        Ratio64::from_int(1)
    );
}
