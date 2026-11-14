//! Unit tests for the value model (FE-VALUE required tests).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::value::{intern_kw, intern_sym, KwId, RangeVal, Ratio64, StructVal, Value, VariantVal};

mod access;
mod dict;
mod eq;
mod intern;
mod key;
mod num;
mod pathval;
mod print;
mod ratio;

pub(super) fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).expect("valid ratio")
}

pub(super) fn pair(k: Value, v: Value) -> Value {
    Value::list(vec![k, v])
}

pub(super) fn fields(items: &[(&str, Value)]) -> Box<[(KwId, Value)]> {
    let mut out: Vec<(KwId, Value)> = items
        .iter()
        .map(|(k, v)| (intern_kw(k), v.clone()))
        .collect();
    out.sort_by(|a, b| crate::value::name_of_kw(a.0).cmp(&crate::value::name_of_kw(b.0)));
    out.into_boxed_slice()
}

pub(super) fn strukt(ty: &str, items: &[(&str, Value)]) -> Value {
    Value::Struct(Rc::new(StructVal {
        ty: intern_sym(ty),
        fields: fields(items),
    }))
}

pub(super) fn variant(enum_ty: &str, tag: &str, items: &[(&str, Value)]) -> Value {
    Value::Variant(Rc::new(VariantVal {
        enum_ty: intern_sym(enum_ty),
        tag: intern_sym(tag),
        fields: fields(items),
    }))
}

#[test]
fn tagging_every_non_shell_variant() {
    let values = vec![
        finite_values()[0].clone(),
        finite_values()[1].clone(),
        finite_values()[2].clone(),
        Value::Nil,
        Value::Bool(true),
        Value::Int(1),
        Value::Int64(2),
        Value::Float(0.5),
        Value::Float64(0.25),
        Value::Ratio(r(3, 8)),
        Value::kw("kick"),
        Value::str("hi"),
        Value::list(vec![Value::Int(1)]),
        Value::dict(BTreeMap::new()),
        strukt("voice", &[("amp", Value::Float(1.0))]),
        variant("shape", "circle", &[("r", Value::Float(1.0))]),
        Value::Native(crate::value::NativeId::new(7)),
        Value::Inst(crate::dsp::graph::InstId::new(3)),
        Value::Range(RangeVal {
            start: 0,
            end: Some(8),
        }),
        Value::path("/abs/x", None),
        Value::url("https://example.org/x.vact"),
        Value::Sound(Rc::new(crate::value::value::Sound::MidiOut(1))),
        Value::UGen(Rc::new(crate::dsp::graph::UGenNode {
            kind: crate::dsp::graph::UGenKind::BusInput,
            args: Box::new([]),
        })),
    ];
    let tags: Vec<&str> = values
        .iter()
        .map(|v| match v {
            Value::Part(_) => "part",
            Value::Song(_) => "song",
            Value::EventHandle(_) => "event-handle",
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Int64(_) => "int64",
            Value::Float(_) => "float",
            Value::Float64(_) => "float64",
            Value::Ratio(_) => "ratio",
            Value::Keyword(_) => "keyword",
            Value::Str(_) => "str",
            Value::List(_) => "list",
            Value::Dict(_) => "dict",
            Value::Struct(_) => "struct",
            Value::Variant(_) => "variant",
            Value::Native(_) => "native",
            Value::Inst(_) => "inst",
            Value::Range(_) => "range",
            Value::Path(_) => "path",
            Value::Url(_) => "url",
            Value::Sound(_) => "sound",
            Value::UGen(_) => "ugen",
            Value::Fn(_)
            | Value::Thunk(_)
            | Value::VarRef(_)
            | Value::Pattern(_)
            | Value::Signal(_)
            | Value::Tex(_) => "shell",
        })
        .collect();
    assert_eq!(
        tags,
        [
            "part",
            "song",
            "event-handle",
            "nil",
            "bool",
            "int",
            "int64",
            "float",
            "float64",
            "ratio",
            "keyword",
            "str",
            "list",
            "dict",
            "struct",
            "variant",
            "native",
            "inst",
            "range",
            "path",
            "url",
            "sound",
            "ugen"
        ]
    );
}

