//! Pure tuning specifications, frequency mapping and event-control codec.

mod presets;
mod scala;

pub use presets::{scale_preset, tuning_preset, ScalePreset};
pub use scala::scala_spec;

use std::collections::BTreeMap;

use crate::pattern::combinators::music::note_number;
use crate::sched::commit::note_to_freq;
use crate::value::intern::{intern_kw, name_of_kw};
use crate::value::key::Key;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

#[cfg(test)]
mod tests;

/// Control name used to carry a canonical tuning specification on an event.
pub const TUNING_CONTROL: &str = "tuning";

/// Optional key and frequency anchors. Missing values use the tuning defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mapping {
    pub root: Option<i64>,
    pub ref_key: Option<i64>,
    pub ref_freq: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pitch {
    Ratio(Ratio64),
    Cents(f64),
}

#[derive(Clone, Debug, PartialEq)]
struct Keymap {
    first: i64,
    last: i64,
    middle: i64,
    ref_key: i64,
    ref_freq: f64,
    octave_degree: i64,
    entries: Vec<Option<i64>>,
}

#[derive(Clone, Debug, PartialEq)]
enum Temperament {
    Edo {
        steps: i64,
        period: Ratio64,
    },
    Degrees {
        degrees: Vec<Pitch>,
        keymap: Option<Keymap>,
    },
}

/// A validated tuning model. Values are resolved away from the audio thread.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    temperament: Temperament,
    root: i64,
    ref_key: i64,
    ref_freq: f64,
}

impl Tuning {
    /// Parses a tuning dictionary or one of the named tuning presets.
    pub fn from_spec(spec: &Value) -> Result<Tuning, Failure> {
        if let Value::Keyword(keyword) = spec {
            let name = name_of_kw(*keyword);
            let value = tuning_preset(&name).ok_or_else(|| type_err("unknown tuning preset"))?;
            return Self::from_spec(&value);
        }
        let fields = dict(spec)?;
        let kind = kw_field(fields, "kind")?;
        let kind = keyword_name(kind)?;
        let mut temperament = match kind.as_str() {
            "edo" => {
                let steps = int_field(fields, "steps")?;
                let period = exact_ratio(field(fields, "period")?)?;
                validate_edo(steps, period)?;
                Temperament::Edo { steps, period }
            }
            "degrees" => {
                let Value::List(values) = field(fields, "degrees")? else {
                    return Err(type_err("degrees must be a list"));
                };
                if values.items.is_empty() || values.items.len() > 1024 {
                    return Err(type_err("degrees must contain 1..=1024 entries"));
                }
                let degrees = values
                    .items
                    .iter()
                    .map(parse_pitch)
                    .collect::<Result<Vec<_>, _>>()?;
                validate_degrees(&degrees)?;
                let keymap = fields
                    .get(&Key::Kw(intern_kw("keymap")))
                    .map(parse_keymap)
                    .transpose()?;
                Temperament::Degrees { degrees, keymap }
            }
            _ => return Err(type_err("kind must be :edo or :degrees")),
        };
        let (default_root, default_ref_key, default_ref_freq) = match &temperament {
            Temperament::Edo { .. } => (60, 60, note_to_freq(60.0)),
            Temperament::Degrees {
                keymap: Some(km), ..
            } => {
                let reference = km.middle;
                (reference, km.ref_key, km.ref_freq)
            }
            Temperament::Degrees { keymap: None, .. } => (60, 60, note_to_freq(60.0)),
        };
        let mapping = Mapping {
            root: optional_int(fields, "root")?,
            ref_key: optional_int(fields, "ref-key")?,
            ref_freq: optional_number(fields, "ref-freq")?,
        };
        let root = mapping.root.unwrap_or(default_root);
        let follows_root = matches!(&temperament, Temperament::Degrees { keymap: None, .. })
            || matches!(&temperament, Temperament::Edo { .. });
        let ref_key = mapping
            .ref_key
            .unwrap_or(if follows_root { root } else { default_ref_key });
        let ref_freq = mapping.ref_freq.unwrap_or(if follows_root {
            if mapping.ref_key.is_some() {
                note_to_freq(ref_key as f64)
            } else {
                note_to_freq(root as f64)
            }
        } else {
            default_ref_freq
        });
        if let (
            true,
            Temperament::Degrees {
                keymap: Some(keymap),
                ..
            },
        ) = (mapping.root.is_some(), &mut temperament)
        {
            keymap.middle = root;
        }
        validate_anchor(ref_freq)?;
        let tuning = Tuning {
            temperament,
            root,
            ref_key,
            ref_freq,
        };
        if tuning.degree_for_key(ref_key)?.is_none() {
            return Err(type_err("reference key is unmapped"));
        }
        Ok(tuning)
    }

