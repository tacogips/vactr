//! Tuning-control recognition and tuned note-key decoding for commit.

use crate::pattern::eval::num_f64;
use crate::pattern::tuning::Tuning;
use crate::value::intern::name_of_kw;
use crate::value::value::Value;
use crate::vm::fail::Failure;

use super::{current, entry, Controls};

pub(super) fn tuning(controls: &Controls) -> Result<Option<Tuning>, Failure> {
    entry(controls, "tuning")
        .map(current)
        .and_then(|value| Tuning::from_control(&value))
        .transpose()
}

pub(super) fn notes(
    controls: &Controls,
    bank: bool,
    tuning: &Tuning,
) -> Result<Option<Vec<f64>>, Failure> {
    let Some(entry) =
        entry(controls, "note").or_else(|| (!bank).then(|| entry(controls, "n")).flatten())
    else {
        return Ok(None);
    };
    let value = current(entry);
    let notes = match value {
        Value::List(list) => list
            .items
            .iter()
            .map(|value| note_of(value, tuning))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect(),
        value => {
            let Some(note) = note_of(&value, tuning)? else {
                return Ok(None);
            };
            vec![note]
        }
    };
    Ok(Some(notes))
}

fn note_of(value: &Value, tuning: &Tuning) -> Result<Option<f64>, Failure> {
    match value {
        Value::Keyword(name) => tuning
            .note_key(&name_of_kw(*name))
            .map(|key| key.map(|key| key as f64)),
        other => Ok(num_f64(other)),
    }
}