#[test]
fn sound_buffer_prints_by_state_and_equals_only_itself() {
    use crate::value::deep_eq;
    use crate::value::sample::SampleBuf;
    use crate::value::value::Sound;
    use crate::vm::fail::FailCode;

    let sound = |b: &Rc<SampleBuf>| Value::Sound(Rc::new(Sound::Buffer(Rc::clone(b))));
    let pending = SampleBuf::pending(48_000);
    assert_eq!(sound(&pending).to_string(), "(sound buffer pending)");
    pending.fill(vec![0.0_f32; 6]);
    assert_eq!(sound(&pending).to_string(), "(sound buffer 3 frames)");
    pending.fail(FailCode::HostUnavailable, "no taps");
    assert_eq!(sound(&pending).to_string(), "(sound buffer failed)");

    // Equal only to itself, never to a buffer with the same frames.
    let a = SampleBuf::ready(48_000, vec![0.5_f32, 0.5]);
    let b = SampleBuf::ready(48_000, vec![0.5_f32, 0.5]);
    assert_ne!(a.id, b.id);
    assert!(deep_eq(&sound(&a), &sound(&a)).expect("comparable"));
    assert!(!deep_eq(&sound(&a), &sound(&b)).expect("comparable"));
    assert_eq!(a.frames(), Some(1));
}

fn finite_values() -> [Value; 3] {
    use crate::song::*;
    let part = Rc::new(capture_part(BTreeMap::new(), Ratio64::ONE).unwrap());
    let handle = EventHandle::issue(
        part.revision(),
        intern_kw("drums"),
        PlacementPath(vec![0, 2]),
        OccurrencePath {
            producer_ordinals: vec![1, 4, 0],
            cycle: 3,
            onset: r(7, 2),
        },
        1,
    );
    let song = Rc::new(Song::new(Rc::clone(&part), SongSettings::default()).unwrap());
    [
        Value::Part(part),
        Value::Song(song),
        Value::EventHandle(Rc::new(handle)),
    ]
}

#[test]
fn finite_handle_full_equality_and_opaque_descriptors() {
    use crate::song::*;
    use crate::value::deep_eq;
    let values = finite_values();
    for (value, text, kind) in [
        (&values[0], "<part>", "a part"),
        (&values[1], "<song>", "a song"),
        (&values[2], "<event-handle>", "an event handle"),
    ] {
        assert_eq!(value.to_string(), text);
        assert_eq!(crate::vm::call::kind_name(value), kind);
        assert!(crate::pattern::build::pattern_of(value, None).is_err());
        assert!(crate::pattern::build::param_of(value).is_err());
    }
    for value in &values[..2] {
        assert!(deep_eq(value, value).is_err());
    }
    let Value::EventHandle(handle) = &values[2] else {
        unreachable!()
    };
    assert!(deep_eq(&values[2], &values[2]).unwrap());
    for component in 0..7 {
        let different = EventHandle::issue(
            PartRevision(handle.revision().0 + u64::from(component == 0)),
            if component == 1 {
                intern_kw("bass")
            } else {
                handle.track()
            },
            if component == 2 {
                PlacementPath(vec![0, 3])
            } else {
                handle.placement().clone()
            },
            OccurrencePath {
                producer_ordinals: if component == 3 {
                    vec![1, 4, 1]
                } else {
                    handle.occurrence().producer_ordinals.clone()
                },
                cycle: handle.occurrence().cycle + i64::from(component == 4),
                onset: if component == 6 {
                    r(9, 2)
                } else {
                    handle.occurrence().onset
                },
            },
            handle.tone() + u32::from(component == 5),
        );
        assert!(!deep_eq(&values[2], &Value::EventHandle(Rc::new(different))).unwrap());
    }
    let nested = Value::list(vec![Value::list(vec![values[2].clone()])]);
    assert!(crate::pattern::build::pattern_of(&nested, None).is_err());
}

#[test]
fn certified_handle_is_an_ordinary_value_not_a_callable() {
    use crate::ns::evaluator::Evaluator;
    use crate::ns::load::NoopHost;
    use crate::ns::namespace::Prelude;
    use crate::ns::stage::RecordingSink;
    use crate::vm::fail::FailCode;
    let handle = finite_values()[2].clone();
    let mut ev = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    let (vm, ns) = ev.vm_and_ns();
    let result = vm
        .call_value(
            ns,
            &Value::list(vec![handle.clone()]),
            vec![Value::Int(0)],
            vec![],
        )
        .unwrap();
    assert!(crate::value::deep_eq(&handle, &result).unwrap());
    assert_eq!(
        vm.call_value(ns, &handle, vec![], vec![]).unwrap_err().code,
        FailCode::NotCallable
    );
}
