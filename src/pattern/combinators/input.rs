//! `midi-notes` after `s` and the bind-time input-lane walk (design 11.7).
//!
//! `MidiNotes` yields no events under `query` (future input is unknowable).
//! At bind time [`input_lane_walk`] classifies the operators between each
//! `MidiNotes` node and the root: controls, scale/chord kin and per-note
//! probabilistic filters apply per arriving note in tree order
//! ([`realize_note`]); a re-timing or structural operator, or a structured
//! subject, is `input-lane-operator`.

use std::rc::Rc;

use crate::pattern::combinators::control::control;
use crate::pattern::combinators::music::{chord_notes, scale_event, voice};
use crate::pattern::combinators::{kw, op};
use crate::pattern::eval::{apply_transform, eval_param_f64, QState, QueryCtx};
use crate::pattern::occ::OccKey;
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{q, Event, TimeSpan};
use crate::pattern::rng::unit;
use crate::pattern::step::Step;
use crate::reader::span::{NodeId, Span};
use crate::types::diag::DiagCode;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// `midi-notes channel:` after `s`.
#[must_use]
pub fn midi_notes(subject: Rc<Pat>, channel: Option<u8>, span: Option<Span>) -> Pat {
    op(PatNode::MidiNotes { subject, channel }, span)
}

pub(crate) fn query_midi_notes() -> Vec<Event> {
    Vec::new()
}

/// A per-note operator of an input lane, in application order.
#[derive(Clone, Debug)]
pub enum LaneOp {
    Control(KwId, Rc<Pat>),
    Scale(KwId, KwId),
    Chord(Rc<Pat>),
    Voicing,
    DegradeBy(NodeId, PParam),
    Maybe(NodeId, PParam),
    SometimesBy(NodeId, PParam, Value),
}

/// One live input lane: a `MidiNotes` node and its per-note operators.
#[derive(Clone, Debug)]
pub struct Lane {
    pub node: NodeId,
    pub channel: Option<u8>,
    pub subject: Rc<Pat>,
    pub ops: Vec<LaneOp>,
}

/// Every input lane of a bound pattern.
#[derive(Clone, Debug, Default)]
pub struct LanePlan {
    pub lanes: Vec<Lane>,
}

/// A bind-time rejection. The plan's signature carries a `Span`; patterns
/// built in tests may have none, so the span is optional.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LaneError {
    pub code: DiagCode,
    pub span: Option<Span>,
    pub message: String,
}

/// A NoteOn delivered to a lane.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LiveNote {
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
    /// The arrival sequence number (keys the per-note RNG).
    pub seq: u64,
    /// The logical position the note is realized at.
    pub at: Ratio64,
}

fn contains_lane(p: &Pat) -> bool {
    match &p.node {
        PatNode::MidiNotes { .. } => true,
        PatNode::Steps(items) => items.iter().any(|s| match &s.value {
            Value::Pattern(inner) => contains_lane(inner),
            _ => false,
        }),
        PatNode::Sound { src, kit } => {
            param_has_lane(src) || kit.as_ref().is_some_and(param_has_lane)
        }
        PatNode::Signal(_) | PatNode::SongSource(_) => false,
        PatNode::Pure(s) => matches!(&s.value, Value::Pattern(inner) if contains_lane(inner)),
        PatNode::Choose(ps) | PatNode::Stack(ps) | PatNode::Cat(ps) | PatNode::FastCat(ps) => {
            ps.iter().any(contains_lane)
        }
        PatNode::Grid(a, b) | PatNode::Chord(a, b) => contains_lane(a) || contains_lane(b),
        PatNode::Control(_, v, s) => contains_lane(v) || contains_lane(s),
        PatNode::Slice { pat, index, .. } | PatNode::Splice { pat, index, .. } => {
            contains_lane(pat) || contains_lane(index)
        }
        PatNode::Fast(x, _)
        | PatNode::Slow(x, _)
        | PatNode::Hurry(x, _)
        | PatNode::Rev(x)
        | PatNode::Every(_, _, x)
        | PatNode::WhenMod(_, _, _, x)
        | PatNode::SometimesBy(_, _, x)
        | PatNode::DegradeBy(x, _)
        | PatNode::Maybe(x, _)
        | PatNode::Hold(x, _)
        | PatNode::Repeat(x, _)
        | PatNode::Superimpose(x, _)
        | PatNode::Off(x, _, _)
        | PatNode::Jux(x, _)
        | PatNode::Iter(x, _)
        | PatNode::Chop(x, _)
        | PatNode::Ply(x, _)
        | PatNode::Striate(x, _)
        | PatNode::LoopAt(x, _)
        | PatNode::Fit(x)
        | PatNode::Chunk(x, _, _)
        | PatNode::Euclid(x, _, _, _)
        | PatNode::ScaleNotes(_, _, x)
        | PatNode::Voicing(x)
        | PatNode::Arp(x, _)
        | PatNode::Segment(x, _)
        | PatNode::Range(x, _, _) => contains_lane(x),
    }
}

