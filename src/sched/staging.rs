//! Staging and the occurrence merge (design 11.3 layer 2, 12.8.3).
//!
//! Staging holds ONE record per occurrence (`OccKey`) per lane, i.e. per
//! (slot, generation). A record keeps its payload (the event), its `whole`
//! and its COVERED EXTENT: the union of the query fragments seen so far.
//! Ordinary query results only EXTEND coverage (never delete, never
//! replace), so a continuation can never erase an uncommitted onset. Only
//! scoped semantic invalidation removes coverage, drops records whose onset
//! lies in the span and whose key the re-query no longer returns, and
//! REPLACES the payload of a surviving in-span record wholesale. Captured
//! query output is held per query fragment and forwarded at commit (10.4).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::clock::tempo::Tempo;
use crate::pattern::eval::{InputCells, QueryCtx, QueryVm};
use crate::pattern::occ::OccKey;
use crate::pattern::pat::Pat;
use crate::pattern::query::query;
use crate::pattern::query::{sect, Event, TimeSpan};
use crate::sched::ledger::Ledger;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::Failure;

/// A coalesced, sorted list of disjoint spans inside one `whole`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Coverage(Vec<TimeSpan>);

impl Coverage {
    /// The spans, sorted and disjoint.
    #[must_use]
    pub fn spans(&self) -> &[TimeSpan] {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Unions `s` in (adjacent spans coalesce).
    pub fn add(&mut self, s: TimeSpan) {
        if s.begin >= s.end {
            return;
        }
        let mut merged = s;
        let mut out = Vec::with_capacity(self.0.len() + 1);
        for c in self.0.drain(..) {
            if c.end < merged.begin || c.begin > merged.end {
                out.push(c);
            } else {
                merged.begin = merged.begin.min(c.begin);
                merged.end = merged.end.max(c.end);
            }
        }
        out.push(merged);
        out.sort();
        self.0 = out;
    }

    /// Removes everything inside `s`.
    pub fn remove(&mut self, s: TimeSpan) {
        let mut out = Vec::with_capacity(self.0.len() + 1);
        for c in self.0.drain(..) {
            if c.end <= s.begin || c.begin >= s.end {
                out.push(c);
                continue;
            }
            if c.begin < s.begin {
                out.push(TimeSpan {
                    begin: c.begin,
                    end: s.begin,
                });
            }
            if c.end > s.end {
                out.push(TimeSpan {
                    begin: s.end,
                    end: c.end,
                });
            }
        }
        self.0 = out;
    }

    /// Keeps only what lies inside `whole`.
    pub fn intersect(&mut self, whole: TimeSpan) {
        self.0 = self.0.iter().filter_map(|c| sect(*c, whole)).collect();
    }

    /// True when `t` is covered.
    #[must_use]
    pub fn contains(&self, t: Ratio64) -> bool {
        self.0.iter().any(|c| c.begin <= t && t < c.end)
    }
}

/// One occurrence in staging (design 11.3).
#[derive(Clone, Debug)]
pub struct OccRecord {
    pub key: OccKey,
    pub whole: TimeSpan,
    /// The event of the newest snapshot covering the onset.
    pub payload: Event,
    pub covered: Coverage,
    /// Set once the onset was emitted: the record never emits again.
    pub onset_committed: bool,
}

impl OccRecord {
    /// True when the covered extent includes the onset.
    #[must_use]
    pub fn onset_bearing(&self) -> bool {
        self.covered.contains(self.whole.begin)
    }
}

/// Captured `print` output of one query fragment, held until commit
/// (10.4, 12.8.3). The output belongs to the occurrences whose ONSET the
/// fragment carried: it is forwarded when the first of them commits. A
/// fragment that only continued already-known occurrences re-evaluated
/// work that has no event of its own, so its output is discarded with its
/// keys (one print per event, never one per query window).
#[derive(Clone, Debug)]
pub struct Fragment {
    pub span: TimeSpan,
    /// The occurrences the fragment produced.
    pub keys: Vec<OccKey>,
    /// The occurrences whose onset the fragment carried.
    pub onsets: Vec<OccKey>,
    pub output: Vec<Rc<str>>,
}

/// An invalidated span waiting for its re-query: `query` is what is
/// re-queried, `scope` what the payload replacement and removal rules
/// apply to (`query` covers `scope` plus any discarded fragment around it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dirty {
    pub query: TimeSpan,
    pub scope: TimeSpan,
}

