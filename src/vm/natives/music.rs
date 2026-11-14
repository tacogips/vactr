//! Control and music natives (design-music.md sections 2-3, design 7.1.4):
//! the pattern controls, `n`/`note` (structure-giving steps after `s`,
//! 10.1), `chord`, `voicing`, `arp`, `range`, and the subject-overloaded
//! `scale` and `shape` (M1): the runtime tag of the subject picks the
//! pattern or the visual meaning, and a tag outside the group is `type`.

use crate::ns::namespace::Prelude;
use crate::pattern::build::pattern_of;
use crate::pattern::combinators::control::control;
use crate::pattern::combinators::music;
use crate::pattern::combinators::music::chord_pattern_of;
use crate::tex::texnode::{scale_texture, shape_source};
use crate::value::intern::{intern_kw, KwId};
use crate::value::value::Value;
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::Failure;
use crate::vm::natives::pattern::{keyword_now, out, param, pat};
use crate::vm::natives::tex::{tex_out, vparams};
use crate::vm::natives::{arg, type_err};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

/// Every control native except `shape` (in the overload group), with the
/// structure-giving `n` and `note`.
const CONTROLS: [&str; 26] = [
    "n",
    "note",
    "gain",
    "pan",
    "speed",
    "lpf",
    "hpf",
    "resonance",
    "room",
    "size",
    "delay",
    "delaytime",
    "delayfeedback",
    "crush",
    "vowel",
    "legato",
    "attack",
    "release",
    "sustain",
    "begin",
    "end",
    "cut",
    "orbit",
    "velocity",
    "start-ms",
    "stop-ms",
];

pub(super) fn register(p: &mut Prelude) {
    for name in CONTROLS {
        p.register(name, control_native);
    }
    p.register("shape", shape);
    p.register("scale", scale);
    p.register("chord", chord);
    p.register("voicing", voicing);
    p.register("arp", arp);
    p.register("range", range);
    p.register("inst control", inst_control_native);
}

/// `subject > gain v`: the control named by the native's own entry. The
/// value is a pattern (a list gives structure to an unstructured subject;
/// a `var` or block is read per query).
fn control_native(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let name = cx
        .ns
        .prelude()
        .native(cx.id)
        .map(|e| e.sig.name)
        .ok_or_else(|| type_err("unregistered control"))?;
    out(cx, control(intern_kw(name), pat(a, 1)?, pat(a, 0)?, None))
}

/// An `inst` template header parameter used as a call head (compiler.rs,
/// design 12.8.6 M3, B2): the control name is not the native's own table
/// entry (it has none) but the keyword the compiler appended as the last
/// argument.
fn inst_control_native(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let Value::Keyword(kw) = arg(a, 2) else {
        return Err(type_err(format!(
            "`inst control` expects a keyword control name, got {}",
            kind_name(&arg(a, 2))
        )));
    };
    out(cx, control(kw, pat(a, 1)?, pat(a, 0)?, None))
}

/// True for a subject in the pattern domain (a pattern, a step list, a
/// range or a signal).
fn is_pattern_like(v: &Value) -> bool {
    matches!(
        v,
        Value::Pattern(_) | Value::List(_) | Value::Range(_) | Value::Signal(_)
    )
}

fn is_number(v: &Value) -> bool {
    matches!(
        v,
        Value::Int(_) | Value::Int64(_) | Value::Float(_) | Value::Float64(_) | Value::Ratio(_)
    )
}

/// `scale`: a pattern subject is scale notes (`scale :c :minor`); a
/// texture subject is the visual transform (`scale amount ..`).
fn scale(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    match arg(a, 0) {
        Value::Tex(t) => tex_out(scale_texture(t, vparams(&a[1..])?, cx.span)),
        v if is_pattern_like(&v) => {
            if a.len() != 3 {
                return Err(Failure::new(
                    crate::vm::fail::FailCode::Arity,
                    "`scale` on a pattern takes a root and a scale name",
                ));
            }
            let root = keyword_now(cx, a, 1, "`scale`")?;
            let name = keyword_now(cx, a, 2, "`scale`")?;
            out(cx, music::scale(pattern_of(&v, None)?, root, name, None)?)
        }
        other => Err(type_err(format!(
            "`scale` expects a pattern or a texture, got {}",
            kind_name(&other)
        ))),
    }
}

/// `shape`: a number subject is the visual source (`shape 3`); a pattern
/// subject is the `shape` control (`s :bd > shape 0.5`).
fn shape(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    match arg(a, 0) {
        v if is_number(&v) => tex_out(shape_source(vparams(a)?, cx.span)),
        v if is_pattern_like(&v) => {
            if a.len() != 2 {
                return Err(Failure::new(
                    crate::vm::fail::FailCode::Arity,
                    "the `shape` control takes one value",
                ));
            }
            out(
                cx,
                control(intern_kw("shape"), pat(a, 1)?, pat(a, 0)?, None),
            )
        }
        other => Err(type_err(format!(
            "`shape` expects a number or a pattern, got {}",
            kind_name(&other)
        ))),
    }
}

/// `subject > chord value`: a `[root quality]` list is ONE chord. A block
/// is forced here, so the chord literals it builds are recognized; a `var`
/// stays late (one atomic chord value per query).
fn chord(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let v = match arg(a, 1) {
        t @ Value::Thunk(_) => cx.force(&t)?,
        other => other,
    };
    out(
        cx,
        music::chord(chord_pattern_of(&v, None)?, pat(a, 0)?, None),
    )
}

fn voicing(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, music::voicing(pat(a, 0)?, None))
}

/// `arp p :up`: the mode is read per query.
fn arp(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, music::arp(pat(a, 0)?, param(a, 1)?, None))
}

/// `range sine 200 2000` (subject first, M2).
fn range(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        music::range(pat(a, 0)?, param(a, 1)?, param(a, 2)?, None),
    )
}
