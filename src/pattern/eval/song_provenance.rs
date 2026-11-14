//! Private query-local issuance. Leaves never own executions or transcripts.
use super::{
    song_clock::{CanonicalClockProjection, SelectedSourceBoundary},
    song_observation::{CanonicalIndexCollector, CanonicalOwnerFrame, SharedIndexWork},
    song_replay::OwnerCycleExecution,
};
use crate::{
    pattern::occ::ProducerTrace,
    song::{source::SongEventOrigin, EventHandle, Song},
    vm::fail::{FailCode, Failure},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

#[derive(Debug)]
pub(crate) struct ExecutionSeal {
    original: Weak<Song>,
    owner: CanonicalOwnerFrame,
    entry: ProducerTrace,
    seed: u64,
    admitted_depth: u32,
    entry_depth: u32,
}
#[derive(Debug)]
pub(crate) struct InvocationSeal {
    execution: Rc<ExecutionSeal>,
}
#[derive(Debug)]
pub(crate) struct SourceBindingSeal {
    child: Rc<InvocationSeal>,
    issued_handle: EventHandle,
}
#[derive(Clone, Debug)]
pub(crate) struct IssuedSourceLeaves {
    pub(super) invocations: Vec<Rc<InvocationSeal>>,
    bindings: Vec<Rc<SourceBindingSeal>>,
}
#[derive(Debug)]
pub(crate) struct IssuedOwnerScope {
    original: Weak<Song>,
    owner: CanonicalOwnerFrame,
    entry: ProducerTrace,
    seed: u64,
    children: usize,
    bindings: usize,
    completions: usize,
}
pub(crate) struct IssuedOwnerMark {
    scope: Rc<IssuedOwnerScope>,
    clock: CanonicalClockProjection,
    depth: u32,
}
#[derive(Clone)]
pub(crate) struct CompletedIssuedInvocation {
    pub(super) seal: Rc<InvocationSeal>,
    pub(super) execution: Rc<OwnerCycleExecution>,
    actual: Rc<super::song_replay::OwnerInvocation>,
    pub(super) clock: CanonicalClockProjection,
    scope: Rc<IssuedOwnerScope>,
}
#[derive(Clone)]
struct CompletedSourceBinding {
    seal: Rc<SourceBindingSeal>,
    boundary: Rc<SelectedSourceBoundary>,
    member: Rc<SongEventOrigin>,
    parent: Rc<IssuedOwnerScope>,
}
#[derive(Clone)]
struct CompletedSourceBoundary {
    boundary: Rc<SelectedSourceBoundary>,
    parent: Rc<IssuedOwnerScope>,
}
#[derive(Clone)]
pub(crate) struct RetainedIssuedChildren {
    parent: Rc<IssuedOwnerScope>,
    invocations: Vec<CompletedIssuedInvocation>,
    bindings: Vec<CompletedSourceBinding>,
    completions: Vec<CompletedSourceBoundary>,
}
struct PendingSourceBinding {
    seal: Rc<SourceBindingSeal>,
    boundary: Rc<SelectedSourceBoundary>,
    parent: Rc<IssuedOwnerScope>,
}
pub(crate) struct IssuedQueryTransaction {
    original: Rc<Song>,
    work: SharedIndexWork,
    depth: u32,
    active: Vec<Rc<IssuedOwnerScope>>,
    invocations: Vec<CompletedIssuedInvocation>,
    bindings: Vec<CompletedSourceBinding>,
    pending: Vec<PendingSourceBinding>,
    completions: Vec<CompletedSourceBoundary>,
    failed: Option<Failure>,
}
pub(crate) type SharedIssuedTranscript = Rc<RefCell<IssuedQueryTransaction>>;
#[cfg_attr(not(test), allow(dead_code))] // Immutable batch consumer is the required next handoff.
pub(crate) struct IssuedQueryTranscript {
    original: Rc<Song>,
    invocations: Vec<CompletedIssuedInvocation>,
    bindings: Vec<CompletedSourceBinding>,
    completions: Vec<CompletedSourceBoundary>,
}
impl ExecutionSeal {
    pub(super) fn retained(
        original: &Rc<Song>,
        owner: &CanonicalOwnerFrame,
        entry: &ProducerTrace,
        seed: u64,
        admitted_depth: u32,
        entry_depth: u32,
        work: &mut CanonicalIndexCollector,
    ) -> Result<Rc<Self>, Failure> {
        work.charge(owner.placement.0.len() as u64 + entry.steps.len() as u64 + 2)?;
        Ok(Rc::new(Self {
            original: Rc::downgrade(original),
            owner: owner.clone(),
            entry: entry.clone(),
            seed,
            admitted_depth,
            entry_depth,
        }))
    }
    fn matches(&self, scope: &IssuedOwnerScope, depth: u32) -> bool {
        self.original.ptr_eq(&scope.original)
            && self.owner == scope.owner
            && self.entry == scope.entry
            && self.seed == scope.seed
            && depth <= self.entry_depth
    }
}
impl IssuedQueryTransaction {
    pub(crate) fn begin(
        original: Rc<Song>,
        work: SharedIndexWork,
        depth: u32,
    ) -> Result<SharedIssuedTranscript, Failure> {
        {
            let mut ledger = work.borrow_mut();
            ledger.charge(2)?;
            if depth > ledger.limits.max_depth {
                return Err(deep());
            }
            if !ledger
                .original
                .as_ref()
                .is_some_and(|song| Rc::ptr_eq(song, &original))
            {
                return Err(invalid("foreign issued query original"));
            }
        }
        Ok(Rc::new(RefCell::new(Self {
            original,
            work,
            depth,
            active: Vec::new(),
            invocations: Vec::new(),
            bindings: Vec::new(),
            pending: Vec::new(),
            completions: Vec::new(),
            failed: None,
        })))
    }
    pub(crate) fn publish(tx: SharedIssuedTranscript) -> Result<IssuedQueryTranscript, Failure> {
        let tx = Rc::try_unwrap(tx)
            .map_err(|_| invalid("live mutable issued query alias"))?
            .into_inner();
        if let Some(failure) = tx.failed {
            return Err(failure);
        }
        if !tx.active.is_empty() || !tx.pending.is_empty() {
            return Err(invalid("unfinished issued query scope"));
        }
        {
            let mut work = tx.work.borrow_mut();
            work.charge(
                (tx.invocations.len() + tx.bindings.len() + tx.completions.len()) as u64 + 1,
            )?;
            if tx.depth > work.limits.max_depth
                || !work
                    .original
                    .as_ref()
                    .is_some_and(|song| Rc::ptr_eq(song, &tx.original))
            {
                return Err(invalid("issued query attachment changed"));
            }
            for completion in &tx.completions {
                work.charge(tx.invocations.len() as u64 + 1)?;
                if completion.boundary.actual_returned().is_none()
                    || !tx
                        .invocations
                        .iter()
                        .any(|row| Rc::ptr_eq(&row.scope, &completion.parent))
                {
                    return Err(invalid("unsealed issued source completion"));
                }
            }
            for binding in &tx.bindings {
                work.charge(
                    binding
                        .boundary
                        .actual_returned()
                        .map_or(0, |members| members.len()) as u64
                        + 1,
                )?;
                if !binding.boundary.actual_returned().is_some_and(|members| {
                    members
                        .iter()
                        .any(|member| Rc::ptr_eq(member, &binding.member))
                }) {
                    return Err(invalid("foreign issued source member"));
                }
            }
        }
        Ok(IssuedQueryTranscript {
            original: tx.original,
            invocations: tx.invocations,
            bindings: tx.bindings,
            completions: tx.completions,
        })
    }
}
impl IssuedSourceLeaves {
    pub(crate) fn contributors(&self) -> &[Rc<InvocationSeal>] {
        &self.invocations
    }
}
#[cfg_attr(not(test), allow(dead_code))] // Borrowed member consumer is the next route authority phase.
pub(crate) struct IssuedSourceMemberRef<'s> {
    binding: &'s CompletedSourceBinding,
    parent: &'s super::song_replay::OwnerInvocation,
    child: &'s super::song_replay::OwnerInvocation,
}
impl IssuedSourceMemberRef<'_> {
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn is_sealed_member(
        &self,
        work: &SharedIndexWork,
        depth: u32,
    ) -> Result<bool, Failure> {
        if depth >= work.borrow().limits.max_depth {
            return Err(deep());
        }
        let members = self
            .binding
            .boundary
            .actual_returned()
            .ok_or_else(|| invalid("unsealed issued binding"))?;
        work.borrow_mut().charge(members.len() as u64 + 1)?;
        Ok(members
            .iter()
            .any(|member| Rc::ptr_eq(member, &self.binding.member)))
    }
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn member(&self) -> &Rc<SongEventOrigin> {
        &self.binding.member
    }
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn boundary(&self) -> &Rc<SelectedSourceBoundary> {
        &self.binding.boundary
    }
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn parent(&self) -> &super::song_replay::OwnerInvocation {
        self.parent
    }
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn child(&self) -> &super::song_replay::OwnerInvocation {
        self.child
    }
}
impl IssuedQueryTranscript {
    #[cfg(test)]
    pub(crate) fn empty_source_completions(
        &self,
        work: &SharedIndexWork,
        depth: u32,
    ) -> Result<usize, Failure> {
        if depth >= work.borrow().limits.max_depth {
            return Err(deep());
        }
        work.borrow_mut()
            .charge(self.completions.len() as u64 + 1)?;
        Ok(self
            .completions
            .iter()
            .filter(|completion| {
                completion
                    .boundary
                    .actual_returned()
                    .is_some_and(|members| members.is_empty())
            })
            .count())
    }
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn source_members<'s>(
        &'s self,
        bag: &Rc<IssuedSourceLeaves>,
        work: &SharedIndexWork,
        depth: u32,
    ) -> Result<Vec<IssuedSourceMemberRef<'s>>, Failure> {
        if depth >= work.borrow().limits.max_depth {
            return Err(deep());
        }
        work.borrow_mut().charge(bag.bindings.len() as u64 + 1)?;
        let mut result = Vec::with_capacity(bag.bindings.len());
        for seal in &bag.bindings {
            work.borrow_mut()
                .charge((self.bindings.len() + self.invocations.len()) as u64 + 1)?;
            let binding = self
                .bindings
                .iter()
                .find(|binding| Rc::ptr_eq(&binding.seal, seal))
                .ok_or_else(|| invalid("foreign issued source binding leaf"))?;
            let parent = self
                .invocations
                .iter()
                .find(|row| Rc::ptr_eq(&row.scope, &binding.parent))
                .ok_or_else(|| invalid("uncompleted issued source parent"))?;
            let child = self.invocation(&seal.child, work, depth)?;
            result.push(IssuedSourceMemberRef {
                binding,
                parent: &parent.actual,
                child,
            });
        }
        Ok(result)
    }
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn check_rows(
        &self,
        rows: &[crate::song::query::issued::IssuedQueryRow],
        work: &SharedIndexWork,
        depth: u32,
    ) -> Result<(), Failure> {
        for row in rows {
            if !row.authentic_contributors(&mut work.borrow_mut())? {
                return Err(invalid("foreign row contributor set"));
            }
            for bag in &row.sources {
                for binding in &bag.bindings {
                    work.borrow_mut().charge(self.bindings.len() as u64 + 1)?;
                    if !self
                        .bindings
                        .iter()
                        .any(|record| Rc::ptr_eq(&record.seal, binding))
                    {
                        return Err(invalid("foreign row source binding"));
                    }
                }
            }
            if row.contributors.is_empty() {
                return Err(invalid("issued output missing invocation"));
            }
            for seal in &row.contributors {
                if !self.authentic(&self.original, seal, work, depth)? {
                    return Err(invalid("foreign issued row contribution"));
                }
            }
        }
        Ok(())
    }
    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn invocation<'s>(
        &'s self,
        seal: &Rc<InvocationSeal>,
        work: &SharedIndexWork,
        depth: u32,
    ) -> Result<&'s super::song_replay::OwnerInvocation, Failure> {
        if !self.authentic(&self.original, seal, work, depth)? {
            return Err(invalid("foreign issued invocation leaf"));
        }
        self.invocations
            .iter()
            .find(|row| Rc::ptr_eq(&row.seal, seal))
            .map(|row| row.actual.as_ref())
            .ok_or_else(|| invalid("missing issued invocation"))
    }

    /// Exact raw execution membership only; site and rebound clock need later validation.
    #[cfg_attr(not(test), allow(dead_code))] // Mandatory immutable route adapter follows this phase.
    pub(crate) fn authentic_retained_invocation(
        &self,
        original: &Rc<Song>,
        seal: &Rc<InvocationSeal>,
        retained: &super::song_replay::OwnerInvocation,
        work: &SharedIndexWork,
        depth: u32,
    ) -> Result<bool, Failure> {
        if !self.authentic(original, seal, work, depth)? {
            return Ok(false);
        }
        work.borrow_mut()
            .charge(self.invocations.len() as u64 + 1)?;
        Ok(retained.authentic_original(original)
            && self.invocations.iter().any(|record| {
                Rc::ptr_eq(&record.seal, seal) && retained.authentic_execution(&record.execution)
            }))
    }

    #[cfg_attr(not(test), allow(dead_code))] // Authenticated owning API for the mandatory next consumer.
    pub(crate) fn authentic(
        &self,
        original: &Rc<Song>,
        seal: &Rc<InvocationSeal>,
        work: &SharedIndexWork,
        depth: u32,
    ) -> Result<bool, Failure> {
        let mut ledger = work.borrow_mut();
        if depth > ledger.limits.max_depth {
            return Err(deep());
        }
        ledger.charge(self.invocations.len() as u64 + 1)?;
        Ok(Rc::ptr_eq(&self.original, original)
            && self
                .invocations
                .iter()
                .any(|row| Rc::ptr_eq(&row.seal, seal)))
    }
}
impl super::QState<'_, '_> {
    pub(crate) fn install_issued_query(&mut self, tx: SharedIssuedTranscript) {
        self.issued_query = Some(tx);
    }
    pub(crate) fn issued_work(&self) -> Option<&SharedIndexWork> {
        if self.issued_query.is_some() {
            self.observation.as_ref()
        } else {
            None
        }
    }
    pub(crate) fn issuance_enabled(&self) -> bool {
        self.issued_query.is_some()
    }
    pub(crate) fn begin_issued_owner(
        &mut self,
        owner: &CanonicalOwnerFrame,
    ) -> Result<Option<IssuedOwnerMark>, Failure> {
        let Some(tx) = self.issued_query.clone() else {
            return Ok(None);
        };
        let entry = self
            .producer()
            .ok_or_else(|| invalid("issued query missing producer"))?;
        let mut tx = tx.borrow_mut();
        tx.work.borrow_mut().charge(
            owner.placement.0.len() as u64
                + entry.steps.len() as u64
                + self.song_clock.copy_work()
                + 2,
        )?;
        let scope = Rc::new(IssuedOwnerScope {
            original: Rc::downgrade(&tx.original),
            owner: owner.clone(),
            entry,
            seed: self.cx.seed,
            children: tx.invocations.len(),
            bindings: tx.bindings.len(),
            completions: tx.completions.len(),
        });
        let mark = IssuedOwnerMark {
            scope: scope.clone(),
            clock: self.song_clock.clone(),
            depth: self.depth,
        };
        tx.active.push(scope);
        Ok(Some(mark))
    }
    pub(crate) fn retain_issued_children(&self) -> Result<Option<RetainedIssuedChildren>, Failure> {
        let Some(tx) = &self.issued_query else {
            return Ok(None);
        };
        let tx = tx.borrow();
        let parent = tx
            .active
            .last()
            .ok_or_else(|| invalid("raw execution lacks active issued owner"))?;
        // Only completed descendants of this active scope can already exist.
        let count = tx.invocations.len() - parent.children;
        tx.work.borrow_mut().charge(
            (count
                + (tx.bindings.len() - parent.bindings)
                + (tx.completions.len() - parent.completions)) as u64
                + 1,
        )?;
        let invocations = tx.invocations[parent.children..].to_vec();
        Ok(Some(RetainedIssuedChildren {
            parent: parent.clone(),
            invocations,
            bindings: tx.bindings[parent.bindings..].to_vec(),
            completions: tx.completions[parent.completions..].to_vec(),
        }))
    }
    pub(crate) fn finish_issued_owner(
        &mut self,
        mark: Option<IssuedOwnerMark>,
        execution: Option<Rc<OwnerCycleExecution>>,
        actual: Option<Rc<super::song_replay::OwnerInvocation>>,
    ) -> Result<Option<Rc<InvocationSeal>>, Failure> {
        let Some(mark) = mark else {
            return Ok(None);
        };
        let execution = execution.ok_or_else(|| invalid("issued owner lacks genuine execution"))?;
        let actual = actual.ok_or_else(|| invalid("issued owner lacks actual invocation"))?;
        let tx = self
            .issued_query
            .as_ref()
            .ok_or_else(|| invalid("missing issued transaction"))?;
        let mut tx = tx.borrow_mut();
        if tx.work.borrow().limits.max_depth < execution.issued_execution().admitted_depth {
            return Err(deep());
        }
        tx.work.borrow_mut().charge(
            mark.scope.owner.placement.0.len() as u64 + mark.scope.entry.steps.len() as u64 + 2,
        )?;
        if !actual.authentic_original(&tx.original)
            || !actual.authentic_execution(&execution)
            || actual.lookup_owner() != &mark.scope.owner
            || actual.lookup_entry() != &mark.scope.entry
            || actual.lookup_seed() != mark.scope.seed
        {
            return Err(invalid("foreign actual issued invocation"));
        }
        if !execution
            .issued_execution()
            .matches(&mark.scope, mark.depth)
        {
            return Err(invalid("foreign issued raw execution"));
        }
        if !tx
            .active
            .last()
            .is_some_and(|scope| Rc::ptr_eq(scope, &mark.scope))
        {
            return Err(invalid("issued owner scope order"));
        }
        let seal = Rc::new(InvocationSeal {
            execution: execution.issued_execution().clone(),
        });
        tx.active.pop();
        tx.invocations.push(CompletedIssuedInvocation {
            seal: seal.clone(),
            execution,
            actual,
            clock: mark.clock,
            scope: mark.scope,
        });
        Ok(Some(seal))
    }
    pub(crate) fn abort_issued_owner(&mut self, mark: Option<IssuedOwnerMark>, failure: &Failure) {
        if let (Some(tx), Some(mark)) = (&self.issued_query, mark) {
            let mut tx = tx.borrow_mut();
            tx.failed = Some(failure.clone());
            if tx
                .active
                .last()
                .is_some_and(|scope| Rc::ptr_eq(scope, &mark.scope))
            {
                tx.active.pop();
            }
        }
    }
    pub(crate) fn bind_issued_source_member(
        &mut self,
        boundary: Option<&Rc<SelectedSourceBoundary>>,
        child: &crate::song::query::issued::IssuedQueryRow,
        mut member: SongEventOrigin,
    ) -> Result<Rc<SongEventOrigin>, Failure> {
        let Some(tx) = &self.issued_query else {
            return Ok(Rc::new(member));
        };
        let contributors = &child.contributors;
        if child.handle != member.issued_handle
            || child.event.whole != member.source_whole
            || child.event.part != member.source_part
            || child.track != member.handle.track()
        {
            return Err(invalid("issued child member descriptor mismatch"));
        }
        let boundary = boundary.ok_or_else(|| invalid("issued source boundary missing"))?;
        let mut tx = tx.borrow_mut();
        if !child.authentic_contributors(&mut tx.work.borrow_mut())? {
            return Err(invalid("swapped original row contributor"));
        }
        let parent = tx
            .active
            .last()
            .cloned()
            .ok_or_else(|| invalid("issued source outside actual owner"))?;
        tx.work.borrow_mut().charge(
            contributors.len() as u64 * (tx.invocations.len() as u64 + 1)
                + member.entry_trace.len() as u64
                + 3,
        )?;
        if contributors.is_empty() {
            return Err(invalid("issued source member lacks genuine child"));
        }
        let mut bindings = Vec::with_capacity(contributors.len());
        for child in contributors {
            if !boundary.validate_issued_child(
                &child.execution.owner,
                &member.entry_trace,
                &member.original_instrument,
                self.depth,
                &mut tx.work.borrow_mut(),
            )? {
                return Err(invalid("foreign source child scope"));
            }
            if !tx
                .invocations
                .iter()
                .any(|row| Rc::ptr_eq(&row.seal, child))
            {
                return Err(invalid("foreign issued source child"));
            }
            let seal = Rc::new(SourceBindingSeal {
                child: child.clone(),
                issued_handle: member.issued_handle.clone(),
            });
            tx.pending.push(PendingSourceBinding {
                seal: seal.clone(),
                boundary: boundary.clone(),
                parent: parent.clone(),
            });
            bindings.push(seal);
        }
        member.issued_leaves = Some(Rc::new(IssuedSourceLeaves {
            invocations: contributors.to_vec(),
            bindings,
        }));
        Ok(Rc::new(member))
    }
    pub(crate) fn finish_issued_source_boundary(
        &mut self,
        boundary: &Rc<SelectedSourceBoundary>,
    ) -> Result<(), Failure> {
        let Some(tx) = &self.issued_query else {
            return Ok(());
        };
        let mut tx = tx.borrow_mut();
        let returned = boundary
            .actual_returned()
            .ok_or_else(|| invalid("unsealed actual source boundary"))?;
        tx.work.borrow_mut().charge(
            returned.len() as u64 * (tx.pending.len() as u64 + 1) + tx.pending.len() as u64 + 1,
        )?;
        let mut keep = Vec::new();
        let pending = std::mem::take(&mut tx.pending);
        for pending in pending {
            if !Rc::ptr_eq(&pending.boundary, boundary) {
                keep.push(pending);
                continue;
            }
            let member = returned
                .iter()
                .find(|member| {
                    member.issued_leaves.as_ref().is_some_and(|bag| {
                        bag.bindings
                            .iter()
                            .any(|seal| Rc::ptr_eq(seal, &pending.seal))
                    })
                })
                .ok_or_else(|| invalid("issued returned member not sealed"))?;
            tx.bindings.push(CompletedSourceBinding {
                seal: pending.seal,
                boundary: boundary.clone(),
                member: member.clone(),
                parent: pending.parent,
            });
        }
        tx.pending = keep;
        let parent = tx
            .active
            .last()
            .cloned()
            .ok_or_else(|| invalid("source completion missing parent"))?;
        tx.completions.push(CompletedSourceBoundary {
            boundary: boundary.clone(),
            parent,
        });
        Ok(())
    }
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}
fn deep() -> Failure {
    Failure::new(FailCode::DepthExceeded, "issued query inherited depth")
}