/// The records and held output of one lane.
#[derive(Clone, Debug, Default)]
pub struct Staging {
    records: BTreeMap<OccKey, OccRecord>,
    fragments: Vec<Fragment>,
}

impl Staging {
    /// The records, by key.
    #[must_use]
    pub fn records(&self) -> &BTreeMap<OccKey, OccRecord> {
        &self.records
    }

    /// A record by key.
    #[must_use]
    pub fn get(&self, key: &OccKey) -> Option<&OccRecord> {
        self.records.get(key)
    }

    /// The held fragments.
    #[must_use]
    pub fn fragments(&self) -> &[Fragment] {
        &self.fragments
    }

    /// True when nothing is staged or held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.fragments.is_empty()
    }

    /// Coverage extension: every event with a `whole` unions its `part`
    /// into its occurrence's record, creating the record on first sight.
    /// A key already in the ledger never becomes an emittable record again.
    /// Events without a `whole` (signal fragments) are not occurrences.
    pub fn merge_fragment(
        &mut self,
        span: TimeSpan,
        events: Vec<Event>,
        output: Vec<Rc<str>>,
        ledger: &Ledger,
    ) {
        let mut keys = Vec::new();
        let mut onsets = Vec::new();
        for e in events {
            let Some(whole) = e.whole else {
                continue;
            };
            keys.push(e.occ.clone());
            if e.is_onset() {
                onsets.push(e.occ.clone());
            }
            let part = sect(whole, e.part).unwrap_or(e.part);
            match self.records.get_mut(&e.occ) {
                Some(r) => r.covered.add(part),
                None => {
                    let mut covered = Coverage::default();
                    covered.add(part);
                    self.records.insert(
                        e.occ.clone(),
                        OccRecord {
                            key: e.occ.clone(),
                            whole,
                            onset_committed: ledger.contains(&e.occ),
                            payload: e,
                            covered,
                        },
                    );
                }
            }
        }
        if !output.is_empty() {
            self.fragments.push(Fragment {
                span,
                keys,
                onsets,
                output,
            });
        }
    }

    /// Semantic invalidation, first half (design 11.3): discards coverage
    /// inside `scope` and every held fragment touching it. Returns the span
    /// to re-query: `scope` widened to the discarded fragments, so their
    /// output is captured afresh by the re-query.
    pub fn invalidate(&mut self, scope: TimeSpan) -> Dirty {
        for r in self.records.values_mut() {
            if !r.onset_committed {
                r.covered.remove(scope);
            }
        }
        let mut query = scope;
        self.fragments.retain(|f| {
            let touches = f.span.begin < scope.end && scope.begin < f.span.end;
            if touches {
                query.begin = query.begin.min(f.span.begin);
                query.end = query.end.max(f.span.end);
            }
            !touches
        });
        Dirty { query, scope }
    }

    /// Semantic invalidation, second half: the re-query result of a dirty
    /// span. An uncommitted record whose onset lies in `scope` takes the new
    /// snapshot's payload when its key is present and is dropped when it is
    /// absent; everything else is only re-extended.
    pub fn replace(&mut self, d: Dirty, events: Vec<Event>, output: Vec<Rc<str>>, ledger: &Ledger) {
        let mut fresh: BTreeMap<&OccKey, &Event> = BTreeMap::new();
        for e in &events {
            if e.whole.is_some() {
                fresh.entry(&e.occ).or_insert(e);
            }
        }
        let in_scope = |t: Ratio64| d.scope.begin <= t && t < d.scope.end;
        let mut dropped = Vec::new();
        for (k, r) in &mut self.records {
            if r.onset_committed || !in_scope(r.whole.begin) {
                continue;
            }
            match fresh.get(k) {
                Some(e) => {
                    let whole = e.whole.unwrap_or(r.whole);
                    r.payload = (*e).clone();
                    r.whole = whole;
                    r.covered.intersect(whole);
                }
                None => dropped.push(k.clone()),
            }
        }
        for k in &dropped {
            self.records.remove(k);
            self.forget_key(k);
        }
        self.merge_fragment(d.query, events, output, ledger);
    }

    /// Discards everything at or after `t` that is not committed (a
    /// rebind's boundary): records whose onset is at or after `t` and the
    /// coverage beyond it.
    pub fn truncate_from(&mut self, t: Ratio64) {
        let far = TimeSpan {
            begin: t,
            end: Ratio64::from_int(i64::MAX / 4),
        };
        let mut dropped = Vec::new();
        for (k, r) in &mut self.records {
            if r.onset_committed {
                continue;
            }
            if r.whole.begin >= t {
                dropped.push(k.clone());
            } else {
                r.covered.remove(far);
            }
        }
        for k in &dropped {
            self.records.remove(k);
            self.forget_key(k);
        }
        self.fragments.retain(|f| f.span.begin < t);
    }

    /// The uncommitted onset-bearing records whose onset lies in
    /// `[from, until)`, in onset order.
    #[must_use]
    pub fn emittable(&self, from: Ratio64, until: Option<Ratio64>) -> Vec<OccKey> {
        let mut out: Vec<(Ratio64, OccKey)> = self
            .records
            .values()
            .filter(|r| {
                !r.onset_committed
                    && r.onset_bearing()
                    && r.whole.begin >= from
                    && until.is_none_or(|u| r.whole.begin < u)
            })
            .map(|r| (r.whole.begin, r.key.clone()))
            .collect();
        out.sort();
        out.into_iter().map(|(_, k)| k).collect()
    }

    /// Marks a record emitted and returns the held output of the fragments
    /// that carried its onset (forwarded now, 10.4).
    pub fn mark_committed(&mut self, key: &OccKey) -> Vec<Rc<str>> {
        if let Some(r) = self.records.get_mut(key) {
            r.onset_committed = true;
        }
        let mut out = Vec::new();
        self.fragments.retain(|f| {
            if f.onsets.contains(key) {
                out.extend(f.output.iter().cloned());
                false
            } else {
                true
            }
        });
        self.forget_key(key);
        out
    }

    /// True when output is held for the onset of `key`.
    #[must_use]
    pub fn holds_onset(&self, key: &OccKey) -> bool {
        self.fragments.iter().any(|f| f.onsets.contains(key))
    }

    /// Holds `output` for the onsets `keys` (nothing when empty).
    pub fn hold(&mut self, span: TimeSpan, keys: Vec<OccKey>, output: Vec<Rc<str>>) {
        if output.is_empty() {
            return;
        }
        self.fragments.push(Fragment {
            span,
            onsets: keys.clone(),
            keys,
            output,
        });
    }

    /// Drops a record whose commit failed, with its held output (10.3).
    pub fn drop_failed(&mut self, key: &OccKey) {
        self.records.remove(key);
        self.forget_key(key);
    }

    /// Output of fragments that produced no occurrence, once their end is
    /// inside the commit horizon.
    pub fn take_keyless(&mut self, horizon: Ratio64) -> Vec<Rc<str>> {
        let mut out = Vec::new();
        self.fragments.retain(|f| {
            if f.keys.is_empty() && f.span.end <= horizon {
                out.extend(f.output.iter().cloned());
                false
            } else {
                true
            }
        });
        out
    }

    /// Drops records whose whole ended at or before `pos` (their onset was
    /// emitted or can no longer be).
    pub fn expire(&mut self, pos: Ratio64) {
        let old: Vec<OccKey> = self
            .records
            .values()
            .filter(|r| r.whole.end <= pos && (r.onset_committed || r.whole.begin < pos))
            .map(|r| r.key.clone())
            .collect();
        for k in &old {
            self.records.remove(k);
            self.forget_key(k);
        }
    }

    /// Removes a key from the held fragments; a fragment left with no key
    /// is discarded with its output (its work was removed).
    fn forget_key(&mut self, key: &OccKey) {
        self.fragments.retain_mut(|f| {
            let had = !f.keys.is_empty();
            f.keys.retain(|k| k != key);
            f.onsets.retain(|k| k != key);
            !(had && (f.keys.is_empty() || f.onsets.is_empty()))
        });
    }
}

