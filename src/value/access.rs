//! Truthiness and nil-punning accessors (design 5.1, 6.5.3).
//!
//! Only accessors pun nil: indexing or keying `nil` gives `nil`, and
//! `len nil` is 0.

use crate::value::dict::pairs;
use crate::value::key::Key;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

fn type_err(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}

/// Only `nil` and `false` are falsy.
#[must_use]
pub const fn truthy(v: &Value) -> bool {
    !matches!(v, Value::Nil | Value::Bool(false))
}

fn int_of(v: &Value) -> Result<i64, Failure> {
    match v {
        Value::Int(i) => Ok(i64::from(*i)),
        Value::Int64(i) => Ok(*i),
        Value::Ratio(r) if r.is_integral() => Ok(r.num()),
        _ => Err(type_err("an index must be an integer")),
    }
}

fn int_value(n: i64) -> Value {
    match i32::try_from(n) {
        Ok(i) => Value::Int(i),
        Err(_) => Value::Int64(n),
    }
}

/// `index v i`, 0-based. Out of range (including negative) gives `nil`.
///
/// # Errors
/// `Type` for a non-integer index or a non-indexable value.
pub fn index(v: &Value, i: &Value) -> Result<Value, Failure> {
    if !matches!(
        v,
        Value::Nil | Value::List(_) | Value::Dict(_) | Value::Range(_)
    ) {
        return Err(type_err("index expects a list, dict or range"));
    }
    let i = int_of(i)?;
    let Ok(idx) = usize::try_from(i) else {
        return Ok(Value::Nil);
    };
    match v {
        Value::Nil => Ok(Value::Nil),
        Value::List(l) => Ok(l.items.get(idx).cloned().unwrap_or(Value::Nil)),
        Value::Dict(d) => Ok(pairs(d).nth(idx).unwrap_or(Value::Nil)),
        Value::Range(r) => {
            let Some(n) = r.start.checked_add(i) else {
                return Ok(Value::Nil);
            };
            match r.end {
                Some(end) if n >= end => Ok(Value::Nil),
                _ => Ok(int_value(n)),
            }
        }
        _ => Ok(Value::Nil),
    }
}

/// `get v key`. A dict miss gives `nil`.
///
/// # Errors
/// `Type` for a non-key or a non-keyed value; `UnknownField` for an unknown
/// struct or variant field.
pub fn get(v: &Value, key: &Value) -> Result<Value, Failure> {
    let fields = match v {
        Value::Nil => return Ok(Value::Nil),
        Value::Dict(d) => {
            let k = Key::from_value(key)?;
            return Ok(d.get(&k).cloned().unwrap_or(Value::Nil));
        }
        Value::Struct(s) => &s.fields,
        Value::Variant(s) => &s.fields,
        _ => return Err(type_err("get expects a dict or struct")),
    };
    let Value::Keyword(kw) = key else {
        return Err(Failure::new(
            FailCode::UnknownField,
            "struct fields are keywords",
        ));
    };
    fields
        .iter()
        .find(|(f, _)| f == kw)
        .map(|(_, val)| val.clone())
        .ok_or_else(|| Failure::new(FailCode::UnknownField, "unknown struct field"))
}

/// `len v`. `nil` gives 0.
///
/// # Errors
/// `Type` for a value without a length (including the open range);
/// `Overflow` for a range longer than the address space.
pub fn len(v: &Value) -> Result<usize, Failure> {
    match v {
        Value::Nil => Ok(0),
        Value::List(l) => Ok(l.items.len()),
        Value::Dict(d) => Ok(d.len()),
        Value::Str(s) => Ok(s.chars().count()),
        Value::Struct(s) => Ok(s.fields.len()),
        Value::Variant(s) => Ok(s.fields.len()),
        Value::Range(r) => match r.end {
            Some(end) => {
                let n = (i128::from(end) - i128::from(r.start)).max(0);
                usize::try_from(n).map_err(|_| Failure::new(FailCode::Overflow, "range too long"))
            }
            None => Err(type_err("an open range has no length")),
        },
        _ => Err(type_err("len expects a collection")),
    }
}

/// `first v`. `nil` or an empty collection gives `nil`; a dict gives its
/// smallest pair.
///
/// # Errors
/// `Type` for a non-collection.
pub fn first(v: &Value) -> Result<Value, Failure> {
    match v {
        Value::Nil => Ok(Value::Nil),
        Value::List(l) => Ok(l.items.first().cloned().unwrap_or(Value::Nil)),
        Value::Dict(d) => Ok(pairs(d).next().unwrap_or(Value::Nil)),
        Value::Range(_) => index(v, &Value::Int(0)),
        _ => Err(type_err("first expects a collection")),
    }
}
