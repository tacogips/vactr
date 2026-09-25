//! Spatial effects (design 12.5, 12.8.8; catalog design-music.md section
//! 5): `width`, `balance`, `multiband-balance`, `ms`, `crossfeed`,
//! `crosstalk-cancel`, `phase-select-eq`, `spatial-map`, `pan`, `matrix`.
//!
//! None of these kinds have a `mix` parameter (the catalog list for this
//! group never names one): a spatial kind redefines the stereo image
//! itself rather than blending a wet path with a dry one, so it always
//! runs fully applied when used. Kinds that need biquad coefficients
//! (`multiband-balance`, `crossfeed`, `crosstalk-cancel`,
//! `phase-select-eq`) recompute them once per block, before the
//! per-sample loop, never per sample.
//!
//! **Approximations (documented per 12.8.8 B4):**
//! - `crosstalk-cancel`: a fixed single-tap delayed, inverted, low-passed
//!   crossfeed subtraction, not HRTF-based cancellation.
//! - `spatial-map`: a mid/side split plus a short Schroeder-allpass
//!   decorrelation tap standing in for `direct`/`diffuse`/`residual`,
//!   not the reference upmixer.

use super::prim::{allpass, db_to_gain, pan_gains, DelayLine, Shape};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

const WIDTH_PARAMS: &[ParamDef] = &[ParamDef::new("width", 1.0, 0.0, 2.0)];
const BALANCE_PARAMS: &[ParamDef] = &[ParamDef::new("balance", 0.0, -1.0, 1.0)];
const MULTIBAND_BALANCE_PARAMS: &[ParamDef] = &[
    ParamDef::unit("low-freq", 250.0, 40.0, 2000.0, "Hz"),
    ParamDef::unit("high-freq", 3000.0, 500.0, 12_000.0, "Hz"),
    ParamDef::new("low-balance", 0.0, -1.0, 1.0),
    ParamDef::new("mid-balance", 0.0, -1.0, 1.0),
    ParamDef::new("high-balance", 0.0, -1.0, 1.0),
];
const MS_PARAMS: &[ParamDef] = &[
    ParamDef::new("mode", 0.0, 0.0, 1.0),
    ParamDef::unit("mid-gain", 0.0, -24.0, 24.0, "dB"),
    ParamDef::unit("side-gain", 0.0, -24.0, 24.0, "dB"),
];
const CROSSFEED_PARAMS: &[ParamDef] = &[
    ParamDef::new("amount", 0.3, 0.0, 1.0),
    ParamDef::unit("freq", 700.0, 200.0, 2000.0, "Hz"),
];
const CROSSTALK_CANCEL_PARAMS: &[ParamDef] = &[
    ParamDef::new("amount", 0.5, 0.0, 1.0),
    ParamDef::unit("delay", 0.3, 0.1, 1.5, "ms"),
];
const PHASE_SELECT_EQ_PARAMS: &[ParamDef] = &[
    ParamDef::unit("freq", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 0.7, 0.1, 10.0),
    ParamDef::new("flip", 0.0, 0.0, 1.0),
];
const SPATIAL_MAP_PARAMS: &[ParamDef] = &[
    ParamDef::new("direct", 0.7, 0.0, 1.0),
    ParamDef::new("diffuse", 0.5, 0.0, 1.0),
    ParamDef::new("residual", 0.3, 0.0, 1.0),
];
const PAN_PARAMS: &[ParamDef] = &[ParamDef::new("pan", 0.5, 0.0, 1.0)];
const MATRIX_PARAMS: &[ParamDef] = &[
    ParamDef::new("ll", 1.0, -1.0, 1.0),
    ParamDef::new("lr", 0.0, -1.0, 1.0),
    ParamDef::new("rl", 0.0, -1.0, 1.0),
    ParamDef::new("rr", 1.0, -1.0, 1.0),
];

/// The named parameters of a spatial kind, in catalog order.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind::{
        Balance, Crossfeed, CrosstalkCancel, Matrix, Ms, MultibandBalance, Pan, PhaseSelectEq,
        SpatialMap, Width,
    };
    match kind {
        Width => WIDTH_PARAMS,
        Balance => BALANCE_PARAMS,
        MultibandBalance => MULTIBAND_BALANCE_PARAMS,
        Ms => MS_PARAMS,
        Crossfeed => CROSSFEED_PARAMS,
        CrosstalkCancel => CROSSTALK_CANCEL_PARAMS,
        PhaseSelectEq => PHASE_SELECT_EQ_PARAMS,
        SpatialMap => SPATIAL_MAP_PARAMS,
        Pan => PAN_PARAMS,
        Matrix => MATRIX_PARAMS,
        _ => &[],
    }
}

