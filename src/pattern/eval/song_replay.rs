//! Original owner-local q executions, shared before any clipping or expansion.
use std::rc::Rc;

use super::song_observation::{
    event_copy_work, CanonicalIndexObservation, CanonicalOwnerFrame, RetainedQueryCall,
    SharedIndexWork,
};
use crate::pattern::{occ::ProducerTrace, query::Event};
use crate::song::Song;
use crate::vm::fail::{FailCode, Failure};

#[derive(Clone)]
struct OwnerCycleKey {
    original: Rc<Song>,
    owner: CanonicalOwnerFrame,
    entry: ProducerTrace,
    seed: u64,
    // Actual admitted call context; not part of semantic owner identity.
    entry_depth: u32,
}
impl OwnerCycleKey {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.original, &other.original)
            && self.owner == other.owner
            && self.entry == other.entry
            && self.seed == other.seed
    }
    fn same_context(&self, other: &Self) -> bool {
        self.same(other) && self.entry_depth == other.entry_depth
    }
    fn words(&self) -> u64 {
        self.owner.placement.0.len() as u64 + self.entry.steps.len() as u64 + 1
    }
}
/// Constructed only around an actual successful q on the original snapshot.
pub(crate) struct OwnerCycleExecution {
    key: OwnerCycleKey,
    clock: super::song_clock::CanonicalClockProjection,
    children: Vec<Rc<OwnerInvocation>>,
    seal: Rc<super::song_provenance::ExecutionSeal>,
    issued_children: Option<super::song_provenance::RetainedIssuedChildren>,
    events: Vec<Event>,
    observations: Vec<CanonicalIndexObservation>,
    calls: Vec<RetainedQueryCall>,
    admitted_depth: u32,
    entry_depth: u32,
}
type ReplayedOwnerCycle = (Vec<Event>, Rc<OwnerCycleExecution>);

/// Snapshot-owned immutable view; no caller can forge execution records.
pub(crate) struct ReplayView {
    original: Rc<Song>,
    executions: Vec<Rc<OwnerCycleExecution>>,
}
impl OwnerCycleExecution {
    pub(super) fn copy_issued_observations(
        &self,
        current: &super::song_clock::CanonicalClockProjection,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
        mapper: &mut super::song_provenance::IssuedOriginRebinder,
    ) -> Result<(), Failure> {
        charge_observations(work, &self.observations)?;
        charge_calls(work, &self.calls)?;
        for row in &self.observations {
            let mut copied = row.clone();
            copied.clock = current.rebind_observation_mapped(
                &self.clock,
                &row.clock,
                &self.key.owner,
                depth,
                work,
                Some(mapper),
            )?;
            if let Some(origin) = &row.event.song_source {
                copied.event.song_source = Some(mapper.copy_origin(origin, depth + 1, work)?);
            }
            work.observations.push(copied);
        }
        work.calls.extend(self.calls.iter().cloned());
        Ok(())
    }
    pub(super) fn issued_owner(&self) -> &CanonicalOwnerFrame {
        &self.key.owner
    }
    pub(super) fn issued_clock(&self) -> &super::song_clock::CanonicalClockProjection {
        &self.clock
    }
    pub(super) fn issued_events(&self) -> &[Event] {
        &self.events
    }
    pub(super) fn issued_execution(&self) -> &Rc<super::song_provenance::ExecutionSeal> {
        &self.seal
    }
    pub(super) fn issued_children(
        &self,
    ) -> Option<&super::song_provenance::RetainedIssuedChildren> {
        self.issued_children.as_ref()
    }
    #[cfg(test)]
    pub(crate) fn producer_entry(&self) -> &ProducerTrace {
        &self.key.entry
    }
    #[cfg(test)]
    pub(crate) fn entry_depth(&self) -> u32 {
        self.entry_depth
    }
    #[cfg(test)]
    pub(crate) fn observations(&self) -> &[CanonicalIndexObservation] {
        &self.observations
    }

