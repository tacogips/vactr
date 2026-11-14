//! The reverb group (design-music 5 "reverb", design-implementation 12.5,
//! 12.8.8): `plate`, `fdn`, `convolution`, `scatter`, `room`.
//!
//! `plate`, `fdn` and `room` share a 4-line feedback-delay network
//! (`run_fdn4`) with a Householder feedback matrix, per-line damping and a
//! decay gain derived from the `decay` parameter, differing only in their
//! delay-line ratios and defaults. `scatter` chains allpass diffusers
//! around a single scalar feedback path per channel. `convolution` is a
//! bounded-cost FIR approximation (documented on `run_convolution`).

use super::prim::{allpass, clampf, DelayLine, OnePole};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

/// The largest `size` scale factor (multiplies the base delay-line ratios).
const MAX_SCALE: f32 = 2.0;
/// The largest predelay (`plate`), in seconds.
const MAX_PREDELAY_S: f32 = 0.1;
/// The base delay-line lengths (ms) for `plate`/`fdn` (a larger, more
/// diffuse hall-like set) and `room` (a tighter, room-like set).
const HALL_MS: [f32; 4] = [29.7, 37.1, 41.3, 47.9];
const ROOM_MS: [f32; 4] = [13.1, 17.3, 19.7, 23.9];
/// The allpass diffuser lengths (ms) for `scatter`, per channel.
const SCATTER_MS: [f32; 4] = [4.3, 6.1, 9.7, 13.3];
/// The most taps the `convolution` FIR approximation reads from an IR.
const MAX_TAPS: usize = 512;
/// The convolution input history kept per channel, in floats.
const CONV_HISTORY: usize = 2048;

const PLATE_PARAMS: &[ParamDef] = &[
    ParamDef::new("size", 0.6, 0.0, 1.0),
    ParamDef::unit("decay", 1.8, 0.1, 10.0, "s"),
    ParamDef::new("damping", 0.5, 0.0, 1.0),
    ParamDef::unit("predelay", 0.02, 0.0, MAX_PREDELAY_S, "s"),
    ParamDef::new("mix", 0.2, 0.0, 1.0),
];

const FDN_PARAMS: &[ParamDef] = &[
    ParamDef::new("size", 0.6, 0.0, 1.0),
    ParamDef::unit("decay", 2.5, 0.1, 10.0, "s"),
    ParamDef::new("damping", 0.4, 0.0, 1.0),
    ParamDef::new("mix", 0.25, 0.0, 1.0),
];

const ROOM_PARAMS: &[ParamDef] = &[
    ParamDef::new("size", 0.5, 0.0, 1.0),
    ParamDef::unit("decay", 1.2, 0.1, 6.0, "s"),
    ParamDef::new("damping", 0.6, 0.0, 1.0),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];

const SCATTER_PARAMS: &[ParamDef] = &[
    ParamDef::new("density", 0.5, 0.0, 1.0),
    ParamDef::new("size", 0.5, 0.0, 1.0),
    ParamDef::new("decay", 0.5, 0.0, 0.95),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];

const CONVOLUTION_PARAMS: &[ParamDef] = &[
    ParamDef::new("ir", -1.0, -1.0, 1.0e6),
    ParamDef::unit("gain", 0.0, -24.0, 24.0, "dB"),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];

/// The named parameters of a reverb-group kind.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Plate => PLATE_PARAMS,
        K::Fdn => FDN_PARAMS,
        K::Room => ROOM_PARAMS,
        K::Scatter => SCATTER_PARAMS,
        K::Convolution => CONVOLUTION_PARAMS,
        _ => &[],
    }
}

/// The floats needed for a 4-line network at `base_ms` ratios, scaled by
/// the largest `size` the unit can reach.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn fdn4_capacity(base_ms: [f32; 4], sr: f32) -> usize {
    base_ms
        .iter()
        .map(|ms| (ms / 1000.0 * MAX_SCALE * sr) as usize + 4)
        .sum()
}

/// The preferred delay memory of a kind, in floats.
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    use EffectKind as K;
    let sr = sr.max(1.0);
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    match kind {
        K::Plate => fdn4_capacity(HALL_MS, sr) + (MAX_PREDELAY_S * sr) as usize + 4,
        K::Fdn => fdn4_capacity(HALL_MS, sr),
        K::Room => fdn4_capacity(ROOM_MS, sr),
        K::Scatter => {
            2 * SCATTER_MS
                .iter()
                .map(|ms| (ms / 1000.0 * MAX_SCALE * sr) as usize + 4)
                .sum::<usize>()
        }
        K::Convolution => 2 * CONV_HISTORY,
        _ => 0,
    }
}

