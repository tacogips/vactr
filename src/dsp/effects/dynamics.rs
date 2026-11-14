//! Dynamics effects (design 12.5, 12.8.8; design-music.md section 5):
//! `compressor`, `expander`, `gate`, `limiter`, `multiband-compressor`,
//! `multiband-expander`, `transient`, `multiband-transient`, `auto-level`,
//! `sag`.
//!
//! Every kind is a bounded, allocation-free algorithm over the shared
//! primitives in `prim`. Multiband kinds split the signal into three bands
//! with two cascaded (2nd-order) Butterworth low/high-pass pairs per
//! channel rather than a true Linkwitz-Riley crossover, and reconstruct the
//! middle band as `input - low - high`; this is close enough for the
//! per-band dynamics processing here (sound quality is not an acceptance
//! criterion, only bounded and finite output).

use crate::dsp::graph::EffectKind;

use super::prim::{self, Biquad, DelayLine, OnePole, Shape};
use super::{FxCtx, FxState, ParamDef};

// -- parameter tables (kebab-case, catalog order) --------------------------

const COMPRESSOR: [ParamDef; 7] = [
    ParamDef::unit("threshold", -18.0, -60.0, 0.0, "dB"),
    ParamDef::new("ratio", 4.0, 1.0, 20.0),
    ParamDef::unit("attack", 0.01, 0.0005, 1.0, "s"),
    ParamDef::unit("release", 0.1, 0.005, 2.0, "s"),
    ParamDef::unit("makeup", 0.0, -24.0, 24.0, "dB"),
    ParamDef::unit("knee", 6.0, 0.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const EXPANDER: [ParamDef; 7] = [
    ParamDef::unit("threshold", -40.0, -80.0, 0.0, "dB"),
    ParamDef::new("ratio", 2.0, 1.0, 20.0),
    ParamDef::unit("attack", 0.005, 0.0005, 1.0, "s"),
    ParamDef::unit("release", 0.15, 0.005, 2.0, "s"),
    ParamDef::unit("makeup", 0.0, -24.0, 24.0, "dB"),
    ParamDef::unit("knee", 6.0, 0.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const GATE: [ParamDef; 6] = [
    ParamDef::unit("threshold", -50.0, -90.0, 0.0, "dB"),
    ParamDef::unit("attack", 0.001, 0.0002, 0.5, "s"),
    ParamDef::unit("release", 0.1, 0.005, 2.0, "s"),
    ParamDef::unit("hold", 0.02, 0.0, 1.0, "s"),
    ParamDef::unit("range", -80.0, -100.0, 0.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const LIMITER: [ParamDef; 3] = [
    ParamDef::unit("ceiling", -0.3, -24.0, 0.0, "dB"),
    ParamDef::unit("release", 0.05, 0.001, 2.0, "s"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const MULTIBAND_COMPRESSOR: [ParamDef; 10] = [
    ParamDef::unit("low-freq", 200.0, 40.0, 2000.0, "Hz"),
    ParamDef::unit("high-freq", 2000.0, 800.0, 18000.0, "Hz"),
    ParamDef::unit("low-threshold", -18.0, -60.0, 0.0, "dB"),
    ParamDef::unit("mid-threshold", -18.0, -60.0, 0.0, "dB"),
    ParamDef::unit("high-threshold", -18.0, -60.0, 0.0, "dB"),
    ParamDef::new("ratio", 4.0, 1.0, 20.0),
    ParamDef::unit("attack", 0.01, 0.0005, 1.0, "s"),
    ParamDef::unit("release", 0.1, 0.005, 2.0, "s"),
    ParamDef::unit("makeup", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const MULTIBAND_EXPANDER: [ParamDef; 10] = [
    ParamDef::unit("low-freq", 200.0, 40.0, 2000.0, "Hz"),
    ParamDef::unit("high-freq", 2000.0, 800.0, 18000.0, "Hz"),
    ParamDef::unit("low-threshold", -40.0, -80.0, 0.0, "dB"),
    ParamDef::unit("mid-threshold", -40.0, -80.0, 0.0, "dB"),
    ParamDef::unit("high-threshold", -40.0, -80.0, 0.0, "dB"),
    ParamDef::new("ratio", 2.0, 1.0, 20.0),
    ParamDef::unit("attack", 0.005, 0.0005, 1.0, "s"),
    ParamDef::unit("release", 0.15, 0.005, 2.0, "s"),
    ParamDef::unit("makeup", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const TRANSIENT: [ParamDef; 3] = [
    ParamDef::new("attack", 0.0, -1.0, 1.0),
    ParamDef::new("sustain", 0.0, -1.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const MULTIBAND_TRANSIENT: [ParamDef; 9] = [
    ParamDef::unit("low-freq", 200.0, 40.0, 2000.0, "Hz"),
    ParamDef::unit("high-freq", 2000.0, 800.0, 18000.0, "Hz"),
    ParamDef::new("low-attack", 0.0, -1.0, 1.0),
    ParamDef::new("low-sustain", 0.0, -1.0, 1.0),
    ParamDef::new("mid-attack", 0.0, -1.0, 1.0),
    ParamDef::new("mid-sustain", 0.0, -1.0, 1.0),
    ParamDef::new("high-attack", 0.0, -1.0, 1.0),
    ParamDef::new("high-sustain", 0.0, -1.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const AUTO_LEVEL: [ParamDef; 2] = [
    ParamDef::unit("target", -16.0, -40.0, 0.0, "dB"),
    ParamDef::unit("speed", 2.0, 0.05, 10.0, "s"),
];

const SAG: [ParamDef; 2] = [
    ParamDef::new("amount", 0.3, 0.0, 1.0),
    ParamDef::unit("recovery", 0.3, 0.01, 3.0, "s"),
];

/// The named parameters of a dynamics kind (empty for kinds outside this
/// group).
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Compressor => &COMPRESSOR,
        K::Expander => &EXPANDER,
        K::Gate => &GATE,
        K::Limiter => &LIMITER,
        K::MultibandCompressor => &MULTIBAND_COMPRESSOR,
        K::MultibandExpander => &MULTIBAND_EXPANDER,
        K::Transient => &TRANSIENT,
        K::MultibandTransient => &MULTIBAND_TRANSIENT,
        K::AutoLevel => &AUTO_LEVEL,
        K::Sag => &SAG,
        _ => &[],
    }
}

/// The lookahead window (samples) the limiter's delay line carves.
fn look_samples(sr: f32) -> usize {
    let n = (0.005 * sr).round();
    if n < 1.0 {
        1
    } else {
        n as usize
    }
}

/// The preferred delay memory of a dynamics kind (only `limiter` uses any,
/// for its short lookahead delay line).
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    match kind {
        EffectKind::Limiter => look_samples(sr) * 2,
        _ => 0,
    }
}

/// Sets up a zeroed dynamics unit.
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, sr: f32) {
    match kind {
        EffectKind::Limiter => {
            let look = look_samples(sr);
            let mut cursor = 0usize;
            st.dl[0] = DelayLine::carve(&mut cursor, mem_len, look);
            st.dl[1] = DelayLine::carve(&mut cursor, mem_len, look);
        }
        EffectKind::AutoLevel => st.s[0] = 1.0,
        _ => {}
    }
}

// -- shared helpers ---------------------------------------------------------

/// The feed-forward compressor gain-computer (dB), soft-kneed, always
/// `<= 0`: unity above `knee` below threshold, `1/ratio` slope above it.
fn comp_gain_db(x: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let w = knee.max(0.0);
    let over = x - threshold;
    let y = if 2.0 * over < -w {
        x
    } else if 2.0 * over > w || w < 1.0e-6 {
        threshold + (x - threshold) / ratio
    } else {
        let k = over + w * 0.5;
        x + ((1.0 / ratio - 1.0) * k * k) / (2.0 * w)
    };
    y - x
}

/// The downward-expander gain-computer (dB), soft-kneed, `<= 0`: unity
/// above threshold, `ratio`-steepened attenuation below it.
fn exp_gain_db(x: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let w = knee.max(0.0);
    let over = x - threshold;
    let y = if 2.0 * over > w {
        x
    } else if 2.0 * over < -w || w < 1.0e-6 {
        threshold + (x - threshold) * ratio
    } else {
        let k = over - w * 0.5;
        x + ((ratio - 1.0) * k * k) / (2.0 * w)
    };
    (y - x).min(0.0)
}

/// Sets up the two cascaded low/high-pass pairs of a three-band split at
/// `bq[base..base + 4]` (approximate; see the module doc).
fn setup_split3(bq: &mut [Biquad; 16], base: usize, low_freq: f32, high_freq: f32, sr: f32) {
    bq[base].set(Shape::Lowpass, low_freq, 0.707, 0.0, sr);
    bq[base + 1].set(Shape::Lowpass, low_freq, 0.707, 0.0, sr);
    bq[base + 2].set(Shape::Highpass, high_freq, 0.707, 0.0, sr);
    bq[base + 3].set(Shape::Highpass, high_freq, 0.707, 0.0, sr);
}

/// One sample through a three-band split at `bq[base..base + 4]`
/// (low, mid, high); mid is reconstructed as `x - low - high`.
fn split3(bq: &mut [Biquad; 16], base: usize, x: f32) -> (f32, f32, f32) {
    let a = bq[base].run(x);
    let low = bq[base + 1].run(a);
    let b = bq[base + 2].run(x);
    let high = bq[base + 3].run(b);
    (low, x - low - high, high)
}

/// One step of a plain abs-value follower kept in caller state (used where
/// more followers are needed than `FxState::env` holds).
fn env_step(prev: f32, x: f32, att: f32, rel: f32) -> f32 {
    let a = x.abs();
    let c = if a > prev { att } else { rel };
    let v = prev + c * (a - prev);
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Processes one stereo block of a dynamics kind in place, fully wet
/// (kinds outside this group are a no-op).
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
    let sr = ctx.sr;
    match kind {
        K::Compressor => {
            let thr = p[0];
            let ratio = p[1].max(1.0);
            let att = OnePole::time_coef(p[2], sr);
            let rel = OnePole::time_coef(p[3], sr);
            let makeup = prim::db_to_gain(p[4]);
            let knee = p[5].max(0.0);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let peak = xl.abs().max(xr.abs());
                let env = st.env[0].run(peak, att, rel);
                let gr = comp_gain_db(prim::gain_to_db(env), thr, ratio, knee);
                let g = (prim::db_to_gain(gr) * makeup).clamp(0.0, 8.0);
                *xl *= g;
                *xr *= g;
            }
        }
        K::Expander => {
            let thr = p[0];
            let ratio = p[1].max(1.0);
            let att = OnePole::time_coef(p[2], sr);
            let rel = OnePole::time_coef(p[3], sr);
            let makeup = prim::db_to_gain(p[4]);
            let knee = p[5].max(0.0);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let peak = xl.abs().max(xr.abs());
                let env = st.env[0].run(peak, att, rel);
                let gr = exp_gain_db(prim::gain_to_db(env), thr, ratio, knee);
                let g = (prim::db_to_gain(gr) * makeup).clamp(0.0, 8.0);
                *xl *= g;
                *xr *= g;
            }
        }
        K::Gate => {
            let thr = p[0];
            let att = OnePole::time_coef(p[1], sr);
            let rel = OnePole::time_coef(p[2], sr);
            let hold_frames = (p[3] * sr).max(0.0);
            let range_db = p[4];
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let peak = xl.abs().max(xr.abs());
                let env = st.env[0].run(peak, att, rel);
                let env_db = prim::gain_to_db(env);
                if env_db > thr {
                    st.s[0] = hold_frames;
                } else if st.s[0] > 0.0 {
                    st.s[0] -= 1.0;
                }
                let target_db = if env_db > thr || st.s[0] > 0.0 {
                    0.0
                } else {
                    range_db
                };
                let coef = if target_db > st.s[1] { att } else { rel };
                st.s[1] += coef * (target_db - st.s[1]);
                if !st.s[1].is_finite() {
                    st.s[1] = 0.0;
                }
                let g = prim::db_to_gain(st.s[1]);
                *xl *= g;
                *xr *= g;
            }
        }
        K::Limiter => {
            let ceiling_g = prim::db_to_gain(p[0]).clamp(0.05, 1.5);
            let rel = OnePole::time_coef(p[1].max(0.001), sr);
            let att = OnePole::time_coef(0.0005, sr);
            let look_l = st.dl[0].capacity();
            let look_r = st.dl[1].capacity();
            let n = l.len().min(r.len());
            for i in 0..n {
                let in_l = l[i];
                let in_r = r[i];
                let peak = in_l.abs().max(in_r.abs());
                let env = st.env[0].run(peak, att, rel);
                let g = if env > ceiling_g && env > 1.0e-9 {
                    (ceiling_g / env).min(1.0)
                } else {
                    1.0
                };
                let gc = if g < st.s[0] { att } else { rel };
                st.s[0] += gc * (g - st.s[0]);
                if !st.s[0].is_finite() {
                    st.s[0] = 1.0;
                }
                let gain = st.s[0].clamp(0.0, 1.0);
                st.dl[0].write(mem, in_l);
                st.dl[1].write(mem, in_r);
                let out_l = if look_l > 0 {
                    st.dl[0].tap(mem, look_l)
                } else {
                    in_l
                };
                let out_r = if look_r > 0 {
                    st.dl[1].tap(mem, look_r)
                } else {
                    in_r
                };
                l[i] = out_l * gain;
                r[i] = out_r * gain;
            }
        }
        K::MultibandCompressor => {
            let low_freq = p[0].max(20.0);
            let high_freq = p[1].max(low_freq + 20.0);
            setup_split3(&mut st.bq, 0, low_freq, high_freq, sr);
            setup_split3(&mut st.bq, 4, low_freq, high_freq, sr);
            let thr = [p[2], p[3], p[4]];
            let ratio = p[5].max(1.0);
            let att = OnePole::time_coef(p[6], sr);
            let rel = OnePole::time_coef(p[7], sr);
            let makeup = prim::db_to_gain(p[8]);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let (ll, lm, lh) = split3(&mut st.bq, 0, *xl);
                let (rl, rm, rh) = split3(&mut st.bq, 4, *xr);
                let bl = [ll, lm, lh];
                let br = [rl, rm, rh];
                let mut outl = 0.0f32;
                let mut outr = 0.0f32;
                for b in 0..3 {
                    let peak = bl[b].abs().max(br[b].abs());
                    let env = st.env[b].run(peak, att, rel);
                    let gr = comp_gain_db(prim::gain_to_db(env), thr[b], ratio, 6.0);
                    let g = (prim::db_to_gain(gr) * makeup).clamp(0.0, 8.0);
                    outl += bl[b] * g;
                    outr += br[b] * g;
                }
                *xl = outl;
                *xr = outr;
            }
        }
        K::MultibandExpander => {
            let low_freq = p[0].max(20.0);
            let high_freq = p[1].max(low_freq + 20.0);
            setup_split3(&mut st.bq, 0, low_freq, high_freq, sr);
            setup_split3(&mut st.bq, 4, low_freq, high_freq, sr);
            let thr = [p[2], p[3], p[4]];
            let ratio = p[5].max(1.0);
            let att = OnePole::time_coef(p[6], sr);
            let rel = OnePole::time_coef(p[7], sr);
            let makeup = prim::db_to_gain(p[8]);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let (ll, lm, lh) = split3(&mut st.bq, 0, *xl);
                let (rl, rm, rh) = split3(&mut st.bq, 4, *xr);
                let bl = [ll, lm, lh];
                let br = [rl, rm, rh];
                let mut outl = 0.0f32;
                let mut outr = 0.0f32;
                for b in 0..3 {
                    let peak = bl[b].abs().max(br[b].abs());
                    let env = st.env[b].run(peak, att, rel);
                    let gr = exp_gain_db(prim::gain_to_db(env), thr[b], ratio, 6.0);
                    let g = (prim::db_to_gain(gr) * makeup).clamp(0.0, 8.0);
                    outl += bl[b] * g;
                    outr += br[b] * g;
                }
                *xl = outl;
                *xr = outr;
            }
        }
        K::Transient => {
            let a_amt = p[0];
            let s_amt = p[1];
            let fatt = OnePole::time_coef(0.001, sr);
            let frel = OnePole::time_coef(0.001, sr);
            let satt = OnePole::time_coef(0.025, sr);
            let srel = OnePole::time_coef(0.150, sr);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let peak = xl.abs().max(xr.abs());
                let fast = st.env[0].run(peak, fatt, frel);
                let slow = st.env[1].run(peak, satt, srel);
                let diff = fast - slow;
                let gain = if diff > 0.0 {
                    1.0 + a_amt * diff.min(2.0)
                } else {
                    1.0 + s_amt * (-diff).min(2.0)
                };
                let g = gain.clamp(0.0, 4.0);
                *xl *= g;
                *xr *= g;
            }
        }
        K::MultibandTransient => {
            let low_freq = p[0].max(20.0);
            let high_freq = p[1].max(low_freq + 20.0);
            setup_split3(&mut st.bq, 0, low_freq, high_freq, sr);
            setup_split3(&mut st.bq, 4, low_freq, high_freq, sr);
            let amt = [(p[2], p[3]), (p[4], p[5]), (p[6], p[7])];
            let fatt = OnePole::time_coef(0.001, sr);
            let frel = OnePole::time_coef(0.001, sr);
            let satt = OnePole::time_coef(0.025, sr);
            let srel = OnePole::time_coef(0.150, sr);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let (ll, lm, lh) = split3(&mut st.bq, 0, *xl);
                let (rl, rm, rh) = split3(&mut st.bq, 4, *xr);
                let bl = [ll, lm, lh];
                let br = [rl, rm, rh];
                let mut outl = 0.0f32;
                let mut outr = 0.0f32;
                for b in 0..3 {
                    let peak = bl[b].abs().max(br[b].abs());
                    let fast = env_step(st.s[b], peak, fatt, frel);
                    let slow = env_step(st.s[b + 3], peak, satt, srel);
                    st.s[b] = fast;
                    st.s[b + 3] = slow;
                    let diff = fast - slow;
                    let (a_amt, s_amt) = amt[b];
                    let gain = if diff > 0.0 {
                        1.0 + a_amt * diff.min(2.0)
                    } else {
                        1.0 + s_amt * (-diff).min(2.0)
                    };
                    let g = gain.clamp(0.0, 4.0);
                    outl += bl[b] * g;
                    outr += br[b] * g;
                }
                *xl = outl;
                *xr = outr;
            }
        }
        K::AutoLevel => {
            let target = prim::db_to_gain(p[0]);
            let coef = OnePole::time_coef(p[1].max(0.05), sr);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let peak = xl.abs().max(xr.abs());
                let level = st.env[0].run(peak, coef, coef).max(1.0e-4);
                let desired = (target / level).clamp(0.1, 10.0);
                st.s[0] += coef * (desired - st.s[0]);
                if !st.s[0].is_finite() {
                    st.s[0] = 1.0;
                }
                let g = st.s[0].clamp(0.1, 10.0);
                *xl *= g;
                *xr *= g;
            }
        }
        K::Sag => {
            let amount = p[0].clamp(0.0, 1.0);
            let rel = OnePole::time_coef(p[1].max(0.01), sr);
            let att = OnePole::time_coef(0.005, sr);
            let n = l.len().min(r.len());
            for (xl, xr) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let peak = xl.abs().max(xr.abs());
                let drive = st.env[0].run(peak, att, rel).min(2.0);
                let g = (1.0 - amount * drive * 0.5).clamp(0.2, 1.0);
                *xl *= g;
                *xr *= g;
            }
        }
        _ => {}
    }
}