    #[cfg(test)]
    pub(crate) fn owner(&self) -> &CanonicalOwnerFrame {
        &self.key.owner
    }
    #[cfg(test)]
    pub(crate) fn seed(&self) -> u64 {
        self.key.seed
    }
    #[cfg(test)]
    pub(crate) fn calls(&self) -> &[RetainedQueryCall] {
        &self.calls
    }
    #[cfg(test)]
    pub(crate) fn output_len(&self) -> usize {
        self.events.len()
    }
}
/// Issued only at a genuine successful owner scope. No identity constructor.
/// Creation order permits raw execution -> completed child invocation only.
pub(crate) struct OwnerInvocation {
    key: OwnerCycleKey,
    execution: Rc<OwnerCycleExecution>,
    clock: super::song_clock::CanonicalClockProjection,
    observations: Vec<CanonicalIndexObservation>,
}
pub(crate) struct OwnerInvocationMark {
    key: OwnerCycleKey,
    clock: super::song_clock::CanonicalClockProjection,
    observations: usize,
}
impl OwnerInvocation {
    pub(super) fn authentic_execution(&self, execution: &Rc<OwnerCycleExecution>) -> bool {
        Rc::ptr_eq(&self.execution, execution)
    }
    pub(super) fn rebound_issued(
        &self,
        current: &super::song_clock::CanonicalClockProjection,
        original: &super::song_clock::CanonicalClockProjection,
        owner: &CanonicalOwnerFrame,
        depth: u32,
        work: &mut super::song_observation::CanonicalIndexCollector,
        mapper: &mut super::song_provenance::IssuedOriginRebinder,
    ) -> Result<Rc<Self>, Failure> {
        work.charge(self.key.words() + observations_cost(&self.observations)? + 2)?;
        let clock = current.rebind_observation_mapped(
            original,
            &self.clock,
            owner,
            depth,
            work,
            Some(mapper),
        )?;
        let mut observations = Vec::with_capacity(self.observations.len());
        for row in &self.observations {
            let mut copied = row.clone();
            copied.clock = current.rebind_observation_mapped(
                original,
                &row.clock,
                owner,
                depth,
                work,
                Some(mapper),
            )?;
            observations.push(copied);
        }
        Ok(Rc::new(Self {
            key: self.key.clone(),
            execution: self.execution.clone(),
            clock,
            observations,
        }))
    }

    pub(crate) fn authentic_original(&self, original: &Rc<Song>) -> bool {
        Rc::ptr_eq(&self.key.original, original)
    }
    pub(crate) fn lookup_owner(&self) -> &CanonicalOwnerFrame {
        &self.key.owner
    }
    pub(crate) fn lookup_seed(&self) -> u64 {
        self.key.seed
    }
    pub(crate) fn lookup_entry(&self) -> &ProducerTrace {
        &self.key.entry
    }
    pub(crate) fn lookup_clock(&self) -> &super::song_clock::CanonicalClockProjection {
        &self.clock
    }
    pub(crate) fn lookup_depth(&self) -> u32 {
        self.execution.admitted_depth
    }

    #[cfg(test)]
    pub(crate) fn owner(&self) -> &CanonicalOwnerFrame {
        &self.key.owner
    }
    #[cfg(test)]
    pub(crate) fn seed(&self) -> u64 {
        self.key.seed
    }
    #[cfg(test)]
    pub(crate) fn entry(&self) -> &ProducerTrace {
        &self.key.entry
    }
    #[cfg(test)]
    pub(crate) fn execution(&self) -> &Rc<OwnerCycleExecution> {
        &self.execution
    }
    #[cfg(test)]
    pub(crate) fn clock(&self) -> &super::song_clock::CanonicalClockProjection {
        &self.clock
    }
    #[allow(dead_code)] // Immutable lookup is the next bounded consumer.
    pub(crate) fn observations(&self) -> &[CanonicalIndexObservation] {
        &self.observations
    }
}
impl ReplayView {
    #[cfg(test)]
    pub(crate) fn executions(&self) -> &[Rc<OwnerCycleExecution>] {
        &self.executions
    }

