//! Modal kalimba tine synthesis based on clamped-free beam ratios.
//!
//! The modal gains are an original tuning, and the recurrence is written from
//! the complex-rotation form of a damped sinusoidal mode.

use std::f32::consts::{PI, TAU};

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const MODES: usize = 5;
const MODE_STATE: usize = 10;
const BODY_STATE: usize = MODE_STATE;
const BUZZ_STATE: usize = BODY_STATE + 8;
const PLUCK_COUNTER: usize = BUZZ_STATE + 4;
const FUNDAMENTAL_REF: usize = PLUCK_COUNTER + 1;
const NORMALIZATION: f32 = 160.0;
// Output-referred squared amplitude threshold for -80 dB.
const FINISH_ENERGY: f32 = 1.0e-8 / (NORMALIZATION * NORMALIZATION);
const RATIOS: [f32; 4] = [1.0, 6.2669, 17.5475, 34.3861];
// Tuned original modal gains: progressively quieter beam flexure modes.
const MODE_GAINS: [f32; 4] = [1.0, 0.45, 0.2, 0.1];

/// Number of ports in the pinned contract.
pub const PORT_COUNT: usize = 8;
/// Stable port names and defaults; order is part of the template contract.
pub const PORTS: [(&str, f32); PORT_COUNT] = [
    ("freq", 440.0),
    ("kalimba-beat", 1.5),
    ("kalimba-hardness", 0.5),
    ("kalimba-decay", 2.5),
    ("kalimba-damping", 0.5),
    ("kalimba-body", 0.3),
    ("kalimba-buzz", 0.0),
    ("velocity", 1.0),
];

/// Stable indices for the named ports.
pub mod port {
    pub const FREQ: usize = 0;
    pub const KALIMBA_BEAT: usize = 1;
    pub const KALIMBA_HARDNESS: usize = 2;
    pub const KALIMBA_DECAY: usize = 3;
    pub const KALIMBA_DAMPING: usize = 4;
    pub const KALIMBA_BODY: usize = 5;
    pub const KALIMBA_BUZZ: usize = 6;
    pub const VELOCITY: usize = 7;
}

/// Five complex modes, two body biquads, one buzz biquad and two counters.
pub const STATE_FLOATS: usize = FUNDAMENTAL_REF + 1;

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
fn param(value: f32, fallback: f32, low: f32, high: f32) -> f32 {
    finite_or(value, fallback).clamp(low, high)
}

