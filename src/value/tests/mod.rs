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
    ];
    let tags: Vec<&str> = values
        .iter()
        .map(|v| match v {
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
            "nil", "bool", "int", "int64", "float", "float64", "ratio", "keyword", "str", "list",
            "dict", "struct", "variant", "native", "inst", "range"
        ]
    );
}
