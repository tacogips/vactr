//! Coercions from values to patterns and parameters, used by the native
//! wrappers (INTEGRATE). A plain list becomes `Steps` only where a pattern
//! is expected; the list itself is never changed (10.1).

use std::rc::Rc;

use crate::pattern::combinators::structure::cat;
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::signal::Sig;
use crate::pattern::step::{steps, steps_of_list, Step};
use crate::reader::span::Span;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// Longest eager range turned into steps.
const MAX_RANGE_STEPS: i64 = 4096;

/// One value for every cycle, UNSTRUCTURED (a scalar control value, one
/// sound, a late ref or a function of time).
#[must_use]
pub fn pure(v: Value, span: Option<Span>) -> Pat {
    Pat::new(PatNode::Pure(Step::bare(v)), span, false)
}

/// A signal as a pattern (unstructured, continuous).
#[must_use]
pub fn signal(sig: Sig, span: Option<Span>) -> Pat {
    Pat::new(PatNode::Signal(Rc::new(sig)), span, false)
}

/// A step list of the given values.
#[must_use]
pub fn value_steps(values: &[Value]) -> Pat {
    steps(values.iter().cloned().map(Step::bare).collect(), None)
}

/// The pattern for a value in a pattern position.
///
/// # Errors
/// `Type` for an open or over-long range.
pub fn pattern_of(v: &Value, span: Option<Span>) -> Result<Rc<Pat>, Failure> {
    Ok(match v {
        Value::Pattern(p) => Rc::clone(p),
        Value::List(l) => Rc::new(steps(steps_of_list(l), span)),
        Value::Signal(s) => Rc::new(Pat::new(PatNode::Signal(Rc::clone(s)), span, false)),
        Value::Range(r) => {
            let end = r
                .end
                .ok_or_else(|| Failure::new(FailCode::Type, "an open range is not a pattern"))?;
            if end.saturating_sub(r.start) > MAX_RANGE_STEPS {
                return Err(Failure::new(FailCode::Type, "range too long for a pattern"));
            }
            let items: Vec<Value> = (r.start..end)
                .map(|i| match i32::try_from(i) {
                    Ok(i) => Value::Int(i),
                    Err(_) => Value::Int64(i),
                })
                .collect();
            Rc::new(value_steps(&items).with_span(span))
        }
        other => Rc::new(pure(other.clone(), span)),
    })
}

/// The parameter for a value in a numeric-parameter position: a var ref is
/// `Late`, a function is `Fn`, a pattern-like value is `Pat`.
///
/// # Errors
/// As [`pattern_of`].
pub fn param_of(v: &Value) -> Result<PParam, Failure> {
    Ok(match v {
        Value::VarRef(r) => PParam::Late(r.clone()),
        Value::Fn(_) | Value::Native(_) | Value::Thunk(_) => PParam::Fn(v.clone()),
        Value::Pattern(_) | Value::List(_) | Value::Signal(_) | Value::Range(_) => {
            PParam::Pat(pattern_of(v, None)?)
        }
        other => PParam::Const(other.clone()),
    })
}

/// `alt a b ..`: one per cycle, in turn (`<a b>`).
///
/// # Errors
/// As [`pattern_of`].
pub fn alt(items: &[Value], span: Option<Span>) -> Result<Pat, Failure> {
    let pats = items
        .iter()
        .map(|v| pattern_of(v, None).map(|p| (*p).clone()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cat(pats, span))
}

/// A MIDI channel `1..16`.
///
/// # Errors
/// `Type` outside the range.
pub fn midi_channel(ch: i64) -> Result<u8, Failure> {
    u8::try_from(ch)
        .ok()
        .filter(|c| (1..=16).contains(c))
        .ok_or_else(|| Failure::new(FailCode::Type, "a MIDI channel is 1..16"))
}
