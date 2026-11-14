//! Ordered sampled membership and authenticated emitted-whole clocks.
mod reflection;
use super::super::source::{invalid, ResolutionBudget};
use super::super::*;
use crate::pattern::{occ::ProducerStep, TimeSpan};
use crate::song::source_uses::{
    layout, FrozenSourceUseCover, FrozenSourceUseEdge, FrozenSourceUseIdentity,
    FrozenSourceUseNode, FrozenStaticCondition as C, FrozenStaticSampling, FrozenUseMapping as M,
};
use crate::vm::fail::Failure;
pub(super) use reflection::ordinary_reflection_configuration;
use reflection::{
    inverse_slot_membership, membership_contains, membership_cycle, quantized_membership,
    reflect_membership, reflect_whole, reflected_component_for_member, replay_iterate_membership,
    replay_periodic_membership,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::song::routing) enum MembershipSide {
    At,
    Before,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::song::routing) struct SourceMembership {
    pub point: Ratio64,
    pub side: MembershipSide,
}
#[derive(Clone, Copy)]
pub(in crate::song::routing) struct SourceTimingContext {
    pub incoming_member: Option<SourceMembership>,
    pub output_whole: Option<TimeSpan>,
    pub source_whole: Option<TimeSpan>,
}
pub(in crate::song::routing) enum SourceLocatorMapping {
    UnchangedBirth,
    SampledPoint(SourceMembership),
    NeedsJointGeometry,
}
#[derive(Clone, Copy)]
pub(in crate::song::routing) struct SampledRecipeInput {
    pub output_birth: Ratio64,
    pub incoming_member: Option<SourceMembership>,
    pub output_whole: Option<TimeSpan>,
    pub source_whole: Option<TimeSpan>,
    pub source_birth: Ratio64,
    pub source_scope: TimeSpan,
    pub output_owner: TimeSpan,
}
#[derive(Clone, Copy, Debug)]
enum CycleMembership {
    Identity,
    Reflect {
        cycle: i64,
    },
    Iterate {
        count: u32,
        shift: Ratio64,
    },
    Cat {
        input_cycle: i64,
        shift: Ratio64,
        scale: Ratio64,
    },
}
struct SampledStep<'a> {
    node: &'a FrozenSourceUseNode,
    edge: &'a FrozenSourceUseEdge,
    trace: &'a [ProducerStep],
    ordinal: u32,
    output_member: SourceMembership,
    input_member: SourceMembership,
    slot_cycle: i64,
    cycle_context: CycleMembership,
    output_owner: TimeSpan,
}
pub(in crate::song::routing) struct SampledRecipe<'a> {
    input: SampledRecipeInput,
    steps: Vec<SampledStep<'a>>,
    input_member: SourceMembership,
}

fn intersect(left: TimeSpan, right: TimeSpan) -> Result<TimeSpan, Failure> {
    let begin = left.begin.max(right.begin);
    let end = left.end.min(right.end);
    if begin >= end {
        return Err(invalid("joint sampled constraints have no common support"));
    }
    TimeSpan::new(begin, end)
}

fn quantized_point(point: Ratio64, divisions: i64) -> Result<Ratio64, Failure> {
    if divisions <= 0 {
        return Err(invalid("sampled locator divisions"));
    }
    let numerator = i128::from(point.num())
        .checked_mul(i128::from(divisions))
        .ok_or_else(capacity_overflow)?;
    let phase = numerator.div_euclid(i128::from(point.den()));
    let n = i128::from(divisions);
    Ratio64::from_int(i64::try_from(phase.div_euclid(n)).map_err(|_| capacity_overflow())?)
        .checked_add(Ratio64::new(
            i64::try_from(phase.rem_euclid(n)).map_err(|_| capacity_overflow())?,
            divisions,
        )?)
}

fn charge_slot(edge: &FrozenSourceUseEdge, budget: &mut ResolutionBudget) -> Result<(), Failure> {
    let work = edge
        .layout
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(edge.trace.len()))
        .ok_or_else(capacity_overflow)?;
    budget.charge(work)
}

