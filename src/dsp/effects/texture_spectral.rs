//! Bounded stereo spectral texture informed by the MIT Clouds PVOC mode.
//! Four 512-point spectral snapshots, analytic Hann windows and host-rate
//! overlap-add replace source texture history, phase handling and tables.
//! This is an adaptation; see THIRD_PARTY_NOTICES.md.

use std::f32::consts::TAU;

use super::texture::{self, read_ring};
use super::{FxCtx, FxState};
use crate::dsp::fft::FFT_SIZE;

const N: usize = FFT_SIZE;
const HOP: usize = N / 4;
const BINS: usize = N / 2;
const HISTORY: usize = 4;
const HIST_FRAME: usize = 2 * BINS;
const FIXED: usize = 2 * N + 2 * HISTORY * HIST_FRAME;

#[must_use]
pub fn mem_len(sr: f32) -> usize {
    let capture = (sr.clamp(8_000.0, 192_000.0) * texture::CAPTURE_SECONDS).ceil() as usize;
    2 * capture + FIXED
}

pub fn init(st: &mut FxState, mem: &mut [f32]) {
    texture::init(st, mem);
}

fn window(i: usize) -> f32 {
    0.5 - 0.5 * (TAU * i as f32 / N as f32).cos()
}

#[allow(clippy::too_many_arguments)]
fn spectral_frame(
    p: &[f32],
    st: &mut FxState,
    capture: &[f32],
    ola: &mut [f32],
    history: &mut [f32],
    frames: usize,
    write: usize,
    valid: usize,
    cursor: usize,
    ctx: &mut FxCtx<'_>,
) {
    if valid < N || ctx.fft.len() != N || ctx.scratch.len() < 2 * N {
        return;
    }
    let (re, rest) = ctx.scratch.split_at_mut(N);
    let im = &mut rest[..N];
    let next = st.s[5] as usize % HISTORY;
    let count = (st.s[4] as usize).min(HISTORY);
    let frame_number = st.s[12] as usize;
    let refresh_period = 1 + ((1.0 - p[3]) * 3.0) as usize;
    let record = st.s[13] >= 0.5 || (p[9] < 0.5 && frame_number % refresh_period == 0);
    if record {
        let max_back = valid - N;
        let back = (p[0] * max_back as f32) as usize;
        for channel in 0..2 {
            let source = &capture[channel * frames..(channel + 1) * frames];
            for i in 0..N {
                re[i] = read_ring(source, (write + frames - N - back + i) as f32) * window(i);
                im[i] = 0.0;
            }
            ctx.fft.forward(re, im);
            let base = (channel * HISTORY + next) * HIST_FRAME;
            let (mag, phase) = history[base..base + HIST_FRAME].split_at_mut(BINS);
            for k in 0..BINS {
                mag[k] = (re[k] * re[k] + im[k] * im[k]).sqrt();
                phase[k] = im[k].atan2(re[k]);
            }
        }
        st.s[5] = ((next + 1) % HISTORY) as f32;
        st.s[4] = (count + 1).min(HISTORY) as f32;
        st.s[13] = 0.0;
    }
    let count = (st.s[4] as usize).min(HISTORY);
    if count == 0 {
        return;
    }
    let newest = ((st.s[5] as usize) + HISTORY - 1) % HISTORY;
    let back = (p[0] * count.saturating_sub(1) as f32).round() as usize;
    let selected = (newest + HISTORY - back) % HISTORY;
    let ratio = 2.0f32.powf(p[2] / 12.0);
    let warp = 0.55 + p[1] * 1.8;
    for channel in 0..2 {
        let base = (channel * HISTORY + selected) * HIST_FRAME;
        let (mag, phase) = history[base..base + HIST_FRAME].split_at(BINS);
        let max_mag = mag.iter().copied().fold(0.0, f32::max);
        let quantum = max_mag * (0.001 + p[4] * p[4] * 0.14) + 1.0e-8;
        re.fill(0.0);
        im.fill(0.0);
        for k in 1..BINS {
            let normalized = k as f32 / (BINS - 1) as f32;
            let source = normalized.powf(warp) * (BINS - 1) as f32 / ratio;
            if source >= (BINS - 1) as f32 {
                continue;
            }
            let a = source.floor() as usize;
            let b = (a + 1).min(BINS - 1);
            let magnitude = mag[a] + (mag[b] - mag[a]) * source.fract();
            let quantized = (magnitude / quantum).round() * quantum;
            let glitch = p[11] < 0.5 && k % 5 == 0;
            let amount = if glitch { 0.0 } else { quantized };
            let phase_drift = TAU * k as f32 * HOP as f32 * frame_number as f32 / N as f32;
            let decorrelation = st.rng.bipolar() * p[3] * p[3] * 0.35;
            let angle = phase[a] + phase_drift + decorrelation;
            re[k] = amount * angle.cos();
            im[k] = amount * angle.sin();
            re[N - k] = re[k];
            im[N - k] = -im[k];
        }
        ctx.fft.inverse(re, im);
        let out = &mut ola[channel * N..(channel + 1) * N];
        for (i, sample) in re.iter().copied().enumerate().take(N) {
            let at = (cursor + i) % N;
            out[at] += sample * window(i) * (2.0 / 3.0);
        }
    }
}

