//! Exact Index whole replay arithmetic, independent of query clipping.
pub(in crate::song::routing) mod canonical;
use super::super::source::{invalid, ResolutionBudget};
use super::*;
use crate::pattern::TimeSpan;
mod cluster;

fn mul(a: i128, b: i128) -> Result<i128, Failure> {
    a.checked_mul(b).ok_or_else(capacity_overflow)
}
fn sub(a: i128, b: i128) -> Result<i128, Failure> {
    a.checked_sub(b).ok_or_else(capacity_overflow)
}
fn scaled(value: Ratio64, denominator: i128) -> Result<i128, Failure> {
    mul(
        i128::from(value.num()),
        denominator / i128::from(value.den()),
    )
}
fn ceil_signed(value: Ratio64) -> Result<i128, Failure> {
    i128::from(value.floor())
        .checked_add(i128::from(value.frac() != Ratio64::ZERO))
        .ok_or_else(capacity_overflow)
}
fn checked_lcm(a: i128, b: i128, budget: &mut ResolutionBudget) -> Result<i128, Failure> {
    let (mut x, mut y) = (a, b);
    while y != 0 {
        budget.charge(1)?;
        (x, y) = (y, x.rem_euclid(y));
    }
    mul(a / x, b)
}
fn modular_product(
    a: i128,
    b: i128,
    modulus: i128,
    budget: &mut ResolutionBudget,
) -> Result<i128, Failure> {
    let mut a = a.rem_euclid(modulus);
    let mut b = b.rem_euclid(modulus);
    let mut product = 0;
    let add = |a: i128, b: i128| {
        if a >= modulus - b {
            a - (modulus - b)
        } else {
            a + b
        }
    };
    while b != 0 {
        budget.charge(1)?;
        if b & 1 != 0 {
            product = add(product, a);
        }
        b >>= 1;
        if b != 0 {
            a = add(a, a);
        }
    }
    Ok(product)
}
/// Return the earliest integer cycle that actually emits this weighted Slow
/// whole. Output parts and slot clipping are deliberately absent from this proof.
/// This helper does not certify the source identity or its owner membership.
pub(in crate::song::routing) fn weighted_slow_whole_cycle(
    prefix: Ratio64,
    width: Ratio64,
    slow: Ratio64,
    whole: TimeSpan,
    issuing_owner: Option<TimeSpan>,
    budget: &mut ResolutionBudget,
) -> Result<Option<i64>, Failure> {
    budget.charge(1)?;
    if width <= Ratio64::ZERO || width > Ratio64::ONE || slow <= Ratio64::ZERO {
        return Err(invalid("weighted Slow whole parameters"));
    }
    let length = width.checked_mul(slow)?;
    if whole.end.checked_sub(whole.begin)? != length {
        return Ok(None);
    }
    let q = whole.begin.checked_sub(prefix)?;
    let d = Ratio64::ONE.checked_sub(width)?;
    let mut lower = i128::from(q.checked_sub(width)?.floor())
        .checked_add(1)
        .ok_or_else(capacity_overflow)?;
    let mut upper = ceil_signed(q.checked_add(length)?)?;
    if let Some(owner) = issuing_owner {
        budget.charge(1)?;
        lower = lower.max(
            i128::from(owner.begin.checked_sub(prefix)?.checked_sub(width)?.floor())
                .checked_add(1)
                .ok_or_else(capacity_overflow)?,
        );
        upper = upper.min(ceil_signed(owner.end.checked_sub(prefix)?)?);
    }
    if lower >= upper {
        return Ok(None);
    }
    let cycle = if d == Ratio64::ZERO {
        if !q.checked_div(length)?.is_integral() {
            return Ok(None);
        }
        lower
    } else {
        // Clearing a checked common denominator preserves the exact congruence.
        let denominator = checked_lcm(
            checked_lcm(i128::from(q.den()), i128::from(d.den()), budget)?,
            i128::from(length.den()),
            budget,
        )?;
        let a = scaled(d, denominator)?;
        let modulus = scaled(length, denominator)?;
        let target = scaled(q, denominator)?;
        let (mut old_r, mut r) = (a, modulus);
        let (mut old_s, mut s) = (1i128, 0i128);
        while r != 0 {
            budget.charge(1)?;
            let quotient = old_r / r;
            let next_r = sub(old_r, mul(quotient, r)?)?;
            let next_s = sub(old_s, mul(quotient, s)?)?;
            (old_r, r) = (r, next_r);
            (old_s, s) = (s, next_s);
        }
        if target.rem_euclid(old_r) != 0 {
            return Ok(None);
        }
        let period = modulus / old_r;
        let residue = modular_product(old_s, target / old_r, period, budget)?;
        let advance = sub(residue, lower.rem_euclid(period))?.rem_euclid(period);
        let first = lower.checked_add(advance).ok_or_else(capacity_overflow)?;
        if first >= upper {
            return Ok(None);
        }
        first
    };
    let cycle = i64::try_from(cycle).map_err(|_| capacity_overflow())?;
    // Replay the original equation as a separate checked verification, retaining
    // the cycle rather than guessing it from floor(whole.begin).
    let c = Ratio64::from_int(cycle);
    let quotient = q.checked_sub(d.checked_mul(c)?)?.checked_div(length)?;
    if !quotient.is_integral() {
        return Err(invalid("weighted Slow child cycle"));
    }
    let k = quotient.num();
    let begin = d
        .checked_mul(c)?
        .checked_add(prefix)?
        .checked_add(length.checked_mul(Ratio64::from_int(k))?)?;
    if begin != whole.begin {
        return Err(invalid("weighted Slow congruence replay"));
    }
    Ok(Some(cycle))
}

