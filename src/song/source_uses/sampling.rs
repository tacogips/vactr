//! Bounded fractional Euclidean sample grids, distinct from cycle sampling.
use super::{
    use_invalid, FrozenSourceUseNode, FrozenUseMapping, FrozenUseOperation, FrozenUseTraceTerm,
};
use crate::pattern::occ::{ProducerKind, ProducerStep};
use crate::pattern::TimeSpan;
use crate::value::Ratio64;
use crate::vm::fail::{FailCode, Failure};

/// Raw signed integers interpreted exactly by the pattern Euclid runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenStaticSampling {
    Euclid {
        pulses: i64,
        divisions: i64,
        rotation: i64,
    },
}
fn divisions(recipe: FrozenStaticSampling) -> Result<u32, Failure> {
    let FrozenStaticSampling::Euclid { divisions, .. } = recipe;
    if !(1..=4096).contains(&divisions) {
        return Err(Failure::new(
            FailCode::Type,
            "euclid steps must be between 1 and 4096",
        ));
    }
    Ok(divisions as u32)
}
pub(crate) fn validate(node: &FrozenSourceUseNode) -> Result<(), Failure> {
    let FrozenUseMapping::SampleGrid { content, sampling } = node.mapping else {
        return Ok(());
    };
    divisions(sampling)?;
    if node.operation != FrozenUseOperation::Euclid || content != 0 || node.edges.len() != 1 {
        return Err(use_invalid("sample grid operation or content mismatch"));
    }
    let edge = &node.edges[0];
    if !edge.layout.is_empty()
        || edge.trace
            != [FrozenUseTraceTerm::Exact(ProducerStep {
                kind: ProducerKind::Child,
                ordinal: 0,
            })]
    {
        return Err(use_invalid("sample grid child trace or layout mismatch"));
    }
    Ok(())
}
fn spend(remaining: &mut u32, cost: u64) -> Result<(), Failure> {
    let cost = u32::try_from(cost)
        .map_err(|_| Failure::new(FailCode::FuelExhausted, "sample grid work exhausted"))?;
    *remaining = remaining
        .checked_sub(cost)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "sample grid work exhausted"))?;
    Ok(())
}
/// Conservative logical work/storage requests, admitted before allocating masks.
fn admitted_mask_cost(recipe: FrozenStaticSampling, remaining: &mut u32) -> Result<(), Failure> {
    let n = u64::from(divisions(recipe)?);
    let FrozenStaticSampling::Euclid { pulses, .. } = recipe;
    let (mut i, mut a, mut j, mut b) = (
        pulses.unsigned_abs().min(n),
        1u64,
        n - pulses.unsigned_abs().min(n),
        1u64,
    );
    spend(remaining, 16 + 6 * n)?;
    while i.min(j) > 1 {
        spend(remaining, 1)?;
        let m = i.min(j);
        let sum = a.checked_add(b).ok_or_else(super::use_overflow)?;
        let cost = m
            .checked_mul(sum)
            .and_then(|v| v.checked_add(i + j))
            .and_then(|v| v.checked_add(i.abs_diff(j)))
            .ok_or_else(super::use_overflow)?;
        spend(remaining, cost)?;
        if i > j {
            (i, a, j, b) = (j, sum, i - j, a);
        } else {
            (i, a, j, b) = (i, sum, j - i, b);
        }
    }
    Ok(())
}
pub(crate) fn enabled_phases(
    recipe: FrozenStaticSampling,
    remaining: &mut u32,
) -> Result<Vec<u32>, Failure> {
    admitted_mask_cost(recipe, remaining)?;
    let FrozenStaticSampling::Euclid {
        pulses,
        divisions,
        rotation,
    } = recipe;
    let mask = crate::pattern::combinators::structure::euclid_steps(pulses, divisions, rotation)?;
    Ok(mask
        .into_iter()
        .enumerate()
        .filter_map(|(i, active)| active.then_some(i as u32))
        .collect())
}
fn cell(recipe: FrozenStaticSampling, cycle: i64, phase: u32) -> Result<TimeSpan, Failure> {
    let n = i64::from(divisions(recipe)?);
    let start = Ratio64::from_int(cycle).checked_add(Ratio64::new(i64::from(phase), n)?)?;
    TimeSpan::new(start, start.checked_add(Ratio64::new(1, n)?)?)
}
fn touches(cell: TimeSpan, window: TimeSpan) -> bool {
    if window.is_point() {
        cell.begin <= window.begin && window.begin < cell.end
    } else {
        cell.begin < window.end && window.begin < cell.end
    }
}
/// Conservative cell envelope containing every sampled start; never exact membership.
pub(crate) fn support_envelope(
    recipe: FrozenStaticSampling,
    window: TimeSpan,
    remaining: &mut u32,
) -> Result<Option<TimeSpan>, Failure> {
    if window.is_point() {
        let n = i64::from(divisions(recipe)?);
        let phase = window
            .begin
            .frac()
            .checked_mul(Ratio64::from_int(n))?
            .floor();
        let slot = cell(recipe, window.begin.floor(), phase as u32)?;
        return if contains_sample(recipe, window, slot.begin, remaining)? {
            Ok(Some(slot))
        } else {
            Ok(None)
        };
    }
    let phases = enabled_phases(recipe, remaining)?;
    if phases.is_empty() {
        return Ok(None);
    }
    let first = window.begin.floor();
    let last = if window.is_point() || window.end.frac() != Ratio64::ZERO {
        window.end.floor()
    } else {
        window
            .end
            .floor()
            .checked_sub(1)
            .ok_or_else(super::use_overflow)?
    };
    let mut lo = None;
    let mut hi = None;
    let adjacent = if first < last {
        Some((
            first.checked_add(1).ok_or_else(super::use_overflow)?,
            last.checked_sub(1).ok_or_else(super::use_overflow)?,
        ))
    } else {
        None
    };
    for c in [
        Some(first),
        Some(last),
        adjacent.map(|v| v.0),
        adjacent.map(|v| v.1),
    ]
    .into_iter()
    .flatten()
    {
        if c < first || c > last {
            continue;
        }
        for &phase in &phases {
            spend(remaining, 1)?;
            let slot = cell(recipe, c, phase)?;
            if touches(slot, window) {
                lo = Some(lo.map_or(slot.begin, |v: Ratio64| v.min(slot.begin)));
                hi = Some(hi.map_or(slot.end, |v: Ratio64| v.max(slot.end)));
            }
        }
    }
    match (lo, hi) {
        (Some(begin), Some(end)) => Ok(Some(TimeSpan::new(begin, end)?)),
        _ => Ok(None),
    }
}
pub(crate) fn contains_sample(
    recipe: FrozenStaticSampling,
    window: TimeSpan,
    sampled_at: Ratio64,
    remaining: &mut u32,
) -> Result<bool, Failure> {
    let phases = enabled_phases(recipe, remaining)?;
    let n = i64::from(divisions(recipe)?);
    let scaled = sampled_at.frac().checked_mul(Ratio64::from_int(n))?;
    if scaled.frac() != Ratio64::ZERO {
        return Ok(false);
    }
    let phase = u32::try_from(scaled.floor()).map_err(|_| super::use_overflow())?;
    spend(remaining, phases.len() as u64)?;
    Ok(phases.contains(&phase) && touches(cell(recipe, sampled_at.floor(), phase)?, window))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recipe(k: i64, n: i64, r: i64) -> FrozenStaticSampling {
        FrozenStaticSampling::Euclid {
            pulses: k,
            divisions: n,
            rotation: r,
        }
    }
    #[test]
    fn masks_match_runtime_signed_complements_and_rotation() {
        for n in [1, 2, 4, 17, 64, 4096] {
            for k in [0, 1, -1, n, -n, i64::MIN, i64::MAX] {
                for r in [0, 1, -1, i64::MIN, i64::MAX] {
                    let mut work = 100_000_000;
                    let actual = enabled_phases(recipe(k, n, r), &mut work).unwrap();
                    let expected: Vec<_> =
                        crate::pattern::combinators::structure::euclid_steps(k, n, r)
                            .unwrap()
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, v)| v.then_some(i as u32))
                            .collect();
                    assert_eq!(actual, expected);
                    assert!(work < 100_000_000);
                }
            }
        }
    }
    #[test]
    fn point_cells_end_exclusivity_empty_masks_and_huge_windows() {
        let full = recipe(4, 4, 0);
        let point = TimeSpan::point(Ratio64::new(3, 8).unwrap());
        let mut work = 100_000;
        let support = support_envelope(full, point, &mut work).unwrap().unwrap();
        assert_eq!(support.begin, Ratio64::new(1, 4).unwrap());
        assert!(contains_sample(full, point, support.begin, &mut work).unwrap());
        assert!(!contains_sample(full, point, Ratio64::new(3, 8).unwrap(), &mut work).unwrap());
        let window = TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(1)).unwrap();
        assert!(contains_sample(full, window, Ratio64::new(3, 4).unwrap(), &mut work).unwrap());
        assert!(!contains_sample(full, window, Ratio64::ONE, &mut work).unwrap());
        assert!(support_envelope(recipe(0, 4, 0), window, &mut work)
            .unwrap()
            .is_none());
        let huge = TimeSpan::new(
            Ratio64::from_int(-1_000_000_000),
            Ratio64::from_int(1_000_000_000),
        )
        .unwrap();
        let before = work;
        let hull = support_envelope(full, huge, &mut work).unwrap().unwrap();
        assert_eq!(hull.begin, huge.begin);
        assert_eq!(hull.end, huge.end);
        assert!(before - work < 1000);
    }
    #[test]
    fn admission_price_exact_quota_and_one_less_are_distinct() {
        let grid = recipe(5, 17, -3);
        let mut ample = 1_000_000;
        let phases = enabled_phases(grid, &mut ample).unwrap();
        let cost = 1_000_000 - ample;
        let mut exact = cost;
        assert_eq!(enabled_phases(grid, &mut exact).unwrap(), phases);
        assert_eq!(exact, 0);
        let mut short = cost - 1;
        assert_eq!(
            enabled_phases(grid, &mut short).unwrap_err().code,
            FailCode::FuelExhausted
        );
    }
    #[test]
    fn extreme_single_cycle_needs_no_unused_neighbor_arithmetic() {
        let window =
            TimeSpan::new(Ratio64::from_int(i64::MIN), Ratio64::from_int(i64::MIN + 1)).unwrap();
        let mut work = 1000;
        assert_eq!(
            support_envelope(recipe(1, 1, 0), window, &mut work).unwrap(),
            Some(window)
        );
    }
    #[test]
    fn admitted_work_is_checked_before_masks_and_expensive_valid_inputs_fail() {
        let mut quota = 0;
        assert_eq!(
            enabled_phases(recipe(1, 4, 0), &mut quota)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        let mut quota = 1_000_000;
        assert_eq!(
            enabled_phases(recipe(2, 4096, 0), &mut quota)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        for n in [0, -1, 4097] {
            let e = enabled_phases(recipe(1, n, 0), &mut quota).unwrap_err();
            assert_eq!(e.code, FailCode::Type);
            assert_eq!(e.message, "euclid steps must be between 1 and 4096");
        }
    }
}
