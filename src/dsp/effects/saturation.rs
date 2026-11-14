//! Saturation and harmonic-enhancement effects (design 12.5, 12.8.8).
//!
//! Every kind is a bounded, allocation-free algorithm over the shared
//! primitives in `prim`. Coefficients (filter cutoffs, gains) are computed
//! once per block; only per-sample state (filter memory, envelopes, flip-
//! flops) lives in `FxState`. None of these kinds need delay memory, so
//! `mem_len` is zero and `init` is a no-op — filters are re-configured at
//! the top of every `process` call instead.

use super::prim::{clampf, db_to_gain, tanh, OnePole, Shape};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

/// Reads parameter `i`, falling back to `default` if the caller passed a
/// shorter slice than `params(kind)` (never indexes out of bounds).
#[inline]
fn pv(p: &[f32], i: usize, default: f32) -> f32 {
    p.get(i).copied().unwrap_or(default)
}

const SATURATE: &[ParamDef] = &[
    ParamDef::unit("drive", 12.0, 0.0, 48.0, "dB"),
    ParamDef::new("bias", 0.0, -1.0, 1.0),
    ParamDef::unit("tone", 8000.0, 200.0, 20_000.0, "Hz"),
    ParamDef::unit("output", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const TUBE: &[ParamDef] = &[
    ParamDef::unit("drive", 9.0, 0.0, 36.0, "dB"),
    ParamDef::new("bias", 0.1, -1.0, 1.0),
    ParamDef::unit("tone", 6000.0, 200.0, 20_000.0, "Hz"),
    ParamDef::unit("output", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const CLIP: &[ParamDef] = &[
    ParamDef::unit("drive", 6.0, 0.0, 36.0, "dB"),
    ParamDef::new("threshold", 0.8, 0.05, 1.0),
    ParamDef::unit("output", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const HARMONICS: &[ParamDef] = &[
    ParamDef::new("h2", 0.2, 0.0, 1.0),
    ParamDef::new("h3", 0.1, 0.0, 1.0),
    ParamDef::new("h4", 0.05, 0.0, 1.0),
    ParamDef::new("h5", 0.02, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const EXCITER: &[ParamDef] = &[
    ParamDef::unit("freq", 3000.0, 200.0, 12_000.0, "Hz"),
    ParamDef::new("amount", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const MULTIBAND_SATURATE: &[ParamDef] = &[
    ParamDef::unit("low-freq", 200.0, 40.0, 1000.0, "Hz"),
    ParamDef::unit("high-freq", 4000.0, 1000.0, 16_000.0, "Hz"),
    ParamDef::unit("low-drive", 6.0, 0.0, 36.0, "dB"),
    ParamDef::unit("mid-drive", 6.0, 0.0, 36.0, "dB"),
    ParamDef::unit("high-drive", 6.0, 0.0, 36.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const SUB_SYNTH: &[ParamDef] = &[
    ParamDef::new("amount", 0.5, 0.0, 1.0),
    ParamDef::unit("freq", 150.0, 40.0, 400.0, "Hz"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const BANDWIDTH_EXTEND: &[ParamDef] = &[
    ParamDef::unit("freq", 6000.0, 2000.0, 16_000.0, "Hz"),
    ParamDef::new("amount", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const DYNAMIC_SATURATE: &[ParamDef] = &[
    ParamDef::unit("threshold", -12.0, -48.0, 0.0, "dB"),
    ParamDef::unit("drive", 18.0, 0.0, 48.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

/// The named parameters of a saturation kind.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Saturate => SATURATE,
        K::Tube => TUBE,
        K::Clip => CLIP,
        K::Harmonics => HARMONICS,
        K::Exciter => EXCITER,
        K::MultibandSaturate => MULTIBAND_SATURATE,
        K::SubSynth => SUB_SYNTH,
        K::BandwidthExtend => BANDWIDTH_EXTEND,
        K::DynamicSaturate => DYNAMIC_SATURATE,
        _ => &[],
    }
}

/// No saturation kind needs delay memory.
pub(super) fn mem_len(_kind: EffectKind, _sr: f32) -> usize {
    0
}

/// Filter coefficients are recomputed every block in `process`, so there is
/// nothing to set up beyond the zeroed default state.
pub(super) fn init(_kind: EffectKind, _st: &mut FxState, _mem_len: usize, _sr: f32) {}

/// Processes one stereo block in place, fully wet.
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
        K::Saturate => saturate(p, st, l, r, sr),
        K::Tube => tube(p, st, l, r, sr),
        K::Clip => clip(p, l, r),
        K::Harmonics => harmonics(p, l, r),
        K::Exciter => exciter(p, st, l, r, sr),
        K::MultibandSaturate => multiband_saturate(p, st, l, r, sr),
        K::SubSynth => sub_synth(p, st, l, r, sr),
        K::BandwidthExtend => bandwidth_extend(p, st, l, r, sr),
        K::DynamicSaturate => dynamic_saturate(p, st, l, r, sr),
        _ => {}
    }
}

/// Drive into `tanh`, a post lowpass ("tone") and an output trim.
fn saturate(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let drive = db_to_gain(pv(p, 0, 12.0));
    let bias = pv(p, 1, 0.0) * 0.3;
    let tone = pv(p, 2, 8000.0);
    let out_g = db_to_gain(pv(p, 3, 0.0));
    st.bq[0].set(Shape::Lowpass, tone, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, tone, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let yl = tanh(l[i] * drive + bias);
        l[i] = clampf(st.bq[0].run(yl) * out_g, -16.0, 16.0);
        let yr = tanh(r[i] * drive + bias);
        r[i] = clampf(st.bq[1].run(yr) * out_g, -16.0, 16.0);
    }
}

/// An asymmetric (DC-removed) `tanh` waveshaper approximating tube warmth.
fn tube(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let drive = db_to_gain(pv(p, 0, 9.0));
    let bias = pv(p, 1, 0.1);
    let tone = pv(p, 2, 6000.0);
    let out_g = db_to_gain(pv(p, 3, 0.0));
    let dc = tanh(bias);
    st.bq[0].set(Shape::Lowpass, tone, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, tone, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let yl = tanh(l[i] * drive + bias) - dc;
        l[i] = clampf(st.bq[0].run(yl) * out_g, -16.0, 16.0);
        let yr = tanh(r[i] * drive + bias) - dc;
        r[i] = clampf(st.bq[1].run(yr) * out_g, -16.0, 16.0);
    }
}

/// A hard clipper against a threshold, driven and trimmed.
fn clip(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let drive = db_to_gain(pv(p, 0, 6.0));
    let thr = pv(p, 1, 0.8).clamp(0.05, 1.0);
    let out_g = db_to_gain(pv(p, 2, 0.0));
    let n = l.len().min(r.len());
    for i in 0..n {
        l[i] = clampf((l[i] * drive).clamp(-thr, thr) * out_g, -16.0, 16.0);
        r[i] = clampf((r[i] * drive).clamp(-thr, thr) * out_g, -16.0, 16.0);
    }
}

/// Adds 2nd-5th order harmonics via Chebyshev polynomials of the (`tanh`
/// normalized) input.
fn harmonics(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let h2 = pv(p, 0, 0.2).clamp(0.0, 1.0);
    let h3 = pv(p, 1, 0.1).clamp(0.0, 1.0);
    let h4 = pv(p, 2, 0.05).clamp(0.0, 1.0);
    let h5 = pv(p, 3, 0.02).clamp(0.0, 1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        l[i] = clampf(harmonics_one(l[i], h2, h3, h4, h5), -16.0, 16.0);
        r[i] = clampf(harmonics_one(r[i], h2, h3, h4, h5), -16.0, 16.0);
    }
}

#[inline]
fn harmonics_one(x: f32, h2: f32, h3: f32, h4: f32, h5: f32) -> f32 {
    let xn = tanh(x);
    let x2 = xn * xn;
    let x3 = x2 * xn;
    let x4 = x2 * x2;
    let x5 = x4 * xn;
    let t2 = 2.0 * x2 - 1.0;
    let t3 = 4.0 * x3 - 3.0 * xn;
    let t4 = 8.0 * x4 - 8.0 * x2 + 1.0;
    let t5 = 16.0 * x5 - 20.0 * x3 + 5.0 * xn;
    x + h2 * t2 + h3 * t3 + h4 * t4 + h5 * t5
}

/// Extracts a high band, drives it into odd harmonics and adds it back.
fn exciter(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = pv(p, 0, 3000.0);
    let amount = pv(p, 1, 0.3).clamp(0.0, 1.0);
    st.bq[0].set(Shape::Highpass, freq, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Highpass, freq, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let hp = st.bq[0].run(l[i]);
        l[i] = clampf(l[i] + tanh(hp * 4.0) * amount, -16.0, 16.0);
        let hp = st.bq[1].run(r[i]);
        r[i] = clampf(r[i] + tanh(hp * 4.0) * amount, -16.0, 16.0);
    }
}

/// Splits into low/mid/high bands and saturates each with its own drive.
fn multiband_saturate(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let low_f = pv(p, 0, 200.0);
    let high_f = pv(p, 1, 4000.0).max(low_f + 50.0);
    let low_g = db_to_gain(pv(p, 2, 6.0));
    let mid_g = db_to_gain(pv(p, 3, 6.0));
    let high_g = db_to_gain(pv(p, 4, 6.0));
    st.bq[0].set(Shape::Lowpass, low_f, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, low_f, 0.707, 0.0, sr);
    st.bq[2].set(Shape::Highpass, high_f, 0.707, 0.0, sr);
    st.bq[3].set(Shape::Highpass, high_f, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let low = st.bq[0].run(l[i]);
        let high = st.bq[2].run(l[i]);
        let mid = l[i] - low - high;
        l[i] = clampf(
            tanh(low * low_g) + tanh(mid * mid_g) + tanh(high * high_g),
            -16.0,
            16.0,
        );
        let low = st.bq[1].run(r[i]);
        let high = st.bq[3].run(r[i]);
        let mid = r[i] - low - high;
        r[i] = clampf(
            tanh(low * low_g) + tanh(mid * mid_g) + tanh(high * high_g),
            -16.0,
            16.0,
        );
    }
}

/// An octave-down sub oscillator: a zero-crossing flip-flop on the
/// lowpassed input, amplitude-shaped by its envelope and added back.
fn sub_synth(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let amount = pv(p, 0, 0.5).clamp(0.0, 1.0);
    let cutoff = pv(p, 1, 150.0);
    st.bq[0].set(Shape::Lowpass, cutoff, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, cutoff, 0.707, 0.0, sr);
    let att = OnePole::time_coef(0.001, sr);
    let rel = OnePole::time_coef(0.05, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let fl = st.bq[0].run(l[i]);
        let sign = fl >= 0.0;
        if !st.s[0].is_sign_positive() && sign {
            st.s[1] = if st.s[1] == 0.0 { 1.0 } else { -st.s[1] };
        }
        st.s[0] = if sign { 1.0 } else { -1.0 };
        let env = st.env[0].run(fl, att, rel);
        l[i] = clampf(l[i] + st.s[1] * env * amount, -16.0, 16.0);

        let fr = st.bq[1].run(r[i]);
        let sign = fr >= 0.0;
        if !st.s[2].is_sign_positive() && sign {
            st.s[3] = if st.s[3] == 0.0 { 1.0 } else { -st.s[3] };
        }
        st.s[2] = if sign { 1.0 } else { -1.0 };
        let env = st.env[1].run(fr, att, rel);
        r[i] = clampf(r[i] + st.s[3] * env * amount, -16.0, 16.0);
    }
}

/// Synthesizes new high-frequency content from a top band via rectification
/// and adds it back (approximates bandwidth extension / aliasing-style
/// harmonic synthesis above the source's natural top end).
fn bandwidth_extend(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = pv(p, 0, 6000.0);
    let amount = pv(p, 1, 0.3).clamp(0.0, 1.0);
    st.bq[0].set(Shape::Highpass, freq * 0.5, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Highpass, freq * 0.5, 0.707, 0.0, sr);
    st.bq[2].set(Shape::Highpass, freq, 0.707, 0.0, sr);
    st.bq[3].set(Shape::Highpass, freq, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let band = st.bq[0].run(l[i]);
        let harm = band.abs() * band;
        let hf = st.bq[2].run(harm);
        l[i] = clampf(l[i] + hf * amount * 4.0, -16.0, 16.0);

        let band = st.bq[1].run(r[i]);
        let harm = band.abs() * band;
        let hf = st.bq[3].run(harm);
        r[i] = clampf(r[i] + hf * amount * 4.0, -16.0, 16.0);
    }
}

/// Saturation whose drive grows with the input envelope above a threshold.
fn dynamic_saturate(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let thr_lin = db_to_gain(pv(p, 0, -12.0));
    let drive_g = db_to_gain(pv(p, 1, 18.0));
    let att = OnePole::time_coef(0.005, sr);
    let rel = OnePole::time_coef(0.15, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let env = st.env[0].run(l[i], att, rel);
        let t = ((env / thr_lin.max(1.0e-6)) - 1.0).clamp(0.0, 4.0) / 4.0;
        let g = 1.0 + t * (drive_g - 1.0);
        l[i] = clampf(tanh(l[i] * g), -16.0, 16.0);

        let env = st.env[1].run(r[i], att, rel);
        let t = ((env / thr_lin.max(1.0e-6)) - 1.0).clamp(0.0, 4.0) / 4.0;
        let g = 1.0 + t * (drive_g - 1.0);
        r[i] = clampf(tanh(r[i] * g), -16.0, 16.0);
    }
}
