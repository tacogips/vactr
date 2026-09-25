//! Visual natives (design 9.1, design-visual.md section 2): sources,
//! geometry, color, blend and modulate operators build `TexNode` chains;
//! `out` validates the chain (`compile_tex`) and stages the texture bind.
//!
//! Every numeric parameter is `Late` (5.5 "osc is all-Late"): it is kept as
//! a `VParam` as it arrived, so a block such as `osc {* 20 {sin time}}`
//! stays a thunk and is forced once per `resolve_uniforms`, never at the
//! call.

use std::rc::Rc;

use crate::ns::namespace::Prelude;
use crate::ns::stage::{SlotKey, StagedEffect};
use crate::pattern::build::pattern_of;
use crate::pattern::combinators::music::range as range_node;
use crate::pattern::pat::PParam;
use crate::tex::shader::compile_tex;
use crate::tex::texnode::{pipe, BlendOp, ModKind, OutId, TexKind, TexNode, VParam};
use crate::types::natives::NativeTable;
use crate::value::intern::{intern_kw, name_of_kw, KwId};
use crate::value::value::{Sound, Value};
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::Failure;
use crate::vm::natives::pattern::out as pat_out;
use crate::vm::natives::{arg, type_err};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

/// Sources whose every parameter is optional (Hydra defaults).
const SOURCES: [&str; 5] = ["osc", "noise", "voronoi", "gradient", "solid"];

/// Geometry and color operators: a texture subject, then parameters.
const UNARY: [&str; 18] = [
    "rotate",
    "pixelate",
    "tile",
    "tile-x",
    "tile-y",
    "kaleid",
    "scroll",
    "posterize",
    "shift",
    "invert",
    "contrast",
    "brightness",
    "luma",
    "thresh",
    "color",
    "saturate",
    "hue",
    "colorama",
];

const BLENDS: [(&str, BlendOp); 5] = [
    ("layer", BlendOp::Layer),
    ("blend", BlendOp::Blend),
    ("mult", BlendOp::Mult),
    ("diff", BlendOp::Diff),
    ("mask", BlendOp::Mask),
];

const MODULATES: [(&str, ModKind); 7] = [
    ("modulate", ModKind::Modulate),
    ("modulate-tile", ModKind::Tile),
    ("modulate-kaleid", ModKind::Kaleid),
    ("modulate-scroll", ModKind::Scroll),
    ("modulate-rotate", ModKind::Rotate),
    ("modulate-scale", ModKind::Scale),
    ("modulate-pixelate", ModKind::Pixelate),
];

/// The visual outputs.
const OUTPUTS: [&str; 4] = ["o0", "o1", "o2", "o3"];

pub(super) fn register(p: &mut Prelude) {
    for name in SOURCES {
        p.register(name, source);
    }
    for name in UNARY {
        p.register(name, unary);
    }
    for (name, _) in BLENDS {
        p.register(name, blend);
    }
    for (name, _) in MODULATES {
        p.register(name, modulate);
    }
    p.register("add", add);
    p.register("sub", sub);
    p.register("src", src);
    p.register("text", text);
    p.register("out", out);
    p.register("render", render);
    p.register("use-fps", use_fps);
    p.register("use-canvas", use_canvas);
    for name in OUTPUTS {
        p.register_value(name, Value::kw(name));
    }
}

/// A texture result.
pub(crate) fn tex_out(t: TexNode) -> R {
    Ok(Value::Tex(Rc::new(t)))
}

/// The visual parameters, each kept as it arrived.
pub(crate) fn vparams(args: &[Value]) -> Result<Vec<VParam>, Failure> {
    args.iter().map(VParam::from_value).collect()
}

/// The name of the native being called.
fn own_name(cx: &NativeCx<'_>) -> Result<&'static str, Failure> {
    cx.ns
        .prelude()
        .native(cx.id)
        .map(|e| e.sig.name)
        .ok_or_else(|| type_err("unregistered visual native"))
}

fn tex_arg(v: &Value, what: &str) -> Result<Rc<TexNode>, Failure> {
    match v {
        Value::Tex(t) => Ok(Rc::clone(t)),
        other => Err(type_err(format!(
            "{what} expects a texture, got {}",
            kind_name(other)
        ))),
    }
}

fn source(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let name = own_name(cx)?;
    // `osc "/addr"` is an OSC-out sound; `osc 20` stays the Hydra source.
    if let ("osc", [Value::Str(addr)]) = (name, a) {
        return Ok(Value::Sound(Rc::new(Sound::Osc(Rc::clone(addr)))));
    }
    let kind = TexKind::from_name(name).ok_or_else(|| type_err("unknown source"))?;
    tex_out(TexNode::new(kind, vparams(a)?, cx.span))
}

fn unary(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let name = own_name(cx)?;
    let kind = TexKind::from_name(name).ok_or_else(|| type_err("unknown operator"))?;
    let subject = tex_arg(&arg(a, 0), &format!("`{name}`"))?;
    let op = TexNode::new(kind, vparams(a.get(1..).unwrap_or(&[]))?, cx.span);
    tex_out(pipe(subject, op, cx.span))
}

/// `a > blend b amount`: combines with another chain.
fn blend_with(cx: &NativeCx<'_>, op: BlendOp, a: &[Value], name: &str) -> R {
    let subject = tex_arg(&arg(a, 0), &format!("`{name}`"))?;
    let other = tex_arg(&arg(a, 1), &format!("`{name}`"))?;
    let node = TexNode::new(
        TexKind::Blend(op, other),
        vparams(a.get(2..).unwrap_or(&[]))?,
        cx.span,
    );
    tex_out(pipe(subject, node, cx.span))
}

fn blend(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let name = own_name(cx)?;
    let op = BLENDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, op)| *op)
        .ok_or_else(|| type_err("unknown blend"))?;
    blend_with(cx, op, a, name)
}

