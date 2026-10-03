//! Step lists (design 10.1): one cycle, a nested list subdivides its step,
//! `nil` is a rest, a pattern value is squeezed into its step.

use std::rc::Rc;

use crate::pattern::eval::{eval_param_int, resolve_dynamic, QState};
use crate::pattern::occ::{OccKey, ProducerKind};
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::query::{q_child, sect, Event, TimeSpan};
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
    ordinal: u32,
    copy: Option<u32>,
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
    for (ordinal, s) in items.iter().enumerate() {
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| Failure::new(FailCode::Overflow, "step ordinal overflow"))?;
        let repeated =
            matches!(&s.value, Value::Pattern(p) if matches!(p.node, PatNode::Repeat(..)));
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
        for copy in 0..copies {
            st.spend(1, span)?;
            out.push(Laid {
                value: &s.value,
                src: s.src,
                weight,
                ordinal,
                copy: if repeated {
                    Some(u32::try_from(copy).map_err(|_| {
                        Failure::new(FailCode::Overflow, "step copy ordinal overflow")
                    })?)
                } else {
                    None
                },
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
    // Nested lists recurse here without going through q(). Apply the same
    // evaluator bound so programmatically built lists cannot bypass it.
    st.enter()?;
    let result = subdivide_inner(items, p, outer, piece, c, st, out);
    st.leave();
    result
}

fn subdivide_inner(
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
        st.with_producer(ProducerKind::NestedStep, l.ordinal, |st| {
            let mut emit = |st: &mut QState<'_, '_>| {
                step_events(l.value, l.src, whole, piece, c, false, p, st, out)
            };
            match l.copy {
                Some(copy) => st.with_producer(ProducerKind::GeneratedBranch, copy, emit),
                None => emit(st),
            }
        })?;
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
            let wrapped = matches!(inner.node, PatNode::Hold(..) | PatNode::Repeat(..));
            let child = unwrap_step(inner);
            if wrapped {
                st.with_producer(ProducerKind::Child, 0, |st| {
                    squeeze(&child, whole, piece, c, st, out)
                })
            } else {
                squeeze(&child, whole, piece, c, st, out)
            }
        }
        Value::VarRef(_) | Value::Fn(_) | Value::Native(_) | Value::Thunk(_) | Value::Signal(_) => {
            st.with_producer(ProducerKind::DynamicExpansion, 0, |st| {
                let at = st.step_onset(whole, piece);
                let resolved = resolve_dynamic(value, at, st)?;
                match resolved {
                    Value::Pattern(inner) => squeeze(&inner, whole, piece, c, st, out),
                    // A signal is continuous: no whole, sampled at the part start.
                    Value::Signal(sig) => {
                        if let Some(part) = sect(whole, piece) {
                            let v = sig.value_at(part.begin, st.cx)?;
                            st.spend(1, p.span)?;
                            let mut event = Event::new(None, part, v, src);
                            event.producer = st.producer();
                            out.push(event);
                        }
                        Ok(())
                    }
                    Value::Nil => Ok(()),
                    // A late name whose value is a list (`let bar [..]`, then
                    // `[bar bar]` or `cat [bar ..]`) is a nested step list, like
                    // the list literal itself; atomic positions keep it whole.
                    Value::List(l) if !atomic_lists => {
                        let nested = steps_of_list(&l);
                        let Some(part) = sect(whole, piece) else {
                            return Ok(());
                        };
                        subdivide(&nested, p, whole, part, c, st, out)
                    }
                    other => {
                        let mark = out.len();
                        atomic(other, src, whole, piece, p, st, out)?;
                        // A direct var or tweak read (the slot holds the value
                        // itself, not a function of time): commit keeps the
                        // source so the value can travel as a control cell
                        // (11.3).
                        if let Value::VarRef(r) = value {
                            let direct = !matches!(
                                r.get(),
                                Value::Fn(_)
                                    | Value::Native(_)
                                    | Value::Thunk(_)
                                    | Value::VarRef(_)
                            );
                            if direct {
                                for e in &mut out[mark..] {
                                    e.late = Some(r.clone());
                                }
                            }
                        }
                        Ok(())
                    }
                }
            })
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
    crate::pattern::build::reject_finite(&value)?;
    if let Some(part) = sect(whole, piece) {
        st.spend(1, p.span)?;
        let mut event = Event::new(Some(whole), part, value, src);
        event.producer = st.producer();
        out.push(event);
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
    let events = if st.has_clock_observation() {
        st.with_clock_frame(
            || {
                crate::pattern::eval::song_clock::CanonicalClockFrame::squeeze(
                    width, base, step, inner_span, part,
                )
            },
            |st| Ok(q_child(inner, inner_span, 0, st)),
        )?
    } else {
        q_child(inner, inner_span, 0, st)
    };
    for mut e in events {
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
        return q_child(inner, span, 0, st);
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
            producer: None,
            song_source: None,
            late: None,
            cells: std::collections::BTreeMap::new(),
        }
    }
}

impl Event {
    /// Rebuilds sampled content at new timing while retaining typed provenance,
    /// controls and full producer identity. The new occurrence key is assigned
    /// by the normal query finalizer, independently of certified source identity.
    pub(crate) fn from_sample(whole: Option<TimeSpan>, part: TimeSpan, sampled: Self) -> Self {
        let mut event = Self::new(whole, part, sampled.value, sampled.src);
        event.controls = sampled.controls;
        event.producer = sampled.producer;
        event.song_source = sampled.song_source;
        event.late = sampled.late;
        event.cells = sampled.cells;
        event
    }
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    #[test]
    fn sampled_reconstruction_retains_metadata_and_assigns_new_timing() {
        let source = crate::song::source::provenance_fixture();
        let origin = source.song_source.clone().unwrap();
        let span = TimeSpan::cycle(7).unwrap();
        let reconstructed = Event::from_sample(Some(span), span, source);
        assert!(Rc::ptr_eq(
            &origin,
            reconstructed.song_source.as_ref().unwrap()
        ));
        assert_eq!(reconstructed.whole, Some(span));
        assert_eq!(reconstructed.part, span);
        assert_eq!(origin.handle.occurrence().onset, Ratio64::ONE);
        assert_eq!(origin.handle.tone(), 2);
    }
}
