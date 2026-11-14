use super::*;
use crate::pattern::eval::song_observation::CanonicalOwnerFrame;
use crate::song::snapshot::occupancy::lookup::authority::{bind_issued_member, IssuedMemberQuery};

pub(super) fn owner_matches(
    frame: &CanonicalOwnerFrame,
    stage: &Stage<'_>,
    output_handle: &crate::song::EventHandle,
    output_revision: crate::song::PartRevision,
) -> bool {
    frame.root == stage.output_owner.payload().id
        && frame.track == output_handle.track()
        && frame.revision == output_revision
        && frame.placement == *output_handle.placement()
}

pub(super) fn stage_owner_frame<'a>(
    context: &IssuedResolution<'a>,
    stage: &Stage<'_>,
    output_handle: &crate::song::EventHandle,
    output_revision: crate::song::PartRevision,
    bridge: &mut WorkBridge<'_>,
    budget: &mut ResolutionBudget,
) -> Result<&'a CanonicalOwnerFrame, Failure> {
    for seal in context.seals {
        sync_local_work(bridge, budget)?;
        let before = bridge.work.borrow().remaining();
        let actual = context
            .transcript
            .invocation(seal, bridge.work, context.depth)?;
        let after = bridge.work.borrow().remaining();
        let spent = before
            .checked_sub(after)
            .ok_or_else(|| invalid("issued invocation replenished shared work"))?;
        budget.charge(usize::try_from(spent).map_err(|_| capacity_overflow())?)?;
        bridge.synced_remaining = budget.remaining();
        let frame = actual.lookup_owner();
        if owner_matches(frame, stage, output_handle, output_revision) {
            return Ok(frame);
        }
    }
    Err(invalid(
        "issued Index timing has no matching fresh invocation",
    ))
}

pub(super) fn parent_use_policy<'a>(
    context: &IssuedResolution<'_>,
    prepared: &'a crate::song::routing::PreparedRoutes,
    stages: &[Stage<'_>],
    output_scopes: &[usize],
    stage_index: usize,
    budget: &mut ResolutionBudget,
    depth: u32,
) -> Result<Option<crate::song::routing::PreparedPolicyRef<'a>>, Failure> {
    if stage_index == 0 {
        let Some(parent) = context.anchor_parent else {
            return Ok(None);
        };
        let limits = budget.limits();
        let site = prepared.site(
            parent.scope,
            parent.track,
            limits,
            budget.remaining_mut(),
            depth,
        )?;
        return prepared
            .policy(
                site,
                parent.source,
                budget.limits(),
                budget.remaining_mut(),
                depth,
            )
            .map(Some);
    }
    let enclosing_index = stage_index - 1;
    let enclosing = stages
        .get(enclosing_index)
        .ok_or_else(|| invalid("issued enclosing stage missing"))?;
    let scope = *output_scopes
        .get(enclosing_index)
        .ok_or_else(|| invalid("issued enclosing output scope missing"))?;
    let limits = budget.limits();
    let site = prepared.site(
        scope,
        enclosing.handle.track(),
        limits,
        budget.remaining_mut(),
        depth,
    )?;
    prepared
        .policy(
            site,
            enclosing.identity.policy as usize,
            budget.limits(),
            budget.remaining_mut(),
            depth,
        )
        .map(Some)
}

