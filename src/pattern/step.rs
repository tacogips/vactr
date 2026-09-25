//! Step lists (design 10.1): one cycle, a nested list subdivides its step,
//! `nil` is a rest, a pattern value is squeezed into its step.

use std::rc::Rc;

use crate::pattern::eval::{eval_param_int, resolve_dynamic, QState};
use crate::pattern::occ::OccKey;
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::query::{q, sect, Event, TimeSpan};
use crate::reader::span::{Span, SrcRef};
use crate::value::ratio::Ratio64;
use crate::value::value::{ListVal, Value};
use crate::vm::fail::{FailCode, Failure};

/// A step value with the `SrcRef` of the literal element it came from.
#[derive(Clone, Debug)]
pub struct Step {
    pub value: Value,
    pub src: Option<SrcRef>,
}

impl Step {
    /// A step without provenance.
    #[must_use]
    pub const fn bare(value: Value) -> Self {
        Self { value, src: None }
    }
}

/// The steps of a list, with element provenance from `ListProv` when the
/// list came from a literal (the fallback for derived lists is `None`).
#[must_use]
pub fn steps_of_list(list: &ListVal) -> Box<[Step]> {
    list.items
        .iter()
        .enumerate()
        .map(|(i, v)| Step {
            value: v.clone(),
            src: list.prov.as_ref().and_then(|p| {
                p.elems.get(i).map(|span| SrcRef {
                    span: *span,
                    doc_revision: p.doc_revision,
                    form_gen: p.form_gen,
                })
            }),
        })
        .collect()
}

/// A structured step-list pattern.
#[must_use]
pub fn steps(items: Box<[Step]>, span: Option<Span>) -> Pat {
    Pat::new(PatNode::Steps(items), span, true)
}

/// One step laid out in a cycle: its value, weight and provenance.
struct Laid<'a> {
    value: &'a Value,
    src: Option<SrcRef>,
    weight: Ratio64,
}

/// Expands `hold` weights and `repeat` counts for one cycle.
fn lay_out<'a>(
    items: &'a [Step],
    cycle: i64,
    st: &mut QState<'_, '_>,
    span: Option<Span>,
) -> Result<Vec<Laid<'a>>, Failure> {
    let mut out = Vec::with_capacity(items.len());
    let at = Ratio64::from_int(cycle);
    for s in items {
        let (weight, copies) = match &s.value {
            Value::Pattern(p) => match &p.node {
                PatNode::Hold(_, w) => {
                    let w = crate::pattern::eval::eval_param_ratio(w, at, st)?;
                    if w <= Ratio64::ZERO {
                        return Err(Failure::new(FailCode::Type, "hold weight must be positive"));
                    }
                    (w, 1)
                }
                PatNode::Repeat(_, n) => {
                    let n = eval_param_int(n, at, st)?;
                    (Ratio64::ONE, n.clamp(0, 4096))
                }
                _ => (Ratio64::ONE, 1),
            },
            _ => (Ratio64::ONE, 1),
        };
        for _ in 0..copies {
            st.spend(1, span)?;
            out.push(Laid {
                value: &s.value,
                src: s.src,
                weight,
            });
        }
    }
    Ok(out)
}

/// Queries a step list over `span`.
pub(crate) fn query_steps(
    items: &[Step],
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        let step_span = match TimeSpan::cycle(c) {
            Ok(s) => s,
            Err(f) => {
                st.fault(f, p.span, Some(piece.begin));
                continue;
            }
        };
        if let Err(f) = subdivide(items, p, step_span, piece, c, st, &mut out) {
            st.fault(f, p.span, Some(piece.begin));
        }
    }
    out
}

/// Lays `items` out over `outer` (a step of cycle `c`) and queries the
/// part inside `piece`.
fn subdivide(
    items: &[Step],
    p: &Pat,
    outer: TimeSpan,
    piece: TimeSpan,
    c: i64,
    st: &mut QState<'_, '_>,
    out: &mut Vec<Event>,
) -> Result<(), Failure> {
    let laid = lay_out(items, c, st, p.span)?;
    let total = laid
        .iter()
        .try_fold(Ratio64::ZERO, |acc, l| acc.checked_add(l.weight))?;
    if total == Ratio64::ZERO {
        return Ok(());
    }
    let width = outer.end.checked_sub(outer.begin)?;
    let mut offset = Ratio64::ZERO;
    for l in laid {
        let begin = outer
            .begin
            .checked_add(width.checked_mul(offset.checked_div(total)?)?)?;
        offset = offset.checked_add(l.weight)?;
        let end = outer
            .begin
            .checked_add(width.checked_mul(offset.checked_div(total)?)?)?;
        let whole = TimeSpan { begin, end };
        if sect(whole, piece).is_none() {
            continue;
        }
        step_events(l.value, l.src, whole, piece, c, false, p, st, out)?;
    }
    Ok(())
}

