//! Time operators: fast, slow, hurry, rev, every, whenmod, iter, chunk,
//! segment.

use std::rc::Rc;

use crate::pattern::combinators::{kw, map_times, op, subtree, MAX_COUNT};
use crate::pattern::eval::{apply_transform, eval_param_int, eval_param_ratio, num_ratio, QState};
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{q, sect, Event, TimeSpan};
use crate::reader::span::Span;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// `fast p k`.
#[must_use]
pub fn fast(p: Rc<Pat>, k: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Fast(p, k), span)
}

/// `slow p k`.
#[must_use]
pub fn slow(p: Rc<Pat>, k: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Slow(p, k), span)
}

/// `hurry p k`: fast, and `speed` multiplied by k.
#[must_use]
pub fn hurry(p: Rc<Pat>, k: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Hurry(p, k), span)
}

/// `rev p`: each cycle reversed.
#[must_use]
pub fn rev(p: Rc<Pat>, span: Option<Span>) -> Pat {
    op(PatNode::Rev(p), span)
}

/// `every p n f`: `f` applied when `cycle mod n == 0`.
#[must_use]
pub fn every(p: Rc<Pat>, n: PParam, f: Value, span: Option<Span>) -> Pat {
    op(PatNode::Every(n, f, p), span)
}

/// `whenmod p a b f`: `f` applied when `cycle mod a >= b`.
#[must_use]
pub fn whenmod(p: Rc<Pat>, a: PParam, b: PParam, f: Value, span: Option<Span>) -> Pat {
    op(PatNode::WhenMod(a, b, f, p), span)
}

/// `iter p n`: cycle c starts `(c mod n) / n` later into the pattern.
#[must_use]
pub fn iter(p: Rc<Pat>, n: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Iter(p, n), span)
}

/// `chunk p n f`: `f` applied to the `(c mod n)`-th of n parts of cycle c.
#[must_use]
pub fn chunk(p: Rc<Pat>, n: PParam, f: Value, span: Option<Span>) -> Pat {
    op(PatNode::Chunk(p, n, f), span)
}

/// `segment p n`: n samples of p per cycle, each at its step start.
#[must_use]
pub fn segment(p: Rc<Pat>, n: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Segment(p, n), span)
}

/// The events of `inner` over `span` sped up by `factor` (none for a
/// factor <= 0).
fn fast_by(
    inner: &Pat,
    factor: Ratio64,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Result<Vec<Event>, Failure> {
    if factor <= Ratio64::ZERO {
        return Ok(Vec::new());
    }
    let inner_span = span.map(|t| t.checked_mul(factor))?;
    let events = q(inner, inner_span, st);
    map_times(events, |t| t.checked_div(factor))
}

/// Runs `f` once over the whole span when `param` is constant, else once
/// per cycle piece with the parameter read at the cycle start.
fn per_cycle(
    param: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
    mut f: impl FnMut(TimeSpan, Ratio64, &mut QState<'_, '_>) -> Result<Vec<Event>, Failure>,
) -> Vec<Event> {
    if param.is_const() {
        return subtree(p, span.begin, st, |st| f(span, span.begin, st));
    }
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let at = Ratio64::from_int(piece.begin.floor());
        out.extend(subtree(p, piece.begin, st, |st| f(piece, at, st)));
    }
    out
}

pub(crate) fn query_fast(
    inner: &Pat,
    k: &PParam,
    slow: bool,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    per_cycle(k, p, span, st, |piece, at, st| {
        let mut factor = eval_param_ratio(k, at, st)?;
        if slow {
            if factor <= Ratio64::ZERO {
                return Ok(Vec::new());
            }
            factor = Ratio64::ONE.checked_div(factor)?;
        }
        fast_by(inner, factor, piece, st)
    })
}

pub(crate) fn query_hurry(
    inner: &Pat,
    k: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let speed = kw("speed");
    per_cycle(k, p, span, st, |piece, at, st| {
        let factor = eval_param_ratio(k, at, st)?;
        let mut events = fast_by(inner, factor, piece, st)?;
        for e in &mut events {
            let old = e
                .controls
                .get(&speed)
                .and_then(num_ratio)
                .unwrap_or(Ratio64::ONE);
            e.controls.insert(
                speed,
                crate::pattern::eval::exact_value(old.checked_mul(factor)?),
            );
        }
        Ok(events)
    })
}

pub(crate) fn query_rev(
    inner: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        out.extend(subtree(p, piece.begin, st, |st| {
            rev_piece(inner, piece, st)
        }));
    }
    out
}

