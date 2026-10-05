//! The reactive dependency graph (design 5.6 revised).
//!
//! One `FormRec` per recorded top-level form: a form that defines at least
//! one name or binds at least one slot. It keeps the form's expanded source
//! (a rebuild re-evaluates it exactly as re-typing the line would), its
//! COMMITTED edge set (the eager reads of its last successful evaluation,
//! with the version read), and the failure metadata of an unsuccessful
//! attempt: the ATTEMPT edge set (every slot it read successfully before
//! failing) and the RECOVERY subscriptions (every `Failed`/`Blocked` slot it
//! dereferenced). The active-owner registry maps every defined name and every
//! bound slot to the form that currently owns it; a form is eligible for an
//! automatic rebuild only while it owns its whole write set and performed no
//! one-shot effect.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::ns::namespace::{FormGen, VarSlotRef};
use crate::ns::stage::SlotKey;
use crate::reader::node::Node;
use crate::value::intern::SymId;
use crate::vm::fail::Failure;

id_newtype!(
    /// A recorded top-level form, numbered in definition (evaluation) order.
    FormId(u32)
);

/// A form's pass state (5.6 joins). `Failed` and `Blocked` are sticky
/// across passes until the form next commits.
#[derive(Clone, Debug, PartialEq)]
pub enum FormState {
    /// Untouched by the last pass that could have changed it.
    Clean,
    /// Recomputed and validated in the most recent pass that touched it.
    Recomputed,
    /// Its own evaluation failed (the unwind origin is in the failure).
    Failed(Failure),
    /// It dereferenced a `Failed`/`Blocked` owner's slot (`blocked-on`).
    Blocked { on: SymId },
}

impl FormState {
    /// True for `Failed` and `Blocked`.
    #[must_use]
    pub fn is_bad(&self) -> bool {
        matches!(self, FormState::Failed(_) | FormState::Blocked { .. })
    }
}

/// One member of a form's write set.
#[derive(Clone, Debug)]
pub enum WriteItem {
    /// A top-level name the form defines.
    Name(VarSlotRef),
    /// A playing slot the form binds.
    Bind(SlotKey),
}

/// An eager read: the slot and the version the evaluation consumed.
#[derive(Clone, Debug)]
pub struct Edge {
    pub slot: VarSlotRef,
    pub version: u64,
}

/// A recorded top-level form.
#[derive(Debug)]
pub struct FormRec {
    /// The expanded form, re-evaluated on rebuild.
    pub node: Rc<Node>,
    /// The generation of its last committed evaluation.
    pub gen: FormGen,
    /// The eager reads of the last successful evaluation.
    pub edges: Vec<Edge>,
    /// The slots an unsuccessful attempt read successfully.
    pub attempt: Vec<VarSlotRef>,
    /// The `Failed`/`Blocked` slots an unsuccessful attempt dereferenced.
    pub subs: Vec<VarSlotRef>,
    pub state: FormState,
    pub writes: Vec<WriteItem>,
    /// It performed a one-shot effect: never rebuilt automatically.
    pub non_replayable: bool,
    /// Evaluations run (standalone and rebuilds, successful or not).
    pub runs: u64,
}

impl FormRec {
    /// A record for a form that just committed standalone.
    #[must_use]
    pub fn new(node: Rc<Node>, gen: FormGen, edges: Vec<Edge>, writes: Vec<WriteItem>) -> FormRec {
        FormRec {
            node,
            gen,
            edges,
            attempt: Vec::new(),
            subs: Vec::new(),
            state: FormState::Clean,
            writes,
            non_replayable: false,
            runs: 1,
        }
    }

    /// The slots of the names it defines.
    pub fn names(&self) -> impl Iterator<Item = &VarSlotRef> + '_ {
        self.writes.iter().filter_map(|w| match w {
            WriteItem::Name(s) => Some(s),
            WriteItem::Bind(_) => None,
        })
    }

    /// True when a write to `slot` wakes the form: its committed edges, and
    /// while it is `Failed`/`Blocked` also its attempt edges and recovery
    /// subscriptions (5.6 "metadata across any unsuccessful evaluation").
    #[must_use]
    pub fn wakes_on(&self, slot: u64) -> bool {
        self.edges.iter().any(|e| e.slot.id() == slot)
            || (self.state.is_bad()
                && (self.attempt.iter().any(|s| s.id() == slot)
                    || self.subs.iter().any(|s| s.id() == slot)))
    }

    /// Commits a successful evaluation's reads: the committed edge set is
    /// REPLACED and the failure metadata dropped.
    pub fn commit_edges(&mut self, edges: Vec<Edge>) {
        self.edges = edges;
        self.attempt.clear();
        self.subs.clear();
    }

    fn wake_slots(&self) -> BTreeSet<u64> {
        self.edges
            .iter()
            .map(|edge| edge.slot.id())
            .chain(self.attempt.iter().map(VarSlotRef::id))
            .chain(self.subs.iter().map(VarSlotRef::id))
            .collect()
    }
}