/// The staged work of one (slot, generation): its binding over
/// `[from, until)`, shifted by `offset` (a `once` plays cycle 0 of its
/// pattern from its start).
#[derive(Clone, Debug)]
pub struct Lane {
    pub gen: u32,
    pub pat: Rc<Pat>,
    pub from: Ratio64,
    pub until: Option<Ratio64>,
    pub offset: Ratio64,
    /// Everything before this position has been queried.
    pub queried_to: Ratio64,
    pub staging: Staging,
    pub ledger: Ledger,
    pub dirty: Vec<Dirty>,
    /// `once` control overrides, applied at commit.
    pub overrides: Vec<(KwId, Value)>,
}

impl Lane {
    /// A lane playing `pat` from `from` on.
    #[must_use]
    pub fn new(gen: u32, pat: Rc<Pat>, from: Ratio64) -> Self {
        Self {
            gen,
            pat,
            from,
            until: None,
            offset: Ratio64::ZERO,
            queried_to: from,
            staging: Staging::default(),
            ledger: Ledger::default(),
            dirty: Vec::new(),
            overrides: Vec::new(),
        }
    }

    /// Clips `s` to the lane's extent.
    #[must_use]
    pub fn clip(&self, s: TimeSpan) -> Option<TimeSpan> {
        let begin = s.begin.max(self.from);
        let end = match self.until {
            Some(u) => s.end.min(u),
            None => s.end,
        };
        (begin < end).then_some(TimeSpan { begin, end })
    }