fn cycle_membership(
    mapping: &M,
    arity: usize,
    ordinal: u32,
    member: SourceMembership,
    budget: &mut ResolutionBudget,
) -> Result<Option<(SourceMembership, CycleMembership)>, Failure> {
    budget.charge(1)?;
    match mapping {
        M::Iterate { count: 0 } => Ok(Some((member, CycleMembership::Identity))),
        M::Iterate { count } => {
            let shift = Ratio64::new(
                membership_cycle(member)?.rem_euclid(i64::from(*count)),
                i64::from(*count),
            )?;
            Ok(Some((
                SourceMembership {
                    point: member.point.checked_add(shift)?,
                    side: member.side,
                },
                CycleMembership::Iterate {
                    count: *count,
                    shift,
                },
            )))
        }
        M::CycleSelect | M::CycleConcat => {
            let n = i64::try_from(arity).map_err(|_| capacity_overflow())?;
            if n == 0 || i64::from(ordinal) >= n {
                return Err(invalid("empty or invalid Cat child"));
            }
            let scale = if matches!(mapping, M::CycleConcat) {
                Ratio64::from_int(n)
            } else {
                Ratio64::ONE
            };
            let expanded = SourceMembership {
                point: member.point.checked_mul(scale)?,
                side: member.side,
            };
            let cycle = membership_cycle(expanded)?;
            if cycle.rem_euclid(n) != i64::from(ordinal) {
                return Err(invalid("Cat membership disagrees with authenticated child"));
            }
            if n == 1 {
                return Ok(Some((member, CycleMembership::Identity)));
            }
            let input_cycle = cycle.div_euclid(n);
            let shift = Ratio64::from_int(cycle).checked_sub(Ratio64::from_int(input_cycle))?;
            Ok(Some((
                SourceMembership {
                    point: expanded.point.checked_sub(shift)?,
                    side: member.side,
                },
                CycleMembership::Cat {
                    input_cycle,
                    shift,
                    scale,
                },
            )))
        }
        _ => Ok(None),
    }
}
fn cat_birth_context(
    mapping: &M,
    arity: usize,
    ordinal: u32,
    output_birth: Ratio64,
    source_birth: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<CycleMembership>, Failure> {
    budget.charge(1)?;
    if !matches!(mapping, M::CycleSelect | M::CycleConcat) {
        return Ok(None);
    }
    let n = i64::try_from(arity).map_err(|_| capacity_overflow())?;
    if n == 0 || i64::from(ordinal) >= n {
        return Err(invalid("invalid Cat birth child"));
    }
    if n == 1 {
        return Ok(Some(CycleMembership::Identity));
    }
    let scale = if matches!(mapping, M::CycleConcat) {
        Ratio64::from_int(n)
    } else {
        Ratio64::ONE
    };
    let q = output_birth
        .checked_mul(scale)?
        .checked_sub(source_birth)?
        .checked_sub(Ratio64::from_int(i64::from(ordinal)))?
        .checked_div(Ratio64::from_int(n - 1))?;
    if q.den() != 1 {
        return Err(invalid(
            "Cat whole does not authenticate an intrinsic cycle",
        ));
    }
    let input_cycle = q.num();
    let cycle = input_cycle
        .checked_mul(n)
        .and_then(|v| v.checked_add(i64::from(ordinal)))
        .ok_or_else(capacity_overflow)?;
    let shift = Ratio64::from_int(cycle).checked_sub(Ratio64::from_int(input_cycle))?;
    Ok(Some(CycleMembership::Cat {
        input_cycle,
        shift,
        scale,
    }))
}
fn replay_cycle_component(
    context: CycleMembership,
    source: TimeSpan,
    owner: TimeSpan,
    member: SourceMembership,
    whole_anchor: &mut Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<TimeSpan, Failure> {
    budget.charge(1)?;
    let component = match context {
        CycleMembership::Reflect { .. } => {
            return Err(invalid("reflection replay needs authentic whole endpoints"))
        }
        CycleMembership::Identity => intersect(source, owner)?,
        CycleMembership::Iterate { count, shift } => {
            *whole_anchor = whole_anchor.checked_sub(shift)?;
            replay_iterate_membership(source, count, owner, member, budget)?
        }
        CycleMembership::Cat {
            input_cycle,
            shift,
            scale,
        } => {
            let clipped = intersect(source, TimeSpan::cycle(input_cycle)?)?;
            *whole_anchor = whole_anchor.checked_add(shift)?.checked_div(scale)?;
            intersect(
                clipped.map(|t| t.checked_add(shift)?.checked_div(scale))?,
                owner,
            )?
        }
    };
    if !membership_contains(component, member) {
        return Err(invalid("cycle mapping excludes authentic membership"));
    }
    Ok(component)
}

pub(super) fn ordinary_cat_configuration(
    cover: &FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &[ProducerStep],
    mut source: TimeSpan,
    output_birth: Ratio64,
    source_birth: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    budget.charge(identity.edges.len())?;
    let mut chain = Vec::with_capacity(identity.edges.len());
    let mut index = cover.graph().root;
    let mut cursor = 0usize;
    let mut window = cover.window();
    let mut anchor = output_birth;
    let mut cats = 0u32;
    for &ordinal in &identity.edges {
        budget.charge(1)?;
        let node = cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("Cat recipe node"))?;
        let edge = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("Cat recipe edge"))?;
        let end = cursor
            .checked_add(edge.trace.len())
            .ok_or_else(capacity_overflow)?;
        let actual = trace
            .get(cursor..end)
            .ok_or_else(|| invalid("Cat recipe trace"))?;
        if !edge.layout.is_empty() {
            charge_slot(edge, budget)?;
            let unit = layout::slot_interval(edge, 0, actual)?;
            if unit.begin != Ratio64::ZERO || unit.end != Ratio64::ONE {
                return Ok(None);
            }
        }
        let output_anchor = anchor;
        let owner = window;
        match node.mapping {
            M::Rate { factor } if factor > Ratio64::ZERO => {
                if cats == 0 {
                    anchor = anchor.checked_mul(factor)?;
                }
            }
            M::Shift { amount } => {
                if cats == 0 {
                    anchor = anchor.checked_sub(amount)?;
                }
            }
            M::CycleSelect | M::CycleConcat => {
                cats += 1;
                if cats > 1 {
                    return Ok(None);
                }
            }
            M::Preserve | M::Parallel | M::SelectContent { .. } => {}
            _ => return Ok(None),
        }
        window = crate::song::source::mapped_support(node, window)?;
        chain.push((node, ordinal, output_anchor, owner));
        index = edge.child;
        cursor = end;
    }
    if cats == 0 {
        return Ok(None);
    }
    let mut whole_anchor = source_birth;
    for (node, ordinal, output_anchor, owner) in chain.into_iter().rev() {
        budget.charge(1)?;
        source = match node.mapping {
            M::Rate { factor } => {
                whole_anchor = whole_anchor.checked_div(factor)?;
                source.map(|t| t.checked_div(factor))?
            }
            M::Shift { amount } => {
                whole_anchor = whole_anchor.checked_add(amount)?;
                source.map(|t| t.checked_add(amount))?
            }
            M::CycleSelect | M::CycleConcat => {
                let context = cat_birth_context(
                    &node.mapping,
                    node.edges.len(),
                    ordinal,
                    output_anchor,
                    whole_anchor,
                    budget,
                )?
                .ok_or_else(|| invalid("missing Cat context"))?;
                let member = match context {
                    CycleMembership::Cat {
                        input_cycle,
                        shift,
                        scale,
                    } => source
                        .begin
                        .max(Ratio64::from_int(input_cycle))
                        .checked_add(shift)?
                        .checked_div(scale)?,
                    _ => source.begin.max(owner.begin),
                };
                replay_cycle_component(
                    context,
                    source,
                    owner,
                    SourceMembership {
                        point: member,
                        side: MembershipSide::At,
                    },
                    &mut whole_anchor,
                    budget,
                )?
            }
            _ => source,
        };
    }
    if whole_anchor != output_birth {
        return Err(invalid(
            "Cat whole replay disagrees with authenticated emission",
        ));
    }
    Ok(Some(intersect(source, cover.window())?))
}