/// The per-form records and the active-owner registry.
#[derive(Debug, Default)]
pub struct DepGraph {
    forms: Vec<FormRec>,
    /// Committed generation -> form record.
    generations: BTreeMap<u64, FormId>,
    /// Defined name (slot id) -> owning form.
    name_owner: BTreeMap<u64, FormId>,
    /// Bound playing slot -> owning form.
    bind_owner: BTreeMap<SlotKey, FormId>,
    /// Slot -> forms that may wake through committed edges or failure metadata.
    readers: BTreeMap<u64, BTreeSet<FormId>>,
}

impl DepGraph {
    /// Records a form; it does not own anything until `claim`.
    pub fn add(&mut self, rec: FormRec) -> FormId {
        let id = FormId::new(u32::try_from(self.forms.len()).unwrap_or(u32::MAX));
        let slots = rec.wake_slots();
        self.generations.insert(rec.gen.get(), id);
        self.forms.push(rec);
        self.add_readers(id, slots);
        id
    }

    /// Replaces a form's committed generation and returns the retired one.
    pub fn set_generation(&mut self, form: FormId, gen: FormGen) -> Option<FormGen> {
        let rec = self.get_mut(form)?;
        let old = std::mem::replace(&mut rec.gen, gen);
        self.generations.remove(&old.get());
        self.generations.insert(gen.get(), form);
        Some(old)
    }

    fn add_readers(&mut self, form: FormId, slots: BTreeSet<u64>) {
        for slot in slots {
            self.readers.entry(slot).or_default().insert(form);
        }
    }

    fn remove_readers(&mut self, form: FormId, slots: BTreeSet<u64>) {
        for slot in slots {
            if let Some(forms) = self.readers.get_mut(&slot) {
                forms.remove(&form);
                if forms.is_empty() {
                    self.readers.remove(&slot);
                }
            }
        }
    }

    fn replace_readers(&mut self, form: FormId, previous: BTreeSet<u64>, next: BTreeSet<u64>) {
        self.remove_readers(form, previous.difference(&next).copied().collect());
        self.add_readers(form, next.difference(&previous).copied().collect());
    }

    /// Commits a form's new dependency edges and updates the reverse index.
    pub fn commit_edges(&mut self, form: FormId, edges: Vec<Edge>) {
        let Some(rec) = self.get(form) else {
            return;
        };
        let previous = rec.wake_slots();
        if let Some(rec) = self.get_mut(form) {
            rec.commit_edges(edges);
        }
        let next = self
            .get(form)
            .map_or_else(BTreeSet::new, FormRec::wake_slots);
        self.replace_readers(form, previous, next);
    }

    /// Updates failed-attempt reads and recovery subscriptions.
    pub fn set_failure_reads(
        &mut self,
        form: FormId,
        attempt: Vec<VarSlotRef>,
        subs: Vec<VarSlotRef>,
    ) {
        let Some(rec) = self.get(form) else {
            return;
        };
        let previous = rec.wake_slots();
        if let Some(rec) = self.get_mut(form) {
            rec.attempt = attempt;
            rec.subs = subs;
        }
        let next = self
            .get(form)
            .map_or_else(BTreeSet::new, FormRec::wake_slots);
        self.replace_readers(form, previous, next);
    }

    /// Updates recovery subscriptions while preserving the attempted reads.
    pub fn set_subscriptions(&mut self, form: FormId, subs: Vec<VarSlotRef>) {
        let Some(rec) = self.get(form) else {
            return;
        };
        let previous = rec.wake_slots();
        if let Some(rec) = self.get_mut(form) {
            rec.subs = subs;
        }
        let next = self
            .get(form)
            .map_or_else(BTreeSet::new, FormRec::wake_slots);
        self.replace_readers(form, previous, next);
    }

    #[must_use]
    pub fn get(&self, f: FormId) -> Option<&FormRec> {
        self.forms.get(f.get() as usize)
    }

    pub fn get_mut(&mut self, f: FormId) -> Option<&mut FormRec> {
        self.forms.get_mut(f.get() as usize)
    }

