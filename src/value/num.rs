//! The numeric widening lattice (design 6.5.3, lang-reference section 2).

use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// The five numeric kinds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NumKind {
    Int,
    Int64,
    Float,
    Float64,
    Ratio,
}

impl NumKind {
    /// The kind of a numeric value; `None` for a non-number.
    #[must_use]
    pub const fn of(v: &Value) -> Option<NumKind> {
        match v {
            Value::Int(_) => Some(NumKind::Int),
            Value::Int64(_) => Some(NumKind::Int64),
            Value::Float(_) => Some(NumKind::Float),
            Value::Float64(_) => Some(NumKind::Float64),
            Value::Ratio(_) => Some(NumKind::Ratio),
            _ => None,
        }
    }
}

/// The kind two operands widen to. Symmetric (design 6.5.3 table).
#[must_use]
pub const fn join(a: NumKind, b: NumKind) -> NumKind {
    use NumKind::{Float, Float64, Int, Int64, Ratio};
    match (a, b) {
        (Int, Int) => Int,
        (Int, Int64) | (Int64, Int) | (Int64, Int64) => Int64,
        (Int, Float) | (Float, Int) | (Float, Float) => Float,
        (Float, Ratio) | (Ratio, Float) => Float,
        (Int, Ratio) | (Ratio, Int) | (Int64, Ratio) | (Ratio, Int64) | (Ratio, Ratio) => Ratio,
        (Float64, _) | (_, Float64) | (Int64, Float) | (Float, Int64) => Float64,
    }
}

/// Converts a number upward along the lattice to `target`.
///
/// # Errors
/// `Type` for a non-number or for any narrowing or sideways request.
pub fn widen(v: &Value, target: NumKind) -> Result<Value, Failure> {
    let Some(kind) = NumKind::of(v) else {
        return Err(Failure::new(FailCode::Type, "widen expects a number"));
    };
    if join(kind, target) != target {
        return Err(Failure::new(
            FailCode::Type,
            format!("cannot widen {kind:?} to {target:?}; narrowing is explicit"),
        ));
    }
    Ok(match (v, target) {
        (Value::Int(i), NumKind::Int64) => Value::Int64(i64::from(*i)),
        // Lossy past 2^24; the lattice accepts it (lang-reference section 2).
        (Value::Int(i), NumKind::Float) => Value::Float(*i as f32),
        (Value::Int(i), NumKind::Float64) => Value::Float64(f64::from(*i)),
        (Value::Int(i), NumKind::Ratio) => Value::Ratio(Ratio64::from_int(i64::from(*i))),
        (Value::Int64(i), NumKind::Float64) => Value::Float64(*i as f64),
        (Value::Int64(i), NumKind::Ratio) => Value::Ratio(Ratio64::from_int(*i)),
        (Value::Float(x), NumKind::Float64) => Value::Float64(f64::from(*x)),
        (Value::Ratio(r), NumKind::Float) => Value::Float(r.to_f64() as f32),
        (Value::Ratio(r), NumKind::Float64) => Value::Float64(r.to_f64()),
        // Same kind.
        _ => v.clone(),
    })
}
