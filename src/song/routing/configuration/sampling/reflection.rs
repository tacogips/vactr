//! Exact reflection support and authenticated whole clocks.
use super::*;
fn reflection_birth_cycle(
    output_whole: TimeSpan,
    source_birth: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<i64, Failure> {
    budget.charge(1)?;
    if output_whole.begin >= output_whole.end {
        return Err(invalid("reflection requires a positive authentic whole"));
    }
    let mirror = output_whole.end.checked_add(source_birth)?;
    if mirror.den() != 1 || mirror.num().rem_euclid(2) != 1 {
        return Err(invalid("reflection whole does not authenticate a cycle"));
    }
    mirror
        .num()
        .checked_sub(1)
        .map(|v| v / 2)
        .ok_or_else(capacity_overflow)
}
fn reflected_component(
    source: TimeSpan,
    owner: TimeSpan,
    cycle: i64,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    budget.charge(4)?;
    if source.begin > source.end || owner.begin > owner.end {
        return Err(invalid("reversed reflection support"));
    }
    if source.begin == source.end || owner.begin == owner.end {
        return Ok(None);
    }
    let first = source.begin.floor();
    let last = source.end.floor();
    let reflect = |c: i64, span: TimeSpan| -> Result<TimeSpan, Failure> {
        let mirror = Ratio64::from_int(c)
            .checked_mul(Ratio64::from_int(2))?
            .checked_add(Ratio64::ONE)?;
        TimeSpan::new(
            mirror.checked_sub(span.end)?,
            mirror.checked_sub(span.begin)?,
        )
    };
    let first_cycle = TimeSpan::cycle(first)?;
    let first_piece = reflect(first, intersect(source, first_cycle)?)?;
    let interior_begin = first.checked_add(1).ok_or_else(capacity_overflow)?;
    let interior = if interior_begin < last {
        Some(TimeSpan::new(
            Ratio64::from_int(interior_begin),
            Ratio64::from_int(last),
        )?)
    } else {
        None
    };
    let last_piece = if last > first && source.end > Ratio64::from_int(last) {
        Some(reflect(last, intersect(source, TimeSpan::cycle(last)?)?)?)
    } else {
        None
    };
    let target = TimeSpan::cycle(cycle)?;
    let mut joined: Option<TimeSpan> = None;
    for piece in [Some(first_piece), interior, last_piece]
        .into_iter()
        .flatten()
    {
        let begin = piece.begin.max(owner.begin);
        let end = piece.end.min(owner.end);
        if begin >= end {
            continue;
        }
        let piece = TimeSpan::new(begin, end)?;
        if let Some(previous) = &mut joined {
            if piece.begin <= previous.end {
                previous.end = previous.end.max(piece.end);
                continue;
            }
            if previous.begin < target.end && target.begin < previous.end {
                return Ok(joined);
            }
        }
        joined = Some(piece);
    }
    Ok(joined.filter(|piece| piece.begin < target.end && target.begin < piece.end))
}
pub(in crate::song::routing::configuration) fn ordinary_reflection_configuration(
    cover: &FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &[ProducerStep],
    mut source: TimeSpan,
    output_whole: TimeSpan,
    source_birth: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    budget.charge(identity.edges.len())?;
    let mut chain = Vec::with_capacity(identity.edges.len());
    let mut index = cover.graph().root;
    let mut cursor = 0usize;
    let mut window = cover.window();
    let mut whole = output_whole;
    let mut reflections = 0u32;
    for &ordinal in &identity.edges {
        budget.charge(1)?;
        let node = cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("reflection recipe node"))?;
        let edge = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("reflection recipe edge"))?;
        let end = cursor
            .checked_add(edge.trace.len())
            .ok_or_else(capacity_overflow)?;
        let actual = trace
            .get(cursor..end)
            .ok_or_else(|| invalid("reflection recipe trace"))?;
        if !edge.layout.is_empty() {
            charge_slot(edge, budget)?;
            let unit = layout::slot_interval(edge, 0, actual)?;
            if unit.begin != Ratio64::ZERO || unit.end != Ratio64::ONE {
                return Ok(None);
            }
        }
        let owner = window;
        let emitted = whole;
        match node.mapping {
            M::Rate { factor } if factor > Ratio64::ZERO => {
                if reflections == 0 {
                    whole = whole.map(|t| t.checked_mul(factor))?;
                }
            }
            M::Shift { amount } => {
                if reflections == 0 {
                    whole = whole.map(|t| t.checked_sub(amount))?;
                }
            }
            M::ReflectCycles => {
                reflections += 1;
                if reflections > 1 {
                    return Ok(None);
                }
            }
            M::Preserve | M::Parallel | M::SelectContent { .. } => {}
            _ => return Ok(None),
        }
        window = crate::song::source::mapped_support(node, window)?;
        chain.push((node, owner, emitted));
        index = edge.child;
        cursor = end;
    }
    if reflections == 0 {
        return Ok(None);
    }
    let mut anchor = source_birth;
    for (node, owner, emitted) in chain.into_iter().rev() {
        budget.charge(1)?;
        source = match node.mapping {
            M::Rate { factor } => {
                anchor = anchor.checked_div(factor)?;
                source.map(|t| t.checked_div(factor))?
            }
            M::Shift { amount } => {
                anchor = anchor.checked_add(amount)?;
                source.map(|t| t.checked_add(amount))?
            }
            M::ReflectCycles => {
                let cycle = reflection_birth_cycle(emitted, anchor, budget)?;
                anchor = emitted.begin;
                let Some(component) = reflected_component(source, owner, cycle, budget)? else {
                    return Ok(None);
                };
                component
            }
            _ => source,
        };
    }
    if anchor != output_whole.begin {
        return Err(invalid("reflection whole replay disagrees with emission"));
    }
    Ok(Some(intersect(source, cover.window())?))
}

