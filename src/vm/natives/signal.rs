//! Signal natives (design 10.5, design-music.md section 3): the prelude
//! signal values and the signal constructors. A signal is queried at a
//! point; `cc`, `fft` and `amp` read the input cells (7.1.3).

use std::rc::Rc;

use crate::ns::namespace::Prelude;
use crate::pattern::build::midi_channel;
use crate::pattern::eval::{num_f64, HostSig};
use crate::pattern::signal::Sig;
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::Failure;
use crate::vm::natives::pattern::{named, now};
use crate::vm::natives::{arg, int_of, type_err};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

pub(super) fn register(p: &mut Prelude) {
    let values = [
        ("sine", Sig::Sine),
        ("saw", Sig::Saw),
        ("tri", Sig::Tri),
        ("square", Sig::Square),
        ("rand", Sig::Rand),
        ("perlin", Sig::Perlin),
        ("time", Sig::Time),
        ("beat", Sig::Beat),
        ("phase", Sig::Phase),
        ("cycle", Sig::Cycle),
        ("amp", Sig::Host(HostSig::Amp)),
    ];
    for (name, sig) in values {
        p.register_value(name, signal(sig));
    }
    p.register("irand", irand);
    p.register("fft", fft);
    p.register("cc", cc);
    p.register("lag", lag);
    p.register("map-range", map_range);
}

fn signal(s: Sig) -> Value {
    Value::Signal(Rc::new(s))
}

fn sig_arg(v: &Value, what: &str) -> Result<Rc<Sig>, Failure> {
    match v {
        Value::Signal(s) => Ok(Rc::clone(s)),
        other => Err(type_err(format!(
            "{what} expects a signal, got {}",
            kind_name(other)
        ))),
    }
}

/// A number argument, forced now (it shapes the signal node).
fn number_now(cx: &mut NativeCx<'_>, a: &[Value], k: usize, what: &str) -> Result<f64, Failure> {
    let v = now(cx, a, k)?;
    num_f64(&v).ok_or_else(|| type_err(format!("{what} expects a number, got {}", kind_name(&v))))
}

/// `irand 8`: a random integer in `0..8` per point.
fn irand(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let n = int_of(&now(cx, a, 0)?, "`irand`")?;
    if n <= 0 {
        return Err(type_err("`irand` expects a positive count"));
    }
    Ok(signal(Sig::IRand(n)))
}

/// `fft n`: one analysis band.
fn fft(_: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let n = int_of(&arg(a, 0), "`fft`")?;
    let band = u16::try_from(n).map_err(|_| type_err("`fft` band out of range"))?;
    Ok(signal(Sig::Host(HostSig::Fft(band))))
}

/// `cc 74 channel: 2`: a MIDI controller cell, 0..1 (channel 1 by default).
fn cc(cx: &mut NativeCx<'_>, a: &[Value], kw: Kw<'_>) -> R {
    let n = int_of(&arg(a, 0), "`cc`")?;
    let controller = u8::try_from(n)
        .ok()
        .filter(|c| *c < 128)
        .ok_or_else(|| type_err("a MIDI controller is 0..127"))?;
    let channel = match named(kw, "channel") {
        Some(v) => midi_channel(int_of(&cx.deep(v)?, "`channel:`")?)?,
        None => 1,
    };
    Ok(signal(Sig::Cc {
        controller,
        channel,
    }))
}

/// `lag s seconds`.
fn lag(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let s = sig_arg(&arg(a, 0), "`lag`")?;
    let secs = number_now(cx, a, 1, "`lag`")?;
    Ok(signal(Sig::Lag(s, secs)))
}

/// `map-range s lo hi`: a 0..1 signal onto `lo..hi`. The five-argument
/// form (an input range) has no signal node in this issue.
fn map_range(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let s = sig_arg(&arg(a, 0), "`map-range`")?;
    if a.len() != 3 {
        return Err(Failure::new(
            crate::vm::fail::FailCode::Arity,
            "`map-range` takes a signal, lo and hi",
        ));
    }
    let lo = number_now(cx, a, 1, "`map-range`")?;
    let hi = number_now(cx, a, 2, "`map-range`")?;
    Ok(signal(Sig::MapRange(s, lo, hi)))
}
