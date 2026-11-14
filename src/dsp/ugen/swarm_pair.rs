//! Plaits position 16 eight-voice swarm (MIT source-stage translation).
//!
//! The grain schedule, ranked spread, two-sample BLEP saw and sine auxiliary
//! are implemented with fixed state. See THIRD_PARTY_NOTICES.md.

use std::f32::consts::TAU;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const VOICES: usize = 8;
const STRIDE: usize = 14;
/// Required fixed floats for one output node; a two-output voice needs 224.
pub const STATE_FLOATS: usize = VOICES * STRIDE;

const FROM: usize = 0;
const INTERVAL: usize = 1;
const GRAIN_PHASE: usize = 2;
const FM: usize = 3;
const AMP: usize = 4;
const PREV_SIZE: usize = 5;
const FILTER_COEFF: usize = 6;
const SAW_PHASE: usize = 7;
const SAW_NEXT: usize = 8;
const SAW_FREQ: usize = 9;
const SAW_GAIN: usize = 10;
const SINE_PHASE: usize = 11;
const SINE_FREQ: usize = 12;
const SINE_GAIN: usize = 13;

#[inline]
fn unit(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

#[inline]
fn step_grain(state: &mut [f32], density: f32, burst: bool, start: bool, rng: &mut Rng) {
    let randomize = if start {
        state[GRAIN_PHASE] = 0.5;
        state[FM] = 16.0;
        true
    } else {
        state[GRAIN_PHASE] += density * state[FM];
        if state[GRAIN_PHASE] >= 1.0 {
            state[GRAIN_PHASE] = state[GRAIN_PHASE].fract();
            true
        } else {
            false
        }
    };
    if randomize {
        state[FROM] += state[INTERVAL];
        state[INTERVAL] = rng.unit() - state[FROM];
        if burst {
            state[FM] *= 0.8 + 0.2 * rng.unit();
        } else {
            state[FM] = 0.5 + 1.5 * rng.unit();
        }
    }
}

#[inline]
fn grain_amplitude(state: &mut [f32], size: f32) -> f32 {
    let target = if size < 1.0 {
        1.0
    } else {
        let phase = ((state[GRAIN_PHASE] - 0.5) * size).clamp(-1.0, 1.0);
        0.5 * ((TAU * (0.5 * phase + 1.25)).sin() + 1.0)
    };
    if (size >= 1.0) != (state[PREV_SIZE] >= 1.0) {
        state[FILTER_COEFF] = 0.5;
    }
    state[FILTER_COEFF] *= 0.95;
    state[PREV_SIZE] = size;
    state[AMP] += (0.5 - state[FILTER_COEFF]) * (target - state[AMP]);
    state[AMP]
}

#[inline]
fn saw_sample(state: &mut [f32], frequency: f32, gain: f32) -> f32 {
    let current = state[SAW_NEXT];
    state[SAW_NEXT] = 0.0;
    state[SAW_PHASE] += frequency;
    let mut current = current;
    if state[SAW_PHASE] >= 1.0 {
        state[SAW_PHASE] -= 1.0;
        let t = state[SAW_PHASE] / frequency.max(1.0e-8);
        current -= 0.5 * t * t;
        state[SAW_NEXT] -= -0.5 * (1.0 - t) * (1.0 - t);
    }
    state[SAW_NEXT] += state[SAW_PHASE];
    (2.0 * current - 1.0) * gain
}

#[inline]
fn sine_sample(state: &mut [f32], frequency: f32, gain: f32) -> f32 {
    state[SINE_PHASE] = (state[SINE_PHASE] + frequency).fract();
    (TAU * state[SINE_PHASE]).cos() * gain
}

/// Mode 0 emits the BLEP-corrected saw sum; mode 1 emits the analytic-sine
/// sum. Each node independently stores all eight grains, so main and aux
/// remain separately addressable through the existing `aux-out` contract.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    out.fill(0.0);
    if out.is_empty() {
        return;
    }
    let sr = kx.sr.max(1.0);
    let raw_freq = ins[0].first();
    let freq = if raw_freq.is_finite() {
        raw_freq.clamp(20.0, sr * 0.4)
    } else {
        220.0
    };
    let spread = unit(ins[1].first(), 0.5).powi(3);
    let timbre = unit(ins[2].first(), 0.5);
    let morph = unit(ins[3].first(), 0.5);
    let sine = ins[4].first() >= 0.5;
    let continuous = unit(ins[5].first(), 0.0) >= 0.5;
    let first_block = st.u[1] == 0;
    if first_block {
        for voice in mem[..STATE_FLOATS].chunks_exact_mut(STRIDE) {
            voice[INTERVAL] = 1.0;
            voice[GRAIN_PHASE] = 1.0;
            voice[AMP] = 0.5;
            voice[SAW_FREQ] = 0.01;
        }
        st.u[0] = Rng::new(kx.seed ^ 0x5A12_6D89).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    // Source NoteToFrequency(timbre * 120) converted from normalized 48 kHz
    // to the host rate, then advanced once per block.
    let density_hz = 8.175_799 * 2.0_f32.powf(timbre * 10.0);
    #[allow(clippy::cast_precision_loss)]
    let block = out.len() as f32;
    let density = density_hz / sr * 0.025 * block;
    let mut size_ratio = 0.25 * 2.0_f32.powf((1.0 - morph) * 7.0);
    for (index, voice) in mem[..STATE_FLOATS].chunks_exact_mut(STRIDE).enumerate() {
        step_grain(
            voice,
            density,
            !continuous,
            first_block && !continuous,
            &mut rng,
        );
        let amplitude = grain_amplitude(voice, size_ratio) / VOICES as f32;
        let expo = if size_ratio < 1.0 {
            2.0 * (voice[FROM] + voice[INTERVAL] * voice[GRAIN_PHASE]) - 1.0
        } else {
            voice[FROM]
        };
        #[allow(clippy::cast_precision_loss)]
        let rank = (index as f32 - 3.5) / 3.5;
        let detune = 2.0_f32.powf(4.0 * expo * spread * rank);
        let linear = 1.0 + rank * (rank + 0.01) * spread * 0.25;
        let target_freq = (freq * detune * linear / sr).clamp(0.0, 0.25);
        if sine {
            let target_gain = if target_freq >= 0.25 {
                0.0
            } else {
                amplitude * (1.0 - 4.0 * target_freq)
            };
            let start_freq = voice[SINE_FREQ];
            let start_gain = voice[SINE_GAIN];
            for (j, sample) in out.iter_mut().enumerate() {
                #[allow(clippy::cast_precision_loss)]
                let t = (j + 1) as f32 / block;
                let f = start_freq + (target_freq - start_freq) * t;
                let g = start_gain + (target_gain - start_gain) * t;
                *sample += sine_sample(voice, f, g);
            }
            voice[SINE_FREQ] = target_freq;
            voice[SINE_GAIN] = target_gain;
        } else {
            let start_freq = voice[SAW_FREQ];
            let start_gain = voice[SAW_GAIN];
            for (j, sample) in out.iter_mut().enumerate() {
                #[allow(clippy::cast_precision_loss)]
                let t = (j + 1) as f32 / block;
                let f = start_freq + (target_freq - start_freq) * t;
                let g = start_gain + (amplitude - start_gain) * t;
                *sample += saw_sample(voice, f, g);
            }
            voice[SAW_FREQ] = target_freq;
            voice[SAW_GAIN] = amplitude;
        }
        size_ratio *= 0.97;
    }
    st.u[0] = rng.s;
    if (!continuous && st.s[1] > 4.0) || (continuous && kx.gate == 0) {
        st.finish();
    }
    st.s[1] += block / sr;
    for sample in out {
        *sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
}