#[inline]
fn clean(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[inline]
fn set_bandpass_coefficients(freq: f32, q: f32, sr: f32) -> [f32; 5] {
    let omega = TAU * freq.clamp(10.0, sr * 0.49) / sr;
    let (sin, cos) = omega.sin_cos();
    let alpha = sin / (2.0 * q);
    let a0 = 1.0 + alpha;
    [
        alpha / a0,
        0.0,
        -alpha / a0,
        -2.0 * cos / a0,
        (1.0 - alpha) / a0,
    ]
}

#[inline]
fn biquad(input: f32, state: &mut [f32], coeff: &[f32; 5]) -> f32 {
    let output = coeff[0] * input + coeff[1] * state[0] + coeff[2] * state[1]
        - coeff[3] * state[2]
        - coeff[4] * state[3];
    state[1] = state[0];
    state[0] = input;
    state[3] = state[2];
    state[2] = clean(output);
    clean(output)
}

#[inline]
fn modal_sample(state: &mut [f32], frequency: f32, decay: f32, excitation: f32, sr: f32) -> f32 {
    if frequency >= 0.45 * sr {
        state[0] = 0.0;
        state[1] = 0.0;
        return 0.0;
    }
    let radius = (-3.0 * std::f32::consts::LN_10 / (sr * decay)).exp();
    let (sin, cos) = (TAU * frequency / sr).sin_cos();
    let next_real = radius * (cos * state[0] - sin * state[1]) + (1.0 - radius) * excitation;
    let next_imag = radius * (sin * state[0] + cos * state[1]);
    state[0] = clean(next_real);
    state[1] = clean(next_imag);
    state[0]
}

/// Render a self-enveloped modal kalimba tine voice.
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
    let sr = finite_or(kx.sr, 48_000.0).max(1.0);
    let freq = finite_or(ins[port::FREQ].first(), 440.0).max(1.0);
    let beat = param(ins[port::KALIMBA_BEAT].first(), 1.5, 0.0, 8.0);
    let hardness = param(ins[port::KALIMBA_HARDNESS].first(), 0.5, 0.0, 1.0);
    let decay = param(ins[port::KALIMBA_DECAY].first(), 2.5, 0.1, 10.0);
    let damping = param(ins[port::KALIMBA_DAMPING].first(), 0.5, 0.0, 1.0);
    let body = param(ins[port::KALIMBA_BODY].first(), 0.3, 0.0, 1.0);
    let buzz = param(ins[port::KALIMBA_BUZZ].first(), 0.0, 0.0, 1.0);
    let velocity = param(ins[port::VELOCITY].first(), 1.0, 0.0, 1.0);
    let width = (6.0e-3_f32.ln() + hardness * (0.5e-3_f32.ln() - 6.0e-3_f32.ln())).exp();
    let pulse_samples = (width * sr).ceil().max(1.0);
    let exponent = 0.5 + 1.5 * damping;
    let mut mode_freqs = [freq; MODES];
    let mut mode_decays = [decay; MODES];
    for mode in 1..4 {
        mode_freqs[mode] = freq * RATIOS[mode];
        mode_decays[mode] = decay / RATIOS[mode].powf(exponent);
    }
    mode_freqs[4] = freq + beat;
    mode_decays[4] = decay;
    let body_220 = set_bandpass_coefficients(220.0, 6.0, sr);
    let body_650 = set_bandpass_coefficients(650.0, 4.0, sr);
    let buzz_coeff = set_bandpass_coefficients(4_000.0, 2.0, sr);

    let first_block = st.u[1] == 0;
    if first_block {
        mem[..STATE_FLOATS].fill(0.0);
        st.u[0] = Rng::new(kx.seed ^ 0x4B41_4C49).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    for sample in out.iter_mut() {
        let counter = mem[PLUCK_COUNTER];
        let excitation = if counter < pulse_samples {
            let phase = PI * (counter + 1.0) / pulse_samples;
            velocity * phase.sin()
        } else {
            0.0
        };
        mem[PLUCK_COUNTER] = counter + 1.0;
        let mut tines = 0.0;
        let mut fundamental = 0.0;
        let mut energy = 0.0;
        for mode in 0..MODES {
            let response = modal_sample(
                &mut mem[2 * mode..2 * mode + 2],
                mode_freqs[mode],
                mode_decays[mode],
                excitation,
                sr,
            );
            if mode == 0 {
                fundamental = response;
                if counter < pulse_samples {
                    mem[FUNDAMENTAL_REF] = mem[FUNDAMENTAL_REF].max(response.abs());
                }
                tines += response * MODE_GAINS[0];
            } else if mode == 4 {
                tines += response * (MODE_GAINS[0] * 0.6);
            } else {
                tines += response * MODE_GAINS[mode];
            }
            let real = mem[2 * mode];
            let imag = mem[2 * mode + 1];
            energy += real * real + imag * imag;
        }
        let body_out = biquad(tines, &mut mem[BODY_STATE..BODY_STATE + 4], &body_220)
            + biquad(tines, &mut mem[BODY_STATE + 4..BODY_STATE + 8], &body_650);
        let mut value = (tines + body * body_out) * NORMALIZATION;
        if buzz > 0.0 {
            let gate = (fundamental.abs() - 0.25 * mem[FUNDAMENTAL_REF]).max(0.0);
            let noise = rng.bipolar();
            let rattle = biquad(noise, &mut mem[BUZZ_STATE..BUZZ_STATE + 4], &buzz_coeff);
            value += rattle * gate * buzz * 4.0 * NORMALIZATION;
        }
        *sample = clean(value).clamp(-1.0, 1.0);
        if mem[PLUCK_COUNTER] >= pulse_samples && energy < FINISH_ENERGY {
            st.finish();
        }
    }
    st.u[0] = rng.s;
}
