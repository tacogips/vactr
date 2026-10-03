//! Prepared operand authority and symbolic Index support; no VM execution.
use super::source::{invalid, ResolutionBudget};
use super::*;
use crate::pattern::occ::{ProducerKind, ProducerStep};
use crate::pattern::TimeSpan;
use crate::song::source_uses::{
    timing::{FrozenIndexDynamicKind, FrozenIndexLeaf, FrozenIndexTiming, FrozenIndexTimingNode},
    FrozenPattern, FrozenSliceTiming, FrozenUseOperation, FrozenUseTraceTerm,
};
use crate::song::SongSnapshot;
use crate::vm::fail::{FailCode, Failure};

mod static_families;
pub(super) use static_families::{compile_static_families, StaticIndexFamily};

#[derive(Clone, Copy)]
pub(super) struct OutputOperandOwner<'a> {
    payload: &'a FrozenPattern,
    revision: crate::song::PartRevision,
    track: KwId,
    scope_part: usize,
}
impl<'a> OutputOperandOwner<'a> {
    pub(super) fn payload(self) -> &'a FrozenPattern {
        self.payload
    }
}
pub(super) fn output_operand_owner<'a>(
    inventory: &'a FrozenRoutingInventory,
    scope_part: usize,
    track: KwId,
    budget: &mut ResolutionBudget,
) -> Result<OutputOperandOwner<'a>, Failure> {
    let part = inventory
        .parts
        .get(scope_part)
        .ok_or_else(|| invalid("Index output owner"))?;
    let inspections = match &part.node {
        crate::song::snapshot::FrozenPartNode::Capture(entries) => entries.len(),
        _ => 1,
    };
    budget.charge(inspections)?;
    let payload = super::configuration::scope_payload(inventory, scope_part, track)?;
    Ok(OutputOperandOwner {
        payload,
        revision: part.revision,
        track,
        scope_part,
    })
}
#[derive(Clone, Copy)]
pub(super) struct PreparedSliceAddress<'a> {
    pub(super) owner: OutputOperandOwner<'a>,
    pub(super) recipe: &'a FrozenIndexTiming,
    pub(super) slice: u32,
    pub(super) index_root: u32,
    pub(super) prefix: &'a [FrozenUseTraceTerm],
}
pub(super) struct BoundSliceOperands<'a> {
    pub(super) prepared: PreparedSliceAddress<'a>,
    pub(super) timing: &'a FrozenSliceTiming,
    pub(super) unresolved_path: Option<(u32, FrozenIndexDynamicKind)>,
}
impl<'a> std::ops::Deref for BoundSliceOperands<'a> {
    type Target = PreparedSliceAddress<'a>;
    fn deref(&self) -> &Self::Target {
        &self.prepared
    }
}
pub(super) struct IndexRealizationRequest<'a> {
    pub(super) prepared: PreparedSliceAddress<'a>,
    pub(super) timing: Option<&'a FrozenSliceTiming>,
    pub(super) operand: u32,
    pub(super) kind: FrozenIndexDynamicKind,
}
impl IndexRealizationRequest<'_> {
    pub(super) fn failure(&self) -> Failure {
        let owner = self.prepared.owner;
        Failure::new(FailCode::BeyondCapability, format!(
            "Index requires shared canonical realization: output_scope={}, revision={:?}, track={:?}, root={}, slice={}, prefix={:?}, operand={}, kind={:?}, issuance={:?}",
            owner.scope_part, owner.revision, owner.track, self.prepared.recipe.root(), self.prepared.slice, self.prepared.prefix, self.operand, self.kind, self.timing,
        ))
    }
}

