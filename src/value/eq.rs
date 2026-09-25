//! Structural equality (design 5.1, 6.5.3).

use std::cmp::Ordering;
use std::rc::Rc;

use crate::value::dict::pairs;
use crate::value::intern::KwId;
use crate::value::key::NumKey;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

const fn is_opaque(v: &Value) -> bool {
    matches!(
        v,
        Value::Fn(_)
            | Value::Native(_)
            | Value::Thunk(_)
            | Value::VarRef(_)
            | Value::Pattern(_)
            | Value::Signal(_)
            | Value::Tex(_)
    )
}

const fn is_nan(v: &Value) -> bool {
    match v {
        Value::Float(x) => x.is_nan(),
        Value::Float64(x) => x.is_nan(),
        _ => false,
    }
}

/// Deep structural equality (`=`).
///
/// Numbers compare by exact value across widths (NaN is not equal to
/// itself). A dict equals a list whose elements are its pairs in key order.
/// Structs are nominal; variants compare tag and fields. A path compares
/// text and containing file, a url its text, and a sound structurally; a
/// unit generator is equal only to itself (pointer equality).
///
/// # Errors
/// `Type` when either side is a function, native, thunk, var ref, pattern,
/// signal or visual chain.
pub fn deep_eq(a: &Value, b: &Value) -> Result<bool, Failure> {
    if is_opaque(a) || is_opaque(b) {
        return Err(Failure::new(
            FailCode::Type,
            "these values have no equality",
        ));
    }
    if is_nan(a) || is_nan(b) {
        return Ok(false);
    }
    if let (Some(x), Some(y)) = (NumKey::from_value(a)?, NumKey::from_value(b)?) {
        return Ok(x.cmp(&y) == Ordering::Equal);
    }
    Ok(match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Keyword(x), Value::Keyword(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(x), Value::List(y)) => {
            x.items.len() == y.items.len()
                && seq_eq(x.items.iter().cloned(), y.items.iter().cloned())?
        }
        (Value::Dict(x), Value::Dict(y)) => {
            x.len() == y.len() && {
                let mut all = true;
                for ((kx, vx), (ky, vy)) in x.iter().zip(y.iter()) {
                    if kx != ky || !deep_eq(vx, vy)? {
                        all = false;
                        break;
                    }
                }
                all
            }
        }
        (Value::Dict(d), Value::List(l)) | (Value::List(l), Value::Dict(d)) => {
            d.len() == l.items.len() && seq_eq(pairs(d), l.items.iter().cloned())?
        }
        (Value::Struct(x), Value::Struct(y)) => x.ty == y.ty && fields_eq(&x.fields, &y.fields)?,
        (Value::Variant(x), Value::Variant(y)) => {
            x.enum_ty == y.enum_ty && x.tag == y.tag && fields_eq(&x.fields, &y.fields)?
        }
        (Value::Inst(x), Value::Inst(y)) => x == y,
        (Value::Range(x), Value::Range(y)) => x == y,
        (Value::Path(x), Value::Path(y)) => x == y,
        (Value::Url(x), Value::Url(y)) => x == y,
        (Value::Sound(x), Value::Sound(y)) => x == y,
        (Value::UGen(x), Value::UGen(y)) => Rc::ptr_eq(x, y),
        _ => false,
    })
}

/// Elementwise equality; the caller has already compared the lengths.
fn seq_eq(
    xs: impl Iterator<Item = Value>,
    ys: impl Iterator<Item = Value>,
) -> Result<bool, Failure> {
    for (x, y) in xs.zip(ys) {
        if !deep_eq(&x, &y)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn fields_eq(xs: &[(KwId, Value)], ys: &[(KwId, Value)]) -> Result<bool, Failure> {
    if xs.len() != ys.len() {
        return Ok(false);
    }
    for ((kx, vx), (ky, vy)) in xs.iter().zip(ys) {
        if kx != ky || !deep_eq(vx, vy)? {
            return Ok(false);
        }
    }
    Ok(true)
}
