//! Controls and the first-structure rule (design 10.1 "Sound first").
//!
//! A control on an UNSTRUCTURED subject whose value pattern is structured
//! (a list, `alt`, ...) gives the structure: each value event takes the
//! subject's content sampled at its onset. Otherwise the subject keeps its
//! structure and the value is sampled at each subject event's
//! `whole.begin` (`part.begin` without a whole), Tidal's `#`.

use std::rc::Rc;

use crate::ns::namespace::VarSlotRef;
use crate::pattern::combinators::event_fault;
use crate::pattern::eval::QState;
use crate::pattern::occ::{ProducerKind, ProducerTrace};
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::query::{q, q_child, Event, TimeSpan};
use crate::reader::span::Span;
use crate::value::eq::deep_eq;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
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

/// Events of a control-like node: `(result event, value, late)` triples
/// where the result carries the structure and the subject's content,
/// `value` is the control value found for it and `late` the var or tweak
/// that value was read from directly (11.3), if any.
pub(crate) fn pair_up(
    value: &Pat,
    subject: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<(Event, Value, Option<VarSlotRef>)> {
    let mut out = Vec::new();
    if gives_structure(subject, value) {
        for v in query_child(value, span, 0, st) {
            match st.with_structural_sample(
                subject,
                v.anchor(),
                1,
                v.whole,
                v.part,
                Some(&v),
                |st| sample_child(subject, v.anchor(), 1, st),
            ) {
                Ok(sampled) => {
                    for s in sampled {
                        let mut e = s;
                        let mut path = v.occ.path.clone();
                        path.extend(e.occ.path.iter().copied());
                        e.occ.path = path;
                        merge_producers(&v, &mut e);
                        e.whole = v.whole;
                        e.part = v.part;
                        e.src = v.src.or(e.src);
                        out.push((e, v.value.clone(), v.late.clone()));
                    }
                }
                Err(f) => event_fault(st, &v, p, f),
            }
        }
    } else {
        for e in query_child(subject, span, 1, st) {
            match st.with_structural_sample(value, e.anchor(), 0, e.whole, e.part, Some(&e), |st| {
                sample_child(value, e.anchor(), 0, st)
            }) {
                // No value at the onset (a rest): the event is dropped, as
                // with Tidal's `#`.
                Ok(sampled) => {
                    if let Some(v) = sampled.into_iter().next() {
                        out.push((e, v.value, v.late));
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
    for (mut e, v, late) in pairs {
        match map(&v) {
            Ok(mapped) => {
                // Only an unmapped late value can travel as a cell: a mapped
                // one (a chord's tones) no longer equals its source.
                match late {
                    Some(r) if deep_eq(&mapped, &v).unwrap_or(false) => {
                        e.cells.insert(name, r);
                    }
                    _ => {
                        e.cells.remove(&name);
                    }
                }
                e.controls.insert(name, mapped);
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

/// Samples a statically numbered child while retaining the shared query bounds.
pub(crate) fn sample_child(
    child: &Pat,
    at: Ratio64,
    ordinal: u32,
    st: &mut QState<'_, '_>,
) -> Result<Vec<Event>, Failure> {
    st.with_producer(ProducerKind::Child, ordinal, |st| st.sample(child, at))
}

/// Frames both complete paths by role and length. Nested merges cannot alias
/// a concatenation of leaves or a different timing/content split.
pub(crate) fn merge_producers(timing: &Event, content: &mut Event) {
    let (Some(timing), Some(subject)) = (&timing.producer, &content.producer) else {
        return;
    };
    let mut merged = ProducerTrace::default();
    for (kind, trace) in [
        (ProducerKind::TimingSource, timing),
        (ProducerKind::ContentSource, subject),
    ] {
        let length = trace.steps.len() as u64;
        merged.push(kind, length as u32);
        merged.push(kind, (length >> 32) as u32);
        merged.steps.extend_from_slice(&trace.steps);
    }
    content.producer = Some(merged);
}

/// A transformed child is a separate producer even when its content is equal.
pub(crate) fn transformed_child(
    child: &Pat,
    span: TimeSpan,
    ordinal: u32,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    st.with_producer(ProducerKind::DynamicExpansion, ordinal, |st| {
        query_child(child, span, ordinal, st)
    })
}

/// Avoid additional recursion frames on the unchanged legacy query path.
#[inline(always)]
pub(crate) fn query_child(
    child: &Pat,
    span: TimeSpan,
    ordinal: u32,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    if !st.is_traced() {
        q(child, span, st)
    } else {
        traced_child(child, span, ordinal, st)
    }
}

/// Keep trace-only scope temporaries outside legacy recursive caller frames.
#[inline(never)]
fn traced_child(child: &Pat, span: TimeSpan, ordinal: u32, st: &mut QState<'_, '_>) -> Vec<Event> {
    q_child(child, span, ordinal, st)
}
