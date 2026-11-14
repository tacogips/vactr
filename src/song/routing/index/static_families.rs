//! One bounded scalar/Steps producer program shared by admission and geometry.
use super::*;
use crate::song::source_uses::layout::FrozenUseSlotLayout;

#[derive(Clone, Copy)]
pub(in crate::song::routing) struct StaticIndexFamily {
    pub(in crate::song::routing) prefix: Ratio64,
    pub(in crate::song::routing) width: Ratio64,
    pub(in crate::song::routing) copies: u32,
    pub(in crate::song::routing) slow: Ratio64,
}
struct FamilyProducer {
    geometry: FrozenUseSlotLayout,
    trace: Vec<FrozenUseTraceTerm>,
}
pub(in crate::song::routing) struct StaticIndexFamilies {
    families: Vec<StaticIndexFamily>,
    producers: Vec<FamilyProducer>,
    support: IndexNodeSupport,
}
impl StaticIndexFamilies {
    pub(in crate::song::routing) fn families(&self) -> &[StaticIndexFamily] {
        &self.families
    }
    pub(in crate::song::routing) fn support(&self) -> IndexNodeSupport {
        self.support
    }
    pub(in crate::song::routing) fn matched(
        &self,
        bound: &BoundSliceOperands<'_>,
        depth: u32,
        budget: &mut ResolutionBudget,
    ) -> Result<Option<(usize, u32)>, Failure> {
        budget.enter(depth)?;
        let start = bound
            .timing
            .issuer_trace()
            .len()
            .checked_add(1)
            .ok_or_else(capacity_overflow)?;
        let trace = bound.timing.index_trace();
        let mut found = None;
        for (index, (family, producer)) in self.families.iter().zip(&self.producers).enumerate() {
            budget.charge(1)?;
            if trace_end(&producer.trace, trace, start, budget)? != Some(trace.len()) {
                continue;
            }
            let copy = match producer.geometry.copy_trace_term {
                Some(term) => {
                    trace
                        .get(
                            start
                                .checked_add(term as usize)
                                .ok_or_else(capacity_overflow)?,
                        )
                        .ok_or_else(|| invalid("family copy producer"))?
                        .ordinal
                }
                None => 0,
            };
            if copy >= family.copies {
                return Err(invalid("issued family copy outside symbolic count"));
            }
            if found.replace((index, copy)).is_some() {
                return Err(invalid("ambiguous issued family producer"));
            }
        }
        Ok(found)
    }
}
fn append(
    out: &mut Vec<FrozenUseTraceTerm>,
    terms: &[FrozenUseTraceTerm],
    budget: &mut ResolutionBudget,
) -> Result<(), Failure> {
    budget.charge(terms.len())?;
    out.extend_from_slice(terms);
    Ok(())
}
pub(in crate::song::routing) fn compile_static_families(
    prepared: &PreparedSliceAddress<'_>,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<Option<StaticIndexFamilies>, Failure> {
    use FrozenIndexLeaf as L;
    use FrozenUseOperation as O;
    budget.enter(depth)?;
    let root = node(prepared.recipe, prepared.index_root)?;
    if root.operation() != O::Steps {
        return Ok(None);
    }
    budget.charge(
        root.slots()
            .len()
            .checked_mul(2)
            .ok_or_else(capacity_overflow)?,
    )?;
    let mut families = Vec::with_capacity(root.slots().len());
    let mut producers = Vec::with_capacity(root.slots().len());
    let mut support = IndexNodeSupport::empty();
    let mut end = Ratio64::ZERO;
    let mut full = true;
    let mut count = 0u64;
    let mut events = 0u64;
    for slot in root.slots() {
        budget.charge(1)?;
        let copies = match slot.copies() {
            Some(0) => continue,
            Some(n) => n,
            None => return Ok(None),
        };
        let Some(geometry) = slot.geometry() else {
            return Ok(None);
        };
        let mut child = node(prepared.recipe, slot.child())?;
        let mut current_depth = depth.checked_add(1).ok_or_else(capacity_overflow)?;
        budget.enter(current_depth)?;
        if matches!(child.leaf(), Some(L::Rest)) {
            full = false;
            continue;
        }
        let mut edge = None;
        for candidate in root.children() {
            budget.charge(1)?;
            if candidate.role() == slot.ordinal() {
                edge = Some(candidate);
                break;
            }
        }
        let edge = edge.ok_or_else(|| invalid("family producer edge"))?;
        let mut trace = Vec::new();
        append(&mut trace, edge.trace(), budget)?;
        // Steps already owns the only Repeat/Hold slot wrapper.
        if matches!(child.operation(), O::Repeat | O::Hold) {
            budget.charge(child.children().len())?;
            if child.children().len() != 1 {
                return Ok(None);
            }
            let next = &child.children()[0];
            append(&mut trace, next.trace(), budget)?;
            child = node(prepared.recipe, next.child())?;
            current_depth = current_depth.checked_add(1).ok_or_else(capacity_overflow)?;
            budget.enter(current_depth)?;
        }
        let mut slow = Ratio64::ONE;
        while matches!(child.operation(), O::Slow | O::Fast | O::Hurry) {
            budget.charge(
                child
                    .parameters()
                    .len()
                    .checked_add(child.children().len())
                    .ok_or_else(capacity_overflow)?,
            )?;
            let Some(rate) = child.parameters().first().and_then(static_number) else {
                return Ok(None);
            };
            if rate <= Ratio64::ZERO || child.children().len() != 1 {
                return Ok(None);
            }
            slow = slow.checked_mul(if child.operation() == O::Slow {
                rate
            } else {
                Ratio64::ONE.checked_div(rate)?
            })?;
            let next = &child.children()[0];
            append(&mut trace, next.trace(), budget)?;
            child = node(prepared.recipe, next.child())?;
            current_depth = current_depth.checked_add(1).ok_or_else(capacity_overflow)?;
            budget.enter(current_depth)?;
        }
        if matches!(child.leaf(), Some(L::Rest)) {
            full = false;
            continue;
        }
        if !matches!(child.leaf(), Some(L::Scalar { .. })) {
            return Ok(None);
        }
        append(&mut trace, child.leaf_trace(), budget)?;
        full &= geometry.prefix == end;
        end = geometry.prefix.checked_add(
            geometry
                .width
                .checked_mul(Ratio64::from_int(i64::from(copies)))?,
        )?;
        count = count
            .checked_add(u64::from(copies))
            .ok_or_else(capacity_overflow)?;
        let starts = ceil(Ratio64::ONE.checked_div(slow)?)?
            .checked_add(2)
            .ok_or_else(capacity_overflow)?;
        events = events
            .checked_add(
                starts
                    .checked_mul(u64::from(copies))
                    .ok_or_else(capacity_overflow)?,
            )
            .ok_or_else(capacity_overflow)?;
        support.empty = false;
        support.multiplicity = 1;
        support.max_whole = support.max_whole.max(geometry.width.checked_mul(slow)?);
        families.push(StaticIndexFamily {
            prefix: geometry.prefix,
            width: geometry.width,
            copies,
            slow,
        });
        producers.push(FamilyProducer {
            geometry: *geometry,
            trace,
        });
    }
    support.event_slope =
        Ratio64::from_int(i64::try_from(events).map_err(|_| capacity_overflow())?);
    support.event_burst = events.checked_mul(2).ok_or_else(capacity_overflow)?;
    if full && end == Ratio64::ONE {
        support.full = true;
    } else {
        support = support.fragmented(count)?;
    }
    Ok(Some(StaticIndexFamilies {
        families,
        producers,
        support,
    }))
}
