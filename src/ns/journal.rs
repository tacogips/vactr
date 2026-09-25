//! The pass journal and slot snapshots (design 5.6 "PASS JOURNAL AND
//! PASS-LEVEL STAGING", "Whole-form transaction").
//!
//! A form transaction snapshots the slots it may write before it runs; an
//! unsuccessful evaluation rolls back exactly the slots it changed. Within a
//! reactive pass a successful commit is PROVISIONAL: the first in-pass write
//! of a slot journals its PRE-PASS value, version and owner, every staged
//! effect of the form is held at pass level (a re-run supersedes the earlier
//! round's), and pass validation either keeps the form or restores every slot
//! it wrote to the pre-pass state and drops its effects from every round.
//!
//! The pass driver (`Evaluator::propagate`) lives here with the journal it
//! validates: rounds over the `Schedule`, the per-form rebuild with its
//! deref-time gate outcomes, status events in both directions, round-end
//! stale reads, cycle stops, failure-sensitive validation and release.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::ns::depgraph::{Edge, FormId, FormState, Schedule, WriteItem};
use crate::ns::evaluator::{changed, one_shot, Abort, Evaluator, PassEvent, PassReport, Read};
use crate::ns::namespace::{FormGen, Namespace, SlotKind, VarSlotRef};
use crate::ns::stage::StagedEffect;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::intern::{name_of_sym, SymId};
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// The full state of one slot at a point in time.
#[derive(Clone, Debug)]
pub struct SlotSnap {
    pub slot: VarSlotRef,
    pub value: Value,
    pub version: u64,
    pub owner: Option<FormGen>,
    pub kind: SlotKind,
    pub bound: bool,
}

impl SlotSnap {
    /// Captures `slot` now.
    #[must_use]
    pub fn take(slot: &VarSlotRef) -> SlotSnap {
        SlotSnap {
            slot: slot.clone(),
            value: slot.get(),
            version: slot.version(),
            owner: slot.owner(),
            kind: slot.kind(),
            bound: slot.is_bound(),
        }
    }

    /// True when the slot was written since the snapshot.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.slot.version() != self.version
            || self.slot.is_bound() != self.bound
            || self.slot.owner() != self.owner
    }

    /// Puts the slot back: value, version, kind and owner. A slot that was
    /// unbound (a reserved forward reference) keeps its binding, with `nil`:
    /// the namespace has no unbind operation (recorded residual risk).
    pub fn restore(&self, ns: &Namespace) {
        if self.slot.kind() != self.kind || self.slot.owner() != self.owner {
            if let Some(gen) = self.owner {
                ns.define_slot(&self.slot, self.kind, self.value.clone(), gen);
            }
        }
        self.slot.restore(self.value.clone(), self.version);
    }
}

/// The slots one evaluation may write: every bound session slot plus the
/// form's definition targets.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    slots: Vec<SlotSnap>,
}

impl Snapshot {
    /// Captures every bound session slot of `ns` and each of `extra`.
    #[must_use]
    pub fn take(ns: &Namespace, extra: &[VarSlotRef]) -> Snapshot {
        let mut slots: Vec<SlotSnap> = ns
            .session_names()
            .into_iter()
            .filter_map(|n| ns.session_slot(n))
            .map(|s| SlotSnap::take(&s))
            .collect();
        for s in extra {
            if !slots.iter().any(|x| x.slot.same(s)) {
                slots.push(SlotSnap::take(s));
            }
        }
        Snapshot { slots }
    }

    /// The slots written since the snapshot, with their earlier state.
    #[must_use]
    pub fn changed(&self) -> Vec<&SlotSnap> {
        self.slots.iter().filter(|s| s.changed()).collect()
    }

    /// Undoes every write since the snapshot (an unsuccessful evaluation).
    pub fn rollback(&self, ns: &Namespace) {
        for s in self.slots.iter().filter(|s| s.changed()) {
            s.restore(ns);
        }
    }
}

/// A journal restore, for the pass trace.
#[derive(Clone, Debug)]
pub struct Restored {
    pub form: FormId,
    pub name: SymId,
    pub value: Value,
    pub version: u64,
    pub owner: Option<FormGen>,
}

