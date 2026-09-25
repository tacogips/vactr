//! The delay group (design-music 5 "delay", design-implementation 12.5,
//! 12.8.8): `delay`, `ping-pong`, `multitap`, `time-align`.
//!
//! Every kind keeps its history in one or two `prim::DelayLine`s carved
//! from the unit's `mem` region at install time and reads them with
//! `DelayLine::read` (linear-interpolated, so the smoothed `time`
//! parameter never zippers). Feedback paths always run through a
//! `OnePole` damping filter and a feedback gain kept below the crate's
//! stability bar, so output stays finite and bounded for any input.

use super::prim::{clampf, DelayLine, OnePole};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

/// The largest single delay line this group carves, in seconds.
const MAX_DELAY_S: f32 = 2.0;
/// The largest `time-align` offset, in seconds.
const MAX_ALIGN_S: f32 = 0.02;

const DELAY_PARAMS: &[ParamDef] = &[
    ParamDef::unit("time", 0.25, 0.0, MAX_DELAY_S, "s"),
    ParamDef::new("feedback", 0.4, 0.0, 0.95),
    ParamDef::unit("tone", 8000.0, 200.0, 18_000.0, "Hz"),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];

const MULTITAP_PARAMS: &[ParamDef] = &[
    ParamDef::unit("time-1", 0.125, 0.0, MAX_DELAY_S, "s"),
    ParamDef::new("gain-1", 0.8, 0.0, 1.0),
    ParamDef::unit("time-2", 0.25, 0.0, MAX_DELAY_S, "s"),
    ParamDef::new("gain-2", 0.6, 0.0, 1.0),
    ParamDef::unit("time-3", 0.375, 0.0, MAX_DELAY_S, "s"),
    ParamDef::new("gain-3", 0.4, 0.0, 1.0),
    ParamDef::unit("time-4", 0.5, 0.0, MAX_DELAY_S, "s"),
    ParamDef::new("gain-4", 0.2, 0.0, 1.0),
    ParamDef::new("feedback", 0.2, 0.0, 0.9),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];

const TIME_ALIGN_PARAMS: &[ParamDef] = &[
    ParamDef::unit("offset-l", 0.0, 0.0, 20.0, "ms"),
    ParamDef::unit("offset-r", 0.0, 0.0, 20.0, "ms"),
];

/// The named parameters of a delay-group kind.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Delay | K::PingPong => DELAY_PARAMS,
        K::Multitap => MULTITAP_PARAMS,
        K::TimeAlign => TIME_ALIGN_PARAMS,
        _ => &[],
    }
}

/// The preferred delay memory of a kind, in floats.
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    use EffectKind as K;
    let sr = sr.max(1.0);
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    match kind {
        K::Delay | K::PingPong | K::Multitap => (2.0 * MAX_DELAY_S * sr) as usize,
        K::TimeAlign => (2.0 * MAX_ALIGN_S * sr) as usize + 4,
        _ => 0,
    }
}

/// Carves two equally sized lines (L, R) out of `mem_len` floats.
fn carve_stereo(mem_len: usize) -> (DelayLine, DelayLine) {
    let mut cursor = 0usize;
    let half = mem_len / 2;
    let l = DelayLine::carve(&mut cursor, mem_len, half);
    let r = DelayLine::carve(&mut cursor, mem_len, mem_len - half);
    (l, r)
}

/// Sets up a zeroed unit for a delay-group kind.
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, _sr: f32) {
    use EffectKind as K;
    match kind {
        K::Delay | K::PingPong | K::Multitap | K::TimeAlign => {
            let (l, r) = carve_stereo(mem_len);
            st.dl[0] = l;
            st.dl[1] = r;
        }
        _ => {}
    }
}