pub(super) fn membership_contains(span: TimeSpan, member: SourceMembership) -> bool {
    if span.is_point() {
        return member.point == span.begin;
    }
    match member.side {
        MembershipSide::At => span.begin <= member.point && member.point < span.end,
        MembershipSide::Before => span.begin < member.point && member.point <= span.end,
    }
}
pub(super) fn membership_cycle(member: SourceMembership) -> Result<i64, Failure> {
    member
        .point
        .floor()
        .checked_sub(i64::from(
            member.side == MembershipSide::Before && member.point.frac() == Ratio64::ZERO,
        ))
        .ok_or_else(capacity_overflow)
}
pub(super) fn quantized_membership(
    member: SourceMembership,
    divisions: i64,
) -> Result<SourceMembership, Failure> {
    if divisions <= 0 {
        return Err(invalid("sampled locator divisions"));
    }
    if member.side == MembershipSide::At {
        return Ok(SourceMembership {
            point: super::quantized_point(member.point, divisions)?,
            side: MembershipSide::At,
        });
    }
    let numerator = i128::from(member.point.num())
        .checked_mul(i128::from(divisions))
        .ok_or_else(capacity_overflow)?;
    let denominator = i128::from(member.point.den());
    let phase = numerator
        .div_euclid(denominator)
        .checked_sub(i128::from(
            member.side == MembershipSide::Before && numerator.rem_euclid(denominator) == 0,
        ))
        .ok_or_else(capacity_overflow)?;
    Ok(SourceMembership {
        point: super::super::grid_phase_time(phase, divisions)?,
        side: MembershipSide::At,
    })
}
pub(super) fn inverse_slot_membership(
    edge: &FrozenSourceUseEdge,
    cycle: i64,
    trace: &[ProducerStep],
    member: SourceMembership,
    budget: &mut ResolutionBudget,
) -> Result<SourceMembership, Failure> {
    budget.charge(4)?;
    let slot = layout::slot_interval(edge, cycle, trace)?;
    if !membership_contains(slot, member) {
        return Err(invalid("sample point outside authenticated slot"));
    }
    let width = slot.end.checked_sub(slot.begin)?;
    if width <= Ratio64::ZERO {
        return Err(invalid("empty authenticated slot"));
    }
    Ok(SourceMembership {
        point: Ratio64::from_int(cycle)
            .checked_add(member.point.checked_sub(slot.begin)?.checked_div(width)?)?,
        side: member.side,
    })
}
pub(in crate::song::routing::configuration::sampling) fn reflect_membership(
    member: SourceMembership,
    cycle: i64,
    budget: &mut ResolutionBudget,
) -> Result<SourceMembership, Failure> {
    budget.charge(3)?;
    let mirror = Ratio64::from_int(cycle)
        .checked_mul(Ratio64::from_int(2))?
        .checked_add(Ratio64::ONE)?;
    Ok(SourceMembership {
        point: mirror.checked_sub(member.point)?,
        side: match member.side {
            MembershipSide::At => MembershipSide::Before,
            MembershipSide::Before => MembershipSide::At,
        },
    })
}
pub(in crate::song::routing::configuration::sampling) fn reflect_whole(
    whole: TimeSpan,
    cycle: i64,
    budget: &mut ResolutionBudget,
) -> Result<TimeSpan, Failure> {
    budget.charge(4)?;
    let mirror = Ratio64::from_int(cycle)
        .checked_mul(Ratio64::from_int(2))?
        .checked_add(Ratio64::ONE)?;
    TimeSpan::new(
        mirror.checked_sub(whole.end)?,
        mirror.checked_sub(whole.begin)?,
    )
}