/// Bind a structural address before any query or event issuance. Both the
/// topology owner and the complete symbolic path belong to the original output.
pub(super) fn prepare_slice_operands<'a>(
    owner: OutputOperandOwner<'a>,
    issuer: crate::reader::span::NodeId,
    prefix: &'a [FrozenUseTraceTerm],
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<PreparedSliceAddress<'a>, Failure> {
    let recipe = owner
        .payload
        .index_timing()
        .ok_or_else(|| invalid("prepared output has no operand recipe"))?;
    if node(recipe, recipe.root())?.issuer() != owner.payload.id {
        return Err(invalid("original output recipe root mismatch"));
    }
    budget.charge(1)?;
    let mut pending = vec![(recipe.root(), 0usize, depth)];
    let mut found = None;
    while let Some((index, cursor, depth)) = pending.pop() {
        budget.enter(depth)?;
        let current = node(recipe, index)?;
        if current.issuer() == issuer
            && cursor == prefix.len()
            && matches!(
                current.operation(),
                FrozenUseOperation::Slice | FrozenUseOperation::Splice
            )
            && found.replace(index).is_some()
        {
            return Err(invalid("ambiguous symbolic Slice address"));
        }
        for child in current.children() {
            budget.charge(
                child
                    .trace()
                    .len()
                    .checked_add(1)
                    .ok_or_else(capacity_overflow)?,
            )?;
            let end = cursor
                .checked_add(child.trace().len())
                .ok_or_else(capacity_overflow)?;
            if prefix.get(cursor..end) == Some(child.trace()) {
                budget.charge(1)?;
                pending.push((
                    child.child(),
                    end,
                    depth.checked_add(1).ok_or_else(capacity_overflow)?,
                ));
            }
        }
    }
    let slice = found.ok_or_else(|| invalid("Slice absent from complete symbolic output path"))?;
    let index_root = slice_index_root(recipe, slice, budget)?;
    Ok(PreparedSliceAddress {
        owner,
        recipe,
        slice,
        index_root,
        prefix,
    })
}
fn slice_index_root(
    recipe: &FrozenIndexTiming,
    slice: u32,
    budget: &mut ResolutionBudget,
) -> Result<u32, Failure> {
    let current = node(recipe, slice)?;
    budget.charge(
        current
            .children()
            .len()
            .checked_mul(2)
            .ok_or_else(capacity_overflow)?,
    )?;
    let subject = current
        .children()
        .iter()
        .find(|c| c.role() == 0)
        .ok_or_else(|| invalid("Slice subject operand"))?;
    let index = current
        .children()
        .iter()
        .find(|c| c.role() == 1)
        .ok_or_else(|| invalid("Slice Index operand"))?;
    if node(recipe, subject.child())?.structured() || !node(recipe, index.child())?.structured() {
        return Err(invalid("Index address contradicts first-structure rule"));
    }
    Ok(index.child())
}

