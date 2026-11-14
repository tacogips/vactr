//! Pattern combinators, split by group (design 10.1, design-music
//! section 3). Each file holds the builders and the query of its nodes.

pub mod control;
pub mod input;
pub mod music;
pub mod random;
pub mod region;
pub mod sound;
pub mod structure;
pub mod time;

use std::rc::Rc;

use crate::pattern::eval::QState;
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::query::{sect, Event, TimeSpan};
use crate::reader::span::Span;
use crate::value::intern::{intern_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// Upper bound for per-event counts (`chop`, `ply`, `euclid` steps, ...).
pub(crate) const MAX_COUNT: i64 = 4096;

/// A structured operator node (10.1: any operator other than a control
/// realizes an unstructured subject, so its result is structured).
#[must_use]
pub fn op(node: PatNode, span: Option<Span>) -> Pat {
    Pat::new(node, span, true)
}

/// A named control key.
pub(crate) fn kw(name: &str) -> KwId {
    intern_kw(name)
}

/// Checks a per-event count.
pub(crate) fn count(n: i64, what: &str) -> Result<i64, Failure> {
    if (1..=MAX_COUNT).contains(&n) {
        Ok(n)
    } else {
        Err(Failure::new(
            FailCode::Type,
            format!("{what} must be between 1 and {MAX_COUNT}"),
        ))
    }
}

/// Runs a subtree query; a failure is recorded subtree-locally (10.3) and
/// the subtree contributes no events.
pub(crate) fn subtree(
    p: &Pat,
    at: Ratio64,
    st: &mut QState<'_, '_>,
    f: impl FnOnce(&mut QState<'_, '_>) -> Result<Vec<Event>, Failure>,
) -> Vec<Event> {
    match f(st) {
        Ok(v) => v,
        Err(e) => {
            st.fault(e, p.span, Some(at));
            Vec::new()
        }
    }
}

/// Records an event-local failure with the event's origin.
pub(crate) fn event_fault(st: &mut QState<'_, '_>, e: &Event, p: &Pat, f: Failure) {
    let span = e.src.map(|s| s.span).or(p.span);
    st.fault(f, span, Some(e.anchor()));
}

/// Splits `whole` into `k` equal parts and returns part `i`.
pub(crate) fn sub_whole(whole: TimeSpan, k: i64, i: i64) -> Result<TimeSpan, Failure> {
    let d = whole.duration()?;
    let kk = Ratio64::from_int(k);
    let at = |j: i64| -> Result<Ratio64, Failure> {
        whole
            .begin
            .checked_add(d.checked_mul(Ratio64::from_int(j))?.checked_div(kk)?)
    };
    Ok(TimeSpan {
        begin: at(i)?,
        end: at(i + 1)?,
    })
}

/// The children of `e` when its whole is split into `k` parts: child
/// wholes, parts clipped to the source part, empty clips dropped.
pub(crate) fn split_event(e: &Event, k: i64) -> Result<Vec<(i64, Event)>, Failure> {
    let Some(whole) = e.whole else {
        return Err(no_whole());
    };
    let mut out = Vec::new();
    for i in 0..k {
        let w = sub_whole(whole, k, i)?;
        if let Some(part) = sect(w, e.part) {
            let mut child = e.clone();
            child.whole = Some(w);
            child.part = part;
            out.push((i, child));
        }
    }
    Ok(out)
}

/// The `no-whole` failure of a subdividing operator.
pub(crate) fn no_whole() -> Failure {
    Failure::new(
        FailCode::NoWhole,
        "a continuous event has no whole span to subdivide",
    )
}

/// Maps both spans of every event through `f`.
pub(crate) fn map_times(
    events: Vec<Event>,
    f: impl Fn(Ratio64) -> Result<Ratio64, Failure>,
) -> Result<Vec<Event>, Failure> {
    events
        .into_iter()
        .map(|mut e| {
            e.whole = match e.whole {
                Some(w) => Some(w.map(&f)?),
                None => None,
            };
            e.part = e.part.map(&f)?;
            Ok(e)
        })
        .collect()
}

/// A `Value::Pattern` of a node.
#[must_use]
pub fn pattern_value(p: Pat) -> Value {
    Value::Pattern(Rc::new(p))
}