pub(super) fn reflected_component_for_member(
    source: TimeSpan,
    owner: TimeSpan,
    cycle: i64,
    member: SourceMembership,
    budget: &mut ResolutionBudget,
) -> Result<TimeSpan, Failure> {
    reflected_component(source, owner, cycle, budget)?
        .filter(|span| membership_contains(*span, member))
        .ok_or_else(|| invalid("reflected source configuration excludes oriented membership"))
}

pub(super) fn replay_iterate_membership(
    source: crate::pattern::TimeSpan,
    count: u32,
    owner: crate::pattern::TimeSpan,
    member: SourceMembership,
    budget: &mut ResolutionBudget,
) -> Result<crate::pattern::TimeSpan, Failure> {
    if member.side == MembershipSide::At {
        return super::super::iterator_component(source, count, owner, member.point, budget);
    }
    if count == 0 {
        budget.charge(1)?;
        let clipped = TimeSpan::new(source.begin.max(owner.begin), source.end.min(owner.end))?;
        return if membership_contains(clipped, member) {
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
            if membership_contains(*current, member) {
                return Ok(*current);
            }
        }
        component = Some(span);
    }
    component
        .filter(|span| membership_contains(*span, member))
        .ok_or_else(|| invalid("birth outside Iterate policy support"))
}

