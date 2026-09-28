//! Five original analytic oscillator shapes informed by the MIT Braids registry.
//!
//! These are independent wave functions, not translations of its table-backed
//! oscillators, filters, or waveshapers. One event owns one bounded phase state.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Main phase, sync phase, and strike envelope.
pub const STATE_FLOATS: usize = 3;

fn triangle(p: f32) -> f32 {
    1.0 - 4.0 * (p - 0.5).abs()
}

fn saw(p: f32, bend: f32) -> f32 {
    let width = (0.12 + bend * 0.76).clamp(0.12, 0.88);
    if p < width {
        2.0 * p / width - 1.0
    } else {
        1.0 - 2.0 * (p - width) / (1.0 - width)
    }
}

fn wave(shape: usize, p: f32, color: f32, timbre: f32) -> f32 {
    let sine = (TAU * p).sin();
    let tri = triangle(p);
    let square = if p < 0.5 { 1.0 } else { -1.0 };
    match shape {
        // Curved saw family, with independent phase bend and saturation.
        0 => {
            let ramp = 2.0 * p - 1.0;
            let curved = ramp.signum() * ramp.abs().powf(0.25 + 1.5 * color);
            ((1.0 - timbre) * curved + timbre * (TAU * (p + color * sine * 0.12)).sin()).tanh()
        }
        // Continuous sine/triangle/saw/square morph, with nonlinear color.
        1 => {
            let x = timbre * 3.0;
            let a = if x < 1.0 {
                sine
            } else if x < 2.0 {
                tri
            } else {
                2.0 * p - 1.0
            };
            let b = if x < 1.0 {
                tri
            } else if x < 2.0 {
                2.0 * p - 1.0
            } else {
                square
            };
            let mixed = if x >= 3.0 {
                square
            } else {
                a + (b - a) * x.fract()
            };
            ((1.0 + color * 6.0) * mixed).tanh()
        }
        // Variable-slope saw and pulse with timbre-controlled balance.
        2 => {
            let pulse = if p < 0.1 + color * 0.8 { 1.0 } else { -1.0 };
            saw(p, color) * (1.0 - timbre) + pulse * timbre
        }
        // Sine and triangle folded by an original periodic transfer.
        3 => {
            let x = sine * (1.0 - timbre) + tri * timbre;
            ((1.0 + color * 5.0) * x).sin()
        }
        // Eight harmonic partials: brightness and harmonic detuning.
        _ => {
            let mut sum = 0.0;
            let mut norm = 0.0;
            for harmonic in 1..=8 {
                #[allow(clippy::cast_precision_loss)]
                let h = harmonic as f32;
                let gain = 1.0 / h.powf(2.0 - 1.7 * color);
                sum += (TAU * p * (h + timbre * 0.045 * (h - 1.0))).sin() * gain;
                norm += gain;
            }
            sum / norm.max(1.0)
        }
    }
}

/// Render one of positions 0–4; values outside this set clamp to position 4.
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
    let shape = ins[1].first().round().clamp(0.0, 4.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let step = freq / sr;
    let sync_step = step * (1.0 + 3.0 * sync);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    for sample in out.iter_mut() {
        if sync > 0.0 {
            mem[1] += sync_step;
            if mem[1] >= 1.0 {
                mem[1] -= 1.0;
                mem[0] = 0.0;
            }
        }
        mem[0] = (mem[0] + step).fract();
        let transient = if mem[2] == 0.0 { strike } else { mem[2] };
        mem[2] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        let base = wave(shape, mem[0], color, timbre);
        let overtone = (TAU * mem[0] * (2.0 + color)).sin();
        *sample = 0.7 * (base * (1.0 - 0.35 * transient) + 0.35 * transient * overtone);
    }
}
