use super::{pair, r, strukt, variant};
use crate::value::{dict_from_pairs, NativeId, RangeVal, Value};

#[test]
fn floats() {
    assert_eq!(Value::Float(90.0).to_string(), "90.0");
    assert_eq!(Value::Float(1.0 / 6.0).to_string(), "0.16666667");
    assert_eq!(Value::Float64(0.5).to_string(), "0.5");
    assert_eq!(Value::Float64(-2.0).to_string(), "-2.0");
    assert_eq!(Value::Float64(f64::INFINITY).to_string(), "inf");
}

#[test]
fn numbers_and_keywords() {
    assert_eq!(Value::Int(-3).to_string(), "-3");
    assert_eq!(Value::Int64(1 << 40).to_string(), "1099511627776");
    assert_eq!(Value::Ratio(r(3, 8)).to_string(), "3/8");
    assert_eq!(Value::Ratio(r(4, 2)).to_string(), "2");
    assert_eq!(Value::kw("kick").to_string(), ":kick");
    assert_eq!(Value::Nil.to_string(), "nil");
    assert_eq!(Value::Bool(false).to_string(), "false");
}

#[test]
fn strings_raw_at_top_level_and_quoted_when_nested() {
    assert_eq!(Value::str("a \"b\"").to_string(), "a \"b\"");
    let l = Value::list(vec![Value::str("a \"b\"\n{x}\\")]);
    assert_eq!(l.to_string(), r#"["a \"b\"\n\{x\}\\"]"#);
}

#[test]
fn dict_with_pair_sugar() {
    let d = Value::dict(
        dict_from_pairs(&[
            pair(Value::kw("pan"), Value::Int(-1)),
            pair(Value::kw("amp"), Value::Float(0.5)),
        ])
        .unwrap(),
    );
    assert_eq!(d.to_string(), "[amp: 0.5 pan: -1]");
    let n = Value::dict(dict_from_pairs(&[pair(Value::Int(2), Value::str("x"))]).unwrap());
    assert_eq!(n.to_string(), "[[2 \"x\"]]");
}

#[test]
fn structs_variants_ranges_and_opaque() {
    assert_eq!(
        strukt("voice", &[("pan", Value::Int(0)), ("amp", Value::Int(1))]).to_string(),
        "voice amp: 1 pan: 0"
    );
    assert_eq!(
        variant("shape", "circle", &[("r", Value::Int(2))]).to_string(),
        "circle 2"
    );
    assert_eq!(
        Value::Range(RangeVal {
            start: 0,
            end: Some(8)
        })
        .to_string(),
        "0..8"
    );
    assert_eq!(
        Value::Range(RangeVal {
            start: 0,
            end: None
        })
        .to_string(),
        "0.."
    );
    assert_eq!(Value::Native(NativeId::new(0)).to_string(), "<native>");
    assert_eq!(
        Value::Inst(crate::dsp::graph::InstId::new(0)).to_string(),
        "<inst>"
    );
    assert_eq!(
        Value::list(vec![Value::Int(1), Value::list(vec![])]).to_string(),
        "[1 []]"
    );
}
