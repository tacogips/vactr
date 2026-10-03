//! Notes, scales, chords, voicing, arpeggios and `range`.
//!
//! Note numbers are MIDI-style: `12 * octave + pitch class`, and a note name
//! without an octave is in octave 5, so `:c` is 60 and `:e2` is 28.

use crate::pattern::combinators::control::query_child;
use std::rc::Rc;

use crate::pattern::combinators::control::query_mapped;
use crate::pattern::combinators::{count, event_fault, kw, op, split_event};
use crate::pattern::eval::{eval_param, exact_value, int_of, num_f64, num_ratio, QState};
use crate::pattern::occ::ProducerKind;
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{Event, TimeSpan};
use crate::reader::span::Span;
use crate::value::intern::{name_of_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// Scale intervals by name.
const SCALES: &[(&str, &[i64])] = &[
    ("major", &[0, 2, 4, 5, 7, 9, 11]),
    ("ionian", &[0, 2, 4, 5, 7, 9, 11]),
    ("minor", &[0, 2, 3, 5, 7, 8, 10]),
    ("aeolian", &[0, 2, 3, 5, 7, 8, 10]),
    ("dorian", &[0, 2, 3, 5, 7, 9, 10]),
    ("phrygian", &[0, 1, 3, 5, 7, 8, 10]),
    ("lydian", &[0, 2, 4, 6, 7, 9, 11]),
    ("mixolydian", &[0, 2, 4, 5, 7, 9, 10]),
    ("locrian", &[0, 1, 3, 5, 6, 8, 10]),
    ("harmonic-minor", &[0, 2, 3, 5, 7, 8, 11]),
    ("melodic-minor", &[0, 2, 3, 5, 7, 9, 11]),
    ("major-pentatonic", &[0, 2, 4, 7, 9]),
    ("pentatonic", &[0, 2, 4, 7, 9]),
    ("minor-pentatonic", &[0, 3, 5, 7, 10]),
    ("blues", &[0, 3, 5, 6, 7, 10]),
    ("whole-tone", &[0, 2, 4, 6, 8, 10]),
    ("chromatic", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
];

pub use crate::types::chords::CHORD_QUALITIES;

fn type_err(m: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, m)
}

/// The intervals of a scale name.
#[must_use]
pub fn scale_steps(name: &str) -> Option<&'static [i64]> {
    SCALES.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The note number of a name such as `c`, `e2`, `fs3`, `eb`, `bf4`.
#[must_use]
pub fn note_number(name: &str) -> Option<i64> {
    let mut chars = name.chars();
    let pc: i64 = match chars.next()? {
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
    let octave: i64 = if octave.is_empty() {
        5
    } else if octave.bytes().all(|b| b.is_ascii_digit()) && octave.len() <= 2 {
        octave.parse().ok()?
    } else {
        return None;
    };
    Some(12 * octave + pc + shift)
}

/// A note value (a number or a note-name keyword) as a note number.
fn note_of(v: &Value) -> Option<i64> {
    match v {
        Value::Keyword(k) => note_number(&name_of_kw(*k)),
        other => int_of(other),
    }
}

/// The tones of a chord value `[root quality]`.
///
/// # Errors
/// `Type` for a malformed chord, an unknown root or an unknown quality.
pub fn chord_notes(v: &Value) -> Result<Vec<i64>, Failure> {
    let Value::List(l) = v else {
        return Err(type_err("a chord is [root quality]"));
    };
    let [root, quality] = &*l.items else {
        return Err(type_err("a chord is [root quality]"));
    };
    let root = note_of(root).ok_or_else(|| type_err("unknown chord root"))?;
    let Value::Keyword(qk) = quality else {
        return Err(type_err("a chord quality is a keyword"));
    };
    let qname = name_of_kw(*qk);
    let intervals = crate::types::chords::chord_intervals(&qname)
        .ok_or_else(|| type_err(format!("unknown chord quality :{qname}")))?;
    intervals
        .iter()
        .map(|i| {
            root.checked_add(*i)
                .ok_or_else(|| Failure::new(FailCode::Overflow, "chord root out of range"))
        })
        .collect()
}

/// A note number as a value (`Int` when it fits).
fn note_value(n: i64) -> Value {
    exact_value(Ratio64::from_int(n))
}

fn int_list(notes: &[i64]) -> Value {
    Value::list(notes.iter().map(|n| note_value(*n)).collect())
}

/// `scale p root name`: `n` degrees (or numeric values) become notes.
///
/// # Errors
/// `Type` for an unknown root or scale name.
pub fn scale(p: Rc<Pat>, root: KwId, name: KwId, span: Option<Span>) -> Result<Pat, Failure> {
    if note_number(&name_of_kw(root)).is_none() {
        return Err(type_err("unknown scale root"));
    }
    if scale_steps(&name_of_kw(name)).is_none() {
        return Err(type_err(format!("unknown scale :{}", name_of_kw(name))));
    }
    let s = p.structured;
    Ok(Pat::new(PatNode::ScaleNotes(root, name, p), span, s))
}

/// `chord value` on a subject: sets `note` to the chord tones.
#[must_use]
pub fn chord(chords: Rc<Pat>, subject: Rc<Pat>, span: Option<Span>) -> Pat {
    let s = subject.structured || chords.structured;
    Pat::new(PatNode::Chord(chords, subject), span, s)
}

/// `voicing p`: close position, lowest tone in octave 5.
#[must_use]
pub fn voicing(p: Rc<Pat>, span: Option<Span>) -> Pat {
    let s = p.structured;
    Pat::new(PatNode::Voicing(p), span, s)
}

/// `arp p mode` (`:up`, `:down`, `:updown`, `:downup`).
#[must_use]
pub fn arp(p: Rc<Pat>, mode: PParam, span: Option<Span>) -> Pat {
    op(PatNode::Arp(p, mode), span)
}

/// `range p lo hi` (subject first, M2).
#[must_use]
pub fn range(p: Rc<Pat>, lo: PParam, hi: PParam, span: Option<Span>) -> Pat {
    let s = p.structured;
    Pat::new(PatNode::Range(p, lo, hi), span, s)
}

/// Scales one degree.
///
/// # Errors
/// `Type` for an unknown root or scale.
pub fn scale_note(root: KwId, name: KwId, degree: i64) -> Result<i64, Failure> {
    let r = note_number(&name_of_kw(root)).ok_or_else(|| type_err("unknown scale root"))?;
    let steps = scale_steps(&name_of_kw(name))
        .ok_or_else(|| type_err(format!("unknown scale :{}", name_of_kw(name))))?;
    let len = i64::try_from(steps.len()).unwrap_or(1);
    let oct = degree.div_euclid(len);
    let idx = usize::try_from(degree.rem_euclid(len)).unwrap_or(0);
    oct.checked_mul(12)
        .and_then(|o| o.checked_add(r))
        .and_then(|x| x.checked_add(steps[idx]))
        .ok_or_else(|| Failure::new(FailCode::Overflow, "scale degree out of range"))
}

/// Applies a scale to one event: `n` sets `note`, a numeric value becomes
/// the note number.
pub(crate) fn scale_event(e: &mut Event, root: KwId, name: KwId) -> Result<(), Failure> {
    let nk = kw("n");
    if let Some(n) = e.controls.get(&nk) {
        let deg = int_of(n).ok_or_else(|| type_err("a scale degree must be an integer"))?;
        let note = scale_note(root, name, deg)?;
        e.controls.insert(kw("note"), note_value(note));
    } else if let Some(deg) = int_of(&e.value) {
        e.value = note_value(scale_note(root, name, deg)?);
    }
    Ok(())
}

pub(crate) fn query_scale(
    root: KwId,
    name: KwId,
    inner: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut e in query_child(inner, span, 0, st) {
        match scale_event(&mut e, root, name) {
            Ok(()) => out.push(e),
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

pub(crate) fn query_chord(
    chords: &Pat,
    subject: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    query_mapped(kw("note"), chords, subject, p, span, st, |v| {
        Ok(int_list(&chord_notes(v)?))
    })
}

/// Close position: the first tone in `[60, 72)`, each next tone the lowest
/// above the previous with its pitch class.
#[must_use]
pub fn voice(notes: &[i64]) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::with_capacity(notes.len());
    for n in notes {
        let pc = n.rem_euclid(12);
        let next = match out.last() {
            None => 60 + pc,
            Some(prev) => {
                let mut x = prev - prev.rem_euclid(12) + pc;
                if x <= *prev {
                    x += 12;
                }
                x
            }
        };
        out.push(next);
    }
    out
}

fn notes_of(e: &Event) -> Option<Vec<i64>> {
    match e.controls.get(&kw("note")) {
        Some(Value::List(l)) => l.items.iter().map(note_of).collect(),
        _ => None,
    }
}

pub(crate) fn query_voicing(
    inner: &Pat,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut e in query_child(inner, span, 0, st) {
        match notes_of(&e) {
            Some(notes) => {
                e.controls.insert(kw("note"), int_list(&voice(&notes)));
                out.push(e);
            }
            None if e
                .controls
                .get(&kw("note"))
                .is_some_and(|v| matches!(v, Value::List(_))) =>
            {
                event_fault(st, &e, p, type_err("a chord tone must be a note"));
            }
            None => out.push(e),
        }
    }
    out
}

/// The arpeggio order of a chord.
///
/// # Errors
/// `Type` for an unknown mode.
pub fn arp_order(notes: &[i64], mode: &str) -> Result<Vec<i64>, Failure> {
    let up = notes.to_vec();
    let down: Vec<i64> = notes.iter().rev().copied().collect();
    Ok(match mode {
        "up" => up,
        "down" => down,
        "updown" => up.iter().chain(down.iter().skip(1)).copied().collect(),
        "downup" => down.iter().chain(up.iter().skip(1)).copied().collect(),
        _ => return Err(type_err(format!("unknown arp mode :{mode}"))),
    })
}

pub(crate) fn query_arp(
    inner: &Pat,
    mode: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for e in query_child(inner, span, 0, st) {
        let Some(notes) = notes_of(&e) else {
            out.push(e);
            continue;
        };
        let r = (|| {
            let mode = match eval_param(mode, e.anchor(), st)? {
                Value::Keyword(k) => name_of_kw(k),
                _ => return Err(type_err("an arp mode is a keyword")),
            };
            let seq = arp_order(&notes, &mode)?;
            let k = count(i64::try_from(seq.len()).unwrap_or(0), "arp length")?;
            let mut kids = Vec::new();
            for (i, mut child) in split_event(&e, k)? {
                let note = seq[usize::try_from(i).unwrap_or(0)];
                child.controls.insert(kw("note"), note_value(note));
                child.occ.push(p.id, u32::try_from(i).unwrap_or(u32::MAX));
                if let Some(trace) = &mut child.producer {
                    trace.push(
                        ProducerKind::GeneratedBranch,
                        u32::try_from(i).unwrap_or(u32::MAX),
                    );
                }
                kids.push(child);
            }
            Ok(kids)
        })();
        match r {
            Ok(kids) => out.extend(kids),
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

/// `lo + v * (hi - lo)`: exact when all three are exact, else a float.
pub(crate) fn range_value(v: &Value, lo: &Value, hi: &Value) -> Result<Value, Failure> {
    let exact = |x: &Value| matches!(x, Value::Int(_) | Value::Int64(_) | Value::Ratio(_));
    if exact(v) && exact(lo) && exact(hi) {
        let (Some(v), Some(lo), Some(hi)) = (num_ratio(v), num_ratio(lo), num_ratio(hi)) else {
            return Err(type_err("range expects numbers"));
        };
        return Ok(exact_value(
            lo.checked_add(v.checked_mul(hi.checked_sub(lo)?)?)?,
        ));
    }
    let (Some(v), Some(lo), Some(hi)) = (num_f64(v), num_f64(lo), num_f64(hi)) else {
        return Err(type_err("range expects numbers"));
    };
    Ok(Value::Float64(lo + v * (hi - lo)))
}

pub(crate) fn query_range(
    inner: &Pat,
    lo: &PParam,
    hi: &PParam,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    let mut out = Vec::new();
    for mut e in query_child(inner, span, 0, st) {
        let at = e.anchor();
        let r = eval_param(lo, at, st)
            .and_then(|l| Ok((l, eval_param(hi, at, st)?)))
            .and_then(|(l, h)| range_value(&e.value, &l, &h));
        match r {
            Ok(v) => {
                e.value = v;
                out.push(e);
            }
            Err(f) => event_fault(st, &e, p, f),
        }
    }
    out
}

/// True for a chord literal `[root quality]`: a note, then a keyword that is
/// not a note name.
fn is_chord_literal(items: &[crate::pattern::step::Step]) -> bool {
    match items {
        [root, quality] => {
            note_of(&root.value).is_some()
                && matches!(&quality.value, Value::Keyword(k) if note_number(&name_of_kw(*k)).is_none())
        }
        _ => false,
    }
}

/// The chord-value pattern for a value in a `chord` position: a list
/// `[root quality]` is ONE chord (not two steps), and so is each such list
/// under `alt`, `choose` or `stack` (by the type at that position, 10.1).
///
/// # Errors
/// As [`crate::pattern::build::pattern_of`].
pub fn chord_pattern_of(v: &Value, span: Option<Span>) -> Result<Rc<Pat>, Failure> {
    let p = crate::pattern::build::pattern_of(v, span)?;
    Ok(rewrite_chords(&p, 0))
}

fn rewrite_chords(p: &Rc<Pat>, depth: u32) -> Rc<Pat> {
    if depth > 32 {
        return Rc::clone(p);
    }
    let map = |items: &[Pat]| -> Vec<Pat> {
        items
            .iter()
            .map(|x| (*rewrite_chords(&Rc::new(x.clone()), depth + 1)).clone())
            .collect()
    };
    match &p.node {
        PatNode::Steps(items) if is_chord_literal(items) => {
            let chord = crate::pattern::step::Step {
                value: Value::list(items.iter().map(|s| s.value.clone()).collect()),
                src: items.first().and_then(|i| i.src),
            };
            Rc::new(Pat::new(PatNode::Pure(chord), p.span, false))
        }
        PatNode::Cat(items) => Rc::new(crate::pattern::combinators::structure::cat(
            map(items),
            p.span,
        )),
        PatNode::Choose(items) => Rc::new(crate::pattern::combinators::random::choose(
            map(items),
            p.span,
        )),
        PatNode::Stack(items) => Rc::new(crate::pattern::combinators::structure::stack(
            map(items),
            p.span,
        )),
        _ => Rc::clone(p),
    }
}
