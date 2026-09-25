//! EQ and filter effects (design-music.md section 5, design 12.5/12.8.8):
//! `peq`, `geq`, `dynamic-eq`, `tilt`, `tone`, `loudness-eq`, `lpf`, `hpf`,
//! `bpf`, `notch`, `comb`, `narrow`, `linear-phase-eq`, `group-delay-eq`,
//! `crossover`.
//!
//! Every kind is a bounded, allocation-free algorithm over the shared
//! `prim` primitives. Filter coefficients are computed once per block (at
//! the start of `process`), never per sample. `linear-phase-eq` and
//! `group-delay-eq` ship a documented minimum-phase approximation (design
//! 12.8.8): they do not use an FIR or lookahead.

use super::prim::{gain_to_db, DelayLine, OnePole, Shape};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

/// Reads parameter `i`, defaulting when `p` is shorter than
/// `params(kind)` (defensive: production callers always match lengths).
#[inline]
fn pv(p: &[f32], i: usize, default: f32) -> f32 {
    p.get(i).copied().unwrap_or(default)
}

const PEQ_PARAMS: [ParamDef; 10] = [
    ParamDef::unit("low-freq", 80.0, 20.0, 2000.0, "Hz"),
    ParamDef::unit("low-gain", 0.0, -24.0, 24.0, "dB"),
    ParamDef::unit("mid1-freq", 500.0, 20.0, 10_000.0, "Hz"),
    ParamDef::unit("mid1-gain", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mid1-q", 0.7, 0.1, 10.0),
    ParamDef::unit("mid2-freq", 2000.0, 20.0, 18_000.0, "Hz"),
    ParamDef::unit("mid2-gain", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mid2-q", 0.7, 0.1, 10.0),
    ParamDef::unit("high-freq", 8000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::unit("high-gain", 0.0, -24.0, 24.0, "dB"),
];

/// Eight ISO-ish octave band centers for `geq` (8 bands x 2 channels = 16
/// biquads, exactly the `FxState::bq` bank).
const GEQ_BANDS: [f32; 8] = [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];
const GEQ_Q: f32 = 1.4;

const GEQ_PARAMS: [ParamDef; 8] = [
    ParamDef::unit("band-1", 0.0, -12.0, 12.0, "dB"),
    ParamDef::unit("band-2", 0.0, -12.0, 12.0, "dB"),
    ParamDef::unit("band-3", 0.0, -12.0, 12.0, "dB"),
    ParamDef::unit("band-4", 0.0, -12.0, 12.0, "dB"),
    ParamDef::unit("band-5", 0.0, -12.0, 12.0, "dB"),
    ParamDef::unit("band-6", 0.0, -12.0, 12.0, "dB"),
    ParamDef::unit("band-7", 0.0, -12.0, 12.0, "dB"),
    ParamDef::unit("band-8", 0.0, -12.0, 12.0, "dB"),
];

const DYNAMIC_EQ_PARAMS: [ParamDef; 5] = [
    ParamDef::unit("freq", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 1.0, 0.1, 10.0),
    ParamDef::unit("threshold", -24.0, -60.0, 0.0, "dB"),
    ParamDef::unit("range", 6.0, 0.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const TILT_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("freq", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::unit("gain", 0.0, -24.0, 24.0, "dB"),
];

const TONE_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("bass", 0.0, -24.0, 24.0, "dB"),
    ParamDef::unit("treble", 0.0, -24.0, 24.0, "dB"),
];

const LOUDNESS_EQ_PARAMS: [ParamDef; 1] = [ParamDef::unit("level", 0.0, -24.0, 24.0, "dB")];

const LPF_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 0.707, 0.1, 10.0),
];
const HPF_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("cutoff", 100.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 0.707, 0.1, 10.0),
];
const BPF_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 1.0, 0.1, 10.0),
];
const NOTCH_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 4.0, 0.1, 10.0),
];

/// The longest delay a `comb` unit's `time` parameter can request.
const COMB_TIME_MAX: f32 = 0.05;

