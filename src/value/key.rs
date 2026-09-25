//! Dict keys with a total order (design 5.4, 6.5.3).
//!
//! Numbers < keywords < strings. Numbers order numerically across widths:
//! `1`, `1.0` and `1/1` are one key, and `-0.0` is `0`. Keywords order by
//! name through the thread-local interner.

use std::cmp::Ordering;
use std::rc::Rc;

use crate::value::intern::{name_of_kw, KwId};
use crate::value::ratio::{decompose_f64, Ratio64};
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// A normalized numeric key.
///
/// `Exact` holds every int, int64 and ratio, and every float whose value is
/// exactly a `Ratio64`. `Float` holds the other non-NaN floats, so an
/// `Exact` and a `Float` are never numerically equal.
#[derive(Clone, Copy, Debug)]
pub enum NumKey {
    Exact(Ratio64),
    Float(f64),
}

impl NumKey {
    /// The key of a float. `-0.0` becomes `0`.
    ///
    /// # Errors
    /// `Type` for NaN.
    pub fn from_f64(x: f64) -> Result<NumKey, Failure> {
        if x.is_nan() {
            return Err(Failure::new(FailCode::Type, "NaN is not a valid key"));
        }
        Ok(match Ratio64::from_f64_exact(x) {
            Some(r) => NumKey::Exact(r),
            None => NumKey::Float(x),
        })
    }

    /// The key of a numeric value; `None` for a non-number.
    ///
    /// # Errors
    /// `Type` for NaN.
    pub fn from_value(v: &Value) -> Result<Option<NumKey>, Failure> {
        Ok(Some(match v {
            Value::Int(i) => NumKey::Exact(Ratio64::from_int(i64::from(*i))),
            Value::Int64(i) => NumKey::Exact(Ratio64::from_int(*i)),
            Value::Ratio(r) => NumKey::Exact(*r),
            Value::Float(x) => NumKey::from_f64(f64::from(*x))?,
            Value::Float64(x) => NumKey::from_f64(*x)?,
            _ => return Ok(None),
        }))
    }

    /// The canonical value of the key: an int when integral and in `i32`
    /// range, else `int64`, a ratio, or a `float64`.
    #[must_use]
    pub fn to_value(self) -> Value {
        match self {
            NumKey::Exact(r) if r.is_integral() => match i32::try_from(r.num()) {
                Ok(i) => Value::Int(i),
                Err(_) => Value::Int64(r.num()),
            },
            NumKey::Exact(r) => Value::Ratio(r),
            NumKey::Float(x) => Value::Float64(x),
        }
    }
}

/// Exact numeric comparison of a ratio with a non-NaN float.
pub(crate) fn cmp_ratio_f64(r: Ratio64, x: f64) -> Ordering {
    let Some((m, e)) = decompose_f64(x) else {
        // Infinities (NaN never reaches here).
        return if x > 0.0 {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    };
    let n = i128::from(r.num());
    let d = i128::from(r.den());
    // n/d <=> m*2^e  is  n * 2^max(-e,0) <=> m * d * 2^max(e,0).
    let (lhs, rhs) = if e >= 0 {
        (Some(n), shl(m * d, e))
    } else {
        (shl(n, -e), Some(m * d))
    };
    match (lhs, rhs) {
        (Some(a), Some(b)) => a.cmp(&b),
        // The side that overflowed has the larger magnitude, so its sign decides.
        (None, _) => {
            if n > 0 {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        }
        (Some(_), None) => {
            if m > 0 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
    }
}

fn shl(v: i128, s: i32) -> Option<i128> {
    if v == 0 {
        return Some(0);
    }
    if s >= 127 {
        return None;
    }
    v.checked_mul(1i128 << s)
}

impl Ord for NumKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (NumKey::Exact(a), NumKey::Exact(b)) => a.cmp(b),
            (NumKey::Float(a), NumKey::Float(b)) => a.total_cmp(b),
            // Never Equal numerically (see the type doc); on a tie Exact sorts first.
            (NumKey::Exact(a), NumKey::Float(b)) => cmp_ratio_f64(*a, *b).then(Ordering::Less),
            (NumKey::Float(a), NumKey::Exact(b)) => {
                cmp_ratio_f64(*b, *a).reverse().then(Ordering::Greater)
            }
        }
    }
}

impl PartialOrd for NumKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for NumKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for NumKey {}

/// A dict key: a number, keyword or string.
#[derive(Clone, Debug)]
pub enum Key {
    Num(NumKey),
    Kw(KwId),
    Str(Rc<str>),
}

impl Key {
    /// The key for a value.
    ///
    /// # Errors
    /// `Type` for NaN or for a value that is not a number, keyword or string.
    pub fn from_value(v: &Value) -> Result<Key, Failure> {
        if let Some(n) = NumKey::from_value(v)? {
            return Ok(Key::Num(n));
        }
        match v {
            Value::Keyword(k) => Ok(Key::Kw(*k)),
            Value::Str(s) => Ok(Key::Str(Rc::clone(s))),
            _ => Err(Failure::new(
                FailCode::Type,
                "only numbers, keywords and strings are dict keys",
            )),
        }
    }

    /// The canonical value of the key.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Key::Num(n) => n.to_value(),
            Key::Kw(k) => Value::Keyword(*k),
            Key::Str(s) => Value::Str(Rc::clone(s)),
        }
    }

    const fn rank(&self) -> u8 {
        match self {
            Key::Num(_) => 0,
            Key::Kw(_) => 1,
            Key::Str(_) => 2,
        }
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Key::Num(a), Key::Num(b)) => a.cmp(b),
            (Key::Kw(a), Key::Kw(b)) => {
                if a == b {
                    Ordering::Equal
                } else {
                    name_of_kw(*a).cmp(&name_of_kw(*b)).then(a.cmp(b))
                }
            }
            (Key::Str(a), Key::Str(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Key {}
