//! Intrinsic finite source placement and uncut mapped configuration geometry.
mod index;
pub(super) use index::index_configuration;
mod sampling;
use super::components::{
    iterator_component, joint_periodic_component, periodic_component, ClockedPredicate,
};
use super::source::{invalid, node_count, ResolutionBudget};
use super::*;
use crate::vm::fail::Failure;
pub(super) use sampling::{
    map_source_locator, sampled_configuration, sampled_recipe, MembershipSide, SampledRecipeInput,
    SourceLocatorMapping, SourceMembership, SourceTimingContext,
};

pub(super) fn scope_payload(
    inventory: &FrozenRoutingInventory,
    index: usize,
    track: KwId,
) -> Result<&crate::song::snapshot::FrozenPattern, Failure> {
    use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
    match &inventory
        .parts
        .get(index)
        .ok_or_else(|| invalid("payload owner"))?
        .node
    {
        P::Capture(entries) => entries
            .iter()
            .find_map(|(key, value)| (*key == track).then_some(value)),
        P::Edit {
            edit:
                E::Replace {
                    track: key,
                    payload,
                }
                | E::Overwrite {
                    track: key,
                    payload,
                    ..
                }
                | E::Transform {
                    track: key,
                    payload,
                    ..
                },
            ..
        } if *key == track => Some(payload),
        _ => None,
    }
    .ok_or_else(|| invalid("source handle has no matching payload"))
}
pub(super) struct SourceScope<'a> {
    pub interval: crate::pattern::TimeSpan,
    pub payload: &'a crate::song::snapshot::FrozenPattern,
    pub index: usize,
    pub offset: Ratio64,
    pub depth: u32,
}
pub(super) fn source_scope<'a>(
    inventory: &'a FrozenRoutingInventory,
    root: usize,
    handle: &crate::song::EventHandle,
    base_depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<SourceScope<'a>, Failure> {
    use crate::pattern::TimeSpan;
    use crate::song::snapshot::{FrozenEdit as E, FrozenPartNode as P};
    let root_part = inventory
        .parts
        .get(root)
        .ok_or_else(|| invalid("source root"))?;
    if root_part.revision != handle.revision() || !root_part.tracks.contains(&handle.track()) {
        return Err(invalid("source root revision or track mismatch"));
    }
    let mut index = root;
    let mut offset = Ratio64::ZERO;
    let mut cursor = 0usize;
    let mut depth = base_depth.checked_add(1).ok_or_else(capacity_overflow)?;
    let mut support = TimeSpan::new(Ratio64::ZERO, root_part.duration)?;
    let birth = handle.occurrence().onset;
    loop {
        budget.enter(depth)?;
        let part = inventory
            .parts
            .get(index)
            .ok_or_else(|| invalid("source scope"))?;
        match &part.node {
            P::Capture(_) => {
                if cursor != handle.placement().0.len() {
                    return Err(invalid("source placement remainder"));
                }
                let owner = TimeSpan::new(offset, offset.checked_add(part.duration)?)?;
                support =
                    TimeSpan::new(support.begin.max(owner.begin), support.end.min(owner.end))?;
                if birth < support.begin || birth >= support.end {
                    return Err(invalid("source birth outside configuration"));
                }
                return Ok(SourceScope {
                    interval: support,
                    payload: scope_payload(inventory, index, handle.track())?,
                    index,
                    offset,
                    depth,
                });
            }
            P::Sequence(children) => {
                let Some(&[1, ordinal]) = handle.placement().0.get(cursor..cursor + 2) else {
                    return Err(invalid("source sequence placement"));
                };
                let (shift, child) = children
                    .get(ordinal as usize)
                    .ok_or_else(|| invalid("source sequence ordinal"))?;
                offset = offset.checked_add(*shift)?;
                index = *child;
                cursor += 2;
            }
            P::Repeat { child, count, .. } => {
                let Some(&[2, ordinal]) = handle.placement().0.get(cursor..cursor + 2) else {
                    return Err(invalid("source repeat placement"));
                };
                if ordinal >= *count {
                    return Err(invalid("source repeat ordinal"));
                }
                let duration = inventory
                    .parts
                    .get(*child)
                    .ok_or_else(|| invalid("source repeat child"))?
                    .duration;
                offset = offset
                    .checked_add(duration.checked_mul(Ratio64::from_int(i64::from(ordinal)))?)?;
                index = *child;
                cursor += 2;
            }
            P::Edit { source, edit } => {
                let tag = match edit {
                    E::Replace { .. } => Some(3),
                    E::Overwrite { .. } => Some(4),
                    E::Transform { .. } => Some(5),
                    _ => None,
                };
                if let Some(tag) = tag {
                    if handle.placement().0.get(cursor) == Some(&tag) {
                        let count = node_count(inventory, index, depth, budget)?;
                        if handle.placement().0.get(cursor..cursor + 2) != Some(&[tag, count])
                            || cursor + 2 != handle.placement().0.len()
                        {
                            return Err(invalid("source generated placement"));
                        }
                        let owner = TimeSpan::new(offset, offset.checked_add(part.duration)?)?;
                        support = TimeSpan::new(
                            support.begin.max(owner.begin),
                            support.end.min(owner.end),
                        )?;
                        if let E::Overwrite { region, .. } = edit {
                            let region = region.map(|t| t.checked_add(offset))?;
                            support = TimeSpan::new(
                                support.begin.max(region.begin),
                                support.end.min(region.end),
                            )?;
                        }
                        if birth < support.begin || birth >= support.end {
                            return Err(invalid("generated source birth outside configuration"));
                        }
                        return Ok(SourceScope {
                            interval: support,
                            payload: scope_payload(inventory, index, handle.track())?,
                            index,
                            offset,
                            depth,
                        });
                    }
                }
                if let E::Overwrite { track, region, .. } = edit {
                    if *track == handle.track() {
                        let region = region.map(|t| t.checked_add(offset))?;
                        if birth < region.begin {
                            support.end = support.end.min(region.begin);
                        } else if birth >= region.end {
                            support.begin = support.begin.max(region.end);
                        } else {
                            return Err(invalid("source birth overwritten"));
                        }
                    }
                }
                index = *source;
            }
        }
        depth = depth.checked_add(1).ok_or_else(capacity_overflow)?;
    }
}

