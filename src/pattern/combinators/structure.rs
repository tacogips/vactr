//! Structure: stack, cat, fastcat, superimpose, off, jux, ply, repeat,
//! euclid, grid.

use crate::pattern::combinators::control::query_child;
use crate::pattern::eval::song_clock::CanonicalClockFrame;
use std::rc::Rc;

use crate::pattern::combinators::control::{merge_producers, sample_child, transformed_child};
use crate::pattern::combinators::time::step_span;
use crate::pattern::combinators::{
    count, event_fault, kw, map_times, op, split_event, subtree, MAX_COUNT,
};
use crate::pattern::eval::{apply_transform, eval_param_int, eval_param_ratio, QState};
use crate::pattern::occ::ProducerKind;
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{sect, Event, TimeSpan};
use crate::pattern::step::query_single;
use crate::reader::span::Span;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// `stack [a b ..]`: all at once.
#[must_use]
pub fn stack(items: Vec<Pat>, span: Option<Span>) -> Pat {
    op(PatNode::Stack(items.into_boxed_slice()), span)
}

/// `cat [a b ..]` (and `alt`): one child per cycle, in turn.
#[must_use]
pub fn cat(items: Vec<Pat>, span: Option<Span>) -> Pat {
    op(PatNode::Cat(items.into_boxed_slice()), span)
}

/// `fastcat [a b ..]`: all children within one cycle.
#[must_use]
pub fn fastcat(items: Vec<Pat>, span: Option<Span>) -> Pat {
    op(PatNode::FastCat(items.into_boxed_slice()), span)
}

/// `superimpose p f`: `p` stacked with `f p`.
#[must_use]
pub fn superimpose(p: Rc<Pat>, f: Value, span: Option<Span>) -> Pat {
    op(PatNode::Superimpose(p, f), span)
}

/// `off p t f`: `p` stacked with `f p` shifted `t` cycles later.
#[must_use]
pub fn off(p: Rc<Pat>, t: PParam, f: Value, span: Option<Span>) -> Pat {
    op(PatNode::Off(p, t, f), span)
}

/// `jux p f`: `p` panned left stacked with `f p` panned right.
#[must_use]
pub fn jux(p: Rc<Pat>, f: Value, span: Option<Span>) -> Pat {
    op(PatNode::Jux(p, f), span)
}

/// `ply p n`: each event repeated n times inside its whole.
#[must_use]
pub fn ply(p: Rc<Pat>, n: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Ply(p, n), span)
}

/// `hold x w`: a step weight inside a step list.
#[must_use]
pub fn hold(p: Rc<Pat>, w: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Hold(p, w), span)
}

/// `repeat x n`: a step replicated n times.
#[must_use]
pub fn repeat(p: Rc<Pat>, n: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Repeat(p, n), span)
}

/// `euclid p pulses steps` with `rotation:` (default 0).
#[must_use]
pub fn euclid(p: Rc<Pat>, k: PParam, n: PParam, rot: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Euclid(p, k, n, rot), span)
}

/// `grid p bools` (Tidal's `struct`).
#[must_use]
pub fn grid(p: Rc<Pat>, bools: Rc<Pat>, span: Option<Span>) -> Pat {
    op(PatNode::Grid(p, bools), span)
}

fn tag(mut events: Vec<Event>, p: &Pat, i: usize) -> Vec<Event> {
    let ord = u32::try_from(i).unwrap_or(u32::MAX);
    for e in &mut events {
        e.occ.push(p.id, ord);
    }
    events
}

pub(crate) fn query_stack(
    items: &[Pat],
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        out.extend(tag(
            query_child(item, span, u32::try_from(i).unwrap_or(u32::MAX), st),
            p,
            i,
        ));
    }
    out
}

/// Child `c mod n` plays its cycle `c div n` in cycle `c`.
fn cat_piece(
    items: &[Pat],
    p: &Pat,
    piece: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Result<Vec<Event>, Failure> {
    let n = i64::try_from(items.len()).unwrap_or(i64::MAX);
    if n == 0 {
        return Ok(Vec::new());
    }
    let c = piece.begin.floor();
    let i = c.rem_euclid(n);
    let shift = Ratio64::from_int(c - c.div_euclid(n));
    let Some(item) = usize::try_from(i).ok().and_then(|i| items.get(i)) else {
        return Ok(Vec::new());
    };
    let child = piece.map(|t| t.checked_sub(shift))?;
    let ordinal = u32::try_from(i).unwrap_or(u32::MAX);
    let events = if st.has_clock_observation() {
        st.with_clock_frame(
            || CanonicalClockFrame::iter(Ratio64::ZERO.checked_sub(shift)?, child, piece),
            |st| Ok(query_child(item, child, ordinal, st)),
        )?
    } else {
        query_child(item, child, ordinal, st)
    };
    let events = map_times(events, |t| t.checked_add(shift))?;
    Ok(tag(events, p, usize::try_from(i).unwrap_or(0)))
}

pub(crate) fn query_cat(
    items: &[Pat],
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        out.extend(subtree(p, piece.begin, st, |st| {
            cat_piece(items, p, piece, st)
        }));
    }
    out
}