/// Preallocated stereo STFT capture, spectral replay and overlap-add.
pub fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    if mem.len() < mem_len(ctx.sr) || p.len() < texture::PARAMS.len() {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let frames = (mem.len() - FIXED) / 2;
    let (capture, rest) = mem.split_at_mut(2 * frames);
    let (ola, history) = rest.split_at_mut(2 * N);
    let mut write = st.s[0] as usize % frames;
    let mut valid = (st.s[1] as usize).min(frames);
    let mut hop = st.s[2] as usize;
    let mut cursor = st.s[3] as usize % N;
    let mut prev_trigger = st.s[6] >= 0.5;
    let mut previous_l = st.s[7];
    let mut previous_r = st.s[8];
    let mut tail_l = st.s[9];
    let mut tail_r = st.s[10];
    let quality_phase = super::texture_quality::begin(p, st, l, r);
    let reverb_decay = (-1.0 / (ctx.sr * (0.025 + p[7] * 0.24))).exp();
    for (out_l, out_r) in l.iter_mut().zip(r.iter_mut()) {
        let wet_l = ola[cursor];
        let wet_r = ola[N + cursor];
        ola[cursor] = 0.0;
        ola[N + cursor] = 0.0;
        if p[9] < 0.5 {
            capture[write] = (*out_l + p[6] * previous_l).clamp(-4.0, 4.0);
            capture[frames + write] = (*out_r + p[6] * previous_r).clamp(-4.0, 4.0);
            write = (write + 1) % frames;
            valid = (valid + 1).min(frames);
        }
        let triggered = p[10] >= 0.5 && !prev_trigger;
        prev_trigger = p[10] >= 0.5;
        if triggered {
            st.s[4] = 0.0;
            st.s[5] = 0.0;
            st.s[13] = 1.0;
            hop = HOP;
        } else {
            hop += 1;
        }
        cursor = (cursor + 1) % N;
        if hop >= HOP {
            hop = 0;
            spectral_frame(
                p, st, capture, ola, history, frames, write, valid, cursor, ctx,
            );
            // Phase drift repeats every four hops; refresh periods are 1..4.
            // Their least common multiple is 12, exactly representable in f32.
            st.s[12] = (st.s[12] + 1.0).rem_euclid(12.0);
        }
        let mid = (wet_l + wet_r) * 0.5;
        let widened_l = wet_l + p[5] * 0.35 * (wet_l - mid);
        let widened_r = wet_r + p[5] * 0.35 * (wet_r - mid);
        tail_l = tail_l * reverb_decay + widened_r * (1.0 - reverb_decay);
        tail_r = tail_r * reverb_decay + widened_l * (1.0 - reverb_decay);
        previous_l = widened_l + p[7] * (tail_l * 2.0 + tail_r) * 0.5;
        previous_r = widened_r + p[7] * (tail_r * 2.0 + tail_l) * 0.5;
        *out_l = previous_l.clamp(-4.0, 4.0);
        *out_r = previous_r.clamp(-4.0, 4.0);
    }
    st.s[0] = write as f32;
    st.s[1] = valid as f32;
    st.s[2] = hop as f32;
    st.s[3] = cursor as f32;
    st.s[6] = if prev_trigger { 1.0 } else { 0.0 };
    st.s[7] = previous_l;
    st.s[8] = previous_r;
    st.s[9] = tail_l;
    st.s[10] = tail_r;
    super::texture_quality::end(quality_phase, st, l, r);
}