fn node(recipe: &FrozenIndexTiming, index: u32) -> Result<&FrozenIndexTimingNode, Failure> {
    recipe
        .nodes()
        .get(index as usize)
        .ok_or_else(|| invalid("Index operand reference"))
}
pub(super) fn trace_end(
    terms: &[FrozenUseTraceTerm],
    actual: &[ProducerStep],
    start: usize,
    budget: &mut ResolutionBudget,
) -> Result<Option<usize>, Failure> {
    let mut cursor = start;
    for term in terms {
        budget.charge(1)?;
        let Some(step) = actual.get(cursor) else {
            return Ok(None);
        };
        let matches = match term {
            FrozenUseTraceTerm::Exact(expected) => step == expected,
            FrozenUseTraceTerm::Copies { kind, count } => {
                step.kind == *kind && step.ordinal < *count
            }
        };
        if !matches {
            return Ok(None);
        }
        cursor = cursor.checked_add(1).ok_or_else(capacity_overflow)?;
    }
    Ok(Some(cursor))
}
fn find_issuer(
    recipe: &FrozenIndexTiming,
    issuer: crate::reader::span::NodeId,
    prefix: &[ProducerStep],
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<u32, Failure> {
    budget.charge(1)?;
    let mut pending = vec![(recipe.root(), 0usize, depth)];
    let mut found = None;
    while let Some((index, cursor, depth)) = pending.pop() {
        budget.enter(depth)?;
        let current = node(recipe, index)?;
        if current.issuer() == issuer
            && cursor == prefix.len()
            && matches!(
                current.operation(),
                FrozenUseOperation::Slice | FrozenUseOperation::Splice
            )
            && found.replace(index).is_some()
        {
            return Err(invalid("ambiguous complete Slice operand path"));
        }
        for child in current.children() {
            budget.charge(1)?;
            if let Some(next) = trace_end(child.trace(), prefix, cursor, budget)? {
                budget.charge(1)?;
                pending.push((
                    child.child(),
                    next,
                    depth.checked_add(1).ok_or_else(capacity_overflow)?,
                ));
            }
        }
    }
    found.ok_or_else(|| invalid("Slice issuer absent from original output root/path"))
}
fn validate_index_path(
    recipe: &FrozenIndexTiming,
    root: u32,
    trace: &[ProducerStep],
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<Option<(u32, FrozenIndexDynamicKind)>, Failure> {
    budget.charge(1)?;
    let mut pending = vec![(root, 0usize, depth)];
    let mut found = false;
    let mut unresolved = None;
    while let Some((index, cursor, depth)) = pending.pop() {
        budget.enter(depth)?;
        let current = node(recipe, index)?;
        if let Some(end) = trace_end(current.leaf_trace(), trace, cursor, budget)? {
            if let Some(FrozenIndexLeaf::Dynamic(kind)) = current.leaf() {
                // A genuine dynamic result may append producer descendants that
                // do not exist in the copied operand recipe. Retain the original
                // issued authority; this is not a static path validation success.
                if end <= trace.len() {
                    unresolved = Some((index, *kind));
                }
            } else if current.leaf().is_some() && end == trace.len() {
                found = true;
            }
        }
        for child in current.children() {
            budget.charge(1)?;
            if let Some(next) = trace_end(child.trace(), trace, cursor, budget)? {
                budget.charge(1)?;
                pending.push((
                    child.child(),
                    next,
                    depth.checked_add(1).ok_or_else(capacity_overflow)?,
                ));
            }
        }
    }
    if unresolved.is_some() || found {
        Ok(unresolved)
    } else {
        Err(invalid(
            "issued Index trace differs from prepared operand path",
        ))
    }
}
pub(super) fn bind_slice_operands<'a>(
    prepared: PreparedSliceAddress<'a>,
    timing: &'a FrozenSliceTiming,
    subject: &crate::song::EventHandle,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<BoundSliceOperands<'a>, Failure> {
    budget.enter(depth)?;
    budget.charge(
        timing
            .subject_handle()
            .placement()
            .0
            .len()
            .checked_add(timing.subject_handle().occurrence().producer_ordinals.len())
            .and_then(|n| n.checked_add(subject.placement().0.len()))
            .and_then(|n| n.checked_add(subject.occurrence().producer_ordinals.len()))
            .ok_or_else(capacity_overflow)?,
    )?;
    if timing.subject_handle() != subject {
        return Err(invalid("Slice subject handle/track authority mismatch"));
    }
    let recipe = prepared.recipe;
    let owner = prepared.owner;
    if trace_end(prepared.prefix, timing.issuer_trace(), 0, budget)?
        != Some(timing.issuer_trace().len())
    {
        return Err(invalid("issued Slice prefix differs from prepared address"));
    }
    if node(recipe, recipe.root())?.issuer() != owner.payload.id {
        return Err(invalid("original output recipe root mismatch"));
    }
    if timing.sample_start() != timing.index_whole().begin
        || timing.index_whole().end <= timing.index_whole().begin
    {
        return Err(invalid("issued Index whole/sample start mismatch"));
    }
    let slice = find_issuer(
        recipe,
        timing.issuer(),
        timing.issuer_trace(),
        depth,
        budget,
    )?;
    if slice != prepared.slice {
        return Err(invalid(
            "issued Slice differs from prepared operand address",
        ));
    }
    let index_root = slice_index_root(recipe, slice, budget)?;
    let prefix = timing.issuer_trace();
    budget.charge(prefix.len())?;
    if !timing.index_trace().starts_with(prefix)
        || timing.index_trace().get(prefix.len())
            != Some(&ProducerStep {
                kind: ProducerKind::Child,
                ordinal: 1,
            })
    {
        return Err(invalid("issued Index root prefix mismatch"));
    }
    let unresolved_path = validate_index_path(
        recipe,
        index_root,
        &timing.index_trace()[prefix.len() + 1..],
        depth.checked_add(1).ok_or_else(capacity_overflow)?,
        budget,
    )?;
    Ok(BoundSliceOperands {
        prepared,
        timing,
        unresolved_path,
    })
}

pub(super) enum IndexSupportAdmission<'a> {
    Static(StaticIndexSupport),
    RequiresRealization(IndexRealizationRequest<'a>),
}
pub(super) struct StaticIndexSupport {
    pub(super) root: u32,
    pub(super) nodes: Vec<IndexNodeSupport>,
}
#[derive(Clone, Copy)]
pub(super) struct IndexNodeSupport {
    pub(super) empty: bool,
    pub(super) full: bool,
    pub(super) multiplicity: u64,
    pub(super) openings: u64,
    pub(super) slope: Ratio64,
    pub(super) max_whole: Ratio64,
    pub(super) event_slope: Ratio64,
    pub(super) event_burst: u64,
}

fn realization<'a>(
    prepared: PreparedSliceAddress<'a>,
    timing: Option<&'a FrozenSliceTiming>,
    operand: u32,
    kind: FrozenIndexDynamicKind,
) -> IndexSupportAdmission<'a> {
    IndexSupportAdmission::RequiresRealization(IndexRealizationRequest {
        prepared,
        timing,
        operand,
        kind,
    })
}

