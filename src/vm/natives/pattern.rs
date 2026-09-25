//! Pattern natives (design 10.1, design-music.md sections 1-3 and 7): thin
//! wrappers over the ME-PATTERN builders, plus the coercions every domain
//! native shares.
//!
//! A pattern subject arrives forced (`Value` mask). A `Late` parameter
//! arrives as written: a `var` is a `VarRef` and becomes `PParam::Late`, a
//! block stays a thunk and becomes `PParam::Fn`, so both are read per query
//! (5.5). An `Fn` parameter (a transform) is kept as the callable value.

use std::rc::Rc;

use crate::ns::namespace::Prelude;
use crate::pattern::build::{alt as alt_steps, param_of, pattern_of};
use crate::pattern::combinators::input::midi_notes as midi_notes_node;
use crate::pattern::combinators::{pattern_value, random, region, structure, time};
use crate::pattern::pat::{PParam, Pat, SliceCuts};
use crate::value::intern::{name_of_kw, KwId};
use crate::value::value::Value;
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::Failure;
use crate::vm::natives::{arg, int_of, type_err};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    p.register("fast", fast);
    p.register("slow", slow);
    p.register("hurry", hurry);
    p.register("rev", rev);
    p.register("every", every);
    p.register("whenmod", whenmod);
    p.register("sometimes", sometimes);
    p.register("rarely", rarely);
    p.register("often", often);
    p.register("sometimes-by", sometimes_by);
    p.register("degrade-by", degrade_by);
    p.register("superimpose", superimpose);
    p.register("off", off);
    p.register("jux", jux);
    p.register("iter", iter);
    p.register("ply", ply);
    p.register("chunk", chunk);
    p.register("chop", chop);
    p.register("striate", striate);
    p.register("slice", slice);
    p.register("splice", splice);
    p.register("loop-at", loop_at);
    p.register("fit", fit);
    p.register("grid", grid);
    p.register("alt", alt);
    p.register("choose", choose);
    p.register("maybe", maybe);
    p.register("euclid", euclid);
    p.register("hold", hold);
    p.register("stack", stack);
    p.register("cat", cat);
    p.register("fastcat", fastcat);
    p.register("segment", segment);
    p.register("midi-notes", midi_notes);
}

/// The pattern for argument `k` (a list is one cycle of steps, 10.1).
pub(crate) fn pat(args: &[Value], k: usize) -> Result<Rc<Pat>, Failure> {
    pattern_of(&arg(args, k), None)
}

/// The parameter for argument `k`.
pub(crate) fn param(args: &[Value], k: usize) -> Result<PParam, Failure> {
    param_of(&arg(args, k))
}

/// A pattern result, spanned at the call.
pub(crate) fn out(cx: &NativeCx<'_>, p: Pat) -> R {
    Ok(pattern_value(p.with_span(cx.span)))
}

/// Forces a `Late` argument now (a keyword or count that shapes the node),
/// recording the eager read.
pub(crate) fn now(cx: &mut NativeCx<'_>, args: &[Value], k: usize) -> R {
    cx.force(&arg(args, k))
}

/// A keyword argument, forced.
pub(crate) fn keyword_now(
    cx: &mut NativeCx<'_>,
    args: &[Value],
    k: usize,
    what: &str,
) -> Result<KwId, Failure> {
    match now(cx, args, k)? {
        Value::Keyword(k) => Ok(k),
        other => Err(type_err(format!(
            "{what} expects a keyword, got {}",
            kind_name(&other)
        ))),
    }
}

/// The named argument `name`, when given.
pub(crate) fn named<'a>(kw: Kw<'a>, name: &str) -> Option<&'a Value> {
    kw.iter()
        .find(|(k, _)| *name_of_kw(*k) == *name)
        .map(|(_, v)| v)
}

/// The patterns of a list argument (`stack [..]`, `cat [..]`).
fn pats_of_list(v: &Value, what: &str) -> Result<Vec<Pat>, Failure> {
    let Value::List(l) = v else {
        return Err(type_err(format!(
            "{what} expects a list of patterns, got {}",
            kind_name(v)
        )));
    };
    l.items
        .iter()
        .map(|x| pattern_of(x, None).map(|p| (*p).clone()))
        .collect()
}

fn fast(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::fast(pat(a, 0)?, param(a, 1)?, None))
}

fn slow(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::slow(pat(a, 0)?, param(a, 1)?, None))
}

fn hurry(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::hurry(pat(a, 0)?, param(a, 1)?, None))
}

fn rev(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::rev(pat(a, 0)?, None))
}

fn every(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::every(pat(a, 0)?, param(a, 1)?, arg(a, 2), None))
}

fn whenmod(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let p = time::whenmod(pat(a, 0)?, param(a, 1)?, param(a, 2)?, arg(a, 3), None);
    out(cx, p)
}

fn sometimes(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, random::sometimes(pat(a, 0)?, arg(a, 1), None))
}

fn rarely(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, random::rarely(pat(a, 0)?, arg(a, 1), None))
}