/// The journal of one reactive pass.
#[derive(Debug, Default)]
pub struct PassJournal {
    /// Pre-pass state of every slot written in the pass (first write only).
    pre: BTreeMap<u64, SlotSnap>,
    /// Slots each form wrote in any round.
    written: BTreeMap<FormId, Vec<u64>>,
    /// Which form wrote each in-pass `(slot, version)`.
    writers: BTreeMap<(u64, u64), FormId>,
    /// Each form's staged effects from its latest committed round.
    staged: BTreeMap<FormId, Vec<StagedEffect>>,
    /// Forms in the order of their latest provisional commit.
    order: Vec<FormId>,
}

impl PassJournal {
    /// Records a provisional commit of `f`: the slots it changed since
    /// `snap` (journaling the pre-pass state of first writes) and its
    /// staged effects, which supersede any earlier round's.
    pub fn commit(&mut self, f: FormId, snap: &Snapshot, effects: Vec<StagedEffect>) {
        for s in snap.changed() {
            let id = s.slot.id();
            self.pre.entry(id).or_insert_with(|| s.clone());
            let w = self.written.entry(f).or_default();
            if !w.contains(&id) {
                w.push(id);
            }
            self.writers.insert((id, s.slot.version()), f);
        }
        self.staged.insert(f, effects);
        self.order.retain(|g| *g != f);
        self.order.push(f);
    }

    /// The form that wrote `slot` at `version` in this pass, if any.
    #[must_use]
    pub fn writer(&self, slot: u64, version: u64) -> Option<FormId> {
        self.writers.get(&(slot, version)).copied()
    }

    /// Discards `f` (an invalid form): restores every slot it wrote in any
    /// round to the pre-pass state and drops its staged effects.
    pub fn discard(&mut self, f: FormId, ns: &Namespace) -> Vec<Restored> {
        self.staged.remove(&f);
        self.order.retain(|g| *g != f);
        let mut out = Vec::new();
        for id in self.written.remove(&f).unwrap_or_default() {
            if let Some(s) = self.pre.get(&id) {
                s.restore(ns);
                out.push(Restored {
                    form: f,
                    name: s.slot.name(),
                    value: s.value.clone(),
                    version: s.version,
                    owner: s.owner,
                });
            }
        }
        out
    }

    /// The slots `f` wrote in this pass.
    #[must_use]
    pub fn written_by(&self, f: FormId) -> Vec<u64> {
        self.written.get(&f).cloned().unwrap_or_default()
    }

    /// The pre-pass value of `slot`, when the pass wrote it.
    #[must_use]
    pub fn pre_value(&self, slot: u64) -> Option<&Value> {
        self.pre.get(&slot).map(|s| &s.value)
    }

    /// Takes the effects of the remaining (valid) forms in commit order.
    pub fn take_effects(&mut self) -> Vec<(FormId, Vec<StagedEffect>)> {
        let order = std::mem::take(&mut self.order);
        order
            .into_iter()
            .filter_map(|f| self.staged.remove(&f).map(|e| (f, e)))
            .collect()
    }
}

/// The state of one reactive pass.
#[derive(Default)]
struct Pass {
    sched: Schedule,
    states: BTreeMap<FormId, FormState>,
    /// Each form's in-pass reads from its latest committed attempt.
    reads: BTreeMap<FormId, Vec<Read>>,
    /// Each committed form's new write set (applied when valid).
    writes: BTreeMap<FormId, Vec<WriteItem>>,
    /// The generation each rebuilt form evaluates under in this pass.
    gens: BTreeMap<FormId, FormGen>,
    /// Forms stopped by a cycle: never re-scheduled in this pass.
    stopped: BTreeSet<FormId>,
    journal: PassJournal,
    report: PassReport,
}

fn cycle_failure(names: &str) -> Failure {
    Failure::new(
        FailCode::Blocked,
        format!("dependency cycle through {names}"),
    )
}