/// `ms` converted to samples at `sr` (never negative).
fn ms_to_samples(ms: f32, sr: f32) -> f32 {
    ms.max(0.0) * sr.max(1.0) / 1000.0
}

/// Carves two equal delay lines (left, right) out of `mem_len` floats,
/// each wanting `ms` milliseconds of capacity.
fn carve_stereo(mem_len: usize, ms: f32, sr: f32) -> (DelayLine, DelayLine) {
    let want = ms_to_samples(ms, sr).ceil().max(1.0) as usize;
    let mut cursor = 0usize;
    let a = DelayLine::carve(&mut cursor, mem_len, want);
    let b = DelayLine::carve(&mut cursor, mem_len, want);
    (a, b)
}

/// Parameter `i`, or `default` if `p` is shorter than expected.
fn pv(p: &[f32], i: usize, default: f32) -> f32 {
    p.get(i).copied().unwrap_or(default)
}

/// The (left, right) gain pair of a `-1..1` balance-style control: `0`
/// leaves both channels at unity, `+1` fades the left channel out and
/// `-1` fades the right channel out.
fn balance_gains(b: f32) -> (f32, f32) {
    let b = b.clamp(-1.0, 1.0);
    (
        (1.0 - b.max(0.0)).clamp(0.0, 1.0),
        (1.0 + b.min(0.0)).clamp(0.0, 1.0),
    )
}

/// The preferred delay memory of a kind, in floats (the caller clamps it
/// to what the unit's region holds).
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    match kind {
        EffectKind::CrosstalkCancel => 2 * (ms_to_samples(3.0, sr).ceil() as usize),
        EffectKind::SpatialMap => 2 * (ms_to_samples(10.0, sr).ceil() as usize),
        _ => 0,
    }
}

/// Sets up a zeroed spatial unit.
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, sr: f32) {
    match kind {
        EffectKind::CrosstalkCancel => {
            let (a, b) = carve_stereo(mem_len, 3.0, sr);
            st.dl[0] = a;
            st.dl[1] = b;
        }
        EffectKind::SpatialMap => {
            let (a, b) = carve_stereo(mem_len, 10.0, sr);
            st.dl[0] = a;
            st.dl[1] = b;
        }
        _ => {}
    }
}

/// Processes one stereo block, fully wet (there is no `mix` parameter in
/// this group; see the module doc comment).
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
    let sr = ctx.sr;
    match kind {
        EffectKind::Width => width(p, l, r),
        EffectKind::Balance => balance(p, l, r),
        EffectKind::MultibandBalance => multiband_balance(p, st, l, r, sr),
        EffectKind::Ms => ms(p, l, r),
        EffectKind::Crossfeed => crossfeed(p, st, l, r, sr),
        EffectKind::CrosstalkCancel => crosstalk_cancel(p, st, mem, l, r, sr),
        EffectKind::PhaseSelectEq => phase_select_eq(p, st, l, r, sr),
        EffectKind::SpatialMap => spatial_map(p, st, mem, l, r),
        EffectKind::Pan => pan(p, l, r),
        EffectKind::Matrix => matrix(p, l, r),
        _ => {}
    }
}

fn width(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let w = pv(p, 0, 1.0).clamp(0.0, 2.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let m = (l[i] + r[i]) * 0.5;
        let s = (l[i] - r[i]) * 0.5 * w;
        l[i] = m + s;
        r[i] = m - s;
    }
}

fn balance(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let (gl, gr) = balance_gains(pv(p, 0, 0.0));
    let n = l.len().min(r.len());
    for i in 0..n {
        l[i] *= gl;
        r[i] *= gr;
    }
}

/// A crude 3-band split (lowpass at `low-freq`, highpass at `high-freq`,
/// mid = input minus both) with an independent `-1..1` balance per band.
fn multiband_balance(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let lo = pv(p, 0, 250.0).clamp(20.0, 0.45 * sr.max(200.0));
    let hi = pv(p, 1, 3000.0).clamp(lo + 1.0, 0.45 * sr.max(200.0));
    let (glo_l, glo_r) = balance_gains(pv(p, 2, 0.0));
    let (gmi_l, gmi_r) = balance_gains(pv(p, 3, 0.0));
    let (ghi_l, ghi_r) = balance_gains(pv(p, 4, 0.0));
    st.bq[0].set(Shape::Lowpass, lo, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Highpass, hi, 0.707, 0.0, sr);
    st.bq[2].set(Shape::Lowpass, lo, 0.707, 0.0, sr);
    st.bq[3].set(Shape::Highpass, hi, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let low_l = st.bq[0].run(xl);
        let high_l = st.bq[1].run(xl);
        let mid_l = xl - low_l - high_l;
        let low_r = st.bq[2].run(xr);
        let high_r = st.bq[3].run(xr);
        let mid_r = xr - low_r - high_r;
        l[i] = low_l * glo_l + mid_l * gmi_l + high_l * ghi_l;
        r[i] = low_r * glo_r + mid_r * gmi_r + high_r * ghi_r;
    }
}

