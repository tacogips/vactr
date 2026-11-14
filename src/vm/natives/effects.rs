//! Slot and time effects (lang-reference section 5, design 7.1.3): each
//! stages a `StagedEffect`, released only when the whole form succeeds, and
//! fails `effect-in-query` in Query mode (10.4). The bound value may be any
//! value in this wave; pattern construction is ME-PATTERN/ME-INTEGRATE's.

use crate::ns::namespace::Prelude;
use crate::ns::stage::{SlotKey, StagedEffect, TempoChange};
use crate::value::intern::{intern_kw, name_of_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::Failure;
use crate::vm::natives::{arg, type_err};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("d1", d1);
    p.register("d2", d2);
    p.register("d3", d3);
    p.register("d4", d4);
    p.register("d5", d5);
    p.register("d6", d6);
    p.register("d7", d7);
    p.register("d8", d8);
    p.register("d9", d9);
    p.register("slot", slot);
    p.register("once", once);
    p.register("at", at);
    p.register("hush", hush);
    p.register("stop", stop);
    p.register("use-bpm", use_bpm);
    p.register("use-cycle", use_cycle);
    p.register("use-clock", use_clock);
    p.register("midi-clock-out", midi_clock_out);
}

fn bind(cx: &mut NativeCx<'_>, slot: SlotKey, v: Value) -> R {
    cx.stage(StagedEffect::SlotBind {
        slot,
        value: v.clone(),
    })?;
    Ok(v)
}

macro_rules! sink {
    ($($name:ident => $n:literal,)*) => {
        $(
            fn $name(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
                bind(cx, SlotKey::D($n), arg(args, 0))
            }
        )*
    };
}

sink! {
    d1 => 1, d2 => 2, d3 => 3, d4 => 4, d5 => 5, d6 => 6, d7 => 7, d8 => 8, d9 => 9,
}

fn keyword(v: &Value, what: &str) -> Result<KwId, Failure> {
    match v {
        Value::Keyword(k) => Ok(*k),
        other => Err(type_err(format!(
            "{what} expects a keyword, got {}",
            kind_name(other)
        ))),
    }
}

/// `slot :name v`: a named slot (control pairs are overrides for TASK-007).
fn slot(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let name = keyword(&arg(args, 0), "`slot`")?;
    bind(cx, SlotKey::Named(name), arg(args, 1))
}

/// A beat position as an exact ratio.
fn beat(v: &Value) -> Result<Ratio64, Failure> {
    match v {
        Value::Int(i) => Ok(Ratio64::from_int(i64::from(*i))),
        Value::Int64(i) => Ok(Ratio64::from_int(*i)),
        Value::Ratio(r) => Ok(*r),
        Value::Float(x) => {
            Ratio64::from_f64_exact(f64::from(*x)).ok_or_else(|| type_err("a beat must be finite"))
        }
        Value::Float64(x) => {
            Ratio64::from_f64_exact(*x).ok_or_else(|| type_err("a beat must be finite"))
        }
        other => Err(type_err(format!(
            "a beat is a number, got {}",
            kind_name(other)
        ))),
    }
}

/// `once v at: t gain: g ..`: one-shot, with control overrides.
fn once(cx: &mut NativeCx<'_>, args: &[Value], kw: Kw<'_>) -> R {
    let at_kw = intern_kw("at");
    let mut at = None;
    let mut overrides = Vec::new();
    for (k, v) in kw {
        if *k == at_kw {
            at = Some(beat(&cx.deep(v)?)?);
        } else {
            overrides.push((*k, v.clone()));
        }
    }
    cx.stage(StagedEffect::OneShot {
        at,
        value: arg(args, 0),
        overrides,
    })?;
    Ok(Value::Nil)
}

/// `at t body`: `body` is passed unevaluated (an `Fn` parameter) and runs
/// when the scheduler reaches `t` (TASK-007).
fn at(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let t = beat(&arg(args, 0))?;
    cx.stage(StagedEffect::OneShot {
        at: Some(t),
        value: arg(args, 1),
        overrides: Vec::new(),
    })?;
    Ok(Value::Nil)
}

fn hush(cx: &mut NativeCx<'_>, _: &[Value], _: Kw<'_>) -> R {
    cx.stage(StagedEffect::Revoke(SlotKey::All))?;
    Ok(Value::Nil)
}

/// `stop :d1` or `stop :name`.
fn stop(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let k = keyword(&arg(args, 0), "`stop`")?;
    let name = name_of_kw(k);
    let key = match name.strip_prefix('d').and_then(|n| n.parse::<u8>().ok()) {
        Some(n @ 1..=9) if name.len() == 2 => SlotKey::D(n),
        _ => SlotKey::Named(k),
    };
    cx.stage(StagedEffect::Revoke(key))?;
    Ok(Value::Nil)
}

fn use_bpm(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    cx.stage(StagedEffect::Tempo(TempoChange::Bpm(arg(args, 0))))?;
    Ok(Value::Nil)
}

fn use_cycle(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    cx.stage(StagedEffect::Tempo(TempoChange::Cycle(arg(args, 0))))?;
    Ok(Value::Nil)
}

fn use_clock(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let k = keyword(&arg(args, 0), "`use-clock`")?;
    cx.stage(StagedEffect::Tempo(TempoChange::Clock(k)))?;
    Ok(Value::Nil)
}

fn midi_clock_out(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let on = match arg(args, 0) {
        Value::Bool(b) => b,
        other => {
            return Err(type_err(format!(
                "`midi-clock-out` expects a bool, got {}",
                kind_name(&other)
            )))
        }
    };
    cx.stage(StagedEffect::Tempo(TempoChange::MidiClockOut(on)))?;
    Ok(Value::Nil)
}
