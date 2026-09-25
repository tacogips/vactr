//! `s` / `sound` and the sound kit (design 10.1 "Sound first", 7.1.4).
//!
//! One sound (`s :pluck`, `s kick`, `s {midi 1}`) is UNSTRUCTURED: queried
//! on its own it realizes one event per cycle `[c, c + 1)` (M4), and a
//! structure-giving step replaces that structure. `s [..]` is structured by
//! its steps. Keywords resolve per query against the `kit:` argument when
//! given, else `QueryVm::sound_kit()`, then in the instrument registry
//! (`QueryVm::inst_sound`, 12.8.6); a key found in neither is an
//! event-local `unknown-sound` failure.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::pattern::combinators::{event_fault, kw, subtree};
use crate::pattern::eval::{eval_param, int_of, QState};
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{q, sect, Event, TimeSpan};
use crate::reader::span::Span;
use crate::value::intern::name_of_kw;
use crate::value::key::Key;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// `s src [kit: kit]`. A `PParam::Pat` source is structured; any other
/// source is one sound and unstructured.
#[must_use]
pub fn sound(src: PParam, kit: Option<PParam>, span: Option<Span>) -> Pat {
    let structured = matches!(&src, PParam::Pat(p) if p.structured);
    Pat::new(PatNode::Sound { src, kit }, span, structured)
}

/// Resolves one sound value against a kit: a keyword is looked up, a
/// `sound`, an instrument or a bank (a list of sounds) is used as is.
///
/// # Errors
/// `UnknownSound` for a key missing from the kit; `Type` for a value that
/// is not a sound.
pub fn resolve_sound(v: &Value, kit: &BTreeMap<Key, Value>) -> Result<Value, Failure> {
    match v {
        Value::Keyword(k) => kit.get(&Key::Kw(*k)).cloned().ok_or_else(|| {
            Failure::new(
                FailCode::UnknownSound,
                format!("unknown sound :{} in the current sound kit", name_of_kw(*k)),
            )
        }),
        Value::Sound(_) | Value::Inst(_) => Ok(v.clone()),
        Value::List(l) if is_bank(&l.items) => Ok(v.clone()),
        _ => Err(Failure::new(FailCode::Type, "s expects a sound")),
    }
}

/// A sample bank: a non-empty list of sounds.
fn is_bank(items: &[Value]) -> bool {
    !items.is_empty() && items.iter().all(|v| matches!(v, Value::Sound(_)))
}

fn kit_of(
    kit: Option<&PParam>,
    at: crate::value::ratio::Ratio64,
    st: &mut QState<'_, '_>,
) -> Result<Rc<BTreeMap<Key, Value>>, Failure> {
    let v = match kit {
        Some(k) => eval_param(k, at, st)?,
        None => st.cx.vm.sound_kit()?,
    };
    match v {
        Value::Dict(d) => Ok(d),
        _ => Err(Failure::new(
            FailCode::Type,
            "a sound kit is a dict of keyword to sound",
        )),
    }
}

pub(crate) fn query_sound(
    src: &PParam,
    kit: Option<&PParam>,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    // The kit is read once per query of this node.
    let kit = match kit_of(kit, span.begin, st) {
        Ok(k) => k,
        Err(f) => {
            st.fault(f, p.span, Some(span.begin));
            return Vec::new();
        }
    };
    let mut raw = Vec::new();
    match src {
        PParam::Pat(sp) if p.structured => raw = q(sp, span, st),
        _ => {
            for piece in st.cycles(span, p.span) {
                let c = piece.begin.floor();
                raw.extend(subtree(p, piece.begin, st, |st| {
                    let whole = TimeSpan::cycle(c)?;
                    let Some(part) = sect(whole, piece) else {
                        return Ok(Vec::new());
                    };
                    let v = eval_param(src, whole.begin, st)?;
                    if matches!(v, Value::Nil) {
                        return Ok(Vec::new());
                    }
                    st.spend(1, p.span)?;
                    Ok(vec![Event::new(Some(whole), part, v, None)])
                }));
            }
        }
    }
    let mut out = Vec::with_capacity(raw.len());
    for mut e in raw {
        match resolve_sound(&e.value, &kit) {
            Ok(v) => {
                e.value = v;
                out.push(e);
            }
            // A key the kit lacks may name an instrument (12.8.6).
            Err(f) => match (&e.value, f.code) {
                (Value::Keyword(k), FailCode::UnknownSound) => match st.cx.vm.inst_sound(*k) {
                    Some(v) => {
                        e.value = v;
                        out.push(e);
                    }
                    None => event_fault(st, &e, p, f),
                },
                _ => event_fault(st, &e, p, f),
            },
        }
    }
    out
}

/// Picks the `n`-th sound of a bank (default 0) at the end of a query.
///
/// # Errors
/// `SliceIndex` for an index outside the bank; `Type` for a non-integer
/// index.
pub(crate) fn pick_bank(e: &mut Event) -> Result<(), Failure> {
    let Value::List(l) = &e.value else {
        return Ok(());
    };
    if !is_bank(&l.items) {
        return Ok(());
    }
    let i = match e.controls.get(&kw("n")) {
        None => 0,
        Some(v) => int_of(v)
            .ok_or_else(|| Failure::new(FailCode::Type, "a bank index must be an integer"))?,
    };
    let picked = usize::try_from(i)
        .ok()
        .and_then(|i| l.items.get(i))
        .cloned()
        .ok_or_else(|| {
            Failure::new(
                FailCode::SliceIndex,
                format!("bank index {i} outside 0..{}", l.items.len()),
            )
        })?;
    e.value = picked;
    Ok(())
}
