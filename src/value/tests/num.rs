use super::r;
use crate::value::num::join;
use crate::value::{widen, NumKind, Value};
use crate::vm::fail::FailCode;

use NumKind::{Float, Float64, Int, Int64, Ratio};

const KINDS: [NumKind; 5] = [Int, Int64, Float, Float64, Ratio];

#[test]
fn all_fifteen_join_pairs() {
    let table = [
        (Int, Int, Int),
        (Int, Int64, Int64),
        (Int, Float, Float),
        (Int, Float64, Float64),
        (Int, Ratio, Ratio),
        (Int64, Int64, Int64),
        (Int64, Float, Float64),
        (Int64, Float64, Float64),
        (Int64, Ratio, Ratio),
        (Float, Float, Float),
        (Float, Float64, Float64),
        (Float, Ratio, Float),
        (Float64, Float64, Float64),
        (Float64, Ratio, Float64),
        (Ratio, Ratio, Ratio),
    ];
    assert_eq!(table.len(), 15);
    for (a, b, want) in table {
        assert_eq!(join(a, b), want, "join({a:?}, {b:?})");
        assert_eq!(join(b, a), want, "join({b:?}, {a:?})");
    }
}

#[test]
fn kind_of() {
    assert_eq!(NumKind::of(&Value::Int(1)), Some(Int));
    assert_eq!(NumKind::of(&Value::Ratio(r(1, 2))), Some(Ratio));
    assert_eq!(NumKind::of(&Value::str("1")), None);
}

#[test]
fn allowed_widens() {
    assert!(matches!(
        widen(&Value::Int(3), Int64).unwrap(),
        Value::Int64(3)
    ));
    assert!(matches!(widen(&Value::Int(3), Float64).unwrap(), Value::Float64(x) if x == 3.0));
    assert!(matches!(widen(&Value::Int(3), Ratio).unwrap(), Value::Ratio(q) if q == r(3, 1)));
    assert!(matches!(widen(&Value::Int64(5), Float64).unwrap(), Value::Float64(x) if x == 5.0));
    assert!(matches!(widen(&Value::Int64(5), Ratio).unwrap(), Value::Ratio(q) if q == r(5, 1)));
    assert!(matches!(widen(&Value::Float(0.5), Float64).unwrap(), Value::Float64(x) if x == 0.5));
    assert!(matches!(widen(&Value::Ratio(r(1, 4)), Float).unwrap(), Value::Float(x) if x == 0.25));
    assert!(
        matches!(widen(&Value::Ratio(r(1, 4)), Float64).unwrap(), Value::Float64(x) if x == 0.25)
    );
    // int -> float is lossy past 2^24 and still accepted.
    let big = (1 << 24) + 1;
    assert!(
        matches!(widen(&Value::Int(big), Float).unwrap(), Value::Float(x) if x == 16_777_216.0)
    );
    // Same kind is the identity.
    for k in KINDS {
        let v = match k {
            Int => Value::Int(1),
            Int64 => Value::Int64(1),
            Float => Value::Float(1.0),
            Float64 => Value::Float64(1.0),
            Ratio => Value::Ratio(r(1, 2)),
        };
        assert_eq!(NumKind::of(&widen(&v, k).unwrap()), Some(k));
    }
}

#[test]
fn every_narrowing_is_type() {
    let samples = [
        Value::Int(1),
        Value::Int64(1),
        Value::Float(1.0),
        Value::Float64(1.0),
        Value::Ratio(r(1, 2)),
    ];
    let mut checked = 0;
    for v in &samples {
        let from = NumKind::of(v).unwrap();
        for to in KINDS {
            if join(from, to) != to {
                assert_eq!(
                    widen(v, to).unwrap_err().code,
                    FailCode::Type,
                    "{from:?} -> {to:?}"
                );
                checked += 1;
            }
        }
    }
    // Int64->Int, Int64->Float, Float->{Int,Int64,Ratio}, Float64->all 4, Ratio->{Int,Int64}.
    assert_eq!(checked, 11);
    assert_eq!(
        widen(&Value::str("1"), Float).unwrap_err().code,
        FailCode::Type
    );
}