fn param_has_lane(p: &PParam) -> bool {
    matches!(p, PParam::Pat(x) if contains_lane(x))
}

fn reject(p: &Pat, message: &str) -> LaneError {
    LaneError {
        code: DiagCode::InputLaneOperator,
        span: p.span,
        message: message.to_string(),
    }
}

/// Classifies every operator on the path from each `MidiNotes` node to the
/// root of `p` (the dry-run walk at bind time).
///
/// # Errors
/// `input-lane-operator` at the first unsupported operator.
pub fn input_lane_walk(p: &Pat) -> Result<LanePlan, LaneError> {
    let mut plan = LanePlan::default();
    walk(p, &mut Vec::new(), &mut plan)?;
    Ok(plan)
}

/// `above` holds the operators between this node and the root, outermost
/// first.
fn walk(p: &Pat, above: &mut Vec<LaneOp>, plan: &mut LanePlan) -> Result<(), LaneError> {
    if !contains_lane(p) {
        return Ok(());
    }
    let mut descend = |op: LaneOp, inner: &Pat, above: &mut Vec<LaneOp>| {
        above.push(op);
        let r = walk(inner, above, plan);
        above.pop();
        r
    };
    match &p.node {
        PatNode::MidiNotes { subject, channel } => {
            if subject.structured || contains_lane(subject) {
                return Err(reject(
                    p,
                    "cannot re-time live input: midi-notes needs a single sound",
                ));
            }
            plan.lanes.push(Lane {
                node: p.id,
                channel: *channel,
                subject: Rc::clone(subject),
                ops: above.iter().rev().cloned().collect(),
            });
            Ok(())
        }
        PatNode::Stack(items) => {
            for item in items.iter() {
                walk(item, above, plan)?;
            }
            Ok(())
        }
        PatNode::Control(k, v, s) if !contains_lane(v) => {
            descend(LaneOp::Control(*k, Rc::clone(v)), s, above)
        }
        PatNode::ScaleNotes(r, n, s) => descend(LaneOp::Scale(*r, *n), s, above),
        PatNode::Chord(c, s) if !contains_lane(c) => descend(LaneOp::Chord(Rc::clone(c)), s, above),
        PatNode::Voicing(s) => descend(LaneOp::Voicing, s, above),
        PatNode::DegradeBy(s, x) => descend(LaneOp::DegradeBy(p.id, x.clone()), s, above),
        PatNode::Maybe(s, x) => descend(LaneOp::Maybe(p.id, x.clone()), s, above),
        PatNode::SometimesBy(x, f, s) => {
            descend(LaneOp::SometimesBy(p.id, x.clone(), f.clone()), s, above)
        }
        _ => Err(reject(p, "cannot re-time live input")),
    }
}

/// The one-note pattern handed to a `sometimes-by` transform: the note's
/// value with its controls.
fn note_pattern(e: &Event) -> Rc<Pat> {
    let mut p = Rc::new(Pat::new(
        PatNode::Pure(Step::bare(e.value.clone())),
        None,
        false,
    ));
    for (k, v) in &e.controls {
        let value = Rc::new(Pat::new(PatNode::Pure(Step::bare(v.clone())), None, false));
        p = Rc::new(control(*k, value, p, None));
    }
    p
}