fn sampled_steps<'a>(
    cover: &'a FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &'a [ProducerStep],
    mut point: SourceMembership,
    budget: &mut ResolutionBudget,
) -> Result<Option<Vec<SampledStep<'a>>>, Failure> {
    budget.charge(
        identity
            .edges
            .len()
            .checked_mul(8)
            .ok_or_else(capacity_overflow)?,
    )?;
    let mut steps = Vec::with_capacity(identity.edges.len());
    let mut index = cover.graph().root;
    let mut cursor = 0usize;
    let mut window = cover.window();
    for &ordinal in &identity.edges {
        budget.charge(1)?;
        let node = cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("sampled recipe node"))?;
        let edge = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("sampled recipe edge"))?;
        let output_member = point;
        let output_owner = window;
        let mut cycle_context = CycleMembership::Identity;
        match node.mapping {
            M::Iterate { .. } | M::CycleSelect | M::CycleConcat => {
                let (mapped, context) =
                    cycle_membership(&node.mapping, node.edges.len(), ordinal, point, budget)?
                        .ok_or_else(|| invalid("missing sampled cycle context"))?;
                point = mapped;
                cycle_context = context;
            }
            M::Rate { factor } if factor > Ratio64::ZERO => {
                point.point = point.point.checked_mul(factor)?
            }
            M::Shift { amount } => point.point = point.point.checked_sub(amount)?,
            M::SampleGrid {
                sampling: FrozenStaticSampling::Euclid { divisions, .. },
                ..
            } => {
                budget.charge(2)?;
                point = quantized_membership(point, divisions)?;
            }
            M::ReflectCycles => {
                let cycle = membership_cycle(point)?;
                point = reflect_membership(point, cycle, budget)?;
                cycle_context = CycleMembership::Reflect { cycle };
            }
            M::Preserve | M::Parallel | M::SelectContent { .. } | M::ConditionalStatic(_) => {}
            _ => return Ok(None),
        }
        let slot_cycle = membership_cycle(point)?;
        let end = cursor
            .checked_add(edge.trace.len())
            .ok_or_else(capacity_overflow)?;
        let actual = trace
            .get(cursor..end)
            .ok_or_else(|| invalid("sampled recipe trace"))?;
        if !edge.layout.is_empty() {
            charge_slot(edge, budget)?;
            point = inverse_slot_membership(edge, slot_cycle, actual, point, budget)?;
        }
        window = if let M::SampleGrid { sampling, .. } = node.mapping {
            budget
                .with_remaining(|remaining| {
                    crate::song::source_uses::sampling::support_envelope(
                        sampling, window, remaining,
                    )
                })?
                .ok_or_else(|| invalid("empty sampled owner"))?
        } else {
            crate::song::source::mapped_support(node, window)?
        };
        charge_slot(edge, budget)?;
        window = layout::map_slot_support(edge, window, Some(actual))?
            .ok_or_else(|| invalid("empty sampled slot owner"))?;
        steps.push(SampledStep {
            node,
            edge,
            trace: actual,
            ordinal,
            output_member,
            input_member: point,
            slot_cycle,
            cycle_context,
            output_owner,
        });
        cursor = end;
        index = edge.child;
    }
    Ok(Some(steps))
}

