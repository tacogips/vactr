//! Prepared nested certificates and borrowed, budgeted origin-chain resolution.
mod clock;
mod preparation;
use super::configuration::{
    map_intrinsic_configuration, map_source_locator, sampled_configuration, sampled_recipe,
    scope_payload, source_scope, MembershipSide, SampledRecipeInput, SourceLocatorMapping,
    SourceMembership, SourceTimingContext,
};
use super::source::{invalid, ResolutionBudget};
use super::*;
use crate::vm::fail::Failure;

#[cfg(test)] // Legacy validation witness; metered preparation preserves the original policy directly.
pub(super) fn certification_limits(
    remaining: u32,
    depth: u32,
) -> Result<crate::song::SongLimits, Failure> {
    use crate::vm::fail::FailCode;
    if remaining == 0 {
        return Err(Failure::new(
            FailCode::FuelExhausted,
            "preparation certification quota exhausted",
        ));
    }
    if depth == 0 {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "preparation certification depth exhausted",
        ));
    }
    Ok(crate::song::SongLimits {
        max_nodes: remaining,
        max_depth: depth,
        ..Default::default()
    })
}

fn register_source_roots(
    payload: &crate::song::snapshot::FrozenPattern,
    roots: &mut std::collections::BTreeSet<(usize, KwId)>,
    budget: &mut ResolutionBudget,
) -> Result<(), Failure> {
    use crate::song::source_uses::FrozenUseMapping as M;
    let graph = &payload.source_uses;
    if graph.nodes.is_empty() {
        return Ok(());
    }
    budget.charge(
        graph
            .nodes
            .len()
            .checked_add(1)
            .ok_or_else(capacity_overflow)?,
    )?;
    let mut seen = vec![false; graph.nodes.len()];
    let mut pending = vec![graph.root];
    while let Some(index) = pending.pop() {
        budget.charge(1)?;
        let mark = seen
            .get_mut(index as usize)
            .ok_or_else(|| invalid("nested source graph node"))?;
        if *mark {
            continue;
        }
        *mark = true;
        let node = &graph.nodes[index as usize];
        if let M::Source { policy } = node.mapping {
            let selected = payload
                .sources
                .get(policy as usize)
                .ok_or_else(|| invalid("nested source graph policy"))?;
            budget.charge(1)?;
            roots.insert((selected.root_part, selected.track));
        }
        if matches!(node.mapping,M::Rate{factor} if factor<=Ratio64::ZERO) {
            continue;
        }
        budget.charge(node.edges.len())?;
        for (ordinal, edge) in node.edges.iter().enumerate() {
            if matches!(node.mapping, M::Slices { .. }) && ordinal != 0 {
                continue;
            }
            if let M::SelectContent { content }
            | M::Restructure { content, .. }
            | M::SampleCycles { content } = node.mapping
            {
                if ordinal != content as usize {
                    continue;
                }
            }
            pending.push(edge.child);
        }
    }
    Ok(())
}

/// Prepare inner certificates once under one cumulative budget. Each selected
/// root is traversed symbolically; Repeat visits its child, never its copies.
pub(super) use preparation::prepare_nested_covers;
#[allow(unused_imports)]
// Required next issued route builder; private owning tests call the child directly.
pub(super) use preparation::prepare_nested_covers_metered;

pub(super) struct MatchedSources {
    pub sources: Vec<SongSourceConfiguration>,
    pub configuration: crate::pattern::TimeSpan,
}