fn grid_phase_ceiling(time: Ratio64, divisions: i64) -> Result<i128, Failure> {
    let numerator = i128::from(time.num())
        .checked_mul(i128::from(divisions))
        .ok_or_else(capacity_overflow)?;
    let denominator = i128::from(time.den());
    numerator
        .div_euclid(denominator)
        .checked_add(i128::from(numerator.rem_euclid(denominator) != 0))
        .ok_or_else(capacity_overflow)
}
fn grid_phase_time(phase: i128, divisions: i64) -> Result<Ratio64, Failure> {
    let n = i128::from(divisions);
    Ratio64::from_int(i64::try_from(phase.div_euclid(n)).map_err(|_| capacity_overflow())?)
        .checked_add(Ratio64::new(
            i64::try_from(phase.rem_euclid(n)).map_err(|_| capacity_overflow())?,
            divisions,
        )?)
}
fn sample_grid_component(
    recipe: crate::song::source_uses::FrozenStaticSampling,
    source: crate::pattern::TimeSpan,
    owner: crate::pattern::TimeSpan,
    output_anchor: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    use crate::song::source_uses::sampling;
    budget.charge(1)?;
    let phases = budget.with_remaining(|remaining| sampling::enabled_phases(recipe, remaining))?;
    let crate::song::source_uses::FrozenStaticSampling::Euclid { divisions, .. } = recipe;
    if phases.is_empty() {
        return Ok(None);
    }
    let scaled = i128::from(output_anchor.num())
        .checked_mul(i128::from(divisions))
        .ok_or_else(capacity_overflow)?;
    let den = i128::from(output_anchor.den());
    if scaled.rem_euclid(den) != 0 {
        return Err(invalid("grid anchor is not a sample start"));
    }
    let phase = scaled / den;
    let first = grid_phase_ceiling(source.begin, divisions)?;
    let last = grid_phase_ceiling(source.end, divisions)?;
    if phase < first || phase >= last || output_anchor < owner.begin || output_anchor >= owner.end {
        return Ok(None);
    }
    let n = usize::try_from(divisions).map_err(|_| capacity_overflow())?;
    budget.charge(n.checked_add(phases.len()).ok_or_else(capacity_overflow)?)?;
    let mut mask = vec![false; n];
    for &index in &phases {
        mask[index as usize] = true;
    }
    let index = |q: i128| -> Result<usize, Failure> {
        usize::try_from(q.rem_euclid(i128::from(divisions))).map_err(|_| capacity_overflow())
    };
    if !mask[index(phase)?] {
        return Ok(None);
    }
    let (mut begin, mut end) = (first, last);
    if phases.len() != n {
        begin = phase;
        end = phase.checked_add(1).ok_or_else(capacity_overflow)?;
        while begin > first {
            budget.charge(1)?;
            let previous = begin.checked_sub(1).ok_or_else(capacity_overflow)?;
            if !mask[index(previous)?] {
                break;
            }
            begin = previous;
        }
        while end < last {
            budget.charge(1)?;
            if !mask[index(end)?] {
                break;
            }
            end = end.checked_add(1).ok_or_else(capacity_overflow)?;
        }
    }
    let begin = grid_phase_time(begin, divisions)?.max(owner.begin);
    let end = grid_phase_time(end, divisions)?.min(owner.end);
    if begin >= end {
        return Ok(None);
    }
    crate::pattern::TimeSpan::new(begin, end).map(Some)
}