impl Evaluator {
    /// Runs the reactive pass for `triggers` (written slots), `status`
    /// (slots whose owner recovered: a status event bypassing the equality
    /// cutoff) and `forced` (owners of a `Reeval` tweak write), then
    /// releases `lead` followed by the validated effects and ONE `bindings`
    /// batch.
    pub(super) fn propagate(
        &mut self,
        triggers: &[VarSlotRef],
        status: &[u64],
        forced: &[FormId],
        lead: Vec<StagedEffect>,
        names: Vec<SymId>,
    ) -> PassReport {
        let mut p = Pass::default();
        for s in triggers {
            for g in self.graph.dependents(s.id()) {
                self.mark(g, &mut p);
            }
        }
        for id in status {
            for g in self.graph.status_dependents(*id) {
                self.mark(g, &mut p);
            }
        }
        for f in forced {
            self.mark(*f, &mut p);
        }
        loop {
            if p.sched.round_empty() {
                self.stale_check(&mut p);
                match p.sched.next_round() {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(forms) => {
                        self.cycle(&forms, &mut p);
                        let mut rest = p.sched.queued();
                        if p.sched.next_round().is_err() || !rest.is_empty() {
                            rest.extend(p.sched.queued());
                            self.cycle(&rest, &mut p);
                        }
                        break;
                    }
                }
            }
            match p.sched.pick(&self.graph) {
                Some(f) => self.rebuild(f, &mut p),
                None => {
                    let q = p.sched.queued();
                    self.cycle(&q, &mut p);
                }
            }
        }
        self.validate(&mut p);
        let mut out = lead;
        let mut refreshed = Vec::new();
        for (f, effects) in p.journal.take_effects() {
            out.extend(effects);
            if let Some(gen) = self.graph.get(f).map(|r| r.gen) {
                refreshed.extend(
                    self.ns
                        .tweaks()
                        .borrow()
                        .sites_of(gen)
                        .into_iter()
                        .map(|s| s.id),
                );
            }
        }
        out.extend(refreshed.into_iter().map(StagedEffect::TweakRefresh));
        let mut all = names;
        for f in p.states.keys() {
            if let Some(r) = self.graph.get(*f) {
                all.extend(r.names().map(VarSlotRef::name));
            }
        }
        let mut report = std::mem::take(&mut p.report);
        report.bindings = self.release(out, &all);
        report.states = p.states.into_iter().collect();
        report
    }

    fn mark(&self, g: FormId, p: &mut Pass) {
        if !p.stopped.contains(&g) && self.graph.eligible(g) {
            p.sched.mark(g);
        }
    }

    fn is_bad(&self, f: FormId, p: &Pass) -> bool {
        match p.states.get(&f) {
            Some(s) => s.is_bad(),
            None => self.graph.get(f).is_some_and(|r| r.state.is_bad()),
        }
    }

    fn bad_set(&self, p: &Pass) -> BTreeSet<FormId> {
        let mut bad: BTreeSet<FormId> = self
            .durable_bad()
            .into_iter()
            .filter(|f| !p.states.contains_key(f))
            .collect();
        bad.extend(p.states.iter().filter(|(_, s)| s.is_bad()).map(|(f, _)| *f));
        bad
    }