struct Stage<'a> {
    output_owner: super::index::OutputOperandOwner<'a>,
    slice_timings: &'a [crate::song::source_uses::FrozenSliceTiming],
    input_root: usize,
    policy_depth: u32,
    handle: &'a crate::song::EventHandle,
    cover: &'a crate::song::source_uses::FrozenSourceUseCover,
    identity: crate::song::source_uses::FrozenSourceUseIdentity,
    trace: &'a [crate::pattern::occ::ProducerStep],
    instrument: &'a FrozenSound,
    scope: super::configuration::SourceScope<'a>,
    output_onset: Ratio64,
    output_whole: Option<crate::pattern::TimeSpan>,
    output_offset: Ratio64,
    input_onset: Ratio64,
    sampled_locator: Option<SourceMembership>,
    source_whole: Option<crate::pattern::TimeSpan>,
}
/// All frames are borrowed from one successful immutable realization. Each
/// matcher reserves work from the caller's existing budget before allocating.
pub(super) fn resolve_sources(
    plan: &SongRoutePlan,
    event: &crate::song::snapshot::FrozenSongEvent,
    branch: &SongBranchRoute,
    offset: Ratio64,
    mut configuration: crate::pattern::TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<MatchedSources, Failure> {
    use crate::song::source_uses::{resolve_source_use, resolve_source_use_frame};
    let Some(origin) = &event.source_origin else {
        verify_policy(plan, event, None, budget)?;
        return Ok(MatchedSources {
            sources: Vec::new(),
            configuration,
        });
    };
    let certificate = plan
        .source_covers
        .get(
            branch
                .source_cover
                .ok_or_else(|| invalid("source provenance on ordinary branch"))?
                as usize,
        )
        .ok_or_else(|| invalid("source cover index"))?;
    let mut payload = scope_payload(&plan.topology, certificate.scope_part, certificate.track)?;
    let mut output_scope = certificate.scope_part;
    let mut output_track = certificate.track;
    let mut cover = &certificate.cover;
    let mut base_depth = u32::try_from(
        branch
            .placement
            .iter()
            .filter(|p| !matches!(p, SongRoutePlacement::Region { .. }))
            .count(),
    )
    .map_err(|_| capacity_overflow())?
    .checked_add(1)
    .ok_or_else(capacity_overflow)?;
    let count = origin
        .inherited
        .len()
        .checked_add(1)
        .ok_or_else(capacity_overflow)?;
    budget.charge(count)?;
    let mut stages = Vec::with_capacity(count);
    let mut output_onset = event.handle.occurrence().onset.checked_sub(offset)?;
    let mut output_offset = Ratio64::ZERO;
    let mut incoming_sampled = None;
    budget.charge(4)?;
    let mut output_whole = event
        .whole
        .map(|whole| whole.map(|t| t.checked_sub(offset)))
        .transpose()?;
    for ordinal in 0..count {
        let output_owner =
            super::index::output_operand_owner(&plan.topology, output_scope, output_track, budget)?;
        if !std::ptr::eq(output_owner.payload(), payload) {
            return Err(invalid(
                "inherited Index output payload differs from topology owner",
            ));
        }
        let slice_timings = if ordinal == 0 {
            origin.slice_timings()
        } else {
            origin.inherited[ordinal - 1].slice_timings()
        };
        let (handle, instrument, trace) = if ordinal == 0 {
            (
                &origin.handle,
                &origin.original_instrument,
                origin.entry_trace.as_slice(),
            )
        } else {
            let frame = &origin.inherited[ordinal - 1];
            (
                &frame.handle,
                &frame.original_instrument,
                frame.entry_trace.as_slice(),
            )
        };
        let current_cover = cover;
        let limits = super::source::reserve_source_search(
            cover,
            payload,
            if ordinal == 0 {
                origin.borrowed_view()
            } else {
                origin.inherited[ordinal - 1].borrowed_view()
            },
            base_depth,
            budget,
        )?;
        let identity = if ordinal == 0 {
            resolve_source_use(cover, origin, limits)?
        } else {
            resolve_source_use_frame(cover, &origin.inherited[ordinal - 1], limits)?
        };
        let selected = payload
            .sources
            .get(identity.policy as usize)
            .ok_or_else(|| invalid("nested selected policy"))?;
        let graph_depth = u32::try_from(identity.edges.len())
            .map_err(|_| capacity_overflow())?
            .checked_add(1)
            .ok_or_else(capacity_overflow)?;
        let policy_depth = base_depth
            .checked_add(graph_depth)
            .ok_or_else(capacity_overflow)?;
        let scope = source_scope(
            &plan.topology,
            selected.root_part,
            handle,
            policy_depth,
            budget,
        )?;
        if ordinal + 1 < count {
            budget.charge(plan.nested_source_covers.len())?;
            let next = plan
                .nested_source_covers
                .iter()
                .find(|c| {
                    c.root_part == selected.root_part
                        && c.scope_part == scope.index
                        && c.track == handle.track()
                })
                .ok_or_else(|| invalid("inherited origin has no prepared owning payload"))?;
            cover = &next.cover;
            payload = scope.payload;
            base_depth = scope.depth;
            output_scope = scope.index;
            output_track = handle.track();
        }
        let source_whole = if ordinal == 0 {
            origin.source_whole()
        } else {
            origin.inherited[ordinal - 1].source_whole()
        };
        let sampled_mapping = map_source_locator(
            current_cover,
            &identity,
            trace,
            output_onset,
            SourceTimingContext {
                incoming_member: incoming_sampled,
                output_whole,
                source_whole,
            },
            budget,
        )?;
        let sampled_locator = match sampled_mapping {
            SourceLocatorMapping::UnchangedBirth => None,
            SourceLocatorMapping::SampledPoint(input) => {
                let current = incoming_sampled.unwrap_or(SourceMembership {
                    point: output_onset,
                    side: MembershipSide::At,
                });
                budget.charge(1)?;
                incoming_sampled = Some(SourceMembership {
                    point: input.point.checked_sub(scope.offset)?,
                    side: input.side,
                });
                Some(current)
            }
            SourceLocatorMapping::NeedsJointGeometry => {
                return Err(invalid("sampled context requires joint mapping geometry"));
            }
        };
        let next_onset = handle.occurrence().onset.checked_sub(scope.offset)?;
        let next_offset = scope.offset;
        stages.push(Stage {
            output_owner,
            slice_timings,
            input_root: selected.root_part,
            policy_depth,
            handle,
            cover: current_cover,
            identity,
            trace,
            instrument,
            scope,
            output_onset,
            output_whole,
            source_whole,
            output_offset,
            input_onset: handle.occurrence().onset,
            sampled_locator,
        });
        budget.charge(4)?;
        output_whole = source_whole
            .map(|whole| whole.map(|t| t.checked_sub(next_offset)))
            .transpose()?;
        output_onset = next_onset;
        output_offset = next_offset;
    }
    let mut inherited_route = None;
    for (ordinal, stage) in stages.iter().enumerate().rev() {
        let inner = stages.get(ordinal + 1);
        let query = FxPolicyQuery {
            root: stage.input_root,
            track: stage.handle.track(),
            instrument: inner.map_or(stage.instrument, |s| s.instrument),
            onset: stage.input_onset,
            origin_revision: inner.map(|s| s.handle.revision()),
            depth: stage.policy_depth,
        };
        if let Some(route) = policy_route(&plan.topology, query, budget)? {
            inherited_route = Some(route);
        }
    }
    verify_policy(plan, event, inherited_route, budget)?;
    let index_joint = index_stage_configuration(&stages, configuration, offset, budget)?;
    let joint = if index_joint.is_some() {
        index_joint
    } else if stages.iter().any(|stage| stage.sampled_locator.is_some()) {
        Some(sampled_stage_configuration(
            &stages,
            configuration,
            offset,
            budget,
        )?)
    } else {
        joint_stage_configuration(
            &stages,
            configuration,
            offset,
            event.handle.occurrence().onset,
            budget,
        )?
    };
    let mut mapped = None;
    for stage in stages.iter().rev().filter(|_| joint.is_none()) {
        let mut interval = stage.scope.interval;
        if let Some(inner) = mapped {
            interval = intersect(interval, inner)?;
        }
        let mapped_stage = map_intrinsic_configuration(
            stage.cover,
            &stage.identity,
            stage.trace,
            super::configuration::IntrinsicConfigurationInput {
                span: interval,
                anchor: stage
                    .sampled_locator
                    .map_or(stage.output_onset, |m| m.point),
                source_onset: stage.input_onset,
                output_whole: stage.output_whole,
            },
            budget,
        )?
        .ok_or_else(|| invalid("nested mapping geometry remains unresolved"))?;
        mapped = Some(mapped_stage.map(|t| t.checked_add(stage.output_offset))?);
    }
    if let Some(mapped) = mapped {
        configuration = intersect(configuration, mapped.map(|t| t.checked_add(offset))?)?;
    }
    if let Some(joint) = joint {
        configuration = joint;
    }
    budget.charge(stages.len())?;
    let mut sources = Vec::with_capacity(stages.len());
    for stage in stages {
        budget.charge(1)?;
        if let FrozenSound::Sample { path, .. } = stage.instrument {
            budget.charge(path.len())?;
        }
        sources.push(SongSourceConfiguration {
            identity: stage.identity,
            original_instrument: stage.instrument.clone(),
            original_configuration: stage.scope.interval,
            configuration,
        });
    }
    Ok(MatchedSources {
        sources,
        configuration,
    })
}

fn preserving_source_tail(
    stage: &Stage<'_>,
    slice: &crate::song::source_uses::FrozenSourceUseNode,
    cursor: usize,
    timing: &crate::song::source_uses::FrozenSliceTiming,
    budget: &mut ResolutionBudget,
) -> Result<bool, Failure> {
    use crate::pattern::occ::{ProducerKind, ProducerStep};
    use crate::song::source_uses::{FrozenUseMapping as M, FrozenUseOperation as O};
    let graph = stage.cover.graph();
    let Some(&ordinal) = stage.identity.edges.get(cursor) else {
        return Ok(false);
    };
    if ordinal != 0 {
        return Ok(false);
    }
    let edge = slice
        .edges
        .get(ordinal as usize)
        .ok_or_else(|| invalid("Slice subject tail edge"))?;
    let Some(mut trace) = super::index::trace_end(
        &edge.trace,
        stage.trace,
        timing.issuer_trace().len(),
        budget,
    )?
    else {
        return Ok(false);
    };
    let mut index = edge.child;
    for &ordinal in stage.identity.edges.iter().skip(cursor + 1) {
        budget.enter(
            stage
                .policy_depth
                .checked_add(u32::try_from(trace).map_err(|_| capacity_overflow())?)
                .ok_or_else(capacity_overflow)?,
        )?;
        let node = graph
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("fixed source tail node"))?;
        if node.operation != O::Pure
            || node.mapping != M::Preserve
            || node.edges.len() != 1
            || ordinal != 0
        {
            return Ok(false);
        }
        let edge = &node.edges[0];
        use crate::song::source_uses::FrozenUseTraceTerm as T;
        if edge.trace.as_slice()
            != [
                T::Exact(ProducerStep {
                    kind: ProducerKind::DynamicExpansion,
                    ordinal: 0,
                }),
                T::Exact(ProducerStep {
                    kind: ProducerKind::Child,
                    ordinal: 0,
                }),
            ]
        {
            return Ok(false);
        }
        let Some(end) = super::index::trace_end(&edge.trace, stage.trace, trace, budget)? else {
            return Ok(false);
        };
        budget.charge(edge.layout.len())?;
        let unit =
            crate::song::source_uses::layout::slot_interval(edge, 0, &stage.trace[trace..end])?;
        if unit.begin != Ratio64::ZERO || unit.end != Ratio64::ONE {
            return Ok(false);
        }
        trace = end;
        index = edge.child;
    }
    budget.charge(1)?;
    Ok(graph.nodes.get(index as usize).is_some_and(
        |node| matches!(node.mapping, M::Source { policy } if policy == stage.identity.policy),
    ) && trace == stage.trace.len())
}