/// Exact affine and Iterate stages; remaining nonlinear stages are added incrementally.
pub(super) struct IntrinsicConfigurationInput {
    pub span: crate::pattern::TimeSpan,
    pub anchor: Ratio64,
    pub source_onset: Ratio64,
    pub output_whole: Option<crate::pattern::TimeSpan>,
}
pub(super) fn map_intrinsic_configuration(
    cover: &crate::song::source_uses::FrozenSourceUseCover,
    identity: &crate::song::source_uses::FrozenSourceUseIdentity,
    trace: &[crate::pattern::occ::ProducerStep],
    input: IntrinsicConfigurationInput,
    budget: &mut ResolutionBudget,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    let IntrinsicConfigurationInput {
        mut span,
        mut anchor,
        source_onset,
        output_whole,
    } = input;
    use crate::pattern::TimeSpan;
    use crate::song::source_uses::{layout, FrozenUseMapping as M};
    if let Some(component) =
        joint_static_configuration(cover, identity, trace, span, anchor, budget)?
    {
        return Ok(Some(component));
    }
    if let Some(mapped) =
        weighted_iterate_configuration(cover, identity, trace, span, source_onset, anchor, budget)?
    {
        return Ok(Some(mapped));
    }
    if let Some(component) = sampling::ordinary_cat_configuration(
        cover,
        identity,
        trace,
        span,
        anchor,
        source_onset,
        budget,
    )? {
        return Ok(Some(component));
    }
    if let Some(whole) = output_whole {
        if let Some(component) = sampling::ordinary_reflection_configuration(
            cover,
            identity,
            trace,
            span,
            whole,
            source_onset,
            budget,
        )? {
            return Ok(Some(component));
        }
    }
    budget.charge(identity.edges.len())?;
    let mut chain = Vec::new();
    let mut index = cover.graph().root;
    let mut window = cover.window();
    let mut cursor = 0usize;
    for &ordinal in &identity.edges {
        budget.charge(1)?;
        let node = cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("mapped node"))?;
        let edge = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("mapped edge"))?;
        let output_anchor = anchor;
        let output_window = window;
        match node.mapping {
            M::Rate { factor } if factor > Ratio64::ZERO => anchor = anchor.checked_mul(factor)?,
            M::Shift { amount } => anchor = anchor.checked_sub(amount)?,
            M::Iterate { count } if count > 0 => {
                let phase = Ratio64::new(
                    anchor.floor().rem_euclid(i64::from(count)),
                    i64::from(count),
                )?;
                anchor = anchor.checked_add(phase)?;
            }
            // Subject Slice changes sample controls/occurrence presence, not
            // its source clock. Only the authenticated subject edge preserves it.
            M::Slices {
                structure: crate::song::source_uses::FrozenSliceStructure::Subject { .. },
                ..
            } if ordinal == 0 => {}
            M::Iterate { count: 0 }
            | M::Preserve
            | M::Parallel
            | M::SelectContent { .. }
            | M::ConditionalStatic(_)
            | M::SampleGrid { .. } => {}
            _ => return Ok(None),
        }
        let end = cursor
            .checked_add(edge.trace.len())
            .ok_or_else(capacity_overflow)?;
        let actual = trace
            .get(cursor..end)
            .ok_or_else(|| invalid("mapped source trace"))?;
        let cycle = anchor.floor();
        let active_layout = if edge.layout.is_empty() {
            false
        } else {
            budget.charge(edge.layout.len())?;
            let unit = layout::slot_interval(edge, 0, actual)?;
            unit.begin != Ratio64::ZERO || unit.end != Ratio64::ONE
        };
        if active_layout {
            budget.charge(edge.layout.len())?;
            let Some(point) =
                layout::map_slot_support(edge, TimeSpan::point(anchor), Some(actual))?
            else {
                return Err(invalid("birth is outside intrinsic slot"));
            };
            anchor = point.begin;
        }
        window = if let M::SampleGrid { sampling, .. } = node.mapping {
            budget
                .with_remaining(|remaining| {
                    crate::song::source_uses::sampling::support_envelope(
                        sampling, window, remaining,
                    )
                })?
                .ok_or_else(|| invalid("empty grid owner support"))?
        } else {
            crate::song::source::mapped_support(node, window)?
        };
        window = layout::map_slot_support(edge, window, Some(actual))?
            .ok_or_else(|| invalid("empty mapped owner support"))?;
        chain.push((
            ordinal,
            node,
            edge,
            actual,
            cycle,
            output_anchor,
            output_window,
            active_layout,
        ));
        index = edge.child;
        cursor = end;
    }
    for (ordinal, node, edge, actual, cycle, output_anchor, output_window, active_layout) in
        chain.into_iter().rev()
    {
        if active_layout {
            let intrinsic = TimeSpan::cycle(cycle)?;
            span = TimeSpan::new(span.begin.max(intrinsic.begin), span.end.min(intrinsic.end))?;
            span = layout::map_slot_configuration(edge, cycle, span, actual)?;
        }
        span = match node.mapping {
            M::Rate { factor } => span.map(|t| t.checked_div(factor))?,
            M::Shift { amount } => span.map(|t| t.checked_add(amount))?,
            M::Iterate { count } => {
                iterator_component(span, count, output_window, output_anchor, budget)?
            }
            M::SampleGrid { sampling, .. } => {
                let Some(component) =
                    sample_grid_component(sampling, span, output_window, output_anchor, budget)?
                else {
                    return Ok(None);
                };
                component
            }
            M::ConditionalStatic(condition) => {
                let Some(component) = periodic_component(
                    condition,
                    ordinal == 1,
                    span,
                    output_window,
                    output_anchor,
                    budget,
                )?
                else {
                    return Ok(None);
                };
                component
            }
            _ => span,
        };
    }
    Ok(Some(span))
}