    pub(crate) fn publish(original: Rc<Song>, work: &SharedIndexWork) -> Result<Rc<Self>, Failure> {
        let mut ledger = work.borrow_mut();
        let cost = ledger.executions.len() as u64 + 1;
        ledger.charge(cost)?;
        Ok(Rc::new(Self {
            original,
            executions: ledger.executions.clone(),
        }))
    }
    pub(crate) fn seed_collection(
        &self,
        original: &Rc<Song>,
        work: &SharedIndexWork,
    ) -> Result<(), Failure> {
        if !Rc::ptr_eq(original, &self.original) {
            return Err(invalid("foreign canonical replay view"));
        }
        let mut ledger = work.borrow_mut();
        ledger.charge(self.executions.len() as u64)?;
        ledger.executions = self.executions.clone();
        Ok(())
    }
    fn required(
        &self,
        owner: &CanonicalOwnerFrame,
        work: &SharedIndexWork,
    ) -> Result<bool, Failure> {
        for execution in &self.executions {
            work.borrow_mut().charge(1)?;
            if same_owner(&execution.key.owner, owner) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
fn same_owner(a: &CanonicalOwnerFrame, b: &CanonicalOwnerFrame) -> bool {
    a.revision == b.revision && a.track == b.track && a.root == b.root
}
/// Counts identify the actual observation/journal suffix belonging to one q.
pub(crate) struct ExecutionMark {
    observations: usize,
    calls: usize,
    invocations: usize,
}
impl super::QState<'_, '_> {
    pub(crate) fn owner_cycle_enabled(&self) -> bool {
        self.issuance_enabled()
            || self.song_replay.is_some()
            || (self.is_observed()
                && self
                    .observation
                    .as_ref()
                    .is_some_and(|work| work.borrow().original.is_some()))
    }
    pub(crate) fn install_song_replay(&mut self, view: Rc<ReplayView>) {
        self.song_replay = Some(view);
    }
    pub(crate) fn with_source_dependencies<T>(
        &mut self,
        revision: crate::song::PartRevision,
        track: crate::value::intern::KwId,
        root: Option<crate::reader::span::NodeId>,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let selected = if let Some(work) = &self.observation {
            let mut ledger = work.borrow_mut();
            ledger.charge(1)?;
            ledger.target.as_ref().is_some_and(|target| {
                target.revision == revision && target.track == track && Some(target.root) == root
            })
        } else {
            false
        };
        let saved = self.dependency_scope;
        self.dependency_scope |= selected;
        let result = f(self);
        self.dependency_scope = saved;
        result
    }
    fn records_owner(&self, owner: &CanonicalOwnerFrame) -> Result<bool, Failure> {
        if self.issuance_enabled() {
            return Ok(true);
        }
        if self.computational_query {
            return Ok(false);
        }
        let Some(work) = &self.observation else {
            return Ok(false);
        };
        let mut ledger = work.borrow_mut();
        if ledger.original.is_none() {
            return Ok(false);
        }
        ledger.charge(1)?;
        if ledger.target.as_ref().is_some_and(|target| {
            target.revision == owner.revision
                && target.track == owner.track
                && target.root == owner.root
        }) {
            return Ok(true);
        }
        ledger.permits_dependency(owner.revision, owner.track, owner.root)
    }
    fn execution_key(
        &self,
        owner: &CanonicalOwnerFrame,
        original: Rc<Song>,
    ) -> Result<OwnerCycleKey, Failure> {
        self.spend_key(owner)?;
        Ok(OwnerCycleKey {
            original,
            owner: owner.clone(),
            seed: self.cx.seed,
            entry_depth: self.depth,
            entry: self
                .producer()
                .ok_or_else(|| invalid("replay requires original producer trace"))?,
        })
    }
    fn spend_key(&self, owner: &CanonicalOwnerFrame) -> Result<(), Failure> {
        self.observation
            .as_ref()
            .ok_or_else(|| invalid("replay requires original work"))?
            .borrow_mut()
            .charge(owner.placement.0.len() as u64 + self.producer_len().unwrap_or(0) as u64 + 1)
    }
    pub(crate) fn execution_mark(
        &self,
        owner: &CanonicalOwnerFrame,
    ) -> Result<Option<ExecutionMark>, Failure> {
        if !self.records_owner(owner)? {
            return Ok(None);
        }
        Ok(self.observation.as_ref().map(|work| {
            let ledger = work.borrow();
            ExecutionMark {
                observations: ledger.observations.len(),
                calls: ledger.calls.len(),
                invocations: ledger.invocations.len(),
            }
        }))
    }
    pub(crate) fn begin_owner_invocation(
        &mut self,
        owner: &CanonicalOwnerFrame,
    ) -> Result<Option<OwnerInvocationMark>, Failure> {
        if !self.records_owner(owner)? {
            return Ok(None);
        }
        let work = self
            .observation
            .as_ref()
            .ok_or_else(|| invalid("missing invocation ledger"))?;
        let original = work
            .borrow()
            .original
            .clone()
            .ok_or_else(|| invalid("missing invocation original"))?;
        let key = self.execution_key(owner, original)?;
        let observations = work.borrow().observations.len();
        work.borrow_mut().charge(self.song_clock.copy_work())?;
        Ok(Some(OwnerInvocationMark {
            key,
            clock: self.song_clock.clone(),
            observations,
        }))
    }
    pub(crate) fn finish_owner_invocation(
        &mut self,
        mark: Option<OwnerInvocationMark>,
        execution: Option<Rc<OwnerCycleExecution>>,
    ) -> Result<Option<Rc<OwnerInvocation>>, Failure> {
        let Some(mark) = mark else {
            return Ok(None);
        };
        let execution =
            execution.ok_or_else(|| invalid("invocation lacks actual raw execution"))?;
        let work = self
            .observation
            .as_ref()
            .ok_or_else(|| invalid("missing invocation work"))?;
        let mut ledger = work.borrow_mut();
        ledger.charge(
            mark.key
                .words()
                .checked_add(execution.key.words())
                .ok_or_else(|| invalid("invocation key cost overflow"))?,
        )?;
        if !mark.key.same(&execution.key) {
            return Err(invalid("foreign invocation execution"));
        }
        let rows = ledger
            .observations
            .get(mark.observations..)
            .ok_or_else(|| invalid("invocation observation boundary"))?;
        let cost = observations_cost(rows)?;
        ledger.charge(
            cost.checked_add(1)
                .and_then(|cost| cost.checked_add(mark.clock.copy_work()))
                .ok_or_else(|| invalid("invocation cost overflow"))?,
        )?;
        ledger.limits.check_events(
            ledger
                .invocations
                .len()
                .checked_add(1)
                .ok_or_else(|| invalid("invocation length overflow"))?,
        )?;
        let observations = ledger.observations[mark.observations..].to_vec();
        let invocation = Rc::new(OwnerInvocation {
            key: mark.key,
            execution,
            clock: mark.clock,
            observations,
        });
        ledger.invocations.push(invocation.clone());
        Ok(Some(invocation))
    }
    pub(crate) fn replay_owner_cycle(
        &mut self,
        owner: &CanonicalOwnerFrame,
    ) -> Result<Option<ReplayedOwnerCycle>, Failure> {
        let Some(work) = self.observation.clone() else {
            return Ok(None);
        };
        let (original, required) = if let Some(view) = &self.song_replay {
            if !view.required(owner, &work)? {
                return Ok(None);
            }
            (view.original.clone(), true)
        } else if self.records_owner(owner)? {
            let original = work
                .borrow()
                .original
                .clone()
                .ok_or_else(|| invalid("missing original execution authority"))?;
            (original, false)
        } else {
            return Ok(None);
        };
        let key = self.execution_key(owner, original)?;
        let executions = if let Some(view) = &self.song_replay {
            &view.executions
        } else {
            let mut found = None;
            {
                let mut ledger = work.borrow_mut();
                for index in 0..ledger.executions.len() {
                    let cost = ledger.executions[index].key.words() + key.words();
                    ledger.charge(cost)?;
                    if ledger.executions[index].key.same_context(&key) {
                        found = Some(ledger.executions[index].clone());
                        break;
                    }
                }
            }
            return found
                .map(|record| {
                    self.copy_execution(&record, true)
                        .map(|events| (events, record))
                })
                .transpose();
        };
        let mut insufficient_depth = false;
        for execution in executions {
            work.borrow_mut()
                .charge(execution.key.words() + key.words())?;
            if execution.key.same(&key) {
                if self.depth <= execution.entry_depth
                    && work.borrow().limits.max_depth >= execution.admitted_depth
                {
                    return self
                        .copy_execution(execution, false)
                        .map(|events| Some((events, execution.clone())));
                }
                insufficient_depth = true;
            }
        }
        if insufficient_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "retained execution requires original admitted depth",
            ));
        }
        if required {
            Err(invalid("required original owner-local execution missing"))
        } else {
            Ok(None)
        }
    }
    fn copy_execution(
        &self,
        execution: &OwnerCycleExecution,
        observe: bool,
    ) -> Result<Vec<Event>, Failure> {
        let work = self
            .observation
            .as_ref()
            .ok_or_else(|| invalid("missing replay ledger"))?;
        let mut ledger = work.borrow_mut();
        if ledger.limits.max_depth < execution.admitted_depth || self.depth > execution.entry_depth
        {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "retained execution requires original admitted depth",
            ));
        }
        charge_events(&mut ledger, &execution.events)?;
        if observe && !self.issuance_enabled() {
            charge_observations(&mut ledger, &execution.observations)?;
            charge_calls(&mut ledger, &execution.calls)?;
            for row in &execution.observations {
                let mut copied = row.clone();
                copied.clock = self.song_clock.rebind_observation(
                    &execution.clock,
                    &row.clock,
                    &execution.key.owner,
                    self.depth,
                    &mut ledger,
                )?;
                ledger.observations.push(copied);
            }
            ledger.calls.extend(execution.calls.iter().cloned());
            ledger.charge(execution.children.len() as u64 + 1)?;
            for child in &execution.children {
                let cost = child
                    .key
                    .words()
                    .checked_add(observations_cost(&child.observations)?)
                    .and_then(|n| n.checked_add(1 + child.clock.copy_work()))
                    .ok_or_else(|| invalid("child invocation cost overflow"))?;
                ledger.charge(cost)?;
                ledger.limits.check_events(
                    ledger
                        .invocations
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| invalid("child invocation length overflow"))?,
                )?;
                let clock = self.song_clock.rebind_observation(
                    &execution.clock,
                    &child.clock,
                    &execution.key.owner,
                    self.depth,
                    &mut ledger,
                )?;
                let mut rows = Vec::with_capacity(child.observations.len());
                for row in &child.observations {
                    let mut copied = row.clone();
                    copied.clock = self.song_clock.rebind_observation(
                        &execution.clock,
                        &row.clock,
                        &execution.key.owner,
                        self.depth,
                        &mut ledger,
                    )?;
                    rows.push(copied);
                }
                ledger.invocations.push(Rc::new(OwnerInvocation {
                    key: child.key.clone(),
                    execution: child.execution.clone(),
                    clock,
                    observations: rows,
                }));
            }
        }
        if let Some(events) = self.copy_issued_execution(execution, &mut ledger)? {
            Ok(events)
        } else {
            Ok(execution.events.clone())
        }
    }
    pub(crate) fn retain_owner_cycle(
        &mut self,
        owner: &CanonicalOwnerFrame,
        events: &[Event],
        mark: Option<ExecutionMark>,
    ) -> Result<Option<Rc<OwnerCycleExecution>>, Failure> {
        let Some(mark) = mark else {
            return Ok(None);
        };
        let work = self
            .observation
            .clone()
            .ok_or_else(|| invalid("missing execution ledger"))?;
        let original = work
            .borrow()
            .original
            .clone()
            .ok_or_else(|| invalid("missing original execution authority"))?;
        let key = self.execution_key(owner, original)?;
        let issued_children = self.retain_issued_children()?;
        let mut ledger = work.borrow_mut();
        charge_events(&mut ledger, events)?;
        let observations = &ledger.observations[mark.observations..];
        let observation_cost = observations_cost(observations)?;
        let calls_cost = calls_cost(&ledger.calls[mark.calls..])?;
        let children_cost = ledger
            .invocations
            .len()
            .checked_sub(mark.invocations)
            .ok_or_else(|| invalid("child invocation boundary"))?
            as u64;
        let cost = observation_cost
            .checked_add(calls_cost)
            .and_then(|cost| cost.checked_add(1 + self.song_clock.copy_work()))
            .and_then(|cost| cost.checked_add(children_cost))
            .ok_or_else(|| invalid("execution storage cost overflow"))?;
        ledger.charge(cost)?;
        let seal = super::song_provenance::ExecutionSeal::retained(
            &key.original,
            &key.owner,
            &key.entry,
            key.seed,
            ledger.limits.max_depth,
            self.depth,
            &mut ledger,
        )?;
        let record = Rc::new(OwnerCycleExecution {
            seal,
            issued_children,
            key,
            clock: self.song_clock.clone(),
            children: ledger.invocations[mark.invocations..].to_vec(),
            events: events.to_vec(),
            observations: ledger.observations[mark.observations..].to_vec(),
            calls: ledger.calls[mark.calls..].to_vec(),
            admitted_depth: ledger.limits.max_depth,
            entry_depth: self.depth,
        });
        ledger.executions.push(record.clone());
        Ok(Some(record))
    }
}
fn charge_events(
    ledger: &mut super::song_observation::CanonicalIndexCollector,
    events: &[Event],
) -> Result<(), Failure> {
    ledger.limits.check_events(events.len())?;
    ledger.charge(1)?;
    for event in events {
        ledger.charge(event_copy_work(event)?)?;
    }
    Ok(())
}
fn observations_cost(rows: &[CanonicalIndexObservation]) -> Result<u64, Failure> {
    rows.iter().try_fold(1u64, |cost, row| {
        cost.checked_add(event_copy_work(&row.event)?)
            .and_then(|c| {
                c.checked_add(
                    row.owner.placement.0.len() as u64
                        + row.prefix.steps.len() as u64
                        + row.clock.copy_work()
                        + 1,
                )
            })
            .ok_or_else(|| invalid("observation copy overflow"))
    })
}
fn calls_cost(calls: &[RetainedQueryCall]) -> Result<u64, Failure> {
    calls.iter().try_fold(1u64, |cost, call| {
        cost.checked_add(call.arguments.len() as u64 + 3)
            .ok_or_else(|| invalid("journal copy overflow"))
    })
}
fn charge_observations(
    ledger: &mut super::song_observation::CanonicalIndexCollector,
    rows: &[CanonicalIndexObservation],
) -> Result<(), Failure> {
    ledger.charge(observations_cost(rows)?)
}
fn charge_calls(
    ledger: &mut super::song_observation::CanonicalIndexCollector,
    calls: &[RetainedQueryCall],
) -> Result<(), Failure> {
    ledger.charge(calls_cost(calls)?)
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}