pub(in crate::song::routing) fn map_source_locator(
    cover: &FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &[ProducerStep],
    output_birth: Ratio64,
    timing: SourceTimingContext,
    budget: &mut ResolutionBudget,
) -> Result<SourceLocatorMapping, Failure> {
    let incoming_sampled = timing.incoming_member;
    let mut index = cover.graph().root;
    let mut owns_grid = false;
    for &ordinal in &identity.edges {
        budget.charge(1)?;
        let node = cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("sampled locator node"))?;
        owns_grid |= matches!(node.mapping, M::SampleGrid { .. });
        index = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("sampled locator edge"))?
            .child;
    }
    if incoming_sampled.is_none() && !owns_grid {
        return Ok(SourceLocatorMapping::UnchangedBirth);
    }
    let Some(steps) = sampled_steps(
        cover,
        identity,
        trace,
        incoming_sampled.unwrap_or(SourceMembership {
            point: output_birth,
            side: MembershipSide::At,
        }),
        budget,
    )?
    else {
        return Ok(SourceLocatorMapping::NeedsJointGeometry);
    };
    let member = steps.last().map_or(
        incoming_sampled.unwrap_or(SourceMembership {
            point: output_birth,
            side: MembershipSide::At,
        }),
        |s| s.input_member,
    );
    budget.charge(2)?;
    if timing
        .source_whole
        .is_some_and(|whole| !membership_contains(whole, member))
    {
        return Err(invalid("sampled locator outside authentic source whole"));
    }
    if steps
        .iter()
        .any(|step| matches!(step.cycle_context, CycleMembership::Reflect { .. }))
        && (timing.source_whole.is_none() || timing.output_whole.is_none())
    {
        return Err(invalid(
            "sampled reflection requires authentic whole clocks",
        ));
    }
    Ok(SourceLocatorMapping::SampledPoint(member))
}