pub(super) struct AffineSourceRecipe {
    pub factor: Ratio64,
    pub shift: Ratio64,
    pub predicates: Vec<ClockedPredicate>,
}
fn joint_static_configuration(
    cover: &crate::song::source_uses::FrozenSourceUseCover,
    identity: &crate::song::source_uses::FrozenSourceUseIdentity,
    trace: &[crate::pattern::occ::ProducerStep],
    span: crate::pattern::TimeSpan,
    anchor: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    let Some(recipe) = affine_source_recipe(cover, identity, trace, budget)? else {
        return Ok(None);
    };
    if recipe.predicates.len() < 2 {
        return Ok(None);
    }
    let source = span.map(|t| t.checked_sub(recipe.shift)?.checked_div(recipe.factor))?;
    joint_periodic_component(&recipe.predicates, source, cover.window(), anchor, budget).map(Some)
}

pub(super) fn affine_source_recipe(
    cover: &crate::song::source_uses::FrozenSourceUseCover,
    identity: &crate::song::source_uses::FrozenSourceUseIdentity,
    trace: &[crate::pattern::occ::ProducerStep],
    budget: &mut ResolutionBudget,
) -> Result<Option<AffineSourceRecipe>, Failure> {
    use crate::song::source_uses::{layout, FrozenStaticCondition as C, FrozenUseMapping as M};
    budget.charge(identity.edges.len())?;
    let mut predicates = Vec::with_capacity(identity.edges.len());
    let mut index = cover.graph().root;
    let mut cursor = 0usize;
    let mut scale = Ratio64::ONE;
    let mut shift = Ratio64::ZERO;
    for &ordinal in &identity.edges {
        budget.charge(1)?;
        let node = cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("joint condition node"))?;
        match node.mapping {
            M::Rate { factor } if factor > Ratio64::ZERO => {
                scale = scale.checked_mul(factor)?;
                shift = shift.checked_mul(factor)?;
            }
            M::Shift { amount } => {
                shift = shift.checked_sub(amount)?;
            }
            M::ConditionalStatic(condition) if !matches!(condition, C::Chunk { .. }) => predicates
                .push(ClockedPredicate {
                    condition,
                    transformed: ordinal == 1,
                    factor: scale,
                    shift,
                }),
            M::Slices {
                structure: crate::song::source_uses::FrozenSliceStructure::Subject { .. },
                ..
            } if ordinal == 0 => {}
            M::Preserve | M::Parallel | M::SelectContent { .. } => {}
            _ => return Ok(None),
        }
        let edge = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("joint condition edge"))?;
        let end = cursor
            .checked_add(edge.trace.len())
            .ok_or_else(capacity_overflow)?;
        let actual = trace
            .get(cursor..end)
            .ok_or_else(|| invalid("joint condition trace"))?;
        if !edge.layout.is_empty() {
            budget.charge(edge.layout.len())?;
            let unit = layout::slot_interval(edge, 0, actual)?;
            if unit.begin != Ratio64::ZERO || unit.end != Ratio64::ONE {
                return Ok(None);
            }
        }
        index = edge.child;
        cursor = end;
    }
    Ok(Some(AffineSourceRecipe {
        factor: scale,
        shift,
        predicates,
    }))
}

