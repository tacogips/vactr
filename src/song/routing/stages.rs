//! Control-thread materialization of already-admitted logical bus stages.
use super::*;
use crate::song::SongLimits;
use crate::vm::fail::{FailCode, Failure};

/// One actual immutable graph for an admitted logical stage.
#[derive(Clone, Debug)]
pub struct SongRouteBus {
    pub target: SongDetectorTarget,
    pub template: Arc<BusDef>,
}

fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}
fn charge_stage_work(remaining: &mut u32, count: usize) -> Result<(), Failure> {
    let count = u32::try_from(count).map_err(|_| capacity_overflow())?;
    *remaining = remaining.checked_sub(count).ok_or_else(|| {
        Failure::new(
            FailCode::FuelExhausted,
            "song bus materialization work exhausted",
        )
    })?;
    Ok(())
}
fn neutral_graph(id: BusId) -> Arc<BusDef> {
    Arc::new(BusDef {
        id,
        chain: Box::new([]),
    })
}
fn next_neutral_id(next: &mut u64) -> Result<BusId, Failure> {
    let id = u32::try_from(*next).map_err(|_| capacity_overflow())?;
    *next = next.checked_add(1).ok_or_else(capacity_overflow)?;
    Ok(BusId::new(id))
}
fn graph_for_branch(
    plan: &SongRoutePlan,
    branch: &SongBranchRoute,
    remaining: &mut u32,
) -> Result<Option<Arc<BusDef>>, Failure> {
    let Some(name) = branch.effect_template else {
        return Ok(None);
    };
    for (key, graph) in &plan.topology.buses {
        charge_stage_work(remaining, 1)?;
        if *key == Some(name) {
            return Ok(Some(Arc::clone(graph)));
        }
    }
    Err(invalid("admitted private FX graph is missing"))
}

/// Materializes real branch, track and master graphs without changing policy.
///
/// Named graph Arcs remain exact; absent chains become valid empty definitions.
/// Graph IDs are candidate-local metadata, not resource ownership or host IDs.
///
/// # Errors
/// Invalid limits/duplicate targets/missing named graphs, checked ID exhaustion,
/// or cumulative inspection/allocation work beyond the supplied quota.
pub fn materialize_route_buses(
    plan: &SongRoutePlan,
    limits: SongLimits,
) -> Result<Vec<SongRouteBus>, Failure> {
    limits.validate()?;
    let mut remaining = limits.max_nodes;
    let count = plan
        .branches
        .len()
        .checked_add(plan.tracks.len())
        .and_then(|n| n.checked_add(1))
        .ok_or_else(capacity_overflow)?;
    // Charge output fields and storage before allocation, then all inspections.
    charge_stage_work(
        &mut remaining,
        count.checked_mul(4).ok_or_else(capacity_overflow)?,
    )?;
    let mut neutral = usize::from(plan.master.is_none());
    for (index, branch) in plan.branches.iter().enumerate() {
        charge_stage_work(&mut remaining, 1)?;
        for earlier in &plan.branches[..index] {
            charge_stage_work(&mut remaining, 1)?;
            if earlier.id == branch.id {
                return Err(invalid("duplicate logical song branch"));
            }
        }
        if graph_for_branch(plan, branch, &mut remaining)?.is_none() {
            neutral = neutral.checked_add(1).ok_or_else(capacity_overflow)?;
        }
    }
    for (index, track) in plan.tracks.iter().enumerate() {
        charge_stage_work(&mut remaining, 1)?;
        for earlier in &plan.tracks[..index] {
            charge_stage_work(&mut remaining, 1)?;
            if earlier.track == track.track {
                return Err(invalid("duplicate logical song track"));
            }
        }
        neutral = neutral
            .checked_add(usize::from(track.template.is_none()))
            .ok_or_else(capacity_overflow)?;
    }
    let mut largest = 0;
    for (_, graph) in &plan.topology.buses {
        charge_stage_work(&mut remaining, 1)?;
        largest = largest.max(graph.id.get());
    }
    for track in &plan.tracks {
        charge_stage_work(&mut remaining, 1)?;
        largest = largest.max(track.destination.get());
    }
    // No successor is required for an all-explicit plan, including id MAX.
    let mut next = u64::from(largest)
        .checked_add(1)
        .ok_or_else(capacity_overflow)?;
    if neutral > 0 {
        let last = next
            .checked_add(u64::try_from(neutral - 1).map_err(|_| capacity_overflow())?)
            .ok_or_else(capacity_overflow)?;
        u32::try_from(last).map_err(|_| capacity_overflow())?;
    }
    charge_stage_work(
        &mut remaining,
        neutral.checked_mul(3).ok_or_else(capacity_overflow)?,
    )?;
    // Pre-admit the second named lookup pass, so failure cannot allocate a
    // partial output before its remaining lookup cost has been admitted.
    for branch in &plan.branches {
        if branch.effect_template.is_some() {
            charge_stage_work(&mut remaining, plan.topology.buses.len())?;
        }
    }
    let mut output = Vec::with_capacity(count);
    for branch in &plan.branches {
        // The identical immutable lookup was admitted above; no mutable input
        // can change its result or traversal between admission and copying.
        let graph = branch.effect_template.and_then(|name| {
            plan.topology
                .buses
                .iter()
                .find_map(|(key, graph)| (*key == Some(name)).then(|| Arc::clone(graph)))
        });
        output.push(SongRouteBus {
            target: SongDetectorTarget::Branch(branch.id),
            template: match graph {
                Some(graph) => graph,
                None => neutral_graph(next_neutral_id(&mut next)?),
            },
        });
    }
    for track in &plan.tracks {
        output.push(SongRouteBus {
            target: SongDetectorTarget::Track(track.track),
            template: match &track.template {
                Some(graph) => Arc::clone(graph),
                None => neutral_graph(next_neutral_id(&mut next)?),
            },
        });
    }
    output.push(SongRouteBus {
        target: SongDetectorTarget::Master,
        template: match &plan.master {
            Some(graph) => Arc::clone(graph),
            None => neutral_graph(next_neutral_id(&mut next)?),
        },
    });
    Ok(output)
}