    /// Creates an equal division with its default key and frequency anchor.
    pub fn edo(steps: i64, period: Ratio64) -> Result<Tuning, Failure> {
        validate_edo(steps, period)?;
        let spec = edo_spec(&int_value(steps), Some(&exact_value(period)))?;
        Self::from_spec(&spec)
    }

    /// Overrides each supplied mapping field while retaining other anchors.
    pub fn with_mapping(mut self, mapping: &Mapping) -> Result<Tuning, Failure> {
        let old_root = self.root;
        let old_ref_key = self.ref_key;
        let old_ref_freq = self.ref_freq;
        let has_keymap = matches!(
            &self.temperament,
            Temperament::Degrees {
                keymap: Some(_),
                ..
            }
        );
        let default_anchor = !has_keymap
            && old_ref_key == old_root
            && old_ref_freq == note_to_freq(old_ref_key as f64);

        if let Some(root) = mapping.root {
            self.root = root;
            if let Temperament::Degrees {
                keymap: Some(keymap),
                ..
            } = &mut self.temperament
            {
                keymap.middle = root;
            }
        }
        if let Some(ref_key) = mapping.ref_key {
            self.ref_key = ref_key;
        } else if default_anchor {
            self.ref_key = self.root;
        }
        if let Some(ref_freq) = mapping.ref_freq {
            validate_anchor(ref_freq)?;
            self.ref_freq = ref_freq;
        } else if !has_keymap
            && self.ref_key != old_ref_key
            && old_ref_freq == note_to_freq(old_ref_key as f64)
        {
            self.ref_freq = note_to_freq(self.ref_key as f64);
        }
        if self.degree_for_key(self.ref_key)?.is_none() {
            return Err(type_err("reference key is unmapped"));
        }
        Ok(self)
    }

    /// Encodes the canonical event control form.
    #[must_use]
    pub fn to_control(&self) -> Value {
        let mapping = vec![
            int_value(self.root),
            int_value(self.ref_key),
            Value::Float64(self.ref_freq),
        ];
        match &self.temperament {
            Temperament::Edo { steps, period } => Value::list(vec![
                Value::kw("edo"),
                int_value(*steps),
                exact_value(*period),
                mapping[0].clone(),
                mapping[1].clone(),
                mapping[2].clone(),
            ]),
            Temperament::Degrees { degrees, keymap } => {
                let mut items = vec![
                    Value::kw("degrees"),
                    Value::list(degrees.iter().map(pitch_value).collect()),
                ];
                items.extend(mapping);
                items.push(keymap.as_ref().map_or(Value::Nil, keymap_value));
                Value::list(items)
            }
        }
    }

    /// Decodes only lists beginning with `:edo` or `:degrees`.
    pub fn from_control(value: &Value) -> Option<Result<Tuning, Failure>> {
        let Value::List(list) = value else {
            return None;
        };
        let Some(Value::Keyword(kind)) = list.items.first() else {
            return None;
        };
        if !matches!(name_of_kw(*kind).as_ref(), "edo" | "degrees") {
            return None;
        }
        Some(parse_control(&list.items))
    }

    /// Number of keys in one tuning period.
    #[must_use]
    pub fn keys_per_period(&self) -> i64 {
        match &self.temperament {
            Temperament::Edo { steps, .. } => *steps,
            Temperament::Degrees {
                degrees,
                keymap: None,
            } => degrees.len() as i64,
            Temperament::Degrees {
                degrees,
                keymap: Some(km),
            } => {
                if km.entries.is_empty() {
                    degrees.len() as i64
                } else {
                    km.entries.len() as i64
                }
            }
        }
    }

    /// Root key, which is degree zero.
    #[must_use]
    pub fn root(&self) -> i64 {
        self.root
    }

    /// Cents in the formal period.
    #[must_use]
    pub fn period_cents(&self) -> f64 {
        match &self.temperament {
            Temperament::Edo { period, .. } => 1200.0 * period.to_f64().log2(),
            Temperament::Degrees { degrees, keymap } => {
                let index = keymap.as_ref().map_or(degrees.len() as i64, |km| {
                    if km.octave_degree == 0 {
                        degrees.len() as i64
                    } else {
                        km.octave_degree
                    }
                });
                log_degree(degrees, index) * 1200.0
            }
        }
    }

