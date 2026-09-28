//! Original procedural percussion adaptations of Braids positions 34–36.
//!
//! Timed kick pulses, six-square metallic/noise cymbal and dual-resonance
//! snare preserve source roles. Filter equations, oscillator ratios, noise,
//! envelopes and timing interpolation are independently written; no source
//! waveform, LUT, pulse class or binary resource is imported.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Timers, resonators, six phases, filter states, RNG and sync clock.
pub const STATE_FLOATS: usize = 24;

const CYMBAL_RATIOS: [f32; 6] = [1.0, 1.31, 1.49, 1.73, 2.07, 2.41];

fn noise(state: &mut f32) -> f32 {
    let mut bits = if *state == 0.0 {
        0xACE1u16
    } else {
        *state as u16
    };
    bits ^= bits << 7;
    bits ^= bits >> 9;
    bits ^= bits << 8;
    *state = f32::from(bits);
    f32::from(bits) / 32767.5 - 1.0
}

fn advance(phase: &mut f32, step: f32) -> bool {
    *phase += step;
    if *phase >= 1.0 {
        *phase -= 1.0;
        true
    } else {
        false
    }
}

fn resonator(input: f32, coef: f32, radius: f32, history: &mut [f32]) -> f32 {
    let output = (input + coef * history[0] - radius * radius * history[1]).clamp(-8.0, 8.0);
    history[1] = history[0];
    history[0] = output;
    output
}

fn trigger(state: &mut [f32], shape: usize, strike: f32) {
    state[0] = 1.0; // sample counter + 1; zero means not triggered
    state[1] = 0.25 + 0.75 * strike;
    if shape == 35 {
        state[8..14].fill(0.0);
        state[14] = 0.0;
    }
    state[23] = 1.0;
}

/// Render one source-ordered percussion role using fixed state.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    let freq = ins[0].first().clamp(20.0, sr * 0.2);
    let shape = ins[1].first().round().clamp(34.0, 36.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    if mem[23] == 0.0 {
        trigger(mem, shape, strike);
    }
    let sync_step = freq / sr * (0.08 + sync * 0.3);
    let kick_radius = (-1.0 / ((0.12 + color * 1.6) * sr)).exp();
    let snare_radius = (-1.0 / ((0.05 + color * 0.5) * sr)).exp();
    let hat_decay = (-1.0 / ((0.03 + color * 0.28) * sr)).exp();
    let snare_noise_decay = (-1.0 / ((0.025 + timbre * 0.25) * sr)).exp();
    let kick_base_coef = 2.0 * kick_radius * (TAU * freq / sr).cos();
    let kick_sweep_coef = 2.0 * kick_radius * (TAU * (freq * 2.1).min(sr * 0.45) / sr).cos();
    let snare_a_coef = 2.0 * snare_radius * (TAU * (freq * 2.0).min(sr * 0.45) / sr).cos();
    let snare_b_radius = snare_radius * 0.998;
    let snare_b_coef = 2.0 * snare_b_radius * (TAU * (freq * 4.0).min(sr * 0.45) / sr).cos();
    for sample in out.iter_mut() {
        if sync > 0.0 && advance(&mut mem[22], sync_step) {
            trigger(mem, shape, strike);
        }
        let elapsed = (mem[0] - 1.0) / sr;
        let body = match shape {
            34 => {
                let initial = if mem[0] == 1.0 { 0.7 } else { 0.0 };
                let delayed = if (elapsed >= 0.001 && elapsed < 0.001 + 1.0 / sr)
                    || (elapsed >= 0.004 && elapsed < 0.004 + 1.0 / sr)
                {
                    if elapsed < 0.002 {
                        -0.32
                    } else {
                        0.24
                    }
                } else {
                    0.0
                };
                let coef = if elapsed < 0.004 {
                    kick_sweep_coef
                } else {
                    kick_base_coef
                };
                let excitation = (initial + delayed) * mem[1];
                let resonance = resonator(excitation, coef, kick_radius, &mut mem[2..4]);
                let coeff = 0.03 + timbre * 0.65;
                mem[4] += coeff * (resonance - mem[4]);
                mem[4] * 0.12
            }
            35 => {
                let mut metallic = 0.0;
                for (phase, ratio) in mem[8..14].iter_mut().zip(CYMBAL_RATIOS) {
                    advance(phase, (freq * ratio / sr).min(0.45));
                    metallic += if *phase < 0.5 { 1.0 } else { -1.0 };
                }
                metallic /= 6.0;
                if advance(&mut mem[14], (freq * 13.0 / sr).min(0.45)) {
                    mem[15] = noise(&mut mem[16]);
                }
                let cutoff = (100.0 + timbre * (sr * 0.25 - 100.0)) / sr;
                let coeff = (TAU * cutoff).min(0.9);
                mem[17] += coeff * (metallic - mem[17]);
                let band = metallic - mem[17];
                mem[18] += coeff * (mem[15] - mem[18]);
                let high = mem[15] - mem[18];
                mem[1] *= hat_decay;
                (band * (1.0 - timbre) + high * timbre) * mem[1] * 0.7
            }
            _ => {
                let pulse = if mem[0] == 1.0 { mem[1] } else { 0.0 };
                let mode_a = resonator(pulse, snare_a_coef, snare_radius, &mut mem[2..4]);
                let mode_b = resonator(pulse * 0.7, snare_b_coef, snare_b_radius, &mut mem[5..7]);
                mem[7] *= snare_noise_decay;
                if mem[0] == 1.0 {
                    mem[7] = mem[1];
                }
                let random = noise(&mut mem[16]) * mem[7];
                let coeff = (TAU * (150.0 + timbre * freq * 8.0) / sr).min(0.9);
                mem[17] += coeff * (random - mem[17]);
                let hiss = random - mem[17];
                ((1.0 - color) * mode_a + color * mode_b) * 0.09 + hiss * timbre * 0.3
            }
        };
        mem[0] = (mem[0] + 1.0).min(sr * 10.0);
        *sample = body.clamp(-1.0, 1.0);
    }
}