/// One query-local memo is used by raw events AND authentic clock members.
pub(super) struct IssuedOriginRebinder {
    invocations: Vec<(Rc<InvocationSeal>, Rc<InvocationSeal>)>,
    bindings: Vec<(Rc<SourceBindingSeal>, Rc<SourceBindingSeal>)>,
    origins: Vec<(Rc<SongEventOrigin>, Rc<SongEventOrigin>)>,
    bags: Vec<(Rc<IssuedSourceLeaves>, Rc<IssuedSourceLeaves>)>,
    boundaries: Vec<(Rc<SelectedSourceBoundary>, Rc<SelectedSourceBoundary>)>,
}
impl IssuedOriginRebinder {
    fn invocation(
        &self,
        old: &Rc<InvocationSeal>,
        work: &mut CanonicalIndexCollector,
    ) -> Result<Rc<InvocationSeal>, Failure> {
        for (original, current) in &self.invocations {
            work.charge(1)?;
            if Rc::ptr_eq(original, old) {
                return Ok(current.clone());
            }
        }
        Err(invalid("unsealed replay invocation contribution"))
    }
    fn binding(
        &self,
        old: &Rc<SourceBindingSeal>,
        work: &mut CanonicalIndexCollector,
    ) -> Result<Rc<SourceBindingSeal>, Failure> {
        for (original, current) in &self.bindings {
            work.charge(1)?;
            if Rc::ptr_eq(original, old) {
                return Ok(current.clone());
            }
        }
        Err(invalid("unsealed replay source contribution"))
    }
    pub(super) fn copy_origin(
        &mut self,
        old: &Rc<SongEventOrigin>,
        depth: u32,
        work: &mut CanonicalIndexCollector,
    ) -> Result<Rc<SongEventOrigin>, Failure> {
        if depth >= work.limits.max_depth {
            return Err(deep());
        }
        for (original, current) in &self.origins {
            work.charge(1)?;
            if Rc::ptr_eq(original, old) {
                return Ok(current.clone());
            }
        }
        let mut copied = crate::song::source::copy_issued_origin_fields(old, work, depth)?;
        if let Some(inherited) = &old.inherited {
            copied.inherited = Some(self.copy_origin(inherited, depth + 1, work)?);
        }
        if let Some(bag) = &old.issued_leaves {
            let mut known = None;
            for (original, current) in &self.bags {
                work.charge(1)?;
                if Rc::ptr_eq(original, bag) {
                    known = Some(current.clone());
                    break;
                }
            }
            if let Some(current) = known {
                copied.issued_leaves = Some(current);
            } else {
                work.charge((bag.invocations.len() + bag.bindings.len()) as u64 + 2)?;
                let mut invocations = Vec::with_capacity(bag.invocations.len());
                for seal in &bag.invocations {
                    invocations.push(self.invocation(seal, work)?);
                }
                let mut bindings = Vec::with_capacity(bag.bindings.len());
                for seal in &bag.bindings {
                    bindings.push(self.binding(seal, work)?);
                }
                work.charge(2)?;
                let current = Rc::new(IssuedSourceLeaves {
                    invocations,
                    bindings,
                });
                self.bags.push((bag.clone(), current.clone()));
                copied.issued_leaves = Some(current);
            }
        }
        work.charge(2)?;
        let copied = Rc::new(copied);
        self.origins.push((old.clone(), copied.clone()));
        Ok(copied)
    }
    pub(super) fn copy_members(
        &mut self,
        old: &[Rc<SongEventOrigin>],
        depth: u32,
        work: &mut CanonicalIndexCollector,
    ) -> Result<Vec<Rc<SongEventOrigin>>, Failure> {
        work.charge(old.len() as u64 + 1)?;
        let mut copied = Vec::with_capacity(old.len());
        for member in old {
            copied.push(self.copy_origin(member, depth, work)?);
        }
        Ok(copied)
    }
    pub(super) fn copied_boundary(
        &self,
        old: &Rc<SelectedSourceBoundary>,
        work: &mut CanonicalIndexCollector,
    ) -> Result<Option<Rc<SelectedSourceBoundary>>, Failure> {
        for (original, current) in &self.boundaries {
            work.charge(1)?;
            if Rc::ptr_eq(original, old) {
                return Ok(Some(current.clone()));
            }
        }
        Ok(None)
    }
    pub(super) fn boundary_pair(
        &mut self,
        old: &Rc<SelectedSourceBoundary>,
        current: &Rc<SelectedSourceBoundary>,
        work: &mut CanonicalIndexCollector,
    ) -> Result<(), Failure> {
        if self.copied_boundary(old, work)?.is_none() {
            work.charge(2)?;
            self.boundaries.push((old.clone(), current.clone()));
        }
        Ok(())
    }
}
impl super::QState<'_, '_> {
    pub(super) fn copy_issued_execution(
        &self,
        execution: &OwnerCycleExecution,
        work: &mut CanonicalIndexCollector,
    ) -> Result<Option<Vec<crate::pattern::query::Event>>, Failure> {
        let Some(tx) = &self.issued_query else {
            return Ok(None);
        };
        let children = execution
            .issued_children()
            .ok_or_else(|| invalid("retained issued child templates missing"))?;
        let mut tx = tx.borrow_mut();
        let parent = tx
            .active
            .last()
            .cloned()
            .ok_or_else(|| invalid("replay missing issued owner scope"))?;
        let mut mapper = IssuedOriginRebinder {
            invocations: Vec::new(),
            bindings: Vec::new(),
            origins: Vec::new(),
            bags: Vec::new(),
            boundaries: Vec::new(),
        };
        work.charge(2)?;
        let mut scopes = vec![(children.parent.clone(), parent)];
        work.charge((children.invocations.len() + children.bindings.len()) as u64 + 2)?;
        for child in &children.invocations {
            work.charge(
                child.scope.owner.placement.0.len() as u64
                    + child.scope.entry.steps.len() as u64
                    + 3,
            )?;
            let seal = Rc::new(InvocationSeal {
                execution: child.execution.issued_execution().clone(),
            });
            mapper.invocations.push((child.seal.clone(), seal));
            let scope = Rc::new(IssuedOwnerScope {
                original: child.scope.original.clone(),
                owner: child.scope.owner.clone(),
                entry: child.scope.entry.clone(),
                seed: child.scope.seed,
                children: 0,
                bindings: 0,
                completions: 0,
            });
            scopes.push((child.scope.clone(), scope));
        }
        for binding in &children.bindings {
            let child = mapper.invocation(&binding.seal.child, work)?;
            work.charge(
                binding.seal.issued_handle.placement().0.len() as u64
                    + binding
                        .seal
                        .issued_handle
                        .occurrence()
                        .producer_ordinals
                        .len() as u64
                    + 2,
            )?;
            mapper.bindings.push((
                binding.seal.clone(),
                Rc::new(SourceBindingSeal {
                    child,
                    issued_handle: binding.seal.issued_handle.clone(),
                }),
            ));
        }
        let scope_for = |old: &Rc<IssuedOwnerScope>,
                         work: &mut CanonicalIndexCollector|
         -> Result<Rc<IssuedOwnerScope>, Failure> {
            for (original, current) in &scopes {
                work.charge(1)?;
                if Rc::ptr_eq(old, original) {
                    return Ok(current.clone());
                }
            }
            Err(invalid("unsealed replay parent scope"))
        };
        for child in &children.invocations {
            work.limits.check_events(tx.invocations.len() + 1)?;
            let clock = self.song_clock.rebind_observation_mapped(
                execution.issued_clock(),
                &child.clock,
                execution.issued_owner(),
                self.depth,
                work,
                Some(&mut mapper),
            )?;
            let actual = child.actual.rebound_issued(
                &self.song_clock,
                execution.issued_clock(),
                execution.issued_owner(),
                self.depth,
                work,
                &mut mapper,
            )?;
            let seal = mapper.invocation(&child.seal, work)?;
            tx.invocations.push(CompletedIssuedInvocation {
                seal,
                execution: child.execution.clone(),
                actual: actual.clone(),
                clock,
                scope: scope_for(&child.scope, work)?,
            });
            work.charge(1)?;
            work.invocations.push(actual);
        }
        for completion in &children.completions {
            let boundary = self.song_clock.rebind_completed_boundary(
                execution.issued_clock(),
                &completion.boundary,
                execution.issued_owner(),
                self.depth,
                work,
                &mut mapper,
            )?;
            work.charge(1)?;
            tx.completions.push(CompletedSourceBoundary {
                boundary,
                parent: scope_for(&completion.parent, work)?,
            });
        }
        for binding in &children.bindings {
            let boundary = self.song_clock.rebind_completed_boundary(
                execution.issued_clock(),
                &binding.boundary,
                execution.issued_owner(),
                self.depth,
                work,
                &mut mapper,
            )?;
            let member = mapper.copy_origin(&binding.member, self.depth + 1, work)?;
            work.charge(1)?;
            tx.bindings.push(CompletedSourceBinding {
                seal: mapper.binding(&binding.seal, work)?,
                boundary,
                member,
                parent: scope_for(&binding.parent, work)?,
            });
        }
        execution.copy_issued_observations(&self.song_clock, self.depth, work, &mut mapper)?;
        work.charge(execution.issued_events().len() as u64 + 1)?;
        let mut events = Vec::with_capacity(execution.issued_events().len());
        for event in execution.issued_events() {
            work.charge(super::song_observation::event_copy_work(event)?)?;
            let mut copied = event.clone();
            if let Some(origin) = &event.song_source {
                copied.song_source = Some(mapper.copy_origin(origin, self.depth + 1, work)?);
            }
            events.push(copied);
        }
        Ok(Some(events))
    }
}

#[cfg(test)]
mod retained_tests;