fn value(
    number: crate::song::source_uses::timing::FrozenIndexNumber,
) -> crate::value::value::Value {
    use crate::song::source_uses::timing::FrozenIndexNumber as N;
    use crate::value::value::Value;
    match number {
        N::Int(v) => Value::Int(v),
        N::Int64(v) => Value::Int64(v),
        N::Ratio(v) => Value::Ratio(v),
        N::FloatBits(v) => Value::Float(f32::from_bits(v)),
        N::Float64Bits(v) => Value::Float64(f64::from_bits(v)),
    }
}
pub(super) fn static_number(
    parameter: &crate::song::source_uses::timing::FrozenIndexParameter,
) -> Option<Ratio64> {
    if let crate::song::source_uses::timing::FrozenIndexParameter::Number(number) = parameter {
        crate::pattern::eval::num_ratio(&value(*number))
    } else {
        None
    }
}
fn ceil(value: Ratio64) -> Result<u64, Failure> {
    if value < Ratio64::ZERO {
        return Err(invalid("negative Index opening bound"));
    }
    let floor = value.floor();
    u64::try_from(
        floor
            .checked_add(i64::from(value.frac() != Ratio64::ZERO))
            .ok_or_else(capacity_overflow)?,
    )
    .map_err(|_| capacity_overflow())
}
impl IndexNodeSupport {
    fn empty() -> Self {
        Self {
            empty: true,
            full: false,
            multiplicity: 0,
            openings: 0,
            slope: Ratio64::ZERO,
            max_whole: Ratio64::ZERO,
            event_slope: Ratio64::ZERO,
            event_burst: 0,
        }
    }
    fn occupied() -> Self {
        Self {
            empty: false,
            full: true,
            multiplicity: 1,
            openings: 0,
            slope: Ratio64::ZERO,
            max_whole: Ratio64::ONE,
            event_slope: Ratio64::ONE,
            event_burst: 2,
        }
    }
    pub(super) fn starts(self, width: Ratio64) -> Result<u64, Failure> {
        ceil(self.event_slope.checked_mul(width)?)?
            .checked_add(self.event_burst)
            .ok_or_else(capacity_overflow)
    }
    fn fragmented(self, count: u64) -> Result<Self, Failure> {
        if self.empty {
            return Ok(self);
        }
        let openings = self
            .openings
            .checked_add(ceil(self.slope)?)
            .and_then(|n| n.checked_add(2))
            .and_then(|n| n.checked_mul(count))
            .ok_or_else(capacity_overflow)?;
        Ok(Self {
            full: false,
            slope: Ratio64::from_int(i64::try_from(openings).map_err(|_| capacity_overflow())?),
            openings,
            ..self
        })
    }
}
fn operand_leaf_height(
    leaf: &FrozenIndexLeaf,
    prior: &[u32],
    budget: &mut ResolutionBudget,
) -> Result<u32, Failure> {
    budget.charge(1)?;
    let mut pending = vec![(leaf, 1u32)];
    let mut height = 1;
    while let Some((leaf, depth)) = pending.pop() {
        budget.charge(1)?;
        height = height.max(depth);
        match leaf {
            FrozenIndexLeaf::Pattern { child } => {
                let child = prior
                    .get(*child as usize)
                    .ok_or_else(|| invalid("Index height forward reference"))?;
                height = height.max(depth.checked_add(*child).ok_or_else(capacity_overflow)?);
            }
            FrozenIndexLeaf::AtomicList { elements } => {
                budget.charge(elements.len())?;
                let next = depth.checked_add(1).ok_or_else(capacity_overflow)?;
                for element in elements {
                    pending.push((element, next));
                }
            }
            _ => {}
        }
    }
    Ok(height)
}
fn operand_parameter_height(
    parameter: &crate::song::source_uses::timing::FrozenIndexParameter,
    prior: &[u32],
    budget: &mut ResolutionBudget,
) -> Result<u32, Failure> {
    use crate::song::source_uses::timing::FrozenIndexParameter as P;
    budget.charge(1)?;
    match parameter {
        P::Pattern { child } => prior
            .get(*child as usize)
            .copied()
            .ok_or_else(|| invalid("Index parameter height forward reference")),
        P::Literal(leaf) => operand_leaf_height(leaf, prior, budget),
        P::Number(_) | P::Dynamic(_) => Ok(1),
    }
}
fn operand_heights(
    recipe: &FrozenIndexTiming,
    budget: &mut ResolutionBudget,
) -> Result<Vec<u32>, Failure> {
    budget.charge(recipe.nodes().len())?;
    let mut heights = Vec::with_capacity(recipe.nodes().len());
    for current in recipe.nodes() {
        budget.charge(1)?;
        let mut height = 1;
        for child in current.children() {
            budget.charge(1)?;
            let child = heights
                .get(child.child() as usize)
                .copied()
                .ok_or_else(|| invalid("Index height forward child"))?;
            height = height.max(1u32.checked_add(child).ok_or_else(capacity_overflow)?);
        }
        if let Some(leaf) = current.leaf() {
            height = height.max(operand_leaf_height(leaf, &heights, budget)?);
        }
        for parameter in current.parameters() {
            height = height.max(
                1u32.checked_add(operand_parameter_height(parameter, &heights, budget)?)
                    .ok_or_else(capacity_overflow)?,
            );
        }
        for slot in current.slots() {
            height = height.max(
                1u32.checked_add(operand_parameter_height(slot.weight(), &heights, budget)?)
                    .ok_or_else(capacity_overflow)?,
            );
            if let Some(count) = slot.count() {
                height = height.max(
                    1u32.checked_add(operand_parameter_height(count, &heights, budget)?)
                        .ok_or_else(capacity_overflow)?,
                );
            }
        }
        heights.push(height);
    }
    Ok(heights)
}