fn joint_requirement(bound: &super::super::index::BoundSliceOperands<'_>) -> Failure {
    super::super::index::IndexRealizationRequest {
        prepared: bound.prepared,
        timing: Some(bound.timing),
        operand: bound.index_root,
        kind: crate::song::source_uses::timing::FrozenIndexDynamicKind::UnresolvedPattern,
    }
    .failure()
}
fn integer_ceiling(value: Ratio64) -> Result<i64, Failure> {
    i64::try_from(ceil_signed(value)?).map_err(|_| capacity_overflow())
}
/// Exact first connected whole footprint for a direct selected-source clock.
/// Partial joint constraints that have no proof here retain genuine authority.
pub(in crate::song::routing) fn index_configuration(
    bound: &super::super::index::BoundSliceOperands<'_>,
    source: TimeSpan,
    owner: TimeSpan,
    depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<TimeSpan, Failure> {
    use crate::song::source_uses::FrozenUseOperation as Op;
    match super::super::index::admit_index_support(bound, depth, budget)? {
        super::super::index::IndexSupportAdmission::RequiresRealization(request) => {
            return Err(request.failure())
        }
        super::super::index::IndexSupportAdmission::Static(_) => {}
    }
    if bound.timing.sample_start() < source.begin || bound.timing.sample_start() >= source.end {
        return Err(joint_requirement(bound));
    }
    let root = bound
        .recipe
        .nodes()
        .get(bound.index_root as usize)
        .ok_or_else(|| invalid("Index component root"))?;
    if root.operation() != Op::Steps {
        return Err(joint_requirement(bound));
    }
    let Some(program) =
        super::super::index::compile_static_families(&bound.prepared, depth, budget)?
    else {
        return Err(joint_requirement(bound));
    };
    let Some((selected, copy)) = program.matched(bound, depth, budget)? else {
        return Err(invalid("issued Index family producer absent"));
    };
    let families = program.families();
    let selected = &families[selected];
    let emitted_prefix = selected.prefix.checked_add(
        selected
            .width
            .checked_mul(Ratio64::from_int(i64::from(copy)))?,
    )?;
    let emitted_cycle = weighted_slow_whole_cycle(
        emitted_prefix,
        selected.width,
        selected.slow,
        bound.timing.index_whole(),
        Some(owner),
        budget,
    )?
    .ok_or_else(|| invalid("issued family whole differs from exact producer clock"))?;
    if let Some(component) = cluster::family_component(
        families,
        emitted_cycle,
        bound.timing.index_whole(),
        source,
        owner,
        budget,
    )? {
        return Ok(component);
    }
    // The established one-slot solver also handles partial source membership.
    // Unproved multi-family filtering stays addressed; never replace it by a hull.
    if families.len() != 1 || families[0].copies != 1 {
        return Err(joint_requirement(bound));
    }
    let geometry = &families[0];
    let slow = geometry.slow;
    let whole = bound.timing.index_whole();
    let c = weighted_slow_whole_cycle(
        geometry.prefix,
        geometry.width,
        slow,
        whole,
        Some(owner),
        budget,
    )?
    .ok_or_else(|| invalid("issued Index whole differs from prepared weighted clock"))?;
    let width = geometry.width;
    let prefix = geometry.prefix;
    let length = width.checked_mul(slow)?;
    let d = Ratio64::ONE.checked_sub(width)?;
    let first_parent = owner
        .begin
        .checked_sub(prefix)?
        .checked_sub(width)?
        .floor()
        .checked_add(1)
        .ok_or_else(capacity_overflow)?;
    let end_parent = integer_ceiling(owner.end.checked_sub(prefix)?)?;
    let span = if width == Ratio64::ONE {
        let begin_k = Ratio64::from_int(first_parent)
            .checked_div(slow)?
            .floor()
            .max(integer_ceiling(
                source.begin.checked_sub(prefix)?.checked_div(slow)?,
            )?);
        let end_k = integer_ceiling(Ratio64::from_int(end_parent).checked_div(slow)?)?.min(
            integer_ceiling(source.end.checked_sub(prefix)?.checked_div(slow)?)?,
        );
        TimeSpan::new(
            prefix.checked_add(slow.checked_mul(Ratio64::from_int(begin_k))?)?,
            prefix.checked_add(slow.checked_mul(Ratio64::from_int(end_k))?)?,
        )?
    } else if d <= length {
        let period = slow.num();
        let block = c
            .div_euclid(period)
            .checked_mul(period)
            .ok_or_else(capacity_overflow)?;
        let first = block.max(first_parent);
        let last = block
            .checked_add(period)
            .ok_or_else(capacity_overflow)?
            .min(end_parent)
            .checked_sub(1)
            .ok_or_else(capacity_overflow)?;
        if first > last {
            return Err(invalid("Index parent block outside issuing owner"));
        }
        let first = Ratio64::from_int(first);
        let last = Ratio64::from_int(last);
        let begin = d.checked_mul(first)?.checked_add(prefix)?.checked_add(
            length.checked_mul(Ratio64::from_int(first.checked_div(slow)?.floor()))?,
        )?;
        let end = d
            .checked_mul(last)?
            .checked_add(prefix)?
            .checked_add(length.checked_mul(Ratio64::from_int(integer_ceiling(
                last.checked_add(Ratio64::ONE)?.checked_div(slow)?,
            )?))?)?;
        let component = TimeSpan::new(begin.max(owner.begin), end.min(owner.end))?;
        let all_eligible = begin >= source.begin && end.checked_sub(length)? < source.end;
        let covered_by_issued = whole.begin <= component.begin && whole.end >= component.end;
        if !all_eligible && !covered_by_issued {
            return Err(joint_requirement(bound));
        }
        component
    } else {
        let c = Ratio64::from_int(c);
        let base = d.checked_mul(c)?.checked_add(prefix)?;
        let first = c.checked_div(slow)?.floor().max(integer_ceiling(
            source.begin.checked_sub(base)?.checked_div(length)?,
        )?);
        let last = integer_ceiling(c.checked_add(Ratio64::ONE)?.checked_div(slow)?)?.min(
            integer_ceiling(source.end.checked_sub(base)?.checked_div(length)?)?,
        );
        if first >= last {
            return Err(invalid("issued Index lacks eligible parent whole"));
        }
        TimeSpan::new(
            base.checked_add(length.checked_mul(Ratio64::from_int(first))?)?,
            base.checked_add(length.checked_mul(Ratio64::from_int(last))?)?,
        )?
    };
    let span = TimeSpan::new(span.begin.max(owner.begin), span.end.min(owner.end))?;
    if span.begin >= span.end || whole.end <= span.begin || whole.begin >= span.end {
        return Err(invalid("issued Index outside maximal owner component"));
    }
    Ok(span)
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    fn ratio(n: i64, d: i64) -> Ratio64 {
        Ratio64::new(n, d).unwrap()
    }
    #[test]
    fn exact_weighted_slow_cycles_cover_fractional_and_large_denominators() {
        for (width, slow, begin, end, expected) in [
            (
                ratio(1, 2),
                Ratio64::from_int(2),
                Ratio64::ZERO,
                Ratio64::ONE,
                0,
            ),
            (
                ratio(1, 2),
                Ratio64::from_int(2),
                ratio(-3, 2),
                ratio(-1, 2),
                -1,
            ),
            (
                ratio(1, 2),
                ratio(3, 2),
                ratio(5, 4),
                Ratio64::from_int(2),
                1,
            ),
            (
                ratio(1, 1_000_000_000_000_000),
                Ratio64::from_int(2),
                ratio(999_999_999_999_999, 1_000_000_000_000_000),
                ratio(1_000_000_000_000_001, 1_000_000_000_000_000),
                1,
            ),
        ] {
            let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
            assert_eq!(
                weighted_slow_whole_cycle(
                    Ratio64::ZERO,
                    width,
                    slow,
                    TimeSpan::new(begin, end).unwrap(),
                    None,
                    &mut budget
                )
                .unwrap(),
                Some(expected)
            );
        }
    }
    #[test]
    fn replay_uses_one_cumulative_budget_with_exact_one_less_refusal() {
        let limits = crate::song::SongLimits::default();
        let whole = TimeSpan::new(ratio(5, 4), Ratio64::from_int(2)).unwrap();
        let mut measured = ResolutionBudget::new(limits);
        assert_eq!(
            weighted_slow_whole_cycle(
                Ratio64::ZERO,
                ratio(1, 2),
                ratio(3, 2),
                whole,
                None,
                &mut measured
            )
            .unwrap(),
            Some(1)
        );
        let work = limits.max_nodes - measured.limits().max_nodes;
        assert!(work > 0);
        let mut exact = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: work,
            ..limits
        });
        assert_eq!(
            weighted_slow_whole_cycle(
                Ratio64::ZERO,
                ratio(1, 2),
                ratio(3, 2),
                whole,
                None,
                &mut exact
            )
            .unwrap(),
            Some(1)
        );
        assert_eq!(exact.limits().max_nodes, 0);
        let mut short = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: work - 1,
            ..limits
        });
        assert_eq!(
            weighted_slow_whole_cycle(
                Ratio64::ZERO,
                ratio(1, 2),
                ratio(3, 2),
                whole,
                None,
                &mut short
            )
            .unwrap_err()
            .code,
            crate::vm::fail::FailCode::FuelExhausted
        );
    }
    #[test]
    fn emitting_owner_selects_later_authentic_cycle_without_guessing_birth_floor() {
        let whole = TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(5)).unwrap();
        let owner = TimeSpan::new(Ratio64::from_int(2), Ratio64::from_int(3)).unwrap();
        let mut global = ResolutionBudget::new(crate::song::SongLimits::default());
        assert_eq!(
            weighted_slow_whole_cycle(
                Ratio64::ZERO,
                Ratio64::ONE,
                Ratio64::from_int(5),
                whole,
                None,
                &mut global
            )
            .unwrap(),
            Some(0)
        );
        let mut scoped = ResolutionBudget::new(crate::song::SongLimits::default());
        assert_eq!(
            weighted_slow_whole_cycle(
                Ratio64::ZERO,
                Ratio64::ONE,
                Ratio64::from_int(5),
                whole,
                Some(owner),
                &mut scoped
            )
            .unwrap(),
            Some(2)
        );
    }
}