fn index_stage_configuration(
    stages: &[Stage<'_>],
    owner: crate::pattern::TimeSpan,
    offset: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    use crate::song::source_uses::{FrozenSliceStructure, FrozenUseMapping as M};
    budget.charge(stages.len())?;
    let mut configuration = None;
    for stage in stages {
        for timing in stage.slice_timings {
            budget.charge(1)?;
            let graph = stage.cover.graph();
            let mut index = graph.root;
            let mut prefix = Vec::new();
            let mut matched = false;
            for cursor in 0..=stage.identity.edges.len() {
                budget.enter(
                    stage
                        .policy_depth
                        .checked_add(u32::try_from(cursor).map_err(|_| capacity_overflow())?)
                        .ok_or_else(capacity_overflow)?,
                )?;
                let current = graph
                    .nodes
                    .get(index as usize)
                    .ok_or_else(|| invalid("Index issued source path node"))?;
                if let M::Slices {
                    structure: FrozenSliceStructure::Index { issuer },
                    ..
                } = current.mapping
                {
                    if issuer == timing.issuer()
                        && super::index::trace_end(&prefix, timing.issuer_trace(), 0, budget)?
                            == Some(timing.issuer_trace().len())
                    {
                        let prepared = super::index::prepare_slice_operands(
                            stage.output_owner,
                            issuer,
                            &prefix,
                            stage.policy_depth,
                            budget,
                        )?;
                        let bound = super::index::bind_slice_operands(
                            prepared,
                            timing,
                            stage.handle,
                            stage.policy_depth,
                            budget,
                        )?;
                        let direct_source =
                            preserving_source_tail(stage, current, cursor, timing, budget)?;
                        if stages.len() != 1 || stage.slice_timings.len() != 1 || !direct_source {
                            return Err(super::index::IndexRealizationRequest {
                                prepared: bound.prepared, timing: Some(timing), operand: bound.index_root,
                                kind: crate::song::source_uses::timing::FrozenIndexDynamicKind::UnresolvedPattern,
                            }.failure());
                        }
                        let Some(clock) =
                            clock::issuer_clock(stage, cursor, owner, offset, budget)?
                        else {
                            return Err(super::index::IndexRealizationRequest {
                                prepared: bound.prepared, timing: Some(timing), operand: bound.index_root,
                                kind: crate::song::source_uses::timing::FrozenIndexDynamicKind::UnresolvedPattern,
                            }.failure());
                        };
                        let local = super::configuration::index_configuration(
                            &bound,
                            intersect(stage.scope.interval, clock.birth_owner)?,
                            clock.owner,
                            stage.policy_depth,
                            budget,
                        )?;
                        configuration = Some(local.map(|t| {
                            t.checked_sub(clock.shift)?
                                .checked_div(clock.factor)?
                                .checked_add(offset)
                        })?);
                        matched = true;
                        break;
                    }
                }
                let Some(ordinal) = stage.identity.edges.get(cursor) else {
                    break;
                };
                let edge = current
                    .edges
                    .get(*ordinal as usize)
                    .ok_or_else(|| invalid("Index issued source path edge"))?;
                budget.charge(edge.trace.len())?;
                prefix.extend_from_slice(&edge.trace);
                index = edge.child;
            }
            if !matched {
                return Err(invalid(
                    "issued Slice absent from authentic complete source path",
                ));
            }
        }
    }
    Ok(configuration)
}

fn sampled_stage_configuration(
    stages: &[Stage<'_>],
    owner: crate::pattern::TimeSpan,
    offset: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<crate::pattern::TimeSpan, Failure> {
    budget.charge(stages.len())?;
    let mut recipes = Vec::with_capacity(stages.len());
    // Collect all local predicates and full source bounds before pinning any
    // component. Membership points are authenticated through the whole chain.
    for stage in stages {
        let input = SampledRecipeInput {
            output_birth: stage.output_onset,
            incoming_member: Some(stage.sampled_locator.unwrap_or(SourceMembership {
                point: stage.output_onset,
                side: MembershipSide::At,
            })),
            output_whole: stage.output_whole,
            source_whole: stage.source_whole,
            source_birth: stage.input_onset,
            source_scope: stage.scope.interval,
            output_owner: stage.cover.window(),
        };
        recipes.push(
            sampled_recipe(stage.cover, &stage.identity, stage.trace, input, budget)?
                .ok_or_else(|| invalid("sampled joint recipe remains unresolved"))?,
        );
    }
    let mut mapped = None;
    for (stage, recipe) in stages.iter().zip(&recipes).rev() {
        let source = if let Some(inner) = mapped {
            intersect(stage.scope.interval, inner)?
        } else {
            stage.scope.interval
        };
        let span = sampled_configuration(recipe, source, budget)?
            .ok_or_else(|| invalid("sampled joint constraints have no component"))?;
        mapped = Some(span.map(|t| t.checked_add(stage.output_offset))?);
    }
    let span = mapped.ok_or_else(|| invalid("empty sampled origin chain"))?;
    intersect(owner, span.map(|t| t.checked_add(offset))?)
}

fn joint_stage_configuration(
    stages: &[Stage<'_>],
    mut owner: crate::pattern::TimeSpan,
    offset: Ratio64,
    anchor: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    use super::components::{joint_periodic_component, ClockedPredicate};
    use super::configuration::affine_source_recipe;
    let mut factor = Ratio64::ONE;
    let mut shift = Ratio64::ZERO.checked_sub(offset)?;
    let mut predicates = Vec::new();
    for stage in stages {
        budget.charge(1)?;
        shift = shift.checked_sub(stage.output_offset)?;
        let to_root = |t: Ratio64| t.checked_sub(shift)?.checked_div(factor);
        owner = intersect(owner, stage.cover.window().map(to_root)?)?;
        let Some(recipe) = affine_source_recipe(stage.cover, &stage.identity, stage.trace, budget)?
        else {
            return Ok(None);
        };
        budget.charge(recipe.predicates.len())?;
        predicates.reserve(recipe.predicates.len());
        for predicate in recipe.predicates {
            predicates.push(ClockedPredicate {
                condition: predicate.condition,
                transformed: predicate.transformed,
                factor: predicate.factor.checked_mul(factor)?,
                shift: predicate
                    .factor
                    .checked_mul(shift)?
                    .checked_add(predicate.shift)?,
            });
        }
        factor = recipe.factor.checked_mul(factor)?;
        shift = recipe
            .factor
            .checked_mul(shift)?
            .checked_add(recipe.shift)?;
        owner = intersect(
            owner,
            stage
                .scope
                .interval
                .map(|t| t.checked_sub(shift)?.checked_div(factor))?,
        )?;
    }
    if predicates.len() < 2 {
        return Ok(None);
    }
    joint_periodic_component(&predicates, owner, owner, anchor, budget).map(Some)
}

struct FxPolicyQuery<'a> {
    root: usize,
    track: KwId,
    instrument: &'a FrozenSound,
    onset: Ratio64,
    origin_revision: Option<crate::song::PartRevision>,
    depth: u32,
}
type BorrowedRoute<'a> = (&'a [FrozenSound], KwId);
fn verify_policy(
    plan: &SongRoutePlan,
    event: &crate::song::snapshot::FrozenSongEvent,
    inherited: Option<BorrowedRoute<'_>>,
    budget: &mut ResolutionBudget,
) -> Result<(), Failure> {
    let origin = event.source_origin.as_ref();
    let query = FxPolicyQuery {
        root: plan.topology.root_part,
        track: event.track,
        instrument: origin.map_or(&event.instrument, |o| &o.original_instrument),
        onset: event.handle.occurrence().onset,
        origin_revision: origin.map(|o| o.handle.revision()),
        depth: 0,
    };
    let expected = policy_route(&plan.topology, query, budget)?.or(inherited);
    budget.charge(
        expected
            .map_or(Some(1), |(f, _)| f.len().checked_add(1))
            .ok_or_else(capacity_overflow)?,
    )?;
    let actual = event.route.as_ref().map(|(f, k)| (f.as_slice(), *k));
    if actual != expected {
        return Err(invalid(
            "event FX family/template does not match frozen policy",
        ));
    }
    Ok(())
}
fn policy_route<'a>(
    inventory: &'a crate::song::snapshot::FrozenRoutingInventory,
    mut query: FxPolicyQuery<'_>,
    budget: &mut ResolutionBudget,
) -> Result<Option<BorrowedRoute<'a>>, Failure> {
    use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
    loop {
        query.depth = query.depth.checked_add(1).ok_or_else(capacity_overflow)?;
        budget.enter(query.depth)?;
        let part = inventory
            .parts
            .get(query.root)
            .ok_or_else(|| invalid("FX policy root"))?;
        budget.charge(part.tracks.len())?;
        if query.onset < Ratio64::ZERO
            || query.onset >= part.duration
            || !part.tracks.contains(&query.track)
        {
            return Ok(None);
        }
        match &part.node {
            P::Capture(_) => return Ok(None),
            P::Sequence(children) => {
                let mut found = None;
                for &(begin, child) in children {
                    budget.charge(1)?;
                    let duration = inventory
                        .parts
                        .get(child)
                        .ok_or_else(|| invalid("FX sequence child"))?
                        .duration;
                    let end = begin.checked_add(duration)?;
                    if query.onset >= begin && query.onset < end {
                        found = Some(child);
                        query.onset = query.onset.checked_sub(begin)?;
                        break;
                    }
                }
                query.root = found.ok_or_else(|| invalid("FX sequence membership"))?;
            }
            P::Repeat { child, .. } => {
                let duration = inventory
                    .parts
                    .get(*child)
                    .ok_or_else(|| invalid("FX repeat child"))?
                    .duration;
                if duration == Ratio64::ZERO {
                    return Ok(None);
                }
                let index = query.onset.checked_div(duration)?.floor();
                query.onset = query
                    .onset
                    .checked_sub(duration.checked_mul(Ratio64::from_int(index))?)?;
                query.root = *child;
            }
            P::Edit { source, edit } => {
                if matches!(edit, E::Transform { cutoff, .. } if query.origin_revision==Some(*cutoff))
                {
                    return Ok(None);
                }
                if let E::InstrumentFx {
                    track,
                    family,
                    template,
                } = edit
                {
                    budget.charge(family.len())?;
                    if *track == query.track && family.contains(query.instrument) {
                        return Ok(Some((family.as_slice(), *template)));
                    }
                }
                query.root = *source;
            }
        }
    }
}
fn intersect(
    a: crate::pattern::TimeSpan,
    b: crate::pattern::TimeSpan,
) -> Result<crate::pattern::TimeSpan, Failure> {
    let begin = a.begin.max(b.begin);
    let end = a.end.min(b.end);
    if begin >= end {
        return Err(invalid("nested policy component has no common support"));
    }
    crate::pattern::TimeSpan::new(begin, end)
}

#[cfg(test)]
mod preparation_quota_tests {
    use super::*;
    #[test]
    fn certificate_zero_bounds_have_precise_failures() {
        use crate::vm::fail::FailCode;
        assert_eq!(
            certification_limits(0, 256).unwrap_err().code,
            FailCode::FuelExhausted
        );
        assert_eq!(
            certification_limits(1, 0).unwrap_err().code,
            FailCode::DepthExceeded
        );
        let accepted = certification_limits(1_000_000, 256).unwrap();
        assert_eq!(accepted.max_nodes, 1_000_000);
        assert_eq!(accepted.max_depth, 256);
    }
}
