//! Original control-thread Index observations and one cumulative work ledger.
use crate::pattern::{
    occ::ProducerTrace,
    query::{Event, TimeSpan},
};
use crate::reader::span::NodeId;
use crate::song::{PartRevision, PlacementPath, SongLimits};
use crate::value::{intern::KwId, ratio::Ratio64, value::Value};
use crate::vm::fail::{FailCode, Failure};
use std::{cell::RefCell, rc::Rc};

pub(crate) use super::song_replay::OwnerInvocation;

pub(crate) type SharedIndexWork = Rc<RefCell<CanonicalIndexCollector>>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalOwnerFrame {
    pub revision: PartRevision,
    pub track: KwId,
    pub root: NodeId,
    pub placement: PlacementPath,
    pub offset: Ratio64,
    pub duration: Ratio64,
    pub window: TimeSpan,
}
#[derive(Clone, Debug)]
pub(crate) struct CanonicalIndexObservation {
    pub owner: CanonicalOwnerFrame,
    pub issuer: NodeId,
    pub prefix: ProducerTrace,
    #[allow(dead_code)] // Original semantic seed is consumed by TASK-002 replay.
    pub seed: u64,
    pub event: Event,
    pub clock: super::song_clock::CanonicalClockProjection,
    #[allow(dead_code)] // Original issuer sample time consumed by geometry, not output onset.
    pub issuer_sample_start: Ratio64,
}
#[derive(Clone)]
pub(crate) struct RetainedQueryCall {
    #[allow(dead_code)]
    // Original callable authority is retained for the pending immutable consumer.
    pub callable: Value,
    pub arguments: Vec<Value>,
    #[allow(dead_code)] // Original callback result is retained for the pending immutable consumer.
    pub result: Value,
}
#[derive(Clone)]
pub(crate) struct CanonicalIndexTarget {
    pub revision: PartRevision,
    pub track: KwId,
    pub root: NodeId,
}
pub(crate) struct CanonicalIndexCollector {
    pub target: Option<CanonicalIndexTarget>,
    pub dependencies: Vec<CanonicalIndexTarget>,
    remaining: u32,
    pub limits: SongLimits,
    pub depth: u32,
    pub peak_depth: u32,
    pub observations: Vec<CanonicalIndexObservation>,
    pub calls: Vec<RetainedQueryCall>,
    pub vm_instructions: u64,
    pub original: Option<Rc<crate::song::Song>>,
    pub executions: Vec<Rc<super::song_replay::OwnerCycleExecution>>,
    pub invocations: Vec<Rc<OwnerInvocation>>,
}
impl CanonicalIndexCollector {
    pub fn new(remaining: u32, limits: SongLimits) -> Result<SharedIndexWork, Failure> {
        limits.validate()?;
        if remaining > limits.max_nodes {
            return Err(Failure::new(
                FailCode::Type,
                "canonical work exceeds original policy",
            ));
        }
        Ok(Rc::new(RefCell::new(Self {
            target: None,
            dependencies: Vec::new(),
            remaining: remaining.checked_sub(1).ok_or_else(exhausted)?,
            limits,
            depth: 0,
            peak_depth: 0,
            observations: Vec::new(),
            calls: Vec::new(),
            vm_instructions: 0,
            original: None,
            executions: Vec::new(),
            invocations: Vec::new(),
        })))
    }
    pub fn permits_dependency(
        &mut self,
        revision: PartRevision,
        track: KwId,
        root: NodeId,
    ) -> Result<bool, Failure> {
        for index in 0..self.dependencies.len() {
            self.charge(1)?;
            let owner = &self.dependencies[index];
            if owner.revision == revision && owner.track == track && owner.root == root {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn remaining(&self) -> u32 {
        self.remaining
    }
    pub fn charge(&mut self, count: u64) -> Result<(), Failure> {
        let count = u32::try_from(count).map_err(|_| exhausted())?;
        self.remaining = self.remaining.checked_sub(count).ok_or_else(exhausted)?;
        Ok(())
    }
    pub fn observe(
        &mut self,
        owner: &CanonicalOwnerFrame,
        issuer: NodeId,
        prefix: &ProducerTrace,
        seed: u64,
        event: &Event,
        clock: &super::song_clock::CanonicalClockProjection,
    ) -> Result<(), Failure> {
        self.limits.check_events(
            self.observations
                .len()
                .checked_add(1)
                .ok_or_else(exhausted)?,
        )?;
        let cost = event_copy_work(event)?
            .checked_add(prefix.steps.len() as u64)
            .and_then(|n| n.checked_add(owner.placement.0.len() as u64 + 1))
            .and_then(|n| n.checked_add(clock.copy_work()))
            .ok_or_else(exhausted)?;
        self.charge(cost)?;
        self.observations.push(CanonicalIndexObservation {
            owner: owner.clone(),
            issuer,
            prefix: prefix.clone(),
            seed,
            event: event.clone(),
            clock: clock.clone(),
            issuer_sample_start: event.anchor(),
        });
        Ok(())
    }
    pub fn retain_call(
        &mut self,
        callable: &Value,
        arguments: &[Value],
        result: &Value,
    ) -> Result<(), Failure> {
        self.limits
            .check_events(self.calls.len().checked_add(1).ok_or_else(exhausted)?)?;
        self.charge(arguments.len() as u64 + 3)?;
        // Value clones retain the original immutable closure/result Rc; they
        // neither traverse/rebuild a closure nor allocate the shared payload.
        self.calls.push(RetainedQueryCall {
            callable: callable.clone(),
            arguments: arguments.to_vec(),
            result: result.clone(),
        });
        Ok(())
    }
}
pub(crate) fn event_copy_work(event: &Event) -> Result<u64, Failure> {
    let lengths = [
        event.controls.len(),
        event.cells.len(),
        event.occ.path.len(),
        event.producer.as_ref().map_or(0, |p| p.steps.len()),
    ];
    lengths.into_iter().try_fold(1u64, |cost, n| {
        cost.checked_add(n as u64).ok_or_else(exhausted)
    })
}
fn exhausted() -> Failure {
    Failure::new(FailCode::FuelExhausted, "canonical Index work exhausted")
}

impl<'c, 'a> super::QState<'c, 'a> {
    pub(crate) fn new_observed(
        cx: &'c mut super::QueryCtx<'a>,
        limits: crate::song::SongLimits,
        work: SharedIndexWork,
        depth: u32,
    ) -> Result<Self, Failure> {
        if depth >= limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "canonical inherited depth exhausted",
            ));
        }
        let mut state = Self::new_song(cx, limits)?;
        state.depth = depth;
        work.borrow_mut().depth = depth;
        state.observation = Some(work);
        Ok(state)
    }
    pub(crate) fn is_observed(&self) -> bool {
        self.observation.is_some() && !self.computational_query
    }
    pub(crate) fn observes_owner(
        &self,
        revision: PartRevision,
        track: KwId,
        root: NodeId,
    ) -> Result<bool, Failure> {
        if self.computational_query || self.observation_owner.is_some() {
            return Ok(true);
        }
        let Some(work) = &self.observation else {
            return Ok(true);
        };
        let mut ledger = work.borrow_mut();
        ledger.charge(1)?;
        let Some(target) = &ledger.target else {
            return Ok(true);
        };
        let matches = target.revision == revision && target.track == track && target.root == root;
        if matches {
            return Ok(true);
        }
        if self.dependency_scope {
            ledger.permits_dependency(revision, track, root)
        } else {
            Ok(false)
        }
    }
    pub(crate) fn observation_owner(
        &mut self,
        owner: Option<CanonicalOwnerFrame>,
    ) -> Option<CanonicalOwnerFrame> {
        std::mem::replace(&mut self.observation_owner, owner)
    }
    pub(crate) fn observe_song_index(
        &mut self,
        issuer: &crate::pattern::pat::Pat,
        index: &Event,
    ) -> Result<(), Failure> {
        if let (Some(work), Some(owner), Some(prefix)) =
            (&self.observation, &self.observation_owner, &self.producer)
        {
            work.borrow_mut().observe(
                owner,
                issuer.id,
                prefix,
                self.cx.seed,
                index,
                &self.song_clock,
            )?;
        }
        Ok(())
    }

    pub(crate) fn new_computational(
        cx: &'c mut super::QueryCtx<'a>,
        limits: SongLimits,
        work: SharedIndexWork,
        depth: u32,
    ) -> Result<Self, Failure> {
        let mut state = Self::new_observed(cx, limits, work, depth)?;
        state.computational_query = true;
        Ok(state)
    }
}