fn ms(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let decode = pv(p, 0, 0.0) >= 0.5;
    let mg = db_to_gain(pv(p, 1, 0.0));
    let sg = db_to_gain(pv(p, 2, 0.0));
    let n = l.len().min(r.len());
    for i in 0..n {
        if decode {
            let m = l[i] * mg;
            let s = r[i] * sg;
            l[i] = m + s;
            r[i] = m - s;
        } else {
            let m = (l[i] + r[i]) * 0.5;
            let s = (l[i] - r[i]) * 0.5;
            l[i] = m * mg;
            r[i] = s * sg;
        }
    }
}

fn crossfeed(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let amount = pv(p, 0, 0.3).clamp(0.0, 1.0);
    let freq = pv(p, 1, 700.0).clamp(60.0, 0.45 * sr.max(200.0));
    st.bq[0].set(Shape::Lowpass, freq, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, freq, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let cross_r = st.bq[0].run(xr);
        let cross_l = st.bq[1].run(xl);
        l[i] = xl + amount * cross_r;
        r[i] = xr + amount * cross_l;
    }
}

/// Approximation (see the module doc comment): a fixed single-tap
/// delayed, inverted, low-passed crossfeed subtraction, not HRTF-based.
fn crosstalk_cancel(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let amount = pv(p, 0, 0.5).clamp(0.0, 1.0);
    let delay_ms = pv(p, 1, 0.3).clamp(0.05, 3.0);
    let d = ms_to_samples(delay_ms, sr).max(1.0);
    let cutoff = 6000.0f32.min(0.45 * sr.max(200.0));
    st.bq[0].set(Shape::Lowpass, cutoff, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, cutoff, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let from_r = st.dl[0].read(mem, d);
        st.dl[0].write(mem, xr);
        let from_l = st.dl[1].read(mem, d);
        st.dl[1].write(mem, xl);
        let cross_l = st.bq[0].run(from_r);
        let cross_r = st.bq[1].run(from_l);
        l[i] = xl - amount * cross_l;
        r[i] = xr - amount * cross_r;
    }
}

/// Bandpass-selects a band around `freq`/`q` and optionally flips its
/// phase before recombining with the rest of the signal; `flip = 0`
/// reconstructs the input exactly.
fn phase_select_eq(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = pv(p, 0, 1000.0).clamp(20.0, 0.45 * sr.max(200.0));
    let q = pv(p, 1, 0.7).clamp(0.1, 10.0);
    let sign = if pv(p, 2, 0.0) >= 0.5 { -1.0 } else { 1.0 };
    st.bq[0].set(Shape::Bandpass, freq, q, 0.0, sr);
    st.bq[1].set(Shape::Bandpass, freq, q, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let sel_l = st.bq[0].run(xl);
        let sel_r = st.bq[1].run(xr);
        l[i] = xl - sel_l + sign * sel_l;
        r[i] = xr - sel_r + sign * sel_r;
    }
}

/// Approximation (see the module doc comment): a mid/side split plus a
/// short Schroeder-allpass decorrelation tap standing in for
/// `direct`/`diffuse`/`residual`, not the reference upmixer.
fn spatial_map(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32]) {
    let direct = pv(p, 0, 0.7).clamp(0.0, 1.0);
    let diffuse = pv(p, 1, 0.5).clamp(0.0, 1.0);
    let residual = pv(p, 2, 0.3).clamp(0.0, 1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let m = (xl + xr) * 0.5;
        let s = (xl - xr) * 0.5;
        let dec_l = allpass(&mut st.dl[0], mem, s, 7, 0.5);
        let dec_r = allpass(&mut st.dl[1], mem, s, 5, 0.5);
        l[i] = m * direct + s * diffuse + dec_l * residual;
        r[i] = m * direct - s * diffuse + dec_r * residual;
    }
}

fn pan(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let (gl, gr) = pan_gains(pv(p, 0, 0.5).clamp(0.0, 1.0));
    let n = l.len().min(r.len());
    for i in 0..n {
        let mono = (l[i] + r[i]) * 0.5;
        l[i] = mono * gl;
        r[i] = mono * gr;
    }
}

fn matrix(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let ll = pv(p, 0, 1.0).clamp(-1.0, 1.0);
    let lr = pv(p, 1, 0.0).clamp(-1.0, 1.0);
    let rl = pv(p, 2, 0.0).clamp(-1.0, 1.0);
    let rr = pv(p, 3, 1.0).clamp(-1.0, 1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        l[i] = xl * ll + xr * rl;
        r[i] = xl * lr + xr * rr;
    }
}