/// First supported inverse recipe: one Iterate and one normalized weighted
/// slot, with preserving wrappers. Candidate cycles come from the exact affine
/// equation, not clipped event pieces. Work depends on slot width, never score
/// duration, symbolic repeat count or Iterate residue count.
fn weighted_iterate_configuration(
    cover: &crate::song::source_uses::FrozenSourceUseCover,
    identity: &crate::song::source_uses::FrozenSourceUseIdentity,
    trace: &[crate::pattern::occ::ProducerStep],
    source: crate::pattern::TimeSpan,
    source_onset: Ratio64,
    mut output_onset: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    use crate::pattern::TimeSpan;
    use crate::song::source_uses::{layout, FrozenUseMapping as M};
    let mut index = cover.graph().root;
    let mut cursor = 0usize;
    let mut count = None;
    let mut owner = cover.window();
    let mut scale = Ratio64::ONE;
    let mut shift = Ratio64::ZERO;
    let mut weighted = None;
    for &ordinal in &identity.edges {
        budget.charge(1)?;
        let node = cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("weighted node"))?;
        match node.mapping {
            M::Rate { factor }
                if factor > Ratio64::ZERO && count.is_none() && weighted.is_none() =>
            {
                output_onset = output_onset.checked_mul(factor)?;
                owner = owner.map(|t| t.checked_mul(factor))?;
                scale = scale.checked_mul(factor)?;
                shift = shift.checked_mul(factor)?;
            }
            M::Shift { amount } if count.is_none() && weighted.is_none() => {
                output_onset = output_onset.checked_sub(amount)?;
                owner = owner.map(|t| t.checked_sub(amount))?;
                shift = shift.checked_sub(amount)?;
            }
            M::Iterate { count: n } if count.is_none() && weighted.is_none() => count = Some(n),
            M::Preserve | M::Parallel | M::SelectContent { .. } => {}
            _ => return Ok(None),
        }
        let edge = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("weighted edge"))?;
        let end = cursor
            .checked_add(edge.trace.len())
            .ok_or_else(capacity_overflow)?;
        let actual = trace
            .get(cursor..end)
            .ok_or_else(|| invalid("weighted trace"))?;
        if !edge.layout.is_empty() {
            if weighted.is_some() {
                return Ok(None);
            }
            budget.charge(edge.layout.len())?;
            weighted = Some((edge, actual));
        }
        cursor = end;
        index = edge.child;
    }
    let Some((edge, actual)) = weighted else {
        return Ok(None);
    };
    let count = count.unwrap_or(0);
    let unit = layout::slot_interval(edge, 0, actual)?;
    let width = unit.end.checked_sub(unit.begin)?;
    let slope = Ratio64::ONE.checked_sub(width)?;
    if slope == Ratio64::ZERO {
        let component = iterator_component(source, count, owner, output_onset, budget)?;
        return Ok(Some(
            component.map(|t| t.checked_sub(shift)?.checked_div(scale))?,
        ));
    }
    let base = output_onset
        .checked_sub(unit.begin)?
        .checked_sub(width.checked_mul(source_onset)?)?;
    let low = base.checked_div(slope)?;
    let high = base.checked_add(Ratio64::ONE)?.checked_div(slope)?;
    let first = low.floor();
    let last = high.floor();
    let mut earliest = None;
    for d in first..=last {
        budget.charge(1)?;
        if Ratio64::from_int(d) < low || Ratio64::from_int(d) >= high {
            continue;
        }
        for epsilon in [0i64, 1] {
            budget.charge(1)?;
            if count == 0 && epsilon != 0 {
                continue;
            }
            let c = d.checked_sub(epsilon).ok_or_else(capacity_overflow)?;
            let phase = if count == 0 {
                Ratio64::ZERO
            } else {
                Ratio64::new(c.rem_euclid(i64::from(count)), i64::from(count))?
            };
            let projected = unit
                .begin
                .checked_add(width.checked_mul(source_onset)?)?
                .checked_add(slope.checked_mul(Ratio64::from_int(d))?)?
                .checked_sub(phase)?;
            if projected != output_onset {
                continue;
            }
            let inner = TimeSpan::cycle(d)?;
            let begin = source.begin.max(inner.begin);
            let end = source.end.min(inner.end);
            if begin >= end {
                continue;
            }
            let mapped =
                layout::map_slot_configuration(edge, d, TimeSpan::new(begin, end)?, actual)?
                    .map(|t| t.checked_sub(phase))?;
            let outer = TimeSpan::cycle(c)?;
            let begin = mapped.begin.max(outer.begin).max(owner.begin);
            let end = mapped.end.min(outer.end).min(owner.end);
            if begin >= end {
                continue;
            }
            let support = TimeSpan::new(begin, end)?;
            // Inverse query end > source onset iff output end > the projected
            // whole onset; this comparison is between output coordinates only.
            if support.begin >= support.end || support.end <= output_onset {
                continue;
            }
            if earliest.is_none_or(|old: TimeSpan| support.begin < old.begin) {
                earliest = Some(support);
            }
        }
    }
    let birth = earliest.ok_or_else(|| invalid("no intrinsic weighted Iterate birth"))?;
    Ok(Some(
        weighted_component(edge, actual, source, count, owner, birth, budget)?
            .map(|t| t.checked_sub(shift)?.checked_div(scale))?,
    ))
}