const COMB_PARAMS: [ParamDef; 3] = [
    ParamDef::unit("time", 0.01, 0.0002, COMB_TIME_MAX, "s"),
    ParamDef::new("feedback", 0.5, -0.9, 0.9),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const NARROW_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("freq", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("width", 0.1, 0.01, 2.0),
];

const PHASE_EQ_PARAMS: [ParamDef; 4] = [
    ParamDef::unit("bass", 0.0, -24.0, 24.0, "dB"),
    ParamDef::unit("mid", 0.0, -24.0, 24.0, "dB"),
    ParamDef::unit("treble", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const CROSSOVER_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("freq", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("band", 0.0, 0.0, 1.0),
];

/// The named parameters of an eq/filter kind, in catalog order.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Peq => &PEQ_PARAMS,
        K::Geq => &GEQ_PARAMS,
        K::DynamicEq => &DYNAMIC_EQ_PARAMS,
        K::Tilt => &TILT_PARAMS,
        K::Tone => &TONE_PARAMS,
        K::LoudnessEq => &LOUDNESS_EQ_PARAMS,
        K::Lpf => &LPF_PARAMS,
        K::Hpf => &HPF_PARAMS,
        K::Bpf => &BPF_PARAMS,
        K::Notch => &NOTCH_PARAMS,
        K::Comb => &COMB_PARAMS,
        K::Narrow => &NARROW_PARAMS,
        K::LinearPhaseEq | K::GroupDelayEq => &PHASE_EQ_PARAMS,
        K::Crossover => &CROSSOVER_PARAMS,
        _ => &[],
    }
}

/// The preferred delay memory of an eq/filter kind, in floats. Only
/// `comb` needs one; every other kind is biquad-only.
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    match kind {
        EffectKind::Comb => {
            let want = (COMB_TIME_MAX * sr).ceil();
            let per = if want.is_finite() && want > 0.0 {
                want as usize
            } else {
                1
            };
            per * 2
        }
        _ => 0,
    }
}

/// Sets up a zeroed eq/filter unit. Only `comb` carves delay memory;
/// every other kind sets its biquad coefficients fresh each block.
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, _sr: f32) {
    if kind == EffectKind::Comb {
        let per = mem_len / 2;
        let mut cursor = 0usize;
        st.dl[0] = DelayLine::carve(&mut cursor, mem_len, per);
        st.dl[1] = DelayLine::carve(&mut cursor, mem_len, mem_len - per);
    }
}

/// A single biquad shape applied identically to both channels
/// (`lpf`/`hpf`/`bpf`/`notch`): `p[0]` is cutoff (Hz), `p[1]` is `q`.
fn simple_filter(shape: Shape, p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let cutoff = pv(p, 0, 1000.0);
    let q = pv(p, 1, 0.707);
    st.bq[0].set(shape, cutoff, q, 0.0, sr);
    st.bq[1].set(shape, cutoff, q, 0.0, sr);
    let n = l.len().min(r.len());
    for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
        *lx = st.bq[0].run(*lx);
        *rx = st.bq[1].run(*rx);
    }
}

/// A low-shelf + high-shelf pair with a common corner and opposite gains
/// (`tilt`), or two independent shelves (`tone`, `loudness-eq`).
#[allow(clippy::too_many_arguments)]
fn shelf_pair(
    low_freq: f32,
    low_gain: f32,
    high_freq: f32,
    high_gain: f32,
    st: &mut FxState,
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    st.bq[0].set(Shape::LowShelf, low_freq, 0.707, low_gain, sr);
    st.bq[1].set(Shape::LowShelf, low_freq, 0.707, low_gain, sr);
    st.bq[2].set(Shape::HighShelf, high_freq, 0.707, high_gain, sr);
    st.bq[3].set(Shape::HighShelf, high_freq, 0.707, high_gain, sr);
    let n = l.len().min(r.len());
    for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
        let x = st.bq[0].run(*lx);
        *lx = st.bq[2].run(x);
        let y = st.bq[1].run(*rx);
        *rx = st.bq[3].run(y);
    }
}

/// The dynamic-eq gain reduction (dB, `<= 0`) for a channel's smoothed
/// envelope: no reduction until `db` exceeds `threshold`, ramping to
/// `-range` over the next 12 dB of overshoot.
fn dyn_gain(db: f32, threshold: f32, range: f32) -> f32 {
    let over = (db - threshold).max(0.0);
    let amt = (over / 12.0).clamp(0.0, 1.0);
    -range * amt
}

