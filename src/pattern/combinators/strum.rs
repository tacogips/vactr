//! Chord performance transforms: time spreading, plate selection and inversion.

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::pattern::combinators::control::query_child;
use crate::pattern::combinators::{event_fault, kw, no_whole};
use crate::pattern::eval::{eval_param, exact_value, int_of, num_f64, num_ratio, QState};
use crate::pattern::occ::ProducerKind;
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{sect, Event, TimeSpan};
use crate::pattern::rng::{below, Hasher};
use crate::pattern::tuning::Tuning;
use crate::reader::span::Span;
use crate::value::intern::name_of_kw;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

fn type_err(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, message)
}

/// Spreads chord tones in time. The subject's structure is preserved.
#[must_use]
pub fn strum(p: Rc<Pat>, time: PParam, dir: PParam, curve: PParam, span: Option<Span>) -> Pat {
    let structured = p.structured;
    Pat::new(PatNode::Strum(p, time, dir, curve), span, structured)
}

/// Selects one note from a multi-period chord plate.
///
/// # Errors
/// Returns `Type` when `strips` is outside `1..=64`.
pub fn harp(
    p: Rc<Pat>,
    pos: PParam,
    strips: i64,
    base: Option<i64>,
    span: Option<Span>,
) -> Result<Pat, Failure> {
    if !(1..=64).contains(&strips) {
        return Err(type_err("harp strips must be between 1 and 64"));
    }
    let structured = p.structured;
    Ok(Pat::new(
        PatNode::Harp {
            subject: p,
            pos,
            strips,
            base,
        },
        span,
        structured,
    ))
}

/// Rotates chord tones by `n` scale periods; `bass` adds a lower root tone.
#[must_use]
pub fn inversion(p: Rc<Pat>, n: PParam, bass: bool, span: Option<Span>) -> Pat {
    let structured = p.structured;
    Pat::new(
        PatNode::Inversion {
            subject: p,
            n,
            bass,
        },
        span,
        structured,
    )
}

fn tuning(e: &Event) -> Result<(i64, i64), Failure> {
    match e.controls.get(&kw("tuning")).and_then(Tuning::from_control) {
        Some(Ok(t)) => Ok((t.keys_per_period(), t.root())),
        Some(Err(f)) => Err(f),
        None => Ok((12, 60)),
    }
}

fn note_key(value: &Value, tuning: Option<&Tuning>) -> Result<i64, Failure> {
    match value {
        Value::Keyword(k) => {
            let name = name_of_kw(*k);
            if let Some(t) = tuning {
                t.note_key(&name)?
                    .ok_or_else(|| type_err("unknown note name"))
            } else {
                plain_note_number(&name).ok_or_else(|| type_err("unknown note name"))
            }
        }
        other => {
            int_of(other).ok_or_else(|| type_err("chord tones must be integer notes or note names"))
        }
    }
}

