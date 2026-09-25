//! Sample-region operators (design 10.1): chop, striate, slice, splice,
//! loop-at, fit. Each is defined over the source event's WHOLE span and its
//! occurrence identity, never over the query-clipped part (layer-1 cover
//! equivalence). Seconds exist only at commit: rate fitting is a marker
//! control resolved there with [`SpeedFit::resolve`].

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::pattern::combinators::{count, event_fault, kw, no_whole, op, split_event};
use crate::pattern::eval::{eval_param_int, eval_param_ratio, int_of, num_ratio, QState};
use crate::pattern::occ::OccKey;
use crate::pattern::pat::{PParam, Pat, PatNode, SliceCuts};
use crate::pattern::query::{q, Event, TimeSpan};
use crate::reader::span::{NodeId, Span};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// A commit-time rate fit, carried as the `speed-fit` control.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpeedFit {
    /// `splice`: the slice (a `fraction` of the sample) fits the whole span
    /// of `cycles`.
    Splice { fraction: Ratio64, cycles: Ratio64 },
    /// `fit`: the sample fits the whole span of `cycles`.
    Fit { cycles: Ratio64 },
    /// `loop-at n`: the sample stretches over `cycles` cycles.
    LoopAt { cycles: Ratio64 },
}

impl SpeedFit {
    /// The control value: `[:splice fraction cycles]`, `[:fit cycles]` or
    /// `[:loop-at cycles]`.
    #[must_use]
    pub fn to_value(self) -> Value {
        match self {
            SpeedFit::Splice { fraction, cycles } => Value::list(vec![
                Value::kw("splice"),
                Value::Ratio(fraction),
                Value::Ratio(cycles),
            ]),
            SpeedFit::Fit { cycles } => Value::list(vec![Value::kw("fit"), Value::Ratio(cycles)]),
            SpeedFit::LoopAt { cycles } => {
                Value::list(vec![Value::kw("loop-at"), Value::Ratio(cycles)])
            }
        }
    }

    /// Parses a `speed-fit` control value.
    #[must_use]
    pub fn from_value(v: &Value) -> Option<Self> {
        let Value::List(l) = v else {
            return None;
        };
        let name = match l.items.first()? {
            Value::Keyword(k) => crate::value::intern::name_of_kw(*k),
            _ => return None,
        };
        let r = |i: usize| l.items.get(i).and_then(num_ratio);
        match (&*name, l.items.len()) {
            ("splice", 3) => Some(SpeedFit::Splice {
                fraction: r(1)?,
                cycles: r(2)?,
            }),
            ("fit", 2) => Some(SpeedFit::Fit { cycles: r(1)? }),
            ("loop-at", 2) => Some(SpeedFit::LoopAt { cycles: r(1)? }),
            _ => None,
        }
    }

    /// The `speed` at commit: the region's seconds over the target seconds.
    /// `sample_seconds` is the bank sample's duration (the `SampleLoader`
    /// manifest), `cycle_seconds` the committed cycle length.
    #[must_use]
    pub fn resolve(self, sample_seconds: f64, cycle_seconds: f64) -> f64 {
        let (region, cycles) = match self {
            SpeedFit::Splice { fraction, cycles } => (fraction.to_f64() * sample_seconds, cycles),
            SpeedFit::Fit { cycles } | SpeedFit::LoopAt { cycles } => (sample_seconds, cycles),
        };
        region / (cycles.to_f64() * cycle_seconds)
    }
}

/// `chop p n`: each event's whole cut into n contiguous regions.
#[must_use]
pub fn chop(p: Rc<Pat>, n: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Chop(p, n), span)
}

/// `striate p n`: timing unchanged; the i-th onset of a cycle plays region
/// `i mod n`.
#[must_use]
pub fn striate(p: Rc<Pat>, n: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Striate(p, n), span)
}

/// Whether a slice takes its structure from the index pattern: the index
/// is a structure-giving list on an unstructured subject (10.1
/// first-structure rule, as for a list control).
fn slice_structured(pat: &Pat, index: &Pat) -> bool {
    pat.structured || index.structured
}

/// `slice p cuts index`.
#[must_use]
pub fn slice(p: Rc<Pat>, cuts: SliceCuts, index: Rc<Pat>, span: Option<Span>) -> Pat {
    let s = slice_structured(&p, &index);
    Pat::new(
        PatNode::Slice {
            pat: p,
            cuts,
            index,
        },
        span,
        s,
    )
}

/// `splice p cuts index`: `slice` plus a rate fit to the event.
#[must_use]
pub fn splice(p: Rc<Pat>, cuts: SliceCuts, index: Rc<Pat>, span: Option<Span>) -> Pat {
    let s = slice_structured(&p, &index);
    Pat::new(
        PatNode::Splice {
            pat: p,
            cuts,
            index,
        },
        span,
        s,
    )
}

