use super::{pair, strukt, variant};
use crate::value::{dict_from_pairs, first, get, index, len, truthy, RangeVal, Value};
use crate::vm::fail::FailCode;

#[test]
fn truthiness() {
    assert!(!truthy(&Value::Nil));
    assert!(!truthy(&Value::Bool(false)));
    assert!(truthy(&Value::Bool(true)));
    assert!(truthy(&Value::Int(0)));
    assert!(truthy(&Value::Float64(0.0)));
    assert!(truthy(&Value::str("")));
    assert!(truthy(&Value::list(vec![])));
}

#[test]
fn nil_punning_accessors() {
    assert!(matches!(
        index(&Value::Nil, &Value::Int(3)).unwrap(),
        Value::Nil
    ));
    assert!(matches!(
        get(&Value::Nil, &Value::kw("k")).unwrap(),
        Value::Nil
    ));
    assert_eq!(len(&Value::Nil).unwrap(), 0);
    assert!(matches!(first(&Value::Nil).unwrap(), Value::Nil));
    assert!(matches!(first(&Value::list(vec![])).unwrap(), Value::Nil));
}

#[test]
fn index_out_of_range_or_negative_is_nil() {
    let l = Value::list(vec![Value::Int(10), Value::Int(20)]);
    assert!(matches!(index(&l, &Value::Int(1)).unwrap(), Value::Int(20)));
    assert!(matches!(index(&l, &Value::Int(2)).unwrap(), Value::Nil));
    assert!(matches!(index(&l, &Value::Int(-1)).unwrap(), Value::Nil));
    assert!(matches!(
        index(&l, &Value::Int64(i64::MAX)).unwrap(),
        Value::Nil
    ));
    let range = Value::Range(RangeVal {
        start: 4,
        end: Some(6),
    });
    assert!(matches!(
        index(&range, &Value::Int(1)).unwrap(),
        Value::Int(5)
    ));
    assert!(matches!(index(&range, &Value::Int(2)).unwrap(), Value::Nil));
}

#[test]
fn float_index_is_type() {
    let l = Value::list(vec![Value::Int(10)]);
    assert_eq!(
        index(&l, &Value::Float(0.0)).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(index(&l, &Value::kw("a")).unwrap_err().code, FailCode::Type);
    assert_eq!(
        index(&Value::Int(1), &Value::Int(0)).unwrap_err().code,
        FailCode::Type
    );
}

#[test]
fn get_on_dict_and_struct() {
    let d = Value::dict(dict_from_pairs(&[pair(Value::kw("amp"), Value::Int(1))]).unwrap());
    assert!(matches!(get(&d, &Value::kw("amp")).unwrap(), Value::Int(1)));
    assert!(matches!(get(&d, &Value::kw("pan")).unwrap(), Value::Nil));
    assert_eq!(
        get(&d, &Value::list(vec![])).unwrap_err().code,
        FailCode::Type
    );
    let s = strukt("voice", &[("amp", Value::Int(1))]);
    assert!(matches!(get(&s, &Value::kw("amp")).unwrap(), Value::Int(1)));
    let v = variant("shape", "circle", &[("r", Value::Int(2))]);
    assert!(matches!(get(&v, &Value::kw("r")).unwrap(), Value::Int(2)));
}

#[test]
fn unknown_struct_field_is_unknown_field() {
    let s = strukt("voice", &[("amp", Value::Int(1))]);
    assert_eq!(
        get(&s, &Value::kw("nope")).unwrap_err().code,
        FailCode::UnknownField
    );
    let v = variant("shape", "circle", &[("r", Value::Int(2))]);
    assert_eq!(
        get(&v, &Value::kw("w")).unwrap_err().code,
        FailCode::UnknownField
    );
}

#[test]
fn len_of_collections() {
    assert_eq!(len(&Value::list(vec![Value::Nil, Value::Nil])).unwrap(), 2);
    assert_eq!(len(&Value::str("héllo")).unwrap(), 5);
    assert_eq!(
        len(&Value::Range(RangeVal {
            start: 2,
            end: Some(8)
        }))
        .unwrap(),
        6
    );
    assert_eq!(
        len(&Value::Range(RangeVal {
            start: 8,
            end: Some(2)
        }))
        .unwrap(),
        0
    );
    assert_eq!(
        len(&Value::Range(RangeVal {
            start: 0,
            end: None
        }))
        .unwrap_err()
        .code,
        FailCode::Type
    );
    assert_eq!(len(&Value::Int(3)).unwrap_err().code, FailCode::Type);
}
