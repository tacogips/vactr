//! Ordinary value transport stays distinct from finite song APIs.
#![allow(dead_code)]
mod support;
use std::collections::BTreeMap;
use std::rc::Rc;
use vactr::pattern::build::{param_of, pattern_of};
use vactr::reader::span::FileId;
use vactr::song::{capture_part, Song, SongSettings};
use vactr::value::{deep_eq, intern_kw, Key, Ratio64, Value};
use vactr::vm::fail::FailCode;

fn finite() -> [Value; 2] {
    let part = Rc::new(capture_part(BTreeMap::new(), Ratio64::ONE).unwrap());
    let song = Rc::new(Song::new(Rc::clone(&part), SongSettings::default()).unwrap());
    [Value::Part(part), Value::Song(song)]
}
fn same_identity(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Part(a), Value::Part(b)) => assert!(Rc::ptr_eq(a, b)),
        (Value::Song(a), Value::Song(b)) => assert!(Rc::ptr_eq(a, b)),
        _ => panic!("finite value changed kind"),
    }
}

#[test]
fn nested_functions_return_finite_values_without_coercion() {
    let text = "fn identity x:\n\tfirst [x]\nfn outer x:\n\tidentity {identity x}\nouter";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    let mut run = support::eval::run(text, &forms);
    let function = run.last().unwrap().as_ref().unwrap().clone();
    let (vm, ns) = run.ev.vm_and_ns();
    for value in finite() {
        let result = vm
            .call_value(ns, &function, vec![value.clone()], vec![])
            .unwrap();
        same_identity(&value, &result);
    }
}

#[test]
fn callable_lists_and_dicts_retain_finite_value_identity() {
    let mut ev = support::eval::evaluator();
    let (vm, ns) = ev.vm_and_ns();
    for value in finite() {
        let list = Value::list(vec![value.clone()]);
        let result = vm
            .call_value(ns, &list, vec![Value::Int(0)], vec![])
            .unwrap();
        same_identity(&value, &result);
        let dict = Value::dict(BTreeMap::from([(
            Key::Kw(intern_kw("part")),
            value.clone(),
        )]));
        let result = vm
            .call_value(ns, &dict, vec![Value::kw("part")], vec![])
            .unwrap();
        same_identity(&value, &result);
    }
}

#[test]
fn finite_values_are_opaque_and_not_callable() {
    let mut ev = support::eval::evaluator();
    let (vm, ns) = ev.vm_and_ns();
    for (value, display, kind) in [
        (finite()[0].clone(), "<part>", "a part"),
        (finite()[1].clone(), "<song>", "a song"),
    ] {
        assert_eq!(value.to_string(), display);
        assert_eq!(vactr::vm::call::kind_name(&value), kind);
        assert_eq!(deep_eq(&value, &value).unwrap_err().code, FailCode::Type);
        for args in [vec![], vec![Value::Int(0)]] {
            let failure = vm.call_value(ns, &value, args, vec![]).unwrap_err();
            assert_eq!(failure.code, FailCode::NotCallable);
            assert!(failure.message.contains(kind));
        }
        assert_eq!(
            deep_eq(&Value::list(vec![value.clone()]), &Value::list(vec![value]))
                .unwrap_err()
                .code,
            FailCode::Type
        );
    }
}

#[test]
fn implicit_coercion_rejects_finite_values_in_nested_containers() {
    for value in finite() {
        let dict = Value::dict(BTreeMap::from([(Key::Kw(intern_kw("x")), value.clone())]));
        for input in [
            value.clone(),
            Value::list(vec![value.clone()]),
            Value::list(vec![Value::list(vec![value])]),
            dict,
        ] {
            assert_eq!(pattern_of(&input, None).unwrap_err().code, FailCode::Type);
            assert_eq!(param_of(&input).unwrap_err().code, FailCode::Type);
        }
    }
}

#[test]
fn legacy_scalar_sound_and_nested_steps_still_coerce() {
    for value in [
        Value::Int(2),
        Value::kw("bd"),
        Value::list(vec![
            Value::Int(1),
            Value::list(vec![Value::kw("bd"), Value::kw("hh")]),
        ]),
    ] {
        let pattern = pattern_of(&value, None).unwrap();
        assert!(param_of(&value).is_ok());
        let preserved = pattern_of(&Value::Pattern(Rc::clone(&pattern)), None).unwrap();
        assert!(Rc::ptr_eq(&pattern, &preserved));
    }
}

#[test]
fn native_pattern_chains_reject_finite_arguments_and_preserve_repeat() {
    for text in [
        "fn convert x:\n\ts x\nconvert",
        "fn control x:\n\ts :bd > gain x\ncontrol",
    ] {
        let forms = support::eval::clean_forms(text, FileId::new(1));
        let mut run = support::eval::run(text, &forms);
        let function = run.last().unwrap().as_ref().unwrap().clone();
        let (vm, ns) = run.ev.vm_and_ns();
        for value in finite() {
            assert_eq!(
                vm.call_value(ns, &function, vec![value], vec![])
                    .unwrap_err()
                    .code,
                FailCode::Type
            );
        }
    }
    let text = "repeat 4 3\ns :bd > gain 0.5";
    let forms = support::eval::clean_forms(text, FileId::new(1));
    let run = support::eval::run(text, &forms);
    assert_eq!(run.results[0].1.as_ref().unwrap().to_string(), "[4 4 4]");
    assert!(matches!(
        run.last().unwrap().as_ref().unwrap(),
        Value::Pattern(_)
    ));
}

#[test]
fn nesting_guard_is_bounded_without_recursive_conversion() {
    let mut value = Value::Int(1);
    for _ in 0..258 {
        value = Value::list(vec![value]);
    }
    assert_eq!(pattern_of(&value, None).unwrap_err().code, FailCode::Type);
    assert_eq!(param_of(&value).unwrap_err().code, FailCode::Type);
}
