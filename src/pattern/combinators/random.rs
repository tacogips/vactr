//! Randomness through the pure hash RNG: degrade-by, maybe, sometimes-by
//! (and sometimes/rarely/often), choose.

use std::rc::Rc;

use crate::pattern::combinators::{event_fault, op, subtree};
use crate::pattern::eval::{apply_transform, eval_param_f64, QState};
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{q, Event, TimeSpan};
use crate::pattern::rng::{below, unit_at};
use crate::reader::span::Span;
use crate::value::value::Value;

/// `degrade-by p x`: drops each event with probability x.
#[must_use]
pub fn degrade_by(p: Rc<Pat>, x: PParam, span: Option<Span>) -> Pat {
    op(PatNode::DegradeBy(p, x), span)
}

/// `maybe x [prob]`: keeps each event with probability `prob` (default 0.5).
#[must_use]
pub fn maybe(p: Rc<Pat>, prob: Option<PParam>, span: Option<Span>) -> Pat {
    let prob = prob.unwrap_or(PParam::Const(Value::Float64(0.5)));
    op(PatNode::Maybe(p, prob), span)
}

/// `sometimes-by p x f`: each event comes from `f p` with probability x.
#[must_use]
pub fn sometimes_by(p: Rc<Pat>, x: PParam, f: Value, span: Option<Span>) -> Pat {
    op(PatNode::SometimesBy(x, f, p), span)
}

/// `sometimes p f` = `sometimes-by p 0.5 f`.
#[must_use]
pub fn sometimes(p: Rc<Pat>, f: Value, span: Option<Span>) -> Pat {
    sometimes_by(p, PParam::Const(Value::Float64(0.5)), f, span)
}

/// `rarely p f` = `sometimes-by p 0.25 f`.
#[must_use]
pub fn rarely(p: Rc<Pat>, f: Value, span: Option<Span>) -> Pat {
    sometimes_by(p, PParam::Const(Value::Float64(0.25)), f, span)
}

/// `often p f` = `sometimes-by p 0.75 f`.
#[must_use]
pub fn often(p: Rc<Pat>, f: Value, span: Option<Span>) -> Pat {
    sometimes_by(p, PParam::Const(Value::Float64(0.75)), f, span)
}

/// `choose a b ..`: one child per cycle.
#[must_use]
pub fn choose(items: Vec<Pat>, span: Option<Span>) -> Pat {
    op(PatNode::Choose(items.into_boxed_slice()), span)
}

/// Keeps the events whose draw passes: `keep_below` keeps `r < x`
/// (`maybe`), otherwise `r >= x` (`degrade-by`). The draw is keyed by the
/// node id and the event's anchor, so it is the same in every window.
fn filter(
    events: Vec<Event>,
    x: &PParam,
    keep_below: bool,
    p: &Pat,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::with_capacity(events.len());
    for e in events {
        let at = e.anchor();
        match eval_param_f64(x, at, st) {
            Ok(prob) => {
                let r = unit_at(st.cx.seed, p.id, at);
                if (r < prob) == keep_below {
                    out.push(e);
                }
            }
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_degrade(
    inner: &Pat,
    x: &PParam,
    keep_below: bool,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let events = q(inner, span, st);
    filter(events, x, keep_below, p, st)
}

pub(crate) fn query_sometimes(
    x: &PParam,
    f: &Value,
    inner: &Rc<Pat>,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let events = q(inner, span, st);
    let mut out = filter(events, x, false, p, st);
    out.extend(subtree(p, span.begin, st, |st| {
        let t = apply_transform(f, inner, st)?;
        let events = q(&t, span, st);
        Ok(filter(events, x, true, p, st))
    }));
    out
}

pub(crate) fn query_choose(
    items: &[Pat],
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let n = items.len() as u64;
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        let i = below(st.cx.seed, p.id, c, 0, n);
        let Some(item) = usize::try_from(i).ok().and_then(|i| items.get(i)) else {
            continue;
        };
        for mut e in q(item, piece, st) {
            e.occ.push(p.id, u32::try_from(i).unwrap_or(u32::MAX));
            out.push(e);
        }
    }
    out
}