pub(crate) fn query_fastcat(
    items: &[Pat],
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let n = Ratio64::from_int(i64::try_from(items.len()).unwrap_or(i64::MAX));
    if items.is_empty() {
        return Vec::new();
    }
    subtree(p, span.begin, st, |st| {
        let inner = span.map(|t| t.checked_mul(n))?;
        let mut out = Vec::new();
        let events = if st.has_clock_observation() {
            st.with_clock_frame(
                || CanonicalClockFrame::fast(n, inner, span),
                |st| {
                    let mut events = Vec::new();
                    for piece in st.cycles(inner, p.span) {
                        events.extend(cat_piece(items, p, piece, st)?);
                    }
                    Ok(events)
                },
            )?
        } else {
            let mut events = Vec::new();
            for piece in st.cycles(inner, p.span) {
                events.extend(cat_piece(items, p, piece, st)?);
            }
            events
        };
        out.extend(events);
        map_times(out, |t| t.checked_div(n))
    })
}

pub(crate) fn query_superimpose(
    inner: &Rc<Pat>,
    f: &Value,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = tag(query_child(inner, span, 0, st), p, 0);
    out.extend(subtree(p, span.begin, st, |st| {
        let t = apply_transform(f, inner, st)?;
        Ok(tag(transformed_child(&t, span, 1, st), p, 1))
    }));
    out
}

pub(crate) fn query_off(
    inner: &Rc<Pat>,
    t: &PParam,
    f: &Value,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = tag(query_child(inner, span, 0, st), p, 0);
    out.extend(subtree(p, span.begin, st, |st| {
        let shift = eval_param_ratio(t, Ratio64::from_int(span.begin.floor()), st)?;
        let transformed = apply_transform(f, inner, st)?;
        let child = span.map(|x| x.checked_sub(shift))?;
        let events = if st.has_clock_observation() {
            st.with_clock_frame(
                || CanonicalClockFrame::iter(Ratio64::ZERO.checked_sub(shift)?, child, span),
                |st| Ok(transformed_child(&transformed, child, 1, st)),
            )?
        } else {
            transformed_child(&transformed, child, 1, st)
        };
        Ok(tag(map_times(events, |x| x.checked_add(shift))?, p, 1))
    }));
    out
}

pub(crate) fn query_jux(
    inner: &Rc<Pat>,
    f: &Value,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let pan = kw("pan");
    let mut out = tag(query_child(inner, span, 0, st), p, 0);
    for e in &mut out {
        e.controls.insert(pan, Value::Float64(0.0));
    }
    out.extend(subtree(p, span.begin, st, |st| {
        let t = apply_transform(f, inner, st)?;
        let mut right = tag(transformed_child(&t, span, 1, st), p, 1);
        for e in &mut right {
            e.controls.insert(pan, Value::Float64(1.0));
        }
        Ok(right)
    }));
    out
}

