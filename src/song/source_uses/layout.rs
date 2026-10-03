//! Checked exact cycle-local slot geometry shared by cover and resolution.
use super::{use_invalid, use_overflow, FrozenSourceUseEdge, FrozenUseTraceTerm};
use crate::pattern::occ::ProducerStep;
use crate::pattern::query::TimeSpan;
use crate::value::ratio::Ratio64;
use crate::vm::fail::Failure;

/// One normalized nested slot. Repeated copies stay symbolic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenUseSlotLayout {
    pub prefix: Ratio64,
    pub width: Ratio64,
    /// Index into this edge's typed trace, never a producer-word offset.
    pub copy_trace_term: Option<u32>,
}
/// Validates all slot ranges and symbolic copy references before use.
pub fn validate_layout(edge: &FrozenSourceUseEdge) -> Result<(), Failure> {
    for slot in &edge.layout {
        let count = match slot.copy_trace_term {
            Some(index) => match edge.trace.get(index as usize) {
                Some(FrozenUseTraceTerm::Copies { count, .. }) if *count > 0 && *count <= 4096 => {
                    *count
                }
                _ => return Err(use_invalid("invalid layout copy trace term")),
            },
            None => 1,
        };
        if slot.prefix < Ratio64::ZERO
            || slot.width <= Ratio64::ZERO
            || slot.prefix.checked_add(
                slot.width
                    .checked_mul(Ratio64::from_int(i64::from(count)))?,
            )? > Ratio64::ONE
        {
            return Err(use_invalid("invalid normalized source slot"));
        }
    }
    Ok(())
}
fn composed(
    edge: &FrozenSourceUseEdge,
    actual: Option<&[ProducerStep]>,
    last: bool,
) -> Result<(Ratio64, Ratio64), Failure> {
    validate_layout(edge)?;
    if let Some(actual) = actual {
        if actual.len() != edge.trace.len() {
            return Err(use_invalid("layout trace length mismatch"));
        }
        for (term, step) in edge.trace.iter().zip(actual) {
            match term {
                FrozenUseTraceTerm::Exact(expected) if expected != step => {
                    return Err(use_invalid("layout trace mismatch"))
                }
                FrozenUseTraceTerm::Copies { kind, count }
                    if *kind != step.kind || step.ordinal >= *count =>
                {
                    return Err(use_invalid("layout copy mismatch"))
                }
                _ => {}
            }
        }
    }
    let mut prefix = Ratio64::ZERO;
    let mut width = Ratio64::ONE;
    for slot in &edge.layout {
        let ordinal = match slot.copy_trace_term {
            None => 0,
            Some(index) => {
                let Some(FrozenUseTraceTerm::Copies { kind, count }) =
                    edge.trace.get(index as usize)
                else {
                    return Err(use_invalid("invalid layout trace"));
                };
                if let Some(actual) = actual {
                    let step = actual
                        .get(index as usize)
                        .ok_or_else(|| use_invalid("missing layout copy ordinal"))?;
                    if step.kind != *kind || step.ordinal >= *count {
                        return Err(use_invalid("invalid layout copy ordinal"));
                    }
                    step.ordinal
                } else if last {
                    count.checked_sub(1).ok_or_else(use_overflow)?
                } else {
                    0
                }
            }
        };
        let local = slot.prefix.checked_add(
            slot.width
                .checked_mul(Ratio64::from_int(i64::from(ordinal)))?,
        )?;
        prefix = prefix.checked_add(width.checked_mul(local)?)?;
        width = width.checked_mul(slot.width)?;
    }
    Ok((prefix, width))
}
/// Exact output slot in its containing cycle, using authenticated copy terms.
pub fn slot_interval(
    edge: &FrozenSourceUseEdge,
    cycle: i64,
    actual_trace: &[ProducerStep],
) -> Result<TimeSpan, Failure> {
    let (prefix, width) = composed(edge, Some(actual_trace), false)?;
    let begin = Ratio64::from_int(cycle).checked_add(prefix)?;
    TimeSpan::new(begin, begin.checked_add(width)?)
}
/// Maps a full source configuration to output without clipping or note keys.
pub fn map_slot_configuration(
    edge: &FrozenSourceUseEdge,
    cycle: i64,
    source: TimeSpan,
    actual_trace: &[ProducerStep],
) -> Result<TimeSpan, Failure> {
    if source.end < source.begin {
        return Err(use_invalid("invalid source configuration support"));
    }
    let (prefix, width) = composed(edge, Some(actual_trace), false)?;
    let base = Ratio64::from_int(cycle);
    source.map(|t| {
        base.checked_add(prefix)?
            .checked_add(t.checked_sub(base)?.checked_mul(width)?)
    })
}
/// Inverse squeeze support. None actual_trace denotes a symbolic all-copy cover.
pub fn map_slot_support(
    edge: &FrozenSourceUseEdge,
    window: TimeSpan,
    actual_trace: Option<&[ProducerStep]>,
) -> Result<Option<TimeSpan>, Failure> {
    if window.end < window.begin {
        return Err(use_invalid("invalid slot support window"));
    }
    let (prefix, width) = composed(edge, actual_trace, false)?;
    if edge.layout.is_empty() {
        return Ok(Some(window));
    }
    let first = window.begin.floor();
    let last = if window.is_point() {
        first
    } else {
        let floor = window.end.floor();
        if window.end == Ratio64::from_int(floor) {
            floor.checked_sub(1).ok_or_else(use_overflow)?
        } else {
            floor
        }
    };
    if last > first {
        // Union envelope across cycles; no expanded repeat copies or cycle loop.
        return Ok(Some(TimeSpan::new(
            Ratio64::from_int(first),
            Ratio64::from_int(last.checked_add(1).ok_or_else(use_overflow)?),
        )?));
    }
    let base = Ratio64::from_int(first);
    let (last_prefix, last_width) = composed(edge, actual_trace, true)?;
    let start = base.checked_add(prefix)?;
    let end = base.checked_add(last_prefix)?.checked_add(last_width)?;
    if window.is_point() {
        if window.begin < start || window.begin >= end {
            return Ok(None);
        }
    } else if window.end <= start || window.begin >= end {
        return Ok(None);
    }
    if actual_trace.is_none() && edge.layout.iter().any(|s| s.copy_trace_term.is_some()) {
        return Ok(Some(TimeSpan::cycle(first)?));
    }
    let begin = window.begin.max(start);
    let end = window.end.min(end);
    let mapped = TimeSpan::new(
        base.checked_add(begin.checked_sub(start)?.checked_div(width)?)?,
        base.checked_add(end.checked_sub(start)?.checked_div(width)?)?,
    )?;
    Ok(Some(mapped))
}