    /// Every form id, in definition order.
    pub fn ids(&self) -> impl Iterator<Item = FormId> + '_ {
        (0..self.forms.len()).filter_map(|k| u32::try_from(k).ok().map(FormId::new))
    }

    /// The form that currently owns the defined name `slot`.
    #[must_use]
    pub fn owner_of_slot(&self, slot: u64) -> Option<FormId> {
        self.name_owner.get(&slot).copied()
    }

    /// The whole name-owner registry (slot id -> form).
    #[must_use]
    pub fn name_owners(&self) -> &BTreeMap<u64, FormId> {
        &self.name_owner
    }

    /// The form that currently owns the playing slot `key`.
    #[must_use]
    pub fn owner_of_bind(&self, key: SlotKey) -> Option<FormId> {
        self.bind_owner.get(&key).copied()
    }

    /// The form whose committed generation is `gen`.
    #[must_use]
    pub fn form_of_gen(&self, gen: FormGen) -> Option<FormId> {
        self.generations.get(&gen.get()).copied()
    }

    /// Makes `f` the owner of every member of its write set and returns the
    /// previous owners it superseded (each once, `f` excluded).
    pub fn claim(&mut self, f: FormId) -> Vec<FormId> {
        let Some(rec) = self.get(f) else {
            return Vec::new();
        };
        let writes = rec.writes.clone();
        let mut prev = Vec::new();
        for w in writes {
            let old = match w {
                WriteItem::Name(s) => self.name_owner.insert(s.id(), f),
                WriteItem::Bind(k) => self.bind_owner.insert(k, f),
            };
            if let Some(g) = old.filter(|g| *g != f && !prev.contains(g)) {
                prev.push(g);
            }
        }
        prev
    }

    /// True when `f` owns every member of its write set (5.6 eligibility
    /// over the COMPLETE write set).
    #[must_use]
    pub fn is_current(&self, f: FormId) -> bool {
        self.get(f).is_some_and(|r| {
            r.writes.iter().all(|w| match w {
                WriteItem::Name(s) => self.owner_of_slot(s.id()) == Some(f),
                WriteItem::Bind(k) => self.owner_of_bind(*k) == Some(f),
            })
        })
    }

    /// True when `f` still owns at least one member (a fully superseded
    /// form is dead: it keeps no edges that matter).
    #[must_use]
    pub fn owns_any(&self, f: FormId) -> bool {
        self.get(f).is_some_and(|r| {
            r.writes.iter().any(|w| match w {
                WriteItem::Name(s) => self.owner_of_slot(s.id()) == Some(f),
                WriteItem::Bind(k) => self.owner_of_bind(*k) == Some(f),
            })
        })
    }

    /// Eligible for automatic rebuild: current owner of its whole write set
    /// and replayable.
    #[must_use]
    pub fn eligible(&self, f: FormId) -> bool {
        self.is_current(f) && self.get(f).is_some_and(|r| !r.non_replayable)
    }

    /// The slot ids of the names `f` currently owns.
    #[must_use]
    pub fn owned_slots(&self, f: FormId) -> Vec<u64> {
        self.get(f).map_or_else(Vec::new, |r| {
            r.names()
                .map(VarSlotRef::id)
                .filter(|id| self.owner_of_slot(*id) == Some(f))
                .collect()
        })
    }

    /// Eligible forms a write to `slot` wakes, in definition order.
    #[must_use]
    pub fn dependents(&self, slot: u64) -> Vec<FormId> {
        self.readers
            .get(&slot)
            .into_iter()
            .flatten()
            .copied()
            .filter(|f| self.eligible(*f) && self.get(*f).is_some_and(|r| r.wakes_on(slot)))
            .collect()
    }

    /// Eligible forms with a committed edge on `slot` or a recovery
    /// subscription to it (the targets of a status event).
    #[must_use]
    pub fn status_dependents(&self, slot: u64) -> Vec<FormId> {
        self.readers
            .get(&slot)
            .into_iter()
            .flatten()
            .copied()
            .filter(|f| {
                self.eligible(*f)
                    && self.get(*f).is_some_and(|r| {
                        r.edges.iter().any(|e| e.slot.id() == slot)
                            || r.subs.iter().any(|s| s.id() == slot)
                    })
            })
            .collect()
    }
}

/// The schedule of one reactive pass (5.6 "rounds"): the current round's
/// queue in definition order, the next round, the ordering constraints a
/// dirty-read abort adds, and the round bound.
#[derive(Debug, Default)]
pub struct Schedule {
    queue: Vec<FormId>,
    next: Vec<FormId>,
    ran: BTreeSet<FormId>,
    /// `after[f]` must complete before `f` runs (dirty-read aborts).
    after: BTreeMap<FormId, BTreeSet<FormId>>,
    /// Every form scheduled at least once in the pass.
    scheduled: BTreeSet<FormId>,
    rounds: usize,
}

impl Schedule {
    /// True when `f` waits in this round or the next.
    #[must_use]
    pub fn is_pending(&self, f: FormId) -> bool {
        self.queue.contains(&f) || self.next.contains(&f)
    }