fn dynamic_eq_process(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = pv(p, 0, 1000.0);
    let q = pv(p, 1, 1.0);
    let threshold = pv(p, 2, -24.0);
    let range = pv(p, 3, 6.0);
    let n = l.len().min(r.len());
    let peak_l = l[..n].iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    let peak_r = r[..n].iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    let att = OnePole::time_coef(0.01, sr);
    let rel = OnePole::time_coef(0.2, sr);
    let env_l = st.env[0].run(peak_l, att, rel);
    let env_r = st.env[1].run(peak_r, att, rel);
    let gain_l = dyn_gain(gain_to_db(env_l), threshold, range);
    let gain_r = dyn_gain(gain_to_db(env_r), threshold, range);
    st.bq[0].set(Shape::Peak, freq, q, gain_l, sr);
    st.bq[1].set(Shape::Peak, freq, q, gain_r, sr);
    for x in &mut l[..n] {
        *x = st.bq[0].run(*x);
    }
    for x in &mut r[..n] {
        *x = st.bq[1].run(*x);
    }
}

/// `linear-phase-eq`/`group-delay-eq`: approximation — a minimum-phase
/// bass/mid/treble shelving+peaking cascade followed by a fixed allpass
/// stage that approximately flattens the cascade's phase response; no
/// FIR filter or lookahead is used (design 12.8.8).
fn phase_eq_process(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let bass = pv(p, 0, 0.0);
    let mid = pv(p, 1, 0.0);
    let treble = pv(p, 2, 0.0);
    st.bq[0].set(Shape::LowShelf, 150.0, 0.707, bass, sr);
    st.bq[1].set(Shape::LowShelf, 150.0, 0.707, bass, sr);
    st.bq[2].set(Shape::Peak, 1000.0, 0.9, mid, sr);
    st.bq[3].set(Shape::Peak, 1000.0, 0.9, mid, sr);
    st.bq[4].set(Shape::HighShelf, 6000.0, 0.707, treble, sr);
    st.bq[5].set(Shape::HighShelf, 6000.0, 0.707, treble, sr);
    st.bq[6].set(Shape::Allpass, 800.0, 0.7, 0.0, sr);
    st.bq[7].set(Shape::Allpass, 800.0, 0.7, 0.0, sr);
    let n = l.len().min(r.len());
    for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
        let mut x = *lx;
        x = st.bq[0].run(x);
        x = st.bq[2].run(x);
        x = st.bq[4].run(x);
        x = st.bq[6].run(x);
        *lx = x;
        let mut y = *rx;
        y = st.bq[1].run(y);
        y = st.bq[3].run(y);
        y = st.bq[5].run(y);
        y = st.bq[7].run(y);
        *rx = y;
    }
}