fn often(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, random::often(pat(a, 0)?, arg(a, 1), None))
}

fn sometimes_by(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        random::sometimes_by(pat(a, 0)?, param(a, 1)?, arg(a, 2), None),
    )
}

fn degrade_by(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, random::degrade_by(pat(a, 0)?, param(a, 1)?, None))
}

fn superimpose(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, structure::superimpose(pat(a, 0)?, arg(a, 1), None))
}

fn off(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        structure::off(pat(a, 0)?, param(a, 1)?, arg(a, 2), None),
    )
}

fn jux(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, structure::jux(pat(a, 0)?, arg(a, 1), None))
}

fn iter(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::iter(pat(a, 0)?, param(a, 1)?, None))
}

fn ply(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, structure::ply(pat(a, 0)?, param(a, 1)?, None))
}

fn chunk(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::chunk(pat(a, 0)?, param(a, 1)?, arg(a, 2), None))
}

fn chop(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, region::chop(pat(a, 0)?, param(a, 1)?, None))
}

fn striate(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, region::striate(pat(a, 0)?, param(a, 1)?, None))
}

/// `8` is eight equal slices; a list is manual slice points in `[0, 1]`.
fn cuts(v: &Value) -> Result<SliceCuts, Failure> {
    match v {
        Value::List(l) => Ok(SliceCuts::Manual(
            l.items
                .iter()
                .map(param_of)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        )),
        other => Ok(SliceCuts::Equal(param_of(other)?)),
    }
}

fn slice(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        region::slice(pat(a, 0)?, cuts(&arg(a, 1))?, pat(a, 2)?, None),
    )
}

fn splice(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        region::splice(pat(a, 0)?, cuts(&arg(a, 1))?, pat(a, 2)?, None),
    )
}

fn loop_at(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, region::loop_at(pat(a, 0)?, param(a, 1)?, None))
}

fn fit(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, region::fit(pat(a, 0)?, None))
}

fn grid(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, structure::grid(pat(a, 0)?, pat(a, 1)?, None))
}

fn alt(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, alt_steps(a, None)?)
}

fn choose(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let items = a
        .iter()
        .map(|v| pattern_of(v, None).map(|p| (*p).clone()))
        .collect::<Result<Vec<_>, _>>()?;
    out(cx, random::choose(items, None))
}

fn maybe(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let prob = match a.get(1) {
        Some(v) => Some(param_of(v)?),
        None => None,
    };
    out(cx, random::maybe(pat(a, 0)?, prob, None))
}

fn euclid(cx: &mut NativeCx<'_>, a: &[Value], kw: Kw<'_>) -> R {
    let rot = match named(kw, "rotation") {
        Some(v) => param_of(v)?,
        None => PParam::int(0),
    };
    let p = structure::euclid(pat(a, 0)?, param(a, 1)?, param(a, 2)?, rot, None);
    out(cx, p)
}

fn hold(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, structure::hold(pat(a, 0)?, param(a, 1)?, None))
}

fn stack(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        structure::stack(pats_of_list(&arg(a, 0), "`stack`")?, None),
    )
}

fn cat(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, structure::cat(pats_of_list(&arg(a, 0), "`cat`")?, None))
}

fn fastcat(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(
        cx,
        structure::fastcat(pats_of_list(&arg(a, 0), "`fastcat`")?, None),
    )
}

fn segment(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    out(cx, time::segment(pat(a, 0)?, param(a, 1)?, None))
}

/// `s :pluck > midi-notes channel: 1`: live note input after `s` (11.7).
fn midi_notes(cx: &mut NativeCx<'_>, a: &[Value], kw: Kw<'_>) -> R {
    let channel = match named(kw, "channel") {
        Some(v) => Some(crate::pattern::build::midi_channel(int_of(
            &cx.deep(v)?,
            "`channel:`",
        )?)?),
        None => None,
    };
    out(cx, midi_notes_node(pat(a, 0)?, channel, None))
}

/// The bind-time input-lane walk (11.7) over a form's staged slot binds: a
/// bound pattern whose `midi-notes` lane has a re-timing or structural
/// operator is rejected with `input-lane-operator`. The caller fails the
/// form, so nothing is staged and the previous binding stays. The failure
/// code is `type`, the closest listed code (7.1.6 has no lane `FailCode`).
#[must_use]
pub fn reject_input_lanes(
    effects: &[crate::ns::stage::StagedEffect],
    form: crate::reader::span::Span,
) -> Option<(crate::types::diag::Diagnostic, Failure)> {
    use crate::ns::stage::StagedEffect;
    use crate::pattern::combinators::input::input_lane_walk;
    for e in effects {
        let StagedEffect::SlotBind {
            value: Value::Pattern(p),
            ..
        } = e
        else {
            continue;
        };
        if let Err(err) = input_lane_walk(p) {
            let d = crate::types::diag::Diagnostic::error(
                err.code,
                err.span.unwrap_or(form),
                err.message.clone(),
            );
            return Some((d, type_err(err.message)));
        }
    }
    None
}
