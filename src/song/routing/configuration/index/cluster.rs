//! Compressed same-clock families; owner clipping is applied before child ranks.
use super::super::super::index::StaticIndexFamily as IndexFamily;
use super::*;
fn merge(a: TimeSpan, b: TimeSpan) -> Option<TimeSpan> {
    if a.end < b.begin || b.end < a.begin {
        None
    } else {
        Some(TimeSpan {
            begin: a.begin.min(b.begin),
            end: a.end.max(b.end),
        })
    }
}
fn footprint(f: &IndexFamily, c: i64, j: i64, owner: TimeSpan) -> Result<TimeSpan, Failure> {
    let c = Ratio64::from_int(c);
    let p = f
        .prefix
        .checked_add(f.width.checked_mul(Ratio64::from_int(j))?)?;
    let slot = c.checked_add(p)?;
    let lo = owner.begin.max(slot);
    let hi = owner.end.min(slot.checked_add(f.width)?);
    if lo >= hi {
        return Err(invalid("empty Index copy intersection"));
    }
    let inner_lo = c.checked_add(lo.checked_sub(slot)?.checked_div(f.width)?)?;
    let inner_hi = c.checked_add(hi.checked_sub(slot)?.checked_div(f.width)?)?;
    let base = Ratio64::ONE
        .checked_sub(f.width)?
        .checked_mul(c)?
        .checked_add(p)?;
    let length = f.width.checked_mul(f.slow)?;
    TimeSpan::new(
        base.checked_add(
            length.checked_mul(Ratio64::from_int(inner_lo.checked_div(f.slow)?.floor()))?,
        )?,
        base.checked_add(length.checked_mul(Ratio64::from_int(integer_ceiling(
            inner_hi.checked_div(f.slow)?,
        )?))?)?,
    )
}
fn parent_cluster(
    families: &[IndexFamily],
    c: i64,
    owner: TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    let mut joined = None;
    for f in families {
        budget.charge(12)?;
        let base = Ratio64::from_int(c).checked_add(f.prefix)?;
        // Positive slot intersections give an OPEN prefix-rank interval.
        let first = owner
            .begin
            .checked_sub(base)?
            .checked_sub(f.width)?
            .checked_div(f.width)?
            .floor()
            .checked_add(1)
            .ok_or_else(capacity_overflow)?
            .max(0);
        let end = integer_ceiling(owner.end.checked_sub(base)?.checked_div(f.width)?)?
            .min(i64::from(f.copies));
        if first >= end {
            continue;
        }
        let mut span = footprint(f, c, first, owner)?;
        if end - first > 2 {
            // Interior copies see a full unit child query. Their minimum
            // footprint is w*s*ceil(1/s), at least the copy spacing w.
            let interior_first = footprint(f, c, first + 1, owner)?;
            let interior_last = footprint(f, c, end - 2, owner)?;
            let interior = TimeSpan::new(interior_first.begin, interior_last.end)?;
            let Some(next) = merge(span, interior) else {
                return Ok(None);
            };
            span = next;
        }
        if end - first > 1 {
            let Some(next) = merge(span, footprint(f, c, end - 1, owner)?) else {
                return Ok(None);
            };
            span = next;
        }
        joined = match joined {
            None => Some(span),
            Some(old) => match merge(old, span) {
                Some(span) => Some(span),
                None => return Ok(None),
            },
        };
    }
    Ok(joined)
}
pub(super) fn family_component(
    families: &[IndexFamily],
    emitted_cycle: i64,
    issued: TimeSpan,
    source: TimeSpan,
    owner: TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure> {
    let first = families
        .first()
        .ok_or_else(|| invalid("empty Index family"))?;
    let mut max_prefix = first.prefix;
    let mut previous_end = first.prefix;
    let length = first.width.checked_mul(first.slow)?;
    let minimum = length.checked_mul(Ratio64::from_int(integer_ceiling(
        Ratio64::ONE.checked_div(first.slow)?,
    )?))?;
    for (index, f) in families.iter().enumerate() {
        budget.charge(8)?;
        if f.width != first.width || f.slow != first.slow || f.copies == 0 {
            return Ok(None);
        }
        if index > 0 && (f.prefix < previous_end || f.prefix.checked_sub(previous_end)? > minimum) {
            return Ok(None);
        }
        previous_end = f.prefix.checked_add(
            f.width
                .checked_mul(Ratio64::from_int(i64::from(f.copies) - 1))?,
        )?;
        max_prefix = previous_end;
    }
    let a = Ratio64::ONE.checked_sub(first.width)?;
    let range = max_prefix.checked_sub(first.prefix)?;
    let parent_first = owner
        .begin
        .checked_sub(max_prefix)?
        .checked_sub(first.width)?
        .floor()
        .checked_add(1)
        .ok_or_else(capacity_overflow)?;
    let parent_end = integer_ceiling(owner.end.checked_sub(first.prefix)?)?;
    let continuous = range >= a;
    let (mut lo, mut hi) = if continuous {
        (
            parent_first,
            parent_end.checked_sub(1).ok_or_else(capacity_overflow)?,
        )
    } else {
        let period = first.slow.num();
        let start = emitted_cycle
            .div_euclid(period)
            .checked_mul(period)
            .ok_or_else(capacity_overflow)?;
        (
            start.max(parent_first),
            start
                .checked_add(period)
                .ok_or_else(capacity_overflow)?
                .min(parent_end)
                .checked_sub(1)
                .ok_or_else(capacity_overflow)?,
        )
    };
    if lo > hi || emitted_cycle < lo || emitted_cycle > hi {
        return Ok(None);
    }
    if lo < hi && length.checked_add(range)? < a {
        return Ok(None);
    }
    let Some(mut head) = parent_cluster(families, lo, owner, budget)? else {
        return Ok(None);
    };
    let Some(mut tail) = parent_cluster(families, hi, owner, budget)? else {
        return Ok(None);
    };
    // Trimming partial boundary queries can disconnect a head/tail even though
    // complete parent clusters connect. Check those actual boundaries explicitly.
    if lo < hi {
        let Some(next) = parent_cluster(families, lo + 1, owner, budget)? else {
            return Ok(None);
        };
        if merge(head, next).is_none() {
            if emitted_cycle == lo {
                tail = head;
                hi = lo;
            } else {
                lo += 1;
                head = next;
            }
        }
    }
    if lo < hi {
        let Some(previous) = parent_cluster(families, hi - 1, owner, budget)? else {
            return Ok(None);
        };
        if merge(previous, tail).is_none() {
            if emitted_cycle == hi {
                head = tail;
            } else {
                tail = previous;
            }
        }
    }
    let raw = TimeSpan::new(head.begin, tail.end)?;
    // All contributing sample STARTs must belong to this exact source config.
    // A whole's end is not a source membership test and must not be cropped.
    if raw.begin < source.begin || raw.end.checked_sub(length)? >= source.end {
        return Ok(None);
    }
    let component = TimeSpan::new(raw.begin.max(owner.begin), raw.end.min(owner.end))?;
    if component.begin >= component.end
        || issued.end <= component.begin
        || issued.begin >= component.end
    {
        return Ok(None);
    }
    Ok(Some(component))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r(n: i64, d: i64) -> Ratio64 {
        Ratio64::new(n, d).unwrap()
    }
    fn span(a: Ratio64, b: Ratio64) -> TimeSpan {
        TimeSpan::new(a, b).unwrap()
    }
    #[test]
    fn partial_owner_filters_prefix_parts_before_whole_union() {
        let families = [
            IndexFamily {
                prefix: r(0, 1),
                width: r(1, 4),
                copies: 1,
                slow: r(2, 1),
            },
            IndexFamily {
                prefix: r(1, 2),
                width: r(1, 4),
                copies: 1,
                slow: r(2, 1),
            },
        ];
        let owner = span(r(2, 5), r(3, 5));
        let source = span(r(0, 1), r(1, 4));
        let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
        assert_eq!(
            family_component(
                &families,
                0,
                span(r(1, 2), r(1, 1)),
                source,
                owner,
                &mut budget
            )
            .unwrap(),
            None
        );
        // The second actual prefix samples 1/2, not zero: this narrow source
        // correctly refuses it even though the first (ineligible part) samples0.
        let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
        assert_eq!(
            family_component(
                &families,
                0,
                span(r(1, 2), r(1, 1)),
                span(r(0, 1), r(2, 1)),
                owner,
                &mut budget
            )
            .unwrap(),
            Some(span(r(1, 2), r(3, 5)))
        );
    }
    #[test]
    fn symbolic_large_copy_run_has_exact_constant_work_and_one_less_refusal() {
        let families = [IndexFamily {
            prefix: r(0, 1),
            width: r(1, 1_000_001),
            copies: 1_000_000,
            slow: r(1, 2),
        }];
        let source = span(r(0, 1), r(4, 1));
        let owner = span(r(0, 1), r(1, 1));
        let issued = span(r(0, 1), r(1, 2_000_002));
        let limits = crate::song::SongLimits::default();
        let mut budget = ResolutionBudget::new(limits);
        let result = family_component(&families, 0, issued, source, owner, &mut budget).unwrap();
        assert!(result.is_some());
        let work = limits.max_nodes - budget.limits().max_nodes;
        assert!(work < 100);
        let mut exact = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: work,
            ..limits
        });
        assert_eq!(
            family_component(&families, 0, issued, source, owner, &mut exact).unwrap(),
            result
        );
        assert_eq!(exact.limits().max_nodes, 0);
        let mut short = ResolutionBudget::new(crate::song::SongLimits {
            max_nodes: work - 1,
            ..limits
        });
        assert_eq!(
            family_component(&families, 0, issued, source, owner, &mut short)
                .unwrap_err()
                .code,
            crate::vm::fail::FailCode::FuelExhausted
        );
    }
    #[test]
    fn eligible_start_does_not_crop_whole_at_source_end() {
        let families = [IndexFamily {
            prefix: r(0, 1),
            width: r(1, 4),
            copies: 1,
            slow: r(2, 1),
        }];
        let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
        assert_eq!(
            parent_cluster(&families, 0, span(r(0, 1), r(1, 5)), &mut budget).unwrap(),
            Some(span(r(0, 1), r(1, 2)))
        );
        let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
        assert_eq!(
            family_component(
                &families,
                0,
                span(r(0, 1), r(1, 2)),
                span(r(0, 1), r(1, 4)),
                span(r(0, 1), r(1, 5)),
                &mut budget
            )
            .unwrap(),
            Some(span(r(0, 1), r(1, 5)))
        );
    }
}
