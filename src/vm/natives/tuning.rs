//! Native surface for tuning and chord performance patterns.

use std::rc::Rc;

use crate::ns::load::LoaderHost;
use crate::ns::namespace::Prelude;
use crate::pattern::build::{param_of, pattern_of};
use crate::pattern::combinators::{music, strum, tune};
use crate::pattern::pat::PParam;
use crate::pattern::signal::Sig;
use crate::pattern::tuning::{edo_spec, ratios_spec, scala_spec, Mapping, Tuning};
use crate::value::intern::{intern_sym, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::{PathVal, Value};
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::natives::pattern::{named, out, param, pat};
use crate::vm::natives::{arg, int_of, type_err};
use crate::vm::vm::EffectMode;

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("tune", tune_native);
    p.register("edo", edo);
    p.register("ratios", ratios);
    p.register("scala", scala);
    p.register("load-scala", load_scala);
    p.register("strum", strum_native);
    p.register("harp", harp);
    p.register("inversion", inversion);
    p.register("perform", perform);
}

fn tune_native(cx: &mut NativeCx<'_>, args: &[Value], kw: Kw<'_>) -> R {
    let mut spec = arg(args, 1);
    if matches!(spec, Value::Thunk(_)) {
        spec = cx.force(&spec)?;
    }
    let mapping = mapping(cx, kw)?;
    if !matches!(
        spec,
        Value::Pattern(_) | Value::Signal(_) | Value::List(_) | Value::Range(_)
    ) {
        Tuning::from_spec(&spec)?.with_mapping(&mapping)?;
    }
    let tunings = pattern_of(&spec, None)?;
    out(cx, tune::tune(tunings, pat(args, 0)?, mapping, cx.span))
}

fn mapping(cx: &mut NativeCx<'_>, kw: Kw<'_>) -> Result<Mapping, Failure> {
    Ok(Mapping {
        root: named(kw, "root")
            .map(|v| key_value(&cx.deep(v)?, "root"))
            .transpose()?,
        ref_key: named(kw, "ref-key")
            .map(|v| key_value(&cx.deep(v)?, "ref-key"))
            .transpose()?,
        ref_freq: named(kw, "ref-freq")
            .map(|v| {
                let value = cx.deep(v)?;
                let frequency = crate::pattern::eval::num_f64(&value)
                    .filter(|n| n.is_finite() && *n > 0.0)
                    .ok_or_else(|| type_err("`ref-freq:` expects a positive finite number"))?;
                Ok(frequency)
            })
            .transpose()?,
    })
}

fn key_value(value: &Value, name: &str) -> Result<i64, Failure> {
    match value {
        Value::Keyword(k) => {
            crate::pattern::combinators::music::note_number(&crate::value::intern::name_of_kw(*k))
                .ok_or_else(|| {
                    type_err(format!("`{name}:` expects an integer key or note keyword"))
                })
        }
        Value::Int(_) | Value::Int64(_) => int_of(value, name),
        other => Err(type_err(format!(
            "`{name}:` expects an integer key or note keyword, got {}",
            kind_name(other)
        ))),
    }
}

fn edo(cx: &mut NativeCx<'_>, args: &[Value], kw: Kw<'_>) -> R {
    let period = named(kw, "period").map(|v| cx.deep(v)).transpose()?;
    edo_spec(&arg(args, 0), period.as_ref())
}

fn ratios(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let list = cx.deep(&arg(args, 0))?;
    ratios_spec(&list)
}

fn scala(_: &mut NativeCx<'_>, args: &[Value], kw: Kw<'_>) -> R {
    let text_value = arg(args, 0);
    let text = string_arg(&text_value, "scala")?;
    let kbm = named(kw, "kbm")
        .map(|v| string_arg(v, "scala kbm"))
        .transpose()?;
    scala_spec(text, kbm)
}

fn string_arg<'a>(value: &'a Value, what: &str) -> Result<&'a str, Failure> {
    match value {
        Value::Str(text) => Ok(text),
        other => Err(type_err(format!(
            "`{what}` expects a string, got {}",
            kind_name(other)
        ))),
    }
}

fn load_scala(cx: &mut NativeCx<'_>, args: &[Value], kw: Kw<'_>) -> R {
    if cx.effect_mode() == EffectMode::Query {
        return Err(Failure::new(
            FailCode::EffectInQuery,
            "`load-scala` is not allowed inside a query",
        ));
    }
    let scl_path = path_arg(&arg(args, 0), "load-scala")?;
    let kbm_path = named(kw, "kbm")
        .map(|value| path_arg(value, "load-scala kbm"))
        .transpose()?;
    let Some(mut host) = cx.vm.take_host() else {
        return Err(Failure::new(
            FailCode::HostUnavailable,
            "no source loader is installed",
        ));
    };
    let read = match host.downcast_mut::<LoaderHost>() {
        Some(LoaderHost(loader, _)) => (|| {
            let (_, scl) = loader.read(&scl_path)?;
            let kbm = kbm_path
                .as_ref()
                .map(|path| loader.read(path).map(|(_, text)| text))
                .transpose()?;
            scala_spec(&scl, kbm.as_deref())
        })(),
        None => Err(Failure::new(
            FailCode::HostUnavailable,
            "no source loader is installed",
        )),
    };
    cx.vm.set_host(Some(host));
    read
}