    /// Resolves a possibly fractional key. Unmapped keys return `Ok(None)`.
    pub fn freq(&self, key: f64) -> Result<Option<f64>, Failure> {
        if !key.is_finite() {
            return Err(type_err("key must be finite"));
        }
        if let Temperament::Edo { steps, period } = &self.temperament {
            let delta = (key - self.ref_key as f64) / *steps as f64;
            let result = if *period == Ratio64::from_int(2) {
                self.ref_freq * delta.exp2()
            } else {
                self.ref_freq * (delta * period.to_f64().log2()).exp2()
            };
            return checked_frequency(result).map(Some);
        }
        let lo = key.floor();
        if lo < i64::MIN as f64 || lo > i64::MAX as f64 - 1.0 {
            return Err(type_err("key is out of range"));
        }
        let lo = lo as i64;
        let t = key - lo as f64;
        let Some(a) = self.integer_freq(lo)? else {
            return Ok(None);
        };
        if t == 0.0 {
            return Ok(Some(a));
        }
        let Some(b) = self.integer_freq(lo + 1)? else {
            return Ok(None);
        };
        let result = (a.ln() + t * (b.ln() - a.ln())).exp();
        checked_frequency(result).map(Some)
    }

    /// Finds the integer key offset whose interval best matches `cents`.
    pub fn nearest(&self, from: i64, cents: f64) -> Result<i64, Failure> {
        if !cents.is_finite() {
            return Err(type_err("cents must be finite"));
        }
        let base = self
            .integer_freq(from)?
            .ok_or_else(|| type_err("source key is unmapped"))?
            .log2()
            * 1200.0;
        let k = self.keys_per_period().max(1);
        let estimate = (cents * k as f64 / self.period_cents()).round();
        if !estimate.is_finite()
            || estimate < i64::MIN as f64 + k as f64
            || estimate > i64::MAX as f64 - k as f64
        {
            return Err(type_err("nearest key is out of range"));
        }
        let center = estimate as i64;
        let mut best: Option<(f64, i64)> = None;
        for offset in center - k..=center + k {
            let Some(key) = from.checked_add(offset) else {
                continue;
            };
            let Some(freq) = self.integer_freq(key)? else {
                continue;
            };
            let error = ((freq.log2() * 1200.0) - base - cents).abs();
            let replace = best.is_none_or(|(old_error, old_offset)| {
                error + 1e-10 < old_error
                    || ((error - old_error).abs() <= 1e-10
                        && (offset.unsigned_abs(), offset)
                            < (old_offset.unsigned_abs(), old_offset))
            });
            if replace {
                best = Some((error, offset));
            }
        }
        best.map(|(_, offset)| offset)
            .ok_or_else(|| type_err("no mapped key near requested interval"))
    }

    /// Converts a note name to a key using the tuning's root and nearest rule.
    pub fn note_key(&self, name: &str) -> Result<Option<i64>, Failure> {
        let Some(note) = note_number(name) else {
            return Ok(None);
        };
        if self.keys_per_period() == 12 {
            return Ok(Some(note));
        }
        let cents = 100.0 * (note - self.root) as f64;
        self.nearest(self.root, cents)?
            .checked_add(self.root)
            .map(Some)
            .ok_or_else(|| type_err("note key is out of range"))
    }

    fn integer_freq(&self, key: i64) -> Result<Option<f64>, Failure> {
        let Some(degree) = self.degree_for_key(key)? else {
            return Ok(None);
        };
        let ref_degree = self
            .degree_for_key(self.ref_key)?
            .ok_or_else(|| type_err("reference key is unmapped"))?;
        let ratio = match &self.temperament {
            Temperament::Edo { steps, period } => {
                let delta = (key as f64 - self.ref_key as f64) / *steps as f64;
                if *period == Ratio64::from_int(2) {
                    return checked_frequency(self.ref_freq * delta.exp2()).map(Some);
                }
                (delta * period.to_f64().log2()).exp2()
            }
            Temperament::Degrees { .. } => (degree.log2 - ref_degree.log2).exp2(),
        };
        checked_frequency(self.ref_freq * ratio).map(Some)
    }