pub(super) fn replay_periodic_membership(
    condition: C,
    transformed: bool,
    span: TimeSpan,
    owner: TimeSpan,
    member: SourceMembership,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    if member.side == MembershipSide::At {
        return super::super::periodic_component(
            condition,
            transformed,
            span,
            owner,
            member.point,
            budget,
        );
    }
    budget.charge(8)?;
    let span = intersect(span, owner)?;
    let cycle = membership_cycle(member)?;
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
        C::Chunk { .. } => return Err(invalid("Chunk requires authentic child whole anchor")),
        _ => {
            if condition.selects(cycle, member.point)? != transformed
                || !membership_contains(span, member)
            {
                return Err(invalid("constant conditional excludes oriented membership"));
            }
            return Ok(Some(span));
        }
    };
    let residue = cycle.rem_euclid(period);
    if residue < lo || residue >= hi {
        return Err(invalid("conditional edge excludes oriented cycle"));
    }
    let base = Ratio64::from_int(cycle).checked_sub(Ratio64::from_int(residue))?;
    let run = TimeSpan::new(
        base.checked_add(Ratio64::from_int(lo))?,
        base.checked_add(Ratio64::from_int(hi))?,
    )?;
    let component = intersect(span, run)?;
    if !membership_contains(component, member) {
        return Err(invalid("conditional excludes oriented policy membership"));
    }
    Ok(Some(component))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::SongLimits;
    use crate::vm::fail::FailCode;
    fn member(num: i64, den: i64, side: MembershipSide) -> SourceMembership {
        SourceMembership {
            point: Ratio64::new(num, den).unwrap(),
            side,
        }
    }
    #[test]
    fn oriented_reflection_negative_cycles_and_exact_endpoint_quota() {
        let before = member(0, 1, MembershipSide::Before);
        assert_eq!(membership_cycle(before).unwrap(), -1);
        assert_eq!(
            membership_cycle(member(-2, 1, MembershipSide::Before)).unwrap(),
            -3
        );
        assert_eq!(
            membership_cycle(member(-7, 3, MembershipSide::Before)).unwrap(),
            -3
        );
        assert_eq!(
            membership_cycle(member(i64::MIN, 1, MembershipSide::Before))
                .unwrap_err()
                .code,
            FailCode::Overflow
        );
        let original = TimeSpan::new(Ratio64::new(2, 3).unwrap(), Ratio64::ONE).unwrap();
        for nodes in [6, 7] {
            let mut budget = ResolutionBudget::new(SongLimits {
                max_nodes: nodes,
                ..Default::default()
            });
            let mapped =
                reflect_membership(member(0, 1, MembershipSide::At), 0, &mut budget).unwrap();
            assert_eq!(mapped, member(1, 1, MembershipSide::Before));
            assert!(membership_contains(original, mapped));
            let whole = reflect_whole(original, 0, &mut budget);
            if nodes == 6 {
                assert_eq!(whole.unwrap_err().code, FailCode::FuelExhausted);
            } else {
                assert_eq!(
                    whole.unwrap(),
                    TimeSpan::new(Ratio64::ZERO, Ratio64::new(1, 3).unwrap()).unwrap()
                );
                assert_eq!(budget.limits().max_nodes, 0);
            }
        }
        let mut budget = ResolutionBudget::new(SongLimits::default());
        assert_eq!(
            reflect_membership(before, -1, &mut budget).unwrap(),
            member(-1, 1, MembershipSide::At)
        );
        assert_eq!(
            reflect_whole(original, i64::MAX, &mut budget)
                .unwrap_err()
                .code,
            FailCode::Overflow
        );
    }
    #[test]
    fn weighted_endpoint_inverse_and_sampling_use_oriented_cell_start() {
        let edge = FrozenSourceUseEdge {
            child: 0,
            trace: vec![],
            layout: vec![layout::FrozenUseSlotLayout {
                prefix: Ratio64::ZERO,
                width: Ratio64::new(1, 2).unwrap(),
                copy_trace_term: None,
            }],
        };
        let mut budget = ResolutionBudget::new(SongLimits::default());
        assert_eq!(
            inverse_slot_membership(
                &edge,
                -2,
                &[],
                member(-3, 2, MembershipSide::At),
                &mut budget
            )
            .unwrap_err()
            .code,
            FailCode::Type
        );
        let inverse = inverse_slot_membership(
            &edge,
            -2,
            &[],
            member(-3, 2, MembershipSide::Before),
            &mut budget,
        )
        .unwrap();
        assert_eq!(inverse, member(-1, 1, MembershipSide::Before));
        assert_eq!(
            quantized_membership(inverse, 4).unwrap(),
            member(-5, 4, MembershipSide::At)
        );
        assert_eq!(
            quantized_membership(member(1, 4, MembershipSide::Before), 4).unwrap(),
            member(0, 1, MembershipSide::At)
        );
        assert_eq!(
            quantized_membership(member(1, 4, MembershipSide::At), 4).unwrap(),
            member(1, 4, MembershipSide::At)
        );
    }
    #[test]
    fn oriented_periodic_and_iterate_replay_select_previous_boundary_component() {
        let mut budget = ResolutionBudget::new(SongLimits::default());
        let owner = TimeSpan::new(Ratio64::from_int(-4), Ratio64::from_int(4)).unwrap();
        let prior = replay_periodic_membership(
            C::Every { period: 2 },
            true,
            owner,
            owner,
            member(-1, 1, MembershipSide::Before),
            &mut budget,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            prior,
            TimeSpan::new(Ratio64::from_int(-2), Ratio64::from_int(-1)).unwrap()
        );
        assert_eq!(
            replay_periodic_membership(
                C::Every { period: 2 },
                true,
                owner,
                owner,
                member(-1, 1, MembershipSide::At),
                &mut budget
            )
            .unwrap(),
            Some(TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap())
        );
        let source = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
        // c=-1 has (-1).rem_euclid(2)/2 = 1/2, so [0,1) pulls
        // back to [-1/2,1/2), clipped by cycle[-1,0) to [-1/2,0).
        // c=0 contributes [0,1); touching pieces form [-1/2,1).
        let joined = TimeSpan::new(Ratio64::new(-1, 2).unwrap(), Ratio64::ONE).unwrap();
        assert!(membership_contains(
            joined,
            member(1, 1, MembershipSide::Before)
        ));
        assert!(membership_contains(
            joined,
            member(-1, 2, MembershipSide::At)
        ));
        assert!(!membership_contains(
            joined,
            member(1, 1, MembershipSide::At)
        ));
        assert_eq!(
            replay_iterate_membership(
                source,
                2,
                owner,
                member(1, 1, MembershipSide::Before),
                &mut budget
            )
            .unwrap(),
            joined
        );
        assert_eq!(
            replay_iterate_membership(
                source,
                2,
                owner,
                member(1, 1, MembershipSide::At),
                &mut budget
            )
            .unwrap_err()
            .code,
            FailCode::Type
        );
        assert_eq!(
            replay_iterate_membership(
                source,
                0,
                owner,
                member(1, 1, MembershipSide::Before),
                &mut budget
            )
            .unwrap(),
            source
        );
    }
    #[test]
    fn reflected_component_boundaries_and_fuel_are_checked() {
        let mut budget = ResolutionBudget::new(SongLimits::default());
        let source =
            TimeSpan::new(Ratio64::new(1, 4).unwrap(), Ratio64::new(11, 4).unwrap()).unwrap();
        let owner = TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap();
        for (cycle, lo, hi, den) in [(0, 0, 3, 4), (1, 1, 2, 1), (2, 9, 12, 4)] {
            assert_eq!(
                reflected_component(source, owner, cycle, &mut budget)
                    .unwrap()
                    .unwrap(),
                TimeSpan::new(
                    Ratio64::new(lo, den).unwrap(),
                    Ratio64::new(hi, den).unwrap()
                )
                .unwrap()
            );
        }
        let billion = TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(1_000_000_000)).unwrap();
        for nodes in [3, 4] {
            let mut quota = ResolutionBudget::new(SongLimits {
                max_nodes: nodes,
                ..Default::default()
            });
            let result = reflected_component(billion, billion, 900_000_000, &mut quota);
            if nodes == 3 {
                assert_eq!(result.unwrap_err().code, FailCode::FuelExhausted);
            } else {
                assert_eq!(result.unwrap(), Some(billion));
                assert_eq!(quota.limits().max_nodes, 0);
            }
        }
        let output =
            TimeSpan::new(Ratio64::new(-11, 4).unwrap(), Ratio64::new(-5, 2).unwrap()).unwrap();
        assert_eq!(
            reflection_birth_cycle(output, Ratio64::new(-5, 2).unwrap(), &mut budget).unwrap(),
            -3
        );
        assert_eq!(
            reflection_birth_cycle(
                TimeSpan::cycle(0).unwrap(),
                Ratio64::new(1, 4).unwrap(),
                &mut budget
            )
            .unwrap_err()
            .code,
            FailCode::Type
        );
        assert_eq!(
            reflection_birth_cycle(
                TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(i64::MAX)).unwrap(),
                Ratio64::ONE,
                &mut budget
            )
            .unwrap_err()
            .code,
            FailCode::Overflow
        );
    }
}