/// Inspect only the authenticated Index subtree. This admits no VM callback and
/// retains a borrowed realization request when an operand is not statically known.
pub(super) fn admit_index_support<'a>(
    bound: &BoundSliceOperands<'a>,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<IndexSupportAdmission<'a>, Failure> {
    if let Some((operand, kind)) = bound.unresolved_path {
        return Ok(realization(
            bound.prepared,
            Some(bound.timing),
            operand,
            kind,
        ));
    }
    inspect_index_support(bound.prepared, Some(bound.timing), depth, budget)
}
pub(super) fn admit_prepared_index_support<'a>(
    prepared: &PreparedSliceAddress<'a>,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<IndexSupportAdmission<'a>, Failure> {
    inspect_index_support(*prepared, None, depth, budget)
}
fn inspect_index_support<'a>(
    prepared: PreparedSliceAddress<'a>,
    timing: Option<&'a FrozenSliceTiming>,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<IndexSupportAdmission<'a>, Failure> {
    use crate::song::source_uses::timing::{FrozenIndexLeaf as L, FrozenIndexParameter as P};
    let heights = operand_heights(prepared.recipe, budget)?;
    let count = prepared.recipe.nodes().len();
    budget.charge(count.checked_mul(2).ok_or_else(capacity_overflow)?)?;
    let mut reachable = vec![false; count];
    let mut nodes = vec![IndexNodeSupport::empty(); count];
    budget.charge(1)?;
    let mut pending = vec![(prepared.index_root, depth)];
    while let Some((index, depth)) = pending.pop() {
        budget.enter(depth)?;
        budget.enter(
            depth
                .checked_add(
                    heights[index as usize]
                        .checked_sub(1)
                        .ok_or_else(capacity_overflow)?,
                )
                .ok_or_else(capacity_overflow)?,
        )?;
        if reachable[index as usize] {
            continue;
        }
        let current = node(prepared.recipe, index)?;
        if let Some(L::Dynamic(kind)) = current.leaf() {
            return Ok(realization(prepared, timing, index, *kind));
        }
        if matches!(current.leaf(), Some(L::Continuous)) {
            return Ok(realization(
                prepared,
                timing,
                index,
                FrozenIndexDynamicKind::Signal,
            ));
        }
        reachable[index as usize] = true;
        for parameter in current.parameters() {
            budget.charge(1)?;
            match parameter {
                P::Dynamic(kind) => return Ok(realization(prepared, timing, index, *kind)),
                P::Pattern { .. } | P::Literal(L::Pattern { .. } | L::AtomicList { .. }) => {
                    return Ok(realization(
                        prepared,
                        timing,
                        index,
                        FrozenIndexDynamicKind::UnresolvedPattern,
                    ));
                }
                _ => {}
            }
        }
        for child in current.children() {
            budget.charge(1)?;
            pending.push((
                child.child(),
                depth.checked_add(1).ok_or_else(capacity_overflow)?,
            ));
        }
    }
    // The issued recipe is validated postorder. Only reachable Index nodes are
    // evaluated here; the selected subject's unresolved leaf is not inspected.
    for index in 0..count {
        if !reachable[index] {
            continue;
        }
        budget.charge(1)?;
        let current = &prepared.recipe.nodes()[index];
        let summary = if let Some(leaf) = current.leaf() {
            match leaf {
                L::Rest => IndexNodeSupport::empty(),
                // Nonnumeric/atomic values remain occupied conservatively;
                // query_slice still raises its original value/index fault.
                L::Scalar { .. } | L::AtomicList { .. } => IndexNodeSupport::occupied(),
                L::Pattern { .. } => {
                    return Ok(realization(
                        prepared,
                        timing,
                        index as u32,
                        FrozenIndexDynamicKind::UnresolvedPattern,
                    ))
                }
                L::Continuous | L::Dynamic(_) => {
                    return Err(invalid("unresolved Index reached static assembly"))
                }
            }
        } else if current.operation() == FrozenUseOperation::Steps {
            let mut summary = IndexNodeSupport::empty();
            let mut full = true;
            let mut end = Ratio64::ZERO;
            let mut count = 0u64;
            let mut event_count = 0u64;
            for slot in current.slots() {
                budget.charge(1)?;
                let (Some(geometry), Some(copies)) = (slot.geometry(), slot.copies()) else {
                    return Ok(realization(
                        prepared,
                        timing,
                        index as u32,
                        FrozenIndexDynamicKind::UnresolvedPattern,
                    ));
                };
                if copies == 0 {
                    continue;
                }
                let child = nodes[slot.child() as usize];
                full &= geometry.prefix == end && child.full;
                end = geometry.prefix.checked_add(
                    geometry
                        .width
                        .checked_mul(Ratio64::from_int(i64::from(copies)))?,
                )?;
                summary.empty &= child.empty;
                summary.max_whole = summary
                    .max_whole
                    .max(child.max_whole.checked_mul(geometry.width)?);
                event_count = event_count
                    .checked_add(
                        child
                            .starts(Ratio64::ONE)?
                            .checked_mul(u64::from(copies))
                            .ok_or_else(capacity_overflow)?,
                    )
                    .ok_or_else(capacity_overflow)?;
                summary.multiplicity = summary.multiplicity.max(child.multiplicity);
                if !child.empty {
                    count = count
                        .checked_add(u64::from(copies))
                        .ok_or_else(capacity_overflow)?;
                    summary.openings = summary
                        .openings
                        .checked_add(child.openings)
                        .and_then(|n| n.checked_add(ceil(child.slope).ok()?))
                        .ok_or_else(capacity_overflow)?;
                }
            }
            summary.event_slope =
                Ratio64::from_int(i64::try_from(event_count).map_err(|_| capacity_overflow())?);
            summary.event_burst = event_count.checked_mul(2).ok_or_else(capacity_overflow)?;
            if full && end == Ratio64::ONE {
                IndexNodeSupport {
                    full: true,
                    openings: 0,
                    slope: Ratio64::ZERO,
                    ..summary
                }
            } else {
                summary.fragmented(count)?
            }
        } else {
            let Some(child) = current.children().first() else {
                return Ok(realization(
                    prepared,
                    timing,
                    index as u32,
                    FrozenIndexDynamicKind::UnresolvedPattern,
                ));
            };
            let mut summary = nodes[child.child() as usize];
            match current.operation() {
                FrozenUseOperation::Fast | FrozenUseOperation::Slow | FrozenUseOperation::Hurry => {
                    let Some(mut rate) = current.parameters().first().and_then(static_number)
                    else {
                        return Ok(realization(
                            prepared,
                            timing,
                            index as u32,
                            FrozenIndexDynamicKind::UnresolvedPattern,
                        ));
                    };
                    if rate <= Ratio64::ZERO {
                        summary = IndexNodeSupport::empty();
                    } else {
                        if current.operation() == FrozenUseOperation::Slow {
                            rate = Ratio64::ONE.checked_div(rate)?;
                        }
                        summary.slope = summary.slope.checked_mul(rate)?;
                        summary.event_slope = summary.event_slope.checked_mul(rate)?;
                        summary.max_whole = summary.max_whole.checked_div(rate)?;
                    }
                }
                FrozenUseOperation::Hold => {}
                FrozenUseOperation::Rev => {
                    let per_cycle = summary.starts(Ratio64::ONE)?;
                    summary.event_slope = Ratio64::from_int(
                        i64::try_from(per_cycle).map_err(|_| capacity_overflow())?,
                    );
                    summary.event_burst = per_cycle.checked_mul(2).ok_or_else(capacity_overflow)?;
                }
                FrozenUseOperation::Repeat => {
                    let Some(raw) = current.parameters().first().and_then(static_number) else {
                        return Ok(realization(
                            prepared,
                            timing,
                            index as u32,
                            FrozenIndexDynamicKind::UnresolvedPattern,
                        ));
                    };
                    if !raw.is_integral() {
                        return Ok(realization(
                            prepared,
                            timing,
                            index as u32,
                            FrozenIndexDynamicKind::UnresolvedPattern,
                        ));
                    }
                    let copies = raw.num().clamp(0, 4096) as u64;
                    if copies == 0 {
                        summary = IndexNodeSupport::empty();
                    } else {
                        summary.event_slope =
                            summary.event_slope.checked_mul(Ratio64::from_int(
                                i64::try_from(copies).map_err(|_| capacity_overflow())?,
                            ))?;
                        summary.event_burst = summary
                            .event_burst
                            .checked_mul(copies)
                            .ok_or_else(capacity_overflow)?;
                        if !summary.full {
                            summary = summary.fragmented(copies)?;
                        }
                    }
                }
                _ => {
                    return Ok(realization(
                        prepared,
                        timing,
                        index as u32,
                        FrozenIndexDynamicKind::UnresolvedPattern,
                    ))
                }
            }
            summary
        };
        nodes[index] = summary;
    }
    if let Some(program) = compile_static_families(&prepared, depth, budget)? {
        let shared = program.support();
        let root = &mut nodes[prepared.index_root as usize];
        root.max_whole = root.max_whole.max(shared.max_whole);
        root.event_slope = root.event_slope.max(shared.event_slope);
        root.event_burst = root.event_burst.max(shared.event_burst);
        root.openings = root.openings.max(shared.openings);
        root.slope = root.slope.max(shared.slope);
    }
    Ok(IndexSupportAdmission::Static(StaticIndexSupport {
        root: prepared.index_root,
        nodes,
    }))
}

