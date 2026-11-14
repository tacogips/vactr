//! Unit-generator, effect and bus natives (design 12.1, 12.5, 12.8.6,
//! design-music sections 2, 4-6).
//!
//! These natives build nodes, never audio: each returns a `Value::UGen`
//! tree that `ns::insts` lowers to an `InstDef` (an `inst` body) or a
//! `BusDef` (a `bus`/`master` body).
//!
//! Names shared with a pattern control, a signal or a visual (B2): `saw`,
//! `tri`, `lpf`, `hpf`, `delay`, `gain`, `pan`, `room`, `saturate` and
//! `range` keep their registration elsewhere; `collides` (called by the VM
//! before the native runs) sends a call to the DSP meaning when the subject
//! is a ugen, when a numeric argument appears inside an `inst` body, or
//! when there is no pattern subject inside a `bus` body. `bus` itself is
//! the routing control with a pattern subject and a definition with a
//! keyword and a block.

use std::rc::Rc;

use crate::dsp::build::{lower_bus_with_resources, ugen_named, Lowering, SVF_MODES, UGENS};
use crate::dsp::graph::{EffectKind, UGenInput, UGenKind, UGenNode, UGenSpec};
use crate::ns::namespace::Prelude;
use crate::ns::stage::StagedEffect;
use crate::pattern::combinators::control::control;
use crate::pattern::pat::{PParam, Pat, PatNode};
use crate::pattern::signal::Sig;
use crate::reader::span::{FileId, Span};
use crate::value::intern::{intern_kw, KwId};
use crate::value::value::Value;
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::natives::pattern::{out, pat};
use crate::vm::natives::{arg, type_err};
use crate::vm::vm::Vm;

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

/// DSP names registered by another native module (B2).
const SHARED: [&str; 9] = [
    "saw", "tri", "lpf", "hpf", "delay", "gain", "pan", "room", "saturate",
];

/// The names `collides` may send to the DSP meaning.
const COLLIDE: [&str; 8] = [
    "gain", "pan", "lpf", "hpf", "room", "delay", "saturate", "range",
];

pub(super) fn register(p: &mut Prelude) {
    for (name, _) in UGENS {
        if !SHARED.contains(name) {
            p.register(name, ugen);
        }
    }
    for kind in EffectKind::ALL {
        let name = kind.name();
        if !SHARED.contains(&name) && ugen_named(name).is_none() {
            p.register(name, effect);
        }
    }
    p.register("bus", bus);
    p.register("master", master);
    for m in SVF_MODES {
        p.register_value(m, Value::kw(m));
    }
}