/// A width below one leaves a nonempty rest in every full outer cycle.
/// Consequently a connected policy component cannot cross an entire intervening
/// cycle: the located cycle and its two neighbors suffice, independent of count.
fn weighted_component(
    edge: &crate::song::source_uses::FrozenSourceUseEdge,
    trace: &[crate::pattern::occ::ProducerStep],
    source: crate::pattern::TimeSpan,
    count: u32,
    owner: crate::pattern::TimeSpan,
    birth: crate::pattern::TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<crate::pattern::TimeSpan, Failure> {
    use crate::pattern::TimeSpan;
    use crate::song::source_uses::layout;
    let cycle = birth.begin.floor();
    let first = cycle.checked_sub(1).ok_or_else(capacity_overflow)?;
    let last = cycle.checked_add(1).ok_or_else(capacity_overflow)?;
    budget.charge(6)?;
    let mut pieces = Vec::with_capacity(6);
    for c in first..=last {
        let outer = TimeSpan::cycle(c)?;
        let phase = if count == 0 {
            Ratio64::ZERO
        } else {
            Ratio64::new(c.rem_euclid(i64::from(count)), i64::from(count))?
        };
        for epsilon in [0i64, 1] {
            if count == 0 && epsilon != 0 {
                continue;
            }
            budget.charge(1)?;
            let d = c.checked_add(epsilon).ok_or_else(capacity_overflow)?;
            let inner = TimeSpan::cycle(d)?;
            let begin = source.begin.max(inner.begin);
            let end = source.end.min(inner.end);
            if begin >= end {
                continue;
            }
            let mapped =
                layout::map_slot_configuration(edge, d, TimeSpan::new(begin, end)?, trace)?
                    .map(|t| t.checked_sub(phase))?;
            let begin = mapped.begin.max(outer.begin).max(owner.begin);
            let end = mapped.end.min(outer.end).min(owner.end);
            if begin < end {
                pieces.push(TimeSpan::new(begin, end)?);
            }
        }
    }
    pieces.sort_unstable_by_key(|piece| piece.begin);
    let mut component = birth;
    for _ in 0..2 {
        for piece in &pieces {
            budget.charge(1)?;
            if piece.begin <= component.end && piece.end >= component.begin {
                component.begin = component.begin.min(piece.begin);
                component.end = component.end.max(piece.end);
            }
        }
    }
    Ok(component)
}

#[cfg(test)]
mod sampled_component_tests {
    use super::*;
    use crate::song::source_uses::FrozenStaticSampling;
    #[test]
    fn shift_is_helper_only_and_borrowed_quota_survives_failure() {
        let source = crate::pattern::TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(2)).unwrap();
        let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
        let recipe = FrozenStaticSampling::Euclid {
            pulses: 4,
            divisions: 4,
            rotation: 0,
        };
        let component = sample_grid_component(
            recipe,
            source,
            source,
            Ratio64::new(1, 4).unwrap(),
            &mut budget,
        )
        .unwrap()
        .unwrap();
        let shifted = component
            .map(|t| t.checked_add(Ratio64::new(1, 3).unwrap()))
            .unwrap();
        assert_eq!(shifted.begin, Ratio64::new(1, 3).unwrap());
        assert_eq!(shifted.end, Ratio64::new(7, 3).unwrap());
        // No public Shift native is claimed; this proves endpoint arithmetic.
        let mut tiny = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: 1,
            ..Default::default()
        });
        let result: Result<(), Failure> = tiny.with_remaining(|remaining| {
            *remaining -= 1;
            Err(invalid("test failure after consumed work"))
        });
        assert!(result.is_err());
        assert_eq!(tiny.limits().max_nodes, 0);
    }
}