fn plain_note_number(name: &str) -> Option<i64> {
    let mut chars = name.chars();
    let pc = match chars.next()? {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (shift, octave) = match rest.chars().next() {
        Some('s') => (1, &rest[1..]),
        Some('f' | 'b') => (-1, &rest[1..]),
        _ => (0, rest),
    };
    let octave = if octave.is_empty() {
        5
    } else if octave.bytes().all(|b| b.is_ascii_digit()) && octave.len() <= 2 {
        octave.parse().ok()?
    } else {
        return None;
    };
    12_i64.checked_mul(octave)?.checked_add(pc + shift)
}

fn chord_values(e: &Event) -> Option<&[Value]> {
    match e.controls.get(&kw("note")) {
        Some(Value::List(list)) => Some(&list.items),
        _ => None,
    }
}

type ChordTone = (i64, usize, Value);
type EventChord = (Vec<ChordTone>, i64, i64);

fn values_for_event(e: &Event) -> Result<EventChord, Failure> {
    let (period, root) = tuning(e)?;
    let tuned = e.controls.get(&kw("tuning")).and_then(Tuning::from_control);
    let tuning = match tuned {
        Some(Ok(t)) => Some(t),
        Some(Err(f)) => return Err(f),
        None => None,
    };
    let values = chord_values(e).ok_or_else(|| type_err("expected a chord note list"))?;
    let notes = values
        .iter()
        .enumerate()
        .map(|(index, value)| Ok((note_key(value, tuning.as_ref())?, index, value.clone())))
        .collect::<Result<Vec<_>, Failure>>()?;
    Ok((notes, period, root))
}

pub(crate) fn query_strum(
    inner: &Pat,
    time: &PParam,
    dir: &PParam,
    curve: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for e in query_child(inner, span, 0, st) {
        let Some(_values) = chord_values(&e) else {
            out.push(e);
            continue;
        };
        let result = (|| {
            let whole = e.whole.ok_or_else(no_whole)?;
            let duration = whole.duration()?;
            let time_value = eval_param(time, e.anchor(), st)?;
            let time = match &time_value {
                Value::Int(_) | Value::Int64(_) | Value::Ratio(_) => num_ratio(&time_value),
                Value::Float(_) | Value::Float64(_) => {
                    return Err(type_err(
                        "strum time must be exact; use a ratio such as 1/32",
                    ));
                }
                _ => None,
            }
            .ok_or_else(|| type_err("strum time must be an Int or Ratio"))?;
            if time < Ratio64::ZERO {
                return Err(type_err("strum time must be nonnegative"));
            }
            let direction = keyword_param(dir, e.anchor(), st, "strum direction")?;
            let curve = keyword_param(curve, e.anchor(), st, "strum curve")?;
            let mut notes = values_for_event(&e)?.0;
            notes.sort_by_key(|(key, _, _)| *key);
            match direction.as_str() {
                "up" => {}
                "down" => notes.reverse(),
                "alternate" => {
                    let up = duration == Ratio64::ZERO
                        || (whole.begin.checked_div(duration)?.floor().rem_euclid(2) == 0);
                    if !up {
                        notes.reverse();
                    }
                }
                "random" => {
                    let seq = Hasher::new(0x7374_7275).ratio(whole.begin);
                    for i in (1..notes.len()).rev() {
                        let j = usize::try_from(below(
                            st.cx.seed,
                            p.id,
                            whole.begin.floor(),
                            seq.word(i as u64).finish(),
                            (i + 1) as u64,
                        ))
                        .unwrap_or(0);
                        notes.swap(i, j);
                    }
                }
                _ => {
                    return Err(type_err(
                        "strum direction must be :up, :down, :alternate, or :random",
                    ))
                }
            }
            let count = i64::try_from(notes.len()).map_err(|_| type_err("chord is too large"))?;
            if count == 0 {
                return Ok(Vec::new());
            }
            let step = duration.checked_div(Ratio64::from_int(count))?;
            let dt = time.min(step);
            let mut children = Vec::new();
            for (i, (_, original_index, tone)) in notes.iter().enumerate() {
                let offset = dt.checked_mul(Ratio64::from_int(i as i64))?;
                let begin = whole.begin.checked_add(offset)?;
                let child_whole = TimeSpan {
                    begin,
                    end: whole.end,
                };
                let Some(part) = sect(child_whole, e.part) else {
                    continue;
                };
                let mut child = e.clone();
                child.whole = Some(child_whole);
                child.part = part;
                child.controls.insert(kw("note"), tone.clone());
                child
                    .occ
                    .push(p.id, u32::try_from(*original_index).unwrap_or(u32::MAX));
                if let Some(trace) = &mut child.producer {
                    trace.push(
                        ProducerKind::GeneratedBranch,
                        u32::try_from(*original_index).unwrap_or(u32::MAX),
                    );
                }
                if curve != "flat" {
                    let multiplier = if count == 1 {
                        1.0
                    } else {
                        let fraction = i as f64 / (count - 1) as f64;
                        if curve == "fade" {
                            1.0 - 0.5 * fraction
                        } else {
                            0.5 + 0.5 * fraction
                        }
                    };
                    let target = if child.controls.contains_key(&kw("velocity")) {
                        kw("velocity")
                    } else {
                        kw("gain")
                    };
                    let old = child.controls.get(&target).and_then(num_f64).unwrap_or(1.0);
                    child
                        .controls
                        .insert(target, Value::Float64(old * multiplier));
                }
                children.push(child);
            }
            Ok(children)
        })();
        match result {
            Ok(children) => out.extend(children),
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_harp(
    inner: &Pat,
    pos: &PParam,
    strips: i64,
    base: Option<i64>,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut e in query_child(inner, span, 0, st) {
        if chord_values(&e).is_none() {
            out.push(e);
            continue;
        }
        let result = (|| {
            let (notes, period, root) = values_for_event(&e)?;
            let low = base.unwrap_or(
                root.checked_sub(period)
                    .ok_or_else(|| Failure::new(FailCode::Overflow, "harp base out of range"))?,
            );
            let pcs: BTreeSet<i64> = notes
                .iter()
                .map(|(key, _, _)| key.saturating_sub(root).rem_euclid(period))
                .collect();
            let mut plate = Vec::new();
            let search_limit = strips
                .checked_mul(period)
                .and_then(|n| n.checked_add(period))
                .ok_or_else(|| Failure::new(FailCode::Overflow, "harp plate range overflow"))?;
            for offset in 0..search_limit {
                if i64::try_from(plate.len()).unwrap_or(i64::MAX) >= strips {
                    break;
                }
                let key = low
                    .checked_add(offset)
                    .ok_or_else(|| Failure::new(FailCode::Overflow, "harp key out of range"))?;
                if pcs.contains(&key.saturating_sub(root).rem_euclid(period)) {
                    plate.push(key);
                }
            }
            if plate.is_empty() {
                return Err(type_err("the chord has no notes on this harp plate"));
            }
            let value = eval_param(pos, e.anchor(), st)?;
            let position =
                num_f64(&value).ok_or_else(|| type_err("harp position must be numeric"))?;
            if position.is_nan() {
                return Err(type_err("harp position cannot be NaN"));
            }
            let position = position.clamp(0.0, 1.0);
            let index =
                ((position * strips as f64).floor() as usize).min(plate.len().saturating_sub(1));
            e.controls
                .insert(kw("note"), exact_value(Ratio64::from_int(plate[index])));
            Ok(())
        })();
        match result {
            Ok(()) => out.push(e),
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_inversion(
    inner: &Pat,
    n: &PParam,
    bass: bool,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut e in query_child(inner, span, 0, st) {
        if chord_values(&e).is_none() {
            out.push(e);
            continue;
        }
        let result = (|| {
            let (notes, period, _) = values_for_event(&e)?;
            let value = eval_param(n, e.anchor(), st)?;
            let count = match value {
                Value::Int(value) => i64::from(value),
                Value::Int64(value) => value,
                _ => return Err(type_err("inversion n must be an Int")),
            };
            if notes.is_empty() {
                return Err(type_err("cannot invert an empty chord"));
            }
            let original_root = notes[0].0;
            let mut output: Vec<i64> = notes.iter().map(|(note, _, _)| *note).collect();
            output.sort_unstable();
            let len = i64::try_from(output.len()).map_err(|_| type_err("chord is too large"))?;
            let quotient = count.div_euclid(len);
            let remainder = count.rem_euclid(len);
            for _ in 0..remainder {
                let low = output.remove(0);
                output.push(low.checked_add(period).ok_or_else(|| {
                    Failure::new(FailCode::Overflow, "inversion key out of range")
                })?);
            }
            let shift = quotient
                .checked_mul(period)
                .ok_or_else(|| Failure::new(FailCode::Overflow, "inversion shift out of range"))?;
            for note in &mut output {
                *note = note.checked_add(shift).ok_or_else(|| {
                    Failure::new(FailCode::Overflow, "inversion key out of range")
                })?;
            }
            if bass {
                let lowest = *output
                    .first()
                    .ok_or_else(|| type_err("cannot invert an empty chord"))?;
                let mut bass_note = original_root
                    .checked_sub(period)
                    .ok_or_else(|| Failure::new(FailCode::Overflow, "bass key out of range"))?;
                while bass_note >= lowest {
                    bass_note = bass_note
                        .checked_sub(period)
                        .ok_or_else(|| Failure::new(FailCode::Overflow, "bass key out of range"))?;
                }
                output.insert(0, bass_note);
            }
            e.controls.insert(
                kw("note"),
                Value::list(
                    output
                        .into_iter()
                        .map(|note| exact_value(Ratio64::from_int(note)))
                        .collect(),
                ),
            );
            Ok(())
        })();
        match result {
            Ok(()) => out.push(e),
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

fn keyword_param(
    param: &PParam,
    at: Ratio64,
    st: &mut QState<'_, '_>,
    what: &str,
) -> Result<String, Failure> {
    match eval_param(param, at, st)? {
        Value::Keyword(k) => Ok(name_of_kw(k).to_string()),
        _ => Err(type_err(format!("{what} must be a keyword"))),
    }
}

#[cfg(test)]
mod tests;