    /// Scoped invalidation of `span` (queued for the next re-query).
    pub fn invalidate(&mut self, span: TimeSpan) {
        let Some(span) = self.clip(span) else {
            return;
        };
        let d = self.staging.invalidate(span);
        self.dirty.push(d);
    }
}

/// Queries one lane: its invalidated spans first (payload replacement),
/// then the newly uncovered span up to `target` (coverage extension), each
/// split at cycle boundaries of the lane's pattern time.
pub(crate) fn query_lane(
    lane: &mut Lane,
    target: Ratio64,
    vm: &mut dyn QueryVm,
    input: &InputCells,
    seed: u64,
    tempo: Tempo,
    faults: &mut Vec<Failure>,
) {
    let cx = Probe { input, seed, tempo };
    for d in std::mem::take(&mut lane.dirty) {
        let mut events = Vec::new();
        let mut output = Vec::new();
        let mut probes = Vec::new();
        for (evs, out) in query_span(lane, d.query, vm, input, seed, tempo, faults) {
            probes.extend(probe_onsets(&evs, &out));
            output.extend(keyless(&evs, out));
            events.extend(evs);
        }
        lane.staging.replace(d, events, output, &lane.ledger);
        attach_onset_output(lane, probes, vm, &cx);
    }
    let start = lane.queried_to.max(lane.from);
    let end = lane.until.map_or(target, |u| target.min(u));
    if start >= end {
        return;
    }
    extend(lane, TimeSpan { begin: start, end }, vm, &cx, faults);
    lane.queried_to = end;
}

/// What a probe query reads.
pub(crate) struct Probe<'a> {
    pub input: &'a InputCells,
    pub seed: u64,
    pub tempo: Tempo,
}