fn own_name(cx: &NativeCx<'_>) -> Result<&'static str, Failure> {
    cx.ns
        .prelude()
        .native(cx.id)
        .map(|e| e.sig.name)
        .ok_or_else(|| type_err("unregistered DSP native"))
}

fn node(kind: UGenKind, args: Vec<(Option<KwId>, UGenInput)>) -> Rc<UGenNode> {
    Rc::new(UGenNode {
        kind,
        args: args.into_boxed_slice(),
    })
}

fn bus_input() -> Rc<UGenNode> {
    node(UGenKind::BusInput, Vec::new())
}

/// A named argument that arrived as a positional `[:key value]` pair.
fn as_pair(v: &Value) -> Option<(KwId, Value)> {
    match v {
        Value::List(l) if l.items.len() == 2 => match &l.items[0] {
            Value::Keyword(k) => Some((*k, l.items[1].clone())),
            _ => None,
        },
        _ => None,
    }
}

/// Positional arguments and named ones (keywords and pairs).
fn split(a: &[Value], kw: Kw<'_>) -> (Vec<Value>, Vec<(KwId, Value)>) {
    let mut pos = Vec::new();
    let mut named: Vec<(KwId, Value)> = kw.to_vec();
    for v in a {
        match as_pair(v) {
            Some(p) => named.push(p),
            None => pos.push(v.clone()),
        }
    }
    (pos, named)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn number(v: &Value) -> Option<f32> {
    match v {
        Value::Int(i) => Some(*i as f32),
        Value::Int64(i) => Some(*i as f32),
        Value::Float(x) => Some(*x),
        Value::Float64(x) => Some(*x as f32),
        Value::Ratio(r) => Some(r.to_f64() as f32),
        Value::Bool(b) => Some(f32::from(u8::from(*b))),
        _ => None,
    }
}

/// A ugen input from a value that needs no forcing.
fn plain(v: &Value) -> Option<UGenInput> {
    Some(match v {
        Value::UGen(n) => match &n.kind {
            UGenKind::Ugen(UGenSpec::Param(c)) if n.args.is_empty() => UGenInput::Param(*c),
            UGenKind::Ugen(UGenSpec::Const(x)) => UGenInput::Const(*x),
            _ => UGenInput::Node(Rc::clone(n)),
        },
        Value::Keyword(k) => UGenInput::Keyword(*k),
        Value::Signal(s) => UGenInput::Signal(Rc::clone(s)),
        other => UGenInput::Const(number(other)?),
    })
}

/// True for a native that builds DSP nodes (a bare effect name used as a
/// subject: `multiband-compressor > limiter`).
fn is_dsp_native(cx: &NativeCx<'_>, v: &Value) -> bool {
    let Value::Native(id) = v else {
        return false;
    };
    cx.ns.prelude().native(*id).is_some_and(|e| {
        let name = e.sig.name;
        ugen_named(name).is_some() || EffectKind::from_name(name).is_some()
    })
}

/// A ugen input: forces a var or block, calls a bare DSP native, and takes
/// a list of numbers as a realization-time constant.
fn input(cx: &mut NativeCx<'_>, v: &Value, what: &str) -> Result<UGenInput, Failure> {
    let mut v = v.clone();
    for _ in 0..8 {
        match v {
            Value::VarRef(_) | Value::Thunk(_) => v = cx.force(&v)?,
            Value::Native(_) if is_dsp_native(cx, &v) => v = cx.call(&v, Vec::new())?,
            _ => break,
        }
    }
    if let Value::List(l) = &v {
        let mut xs = Vec::with_capacity(l.items.len());
        for item in l.items.iter() {
            let item = match item {
                Value::VarRef(s) => s.get(),
                other => other.clone(),
            };
            xs.push(
                number(&item).ok_or_else(|| type_err(format!("{what} takes a list of numbers")))?,
            );
        }
        return Ok(UGenInput::List(xs.into()));
    }
    if let Value::Pattern(p) = &v {
        if let Some(sig) = signal_of(p) {
            return Ok(UGenInput::Signal(sig));
        }
    }
    plain(&v).ok_or_else(|| {
        type_err(format!(
            "{what} takes a number, unit generator, keyword or signal, got {}",
            kind_name(&v)
        ))
    })
}

/// A continuous pattern over a signal (`{range sine 0 1}`) as the signal a
/// control cell samples (12.8.6).
fn signal_of(p: &Pat) -> Option<Rc<Sig>> {
    let num = |x: &PParam| match x {
        PParam::Const(v) => number(v).map(f64::from),
        _ => None,
    };
    match &p.node {
        PatNode::Signal(s) => Some(Rc::clone(s)),
        PatNode::Range(inner, lo, hi) => {
            let s = signal_of(inner)?;
            Some(Rc::new(Sig::MapRange(s, num(lo)?, num(hi)?)))
        }
        _ => None,
    }
}

fn inputs(
    cx: &mut NativeCx<'_>,
    pos: &[Value],
    named: &[(KwId, Value)],
    what: &str,
) -> Result<Vec<(Option<KwId>, UGenInput)>, Failure> {
    let mut args = Vec::with_capacity(pos.len() + named.len());
    for v in pos {
        args.push((None, input(cx, v, what)?));
    }
    for (k, v) in named {
        args.push((Some(*k), input(cx, v, what)?));
    }
    Ok(args)
}

fn ugen_node(cx: &mut NativeCx<'_>, spec: UGenSpec, name: &str, a: &[Value], kw: Kw<'_>) -> R {
    let (pos, named) = split(a, kw);
    let args = inputs(cx, &pos, &named, &format!("`{name}`"))?;
    Ok(Value::UGen(node(UGenKind::Ugen(spec), args)))
}

/// An effect over its subject: the first positional argument when it is a
/// unit generator, the bus input inside a `bus` body.
fn effect_node(cx: &mut NativeCx<'_>, kind: EffectKind, a: &[Value], kw: Kw<'_>) -> R {
    let name = kind.name();
    let what = format!("`{name}`");
    let (mut pos, named) = split(a, kw);
    let first = pos.first().cloned();
    let subject = match first {
        Some(v @ Value::UGen(_)) => {
            pos.remove(0);
            input(cx, &v, &what)?
        }
        Some(v) if is_dsp_native(cx, &v) => {
            pos.remove(0);
            input(cx, &v, &what)?
        }
        _ if cx.vm.dsp.bus > 0 => UGenInput::Node(bus_input()),
        _ => {
            return Err(type_err(format!(
                "{what} needs an audio subject: write `x > {name}`"
            )))
        }
    };
    let mut args = vec![(None, subject)];
    args.extend(inputs(cx, &pos, &[], &what)?);
    let selector = intern_kw("sidechain");
    let mut selected = false;
    for (name, value) in &named {
        if *name != selector {
            args.push((Some(*name), input(cx, value, &what)?));
            continue;
        }
        if kind != EffectKind::Compressor || cx.vm.dsp.bus == 0 || cx.vm.dsp.inst > 0 {
            return Err(type_err(
                "sidechain is only valid on a bus-chain compressor",
            ));
        }
        if selected {
            return Err(type_err("duplicate sidechain selector"));
        }
        selected = true;
        let Value::Keyword(source) = value else {
            return Err(type_err("sidechain takes a constant bus keyword"));
        };
        if *source == intern_kw("master") {
            return Err(type_err("master cannot be a sidechain source"));
        }
        let reg = cx
            .vm
            .dsp
            .registry
            .as_ref()
            .ok_or_else(|| type_err("no bus registry"))?;
        let id = reg
            .borrow()
            .bus(*source)
            .map(|bus| bus.id)
            .ok_or_else(|| type_err("sidechain source must be a declared bus"))?;
        args.push((Some(*name), UGenInput::Sidechain(id)));
    }
    Ok(Value::UGen(node(UGenKind::Effect(kind), args)))
}

/// A ugen native; `bpf`/`comb` with no ugen subject inside a `bus` body
/// are the effects.
fn ugen(cx: &mut NativeCx<'_>, a: &[Value], kw: Kw<'_>) -> R {
    let name = own_name(cx)?;
    let spec = ugen_named(name).ok_or_else(|| type_err("unknown ugen"))?;
    if cx.vm.dsp.bus > 0 && !matches!(a.first(), Some(Value::UGen(_))) {
        if let Some(kind) = EffectKind::from_name(name) {
            return effect_node(cx, kind, a, kw);
        }
    }
    ugen_node(cx, spec, name, a, kw)
}

fn effect(cx: &mut NativeCx<'_>, a: &[Value], kw: Kw<'_>) -> R {
    let name = own_name(cx)?;
    let kind = EffectKind::from_name(name).ok_or_else(|| type_err("unknown effect"))?;
    effect_node(cx, kind, a, kw)
}

fn is_number(v: &Value) -> bool {
    matches!(
        v,
        Value::Int(_) | Value::Int64(_) | Value::Float(_) | Value::Float64(_) | Value::Ratio(_)
    )
}

fn pattern_like(v: &Value) -> bool {
    matches!(
        v,
        Value::Pattern(_) | Value::List(_) | Value::Range(_) | Value::Signal(_) | Value::Tex(_)
    )
}

/// True when a shared name means its DSP node here (B2).
#[must_use]
pub fn collides(vm: &Vm, name: &str, args: &[Value]) -> bool {
    if !COLLIDE.contains(&name) {
        return false;
    }
    let first = args.iter().find(|v| as_pair(v).is_none());
    if matches!(first, Some(Value::UGen(_))) {
        return true;
    }
    if name == "range" {
        return false;
    }
    (vm.dsp.inst > 0 && first.is_some_and(is_number))
        || (vm.dsp.bus > 0 && !first.is_some_and(pattern_like))
}

/// The DSP meaning of a shared name: `lpf`/`hpf`/`delay` are ugens in an
/// instrument and effects on a bus; the others are effects; `range` scales
/// a ugen.
///
/// # Errors
/// `type` for an input the catalog cannot take.
pub fn collision(cx: &mut NativeCx<'_>, name: &str, a: &[Value], kw: Kw<'_>) -> R {
    if name == "range" {
        return range(cx, a);
    }
    match ugen_named(name) {
        Some(spec) if cx.vm.dsp.bus == 0 => ugen_node(cx, spec, name, a, kw),
        _ => {
            let kind = EffectKind::from_name(name).ok_or_else(|| type_err("unknown effect"))?;
            effect_node(cx, kind, a, kw)
        }
    }
}

/// True when calling the signal `saw`/`tri` builds the ugen: inside an
/// `inst` body, or with a ugen argument.
#[must_use]
pub fn signal_call(vm: &Vm, sig: &Sig, args: &[Value]) -> bool {
    matches!(sig, Sig::Saw | Sig::Tri)
        && !args.is_empty()
        && (vm.dsp.inst > 0 || matches!(args.first(), Some(Value::UGen(_))))
}

/// `saw freq` / `tri freq` as ugens.
///
/// # Errors
/// `type` for an input the catalog cannot take.
pub fn signal_ugen(cx: &mut NativeCx<'_>, sig: &Sig, a: &[Value], kw: Kw<'_>) -> R {
    let (spec, name) = match sig {
        Sig::Tri => (UGenSpec::Tri, "tri"),
        _ => (UGenSpec::Saw, "saw"),
    };
    ugen_node(cx, spec, name, a, kw)
}

fn bin(spec: UGenSpec, x: UGenInput, y: UGenInput) -> UGenInput {
    UGenInput::Node(node(UGenKind::Ugen(spec), vec![(None, x), (None, y)]))
}

/// `+ - *` with a ugen operand build `Add`/`Mul` nodes (12.8.6); `None`
/// when no operand is a ugen.
#[must_use]
pub fn arith(op: char, args: &[Value]) -> Option<R> {
    if !args.iter().any(|v| matches!(v, Value::UGen(_))) {
        return None;
    }
    let mut ins = Vec::with_capacity(args.len());
    for v in args {
        match plain(v) {
            Some(i @ (UGenInput::Node(_) | UGenInput::Const(_) | UGenInput::Param(_))) => {
                ins.push(i)
            }
            Some(i @ UGenInput::Signal(_)) => ins.push(i),
            _ => {
                return Some(Err(type_err(format!(
                    "`{op}` on a unit generator takes numbers, unit generators or signals, got {}",
                    kind_name(v)
                ))))
            }
        }
    }
    let mut it = ins.into_iter();
    let first = it.next()?;
    let neg = |x: UGenInput| bin(UGenSpec::Mul, x, UGenInput::Const(-1.0));
    let mut acc = if op == '-' && args.len() == 1 {
        neg(first)
    } else {
        first
    };
    for x in it {
        acc = match op {
            '*' => bin(UGenSpec::Mul, acc, x),
            '-' => bin(UGenSpec::Add, acc, neg(x)),
            _ => bin(UGenSpec::Add, acc, x),
        };
    }
    Some(Ok(match acc {
        UGenInput::Node(n) => Value::UGen(n),
        other => Value::UGen(node(UGenKind::Ugen(UGenSpec::Add), vec![(None, other)])),
    }))
}

/// `range u lo hi` on a bipolar ugen: `lo + (u + 1) / 2 * (hi - lo)`.
fn range(cx: &mut NativeCx<'_>, a: &[Value]) -> R {
    let what = "`range`";
    let u = input(cx, &arg(a, 0), what)?;
    let lo = input(cx, &arg(a, 1), what)?;
    let hi = input(cx, &arg(a, 2), what)?;
    let unit = bin(UGenSpec::Add, u, UGenInput::Const(1.0));
    let span = bin(
        UGenSpec::Add,
        hi,
        bin(UGenSpec::Mul, lo.clone(), UGenInput::Const(-1.0)),
    );
    let half = bin(UGenSpec::Mul, span, UGenInput::Const(0.5));
    match bin(UGenSpec::Add, bin(UGenSpec::Mul, unit, half), lo) {
        UGenInput::Node(n) => Ok(Value::UGen(n)),
        _ => Err(type_err("`range` builds a node")),
    }
}

/// `subject > bus :name` is the routing control; `bus :name:` + block is
/// a bus definition (12.8.6).
fn bus(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    if let (Some(Value::Keyword(k)), Some(body), 2) = (a.first(), a.get(1), a.len()) {
        if matches!(body, Value::Thunk(_) | Value::UGen(_) | Value::Native(_)) {
            return define_bus(cx, Some(*k), body);
        }
    }
    out(cx, control(intern_kw("bus"), pat(a, 1)?, pat(a, 0)?, None))
}

/// `master:` + block: the root bus every bus feeds.
fn master(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    define_bus(cx, None, &arg(a, 0))
}

/// Evaluates a bus body with the bus input as its implicit subject, lowers
/// it, registers it and stages its `Install`.
fn define_bus(cx: &mut NativeCx<'_>, name: Option<KwId>, body: &Value) -> R {
    let label = if name.is_some() { "`bus`" } else { "`master`" };
    cx.vm.check_effect(label)?;
    let reg = cx.vm.dsp.registry.clone().ok_or_else(|| {
        Failure::new(
            FailCode::HostUnavailable,
            format!("{label} needs a session with an instrument registry"),
        )
    })?;
    cx.vm.dsp.bus += 1;
    let root = (|| {
        let v = cx.force(body)?;
        if is_dsp_native(cx, &v) {
            return cx.call(&v, Vec::new());
        }
        Ok(v)
    })();
    cx.vm.dsp.bus -= 1;
    // A failing body fails the definition (12.8.6).
    let root = root.map_err(|f| {
        Failure::new(
            FailCode::InstFailed,
            format!("{label} failed: {}", f.message),
        )
    })?;
    let root = match root {
        Value::UGen(n) => n,
        Value::Nil => bus_input(),
        other => {
            return Err(type_err(format!(
                "a {label} body is an effect chain, got {}",
                kind_name(&other)
            )))
        }
    };
    let span = cx.span.unwrap_or(Span::new(FileId::new(0), 0, 0));
    let mut r = reg.borrow_mut();
    let id = r.bus_id(name);
    let caps = r.caps;
    let mode = r.resource_capture_mode();
    let mut alloc = || r.alloc_cell();
    let lw = Lowering {
        caps: &caps,
        span,
        alloc: &mut alloc,
    };
    let lowered = lower_bus_with_resources(id, &root, lw, mode);
    let (def, extras) = match lowered {
        Ok(x) => x,
        Err(e) => {
            cx.vm.dsp.diags.extend(e.diag.map(|d| *d));
            return Err(Failure::new(
                FailCode::InstFailed,
                format!("{label} failed: {}", e.failure.message),
            ));
        }
    };
    cx.vm.dsp.diags.extend(extras.diags);
    let g = r.install_bus_with_resources(name, def, extras.signals, extras.resources);
    drop(r);
    cx.stage(StagedEffect::Install(g))?;
    Ok(Value::Nil)
}