/// Realizes one arriving note on a lane: the subject's sound and controls,
/// the note and velocity, then each per-note operator in tree order.
/// `Ok(None)` when the note is for another channel or a filter dropped it.
///
/// # Errors
/// An event-local failure of a control, scale, chord or transform.
pub fn realize_note(
    lane: &Lane,
    note: &LiveNote,
    cx: &mut QueryCtx<'_>,
) -> Result<Option<Event>, Failure> {
    if lane.channel.is_some_and(|c| c != note.channel) {
        return Ok(None);
    }
    let mut st = QState::new(cx);
    let at = note.at;
    let Some(base) = st.sample(&lane.subject, at)?.into_iter().next() else {
        return Ok(None);
    };
    let mut e = sampled_note_event(base, at);
    e.controls
        .insert(kw("note"), Value::Int(i32::from(note.note)));
    e.controls.insert(
        kw("velocity"),
        Value::Float64(f64::from(note.velocity) / 127.0),
    );
    e.occ = OccKey::empty();
    e.occ
        .push(lane.node, u32::try_from(note.seq).unwrap_or(u32::MAX));
    for op in &lane.ops {
        let draw = |node: NodeId, st: &QState<'_, '_>| unit(st.cx.seed, node, 0, note.seq);
        match op {
            LaneOp::Control(k, v) => {
                let value = st.sample_value(v, at)?;
                if matches!(value, Value::Nil) {
                    return Ok(None);
                }
                e.controls.insert(*k, value);
            }
            LaneOp::Scale(root, name) => scale_event(&mut e, *root, *name)?,
            LaneOp::Chord(c) => {
                let v = st.sample_value(c, at)?;
                let notes = chord_notes(&v)?;
                e.controls.insert(kw("note"), note_list(&notes));
            }
            LaneOp::Voicing => {
                if let Some(Value::List(l)) = e.controls.get(&kw("note")) {
                    let notes: Option<Vec<i64>> =
                        l.items.iter().map(crate::pattern::eval::int_of).collect();
                    if let Some(notes) = notes {
                        e.controls.insert(kw("note"), note_list(&voice(&notes)));
                    }
                }
            }
            LaneOp::DegradeBy(node, x) => {
                let prob = eval_param_f64(x, at, &mut st)?;
                if draw(*node, &st) < prob {
                    return Ok(None);
                }
            }
            LaneOp::Maybe(node, x) => {
                let prob = eval_param_f64(x, at, &mut st)?;
                if draw(*node, &st) >= prob {
                    return Ok(None);
                }
            }
            LaneOp::SometimesBy(node, x, f) => {
                let prob = eval_param_f64(x, at, &mut st)?;
                if draw(*node, &st) < prob {
                    let t = apply_transform(f, &note_pattern(&e), &mut st)?;
                    let cycle = TimeSpan::cycle(0)?;
                    let mark = st.faults.len();
                    let got = q(&t, cycle, &mut st);
                    if st.faults.len() > mark {
                        return Err(st.faults.remove(mark));
                    }
                    let Some(first) = got.into_iter().find(|g| g.is_onset()) else {
                        return Ok(None);
                    };
                    e.value = first.value;
                    e.controls = first.controls;
                }
            }
        }
    }
    crate::pattern::combinators::sound::pick_bank(&mut e)?;
    e.occ.anchor_at(at);
    Ok(Some(e))
}

fn note_list(notes: &[i64]) -> Value {
    Value::list(
        notes
            .iter()
            .map(|n| crate::pattern::eval::exact_value(Ratio64::from_int(*n)))
            .collect(),
    )
}

fn sampled_note_event(base: Event, at: Ratio64) -> Event {
    Event::from_sample(None, TimeSpan::point(at), base)
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    #[test]
    fn live_note_reconstruction_retains_sampled_origin() {
        let source = crate::song::source::provenance_fixture();
        let origin = source.song_source.clone().unwrap();
        let mut event = sampled_note_event(source, Ratio64::from_int(12));
        event.controls.insert(kw("note"), Value::Int(72));
        assert!(Rc::ptr_eq(&origin, event.song_source.as_ref().unwrap()));
        assert_eq!(origin.handle.tone(), 2);
        assert!(event.whole.is_none());
        assert_eq!(event.part, TimeSpan::point(Ratio64::from_int(12)));
        assert!(matches!(
            event.controls.get(&kw("note")),
            Some(Value::Int(72))
        ));
    }
}
