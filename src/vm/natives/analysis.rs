//! Self-analysis natives (design 14.5.9, the 12.3 amendment): `scope`,
//! `spectrum`, `capture`, `rms`, `peak`, and the offline form of `render`
//! (`tex.rs` routes a number subject here).
//!
//! The natives reach the host capabilities and the live tap reader through
//! the source loader (`SourceLoader::analysis`, via the VM's `LoaderHost`),
//! so there is no VM field for them. Without that context (a bare
//! `Evaluator`, the spec fixture runner) every one of them fails
//! `host-unavailable`, as under `NoopHost`.
//!
//! Sources: `:master` and the keyword of a defined bus. The slot sources
//! `:d1`..`:d9` and the input `:in` are `beyond-capability` ("not available
//! on this host", S2). Analysis values are copies: nothing here feeds audio
//! back into a bus.

use std::rc::Rc;

use crate::dsp::offline::{self, OFFLINE_RATE};
use crate::host::caps::{AnalysisCx, TapSrc};
use crate::ns::load::LoaderHost;
use crate::ns::namespace::Prelude;
use crate::ns::stage::StagedEffect;
use crate::pattern::eval::num_ratio;
use crate::reader::span::{FileId, Span};
use crate::value::intern::{intern_kw, name_of_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::sample::SampleBuf;
use crate::value::value::{Sound, Value};
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::natives::{arg, dsp, int_of, list_of, type_err};

type Kw<'a> = &'a [(KwId, Value)];
type R = Result<Value, Failure>;

/// The longest live tap snapshot, mono frames (the history ring length).
pub const MAX_SCOPE: i64 = 8192;
/// The default and the range of `spectrum`'s `bins:`.
const DEFAULT_BINS: i64 = 64;
const MIN_BINS: i64 = 8;
const MAX_BINS: i64 = 2048;

pub(super) fn register(p: &mut Prelude) {
    p.register("scope", scope);
    p.register("spectrum", spectrum);
    p.register("capture", capture);
    p.register("rms", rms);
    p.register("peak", peak);
}

/// A failure at the call site.
fn at_call(cx: &NativeCx<'_>, mut f: Failure) -> Failure {
    f.origin.span = f.origin.span.or(cx.span);
    f
}

fn no_context() -> Failure {
    Failure::new(
        FailCode::HostUnavailable,
        "self-analysis is not available: no audio host is attached",
    )
}

fn beyond(what: &str) -> Failure {
    Failure::new(
        FailCode::BeyondCapability,
        format!("{what} is not available on this host"),
    )
}

/// Runs `f` with the analysis context, or fails `host-unavailable`.
fn with_cx<T>(
    cx: &mut NativeCx<'_>,
    f: impl FnOnce(&mut AnalysisCx) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let Some(mut host) = cx.vm.take_host() else {
        return Err(no_context());
    };
    let r = match host
        .downcast_mut::<LoaderHost>()
        .and_then(|h| h.0.analysis())
    {
        Some(a) => f(a),
        None => Err(no_context()),
    };
    cx.vm.set_host(Some(host));
    r
}

/// A live source: `:master` or a defined bus.
fn source(cx: &NativeCx<'_>, v: &Value) -> Result<TapSrc, Failure> {
    let Value::Keyword(k) = v else {
        return Err(type_err(format!(
            "an analysis source is a bus keyword, got {}",
            kind_name(v)
        )));
    };
    let name = name_of_kw(*k);
    match &*name {
        "master" => return Ok(TapSrc::Master),
        "in" | "d1" | "d2" | "d3" | "d4" | "d5" | "d6" | "d7" | "d8" | "d9" => {
            return Err(beyond(&format!("a tap of `:{name}`")))
        }
        _ => {}
    }
    let defined = cx
        .vm
        .dsp
        .registry
        .as_ref()
        .is_some_and(|r| r.borrow().bus(*k).is_some());
    if defined {
        Ok(TapSrc::Bus(*k))
    } else {
        Err(type_err(format!("`:{name}` is not a defined bus")))
    }
}

/// The buffer of a `sound` value, if it is one.
fn buffer(v: &Value) -> Option<Rc<SampleBuf>> {
    match v {
        Value::Sound(s) => match &**s {
            Sound::Buffer(b) => Some(Rc::clone(b)),
            _ => None,
        },
        _ => None,
    }
}

fn buffer_arg(v: &Value, what: &str) -> Result<Rc<SampleBuf>, Failure> {
    buffer(v).ok_or_else(|| {
        type_err(format!(
            "{what} takes a sound buffer (from `capture` or `render`), got {}",
            kind_name(v)
        ))
    })
}

fn floats(xs: &[f32]) -> Value {
    list_of(xs.iter().map(|&x| Value::Float(x)).collect())
}

/// A positive cycle count.
fn cycles_of(v: &Value, what: &str) -> Result<Ratio64, Failure> {
    match num_ratio(v) {
        Some(c) if c > Ratio64::ZERO => Ok(c),
        _ => Err(type_err(format!(
            "{what} takes a positive number of cycles, got {}",
            kind_name(v)
        ))),
    }
}

/// `scope :src n` (the last n mono frames of a live tap) or `scope buf n`
/// (the first n mono frames of a buffer).
fn scope(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let subject = arg(a, 0);
    let n = int_of(&arg(a, 1), "`scope`")?;
    if !(1..=MAX_SCOPE).contains(&n) {
        return Err(type_err(format!(
            "`scope` takes 1..{MAX_SCOPE} frames, got {n}"
        )));
    }
    let n = usize::try_from(n).unwrap_or(0);
    let r = with_cx(cx, |_| Ok(()));
    r.map_err(|f| at_call(cx, f))?;
    if let Some(b) = buffer(&subject) {
        let mono = b.mono_frames().map_err(|f| at_call(cx, f))?;
        return Ok(floats(&offline::scope(&mono, n)));
    }
    let src = source(cx, &subject).map_err(|f| at_call(cx, f))?;
    let frames = snapshot(cx, &src, n)?;
    Ok(floats(&frames))
}

/// The last `n` mono frames of a live source.
fn snapshot(cx: &mut NativeCx<'_>, src: &TapSrc, n: usize) -> Result<Vec<f32>, Failure> {
    let mut out = Vec::with_capacity(n);
    let r = with_cx(cx, |a| match a.taps.as_mut() {
        Some(t) => t.snapshot(src, n, &mut out),
        None => Err(Failure::new(
            FailCode::HostUnavailable,
            "audio taps are not available on this host",
        )),
    });
    r.map_err(|f| at_call(cx, f))?;
    Ok(out)
}

/// `spectrum :src bins: b` (one FFT frame of a live tap), `spectrum buf
/// bins: b` (averaged over the buffer); any other subject is the analyzer
/// unit (12.8.6).
fn spectrum(cx: &mut NativeCx<'_>, a: &[Value], kw: Kw<'_>) -> R {
    let subject = a.first().cloned();
    let tap = matches!(subject, Some(Value::Keyword(_)));
    let buf = subject.as_ref().and_then(buffer);
    if !tap && buf.is_none() {
        return dsp::collision(cx, "spectrum", a, kw);
    }
    let bins_kw = intern_kw("bins");
    let mut bins = DEFAULT_BINS;
    for (k, v) in kw {
        if *k != bins_kw {
            return Err(type_err(format!(
                "`spectrum` of a source has no named argument `{}:`",
                name_of_kw(*k)
            )));
        }
        bins = int_of(v, "`spectrum` `bins:`")?;
    }
    let pow2 = u64::try_from(bins).is_ok_and(u64::is_power_of_two);
    if !pow2 || !(MIN_BINS..=MAX_BINS).contains(&bins) {
        return Err(at_call(
            cx,
            type_err(format!(
                "`spectrum` `bins:` is a power of two in {MIN_BINS}..{MAX_BINS}, got {bins}"
            )),
        ));
    }
    let bins = usize::try_from(bins).unwrap_or(64);
    let r = with_cx(cx, |_| Ok(()));
    r.map_err(|f| at_call(cx, f))?;
    if let Some(b) = buf {
        let mono = b.mono_frames().map_err(|f| at_call(cx, f))?;
        return Ok(floats(&offline::spectrum(&mono, bins)));
    }
    let src = source(cx, &subject.unwrap_or(Value::Nil)).map_err(|f| at_call(cx, f))?;
    let frames = snapshot(cx, &src, 2 * bins)?;
    Ok(floats(&offline::spectrum(&frames, bins)))
}

/// `capture :src cycles`: a `Pending` buffer that records the next `cycles`
/// cycles of `src` from the next cycle boundary (S1). The runtime arms it
/// and bounds it by `max_capture_seconds` at the current tempo.
fn capture(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let r = with_cx(cx, |_| Ok(()));
    r.map_err(|f| at_call(cx, f))?;
    let src = source(cx, &arg(a, 0)).map_err(|f| at_call(cx, f))?;
    let cycles = cycles_of(&arg(a, 1), "`capture`").map_err(|f| at_call(cx, f))?;
    // The rate is the audio host's, set when the capture completes.
    let buf = SampleBuf::pending(0);
    cx.stage(StagedEffect::Capture {
        buf: Rc::clone(&buf),
        src,
        cycles,
        origin: origin(cx),
    })?;
    Ok(Value::Sound(Rc::new(Sound::Buffer(buf))))
}

/// `render cycles`: an offline render of the current bindings into a
/// `Pending` buffer; `beyond-capability` at the call on a host without
/// offline render (the browser, builds without `host-native`).
pub(super) fn render(cx: &mut NativeCx<'_>, cycles: &Value) -> R {
    let able = with_cx(cx, |a| Ok(a.caps.offline_render));
    let able = able.map_err(|f| at_call(cx, f))?;
    if !able {
        return Err(at_call(cx, beyond("offline render")));
    }
    let cycles = cycles_of(cycles, "`render`").map_err(|f| at_call(cx, f))?;
    let buf = SampleBuf::pending(OFFLINE_RATE);
    cx.stage(StagedEffect::Render {
        buf: Rc::clone(&buf),
        cycles,
        origin: origin(cx),
    })?;
    Ok(Value::Sound(Rc::new(Sound::Buffer(buf))))
}

fn origin(cx: &NativeCx<'_>) -> Span {
    cx.span.unwrap_or(Span::new(FileId::CONSOLE, 0, 0))
}

/// `rms buf`: the root mean square over both channels.
fn rms(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let frames = ready(cx, &arg(a, 0), "`rms`")?;
    Ok(Value::Float(offline::rms(&frames)))
}

/// `peak buf`: the absolute peak over both channels.
fn peak(cx: &mut NativeCx<'_>, a: &[Value], _: Kw<'_>) -> R {
    let frames = ready(cx, &arg(a, 0), "`peak`")?;
    Ok(Value::Float(offline::peak(&frames)))
}

fn ready(cx: &mut NativeCx<'_>, v: &Value, what: &str) -> Result<Rc<[f32]>, Failure> {
    let r = with_cx(cx, |_| Ok(()));
    r.map_err(|f| at_call(cx, f))?;
    let b = buffer_arg(v, what).map_err(|f| at_call(cx, f))?;
    b.ready_frames().map_err(|f| at_call(cx, f))
}