/// `loop-at p n`.
#[must_use]
pub fn loop_at(p: Rc<Pat>, n: PParam, span: Option<Span>) -> Pat {
    op(PatNode::LoopAt(p, n), span)
}

/// `fit p`.
#[must_use]
pub fn fit(p: Rc<Pat>, span: Option<Span>) -> Pat {
    op(PatNode::Fit(p), span)
}

/// The current region of an event: `begin`/`end` controls, default 0..1.
fn region_of(e: &Event) -> (Ratio64, Ratio64) {
    let get = |name: &str, default: Ratio64| {
        e.controls
            .get(&kw(name))
            .and_then(num_ratio)
            .unwrap_or(default)
    };
    (get("begin", Ratio64::ZERO), get("end", Ratio64::ONE))
}

fn set_region(e: &mut Event, begin: Ratio64, end: Ratio64) {
    e.controls.insert(kw("begin"), Value::Ratio(begin));
    e.controls.insert(kw("end"), Value::Ratio(end));
}

fn lerp(a: Ratio64, b: Ratio64, i: i64, k: i64) -> Result<Ratio64, Failure> {
    a.checked_add(b.checked_sub(a)?.checked_mul(Ratio64::new(i, k)?)?)
}

pub(crate) fn query_chop(
    inner: &Pat,
    n: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for e in q(inner, span, st) {
        let r = (|| {
            if e.whole.is_none() {
                return Err(no_whole());
            }
            let k = count(eval_param_int(n, e.anchor(), st)?, "chop count")?;
            let (b0, e0) = region_of(&e);
            let mut kids = Vec::new();
            for (i, mut child) in split_event(&e, k)? {
                set_region(&mut child, lerp(b0, e0, i, k)?, lerp(b0, e0, i + 1, k)?);
                child.occ.push(p.id, u32::try_from(i).unwrap_or(u32::MAX));
                kids.push(child);
            }
            Ok(kids)
        })();
        match r {
            Ok(kids) => out.extend(kids),
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

/// The identities of a cycle's onsets, in rank order.
type Ranked = Vec<(Vec<(NodeId, u32)>, Ratio64)>;

/// The onsets of cycle `c` of `inner`, ranked by `whole.begin` (ties keep
/// the deterministic branch order), as (path, begin) identities.
fn rank_cycle(inner: &Pat, c: i64, st: &mut QState<'_, '_>) -> Result<Ranked, Failure> {
    let cycle = TimeSpan::cycle(c)?;
    // Faults of the widening query belong to events the main query reports
    // itself (or to events outside the requested span): drop them here.
    let mark = st.faults.len();
    let events = q(inner, cycle, st);
    st.faults.truncate(mark);
    let mut onsets: Ranked = events
        .into_iter()
        .filter(|e| e.is_onset() && e.anchor().floor() == c)
        .map(|e| {
            let a = e.anchor();
            (e.occ.path, a)
        })
        .collect();
    onsets.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(onsets)
}

pub(crate) fn query_striate(
    inner: &Pat,
    n: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut ranks: BTreeMap<i64, Ranked> = BTreeMap::new();
    let mut out = Vec::new();
    for mut e in q(inner, span, st) {
        let r = (|| {
            let Some(w) = e.whole else {
                return Err(no_whole());
            };
            let k = count(eval_param_int(n, w.begin, st)?, "striate count")?;
            let c = w.begin.floor();
            if let std::collections::btree_map::Entry::Vacant(slot) = ranks.entry(c) {
                slot.insert(rank_cycle(inner, c, st)?);
            }
            let rank = ranks
                .get(&c)
                .and_then(|r| {
                    r.iter()
                        .position(|(path, b)| *path == e.occ.path && *b == w.begin)
                })
                .ok_or_else(|| Failure::new(FailCode::Type, "striate could not rank an event"))?;
            let ord = i64::try_from(rank).unwrap_or(0).rem_euclid(k);
            Ok((ord, k))
        })();
        match r {
            Ok((ord, k)) => {
                let region =
                    (|| Ok::<_, Failure>((Ratio64::new(ord, k)?, Ratio64::new(ord + 1, k)?)))();
                match region {
                    Ok((b, en)) => {
                        set_region(&mut e, b, en);
                        e.occ.push(p.id, u32::try_from(ord).unwrap_or(u32::MAX));
                        out.push(e);
                    }
                    Err(f) => event_fault(st, &e, p, f),
                }
            }
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

/// The slice starts for one event; slice i spans `[s_i, s_{i+1})`, the
/// last to 1.
fn slice_starts(
    cuts: &SliceCuts,
    at: Ratio64,
    st: &mut QState<'_, '_>,
) -> Result<Vec<Ratio64>, Failure> {
    let bad = |m: &str| Failure::new(FailCode::BadSlicePoints, m.to_string());
    match cuts {
        SliceCuts::Equal(n) => {
            let n = eval_param_int(n, at, st)?;
            let n = count(n, "slice count").map_err(|_| bad("slice count must be at least 1"))?;
            (0..n).map(|i| Ratio64::new(i, n)).collect()
        }
        SliceCuts::Manual(points) => {
            let mut out: Vec<Ratio64> = Vec::with_capacity(points.len());
            for pt in points.iter() {
                let v = eval_param_ratio(pt, at, st)?;
                if v < Ratio64::ZERO || v > Ratio64::ONE {
                    return Err(bad("slice points must lie in [0, 1]"));
                }
                if out.last().is_some_and(|last| *last >= v) {
                    return Err(bad("slice points must be strictly ascending"));
                }
                out.push(v);
            }
            if out.is_empty() {
                return Err(bad("manual slice points are empty"));
            }
            Ok(out)
        }
    }
}

/// Sets the region (and, for splice, the rate fit) of one event from its
/// sampled index value.
fn apply_slice(
    e: &mut Event,
    idx: &Value,
    cuts: &SliceCuts,
    splice: bool,
    st: &mut QState<'_, '_>,
) -> Result<(), Failure> {
    let Some(w) = e.whole else {
        return Err(no_whole());
    };
    let starts = slice_starts(cuts, w.begin, st)?;
    let i = int_of(idx)
        .ok_or_else(|| Failure::new(FailCode::Type, "a slice index must be an integer"))?;
    let slot = usize::try_from(i)
        .ok()
        .filter(|i| *i < starts.len())
        .ok_or_else(|| {
            Failure::new(
                FailCode::SliceIndex,
                format!("slice index {i} outside 0..{}", starts.len()),
            )
        })?;
    let begin = starts[slot];
    let end = starts.get(slot + 1).copied().unwrap_or(Ratio64::ONE);
    set_region(e, begin, end);
    if splice {
        let fit = SpeedFit::Splice {
            fraction: end.checked_sub(begin)?,
            cycles: w.duration()?,
        };
        e.controls.insert(kw("speed-fit"), fit.to_value());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn query_slice(
    pat: &Pat,
    cuts: &SliceCuts,
    index: &Pat,
    splice: bool,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    // (event with the subject's content, index value)
    let mut pending: Vec<(Event, Value)> = Vec::new();
    if !pat.structured && index.structured {
        // The index list gives the structure (first-structure rule).
        for ie in q(index, span, st) {
            match st.sample(pat, ie.anchor()) {
                Ok(sampled) => {
                    for s in sampled {
                        let mut e = s;
                        let mut path = ie.occ.path.clone();
                        path.extend(e.occ.path.iter().copied());
                        e.occ = OccKey {
                            path,
                            ..OccKey::empty()
                        };
                        e.whole = ie.whole;
                        e.part = ie.part;
                        e.src = ie.src.or(e.src);
                        pending.push((e, ie.value.clone()));
                    }
                }
                Err(f) => event_fault(st, &ie, p, f),
            }
        }
    } else {
        for e in q(pat, span, st) {
            match e.whole {
                None => event_fault(st, &e, p, no_whole()),
                Some(w) => match st.sample_value(index, w.begin) {
                    Ok(Value::Nil) => {}
                    Ok(v) => pending.push((e, v)),
                    Err(f) => event_fault(st, &e, p, f),
                },
            }
        }
    }
    let mut out = Vec::with_capacity(pending.len());
    for (mut e, idx) in pending {
        match apply_slice(&mut e, &idx, cuts, splice, st) {
            Ok(()) => out.push(e),
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_loop_at(
    inner: &Pat,
    n: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut e in q(inner, span, st) {
        let r = (|| {
            let Some(w) = e.whole else {
                return Err(no_whole());
            };
            let cycles = eval_param_ratio(n, w.begin, st)?;
            if cycles <= Ratio64::ZERO {
                return Err(Failure::new(
                    FailCode::Type,
                    "loop-at expects a positive count",
                ));
            }
            Ok(cycles)
        })();
        match r {
            Ok(cycles) => {
                set_region(&mut e, Ratio64::ZERO, Ratio64::ONE);
                e.controls.insert(kw("loop"), Value::Bool(true));
                e.controls
                    .insert(kw("speed-fit"), SpeedFit::LoopAt { cycles }.to_value());
                out.push(e);
            }
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_fit(
    inner: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut e in q(inner, span, st) {
        match e.whole.map(TimeSpan::duration) {
            Some(Ok(cycles)) => {
                e.controls
                    .insert(kw("speed-fit"), SpeedFit::Fit { cycles }.to_value());
                out.push(e);
            }
            Some(Err(f)) => event_fault(st, &e, p, f),
            None => event_fault(st, &e, p, no_whole()),
        }
    }
    out
}