pub(in crate::song::routing) fn sampled_recipe<'a>(
    cover: &'a FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &'a [ProducerStep],
    input: SampledRecipeInput,
    budget: &mut ResolutionBudget,
) -> Result<Option<SampledRecipe<'a>>, Failure> {
    let point = input.incoming_member.unwrap_or(SourceMembership {
        point: input.output_birth,
        side: MembershipSide::At,
    });
    let Some(steps) = sampled_steps(cover, identity, trace, point, budget)? else {
        return Ok(None);
    };
    let input_member = steps.last().map_or(point, |s| s.input_member);
    Ok(Some(SampledRecipe {
        input,
        steps,
        input_member,
    }))
}

fn sampled_chunk_run(
    divisions: i64,
    transformed: bool,
    member: SourceMembership,
    whole_anchor: Ratio64,
) -> Result<TimeSpan, Failure> {
    if divisions <= 0 {
        if transformed {
            return Err(invalid("nonpositive Chunk transformed edge"));
        }
        return Err(invalid("constant Chunk is handled by its owner"));
    }
    let selected = whole_anchor
        .frac()
        .checked_mul(Ratio64::from_int(divisions))?
        .floor();
    let cycle = membership_cycle(member)?;
    let residue = cycle.rem_euclid(divisions);
    if (residue == selected) != transformed {
        return Err(invalid(
            "Chunk sampled edge disagrees with child whole anchor",
        ));
    }
    let begin = if transformed {
        Ratio64::from_int(cycle)
    } else {
        let since = residue
            .checked_sub(selected)
            .ok_or_else(capacity_overflow)?
            .rem_euclid(divisions);
        Ratio64::from_int(cycle).checked_sub(Ratio64::from_int(since - 1))?
    };
    let width = if transformed { 1 } else { divisions - 1 };
    TimeSpan::new(begin, begin.checked_add(Ratio64::from_int(width))?)
}