/// A plain feedback delay (`delay`): each channel reads and writes its own
/// line, feedback lowpass-filtered by `tone`.
fn run_delay(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let time = p.first().copied().unwrap_or(0.25);
    let feedback = p.get(1).copied().unwrap_or(0.4);
    let tone = p.get(2).copied().unwrap_or(8000.0);
    let a = OnePole::coef(tone, sr);
    let d = (time * sr).max(1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let yl = st.dl[0].read(mem, d);
        let yr = st.dl[1].read(mem, d);
        let fbl = st.op[0].lp(yl, a) * feedback;
        let fbr = st.op[1].lp(yr, a) * feedback;
        st.dl[0].write(mem, xl + fbl);
        st.dl[1].write(mem, xr + fbr);
        l[i] = yl;
        r[i] = yr;
    }
}

/// A cross-feeding stereo bounce (`ping-pong`): each line is fed by the
/// OPPOSITE channel's input plus its own damped feedback, so energy
/// alternates sides on repeats.
fn run_ping_pong(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let time = p.first().copied().unwrap_or(0.25);
    let feedback = p.get(1).copied().unwrap_or(0.4);
    let tone = p.get(2).copied().unwrap_or(8000.0);
    let a = OnePole::coef(tone, sr);
    let d = (time * sr).max(1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let yl = st.dl[0].read(mem, d);
        let yr = st.dl[1].read(mem, d);
        let fbl = st.op[0].lp(yl, a) * feedback;
        let fbr = st.op[1].lp(yr, a) * feedback;
        st.dl[0].write(mem, xr + fbl);
        st.dl[1].write(mem, xl + fbr);
        l[i] = yl;
        r[i] = yr;
    }
}

/// Up to four taps read from one line per channel, summed and fed back
/// through the mean of the taps (so the loop gain never exceeds
/// `feedback`, independent of how the individual tap gains are set).
fn run_multitap(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let mut taps = [(0.125f32, 0.8f32); 4];
    for (k, tap) in taps.iter_mut().enumerate() {
        let base = k * 2;
        tap.0 = p.get(base).copied().unwrap_or(tap.0).max(0.0);
        tap.1 = p.get(base + 1).copied().unwrap_or(tap.1);
    }
    let feedback = p.get(8).copied().unwrap_or(0.2);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let mut outl = 0.0f32;
        let mut outr = 0.0f32;
        for (t, g) in taps {
            let d = (t * sr).max(1.0);
            outl += g * st.dl[0].read(mem, d);
            outr += g * st.dl[1].read(mem, d);
        }
        let fbl = (outl / 4.0) * feedback;
        let fbr = (outr / 4.0) * feedback;
        st.dl[0].write(mem, xl + fbl);
        st.dl[1].write(mem, xr + fbr);
        l[i] = outl;
        r[i] = outr;
    }
}

/// Independent short delays on each channel to correct arrival-time
/// mismatches. No `mix` parameter: the result is always fully wet.
fn run_time_align(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let dl_ms = p.first().copied().unwrap_or(0.0);
    let dr_ms = p.get(1).copied().unwrap_or(0.0);
    let dl_s = clampf(dl_ms, 0.0, 20.0) * 0.001 * sr;
    let dr_s = clampf(dr_ms, 0.0, 20.0) * 0.001 * sr;
    let n = l.len().min(r.len());
    for i in 0..n {
        st.dl[0].write(mem, l[i]);
        st.dl[1].write(mem, r[i]);
        l[i] = st.dl[0].read(mem, dl_s.max(1.0));
        r[i] = st.dl[1].read(mem, dr_s.max(1.0));
    }
}

/// Processes one stereo block for a delay-group kind, fully wet.
#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    use EffectKind as K;
    let sr = ctx.sr.max(1.0);
    match kind {
        K::Delay => run_delay(p, st, mem, l, r, sr),
        K::PingPong => run_ping_pong(p, st, mem, l, r, sr),
        K::Multitap => run_multitap(p, st, mem, l, r, sr),
        K::TimeAlign => run_time_align(p, st, mem, l, r, sr),
        _ => {}
    }
}
