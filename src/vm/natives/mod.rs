//! Native implementations, registered against the native signature table
//! (design 7.1.3): the lang-reference section 5 core prelude, the slot and
//! time effects, and the domain natives and prelude values (patterns,
//! controls, signals, visuals, sounds and the sound kit). `load` is
//! registered by the `Evaluator` (`ns/load.rs`).
//!
//! A native receives its positional arguments already forced per its mask
//! (a `Value` position never sees a thunk or a `VarRef`), and its named
//! arguments forced. Items INSIDE a list may still be `VarRef`s (late-bound
//! names and tweak sites in literals); natives that inspect items use
//! `NativeCx::deep`.

pub mod analysis;
pub mod console;
pub mod dict;
pub mod dsp;
pub mod effects;
pub mod list;
pub mod music;
pub mod num;
pub mod pattern;
pub mod signal;
pub mod song;
pub mod sound;
pub mod tex;
pub mod tuning;
pub mod value;

use std::rc::Rc;

use crate::ns::namespace::Prelude;
use crate::value::dict::pairs;
use crate::value::intern::KwId;
use crate::value::value::{ListVal, Value};
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::vm::int_value;

/// A native implementation: positional arguments, named arguments.
pub type NativeFn = fn(&mut NativeCx<'_>, &[Value], &[(KwId, Value)]) -> Result<Value, Failure>;

/// Registers the core natives (lang-reference section 5, slots and time).
pub fn register_core(p: &mut Prelude) {
    num::register(p);
    list::register(p);
    dict::register(p);
    value::register(p);
    console::register(p);
    effects::register(p);
}

/// Registers the domain natives and prelude values (patterns, controls,
/// signals, visuals, sounds, the sound kit), once: a prelude that already
/// binds `s` is left as it is.
pub fn register_domain(p: &mut Prelude) {
    if p.slot(crate::value::intern::intern_sym("s")).is_some() {
        return;
    }
    pattern::register(p);
    music::register(p);
    signal::register(p);
    sound::register(p);
    tex::register(p);
    register_dsp(p);
    analysis::register(p);
    song::register(p);
    tuning::register(p);
}

/// The DSP natives, except `spectrum`: `analysis` registers the one
/// `spectrum` native, which keeps the analyzer-unit meaning for a ugen or
/// bus-body subject and adds the tap and buffer forms (14.5.9).
fn register_dsp(p: &mut Prelude) {
    let mut d = Prelude::empty();
    dsp::register(&mut d);
    let natives: Vec<(&'static str, NativeFn)> = d
        .natives()
        .map(|(_, e)| (e.sig.name, e.f))
        .filter(|(name, _)| *name != "spectrum")
        .collect();
    for (name, f) in natives {
        p.register(name, f);
    }
    for sym in d.names() {
        let Some(slot) = d.slot(sym) else { continue };
        let v = slot.get();
        if !matches!(v, Value::Native(_)) {
            p.register_value(&crate::value::intern::name_of_sym(sym), v);
        }
    }
}

/// The complete prelude: the core and the domain (everything in the table
/// except `load`, which the `Evaluator` registers).
#[must_use]
pub fn full_prelude() -> Prelude {
    let mut p = Prelude::core();
    register_domain(&mut p);
    p
}

/// A `type` failure.
pub(crate) fn type_err(msg: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, msg)
}

/// The argument `k`, or `nil` when absent.
pub(crate) fn arg(args: &[Value], k: usize) -> Value {
    args.get(k).cloned().unwrap_or(Value::Nil)
}

/// An integer argument.
pub(crate) fn int_of(v: &Value, what: &str) -> Result<i64, Failure> {
    match v {
        Value::Int(i) => Ok(i64::from(*i)),
        Value::Int64(i) => Ok(*i),
        Value::Ratio(r) if r.is_integral() => Ok(r.num()),
        other => Err(type_err(format!(
            "{what} expects an integer, got {}",
            kind_name(other)
        ))),
    }
}

/// A list value.
pub(crate) fn list_of(items: Vec<Value>) -> Value {
    Value::List(Rc::new(ListVal {
        items: items.into_boxed_slice(),
        prov: None,
    }))
}

/// Iterates a sequence: list items, dict pairs, range elements (lazy and
/// fuel-metered, so `0..` without `take` ends in `fuel-exhausted`), `nil`
/// as empty.
pub(crate) enum Seq {
    List { list: Rc<ListVal>, at: usize },
    Items(std::vec::IntoIter<Value>),
    Range { next: i64, end: Option<i64> },
}

impl Seq {
    pub(crate) fn of(v: &Value, what: &str) -> Result<Seq, Failure> {
        match v {
            Value::Nil => Ok(Seq::Items(Vec::new().into_iter())),
            Value::List(l) => Ok(Seq::List {
                list: Rc::clone(l),
                at: 0,
            }),
            Value::Dict(d) => Ok(Seq::Items(pairs(d).collect::<Vec<_>>().into_iter())),
            Value::Range(r) => Ok(Seq::Range {
                next: r.start,
                end: r.end,
            }),
            other => Err(type_err(format!(
                "{what} expects a list, got {}",
                kind_name(other)
            ))),
        }
    }

    /// The next item, consuming one unit of fuel.
    pub(crate) fn next(&mut self, cx: &mut NativeCx<'_>) -> Result<Option<Value>, Failure> {
        cx.tick()?;
        match self {
            Seq::List { list, at } => {
                let v = list.items.get(*at).cloned();
                *at += 1;
                Ok(v)
            }
            Seq::Items(it) => Ok(it.next()),
            Seq::Range { next, end } => {
                if end.is_some_and(|e| *next >= e) {
                    return Ok(None);
                }
                let v = int_value(*next);
                *next = next
                    .checked_add(1)
                    .ok_or_else(|| Failure::new(FailCode::Overflow, "range overflow"))?;
                Ok(Some(v))
            }
        }
    }

    /// Every item (fails on an unbounded range once fuel runs out).
    pub(crate) fn collect(mut self, cx: &mut NativeCx<'_>) -> Result<Vec<Value>, Failure> {
        let mut out = Vec::new();
        while let Some(v) = self.next(cx)? {
            out.push(v);
        }
        Ok(out)
    }
}

/// How deep `NativeCx::deep` descends into nested lists and dicts.
const MAX_DEEP: u32 = 64;

impl NativeCx<'_> {
    /// Replaces every `VarRef` inside lists and dicts with its current value
    /// (an eager read), for natives that compare or print items.
    ///
    /// # Errors
    /// `undefined-name` for an unbound slot, or an observer abort.
    pub fn deep(&mut self, v: &Value) -> Result<Value, Failure> {
        self.deep_at(v, 0)
    }

    fn deep_at(&mut self, v: &Value, depth: u32) -> Result<Value, Failure> {
        if depth > MAX_DEEP {
            return Ok(v.clone());
        }
        match v {
            Value::VarRef(slot) => {
                let inner = self.vm.read_slot(slot)?;
                self.deep_at(&inner, depth + 1)
            }
            Value::List(l) => {
                if !l.items.iter().any(|x| needs_deep(x, 0)) {
                    return Ok(v.clone());
                }
                let mut items = Vec::with_capacity(l.items.len());
                for x in l.items.iter() {
                    items.push(self.deep_at(x, depth + 1)?);
                }
                Ok(Value::List(Rc::new(ListVal {
                    items: items.into_boxed_slice(),
                    prov: l.prov.clone(),
                })))
            }
            Value::Dict(d) => {
                if !d.values().any(|x| needs_deep(x, 0)) {
                    return Ok(v.clone());
                }
                let mut out = std::collections::BTreeMap::new();
                for (k, x) in d.iter() {
                    out.insert(k.clone(), self.deep_at(x, depth + 1)?);
                }
                Ok(Value::dict(out))
            }
            other => Ok(other.clone()),
        }
    }
}

fn needs_deep(v: &Value, depth: u32) -> bool {
    if depth > MAX_DEEP {
        return false;
    }
    match v {
        Value::VarRef(_) => true,
        Value::List(l) => l.items.iter().any(|x| needs_deep(x, depth + 1)),
        Value::Dict(d) => d.values().any(|x| needs_deep(x, depth + 1)),
        _ => false,
    }
}