    /// One scheduled form's evaluation (a rebuild under a fresh generation).
    fn rebuild(&mut self, f: FormId, p: &mut Pass) {
        let Some((node, old_gen)) = self.graph.get(f).map(|r| (Rc::clone(&r.node), r.gen)) else {
            return;
        };
        let own = self.graph.owned_slots(f);
        let was_bad = self.is_bad(f, p);
        let gen = match p.gens.get(&f) {
            Some(g) => *g,
            None => {
                let g = self.next_gen();
                p.gens.insert(f, g);
                g
            }
        };
        self.ns.tweaks().borrow_mut().retire(gen);
        let (dirty, bad) = (p.sched.pending(), self.bad_set(p));
        let a = self.attempt(&node, gen, Some(old_gen), &own, dirty, bad);
        if let Some(r) = self.graph.get_mut(f) {
            r.runs += 1;
        }
        p.report.diags.extend(a.diags);
        let attempted: Vec<VarSlotRef> = a.reads.iter().map(|r| r.slot.clone()).collect();
        match (a.abort, a.value) {
            (Some(Abort::Dirty(owner)), _) => {
                p.report
                    .events
                    .push(PassEvent::DirtyAbort { form: f, owner });
                if let Some(members) = p.sched.wait_for(f, owner) {
                    self.cycle(&members, p);
                }
            }
            (Some(Abort::Blocked(slot)), _) => {
                p.report.events.push(PassEvent::BlockedAbort {
                    form: f,
                    on: slot.name(),
                });
                if let Some(r) = self.graph.get_mut(f) {
                    r.attempt = attempted;
                    r.subs = vec![slot.clone()];
                }
                self.settle_bad(f, FormState::Blocked { on: slot.name() }, was_bad, p);
            }
            (None, Err(e)) => self.fail(f, e, attempted, was_bad, p),
            (None, Ok(_)) => {
                let writes = a.snap.changed();
                let foreign = writes.iter().any(|s| s.slot.owner() != Some(gen));
                if foreign || a.effects.iter().any(one_shot) {
                    a.snap.rollback(&self.ns);
                    let e = Failure::new(
                        FailCode::EffectInRebuild,
                        "a one-shot effect or a global `upd` cannot replay in a rebuild",
                    );
                    return self.fail(f, e, attempted, was_bad, p);
                }
                let mut members: Vec<WriteItem> = writes
                    .iter()
                    .map(|s| WriteItem::Name(s.slot.clone()))
                    .collect();
                for e in &a.effects {
                    if let StagedEffect::SlotBind { slot, .. } = e {
                        members.push(WriteItem::Bind(*slot));
                    }
                }
                let steals = members.iter().any(|m| {
                    let o = match m {
                        WriteItem::Name(s) => self.graph.owner_of_slot(s.id()),
                        WriteItem::Bind(k) => self.graph.owner_of_bind(*k),
                    };
                    o.is_some_and(|o| o != f)
                });
                if steals {
                    a.snap.rollback(&self.ns);
                    let e = Failure::new(
                        FailCode::EffectInRebuild,
                        "the rebuild would take over an artifact another form owns",
                    );
                    return self.fail(f, e, attempted, was_bad, p);
                }
                let defined: BTreeSet<u64> = writes.iter().map(|s| s.slot.id()).collect();
                let values = writes
                    .iter()
                    .map(|s| (s.slot.name(), s.slot.get()))
                    .collect();
                let moved: Vec<u64> = writes
                    .iter()
                    .filter(|s| changed(&s.value, &s.slot.get()))
                    .map(|s| s.slot.id())
                    .collect();
                p.journal.commit(f, &a.snap, a.effects);
                let edges = a
                    .reads
                    .iter()
                    .filter(|r| !defined.contains(&r.slot.id()))
                    .map(|r| Edge {
                        slot: r.slot.clone(),
                        version: r.version,
                    })
                    .collect();
                if let Some(r) = self.graph.get_mut(f) {
                    r.commit_edges(edges);
                }
                p.reads.insert(f, a.reads);
                p.writes.insert(f, members);
                p.states.insert(f, FormState::Recomputed);
                p.sched.done(f);
                p.report.events.push(PassEvent::Commit { form: f, values });
                for id in moved {
                    for g in self.graph.dependents(id) {
                        if g != f {
                            self.mark(g, p);
                        }
                    }
                }
                if was_bad {
                    // Recovery status event: bypasses the equality cutoff.
                    for id in own.iter().chain(defined.iter()) {
                        for g in self.graph.status_dependents(*id) {
                            if g != f {
                                self.mark(g, p);
                            }
                        }
                    }
                }
            }
        }
    }

    fn fail(
        &mut self,
        f: FormId,
        e: Failure,
        attempted: Vec<VarSlotRef>,
        was_bad: bool,
        p: &mut Pass,
    ) {
        p.report.events.push(PassEvent::Failed {
            form: f,
            code: e.code,
        });
        if let Some(r) = self.graph.get_mut(f) {
            r.attempt = attempted;
            r.subs.clear();
        }
        self.settle_bad(f, FormState::Failed(e), was_bad, p);
    }

    /// A form ended its attempt `Failed`/`Blocked`: the failure-direction
    /// status event. In-pass readers of its slots re-run (their retry hits
    /// the deref gate); on a transition INTO failure, dependents over the
    /// committed edges are dirty-marked too.
    fn settle_bad(&mut self, f: FormId, state: FormState, was_bad: bool, p: &mut Pass) {
        p.states.insert(f, state);
        p.sched.done(f);
        let owned = self.graph.owned_slots(f);
        let readers: Vec<FormId> = p
            .reads
            .iter()
            .filter(|(g, rs)| {
                **g != f
                    && matches!(p.states.get(g), Some(FormState::Recomputed))
                    && rs.iter().any(|r| r.owner == Some(f))
            })
            .map(|(g, _)| *g)
            .collect();
        for g in readers {
            self.mark(g, p);
        }
        if !was_bad {
            for id in owned {
                for g in self.graph.status_dependents(id) {
                    if g != f {
                        self.mark(g, p);
                    }
                }
            }
        }
    }