fn modulate(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let name = own_name(cx)?;
    let kind = MODULATES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
        .ok_or_else(|| type_err("unknown modulate operator"))?;
    let subject = tex_arg(&arg(a, 0), &format!("`{name}`"))?;
    let other = tex_arg(&arg(a, 1), &format!("`{name}`"))?;
    let node = TexNode::new(
        TexKind::Modulate(kind, other),
        vparams(a.get(2..).unwrap_or(&[]))?,
        cx.span,
    );
    tex_out(pipe(subject, node, cx.span))
}

/// `add`/`sub` on the subject's tag: a texture blends, a number is the
/// arithmetic word alias of `+`/`-`, and a pattern adds to every event
/// value (`add p 7`, design-music section 3) as `range p k k+1`, which is
/// `k + v` exactly.
fn arith(cx: &mut NativeCx<'_>, a: &[Value], negate: bool) -> R {
    let (name, op, word) = if negate {
        ("sub", BlendOp::Sub, "-")
    } else {
        ("add", BlendOp::Add, "+")
    };
    match arg(a, 0) {
        Value::Tex(_) => blend_with(cx, op, a, name),
        Value::Pattern(_) | Value::List(_) | Value::Range(_) | Value::Signal(_) => {
            if a.len() != 2 {
                return Err(Failure::new(
                    crate::vm::fail::FailCode::Arity,
                    format!("`{name}` on a pattern takes one number"),
                ));
            }
            let k = arg(a, 1);
            let lo = if negate {
                cx.call(&core_native("neg")?, vec![k])?
            } else {
                k
            };
            let hi = cx.call(&core_native("+")?, vec![lo.clone(), Value::Int(1)])?;
            let p = pattern_of(&arg(a, 0), None)?;
            pat_out(
                cx,
                range_node(p, PParam::Const(lo), PParam::Const(hi), None),
            )
        }
        _ => cx.call(&core_native(word)?, a.to_vec()),
    }
}

/// A core arithmetic native as a callable value.
fn core_native(name: &str) -> R {
    NativeTable::global()
        .get(name)
        .map(|(id, _)| Value::Native(id))
        .ok_or_else(|| type_err(format!("`{name}` is missing")))
}

fn add(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    arith(cx, a, false)
}

fn sub(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    arith(cx, a, true)
}

/// The output named by a keyword (`o0`..`o3`).
fn output(v: &Value) -> Result<(KwId, OutId), Failure> {
    let Value::Keyword(k) = v else {
        return Err(type_err(format!(
            "an output is o0..o3, got {}",
            kind_name(v)
        )));
    };
    let n = OUTPUTS
        .iter()
        .position(|o| **o == *name_of_kw(*k))
        .ok_or_else(|| type_err(format!("unknown output :{}", name_of_kw(*k))))?;
    Ok((*k, OutId::new(u32::try_from(n).unwrap_or(0))))
}

/// `src o0`: the previous frame of an output.
fn src(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let (_, id) = output(&arg(a, 0))?;
    tex_out(TexNode::new(TexKind::SrcOut(id), Vec::new(), cx.span))
}

/// `text "hello"`: on-screen text.
fn text(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    match arg(a, 0) {
        Value::Str(s) => tex_out(TexNode::new(TexKind::Text(s), Vec::new(), cx.span)),
        other => Err(type_err(format!(
            "`text` expects a string, got {}",
            kind_name(&other)
        ))),
    }
}

/// `chain > out o1` (default `o0`): the chain must compile (9.2), then the
/// bind is staged. A failing chain stages nothing, so the previous binding
/// of the output stays.
fn out(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let t = tex_arg(&arg(a, 0), "`out`")?;
    let (k, _) = match a.get(1) {
        Some(v) => output(v)?,
        None => output(&Value::Keyword(intern_kw("o0")))?,
    };
    compile_tex(&t)?;
    let value = Value::Tex(t);
    cx.stage(StagedEffect::SlotBind {
        slot: SlotKey::Named(k),
        value: value.clone(),
    })?;
    Ok(value)
}

/// Render settings: effects (`effect-in-query` inside a query). The render
/// host that consumes them is TASK-007/010's, so nothing is staged yet.
fn render_setting(cx: &mut NativeCx<'_>) -> R {
    let name = own_name(cx)?;
    cx.vm.check_effect(&format!("`{name}`"))?;
    Ok(Value::Nil)
}

fn render(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    if let Some(v) = a.first() {
        output(v)?;
    }
    render_setting(cx)
}

fn use_fps(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let v = cx.deep(&arg(a, 0))?;
    if crate::pattern::eval::num_f64(&v).is_none() {
        return Err(type_err("`use-fps` expects a number"));
    }
    render_setting(cx)
}

fn use_canvas(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    for k in 0..2 {
        crate::vm::natives::int_of(&arg(a, k), "`use-canvas`")?;
    }
    render_setting(cx)
}