pub(super) fn bind_timing_member(
    context: &IssuedResolution<'_>,
    stage: &Stage<'_>,
    owner: &CanonicalOwnerFrame,
    site: crate::song::routing::PreparedSiteRef<'_>,
    bound: &super::super::index::BoundSliceOperands<'_>,
    bridge: &mut WorkBridge<'_>,
    budget: &mut ResolutionBudget,
) -> Result<(), Failure> {
    let limits = budget.limits();
    let policy = context.prepared.policy(
        site,
        stage.identity.policy as usize,
        limits,
        budget.remaining_mut(),
        context.depth,
    )?;
    let selected = policy.bind_original(budget.limits(), budget.remaining_mut(), context.depth)?;
    let query = IssuedMemberQuery {
        site,
        owner,
        selected,
        handle: bound.timing.subject_handle(),
    };
    sync_local_work(bridge, budget)?;
    let before = bridge.work.borrow().remaining();
    let member = bind_issued_member(
        &query,
        context.transcript,
        context.seals,
        bridge.work,
        context.depth,
    )?;
    let after = bridge.work.borrow().remaining();
    let spent = before
        .checked_sub(after)
        .ok_or_else(|| invalid("issued member binding replenished shared work"))?;
    budget.charge(usize::try_from(spent).map_err(|_| capacity_overflow())?)?;
    bridge.synced_remaining = budget.remaining();
    drop(member);
    Ok(())
}