pub(in crate::song::routing) fn sampled_configuration(
    recipe: &SampledRecipe<'_>,
    source: TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    // All inherited recipes have already been collected. Every constraint here
    // must contain the authentic point; no independent first-run search occurs.
    let mut span = intersect(source, recipe.input.source_scope)?;
    if !membership_contains(span, recipe.input_member) {
        return Err(invalid(
            "sampled point outside complete source configuration",
        ));
    }
    budget.charge(4)?;
    let mut whole_anchor = recipe.input.source_birth;
    let mut whole = recipe.input.source_whole;
    for step in recipe.steps.iter().rev() {
        budget.charge(1)?;
        if whole.is_some() {
            budget.charge(4)?;
        }
        if !step.edge.layout.is_empty() {
            charge_slot(step.edge, budget)?;
            let unit = layout::slot_interval(step.edge, 0, step.trace)?;
            if unit.begin != Ratio64::ZERO || unit.end != Ratio64::ONE {
                span = intersect(span, TimeSpan::cycle(step.slot_cycle)?)?;
                charge_slot(step.edge, budget)?;
                span =
                    layout::map_slot_configuration(step.edge, step.slot_cycle, span, step.trace)?;
                whole = whole
                    .map(|w| {
                        layout::map_slot_configuration(step.edge, step.slot_cycle, w, step.trace)
                    })
                    .transpose()?;
                charge_slot(step.edge, budget)?;
                whole_anchor = layout::map_slot_configuration(
                    step.edge,
                    step.slot_cycle,
                    TimeSpan::point(whole_anchor),
                    step.trace,
                )?
                .begin;
            }
        }
        span = match step.node.mapping {
            M::Iterate { .. } | M::CycleSelect | M::CycleConcat => {
                whole = whole
                    .map(|w| match step.cycle_context {
                        CycleMembership::Iterate { shift, .. } => w.map(|t| t.checked_sub(shift)),
                        CycleMembership::Cat { shift, scale, .. } => {
                            w.map(|t| t.checked_add(shift)?.checked_div(scale))
                        }
                        _ => Ok(w),
                    })
                    .transpose()?;
                replay_cycle_component(
                    step.cycle_context,
                    span,
                    step.output_owner,
                    step.output_member,
                    &mut whole_anchor,
                    budget,
                )?
            }
            M::ReflectCycles => {
                let CycleMembership::Reflect { cycle } = step.cycle_context else {
                    return Err(invalid("missing sampled reflection cycle"));
                };
                let reflected = reflect_whole(
                    whole.ok_or_else(|| {
                        invalid("sampled reflection requires authentic source whole")
                    })?,
                    cycle,
                    budget,
                )?;
                whole_anchor = reflected.begin;
                whole = Some(reflected);
                reflected_component_for_member(
                    span,
                    step.output_owner,
                    cycle,
                    step.output_member,
                    budget,
                )?
            }
            M::Rate { factor } => {
                whole_anchor = whole_anchor.checked_div(factor)?;
                whole = whole
                    .map(|w| w.map(|t| t.checked_div(factor)))
                    .transpose()?;
                span.map(|t| t.checked_div(factor))?
            }
            M::Shift { amount } => {
                whole_anchor = whole_anchor.checked_add(amount)?;
                whole = whole
                    .map(|w| w.map(|t| t.checked_add(amount)))
                    .transpose()?;
                span.map(|t| t.checked_add(amount))?
            }
            M::SampleGrid { sampling, .. } => {
                let FrozenStaticSampling::Euclid { divisions, .. } = sampling;
                let anchor = quantized_membership(step.output_member, divisions)?.point;
                let Some(component) = super::sample_grid_component(
                    sampling,
                    span,
                    step.output_owner,
                    anchor,
                    budget,
                )?
                else {
                    return Ok(None);
                };
                whole_anchor = anchor;
                whole = Some(TimeSpan::new(
                    anchor,
                    anchor.checked_add(Ratio64::new(1, divisions)?)?,
                )?);
                component
            }
            M::ConditionalStatic(C::Chunk { divisions }) => {
                if divisions <= 0 || divisions == 1 {
                    if (divisions == 1) != (step.ordinal == 1) {
                        return Err(invalid("constant sampled Chunk edge"));
                    }
                    span
                } else {
                    let run = sampled_chunk_run(
                        divisions,
                        step.ordinal == 1,
                        step.output_member,
                        whole_anchor,
                    )?;
                    intersect(span, run)?
                }
            }
            M::ConditionalStatic(condition) => {
                let Some(component) = replay_periodic_membership(
                    condition,
                    step.ordinal == 1,
                    span,
                    step.output_owner,
                    step.output_member,
                    budget,
                )?
                else {
                    return Ok(None);
                };
                component
            }
            _ => span,
        };
        if !membership_contains(span, step.output_member) {
            return Err(invalid(
                "joint sampled constraint excludes authentic membership",
            ));
        }
    }
    span = intersect(span, recipe.input.output_owner)?;
    if recipe
        .input
        .output_whole
        .is_some_and(|expected| whole != Some(expected))
    {
        return Err(invalid(
            "sampled whole endpoints disagree with authenticated emission",
        ));
    }
    if whole_anchor != recipe.input.output_birth {
        return Err(invalid(
            "sampled whole replay disagrees with authenticated emission",
        ));
    }
    Ok(Some(span))
}

