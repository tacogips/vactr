//! Parsers for the public Scala `.scl` and `.kbm` text formats.

use std::collections::BTreeMap;

use crate::value::intern::intern_kw;
use crate::value::key::Key;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// Parses Scala scale text and an optional keyboard map into a tuning spec.
pub fn scala_spec(scl: &str, kbm: Option<&str>) -> Result<Value, Failure> {
    let lines = data_lines(scl);
    if lines.len() < 2 {
        return Err(type_err("Scala scale is missing its description or count"));
    }
    let count = first_token(lines[1])
        .parse::<usize>()
        .map_err(|_| type_err("Scala count must be an integer"))?;
    if count == 0 || count > 1024 || lines.len() < count + 2 {
        return Err(type_err(
            "Scala count must be 1..=1024 and have enough pitches",
        ));
    }
    let mut degrees = Vec::with_capacity(count);
    for line in &lines[2..count + 2] {
        let token = first_token(line);
        if token.is_empty() {
            return Err(type_err("Scala pitch is empty"));
        }
        let pitch = if token.contains('.') {
            let cents = token
                .parse::<f64>()
                .map_err(|_| type_err("invalid Scala cents"))?;
            if !cents.is_finite() {
                return Err(type_err("Scala cents must be finite"));
            }
            Value::Float64(cents)
        } else {
            let ratio = if let Some((n, d)) = token.split_once('/') {
                let n = n
                    .parse::<i64>()
                    .map_err(|_| type_err("invalid Scala ratio"))?;
                let d = d
                    .parse::<i64>()
                    .map_err(|_| type_err("invalid Scala ratio"))?;
                Ratio64::new(n, d).map_err(|_| type_err("invalid Scala ratio"))?
            } else {
                Ratio64::from_int(
                    token
                        .parse::<i64>()
                        .map_err(|_| type_err("invalid Scala ratio"))?,
                )
            };
            if ratio.num() <= 0 {
                return Err(type_err("Scala ratios must be positive"));
            }
            exact_value(ratio)
        };
        degrees.push(pitch);
    }
    let valid_period = match degrees.last() {
        Some(Value::Float64(cents)) => *cents > 0.0,
        Some(Value::Int(value)) => *value > 1,
        Some(Value::Int64(value)) => *value > 1,
        Some(Value::Ratio(ratio)) => ratio.num() > ratio.den(),
        _ => false,
    };
    if !valid_period {
        return Err(type_err("Scala period must be greater than zero cents"));
    }
    let mut fields = BTreeMap::new();
    fields.insert(Key::Kw(intern_kw("kind")), Value::kw("degrees"));
    fields.insert(Key::Kw(intern_kw("degrees")), Value::list(degrees));
    let description = lines[0].trim();
    if !description.is_empty() {
        fields.insert(Key::Kw(intern_kw("description")), Value::str(description));
    }
    if let Some(kbm) = kbm {
        fields.insert(Key::Kw(intern_kw("keymap")), parse_kbm(kbm, count)?);
    }
    Ok(Value::dict(fields))
}

fn parse_kbm(text: &str, count: usize) -> Result<Value, Failure> {
    let lines = data_lines(text);
    if lines.len() < 7 {
        return Err(type_err("Scala keyboard map needs seven header fields"));
    }
    let size = parse_i64(lines[0], "map size")?;
    let first = parse_i64(lines[1], "first key")?;
    let last = parse_i64(lines[2], "last key")?;
    let middle = parse_i64(lines[3], "middle key")?;
    let ref_key = parse_i64(lines[4], "reference key")?;
    let ref_freq = first_token(lines[5])
        .parse::<f64>()
        .map_err(|_| type_err("invalid reference frequency"))?;
    let octave_degree = parse_i64(lines[6], "formal octave degree")?;
    if !(0..=1024).contains(&size)
        || first > last
        || !(0..=count as i64).contains(&octave_degree)
        || !ref_freq.is_finite()
        || ref_freq <= 0.0
    {
        return Err(type_err("invalid Scala keyboard map header"));
    }
    let mut entries = Vec::with_capacity(size as usize);
    for line in lines.iter().skip(7).take(size as usize) {
        let token = first_token(line);
        if token.eq_ignore_ascii_case("x") {
            entries.push(Value::Nil);
        } else {
            entries.push(Value::Int64(parse_i64(token, "keyboard map entry")?));
        }
    }
    while entries.len() < size as usize {
        entries.push(Value::Nil);
    }
    let mut fields = BTreeMap::new();
    fields.insert(Key::Kw(intern_kw("first")), int_value(first));
    fields.insert(Key::Kw(intern_kw("last")), int_value(last));
    fields.insert(Key::Kw(intern_kw("middle")), int_value(middle));
    fields.insert(Key::Kw(intern_kw("ref-key")), int_value(ref_key));
    fields.insert(Key::Kw(intern_kw("ref-freq")), Value::Float64(ref_freq));
    fields.insert(
        Key::Kw(intern_kw("octave-degree")),
        int_value(octave_degree),
    );
    if size > 0 {
        fields.insert(Key::Kw(intern_kw("map")), Value::list(entries));
    }
    Ok(Value::dict(fields))
}

fn data_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim_end)
        .map(str::trim_start)
        .filter(|line| !line.starts_with('!'))
        .collect()
}

fn first_token(line: &str) -> &str {
    line.split_whitespace().next().unwrap_or("")
}
fn parse_i64(text: &str, field: &str) -> Result<i64, Failure> {
    first_token(text)
        .parse()
        .map_err(|_| type_err(format!("invalid Scala {field}")))
}
fn int_value(value: i64) -> Value {
    i32::try_from(value).map_or(Value::Int64(value), Value::Int)
}
fn exact_value(value: Ratio64) -> Value {
    if value.den() == 1 {
        int_value(value.num())
    } else {
        Value::Ratio(value)
    }
}
fn type_err(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, message)
}
