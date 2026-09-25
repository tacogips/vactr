//! Time spans, events and the pure query (design 10.2, 10.3).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::pattern::combinators::{control, input, music, random, region, sound, structure, time};
use crate::pattern::eval::{QState, QueryCtx};
use crate::pattern::occ::OccKey;
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::signal::Sig;
use crate::pattern::step::{query_pure, query_single, query_steps};
use crate::reader::span::SrcRef;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure, Origin};

/// A span of cycles `[begin, end)`, exact. `begin == end` is a point, used
/// to sample a pattern at one position.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TimeSpan {
    pub begin: Ratio64,
    pub end: Ratio64,
}

/// Sorted, small event controls.
pub type Controls = BTreeMap<KwId, Value>;

/// One event (design 10.2). An onset starts a note: `part.begin ==
/// whole.begin`; a clipped continuation never retriggers.
#[derive(Clone, Debug)]
pub struct Event {
    pub whole: Option<TimeSpan>,
    pub part: TimeSpan,
    pub value: Value,
    pub controls: Controls,
    pub src: Option<SrcRef>,
    pub occ: OccKey,
}

/// The result of a query: events, event- and subtree-local faults, and
/// captured `print` output (10.3, 10.4).
#[derive(Debug, Default)]
pub struct QueryResult {
    pub events: Vec<Event>,
    pub faults: Vec<Failure>,
    pub output: Vec<(Origin, Rc<str>)>,
}

impl TimeSpan {
    /// `[begin, end)`.
    ///
    /// # Errors
    /// `Type` when `end < begin`.
    pub fn new(begin: Ratio64, end: Ratio64) -> Result<Self, Failure> {
        if end < begin {
            return Err(Failure::new(
                FailCode::Type,
                "a time span ends before it begins",
            ));
        }
        Ok(Self { begin, end })
    }

    /// Cycle `c`: `[c, c + 1)`.
    ///
    /// # Errors
    /// `Overflow` at the end of the `i64` range.
    pub fn cycle(c: i64) -> Result<Self, Failure> {
        let end = c
            .checked_add(1)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "cycle number overflow"))?;
        Ok(Self {
            begin: Ratio64::from_int(c),
            end: Ratio64::from_int(end),
        })
    }

    /// The point `t`.
    #[must_use]
    pub const fn point(t: Ratio64) -> Self {
        Self { begin: t, end: t }
    }

    /// True for a zero-width span.
    #[must_use]
    pub fn is_point(self) -> bool {
        self.begin == self.end
    }

    /// `end - begin`.
    ///
    /// # Errors
    /// `Overflow`.
    pub fn duration(self) -> Result<Ratio64, Failure> {
        self.end.checked_sub(self.begin)
    }

    /// Applies a monotonic time map to both ends.
    ///
    /// # Errors
    /// The map's failure.
    pub fn map(self, f: impl Fn(Ratio64) -> Result<Ratio64, Failure>) -> Result<Self, Failure> {
        Ok(Self {
            begin: f(self.begin)?,
            end: f(self.end)?,
        })
    }

    /// The pieces of the span inside each cycle it touches.
    pub(crate) fn cycle_pieces(self, st: &mut QState<'_, '_>) -> Result<Vec<TimeSpan>, Failure> {
        if self.is_point() {
            return Ok(vec![self]);
        }
        let mut out = Vec::new();
        let mut c = self.begin.floor();
        loop {
            let cyc = TimeSpan::cycle(c)?;
            if cyc.begin >= self.end {
                break;
            }
            st.spend(1, None)?;
            out.push(TimeSpan {
                begin: self.begin.max(cyc.begin),
                end: self.end.min(cyc.end),
            });
            c = c
                .checked_add(1)
                .ok_or_else(|| Failure::new(FailCode::Overflow, "cycle number overflow"))?;
        }
        Ok(out)
    }
}

/// The part of `whole` inside `span`. For a point span, the point when
/// `whole` contains it; a point `whole` is inside a span that contains it.
#[must_use]
pub fn sect(whole: TimeSpan, span: TimeSpan) -> Option<TimeSpan> {
    if span.is_point() {
        let t = span.begin;
        let inside = if whole.is_point() {
            whole.begin == t
        } else {
            whole.begin <= t && t < whole.end
        };
        return inside.then_some(span);
    }
    if whole.is_point() {
        return (span.begin <= whole.begin && whole.begin < span.end).then_some(whole);
    }
    let begin = whole.begin.max(span.begin);
    let end = whole.end.min(span.end);
    (begin < end).then_some(TimeSpan { begin, end })
}

impl Event {
    /// `whole.begin`, or `part.begin` for a fragment without a whole.
    #[must_use]
    pub fn anchor(&self) -> Ratio64 {
        self.whole.map_or(self.part.begin, |w| w.begin)
    }

    /// True when this (sub-)event starts its note.
    #[must_use]
    pub fn is_onset(&self) -> bool {
        self.whole.is_some_and(|w| w.begin == self.part.begin)
    }

    /// Shifts both spans by `d` cycles.
    ///
    /// # Errors
    /// `Overflow`.
    pub fn shifted(mut self, d: Ratio64) -> Result<Self, Failure> {
        let f = |t: Ratio64| t.checked_add(d);
        self.whole = match self.whole {
            Some(w) => Some(w.map(f)?),
            None => None,
        };
        self.part = self.part.map(f)?;
        Ok(self)
    }
}