#[cfg(test)]
mod cycle_reflection_tests {
    use super::*;
    use crate::song::SongLimits;
    use crate::vm::fail::FailCode;
    fn at(point: Ratio64) -> SourceMembership {
        SourceMembership {
            point,
            side: MembershipSide::At,
        }
    }

    #[test]
    fn signed_cycle_membership_replays_cat_fastcat_and_iterate_under_shared_fuel() {
        for (mapping, member, expected) in [
            (M::CycleSelect, Ratio64::new(-11, 4).unwrap(), (-3, -2, 1)),
            (M::CycleConcat, Ratio64::new(-11, 8).unwrap(), (-3, -2, 2)),
        ] {
            for nodes in [2, 3] {
                let mut budget = ResolutionBudget::new(SongLimits {
                    max_nodes: nodes,
                    ..Default::default()
                });
                let (input, context) = cycle_membership(&mapping, 2, 1, at(member), &mut budget)
                    .unwrap()
                    .unwrap();
                assert_eq!(input.point, Ratio64::new(-7, 4).unwrap());
                let birth = Ratio64::new(expected.0, expected.2).unwrap();
                let authentic =
                    cat_birth_context(&mapping, 2, 1, birth, Ratio64::from_int(-2), &mut budget)
                        .unwrap()
                        .unwrap();
                match (context, authentic) {
                    (
                        CycleMembership::Cat {
                            input_cycle: a,
                            shift: b,
                            scale: c,
                        },
                        CycleMembership::Cat {
                            input_cycle: d,
                            shift: e,
                            scale: f,
                        },
                    ) => assert_eq!((a, b, c), (d, e, f)),
                    _ => panic!("actual Cat context required"),
                }
                let mut whole = Ratio64::from_int(-2);
                let result = replay_cycle_component(
                    context,
                    TimeSpan::new(Ratio64::from_int(-2), Ratio64::from_int(-1)).unwrap(),
                    TimeSpan::new(Ratio64::from_int(-4), Ratio64::ZERO).unwrap(),
                    at(member),
                    &mut whole,
                    &mut budget,
                );
                if nodes == 2 {
                    assert_eq!(result.unwrap_err().code, FailCode::FuelExhausted);
                } else {
                    assert_eq!(
                        result.unwrap(),
                        TimeSpan::new(birth, Ratio64::new(expected.1, expected.2).unwrap())
                            .unwrap()
                    );
                    assert_eq!(whole, birth);
                    assert_eq!(budget.limits().max_nodes, 0);
                }
            }
        }
        let mut budget = ResolutionBudget::new(SongLimits::default());
        let member = Ratio64::new(-3, 4).unwrap();
        let (input, context) =
            cycle_membership(&M::Iterate { count: 4 }, 1, 0, at(member), &mut budget)
                .unwrap()
                .unwrap();
        assert_eq!(input.point, Ratio64::ZERO);
        let mut whole = Ratio64::ZERO;
        let component = replay_cycle_component(
            context,
            TimeSpan::cycle(0).unwrap(),
            TimeSpan::cycle(-1).unwrap(),
            at(member),
            &mut whole,
            &mut budget,
        )
        .unwrap();
        assert_eq!(component, TimeSpan::new(member, Ratio64::ZERO).unwrap());
        assert_eq!(whole, member);
        for mapping in [M::Iterate { count: 0 }, M::CycleSelect, M::CycleConcat] {
            let (input, context) = cycle_membership(&mapping, 1, 0, at(member), &mut budget)
                .unwrap()
                .unwrap();
            assert_eq!(input.point, member);
            assert!(matches!(context, CycleMembership::Identity));
        }
        assert_eq!(
            cycle_membership(&M::CycleSelect, 2, 2, at(member), &mut budget)
                .unwrap_err()
                .code,
            FailCode::Type
        );
        assert_eq!(
            cat_birth_context(
                &M::CycleSelect,
                2,
                0,
                Ratio64::from_int(i64::MAX),
                Ratio64::ZERO,
                &mut budget
            )
            .unwrap_err()
            .code,
            FailCode::Overflow
        );
    }
}
