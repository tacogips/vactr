//! Bounded physical pool projections; submission is never an audio commitment.
use crate::host::caps::SongPhysicalBranch;
use crate::pattern::eval::song_observation::SharedIndexWork;
use crate::song::routing::{
    SongBranchRebind, SongBranchRebound, SongBranchRelease, SongResolvedRoute,
};
use crate::vm::fail::{FailCode, Failure};
use std::collections::VecDeque;

#[derive(Clone)]
struct Slot {
    physical: SongPhysicalBranch,
    configuration: Option<SongResolvedRoute>,
    projected: u32,
    acknowledged: u32,
    deadline: u64,
    expected: VecDeque<SongBranchRebound>,
}
pub(super) struct PoolBook {
    slots: Vec<Slot>,
    max_pending: usize,
}
pub(super) struct Assignment {
    pub physical: SongPhysicalBranch,
    pub generation: u32,
    pub rebind: Option<SongBranchRebind>,
    pub release: Option<SongBranchRelease>,
}
impl PoolBook {
    pub(super) fn new(pools: &[SongPhysicalBranch], max_pending: usize) -> Self {
        Self {
            max_pending,
            slots: pools
                .iter()
                .map(|physical| Slot {
                    physical: *physical,
                    configuration: None,
                    projected: physical.initial.config.generation,
                    acknowledged: physical.initial.config.generation,
                    deadline: 0,
                    expected: VecDeque::new(),
                })
                .collect(),
        }
    }
    pub(super) fn staged(&self, work: &SharedIndexWork) -> Result<Self, Failure> {
        let pending = self.slots.iter().try_fold(0usize, |total, slot| {
            total
                .checked_add(slot.expected.len())
                .ok_or_else(|| Failure::new(FailCode::Overflow, "projected receipt count overflow"))
        })?;
        let count = self
            .slots
            .len()
            .checked_add(pending)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| Failure::new(FailCode::Overflow, "pool staging work overflow"))?;
        let count = u64::try_from(count)
            .map_err(|_| Failure::new(FailCode::Overflow, "pool staging work overflow"))?;
        work.borrow_mut().charge(count)?;
        Ok(Self {
            slots: self.slots.clone(),
            max_pending: self.max_pending,
        })
    }
    pub(super) fn assign(
        &mut self,
        route: SongResolvedRoute,
        onset: u64,
        end: u64,
        deadline: u64,
    ) -> Result<Assignment, Failure> {
        if end < onset || deadline < end {
            return Err(Failure::new(
                FailCode::Type,
                "invalid configuration lifetime",
            ));
        }
        if let Some(slot) = self
            .slots
            .iter()
            .find(|s| s.configuration.as_ref() == Some(&route))
        {
            return Ok(Assignment {
                physical: slot.physical,
                generation: slot.projected,
                rebind: None,
                release: None,
            });
        }
        let pending = self.slots.iter().try_fold(0usize, |total, slot| {
            total
                .checked_add(slot.expected.len())
                .ok_or_else(|| Failure::new(FailCode::Overflow, "projected receipt count overflow"))
        })?;
        let slot = self
            .slots
            .iter_mut()
            .find(|s| {
                s.physical.logical == route.branch
                    && (s.configuration.is_none() || s.deadline <= onset)
            })
            .ok_or_else(|| {
                Failure::new(
                    FailCode::BeyondCapability,
                    "admitted physical branch pool exhausted by overlapping configurations",
                )
            })?;
        let config = slot.physical.initial.config;
        let rebind = if slot.configuration.is_some() {
            let generation = slot.projected.checked_add(1).ok_or_else(|| {
                Failure::new(FailCode::Overflow, "song physical generation overflow")
            })?;
            if generation > slot.physical.initial.last_generation {
                return Err(Failure::new(
                    FailCode::BeyondCapability,
                    "song generation ceiling exceeded",
                ));
            }
            // Outstanding receipts share the caller storage ceiling across all slots.
            if pending >= self.max_pending {
                return Err(Failure::new(
                    FailCode::FuelExhausted,
                    "projected receipt storage exhausted",
                ));
            }
            slot.expected.push_back(SongBranchRebound {
                epoch: config.epoch,
                branch: config.branch,
                generation,
                frame: onset,
            });
            let command = SongBranchRebind {
                epoch: config.epoch,
                branch: config.branch,
                expected_generation: slot.projected,
                generation,
                transition_frame: onset,
                tail_deadline: deadline,
            };
            slot.projected = generation;
            Some(command)
        } else {
            None
        };
        slot.configuration = Some(route);
        slot.deadline = deadline;
        Ok(Assignment {
            physical: slot.physical,
            generation: slot.projected,
            rebind,
            release: Some(SongBranchRelease {
                epoch: config.epoch,
                branch: config.branch,
                generation: slot.projected,
                frame: end,
                tail_deadline: deadline,
            }),
        })
    }
    pub(super) fn receive(&mut self, rebound: SongBranchRebound) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|s| {
            s.expected
                .front()
                .is_some_and(|expected| *expected == rebound)
        }) else {
            return false;
        };
        slot.expected.pop_front();
        slot.acknowledged = rebound.generation;
        true
    }
}
