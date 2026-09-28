//! Procedural replacements for Braids positions 37–38.
//!
//! No `waves.bin`, `map.bin`, `wt_waves`, index list, source wave line,
//! generated resource or LXR table is read or imported. The source's 20-bank
//! scan and 16×16 bilinear coordinate roles remain, but sound content differs.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Root phase, generated sync clock and event-onset strike envelope.
pub const STATE_FLOATS: usize = 3;

fn advance(phase: &mut f32, step: f32) -> bool {
    *phase += step;
    if *phase >= 1.0 {
        *phase -= 1.0;
        true
    } else {
        false
    }
}

fn partial(phase: f32, harmonic: usize, freq: f32, sr: f32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let multiple = harmonic as f32;
    if freq * multiple >= sr * 0.45 {
        0.0
    } else {
        (TAU * phase * multiple).sin()
    }
}

fn bank_cell(bank: usize, step: usize, phase: f32, freq: f32, sr: f32) -> f32 {
    let a = 1 + (bank * 3 + step) % 5;
    let b = 2 + (bank + step * 2) % 7;
    let c = 3 + (bank * 2 + step * 3) % 9;
    let tilt = 0.18 + step as f32 / 30.0;
    (partial(phase, a, freq, sr) * (1.0 - tilt)
        + partial(phase, b, freq, sr) * tilt * 0.7
        + partial(phase, c, freq, sr) * 0.22)
        * 0.75
}

fn bank_wave(bank: usize, scan: f32, phase: f32, freq: f32, sr: f32) -> f32 {
    let coordinate = scan.clamp(0.0, 1.0) * 15.0;
    let left = coordinate.floor() as usize;
    let right = (left + 1).min(15);
    let amount = coordinate - left as f32;
    bank_cell(bank, left, phase, freq, sr) * (1.0 - amount)
        + bank_cell(bank, right, phase, freq, sr) * amount
}

/// One original analytic grid node. Coordinates are 0–15 inclusive.
pub(crate) fn grid_cell(x: usize, y: usize, phase: f32, freq: f32, sr: f32) -> f32 {
    let a = 1 + (x + 2 * y) % 7;
    let b = 2 + (3 * x + y) % 9;
    let c = 3 + (x * y + x + y) % 11;
    let blend = 0.15 + x as f32 / 25.0;
    (partial(phase, a, freq, sr) * (1.0 - blend)
        + partial(phase, b, freq, sr) * blend
        + partial(phase, c, freq, sr) * (0.1 + y as f32 / 70.0))
        * 0.62
}

/// Bilinear interpolation among four independently computed grid nodes.
pub(crate) fn grid_wave(x: f32, y: f32, phase: f32, freq: f32, sr: f32) -> f32 {
    let x = x.clamp(0.0, 1.0) * 15.0;
    let y = y.clamp(0.0, 1.0) * 15.0;
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(15);
    let y1 = (y0 + 1).min(15);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let a =
        grid_cell(x0, y0, phase, freq, sr) * (1.0 - ty) + grid_cell(x0, y1, phase, freq, sr) * ty;
    let b =
        grid_cell(x1, y0, phase, freq, sr) * (1.0 - ty) + grid_cell(x1, y1, phase, freq, sr) * ty;
    a * (1.0 - tx) + b * tx
}

/// Render source position 37 bank scan or 38 two-dimensional map.
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
    let shape = ins[1].first().round().clamp(37.0, 38.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let step = freq / sr;
    let sync_step = step * (0.2 + 0.7 * sync);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    let bank = (timbre * 19.0).round() as usize;
    for sample in out.iter_mut() {
        if sync > 0.0 && advance(&mut mem[1], sync_step) {
            mem[0] = 0.0;
        }
        advance(&mut mem[0], step);
        let body = if shape == 37 {
            bank_wave(bank, color, mem[0], freq, sr)
        } else {
            grid_wave(color, timbre, mem[0], freq, sr)
        };
        let transient = if mem[2] == 0.0 { strike } else { mem[2] };
        mem[2] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (body * (1.0 - 0.2 * transient) + 0.2 * transient * partial(mem[0], 5, freq, sr))
            .clamp(-1.0, 1.0);
    }
}