/// The events of one step value laid out over `whole`. With
/// `atomic_lists`, a list is one value instead of a subdivision.
#[allow(clippy::too_many_arguments)]
fn step_events(
    value: &Value,
    src: Option<SrcRef>,
    whole: TimeSpan,
    piece: TimeSpan,
    c: i64,
    atomic_lists: bool,
    p: &Pat,
    st: &mut QState<'_, '_>,
    out: &mut Vec<Event>,
) -> Result<(), Failure> {
    match value {
        Value::Nil => Ok(()),
        Value::List(l) if !atomic_lists => {
            let nested = steps_of_list(l);
            let Some(part) = sect(whole, piece) else {
                return Ok(());
            };
            subdivide(&nested, p, whole, part, c, st, out)
        }
        Value::Pattern(inner) => {
            let inner = unwrap_step(inner);
            squeeze(&inner, whole, piece, c, st, out)
        }
        Value::VarRef(_) | Value::Fn(_) | Value::Native(_) | Value::Thunk(_) | Value::Signal(_) => {
            let at = sect(whole, piece).map_or(whole.begin, |s| s.begin);
            let resolved = resolve_dynamic(value, at, st)?;
            match resolved {
                Value::Pattern(inner) => squeeze(&inner, whole, piece, c, st, out),
                // A signal is continuous: no whole, sampled at the part start.
                Value::Signal(sig) => {
                    if let Some(part) = sect(whole, piece) {
                        let v = sig.value_at(part.begin, st.cx)?;
                        st.spend(1, p.span)?;
                        out.push(Event::new(None, part, v, src));
                    }
                    Ok(())
                }
                Value::Nil => Ok(()),
                other => atomic(other, src, whole, piece, p, st, out),
            }
        }
        other => atomic(other.clone(), src, whole, piece, p, st, out),
    }
}

fn atomic(
    value: Value,
    src: Option<SrcRef>,
    whole: TimeSpan,
    piece: TimeSpan,
    p: &Pat,
    st: &mut QState<'_, '_>,
    out: &mut Vec<Event>,
) -> Result<(), Failure> {
    if let Some(part) = sect(whole, piece) {
        st.spend(1, p.span)?;
        out.push(Event::new(Some(whole), part, value, src));
    }
    Ok(())
}

/// `hold` and `repeat` wrap their child; inside a step the child plays.
fn unwrap_step(p: &Rc<Pat>) -> Rc<Pat> {
    match &p.node {
        PatNode::Hold(inner, _) | PatNode::Repeat(inner, _) => Rc::clone(inner),
        _ => Rc::clone(p),
    }
}

/// Squeezes cycle `c` of `inner` into `step` (Tidal's compress): inner time
/// `c + (t - step.begin) / width` for outer time `t`.
pub(crate) fn squeeze(
    inner: &Pat,
    step: TimeSpan,
    piece: TimeSpan,
    c: i64,
    st: &mut QState<'_, '_>,
    out: &mut Vec<Event>,
) -> Result<(), Failure> {
    let Some(part) = sect(step, piece) else {
        return Ok(());
    };
    let width = step.end.checked_sub(step.begin)?;
    if width == Ratio64::ZERO {
        return Ok(());
    }
    let base = Ratio64::from_int(c);
    let to_inner = |t: Ratio64| -> Result<Ratio64, Failure> {
        base.checked_add(t.checked_sub(step.begin)?.checked_div(width)?)
    };
    let to_outer = |t: Ratio64| -> Result<Ratio64, Failure> {
        step.begin
            .checked_add(t.checked_sub(base)?.checked_mul(width)?)
    };
    let inner_span = TimeSpan {
        begin: to_inner(part.begin)?,
        end: to_inner(part.end)?,
    };
    for mut e in q(inner, inner_span, st) {
        e.whole = match e.whole {
            Some(w) => Some(w.map(to_outer)?),
            None => None,
        };
        e.part = e.part.map(to_outer)?;
        out.push(e);
    }
    Ok(())
}

/// One atomic value per cycle (`PatNode::Pure`).
pub(crate) fn query_pure(
    step: &Step,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for piece in st.cycles(span, p.span) {
        let c = piece.begin.floor();
        let r = TimeSpan::cycle(c).and_then(|whole| {
            step_events(
                &step.value,
                step.src,
                whole,
                piece,
                c,
                true,
                p,
                st,
                &mut out,
            )
        });
        if let Err(f) = r {
            st.fault(f, p.span, Some(piece.begin));
        }
    }
    out
}

/// The events of a pattern that is one `hold`/`repeat` step on its own.
pub(crate) fn query_single(
    inner: &Pat,
    copies: i64,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    if copies == 1 {
        return q(inner, span, st);
    }
    let step = Step::bare(Value::Pattern(Rc::new(inner.clone())));
    let items: Vec<Step> = (0..copies.clamp(0, 4096)).map(|_| step.clone()).collect();
    query_steps(&items, p, span, st)
}

impl Event {
    /// An event with an empty occurrence key and no controls.
    #[must_use]
    pub fn new(whole: Option<TimeSpan>, part: TimeSpan, value: Value, src: Option<SrcRef>) -> Self {
        Self {
            whole,
            part,
            value,
            controls: std::collections::BTreeMap::new(),
            src,
            occ: OccKey::empty(),
        }
    }
}
