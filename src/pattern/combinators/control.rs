//! Controls and the first-structure rule (design 10.1 "Sound first").
//!
//! A control on an UNSTRUCTURED subject whose value pattern is structured
//! (a list, `alt`, ...) gives the structure: each value event takes the
//! subject's content sampled at its onset. Otherwise the subject keeps its
//! structure and the value is sampled at each subject event's
//! `whole.begin` (`part.begin` without a whole), Tidal's `#`.

use std::rc::Rc;

use crate::pattern::combinators::event_fault;
use crate::pattern::eval::QState;
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::query::{q, Event, TimeSpan};
use crate::reader::span::Span;
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// Whether a control-like node over `subject` with `value` gives structure.
#[must_use]
pub fn gives_structure(subject: &Pat, value: &Pat) -> bool {
    !subject.structured && value.structured
}

/// `name value` applied to `subject` (`gain`, `lpf`, `n`, `note`, ...).
#[must_use]
pub fn control(name: KwId, value: Rc<Pat>, subject: Rc<Pat>, span: Option<Span>) -> Pat {
    let structured = subject.structured || value.structured;
    Pat::new(PatNode::Control(name, value, subject), span, structured)
}

/// Events of a control-like node: `(result event, value)` pairs where the
/// result carries the structure and the subject's content, and `value` is
/// the control value found for it.
pub(crate) fn pair_up(
    value: &Pat,
    subject: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<(Event, Value)> {
    let mut out = Vec::new();
    if gives_structure(subject, value) {
        for v in q(value, span, st) {
            match st.sample(subject, v.anchor()) {
                Ok(sampled) => {
                    for s in sampled {
                        let mut e = s;
                        let mut path = v.occ.path.clone();
                        path.extend(e.occ.path.iter().copied());
                        e.occ.path = path;
                        e.whole = v.whole;
                        e.part = v.part;
                        e.src = v.src.or(e.src);
                        out.push((e, v.value.clone()));
                    }
                }
                Err(f) => event_fault(st, &v, p, f),
            }
        }
    } else {
        for e in q(subject, span, st) {
            match st.sample(value, e.anchor()) {
                // No value at the onset (a rest): the event is dropped, as
                // with Tidal's `#`.
                Ok(sampled) => {
                    if let Some(v) = sampled.into_iter().next() {
                        out.push((e, v.value));
                    }
                }
                Err(f) => event_fault(st, &e, p, f),
            }
        }
    }
    out
}

/// A control-like query whose value goes through `map` before it is set.
pub(crate) fn query_mapped(
    name: KwId,
    value: &Pat,
    subject: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
    map: impl Fn(&Value) -> Result<Value, Failure>,
) -> Vec<Event> {
    let pairs = pair_up(value, subject, p, span, st);
    let mut out = Vec::with_capacity(pairs.len());
    for (mut e, v) in pairs {
        match map(&v) {
            Ok(v) => {
                e.controls.insert(name, v);
                out.push(e);
            }
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_control(
    name: KwId,
    value: &Pat,
    subject: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    query_mapped(name, value, subject, p, span, st, |v| Ok(v.clone()))
}