pub(crate) fn query_ply(
    inner: &Pat,
    n: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for e in query_child(inner, span, 0, st) {
        let r = eval_param_int(n, e.anchor(), st)
            .and_then(|k| count(k, "ply count"))
            .and_then(|k| split_event(&e, k));
        match r {
            Ok(children) => {
                for (i, mut child) in children {
                    child.occ.push(p.id, u32::try_from(i).unwrap_or(u32::MAX));
                    if let Some(trace) = &mut child.producer {
                        trace.push(
                            ProducerKind::GeneratedBranch,
                            u32::try_from(i).unwrap_or(u32::MAX),
                        );
                    }
                    out.push(child);
                }
            }
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_repeat(
    inner: &Pat,
    n: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        match eval_param_int(n, Ratio64::from_int(piece.begin.floor()), st) {
            Ok(k) => out.extend(query_single(inner, k.clamp(0, MAX_COUNT), p, piece, st)),
            Err(f) => st.fault(f, p.span, Some(piece.begin)),
        }
    }
    out
}

/// Bjorklund's algorithm (as in Tidal): `k` pulses over `n` steps.
#[must_use]
pub fn bjorklund(k: usize, n: usize) -> Vec<bool> {
    let k = k.min(n);
    let mut a: Vec<Vec<bool>> = vec![vec![true]; k];
    let mut b: Vec<Vec<bool>> = vec![vec![false]; n - k];
    loop {
        let (i, j) = (a.len(), b.len());
        if i.min(j) <= 1 {
            break;
        }
        if i > j {
            // left: pair the first j of a with b; the rest of a becomes b.
            let rest = a.split_off(j);
            for (x, y) in a.iter_mut().zip(b.iter()) {
                x.extend_from_slice(y);
            }
            b = rest;
        } else {
            // right: pair a with the first i of b; the rest of b stays.
            let rest = b.split_off(i);
            for (x, y) in a.iter_mut().zip(b.iter()) {
                x.extend_from_slice(y);
            }
            b = rest;
        }
    }
    a.into_iter().chain(b).flatten().collect()
}

/// The onset steps of `euclid k n` rotated left by `rot`.
pub(crate) fn euclid_steps(k: i64, n: i64, rot: i64) -> Result<Vec<bool>, Failure> {
    let n = count(n, "euclid steps")?;
    let invert = k < 0;
    let pulses = usize::try_from(k.unsigned_abs().min(n.unsigned_abs())).unwrap_or(0);
    let len = usize::try_from(n).unwrap_or(0);
    let base = bjorklund(pulses, len);
    let r = usize::try_from(rot.rem_euclid(n)).unwrap_or(0);
    Ok((0..len)
        .map(|j| base.get((j + r) % len).copied().unwrap_or(false) != invert)
        .collect())
}

/// A result event with the grid's timing and the subject's content.
fn restruct(timing: &Event, subject: Event) -> Event {
    let mut e = subject;
    let mut path = timing.occ.path.clone();
    path.extend(e.occ.path.iter().copied());
    e.occ.path = path;
    merge_producers(timing, &mut e);
    e.whole = timing.whole;
    e.part = timing.part;
    e
}

pub(crate) fn query_euclid(
    inner: &Pat,
    k: &PParam,
    n: &PParam,
    rot: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        let at = Ratio64::from_int(c);
        let steps = eval_param_int(k, at, st).and_then(|k| {
            let n = eval_param_int(n, at, st)?;
            let r = eval_param_int(rot, at, st)?;
            Ok((n, euclid_steps(k, n, r)?))
        });
        let (n, steps) = match steps {
            Ok(v) => v,
            Err(f) => {
                st.fault(f, p.span, Some(piece.begin));
                continue;
            }
        };
        for (j, on) in steps.iter().enumerate() {
            if !on {
                continue;
            }
            let j64 = i64::try_from(j).unwrap_or(0);
            let whole = match step_span(c, n, j64) {
                Ok(w) => w,
                Err(f) => {
                    st.fault(f, p.span, Some(piece.begin));
                    break;
                }
            };
            let Some(part) = sect(whole, piece) else {
                continue;
            };
            let mut timing = Event::new(Some(whole), part, Value::Nil, None);
            timing.occ.push(p.id, u32::try_from(j).unwrap_or(u32::MAX));
            timing.producer = st.producer();
            if let Some(trace) = &mut timing.producer {
                trace.push(
                    ProducerKind::GeneratedBranch,
                    u32::try_from(j).unwrap_or(u32::MAX),
                );
            }
            match sample_child(inner, whole.begin, 0, st) {
                Ok(sampled) => out.extend(sampled.into_iter().map(|s| restruct(&timing, s))),
                Err(f) => event_fault(st, &timing, p, f),
            }
        }
    }
    out
}

fn is_on(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Nil => false,
        Value::Int(i) => *i != 0,
        Value::Int64(i) => *i != 0,
        Value::Float(x) => *x != 0.0,
        Value::Float64(x) => *x != 0.0,
        Value::Ratio(r) => *r != Ratio64::ZERO,
        _ => true,
    }
}

pub(crate) fn query_grid(
    inner: &Pat,
    bools: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut b in query_child(bools, span, 1, st) {
        if !is_on(&b.value) {
            continue;
        }
        b.occ.push(p.id, 0);
        match st.with_structural_sample(inner, b.anchor(), 0, b.whole, b.part, Some(&b), |st| {
            sample_child(inner, b.anchor(), 0, st)
        }) {
            Ok(sampled) => {
                for s in sampled {
                    let src = s.src.or(b.src);
                    let mut e = restruct(&b, s);
                    e.src = src;
                    out.push(e);
                }
            }
            Err(f) => event_fault(st, &b, p, f),
        }
    }
    out
}
