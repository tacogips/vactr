use super::{pair, r, strukt, variant};
use crate::dsp::graph::InstId;
use crate::value::{deep_eq, dict_from_pairs, NativeId, RangeVal, Value};
use crate::vm::fail::FailCode;

fn eq(a: &Value, b: &Value) -> bool {
    let res = deep_eq(a, b).expect("comparable");
    assert_eq!(res, deep_eq(b, a).expect("comparable"), "symmetry");
    res
}

#[test]
fn numbers_equal_across_widths() {
    assert!(eq(&Value::Int(1), &Value::Float(1.0)));
    assert!(eq(&Value::Int(1), &Value::Float64(1.0)));
    assert!(eq(&Value::Int64(1), &Value::Ratio(r(1, 1))));
    assert!(eq(&Value::Ratio(r(1, 2)), &Value::Float(0.5)));
    assert!(eq(&Value::Float64(-0.0), &Value::Int(0)));
    assert!(!eq(&Value::Ratio(r(1, 3)), &Value::Float64(1.0 / 3.0)));
    assert!(!eq(&Value::Int(1), &Value::Int(2)));
    assert!(!eq(&Value::Int(1), &Value::str("1")));
}

#[test]
fn nan_is_not_equal_to_nan() {
    assert!(!eq(&Value::Float64(f64::NAN), &Value::Float64(f64::NAN)));
    assert!(!eq(&Value::Float(f32::NAN), &Value::Int(0)));
}

#[test]
fn nested_lists() {
    let a = Value::list(vec![
        Value::Int(1),
        Value::list(vec![Value::kw("a"), Value::str("s")]),
    ]);
    let b = Value::list(vec![
        Value::Float(1.0),
        Value::list(vec![Value::kw("a"), Value::str("s")]),
    ]);
    let c = Value::list(vec![Value::Int(1), Value::list(vec![Value::kw("a")])]);
    assert!(eq(&a, &b));
    assert!(!eq(&a, &c));
}

#[test]
fn empty_list_equals_empty_dict() {
    assert!(eq(&Value::list(vec![]), &Value::dict(Default::default())));
}

#[test]
fn pair_list_equals_dict_in_key_order() {
    let d = Value::dict(
        dict_from_pairs(&[
            pair(Value::kw("pan"), Value::Int(0)),
            pair(Value::kw("amp"), Value::Int(1)),
        ])
        .unwrap(),
    );
    let in_order = Value::list(vec![
        pair(Value::kw("amp"), Value::Int(1)),
        pair(Value::kw("pan"), Value::Int(0)),
    ]);
    let out_of_order = Value::list(vec![
        pair(Value::kw("pan"), Value::Int(0)),
        pair(Value::kw("amp"), Value::Int(1)),
    ]);
    assert!(eq(&d, &in_order));
    assert!(!eq(&d, &out_of_order));
    let d2 = Value::dict(
        dict_from_pairs(&[
            pair(Value::kw("amp"), Value::Float(1.0)),
            pair(Value::kw("pan"), Value::Int(0)),
        ])
        .unwrap(),
    );
    assert!(eq(&d, &d2));
}

#[test]
fn structs_are_nominal() {
    let a = strukt("voice", &[("amp", Value::Int(1))]);
    let b = strukt("voice", &[("amp", Value::Float(1.0))]);
    let c = strukt("other", &[("amp", Value::Int(1))]);
    let d = Value::dict(dict_from_pairs(&[pair(Value::kw("amp"), Value::Int(1))]).unwrap());
    assert!(eq(&a, &b));
    assert!(!eq(&a, &c));
    assert!(!eq(&a, &d));
}

#[test]
fn variants_compare_tag_and_fields() {
    let a = variant("shape", "circle", &[("r", Value::Int(1))]);
    let b = variant("shape", "circle", &[("r", Value::Int(1))]);
    let c = variant("shape", "circle", &[("r", Value::Int(2))]);
    let d = variant("shape", "dot", &[("r", Value::Int(1))]);
    assert!(eq(&a, &b));
    assert!(!eq(&a, &c));
    assert!(!eq(&a, &d));
}

#[test]
fn inst_by_id_and_range_structural() {
    assert!(eq(
        &Value::Inst(InstId::new(1)),
        &Value::Inst(InstId::new(1))
    ));
    assert!(!eq(
        &Value::Inst(InstId::new(1)),
        &Value::Inst(InstId::new(2))
    ));
    let a = Value::Range(RangeVal {
        start: 0,
        end: Some(8),
    });
    assert!(eq(
        &a,
        &Value::Range(RangeVal {
            start: 0,
            end: Some(8)
        })
    ));
    assert!(!eq(
        &a,
        &Value::Range(RangeVal {
            start: 0,
            end: None
        })
    ));
}

#[test]
fn native_is_type() {
    let n = Value::Native(NativeId::new(1));
    assert_eq!(deep_eq(&n, &n).unwrap_err().code, FailCode::Type);
    assert_eq!(
        deep_eq(&Value::Int(1), &n).unwrap_err().code,
        FailCode::Type
    );
    let nested = Value::list(vec![n]);
    assert_eq!(deep_eq(&nested, &nested).unwrap_err().code, FailCode::Type);
}
