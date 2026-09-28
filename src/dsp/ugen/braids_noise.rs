//! Original noise adaptations of Braids positions 41–43.
//!
//! LP/BP/HP morph, dual resonant peaks and cyclic clocked quantized noise
//! retain source roles. Filter coefficients, RNG, quantization and drive
//! are independent implementations; no source LUT, overdrive or wave data.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// RNG, filter histories, clock/cycle, held value, seed and transient state.
pub const STATE_FLOATS: usize = 13;

fn next_bits(state: &mut f32) -> u16 {
    let mut bits = if *state == 0.0 {
        0xACE1u16
    } else {
        *state as u16
    };
    bits ^= bits << 7;
    bits ^= bits >> 9;
    bits ^= bits << 8;
    *state = f32::from(bits);
    bits
}

fn noise(state: &mut f32) -> f32 {
    f32::from(next_bits(state)) / 32767.5 - 1.0
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
    let value = (input + coef * history[0] - radius * radius * history[1]).clamp(-8.0, 8.0);
    history[1] = history[0];
    history[0] = value;
    value
}

/// Second peak moves continuously below/above the first across timbre.
pub(crate) fn peak_frequencies(freq: f32, timbre: f32) -> (f32, f32) {
    (
        freq,
        freq * 2.0f32.powf((timbre.clamp(0.0, 1.0) - 0.5) * 4.0),
    )
}

/// Midpoint quantizer for two to 32 distinct held-noise levels.
pub(crate) fn quantize(value: f32, steps: usize) -> f32 {
    let bins = steps.clamp(2, 32) as f32;
    ((value.clamp(-1.0, 1.0) + 1.0) * 0.5 * bins)
        .floor()
        .min(bins - 1.0)
        .mul_add(2.0 / bins, 1.0 / bins - 1.0)
}

/// Advance a deterministic finite random cycle and return its held level.
pub(crate) fn clocked_tick(state: &mut [f32], period: usize, steps: usize) -> f32 {
    let next = noise(&mut state[0]);
    state[8] += 1.0;
    if state[8] >= period.clamp(2, 32) as f32 {
        state[8] = 0.0;
        state[0] = state[10];
    }
    state[9] = quantize(next, steps);
    state[9]
}

/// Render filtered, twin-peak or clocked noise into a fixed voice block.
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
    let freq = ins[0].first().clamp(20.0, sr * 0.22);
    let shape = ins[1].first().round().clamp(41.0, 43.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    if mem[10] == 0.0 {
        let strike_seed = (strike * 65535.0) as u32;
        let seeded = ((kx.seed ^ 0xACE1 ^ strike_seed) & 0xffff) as u16;
        mem[10] = f32::from(seeded.max(1));
        mem[0] = mem[10];
    }
    let sync_step = freq / sr * (0.2 + 0.7 * sync);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    let cutoff = freq.min(sr * 0.45);
    let lp_alpha = 1.0 - (-TAU * cutoff / sr).exp();
    let bp_alpha = 1.0 - (-TAU * (30.0 + color * cutoff) / sr).exp();
    let radius = (-1.0 / ((0.006 + color * 0.12) * sr)).exp();
    let (first_peak, second_peak) = peak_frequencies(freq, timbre);
    let coef_a = 2.0 * radius * (TAU * first_peak / sr).cos();
    let coef_b = 2.0 * radius * (TAU * second_peak.min(sr * 0.45) / sr).cos();
    let clock_step = (freq * 8.0 / sr).min(0.45);
    let period = 2 + (color * 30.0).round() as usize;
    let steps = 2 + (timbre * 30.0).round() as usize;
    for sample in out.iter_mut() {
        if sync > 0.0 && advance(&mut mem[11], sync_step) {
            mem[1..10].fill(0.0);
            mem[0] = mem[10];
        }
        let body = match shape {
            41 => {
                let white = noise(&mut mem[0]);
                mem[1] += lp_alpha * (white - mem[1]);
                let hp = white - mem[1];
                mem[2] += bp_alpha * (hp - mem[2]);
                if timbre < 0.5 {
                    mem[1] * (1.0 - timbre * 2.0) + mem[2] * timbre * 2.0
                } else {
                    mem[2] * (2.0 - timbre * 2.0) + hp * (timbre * 2.0 - 1.0)
                }
            }
            42 => {
                let white = noise(&mut mem[0]) * (1.0 - color * 0.3);
                let a = resonator(white * 0.03, coef_a, radius, &mut mem[3..5]);
                let b = resonator(white * 0.03, coef_b, radius, &mut mem[5..7]);
                (a + b) * 0.18
            }
            _ => {
                if advance(&mut mem[7], clock_step) {
                    clocked_tick(mem, period, steps);
                }
                mem[9]
            }
        };
        let transient = if mem[12] == 0.0 { strike } else { mem[12] };
        mem[12] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (body * (1.0 - 0.15 * transient) + 0.15 * transient * mem[9]).clamp(-1.0, 1.0);
    }
}