    /// Round end: a form whose recorded read version is older than the
    /// slot's current in-pass version is stale and re-runs next round.
    fn stale_check(&self, p: &mut Pass) {
        for f in p.sched.ran_forms() {
            if !matches!(p.states.get(&f), Some(FormState::Recomputed)) {
                continue;
            }
            let stale = p.reads.get(&f).is_some_and(|rs| {
                rs.iter()
                    .any(|r| r.owner != Some(f) && r.slot.version() != r.version)
            });
            if stale {
                p.report.events.push(PassEvent::Stale { form: f });
                p.sched.to_next(f);
            }
        }
    }

    /// Stops a genuine cycle: its members fail with `dependency-cycle` and
    /// keep their previous committed values.
    fn cycle(&mut self, forms: &[FormId], p: &mut Pass) {
        let forms: Vec<FormId> = forms
            .iter()
            .copied()
            .filter(|f| !p.stopped.contains(f))
            .collect();
        if forms.is_empty() {
            return;
        }
        let names: Vec<String> = forms
            .iter()
            .filter_map(|f| self.graph.get(*f))
            .flat_map(|r| {
                r.names()
                    .map(|s| name_of_sym(s.name()).to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        let names = names.join(", ");
        if let Some(r) = forms.first().and_then(|f| self.graph.get(*f)) {
            p.report.diags.push(Diagnostic::error(
                DiagCode::DependencyCycle,
                r.node.span,
                format!("dependency cycle through {names}; the previous values stand"),
            ));
        }
        p.report.events.push(PassEvent::Cycle {
            forms: forms.clone(),
        });
        for f in &forms {
            p.stopped.insert(*f);
            p.sched.drop_form(*f);
        }
        for f in forms {
            let was_bad = self.is_bad(f, p);
            self.settle_bad(f, FormState::Failed(cycle_failure(&names)), was_bad, p);
        }
    }

    /// Pass-end validation (failure-sensitive, to fixpoint) and rollback.
    fn validate(&mut self, p: &mut Pass) {
        let mut invalid: BTreeSet<FormId> = p
            .states
            .iter()
            .filter(|(_, s)| s.is_bad())
            .map(|(f, _)| *f)
            .collect();
        loop {
            let mut grew = false;
            let recomputed: Vec<FormId> = p
                .states
                .iter()
                .filter(|(f, s)| matches!(s, FormState::Recomputed) && !invalid.contains(f))
                .map(|(f, _)| *f)
                .collect();
            for f in recomputed {
                let hit = p.reads.get(&f).and_then(|rs| {
                    rs.iter().find(|r| {
                        let owner_bad = r.owner.filter(|o| *o != f).is_some_and(|o| {
                            invalid.contains(&o)
                                || (!p.states.contains_key(&o) && self.is_bad(o, p))
                        });
                        let writer_bad = p
                            .journal
                            .writer(r.slot.id(), r.version)
                            .is_some_and(|w| w != f && invalid.contains(&w));
                        owner_bad || writer_bad
                    })
                });
                if let Some(r) = hit {
                    let slot = r.slot.clone();
                    p.states.insert(f, FormState::Blocked { on: slot.name() });
                    if let Some(rec) = self.graph.get_mut(f) {
                        rec.subs = vec![slot];
                    }
                    invalid.insert(f);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        for f in &invalid {
            for r in p.journal.discard(*f, &self.ns) {
                p.report.events.push(PassEvent::Restore(r));
            }
            if let Some(gen) = p.gens.get(f) {
                self.ns.tweaks().borrow_mut().retire(*gen);
            }
        }
        for (f, state) in &p.states {
            let valid = !invalid.contains(f);
            let new_gen = p.gens.get(f).copied();
            let writes = p.writes.remove(f);
            let Some(r) = self.graph.get_mut(*f) else {
                continue;
            };
            r.state = state.clone();
            if valid {
                if let (Some(gen), Some(w)) = (new_gen, writes) {
                    let old = std::mem::replace(&mut r.gen, gen);
                    r.writes = w;
                    self.ns.tweaks().borrow_mut().retire(old);
                    for g in self.graph.claim(*f) {
                        self.refresh_tiers(g);
                    }
                }
            }
        }
    }
}
