//! Restoration effects (design 12.5, 12.8.8; design-music.md section 5):
//! `declick`, `declip`, `dehum`, `denoise`.
//!
//! Every kind is a bounded, allocation-free algorithm over the shared
//! primitives in `prim`. None of these need delay memory: `declick` and
//! `declip` work sample-by-sample against short in-state history, `dehum`
//! is a bank of narrow biquads, and `denoise` splits the signal with
//! biquads only (see its approximation note below).

use crate::dsp::graph::EffectKind;

use super::prim::{self, Biquad, Follower, OnePole, Shape};
use super::{FxCtx, FxState, ParamDef};

// -- parameter tables (kebab-case, catalog order) --------------------------

const DECLICK: [ParamDef; 3] = [
    ParamDef::new("threshold", 0.35, 0.05, 1.0),
    ParamDef::new("amount", 1.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const DECLIP: [ParamDef; 2] = [
    ParamDef::new("threshold", 0.95, 0.5, 0.999),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const DEHUM: [ParamDef; 4] = [
    ParamDef::unit("freq", 50.0, 40.0, 70.0, "Hz"),
    ParamDef::new("harmonics", 3.0, 1.0, 6.0),
    ParamDef::new("amount", 0.8, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const DENOISE: [ParamDef; 3] = [
    ParamDef::unit("threshold", -40.0, -80.0, 0.0, "dB"),
    ParamDef::unit("reduction", 12.0, 0.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

/// The named parameters of a restoration kind (empty for kinds outside this
/// group).
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Declick => &DECLICK,
        K::Declip => &DECLIP,
        K::Dehum => &DEHUM,
        K::Denoise => &DENOISE,
        _ => &[],
    }
}

/// No restoration kind needs delay memory.
pub(super) fn mem_len(_kind: EffectKind, _sr: f32) -> usize {
    0
}

/// Sets up a zeroed restoration unit (nothing to do: all state starts at
/// zero, which is a sane rest state for every kind here).
pub(super) fn init(_kind: EffectKind, _st: &mut FxState, _mem_len: usize, _sr: f32) {}

// -- shared helpers ---------------------------------------------------------

/// One channel of the declick predictive-interpolation detector: a click
/// is a sample that deviates from the linear prediction of the last two
/// (already-declicked) samples by more than `threshold` times the local
/// level; deviating samples are pulled `amount` of the way back to the
/// prediction. This is an approximation of true click removal (which needs
/// a look-ahead spline reconstruction over the clicked span); it catches
/// single-sample and short spike clicks without extra delay memory.
///
/// The bare two-sample linear extrapolation (`2*prev1 - prev2`) has a
/// double pole at `z = 1` and is only meant to nudge a genuine click back
/// toward its neighbors; fed uncorrelated input (e.g. white noise
/// repeatedly crossing the click threshold) it would otherwise compound
/// without bound. Both the prediction and the repaired sample are clamped
/// to `+-4x` the local envelope, so the recursive state (`prev1`, `prev2`)
/// can never leave that bound and the effective feedback stays under 1.
#[allow(clippy::too_many_arguments)]
fn declick_channel(
    prev1: &mut f32,
    prev2: &mut f32,
    env: &mut Follower,
    x: f32,
    threshold: f32,
    amount: f32,
    att: f32,
    rel: f32,
) -> f32 {
    let level = env.run(x, att, rel).max(1.0e-4);
    let bound = level * 4.0;
    let predicted = (2.0 * *prev1 - *prev2).clamp(-bound, bound);
    let deviation = (x - predicted).abs();
    let out = if deviation > threshold * level * 8.0 {
        (x + (predicted - x) * amount).clamp(-bound, bound)
    } else {
        x
    };
    *prev2 = *prev1;
    *prev1 = out;
    out
}

/// Declip approximation: samples beyond `threshold` amplitude are re-shaped
/// with a `tanh` soft-saturation curve instead of the hard clip, softening
/// the edge without needing a look-ahead reconstruction of the clipped
/// span.
fn declip_sample(x: f32, threshold: f32) -> f32 {
    let t = threshold.max(0.05);
    let a = x.abs();
    if a <= t {
        x
    } else {
        let span = (1.0 - t).max(1.0e-3);
        let over = (a - t) / span;
        let shaped = t + span * prim::tanh(over);
        x.signum() * shaped
    }
}

/// One sample through a four-band split (three cascaded low-pass taps,
/// each removing its band from the running remainder) using
/// `bq[base..base + 6]`.
fn split4(bq: &mut [Biquad; 16], base: usize, x: f32) -> [f32; 4] {
    let a = bq[base].run(x);
    let band0 = bq[base + 1].run(a);
    let rem0 = x - band0;
    let b = bq[base + 2].run(rem0);
    let band1 = bq[base + 3].run(b);
    let rem1 = rem0 - band1;
    let c = bq[base + 4].run(rem1);
    let band2 = bq[base + 5].run(c);
    let band3 = rem1 - band2;
    [band0, band1, band2, band3]
}

/// The downward-expander gain (dB, `<= 0`) a denoise band applies: unity
/// above `threshold`, a fixed 1.5:1 slope below it, capped at
/// `-max_reduction`.
fn denoise_gain_db(level_db: f32, threshold: f32, max_reduction: f32) -> f32 {
    if level_db >= threshold {
        0.0
    } else {
        ((level_db - threshold) * 1.5).max(-max_reduction)
    }
}

/// Processes one stereo block of a restoration kind in place, fully wet
/// (kinds outside this group are a no-op).
#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    _mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    use EffectKind as K;
    let sr = ctx.sr;
    match kind {
        K::Declick => {
            let threshold = p[0];
            let amount = p[1].clamp(0.0, 1.0);
            let att = OnePole::time_coef(0.005, sr);
            let rel = OnePole::time_coef(0.05, sr);
            let n = l.len().min(r.len());
            let [p1l, p2l, p1r, p2r, ..] = &mut st.s;
            let [envl, envr, ..] = &mut st.env;
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                *xl = declick_channel(p1l, p2l, envl, *xl, threshold, amount, att, rel);
                *xr = declick_channel(p1r, p2r, envr, *xr, threshold, amount, att, rel);
            }
        }
        K::Declip => {
            let threshold = p[0];
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                *xl = declip_sample(*xl, threshold);
                *xr = declip_sample(*xr, threshold);
            }
        }
        K::Dehum => {
            let freq = p[0].max(20.0);
            let n_harm = p[1].round().clamp(1.0, 6.0) as usize;
            let amount = p[2].clamp(0.0, 1.0);
            let gain_db = -amount * 30.0;
            for h in 0..6usize {
                if h < n_harm {
                    let f = (freq * (h as f32 + 1.0)).min(sr * 0.45);
                    st.bq[h].set(Shape::Peak, f, 12.0, gain_db, sr);
                    st.bq[h + 6].set(Shape::Peak, f, 12.0, gain_db, sr);
                } else {
                    st.bq[h] = Biquad::identity();
                    st.bq[h + 6] = Biquad::identity();
                }
            }
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let mut yl = *xl;
                for h in 0..6 {
                    yl = st.bq[h].run(yl);
                }
                let mut yr = *xr;
                for h in 0..6 {
                    yr = st.bq[h + 6].run(yr);
                }
                *xl = yl;
                *xr = yr;
            }
        }
        // Approximation: a reference-quality denoiser needs spectral
        // subtraction over an FFT frame with a noise-floor estimate; this
        // ships a per-band downward expander over a fixed 4-band biquad
        // split instead, which is bounded, allocation-free and needs no
        // block buffering.
        K::Denoise => {
            let threshold = p[0];
            let reduction = p[1].max(0.0);
            setup_split4(&mut st.bq, 0, sr);
            setup_split4(&mut st.bq, 6, sr);
            let att = OnePole::time_coef(0.003, sr);
            let rel = OnePole::time_coef(0.06, sr);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let bl = split4(&mut st.bq, 0, *xl);
                let br = split4(&mut st.bq, 6, *xr);
                let mut outl = 0.0f32;
                let mut outr = 0.0f32;
                for b in 0..4 {
                    let peak = bl[b].abs().max(br[b].abs());
                    let env = st.env[b].run(peak, att, rel);
                    let gr = denoise_gain_db(prim::gain_to_db(env), threshold, reduction);
                    let g = prim::db_to_gain(gr);
                    outl += bl[b] * g;
                    outr += br[b] * g;
                }
                *xl = outl;
                *xr = outr;
            }
        }
        _ => {}
    }
}

/// Sets up the three cascaded low-pass taps of a four-band split at
/// `bq[base..base + 6]` (fixed internal crossovers at 200 Hz, 1 kHz and
/// 4 kHz).
fn setup_split4(bq: &mut [Biquad; 16], base: usize, sr: f32) {
    bq[base].set(Shape::Lowpass, 200.0, 0.707, 0.0, sr);
    bq[base + 1].set(Shape::Lowpass, 200.0, 0.707, 0.0, sr);
    bq[base + 2].set(Shape::Lowpass, 1000.0, 0.707, 0.0, sr);
    bq[base + 3].set(Shape::Lowpass, 1000.0, 0.707, 0.0, sr);
    bq[base + 4].set(Shape::Lowpass, 4000.0, 0.707, 0.0, sr);
    bq[base + 5].set(Shape::Lowpass, 4000.0, 0.707, 0.0, sr);
}