pub(super) fn issued_index_stage_configuration(
    context: &IssuedResolution<'_>,
    stages: &[Stage<'_>],
    output_scopes: &[usize],
    output_handles: &[&crate::song::EventHandle],
    owner: TimeSpan,
    budget: &mut ResolutionBudget,
    bridge: &mut WorkBridge<'_>,
) -> Result<Option<TimeSpan>, Failure> {
    use crate::song::snapshot::occupancy::lookup::authority::{
        bind_issued_index, IssuedOwnerSelector,
    };
    use crate::song::source_uses::FrozenUseTraceTerm;
    let prepared = context.prepared;
    let transcript = context.transcript;
    let depth = context.depth;
    let mut result = None;
    let local_owner = owner.map(|time| time.checked_sub(context.offset))?;
    for (stage_index, stage) in stages.iter().enumerate() {
        let output_handle = output_handles
            .get(stage_index)
            .ok_or_else(|| invalid("issued output handle missing"))?;
        let graph = stage.cover.graph();
        let mut node_index = graph.root;
        let mut prefix = Vec::<FrozenUseTraceTerm>::new();
        let mut matched_timings = vec![false; stage.slice_timings.len()];
        for cursor in 0..=stage.identity.edges.len() {
            budget.enter(
                stage
                    .policy_depth
                    .checked_add(u32::try_from(cursor).map_err(|_| capacity_overflow())?)
                    .ok_or_else(capacity_overflow)?,
            )?;
            let node = graph
                .nodes
                .get(node_index as usize)
                .ok_or_else(|| invalid("issued Index source path node"))?;
            if let FrozenUseMapping::Slices {
                structure: FrozenSliceStructure::Index { issuer },
                ..
            } = node.mapping
            {
                for (timing_index, timing) in stage
                    .slice_timings
                    .iter()
                    .enumerate()
                    .filter(|(_, timing)| timing.issuer() == issuer)
                {
                    if super::super::super::index::trace_end(
                        &prefix,
                        timing.issuer_trace(),
                        0,
                        budget,
                    )? != Some(timing.issuer_trace().len())
                    {
                        continue;
                    }
                    matched_timings[timing_index] = true;
                    let output_scope = *output_scopes
                        .get(stage_index)
                        .ok_or_else(|| invalid("issued output scope missing"))?;
                    let output_revision = prepared
                        .plan()
                        .topology
                        .parts
                        .get(output_scope)
                        .ok_or_else(|| invalid("issued output scope part missing"))?
                        .revision;
                    let output = super::super::super::index::output_operand_owner(
                        &prepared.plan().topology,
                        output_scope,
                        stage.handle.track(),
                        budget,
                    )?;
                    let bound_prepared = super::super::super::index::prepare_slice_operands(
                        output,
                        issuer,
                        &prefix,
                        stage.policy_depth,
                        budget,
                    )?;
                    let bound = super::super::super::index::bind_slice_operands(
                        bound_prepared,
                        timing,
                        stage.handle,
                        stage.policy_depth,
                        budget,
                    )?;
                    let site_limits = budget.limits();
                    let site = prepared.site(
                        output_scope,
                        stage.handle.track(),
                        site_limits,
                        budget.remaining_mut(),
                        depth,
                    )?;
                    let selector = IssuedOwnerSelector {
                        site,
                        issuer,
                        prefix: &prefix,
                        owner_window: stage.scope.interval,
                    };
                    let stage_owner = stage_owner_frame(
                        context,
                        stage,
                        output_handle,
                        output_revision,
                        bridge,
                        budget,
                    )?;
                    bind_timing_member(context, stage, stage_owner, site, &bound, bridge, budget)?;
                    let mut parent_use = parent_use_policy(
                        context,
                        prepared,
                        stages,
                        output_scopes,
                        stage_index,
                        budget,
                        depth,
                    )?;
                    let mut timing_result = None;
                    for seal in context.seals {
                        sync_local_work(bridge, budget)?;
                        let before = bridge.work.borrow().remaining();
                        let actual = transcript.invocation(seal, bridge.work, depth)?;
                        let after = bridge.work.borrow().remaining();
                        let spent = before
                            .checked_sub(after)
                            .ok_or_else(|| invalid("issued invocation replenished shared work"))?;
                        budget.charge(usize::try_from(spent).map_err(|_| capacity_overflow())?)?;
                        bridge.synced_remaining = budget.remaining();
                        if !owner_matches(
                            actual.lookup_owner(),
                            stage,
                            output_handle,
                            output_revision,
                        ) {
                            continue;
                        }
                        let address_policy = match parent_use.take() {
                            Some(policy) => Some(policy),
                            None if stage_index > 0 => parent_use_policy(
                                context,
                                prepared,
                                stages,
                                output_scopes,
                                stage_index,
                                budget,
                                depth,
                            )?,
                            None => None,
                        };
                        sync_local_work(bridge, budget)?;
                        let before = bridge.work.borrow().remaining();
                        let operand = bind_issued_index(
                            &selector,
                            transcript,
                            seal,
                            address_policy,
                            bridge.work,
                            depth,
                        )?;
                        let after = bridge.work.borrow().remaining();
                        let spent = before.checked_sub(after).ok_or_else(|| {
                            invalid("issued index binding replenished shared work")
                        })?;
                        budget.charge(usize::try_from(spent).map_err(|_| capacity_overflow())?)?;
                        bridge.synced_remaining = budget.remaining();
                        let Some(operand) = operand else {
                            continue;
                        };
                        let policy_limits = budget.limits();
                        let selected = operand
                            .policy()
                            .map(|policy| {
                                policy.bind_original(policy_limits, budget.remaining_mut(), depth)
                            })
                            .transpose()?;
                        let Some(local) = super::super::configuration::issued_index_configuration(
                            &bound,
                            &operand,
                            selected,
                            intersect(stage.scope.interval, local_owner)?,
                            local_owner,
                            depth,
                            budget,
                        )?
                        else {
                            return Err(super::missing_issued_index_component());
                        };
                        let local = local.map(|time| time.checked_add(context.offset))?;
                        timing_result = Some(if let Some(previous) = timing_result {
                            if previous != local {
                                return Err(invalid(
                                    "issued invocations resolve conflicting Index routes",
                                ));
                            }
                            previous
                        } else {
                            local
                        });
                    }
                    let local = timing_result.ok_or_else(|| {
                        invalid("issued Index timing has no matching fresh invocation")
                    })?;
                    result = Some(if let Some(previous) = result {
                        intersect(previous, local)?
                    } else {
                        local
                    });
                }
            }
            let Some(ordinal) = stage.identity.edges.get(cursor) else {
                break;
            };
            let edge = node
                .edges
                .get(*ordinal as usize)
                .ok_or_else(|| invalid("issued Index source path edge"))?;
            budget.charge(edge.trace.len())?;
            prefix.extend_from_slice(&edge.trace);
            node_index = edge.child;
        }
        if matched_timings.iter().any(|matched| !matched) {
            return Err(invalid(
                "issued Slice absent from authentic complete source path",
            ));
        }
    }
    Ok(result)
}