    /// Every form waiting in this round or the next (scheduled, not yet
    /// recomputed: a read of their slots is a dirty read).
    #[must_use]
    pub fn pending(&self) -> BTreeSet<FormId> {
        self.queue.iter().chain(self.next.iter()).copied().collect()
    }

    /// Dirty-marks `f`: into the current round, or into the next one when
    /// it already ran in this round. A pending form is left where it is.
    pub fn mark(&mut self, f: FormId) {
        if self.is_pending(f) {
            return;
        }
        self.scheduled.insert(f);
        if self.ran.contains(&f) {
            self.next.push(f);
        } else {
            let k = self.queue.partition_point(|g| *g < f);
            self.queue.insert(k, f);
        }
    }

    /// Records that `f` finished an attempt (committed, failed or blocked)
    /// in this round.
    pub fn done(&mut self, f: FormId) {
        self.ran.insert(f);
    }

    /// True when `f` finished an attempt in this round.
    #[must_use]
    pub fn ran(&self, f: FormId) -> bool {
        self.ran.contains(&f)
    }

    /// The forms that finished an attempt in this round.
    #[must_use]
    pub fn ran_forms(&self) -> Vec<FormId> {
        self.ran.iter().copied().collect()
    }

    /// Re-schedules `f` for the next round (a stale read).
    pub fn to_next(&mut self, f: FormId) {
        self.queue.retain(|g| *g != f);
        if !self.next.contains(&f) {
            self.next.push(f);
        }
    }

    /// A dirty-read abort: `f` re-runs after `owner`, in `owner`'s round.
    /// Returns the members of the ordering cycle when the constraint closes
    /// one (a genuine cycle under the changed edges).
    pub fn wait_for(&mut self, f: FormId, owner: FormId) -> Option<Vec<FormId>> {
        self.after.entry(f).or_default().insert(owner);
        let from_owner = self.reach(owner);
        if owner == f || from_owner.contains(&f) {
            let mut members: Vec<FormId> = std::iter::once(owner)
                .chain(from_owner)
                .filter(|h| *h == f || *h == owner || self.reach(*h).contains(&f))
                .collect();
            members.sort_unstable();
            members.dedup();
            return Some(members);
        }
        if self.next.contains(&owner) {
            self.to_next(f);
        } else if !self.queue.contains(&f) {
            let k = self.queue.partition_point(|g| *g < f);
            self.queue.insert(k, f);
        }
        None
    }

    fn reach(&self, from: FormId) -> BTreeSet<FormId> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(x) = stack.pop() {
            for y in self.after.get(&x).into_iter().flatten() {
                if seen.insert(*y) {
                    stack.push(*y);
                }
            }
        }
        seen
    }

    /// Removes `f` from both rounds (a cycle member stops).
    pub fn drop_form(&mut self, f: FormId) {
        self.queue.retain(|g| *g != f);
        self.next.retain(|g| *g != f);
        self.ran.insert(f);
    }

    /// The next form of the current round: the first in definition order
    /// whose dirty-read constraints are met, preferring one whose committed
    /// edges point at no queued owner (topological order over the current
    /// edges is the heuristic; the deref gate is the correctness check).
    pub fn pick(&mut self, graph: &DepGraph) -> Option<FormId> {
        let queued = |g: &FormId, q: &[FormId]| q.contains(g);
        let hard: Vec<FormId> = self
            .queue
            .iter()
            .copied()
            .filter(|f| {
                self.after
                    .get(f)
                    .is_none_or(|a| a.iter().all(|g| !queued(g, &self.queue)))
            })
            .collect();
        let soft = hard.iter().copied().find(|f| {
            graph.get(*f).is_some_and(|r| {
                r.edges.iter().all(|e| {
                    graph
                        .owner_of_slot(e.slot.id())
                        .is_none_or(|g| g == *f || !queued(&g, &self.queue))
                })
            })
        });
        let f = soft.or_else(|| hard.first().copied())?;
        self.queue.retain(|g| *g != f);
        Some(f)
    }

    /// True when the current round has nothing left.
    #[must_use]
    pub fn round_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// The forms still queued in the current round.
    #[must_use]
    pub fn queued(&self) -> Vec<FormId> {
        self.queue.clone()
    }

    /// Starts the next round. Returns `Err(forms)` when the round bound
    /// (distinct scheduled forms + 1) is exceeded: those forms are a
    /// genuine cycle under the changed edges. `Ok(false)` ends the pass.
    pub fn next_round(&mut self) -> Result<bool, Vec<FormId>> {
        if self.next.is_empty() {
            return Ok(false);
        }
        self.rounds += 1;
        let mut next = std::mem::take(&mut self.next);
        next.sort_unstable();
        next.dedup();
        if self.rounds > self.scheduled.len() + 1 {
            return Err(next);
        }
        self.queue = next;
        self.ran.clear();
        Ok(true)
    }
}