    fn degree_for_key(&self, key: i64) -> Result<Option<Degree>, Failure> {
        match &self.temperament {
            Temperament::Edo { steps, period } => Ok(Some(Degree {
                log2: (key as f64 - self.root as f64) * period.to_f64().log2() / *steps as f64,
            })),
            Temperament::Degrees { degrees, keymap } => {
                let raw = if let Some(km) = keymap {
                    if key < km.first || key > km.last {
                        return Ok(None);
                    }
                    if km.entries.is_empty() {
                        key.checked_sub(km.middle)
                    } else {
                        let Some(offset) = key.checked_sub(km.middle) else {
                            return Err(type_err("keymap offset overflow"));
                        };
                        let m = km.entries.len() as i64;
                        let Some(mapped) = km.entries[offset.rem_euclid(m) as usize] else {
                            return Ok(None);
                        };
                        Some(
                            (offset.div_euclid(m))
                                .checked_mul(if km.octave_degree == 0 {
                                    degrees.len() as i64
                                } else {
                                    km.octave_degree
                                })
                                .and_then(|n| n.checked_add(mapped))
                                .ok_or_else(|| type_err("keymap degree overflow"))?,
                        )
                    }
                } else {
                    key.checked_sub(self.root)
                };
                let raw = raw.ok_or_else(|| type_err("degree out of range"))?;
                let n = degrees.len() as i64;
                let octave = raw.div_euclid(n);
                let index = raw.rem_euclid(n) as usize;
                let period = pitch_log(*degrees.last().ok_or_else(|| type_err("empty degrees"))?);
                let interval = if index == 0 {
                    0.0
                } else {
                    pitch_log(degrees[index - 1])
                };
                Ok(Some(Degree {
                    log2: octave as f64 * period + interval,
                }))
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Degree {
    log2: f64,
}

/// Builds a validated EDO spec dictionary.
pub fn edo_spec(steps: &Value, period: Option<&Value>) -> Result<Value, Failure> {
    let steps = int_value_of(steps).ok_or_else(|| type_err("steps must be an integer"))?;
    let period = period.cloned().unwrap_or_else(|| int_value(2));
    let period = exact_ratio(&period)?;
    validate_edo(steps, period)?;
    let mut fields = BTreeMap::new();
    fields.insert(Key::Kw(intern_kw("kind")), Value::kw("edo"));
    fields.insert(Key::Kw(intern_kw("steps")), int_value(steps));
    fields.insert(Key::Kw(intern_kw("period")), exact_value(period));
    Ok(Value::dict(fields))
}

/// Builds a validated ratio-list spec dictionary.
pub fn ratios_spec(list: &Value) -> Result<Value, Failure> {
    let Value::List(values) = list else {
        return Err(type_err("ratios must be a list"));
    };
    if values.items.is_empty() || values.items.len() > 1024 {
        return Err(type_err("ratios must contain 1..=1024 entries"));
    }
    for value in values.items.iter() {
        if exact_ratio(value)?.num() <= 0 {
            return Err(type_err("ratios must be positive"));
        }
    }
    let final_ratio = exact_ratio(
        values
            .items
            .last()
            .ok_or_else(|| type_err("ratios must not be empty"))?,
    )?;
    if final_ratio.num() <= final_ratio.den() {
        return Err(type_err("the final ratio must define a positive period"));
    }
    let mut fields = BTreeMap::new();
    fields.insert(Key::Kw(intern_kw("kind")), Value::kw("degrees"));
    fields.insert(Key::Kw(intern_kw("degrees")), list.clone());
    Ok(Value::dict(fields))
}

fn parse_control(items: &[Value]) -> Result<Tuning, Failure> {
    let kind = keyword_name(&items[0])?;
    let spec = match kind.as_str() {
        "edo" if items.len() == 6 => {
            let mut fields = BTreeMap::new();
            fields.insert(Key::Kw(intern_kw("kind")), Value::kw("edo"));
            fields.insert(Key::Kw(intern_kw("steps")), items[1].clone());
            fields.insert(Key::Kw(intern_kw("period")), items[2].clone());
            fields.insert(Key::Kw(intern_kw("root")), items[3].clone());
            fields.insert(Key::Kw(intern_kw("ref-key")), items[4].clone());
            fields.insert(Key::Kw(intern_kw("ref-freq")), items[5].clone());
            Value::dict(fields)
        }
        "degrees" if items.len() == 6 => {
            let mut fields = BTreeMap::new();
            fields.insert(Key::Kw(intern_kw("kind")), Value::kw("degrees"));
            fields.insert(Key::Kw(intern_kw("degrees")), items[1].clone());
            fields.insert(Key::Kw(intern_kw("root")), items[2].clone());
            fields.insert(Key::Kw(intern_kw("ref-key")), items[3].clone());
            fields.insert(Key::Kw(intern_kw("ref-freq")), items[4].clone());
            if !matches!(items[5], Value::Nil) {
                fields.insert(
                    Key::Kw(intern_kw("keymap")),
                    control_keymap(&items[5], &items[2], &items[3], &items[4])?,
                );
            }
            Value::dict(fields)
        }
        _ => return Err(type_err("malformed tuning control")),
    };
    Tuning::from_spec(&spec)
}

fn control_keymap(
    value: &Value,
    root: &Value,
    ref_key: &Value,
    ref_freq: &Value,
) -> Result<Value, Failure> {
    let Value::List(list) = value else {
        return Err(type_err("keymap control must be a list"));
    };
    let [first, last, octave, entries] = &*list.items else {
        return Err(type_err("malformed keymap control"));
    };
    let Value::List(entries) = entries else {
        return Err(type_err("keymap entries must be a list"));
    };
    let mut fields = BTreeMap::new();
    fields.insert(Key::Kw(intern_kw("first")), first.clone());
    fields.insert(Key::Kw(intern_kw("last")), last.clone());
    fields.insert(Key::Kw(intern_kw("middle")), root.clone());
    fields.insert(Key::Kw(intern_kw("ref-key")), ref_key.clone());
    fields.insert(Key::Kw(intern_kw("ref-freq")), ref_freq.clone());
    fields.insert(Key::Kw(intern_kw("octave-degree")), octave.clone());
    fields.insert(
        Key::Kw(intern_kw("map")),
        Value::list(entries.items.to_vec()),
    );
    Ok(Value::dict(fields))
}

fn parse_keymap(value: &Value) -> Result<Keymap, Failure> {
    let fields = dict(value)?;
    let first = int_field(fields, "first")?;
    let last = int_field(fields, "last")?;
    let middle = int_field(fields, "middle")?;
    let ref_key = optional_int(fields, "ref-key")?.unwrap_or(middle);
    let ref_freq =
        optional_number(fields, "ref-freq")?.unwrap_or_else(|| note_to_freq(ref_key as f64));
    validate_anchor(ref_freq)?;
    let octave_degree = int_field(fields, "octave-degree")?;
    if first > last || !(0..=1024).contains(&octave_degree) {
        return Err(type_err("invalid keymap bounds"));
    }
    let entries = match fields.get(&Key::Kw(intern_kw("map"))) {
        None => Vec::new(),
        Some(Value::List(list)) if list.items.len() <= 1024 => list
            .items
            .iter()
            .map(|v| {
                if matches!(v, Value::Nil) {
                    Ok(None)
                } else {
                    int_value_of(v)
                        .map(Some)
                        .ok_or_else(|| type_err("keymap entries must be integers or nil"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(type_err(
                "keymap map must be a list of at most 1024 entries",
            ))
        }
    };
    Ok(Keymap {
        first,
        last,
        middle,
        ref_key,
        ref_freq,
        octave_degree,
        entries,
    })
}

fn keymap_value(keymap: &Keymap) -> Value {
    Value::list(vec![
        int_value(keymap.first),
        int_value(keymap.last),
        int_value(keymap.octave_degree),
        Value::list(
            keymap
                .entries
                .iter()
                .map(|entry| entry.map_or(Value::Nil, int_value))
                .collect(),
        ),
    ])
}

fn pitch_value(pitch: &Pitch) -> Value {
    match pitch {
        Pitch::Ratio(r) => exact_value(*r),
        Pitch::Cents(c) => Value::Float64(*c),
    }
}
fn pitch_log(pitch: Pitch) -> f64 {
    match pitch {
        Pitch::Ratio(r) => r.to_f64().log2(),
        Pitch::Cents(c) => c / 1200.0,
    }
}
fn log_degree(degrees: &[Pitch], degree: i64) -> f64 {
    let count = degrees.len() as i64;
    let index = degree.rem_euclid(count) as usize;
    let interval = if index == 0 {
        0.0
    } else {
        pitch_log(degrees[index - 1])
    };
    degree.div_euclid(count) as f64 * pitch_log(degrees[degrees.len() - 1]) + interval
}
fn parse_pitch(value: &Value) -> Result<Pitch, Failure> {
    match value {
        Value::Float(v) => parse_cents(f64::from(*v)),
        Value::Float64(v) => parse_cents(*v),
        _ => {
            let ratio = exact_ratio(value)?;
            if ratio.num() <= 0 {
                return Err(type_err("ratios must be positive"));
            }
            Ok(Pitch::Ratio(ratio))
        }
    }
}
fn parse_cents(cents: f64) -> Result<Pitch, Failure> {
    if cents.is_finite() {
        Ok(Pitch::Cents(cents))
    } else {
        Err(type_err("cents must be finite"))
    }
}
fn validate_degrees(degrees: &[Pitch]) -> Result<(), Failure> {
    if degrees.is_empty() || degrees.len() > 1024 {
        return Err(type_err("degrees must contain 1..=1024 entries"));
    }
    if pitch_log(*degrees.last().ok_or_else(|| type_err("empty degrees"))?) <= 0.0 {
        return Err(type_err("period must be greater than zero cents"));
    }
    Ok(())
}
fn validate_edo(steps: i64, period: Ratio64) -> Result<(), Failure> {
    if !(1..=1200).contains(&steps) || period.num() <= period.den() {
        Err(type_err(
            "steps must be 1..=1200 and period must be exact and greater than 1",
        ))
    } else {
        Ok(())
    }
}
fn validate_anchor(freq: f64) -> Result<(), Failure> {
    if freq.is_finite() && freq > 0.0 {
        Ok(())
    } else {
        Err(type_err("reference frequency must be finite and positive"))
    }
}
fn checked_frequency(freq: f64) -> Result<f64, Failure> {
    if freq.is_finite() && freq > 0.0 {
        Ok(freq)
    } else {
        Err(type_err("resolved frequency must be finite and positive"))
    }
}
fn exact_ratio(value: &Value) -> Result<Ratio64, Failure> {
    match value {
        Value::Int(i) => Ok(Ratio64::from_int(i64::from(*i))),
        Value::Int64(i) => Ok(Ratio64::from_int(*i)),
        Value::Ratio(r) => Ok(*r),
        _ => Err(type_err("value must be an exact integer or ratio")),
    }
}
fn exact_value(ratio: Ratio64) -> Value {
    if ratio.den() == 1 {
        int_value(ratio.num())
    } else {
        Value::Ratio(ratio)
    }
}
fn int_value(value: i64) -> Value {
    i32::try_from(value).map_or(Value::Int64(value), Value::Int)
}
fn int_value_of(value: &Value) -> Option<i64> {
    match value {
        Value::Int(i) => Some(i64::from(*i)),
        Value::Int64(i) => Some(*i),
        Value::Ratio(r) if r.is_integral() => Some(r.num()),
        _ => None,
    }
}
fn dict(value: &Value) -> Result<&BTreeMap<Key, Value>, Failure> {
    if let Value::Dict(fields) = value {
        Ok(fields)
    } else {
        Err(type_err("tuning spec must be a dict"))
    }
}
fn field<'a>(fields: &'a BTreeMap<Key, Value>, name: &str) -> Result<&'a Value, Failure> {
    fields
        .get(&Key::Kw(intern_kw(name)))
        .ok_or_else(|| type_err(format!("missing {name}")))
}
fn kw_field<'a>(fields: &'a BTreeMap<Key, Value>, name: &str) -> Result<&'a Value, Failure> {
    field(fields, name)
}
fn int_field(fields: &BTreeMap<Key, Value>, name: &str) -> Result<i64, Failure> {
    int_value_of(field(fields, name)?).ok_or_else(|| type_err(format!("{name} must be an integer")))
}
fn optional_int(fields: &BTreeMap<Key, Value>, name: &str) -> Result<Option<i64>, Failure> {
    fields
        .get(&Key::Kw(intern_kw(name)))
        .map(|v| int_value_of(v).ok_or_else(|| type_err(format!("{name} must be an integer"))))
        .transpose()
}
fn optional_number(fields: &BTreeMap<Key, Value>, name: &str) -> Result<Option<f64>, Failure> {
    fields
        .get(&Key::Kw(intern_kw(name)))
        .map(|v| match v {
            Value::Int(i) => Ok(f64::from(*i)),
            Value::Int64(i) => Ok(*i as f64),
            Value::Ratio(r) => Ok(r.to_f64()),
            Value::Float(f) => Ok(f64::from(*f)),
            Value::Float64(f) => Ok(*f),
            _ => Err(type_err("ref-freq must be numeric")),
        })
        .transpose()
}
fn keyword_name(value: &Value) -> Result<String, Failure> {
    if let Value::Keyword(k) = value {
        Ok(name_of_kw(*k).to_string())
    } else {
        Err(type_err("kind must be a keyword"))
    }
}
fn type_err(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, message)
}