fn rev_piece(inner: &Pat, piece: TimeSpan, st: &mut QState<'_, '_>) -> Result<Vec<Event>, Failure> {
    let c = piece.begin.floor();
    let mirror = Ratio64::from_int(c)
        .checked_mul(Ratio64::from_int(2))?
        .checked_add(Ratio64::ONE)?;
    let reflect = |s: TimeSpan| -> Result<TimeSpan, Failure> {
        Ok(TimeSpan {
            begin: mirror.checked_sub(s.end)?,
            end: mirror.checked_sub(s.begin)?,
        })
    };
    // A point is reflected against whole spans, which are end-exclusive:
    // query the whole cycle and keep the events whose reflection holds it.
    let inner_span = if piece.is_point() {
        TimeSpan::cycle(c)?
    } else {
        reflect(piece)?
    };
    let mut out = Vec::new();
    for mut e in q(inner, inner_span, st) {
        e.whole = match e.whole {
            Some(w) => Some(reflect(w)?),
            None => None,
        };
        e.part = reflect(e.part)?;
        if piece.is_point() {
            let Some(part) = sect(e.whole.unwrap_or(e.part), piece) else {
                continue;
            };
            e.part = part;
        }
        out.push(e);
    }
    Ok(out)
}

/// Cycle-wise choice between `inner` and `f inner`.
fn choose_transform(
    inner: &Rc<Pat>,
    f: &Value,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
    mut apply: impl FnMut(i64, &mut QState<'_, '_>) -> Result<bool, Failure>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        out.extend(subtree(p, piece.begin, st, |st| {
            if apply(c, st)? {
                let t = apply_transform(f, inner, st)?;
                Ok(q(&t, piece, st))
            } else {
                Ok(q(inner, piece, st))
            }
        }));
    }
    out
}

pub(crate) fn query_every(
    n: &PParam,
    f: &Value,
    inner: &Rc<Pat>,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    choose_transform(inner, f, p, span, st, |c, st| {
        let n = eval_param_int(n, Ratio64::from_int(c), st)?;
        Ok(n > 0 && c.rem_euclid(n) == 0)
    })
}

pub(crate) fn query_whenmod(
    a: &PParam,
    b: &PParam,
    f: &Value,
    inner: &Rc<Pat>,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    choose_transform(inner, f, p, span, st, |c, st| {
        let at = Ratio64::from_int(c);
        let a = eval_param_int(a, at, st)?;
        let b = eval_param_int(b, at, st)?;
        Ok(a > 0 && c.rem_euclid(a) >= b)
    })
}

pub(crate) fn query_iter(
    inner: &Pat,
    n: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        out.extend(subtree(p, piece.begin, st, |st| {
            let n = eval_param_int(n, Ratio64::from_int(c), st)?;
            if n <= 0 {
                return Ok(q(inner, piece, st));
            }
            let shift = Ratio64::new(c.rem_euclid(n), n)?;
            let events = q(inner, piece.map(|t| t.checked_add(shift))?, st);
            map_times(events, |t| t.checked_sub(shift))
        }));
    }
    out
}

pub(crate) fn query_chunk(
    inner: &Rc<Pat>,
    n: &PParam,
    f: &Value,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        out.extend(subtree(p, piece.begin, st, |st| {
            let n = eval_param_int(n, Ratio64::from_int(c), st)?;
            if n <= 0 {
                return Ok(q(inner, piece, st));
            }
            let i = c.rem_euclid(n);
            let lo = Ratio64::new(i, n)?;
            let hi = Ratio64::new(i + 1, n)?;
            let inside = |e: &Event| {
                let pos = e.anchor().frac();
                lo <= pos && pos < hi
            };
            let t = apply_transform(f, inner, st)?;
            let mut events: Vec<Event> = q(&t, piece, st).into_iter().filter(&inside).collect();
            events.extend(q(inner, piece, st).into_iter().filter(|e| !inside(e)));
            Ok(events)
        }));
    }
    out
}

pub(crate) fn query_segment(
    inner: &Pat,
    n: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        let n = match eval_param_int(n, Ratio64::from_int(c), st) {
            Ok(n) => n.min(MAX_COUNT),
            Err(f) => {
                st.fault(f, p.span, Some(piece.begin));
                continue;
            }
        };
        for i in 0..n.max(0) {
            let whole = match step_span(c, n, i) {
                Ok(w) => w,
                Err(f) => {
                    st.fault(f, p.span, Some(piece.begin));
                    break;
                }
            };
            let Some(part) = sect(whole, piece) else {
                continue;
            };
            if let Err(f) = st.spend(1, p.span) {
                st.fault(f, p.span, Some(whole.begin));
                return out;
            }
            match st.sample(inner, whole.begin) {
                Ok(sampled) => {
                    if let Some(s) = sampled.into_iter().next() {
                        let mut e = Event::new(Some(whole), part, s.value, s.src);
                        e.controls = s.controls;
                        out.push(e);
                    }
                }
                Err(f) => st.fault(f, p.span, Some(whole.begin)),
            }
        }
    }
    out
}

/// Step `i` of `n` in cycle `c`.
pub(crate) fn step_span(c: i64, n: i64, i: i64) -> Result<TimeSpan, Failure> {
    let base = Ratio64::from_int(c);
    Ok(TimeSpan {
        begin: base.checked_add(Ratio64::new(i, n)?)?,
        end: base.checked_add(Ratio64::new(i + 1, n)?)?,
    })
}
