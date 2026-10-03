//! Maximal intrinsic component arithmetic with shared route work/depth.
use super::source::{invalid, ResolutionBudget};
use super::*;
use crate::song::snapshot::{FrozenEdit, FrozenPartNode};
use crate::vm::fail::{FailCode, Failure};

#[derive(Clone, Copy)]
pub(super) struct ClockedPredicate {
    pub condition: crate::song::source_uses::FrozenStaticCondition,
    pub transformed: bool,
    pub factor: Ratio64,
    pub shift: Ratio64,
}
pub(super) fn joint_periodic_component(
    predicates: &[ClockedPredicate],
    source: crate::pattern::TimeSpan,
    owner: crate::pattern::TimeSpan,
    anchor: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<crate::pattern::TimeSpan, Failure> {
    let mut locator = anchor.max(source.begin).max(owner.begin);
    let mut horizon = None;
    let origin = locator;
    loop {
        budget.charge(1)?;
        let mut begin = source.begin.max(owner.begin);
        let mut end = source.end.min(owner.end);
        for predicate in predicates {
            let to_local = |t: Ratio64| {
                t.checked_mul(predicate.factor)?
                    .checked_add(predicate.shift)
            };
            let span = periodic_component(
                predicate.condition,
                predicate.transformed,
                source.map(to_local)?,
                owner.map(to_local)?,
                to_local(locator)?,
                budget,
            )?
            .ok_or_else(|| invalid("joint periodic recipe contains unsupported predicate"))?;
            let span = span.map(|t| {
                t.checked_sub(predicate.shift)?
                    .checked_div(predicate.factor)
            })?;
            begin = begin.max(span.begin);
            end = end.min(span.end);
        }
        if begin < end && end > locator {
            return crate::pattern::TimeSpan::new(begin, end);
        }
        let limit = if let Some(limit) = horizon {
            limit
        } else {
            let remaining = source.end.min(owner.end).checked_sub(origin)?;
            let cap = remaining
                .floor()
                .checked_add(i64::from(remaining.frac() != Ratio64::ZERO))
                .ok_or_else(capacity_overflow)?;
            let period = joint_period(predicates, cap, budget)?;
            let limit = if Ratio64::from_int(period) >= remaining {
                source.end.min(owner.end)
            } else {
                origin.checked_add(Ratio64::from_int(period))?
            };
            horizon = Some(limit);
            limit
        };
        if begin <= locator || begin >= limit {
            return Err(invalid("joint periodic birth has no applicable component"));
        }
        locator = begin;
    }
}

fn joint_period(
    predicates: &[ClockedPredicate],
    cap: i64,
    budget: &mut ResolutionBudget,
) -> Result<i64, Failure> {
    use crate::song::source_uses::FrozenStaticCondition as C;
    if cap <= 0 {
        return Err(invalid("joint periodic birth after support"));
    }
    let mut period = 1i64;
    for predicate in predicates {
        budget.charge(1)?;
        let local_period = match predicate.condition {
            C::Every { period } if period > 1 => period,
            C::WhenMod { modulus, threshold }
                if modulus > 0 && threshold > 0 && threshold < modulus =>
            {
                modulus
            }
            _ => 1,
        };
        let numerator = i128::from(local_period)
            .checked_mul(i128::from(predicate.factor.den()))
            .ok_or_else(capacity_overflow)?;
        let (mut a, mut b) = (numerator, i128::from(predicate.factor.num()));
        while b != 0 {
            budget.charge(1)?;
            (a, b) = (b, a.rem_euclid(b));
        }
        let next = numerator / a;
        if next >= i128::from(cap) {
            return Ok(cap);
        }
        // Reduced numerator is an integer multiple of the exact rational
        // predicate period, so a common integer horizon remains sound.
        let (mut a, mut b) = (i128::from(period), next);
        while b != 0 {
            budget.charge(1)?;
            (a, b) = (b, a.rem_euclid(b));
        }
        let product = (i128::from(period) / a)
            .checked_mul(next)
            .ok_or_else(capacity_overflow)?;
        if product >= i128::from(cap) {
            return Ok(cap);
        }
        period = i64::try_from(product).map_err(|_| capacity_overflow())?;
    }
    Ok(period)
}

/// Locate the first applicable intrinsic run for this emitted birth. A held
/// continuation retains that birth run even when later queries touch reentry.
pub(super) fn periodic_component(
    condition: crate::song::source_uses::FrozenStaticCondition,
    transformed: bool,
    span: crate::pattern::TimeSpan,
    owner: crate::pattern::TimeSpan,
    anchor: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<crate::pattern::TimeSpan>, Failure> {
    use crate::pattern::TimeSpan;
    use crate::song::source_uses::FrozenStaticCondition as C;
    budget.charge(8)?;
    let span = TimeSpan::new(span.begin.max(owner.begin), span.end.min(owner.end))?;
    let (period, lo, hi) = match condition {
        C::Every { period } if period > 1 => {
            if transformed {
                (period, 0, 1)
            } else {
                (period, 1, period)
            }
        }
        C::WhenMod { modulus, threshold }
            if modulus > 0 && threshold > 0 && threshold < modulus =>
        {
            if transformed {
                (modulus, threshold, modulus)
            } else {
                (modulus, 0, threshold)
            }
        }
        C::Chunk { divisions } if divisions > 1 => {
            return chunk_component(divisions, transformed, span, anchor, budget).map(Some);
        }
        _ => {
            if condition.selects(0, Ratio64::ZERO)? != transformed {
                return Err(invalid("conditional edge has no intrinsic applicability"));
            }
            if anchor >= span.end {
                return Err(invalid("conditional birth after policy support"));
            }
            return Ok(Some(span));
        }
    };
    let first = anchor.max(span.begin).floor();
    let residue = first.rem_euclid(period);
    let base = Ratio64::from_int(first).checked_sub(Ratio64::from_int(residue))?;
    let cycle = if residue < lo {
        base.checked_add(Ratio64::from_int(lo))?
    } else if residue >= hi {
        base.checked_add(Ratio64::from_int(period))?
            .checked_add(Ratio64::from_int(lo))?
    } else {
        Ratio64::from_int(first)
    };
    let run_base = cycle.checked_sub(Ratio64::from_int(cycle.floor().rem_euclid(period)))?;
    let begin = span.begin.max(run_base.checked_add(Ratio64::from_int(lo))?);
    let end = span.end.min(run_base.checked_add(Ratio64::from_int(hi))?);
    if begin >= end || end <= anchor {
        return Err(invalid("conditional birth has no policy component"));
    }
    Ok(Some(TimeSpan::new(begin, end)?))
}

fn chunk_component(
    divisions: i64,
    transformed: bool,
    span: crate::pattern::TimeSpan,
    anchor: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<crate::pattern::TimeSpan, Failure> {
    budget.charge(8)?;
    let member = anchor
        .frac()
        .checked_mul(Ratio64::from_int(divisions))?
        .floor();
    let first = anchor.max(span.begin).floor();
    let residue = first.rem_euclid(divisions);
    let delta = if transformed {
        member
            .checked_sub(residue)
            .ok_or_else(capacity_overflow)?
            .rem_euclid(divisions)
    } else {
        i64::from(member == residue)
    };
    let cycle = Ratio64::from_int(first).checked_add(Ratio64::from_int(delta))?;
    let residue = cycle.floor().rem_euclid(divisions);
    let lo = Ratio64::new(residue, divisions)?;
    let hi = Ratio64::new(
        residue.checked_add(1).ok_or_else(capacity_overflow)?,
        divisions,
    )?;
    let unit = Ratio64::new(1, divisions)?;
    let (begin, end) = if transformed {
        (
            cycle
                .checked_add(lo)?
                .checked_sub(if residue == 0 { unit } else { Ratio64::ZERO })?,
            cycle
                .checked_add(hi)?
                .checked_add(if residue == divisions - 1 {
                    unit
                } else {
                    Ratio64::ZERO
                })?,
        )
    } else if anchor.frac() < lo {
        let previous = cycle.checked_sub(Ratio64::ONE)?;
        let previous_hi = Ratio64::new(
            previous
                .floor()
                .rem_euclid(divisions)
                .checked_add(1)
                .ok_or_else(capacity_overflow)?,
            divisions,
        )?;
        (previous.checked_add(previous_hi)?, cycle.checked_add(lo)?)
    } else {
        let next = cycle.checked_add(Ratio64::ONE)?;
        let next_lo = Ratio64::new(next.floor().rem_euclid(divisions), divisions)?;
        (cycle.checked_add(hi)?, next.checked_add(next_lo)?)
    };
    let begin = begin.max(span.begin);
    let end = end.min(span.end);
    if begin >= end || end <= anchor {
        return Err(invalid("chunk birth has no intrinsic component"));
    }
    crate::pattern::TimeSpan::new(begin, end)
}

/// All interior query cycles lie inside a source interval at least five cycles
/// wide. Only two cycles at either endpoint can have partial/missing support.
/// This proof is independent of finite duration and preserves end reentry.
pub(super) fn iterator_component(
    source: crate::pattern::TimeSpan,
    count: u32,
    owner: crate::pattern::TimeSpan,
    birth: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<crate::pattern::TimeSpan, Failure> {
    use crate::pattern::TimeSpan;
    if count == 0 {
        budget.charge(1)?;
        let clipped = TimeSpan::new(source.begin.max(owner.begin), source.end.min(owner.end))?;
        return if birth >= clipped.begin && birth < clipped.end {
            Ok(clipped)
        } else {
            Err(invalid("birth outside identity Iterate policy support"))
        };
    }
    let first = source
        .begin
        .floor()
        .checked_sub(1)
        .ok_or_else(capacity_overflow)?
        .max(owner.begin.floor());
    let last = source.end.floor().min(owner.end.floor());
    if last < first {
        return Err(invalid("empty Iterate configuration"));
    }
    let length = last
        .checked_sub(first)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(capacity_overflow)?;
    budget.charge(5)?;
    let mut spans = Vec::with_capacity(5);
    let mut push = |cycle: i64| -> Result<(), Failure> {
        budget.charge(1)?;
        let shift = Ratio64::new(cycle.rem_euclid(i64::from(count)), i64::from(count))?;
        let begin = Ratio64::from_int(cycle)
            .max(source.begin.checked_sub(shift)?)
            .max(owner.begin);
        let end = Ratio64::from_int(cycle.checked_add(1).ok_or_else(capacity_overflow)?)
            .min(source.end.checked_sub(shift)?)
            .min(owner.end);
        if begin < end {
            spans.push(TimeSpan::new(begin, end)?);
        }
        Ok(())
    };
    if length <= 5 {
        for offset in 0..length {
            push(first.checked_add(offset).ok_or_else(capacity_overflow)?)?;
        }
    } else {
        push(first)?;
        push(first.checked_add(1).ok_or_else(capacity_overflow)?)?;
        push(last.checked_sub(1).ok_or_else(capacity_overflow)?)?;
        push(last)?;
        let begin =
            Ratio64::from_int(first.checked_add(2).ok_or_else(capacity_overflow)?).max(owner.begin);
        let end =
            Ratio64::from_int(last.checked_sub(1).ok_or_else(capacity_overflow)?).min(owner.end);
        if begin < end {
            spans.push(TimeSpan::new(begin, end)?);
        }
    }
    spans.sort_by_key(|span| span.begin);
    let mut component: Option<TimeSpan> = None;
    for span in spans {
        if let Some(current) = &mut component {
            if current.end >= span.begin {
                current.end = current.end.max(span.end);
                continue;
            }
            if birth >= current.begin && birth < current.end {
                return Ok(*current);
            }
        }
        component = Some(span);
    }
    component
        .filter(|span| birth >= span.begin && birth < span.end)
        .ok_or_else(|| invalid("birth outside Iterate policy support"))
}

fn lifecycle_invalid(message: &str) -> Failure {
    Failure::new(
        crate::vm::fail::FailCode::BeyondCapability,
        format!("song route: {message}"),
    )
}
pub(super) fn lifecycle_generations(
    bound: density::ConfigurationBound,
    tail_cycles: Ratio64,
) -> Result<u32, Failure> {
    u32::try_from(bound.live_generations(tail_cycles)?).map_err(|_| capacity_overflow())
}
pub(super) fn outer_placement_overlap(
    inventory: &FrozenRoutingInventory,
    path: &[SongRoutePlacement],
    track: KwId,
    tail_cycles: Ratio64,
    remaining: &mut u32,
) -> Result<u64, Failure> {
    let mut count = 1u64;
    for step in path {
        *remaining = remaining.checked_sub(1).ok_or_else(|| {
            Failure::new(FailCode::FuelExhausted, "route placement work exhausted")
        })?;
        match step {
            SongRoutePlacement::Repeat { part, count: n } => {
                let FrozenPartNode::Repeat { child, .. } = &inventory
                    .parts
                    .get(*part)
                    .ok_or_else(|| lifecycle_invalid("invalid repeat placement"))?
                    .node
                else {
                    return Err(lifecycle_invalid("repeat placement mismatch"));
                };
                let duration = inventory
                    .parts
                    .get(*child)
                    .ok_or_else(|| lifecycle_invalid("invalid repeat child"))?
                    .duration;
                if duration > Ratio64::ZERO {
                    let ratio = tail_cycles.checked_div(duration)?;
                    let cycles = ratio
                        .floor()
                        .checked_add(i64::from(ratio.frac() != Ratio64::ZERO))
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(capacity_overflow)?;
                    count = count
                        .checked_mul(
                            u64::from(*n)
                                .min(u64::try_from(cycles).map_err(|_| capacity_overflow())?),
                        )
                        .ok_or_else(capacity_overflow)?;
                }
            }
            SongRoutePlacement::Region {
                part,
                inside: false,
                ..
            } if tail_cycles > Ratio64::ZERO => {
                if matches!(&inventory.parts.get(*part).ok_or_else(||lifecycle_invalid("invalid region placement"))?.node,FrozenPartNode::Edit{edit:FrozenEdit::Overwrite{track:t,..},..} if *t==track)
                {
                    count = count.checked_mul(2).ok_or_else(capacity_overflow)?;
                }
            }
            _ => {}
        }
    }
    Ok(count)
}
fn member_work(members: &[FrozenSound], candidates: &[FrozenSound]) -> Result<u32, Failure> {
    let weight = |sounds: &[FrozenSound]| -> Result<u64, Failure> {
        sounds.iter().try_fold(0u64, |total, sound| {
            let bytes = match sound {
                FrozenSound::Sample { path, .. } => {
                    u64::try_from(path.len()).map_err(|_| capacity_overflow())?
                }
                _ => 0,
            };
            total
                .checked_add(bytes.checked_add(1).ok_or_else(capacity_overflow)?)
                .ok_or_else(capacity_overflow)
        })
    };
    let a = weight(members)?;
    let b = weight(candidates)?;
    u32::try_from(
        a.checked_mul(b)
            .and_then(|n| n.checked_mul(3))
            .and_then(|n| n.checked_add(a))
            .ok_or_else(capacity_overflow)?,
    )
    .map_err(|_| capacity_overflow())
}
pub(super) fn charge_members(
    remaining: &mut u32,
    members: &[FrozenSound],
    candidates: &[FrozenSound],
) -> Result<(), Failure> {
    super::density::density_charge(
        remaining,
        u32::try_from(
            members
                .len()
                .checked_add(candidates.len())
                .ok_or_else(capacity_overflow)?,
        )
        .map_err(|_| capacity_overflow())?,
    )?;
    super::density::density_charge(remaining, member_work(members, candidates)?)
}
#[cfg(test)]
#[path = "components/iterator_component_tests.rs"]
mod iterator_component_tests;

#[cfg(test)]
#[path = "components/affine_candidate_tests.rs"]
mod affine_candidate_tests;