/// Queries `p` over `span` (design 10.3). Never an error: failures are
/// recorded event-locally or subtree-locally in `faults`, each with its
/// origin, and sibling events survive. Pure relative to the values read
/// through `cx` (10.4).
pub fn query(p: &Pat, span: TimeSpan, cx: &mut QueryCtx<'_>) -> QueryResult {
    let mut st = QState::new(cx);
    let raw = q(p, span, &mut st);
    let mut events = Vec::with_capacity(raw.len());
    for mut e in raw {
        let anchor = e.anchor();
        match sound::pick_bank(&mut e) {
            Ok(()) => {
                e.occ.anchor_at(anchor);
                events.push(e);
            }
            Err(f) => {
                let span = e.src.map(|s| s.span).or(p.span);
                st.fault(f, span, Some(anchor));
            }
        }
    }
    let faults = st.faults;
    let output = cx.vm.take_output();
    QueryResult {
        events,
        faults,
        output,
    }
}

/// The internal query: no finalization, faults into `st`.
pub(crate) fn q(p: &Pat, span: TimeSpan, st: &mut QState<'_, '_>) -> Vec<Event> {
    if st.exhausted() {
        return Vec::new();
    }
    if let Err(f) = st.enter() {
        st.fault(f, p.span, Some(span.begin));
        return Vec::new();
    }
    let out = dispatch(p, span, st);
    st.leave();
    out
}

fn dispatch(p: &Pat, span: TimeSpan, st: &mut QState<'_, '_>) -> Vec<Event> {
    match &p.node {
        PatNode::Steps(items) => query_steps(items, p, span, st),
        PatNode::Pure(step) => query_pure(step, p, span, st),
        PatNode::Sound { src, kit } => sound::query_sound(src, kit.as_ref(), p, span, st),
        PatNode::Signal(sig) => query_signal(sig, p, span, st),
        PatNode::Fast(inner, k) => time::query_fast(inner, k, false, p, span, st),
        PatNode::Slow(inner, k) => time::query_fast(inner, k, true, p, span, st),
        PatNode::Hurry(inner, k) => time::query_hurry(inner, k, p, span, st),
        PatNode::Rev(inner) => time::query_rev(inner, p, span, st),
        PatNode::Every(n, f, inner) => time::query_every(n, f, inner, p, span, st),
        PatNode::WhenMod(a, b, f, inner) => time::query_whenmod(a, b, f, inner, p, span, st),
        PatNode::Iter(inner, n) => time::query_iter(inner, n, p, span, st),
        PatNode::Chunk(inner, n, f) => time::query_chunk(inner, n, f, p, span, st),
        PatNode::Segment(inner, n) => time::query_segment(inner, n, p, span, st),
        PatNode::SometimesBy(x, f, inner) => random::query_sometimes(x, f, inner, p, span, st),
        PatNode::DegradeBy(inner, x) => random::query_degrade(inner, x, false, p, span, st),
        PatNode::Maybe(inner, x) => random::query_degrade(inner, x, true, p, span, st),
        PatNode::Choose(items) => random::query_choose(items, p, span, st),
        PatNode::Hold(inner, _) => query_single(inner, 1, p, span, st),
        PatNode::Repeat(inner, n) => structure::query_repeat(inner, n, p, span, st),
        PatNode::Stack(items) => structure::query_stack(items, p, span, st),
        PatNode::Cat(items) => structure::query_cat(items, p, span, st),
        PatNode::FastCat(items) => structure::query_fastcat(items, p, span, st),
        PatNode::Superimpose(inner, f) => structure::query_superimpose(inner, f, p, span, st),
        PatNode::Off(inner, t, f) => structure::query_off(inner, t, f, p, span, st),
        PatNode::Jux(inner, f) => structure::query_jux(inner, f, p, span, st),
        PatNode::Ply(inner, n) => structure::query_ply(inner, n, p, span, st),
        PatNode::Euclid(inner, k, n, r) => structure::query_euclid(inner, k, n, r, p, span, st),
        PatNode::Grid(inner, bools) => structure::query_grid(inner, bools, p, span, st),
        PatNode::Chop(inner, n) => region::query_chop(inner, n, p, span, st),
        PatNode::Striate(inner, n) => region::query_striate(inner, n, p, span, st),
        PatNode::Slice { pat, cuts, index } => {
            region::query_slice(pat, cuts, index, false, p, span, st)
        }
        PatNode::Splice { pat, cuts, index } => {
            region::query_slice(pat, cuts, index, true, p, span, st)
        }
        PatNode::LoopAt(inner, n) => region::query_loop_at(inner, n, p, span, st),
        PatNode::Fit(inner) => region::query_fit(inner, p, span, st),
        PatNode::Control(kw, value, subject) => {
            control::query_control(*kw, value, subject, p, span, st)
        }
        PatNode::ScaleNotes(root, name, inner) => {
            music::query_scale(*root, *name, inner, p, span, st)
        }
        PatNode::Chord(chords, subject) => music::query_chord(chords, subject, p, span, st),
        PatNode::Voicing(inner) => music::query_voicing(inner, p, span, st),
        PatNode::Arp(inner, mode) => music::query_arp(inner, mode, p, span, st),
        PatNode::Range(inner, lo, hi) => music::query_range(inner, lo, hi, p, span, st),
        PatNode::MidiNotes { .. } => input::query_midi_notes(),
    }
}

/// A signal over a span: one continuous fragment (no whole) valued at the
/// span start.
fn query_signal(sig: &Sig, p: &Pat, span: TimeSpan, st: &mut QState<'_, '_>) -> Vec<Event> {
    match sig.value_at(span.begin, st.cx) {
        Ok(v) => vec![Event::new(None, span, v, None)],
        Err(f) => {
            st.fault(f, p.span, Some(span.begin));
            Vec::new()
        }
    }
}