fn path_arg(value: &Value, what: &str) -> Result<Rc<PathVal>, Failure> {
    match value {
        Value::Path(path) => Ok(Rc::clone(path)),
        other => Err(type_err(format!(
            "`{what}` expects a path, got {}",
            kind_name(other)
        ))),
    }
}

fn strum_native(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    let time = param(args, 1)?;
    let dir = optional_param(args, 2, Value::kw("up"))?;
    let curve = optional_param(args, 3, Value::kw("flat"))?;
    out(cx, strum::strum(pat(args, 0)?, time, dir, curve, cx.span))
}

fn harp(cx: &mut NativeCx<'_>, args: &[Value], kw: Kw<'_>) -> R {
    let strips = named(kw, "strips")
        .map(|v| int_of(v, "`strips:`"))
        .transpose()?
        .unwrap_or(12);
    let base = named(kw, "base")
        .map(|v| key_value(v, "base"))
        .transpose()?;
    out(
        cx,
        strum::harp(pat(args, 0)?, param(args, 1)?, strips, base, cx.span)?,
    )
}

fn inversion(cx: &mut NativeCx<'_>, args: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        strum::inversion(pat(args, 0)?, param(args, 1)?, false, cx.span),
    )
}

fn perform(cx: &mut NativeCx<'_>, args: &[Value], kw: Kw<'_>) -> R {
    let mut chords = arg(args, 1);
    if matches!(chords, Value::Thunk(_)) {
        chords = cx.force(&chords)?;
    }
    let chords = music::chord_pattern_of(&chords, None)?;
    let mut pattern = Rc::new(music::chord(chords, pat(args, 0)?, cx.span));
    pattern = Rc::new(music::voicing(pattern, cx.span));
    let inversion = named(kw, "inversion")
        .map(param_of)
        .transpose()?
        .unwrap_or_else(|| PParam::int(0));
    let bass = named(kw, "bass")
        .map(|value| match value {
            Value::Bool(value) => Ok(*value),
            other => Err(type_err(format!(
                "`bass:` expects a boolean, got {}",
                kind_name(other)
            ))),
        })
        .transpose()?
        .unwrap_or(false);
    pattern = Rc::new(strum::inversion(pattern, inversion, bass, cx.span));
    let mode = keyword_kw(kw, "mode", "block")?;
    let result = match mode.as_str() {
        "block" => (*pattern).clone(),
        "strum" => {
            let time = named(kw, "time")
                .map(param_of)
                .transpose()?
                .unwrap_or_else(default_strum_time);
            let dir = named(kw, "dir")
                .map(param_of)
                .transpose()?
                .unwrap_or_else(|| PParam::Const(Value::kw("up")));
            let curve = named(kw, "curve")
                .map(param_of)
                .transpose()?
                .unwrap_or_else(|| PParam::Const(Value::kw("flat")));
            strum::strum(pattern, time, dir, curve, cx.span)
        }
        "arp" => {
            let mode = named(kw, "arp")
                .map(param_of)
                .transpose()?
                .unwrap_or_else(|| PParam::Const(Value::kw("up")));
            music::arp(pattern, mode, cx.span)
        }
        "harp" => {
            let pos = match named(kw, "pos") {
                Some(value) => param_of(value)?,
                None => param_of(&saw_value(cx))?,
            };
            let strips = named(kw, "strips")
                .map(|v| int_of(v, "`strips:`"))
                .transpose()?
                .unwrap_or(12);
            let base = named(kw, "base")
                .map(|v| key_value(v, "base"))
                .transpose()?;
            strum::harp(pattern, pos, strips, base, cx.span)?
        }
        _ => {
            return Err(type_err(
                "`perform mode:` must be :block, :strum, :arp or :harp",
            ));
        }
    };
    out(cx, result)
}

fn keyword_kw(kw: Kw<'_>, name: &str, default: &str) -> Result<String, Failure> {
    match named(kw, name) {
        None => Ok(default.to_owned()),
        Some(Value::Keyword(k)) => Ok(crate::value::intern::name_of_kw(*k).to_string()),
        Some(other) => Err(type_err(format!(
            "`{name}:` expects a keyword, got {}",
            kind_name(other)
        ))),
    }
}

fn optional_param(args: &[Value], index: usize, default: Value) -> Result<PParam, Failure> {
    if args.len() > index {
        param(args, index)
    } else {
        Ok(PParam::Const(default))
    }
}

fn default_strum_time() -> PParam {
    PParam::Const(Value::Ratio(Ratio64::new(1, 32).unwrap_or(Ratio64::ONE)))
}

fn saw_value(cx: &NativeCx<'_>) -> Value {
    cx.ns
        .prelude()
        .slot(intern_sym("saw"))
        .map_or_else(|| Value::Signal(Rc::new(Sig::Saw)), |slot| slot.get())
}