#[cfg(test)]
pub(crate) type StaticFamilyRows = Vec<(Ratio64, Ratio64, u32, Ratio64)>;

#[cfg(test)]
pub(crate) fn inspect_static_family_program(
    inventory: &FrozenRoutingInventory,
    scope: usize,
    track: KwId,
    depth: u32,
    limits: crate::song::SongLimits,
) -> Result<(StaticFamilyRows, u32), Failure> {
    let mut budget = ResolutionBudget::new(limits);
    let owner = output_operand_owner(inventory, scope, track, &mut budget)?;
    let recipe = owner
        .payload
        .index_timing()
        .ok_or_else(|| invalid("fixture missing recipe"))?;
    let issuer = node(recipe, recipe.root())?.issuer();
    let prepared = prepare_slice_operands(owner, issuer, &[], depth, &mut budget)?;
    let program = compile_static_families(&prepared, depth, &mut budget)?
        .ok_or_else(|| invalid("fixture is not a static family"))?;
    budget.charge(program.families().len())?;
    let families = program
        .families()
        .iter()
        .map(|f| (f.prefix, f.width, f.copies, f.slow))
        .collect();
    Ok((families, limits.max_nodes - budget.limits().max_nodes))
}

/// Minted through the original copied output address. Root window selects
/// observations; canonical owner-local execution identity is derived at q.
#[allow(dead_code)] // Pending canonical admission consumer, TASK-002.
impl SongSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn canonical_index_request(
        &self,
        scope: usize,
        track: KwId,
        issuer: crate::reader::span::NodeId,
        path: &[FrozenUseTraceTerm],
        window: TimeSpan,
        depth: u32,
        limits: crate::song::SongLimits,
        remaining: &mut u32,
    ) -> Result<crate::song::snapshot::occupancy::CanonicalIndexRequest, Failure> {
        limits.validate()?;
        if *remaining == 0 || *remaining > limits.max_nodes {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "canonical request work exhausted",
            ));
        }
        let mut budget = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: *remaining,
            ..limits
        });
        let result = (|| {
            let owner = output_operand_owner(self.routing(), scope, track, &mut budget)?;
            prepare_slice_operands(owner, issuer, path, depth, &mut budget)?;
            budget.charge(path.len().checked_add(1).ok_or_else(capacity_overflow)?)?;
            crate::song::snapshot::occupancy::CanonicalIndexRequest::mint(
                self,
                scope,
                track,
                owner.revision,
                owner.payload.id,
                owner
                    .payload
                    .index_timing
                    .as_ref()
                    .ok_or_else(|| invalid("missing original recipe"))?
                    .clone(),
                issuer,
                path,
                window,
                depth,
            )
        })();
        budget.with_remaining(|left| {
            *remaining = *left;
            Ok(())
        })?;
        result
    }
}

#[cfg(test)]
pub(super) type GeometryFixtureOutput = (Vec<Vec<TimeSpan>>, Option<bool>, Option<TimeSpan>);
#[cfg(test)]
impl SongSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retained_geometry_fixture(
        &self,
        site: (
            usize,
            KwId,
            crate::reader::span::NodeId,
            &[FrozenUseTraceTerm],
            TimeSpan,
        ),
        address: &crate::song::snapshot::occupancy::lookup::RetainedIndexAddress<'_>,
        selected: Option<&crate::song::snapshot::FrozenSelectedSource>,
        timing: Option<&FrozenSliceTiming>,
        source: TimeSpan,
        owner: TimeSpan,
        depth: u32,
        limits: crate::song::SongLimits,
        remaining: &mut u32,
    ) -> Result<GeometryFixtureOutput, Failure> {
        self.retained_geometry_fixture_impl(
            site, address, selected, timing, source, owner, depth, limits, remaining,
        )
    }
}
