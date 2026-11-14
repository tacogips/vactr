//! `s` / `sound` and the sound kit (design 10.1 "Sound first", 7.1.4).
//!
//! One sound (`s :pluck`, `s kick`, `s {midi 1}`) is UNSTRUCTURED: queried
//! on its own it realizes one event per cycle `[c, c + 1)` (M4), and a
//! structure-giving step replaces that structure. `s [..]` is structured by
//! its steps. Keywords resolve per query against the `kit:` argument when
//! given, else `QueryVm::sound_kit()`, then in the instrument registry
//! (`QueryVm::inst_sound`, 12.8.6); a key found in neither is an
//! event-local `unknown-sound` failure.

use crate::pattern::combinators::control::query_child;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::pattern::combinators::control::sample_child;
use crate::pattern::combinators::{event_fault, kw, subtree};
use crate::pattern::eval::{eval_param, int_of, resolve_dynamic, QState};
use crate::pattern::occ::{ProducerKind, ProducerTrace};
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::query::{sect, Event, TimeSpan};
use crate::reader::span::Span;
use crate::value::intern::name_of_kw;
use crate::value::key::Key;
use crate::value::ratio::Ratio64;
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
        PParam::Pat(sp) if p.structured => raw = query_child(sp, span, 0, st),
        _ => {
            for piece in st.cycles(span, p.span) {
                let c = piece.begin.floor();
                raw.extend(subtree(p, piece.begin, st, |st| {
                    let whole = TimeSpan::cycle(c)?;
                    let Some(part) = sect(whole, piece) else {
                        return Ok(Vec::new());
                    };
                    let sampled = if st.is_traced() {
                        traced_source(src, whole.begin, whole, part, st)?
                    } else {
                        SourceSample::fresh(eval_param(src, whole.begin, st)?, None, whole.begin)
                    };
                    if matches!(sampled.event.value, Value::Nil) {
                        return Ok(Vec::new());
                    }
                    st.spend(1, p.span)?;
                    let e = sampled.into_event(whole, part);
                    Ok(vec![e])
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

/// Sampled source identity is separate from mutable sound/control payload.
struct SourceSample {
    event: Event,
}
impl SourceSample {
    fn from_event(event: Event) -> Self {
        Self { event }
    }
    fn fresh(value: Value, producer: Option<ProducerTrace>, at: Ratio64) -> Self {
        let mut event = Event::new(None, TimeSpan::point(at), value, None);
        event.producer = producer;
        Self { event }
    }
    fn into_event(self, whole: TimeSpan, part: TimeSpan) -> Event {
        if self.event.song_source.is_some() {
            Event::from_sample(Some(whole), part, self.event)
        } else {
            // Legacy sound sampling intentionally supplies only sound content.
            let mut event = Event::new(Some(whole), part, self.event.value, None);
            event.producer = self.event.producer;
            event
        }
    }
}

/// Resolves a sound source once, preserving the selected pattern's leaf path.
fn traced_source(
    source: &PParam,
    at: Ratio64,
    whole: TimeSpan,
    part: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Result<SourceSample, Failure> {
    if let PParam::Pat(child) = source {
        return sampled_source(child, at, whole, part, st);
    }
    let resolve = |st: &mut QState<'_, '_>| {
        let value = match source {
            PParam::Const(value) | PParam::Fn(value) => resolve_dynamic(value, at, st)?,
            PParam::Late(slot) => {
                let value = st.reenter(|st| st.cx.vm.deref(slot))?;
                resolve_dynamic(&value, at, st)?
            }
            PParam::Pat(child) => return sampled_source(child, at, whole, part, st),
        };
        match value {
            Value::Pattern(child) => sampled_source(&child, at, whole, part, st),
            Value::Signal(signal) => Ok(SourceSample::fresh(
                signal.value_at(at, st.cx)?,
                st.producer(),
                at,
            )),
            other => {
                crate::pattern::build::reject_finite(&other)?;
                Ok(SourceSample::fresh(other, st.producer(), at))
            }
        }
    };
    if source.is_const() {
        resolve(st)
    } else {
        st.with_producer(ProducerKind::DynamicExpansion, 0, resolve)
    }
}

fn sampled_source(
    child: &Pat,
    at: Ratio64,
    whole: TimeSpan,
    part: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Result<SourceSample, Failure> {
    // Resolving callbacks have finished before this exact child receives a permit.
    let event = st
        .with_structural_sample(child, at, 0, Some(whole), part, None, |st| {
            sample_child(child, at, 0, st)
        })?
        .into_iter()
        .next();
    Ok(match event {
        Some(event) => SourceSample::from_event(event),
        None => SourceSample::fresh(Value::Nil, st.producer(), at),
    })
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    #[test]
    fn callable_sound_sampling_retains_original_identity() {
        let mut source = crate::song::source::provenance_fixture();
        source.controls.insert(
            crate::value::intern::intern_kw("note"),
            Value::Float64(66.75),
        );
        source.controls.insert(
            crate::value::intern::intern_kw("gain"),
            Value::Float64(0.42),
        );
        use crate::ns::namespace::{FormGen, SlotKind, VarSlotRef};
        use crate::reader::span::{FileId, SrcRef};
        let slot = VarSlotRef::new(
            crate::value::intern::intern_sym("note-cell"),
            SlotKind::Var,
            Value::Float64(66.75),
        );
        source.late = Some(slot.clone());
        source
            .cells
            .insert(crate::value::intern::intern_kw("note"), slot.clone());
        source.src = Some(SrcRef {
            span: Span::new(FileId::new(7), 10, 20),
            doc_revision: 2,
            form_gen: FormGen::new(3),
        });
        let original_src = source.src;
        let origin = source.song_source.clone().unwrap();
        let span = TimeSpan::cycle(4).unwrap();
        let mut event = SourceSample::from_event(source).into_event(span, span);
        event.value = Value::Int(2);
        assert!(Rc::ptr_eq(&origin, event.song_source.as_ref().unwrap()));
        assert_eq!(origin.handle.tone(), 2);
        assert_eq!(event.src, original_src);
        assert!(Rc::ptr_eq(&slot.0, &event.late.as_ref().unwrap().0));
        assert!(Rc::ptr_eq(
            &slot.0,
            &event.cells[&crate::value::intern::intern_kw("note")].0
        ));
        assert!(event.producer.is_some());
        assert!(matches!(
            event.controls.get(&crate::value::intern::intern_kw("note")),
            Some(Value::Float64(66.75))
        ));
        assert!(matches!(
            event.controls.get(&crate::value::intern::intern_kw("gain")),
            Some(Value::Float64(0.42))
        ));
    }
}

#[cfg(test)]
mod legacy_sampling_tests {
    use super::*;
    #[test]
    fn legacy_sampled_sound_keeps_existing_control_free_behavior() {
        let mut source = crate::song::source::provenance_fixture();
        source.song_source = None;
        let span = TimeSpan::cycle(0).unwrap();
        let event = SourceSample::from_event(source).into_event(span, span);
        assert!(event.controls.is_empty());
        assert!(event.song_source.is_none());
        assert!(event.producer.is_some());
    }
}