/// Carves the four FDN lines (and, if `predelay` is true, a fifth line for
/// it) out of `mem_len` floats.
fn carve_fdn4(base_ms: [f32; 4], sr: f32, mem_len: usize, predelay: bool) -> [DelayLine; 5] {
    let mut cursor = 0usize;
    let mut lines = [DelayLine::default(); 5];
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    for (i, ms) in base_ms.iter().enumerate() {
        let want = (ms / 1000.0 * MAX_SCALE * sr) as usize + 4;
        lines[i] = DelayLine::carve(&mut cursor, mem_len, want);
    }
    if predelay {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let want = (MAX_PREDELAY_S * sr) as usize + 4;
        lines[4] = DelayLine::carve(&mut cursor, mem_len, want);
    }
    lines
}

/// Sets up a zeroed unit for a reverb-group kind.
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, sr: f32) {
    use EffectKind as K;
    let sr = sr.max(1.0);
    match kind {
        K::Plate => {
            let lines = carve_fdn4(HALL_MS, sr, mem_len, true);
            st.dl[..5].copy_from_slice(&lines);
        }
        K::Fdn => {
            let lines = carve_fdn4(HALL_MS, sr, mem_len, false);
            st.dl[..4].copy_from_slice(&lines[..4]);
        }
        K::Room => {
            let lines = carve_fdn4(ROOM_MS, sr, mem_len, false);
            st.dl[..4].copy_from_slice(&lines[..4]);
        }
        K::Scatter => {
            let mut cursor = 0usize;
            #[allow(
                clippy::cast_sign_loss,
                clippy::cast_possible_truncation,
                clippy::needless_range_loop
            )]
            for k in 0..4 {
                let want = (SCATTER_MS[k] / 1000.0 * MAX_SCALE * sr) as usize + 4;
                st.dl[k] = DelayLine::carve(&mut cursor, mem_len, want);
                st.dl[k + 4] = DelayLine::carve(&mut cursor, mem_len, want);
            }
        }
        K::Convolution => {
            let mut cursor = 0usize;
            let half = mem_len / 2;
            st.dl[0] = DelayLine::carve(&mut cursor, mem_len, half);
            st.dl[1] = DelayLine::carve(&mut cursor, mem_len, mem_len - half);
        }
        _ => {}
    }
}

/// A 4-line Householder FDN shared by `plate`, `fdn` and `room`.
#[allow(clippy::too_many_arguments)]
fn run_fdn4(
    base_ms: [f32; 4],
    predelay: bool,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let size = clampf(p.first().copied().unwrap_or(0.6), 0.0, 1.0);
    let decay_s = p.get(1).copied().unwrap_or(1.8).max(0.05);
    let damping = clampf(p.get(2).copied().unwrap_or(0.5), 0.0, 1.0);
    let predelay_s = if predelay {
        p.get(3).copied().unwrap_or(0.0).max(0.0)
    } else {
        0.0
    };
    let scale = 0.3 + size * (MAX_SCALE - 0.3);
    let mut d = [0.0f32; 4];
    let mut g = [0.0f32; 4];
    for k in 0..4 {
        let dk = (base_ms[k] / 1000.0 * scale * sr).max(2.0);
        d[k] = dk;
        // -60 dB (ln(1000) = 6.9078) after `decay_s` seconds of round trips.
        g[k] = (-6.9078 * (dk / sr) / decay_s).exp().min(0.95);
    }
    let damp_hz = 20_000.0 - damping * 19_200.0;
    let damp_a = OnePole::coef(damp_hz, sr);
    let predelay_samples = (predelay_s * sr).max(1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let (xl, xr) = if predelay {
            st.dl[4].write(mem, (l[i] + r[i]) * 0.5);
            let pd = st.dl[4].read(mem, predelay_samples);
            (l[i] * 0.4 + pd * 0.1, r[i] * 0.4 + pd * 0.1)
        } else {
            (l[i], r[i])
        };
        let mut t = [0.0f32; 4];
        for k in 0..4 {
            t[k] = st.op[k].lp(st.dl[k].read(mem, d[k]), damp_a);
        }
        let sum: f32 = t.iter().sum();
        let inject = [xl * 0.5, xr * 0.5, xl * 0.5, xr * 0.5];
        for k in 0..4 {
            let mixed = t[k] - 0.5 * sum;
            st.dl[k].write(mem, inject[k] + mixed * g[k]);
        }
        l[i] = (t[0] + t[2]) * 0.5;
        r[i] = (t[1] + t[3]) * 0.5;
    }
}