/// Derive prerequisite permissions from the same frozen topology that minted
/// the request. These permissions do not supply placement/seed authority: that
/// remains the complete key of each actual execution collected below.
pub(crate) fn prepare_owner_dependencies(
    inventory: &crate::song::snapshot::FrozenRoutingInventory,
    scope: usize,
    track: crate::value::intern::KwId,
    depth: u32,
    work: &SharedIndexWork,
) -> Result<(), Failure> {
    use crate::song::snapshot::{FrozenEdit, FrozenPartNode};
    work.borrow_mut().dependencies.clear();
    work.borrow_mut().charge(1)?;
    let mut pending = vec![(scope, track, depth)];
    let mut visited = Vec::new();
    while let Some((index, track, depth)) = pending.pop() {
        let mut ledger = work.borrow_mut();
        ledger.charge(1)?;
        if depth > ledger.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "canonical prerequisite topology depth exceeded",
            ));
        }
        // Validate every incoming alias depth before skipping its descendants.
        let mut seen = false;
        for prior in &visited {
            ledger.charge(1)?;
            if *prior == (index, track, depth) {
                seen = true;
                break;
            }
        }
        if seen {
            continue;
        }
        ledger.charge(1)?;
        visited.push((index, track, depth));
        let part = inventory
            .parts
            .get(index)
            .ok_or_else(|| invalid("canonical prerequisite part missing"))?;
        let next_depth = depth
            .checked_add(1)
            .ok_or_else(|| invalid("canonical prerequisite depth overflow"))?;
        let payload = match &part.node {
            FrozenPartNode::Capture(tracks) => {
                // Charge the entire scan before selecting the exact track.
                // No other track's callbacks become dependencies.
                ledger.charge(tracks.len() as u64)?;
                tracks
                    .iter()
                    .find(|(name, _)| *name == track)
                    .map(|(_, p)| p)
            }
            FrozenPartNode::Sequence(children) => {
                for (_, child) in children {
                    ledger.charge(1)?;
                    pending.push((*child, track, next_depth));
                }
                None
            }
            FrozenPartNode::Repeat { child, count, .. } => {
                if *count != 0 {
                    ledger.charge(1)?;
                    pending.push((*child, track, next_depth));
                }
                None
            }
            FrozenPartNode::Edit { source, edit } => {
                let replacement = matches!(edit, FrozenEdit::Replace { track: selected, .. } if *selected == track);
                if !replacement {
                    ledger.charge(1)?;
                    pending.push((*source, track, next_depth));
                }
                match edit {
                    FrozenEdit::Replace {
                        track: selected,
                        payload,
                    }
                    | FrozenEdit::Transform {
                        track: selected,
                        payload,
                        ..
                    }
                    | FrozenEdit::Overwrite {
                        track: selected,
                        payload,
                        ..
                    } if *selected == track => Some(payload),
                    _ => None,
                }
            }
        };
        if let Some(payload) = payload {
            ledger.charge(1)?;
            ledger
                .dependencies
                .push(super::song_observation::CanonicalIndexTarget {
                    revision: part.revision,
                    track,
                    root: payload.id,
                });
            for source in &payload.sources {
                ledger.charge(1)?;
                pending.push((source.root_part, source.track, next_depth));
            }
        }
    }
    Ok(())
}
