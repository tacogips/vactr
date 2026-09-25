use std::rc::Rc;

use super::r;
use crate::value::{intern_kw, Key, NumKey, Value};
use crate::vm::fail::FailCode;

fn key(v: &Value) -> Key {
    Key::from_value(v).expect("a key")
}

#[test]
fn numbers_before_keywords_before_strings() {
    let n = key(&Value::Int(1_000_000));
    let k = key(&Value::kw("aaa"));
    let s = key(&Value::str("a"));
    assert!(n < k && k < s && n < s);
}

#[test]
fn one_one_point_zero_and_one_over_one_are_one_key() {
    let a = key(&Value::Int(1));
    let b = key(&Value::Float(1.0));
    let c = key(&Value::Ratio(r(1, 1)));
    let d = key(&Value::Float64(1.0));
    let e = key(&Value::Int64(1));
    assert!(a == b && b == c && c == d && d == e);
    assert!(matches!(a.to_value(), Value::Int(1)));
}

#[test]
fn negative_zero_is_zero() {
    assert_eq!(key(&Value::Float64(-0.0)), key(&Value::Int(0)));
    assert_eq!(key(&Value::Float(-0.0)), key(&Value::Float64(0.0)));
}

#[test]
fn nan_key_is_type() {
    assert_eq!(
        Key::from_value(&Value::Float64(f64::NAN)).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(
        Key::from_value(&Value::Float(f32::NAN)).unwrap_err().code,
        FailCode::Type
    );
}

#[test]
fn keywords_order_by_name_not_id() {
    // Interned in reverse alphabetical order, so ids run opposite to names.
    let z = intern_kw("zz-order-test");
    let m = intern_kw("mm-order-test");
    let a = intern_kw("aa-order-test");
    assert!(z.get() < m.get() && m.get() < a.get());
    let mut keys = [Key::Kw(z), Key::Kw(a), Key::Kw(m)];
    keys.sort();
    let names: Vec<String> = keys.iter().map(|k| k.to_value().to_string()).collect();
    assert_eq!(
        names,
        [":aa-order-test", ":mm-order-test", ":zz-order-test"]
    );
}

#[test]
fn a_list_is_not_a_key() {
    assert_eq!(
        Key::from_value(&Value::list(vec![])).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(
        Key::from_value(&Value::Nil).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(
        Key::from_value(&Value::Bool(true)).unwrap_err().code,
        FailCode::Type
    );
}

#[test]
fn numeric_order_across_widths_and_exact_floats() {
    let mut keys = [
        key(&Value::Float64(1e30)),
        key(&Value::Int(2)),
        key(&Value::Ratio(r(1, 2))),
        key(&Value::Float64(1e-30)),
        key(&Value::Float(-1.5)),
        key(&Value::Float64(-1e30)),
        key(&Value::Int64(i64::MAX)),
    ];
    keys.sort();
    let want = [-1e30, -1.5, 1e-30, 0.5, 2.0, 9.223_372_036_854_776e18, 1e30];
    for (k, w) in keys.iter().zip(want) {
        let x = match k {
            Key::Num(NumKey::Exact(q)) => q.to_f64(),
            Key::Num(NumKey::Float(x)) => *x,
            _ => panic!("not a number key"),
        };
        assert_eq!(x, w);
    }
    // A float that is not a Ratio64 sorts strictly by value against nearby exact keys.
    let tiny = NumKey::from_f64(1e-30).unwrap();
    assert!(matches!(tiny, NumKey::Float(_)));
    assert!(NumKey::Exact(r(0, 1)) < tiny);
    assert!(tiny < NumKey::Exact(r(1, i64::MAX)));
    let huge = NumKey::from_f64(1e19).unwrap();
    assert!(NumKey::Exact(r(i64::MAX, 1)) < huge);
}

#[test]
fn string_keys_order_alphabetically() {
    assert!(Key::Str(Rc::from("a")) < Key::Str(Rc::from("b")));
}