/// Coverage extension of `span`: one fragment per cycle piece.
pub(crate) fn extend(
    lane: &mut Lane,
    span: TimeSpan,
    vm: &mut dyn QueryVm,
    cx: &Probe<'_>,
    faults: &mut Vec<Failure>,
) {
    let mut probes = Vec::new();
    for (evs, out) in query_span(lane, span, vm, cx.input, cx.seed, cx.tempo, faults) {
        probes.extend(probe_onsets(&evs, &out));
        let out = keyless(&evs, out);
        lane.staging.merge_fragment(span, evs, out, &lane.ledger);
    }
    attach_onset_output(lane, probes, vm, cx);
}

/// A window's output is kept as is only when the window produced no
/// occurrence (a print in a structural closure): it is forwarded when the
/// window passes the commit horizon. Otherwise it may hold re-evaluations
/// of continuing events, and each new onset gets its own output instead.
fn keyless(evs: &[Event], out: Vec<Rc<str>>) -> Vec<Rc<str>> {
    if evs.iter().any(|e| e.whole.is_some()) {
        Vec::new()
    } else {
        out
    }
}

/// The onsets of a window that printed.
fn probe_onsets(evs: &[Event], out: &[Rc<str>]) -> Vec<Ratio64> {
    if out.is_empty() {
        return Vec::new();
    }
    evs.iter()
        .filter(|e| e.is_onset())
        .map(Event::anchor)
        .collect()
}

/// Captured output per event (10.4: held keyed by the event's identity):
/// for each onset `t` of a new, uncommitted, onset-bearing record without
/// held output, a point query at `t` evaluates the events sounding at `t`
/// and its output is held under the keys starting there.
fn attach_onset_output(
    lane: &mut Lane,
    mut onsets: Vec<Ratio64>,
    vm: &mut dyn QueryVm,
    cx: &Probe<'_>,
) {
    onsets.sort();
    onsets.dedup();
    for t in onsets {
        let keys: Vec<OccKey> = lane
            .staging
            .records()
            .values()
            .filter(|r| {
                r.whole.begin == t
                    && !r.onset_committed
                    && r.onset_bearing()
                    && !lane.staging.holds_onset(&r.key)
            })
            .map(|r| r.key.clone())
            .collect();
        if keys.is_empty() {
            continue;
        }
        let Ok(inner) = t.checked_sub(lane.offset) else {
            continue;
        };
        let mut qcx = QueryCtx::new(vm, cx.input, cx.seed);
        qcx.tempo = cx.tempo;
        let r = query(&lane.pat, TimeSpan::point(inner), &mut qcx);
        let output: Vec<Rc<str>> = r.output.into_iter().map(|(_, s)| s).collect();
        lane.staging.hold(TimeSpan::point(t), keys, output);
    }
}

/// The events and captured output of `span` (lane time), one entry per
/// cycle piece of the pattern time.
pub(crate) fn query_span(
    lane: &Lane,
    span: TimeSpan,
    vm: &mut dyn QueryVm,
    input: &InputCells,
    seed: u64,
    tempo: Tempo,
    faults: &mut Vec<Failure>,
) -> Vec<(Vec<Event>, Vec<Rc<str>>)> {
    let mut out = Vec::new();
    let Some(span) = lane.clip(span) else {
        return out;
    };
    let shift = |t: Ratio64| t.checked_sub(lane.offset);
    let Ok(inner) = span.map(shift) else {
        return out;
    };
    let mut begin = inner.begin;
    while begin < inner.end {
        let next = Ratio64::from_int(begin.floor().saturating_add(1)).min(inner.end);
        let piece = TimeSpan { begin, end: next };
        let mut cx = QueryCtx::new(vm, input, seed);
        cx.tempo = tempo;
        let r = query(&lane.pat, piece, &mut cx);
        faults.extend(r.faults);
        let mut evs = Vec::with_capacity(r.events.len());
        for e in r.events {
            match e.shifted(lane.offset) {
                Ok(mut e) => {
                    let a = e.anchor();
                    e.occ.anchor_at(a);
                    evs.push(e);
                }
                Err(f) => faults.push(f),
            }
        }
        out.push((evs, r.output.into_iter().map(|(_, s)| s).collect()));
        begin = next;
    }
    out
}