/// A diffuser: a chain of four allpasses per channel, with a single scalar
/// feedback path (`st.s[0]`/`st.s[1]`) around the whole chain so the loop
/// gain equals `decay` exactly (the allpasses themselves are lossless).
fn run_scatter(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let density = clampf(p.first().copied().unwrap_or(0.5), 0.0, 1.0);
    let size = clampf(p.get(1).copied().unwrap_or(0.5), 0.0, 1.0);
    let decay = clampf(p.get(2).copied().unwrap_or(0.5), 0.0, 0.95);
    let scale = 0.3 + size * (MAX_SCALE - 0.3);
    let g = 0.2 + density * 0.5;
    let mut d = [0usize; 4];
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    for k in 0..4 {
        d[k] = ((SCATTER_MS[k] / 1000.0 * scale * sr) as usize).max(2);
    }
    let n = l.len().min(r.len());
    for i in 0..n {
        let mut xl = l[i] + st.s[0] * decay;
        #[allow(clippy::needless_range_loop)]
        for k in 0..4 {
            xl = allpass(&mut st.dl[k], mem, xl, d[k], g);
        }
        st.s[0] = xl;
        l[i] = xl;

        let mut xr = r[i] + st.s[1] * decay;
        #[allow(clippy::needless_range_loop)]
        for k in 0..4 {
            xr = allpass(&mut st.dl[k + 4], mem, xr, d[k], g);
        }
        st.s[1] = xr;
        r[i] = xr;
    }
}

/// A time-domain FIR approximation of convolution with an installed
/// impulse response. The IR (channel 0, or the matching channel when
/// stereo) is decimated to at most [`MAX_TAPS`] taps: tap `k` reads IR
/// sample `k * stride` (`stride = ceil(ir_frames / MAX_TAPS)`) and is
/// scaled by `stride`, approximating the integral of each skipped
/// `stride`-sample window by its first sample. Input history is kept in a
/// per-channel `DelayLine` of [`CONV_HISTORY`] floats; taps beyond that
/// history alias to the oldest sample still held (a bounded, graceful
/// degradation for long IRs on a small `mem` region). The output gain is
/// normalized so it cannot exceed the IR's scaled absolute sum, and the
/// final sample is clamped to +-8.
fn run_convolution(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let ir_id = p.first().copied().unwrap_or(-1.0);
    let gain = super::prim::db_to_gain(p.get(1).copied().unwrap_or(0.0));
    let n = l.len().min(r.len());
    if ir_id < 0.0 {
        return; // No IR installed: pass the input through as wet.
    }
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let resource = ir_id.max(0.0) as u32;
    let Some(ir) = ctx.store.get(resource) else {
        return;
    };
    let frames = ir.frames();
    if frames == 0 {
        return;
    }
    let channels = usize::from(ir.channels.max(1));
    let stride = frames.div_ceil(MAX_TAPS).max(1);
    let taps = frames.div_ceil(stride).min(MAX_TAPS);
    #[allow(clippy::cast_precision_loss)]
    let stride_gain = stride as f32;
    for i in 0..n {
        st.dl[0].write(mem, l[i]);
        st.dl[1].write(mem, r[i]);
        let mut outl = 0.0f32;
        let mut outr = 0.0f32;
        for k in 0..taps {
            let idx = k * stride;
            if idx >= frames {
                break;
            }
            let ch_l = idx * channels;
            let ch_r = idx * channels + (channels.min(2) - 1);
            let hl = ir.data.get(ch_l).copied().unwrap_or(0.0) * stride_gain;
            let hr = ir.data.get(ch_r).copied().unwrap_or(0.0) * stride_gain;
            let d = idx + 1;
            outl += hl * st.dl[0].tap(mem, d);
            outr += hr * st.dl[1].tap(mem, d);
        }
        l[i] = clampf(outl * gain, -8.0, 8.0);
        r[i] = clampf(outr * gain, -8.0, 8.0);
    }
}

/// Processes one stereo block for a reverb-group kind, fully wet.
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
        K::Plate => run_fdn4(HALL_MS, true, p, st, mem, l, r, sr),
        K::Fdn => run_fdn4(HALL_MS, false, p, st, mem, l, r, sr),
        K::Room => run_fdn4(ROOM_MS, false, p, st, mem, l, r, sr),
        K::Scatter => run_scatter(p, st, mem, l, r, sr),
        K::Convolution => run_convolution(p, st, mem, l, r, ctx),
        _ => {}
    }
}