/// Processes one stereo block of an eq/filter kind in place, fully wet.
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
        K::Peq => {
            let lf = pv(p, 0, 80.0);
            let lg = pv(p, 1, 0.0);
            let m1f = pv(p, 2, 500.0);
            let m1g = pv(p, 3, 0.0);
            let m1q = pv(p, 4, 0.7);
            let m2f = pv(p, 5, 2000.0);
            let m2g = pv(p, 6, 0.0);
            let m2q = pv(p, 7, 0.7);
            let hf = pv(p, 8, 8000.0);
            let hg = pv(p, 9, 0.0);
            st.bq[0].set(Shape::LowShelf, lf, 0.707, lg, sr);
            st.bq[1].set(Shape::LowShelf, lf, 0.707, lg, sr);
            st.bq[2].set(Shape::Peak, m1f, m1q, m1g, sr);
            st.bq[3].set(Shape::Peak, m1f, m1q, m1g, sr);
            st.bq[4].set(Shape::Peak, m2f, m2q, m2g, sr);
            st.bq[5].set(Shape::Peak, m2f, m2q, m2g, sr);
            st.bq[6].set(Shape::HighShelf, hf, 0.707, hg, sr);
            st.bq[7].set(Shape::HighShelf, hf, 0.707, hg, sr);
            let n = l.len().min(r.len());
            for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let mut x = *lx;
                x = st.bq[0].run(x);
                x = st.bq[2].run(x);
                x = st.bq[4].run(x);
                x = st.bq[6].run(x);
                *lx = x;
                let mut y = *rx;
                y = st.bq[1].run(y);
                y = st.bq[3].run(y);
                y = st.bq[5].run(y);
                y = st.bq[7].run(y);
                *rx = y;
            }
        }
        K::Geq => {
            for (b, &f) in GEQ_BANDS.iter().enumerate() {
                let g = pv(p, b, 0.0);
                st.bq[b * 2].set(Shape::Peak, f, GEQ_Q, g, sr);
                st.bq[b * 2 + 1].set(Shape::Peak, f, GEQ_Q, g, sr);
            }
            let n = l.len().min(r.len());
            for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                let mut x = *lx;
                let mut y = *rx;
                for b in 0..GEQ_BANDS.len() {
                    x = st.bq[b * 2].run(x);
                    y = st.bq[b * 2 + 1].run(y);
                }
                *lx = x;
                *rx = y;
            }
        }
        K::DynamicEq => dynamic_eq_process(p, st, l, r, sr),
        K::Tilt => {
            let freq = pv(p, 0, 1000.0);
            let gain = pv(p, 1, 0.0);
            let half = gain * 0.5;
            shelf_pair(freq, -half, freq, half, st, l, r, sr);
        }
        K::Tone => {
            let bass = pv(p, 0, 0.0);
            let treble = pv(p, 1, 0.0);
            shelf_pair(150.0, bass, 3000.0, treble, st, l, r, sr);
        }
        K::LoudnessEq => {
            let level = pv(p, 0, 0.0);
            let boost = (-level).clamp(0.0, 24.0);
            shelf_pair(80.0, boost, 8000.0, boost * 0.5, st, l, r, sr);
        }
        K::Lpf => simple_filter(Shape::Lowpass, p, st, l, r, sr),
        K::Hpf => simple_filter(Shape::Highpass, p, st, l, r, sr),
        K::Bpf => simple_filter(Shape::Bandpass, p, st, l, r, sr),
        K::Notch => simple_filter(Shape::Notch, p, st, l, r, sr),
        K::Comb => {
            let time = pv(p, 0, 0.01);
            let feedback = pv(p, 1, 0.5).clamp(-0.9, 0.9);
            let delay_samples = (time * sr).max(1.0);
            let n = l.len().min(r.len());
            for i in 0..n {
                let dxl = st.dl[0].read(mem, delay_samples);
                let vl = l[i] + feedback * dxl;
                st.dl[0].write(mem, vl);
                l[i] = vl;
                let dxr = st.dl[1].read(mem, delay_samples);
                let vr = r[i] + feedback * dxr;
                st.dl[1].write(mem, vr);
                r[i] = vr;
            }
        }
        K::Narrow => {
            let freq = pv(p, 0, 1000.0);
            let width = pv(p, 1, 0.1).max(0.01);
            let q = (1.0 / width).clamp(0.1, 20.0);
            st.bq[0].set(Shape::Bandpass, freq, q, 0.0, sr);
            st.bq[1].set(Shape::Bandpass, freq, q, 0.0, sr);
            let n = l.len().min(r.len());
            for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                *lx = st.bq[0].run(*lx);
                *rx = st.bq[1].run(*rx);
            }
        }
        K::LinearPhaseEq | K::GroupDelayEq => phase_eq_process(p, st, l, r, sr),
        K::Crossover => {
            let freq = pv(p, 0, 1000.0);
            let band = pv(p, 1, 0.0);
            let shape = if band >= 0.5 {
                Shape::Highpass
            } else {
                Shape::Lowpass
            };
            st.bq[0].set(shape, freq, 0.707, 0.0, sr);
            st.bq[1].set(shape, freq, 0.707, 0.0, sr);
            let n = l.len().min(r.len());
            for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                *lx = st.bq[0].run(*lx);
                *rx = st.bq[1].run(*rx);
            }
        }
        _ => {}
    }
}
