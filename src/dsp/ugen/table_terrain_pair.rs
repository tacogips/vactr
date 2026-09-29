//! Procedural 8×8×3 wave grid with mirrored z scan and quantized auxiliary.
//!
//! These analytic wave families are original replacements. No source wave,
//! integrated table, `waves.bin`, or generated lookup data is imported.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Phase, six scan smoothing values and fixed spare state.
pub const STATE_FLOATS: usize = 8;

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[inline]
fn wave(phase: f32, x: usize, y: usize, bank: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let (a, b) = (x as f32 / 7.0, y as f32 / 7.0);
    let angle = TAU * phase;
    match bank {
        0 => {
            let partial = 2.0 + 4.0 * a;
            (angle.sin() + 0.25 * (partial * angle + b * 2.0).sin()) / (1.0 + 0.25 * b)
        }
        1 => {
            let bend = 1.0 + 4.0 * a;
            ((angle.sin() + b * (2.0 * angle).sin()) * bend).tanh() * 0.8
        }
        _ => {
            let ratio = 1.0 + 5.0 * a;
            (angle + (0.2 + 1.2 * b) * (ratio * angle).sin()).sin() * 0.8
        }
    }
}

#[inline]
fn blend(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn snap_fraction(value: f32, amount: f32) -> f32 {
    let snapped = ((value - 0.5) * 16.0 + 0.5).clamp(0.0, 1.0);
    blend(value, snapped, amount)
}

#[inline]
fn bank_at(index: usize) -> usize {
    (if index >= 4 { 7 - index } else { index }).min(2)
}

#[inline]
fn plane(phase: f32, x: f32, y: f32, bank: usize) -> f32 {
    let ix = x.floor() as usize;
    let iy = y.floor() as usize;
    let fx = x.fract();
    let fy = y.fract();
    let a = blend(wave(phase, ix, iy, bank), wave(phase, ix + 1, iy, bank), fx);
    let b = blend(
        wave(phase, ix, iy + 1, bank),
        wave(phase, ix + 1, iy + 1, bank),
        fx,
    );
    blend(a, b, fy)
}

#[inline]
fn grid(phase: f32, x: f32, y: f32, z: f32) -> f32 {
    let iz = z.floor() as usize;
    blend(
        plane(phase, x, y, bank_at(iz)),
        plane(phase, x, y, bank_at(iz + 1)),
        z.fract(),
    )
}

/// Mode 0 renders continuous grid audio; mode 1 truncates to 1/32 steps.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render_worker(ins, mem, out, None, kx);
}

/// Renders continuous and quantized grid outputs from shared state.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    let _ = st;
    render_worker(ins, mem, main, Some(aux), kx);
}

fn render_worker(
    ins: &[Inp<'_>; MAX_PORTS],
    mem: &mut [f32],
    out: &mut [f32],
    mut aux_out: Option<&mut [f32]>,
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        if let Some(aux) = aux_out {
            aux.fill(0.0);
        }
        return;
    }
    let sr = kx.sr.max(1.0);
    let freq = ins[0].first().clamp(20.0, sr * 0.24);
    let x_target = 6.9999 * unit(ins[2].first());
    let y_target = 6.9999 * unit(ins[3].first());
    let z_target = 6.9999 * unit(ins[1].first());
    let auxiliary = ins[4].first() >= 0.5;
    let pre_xy = 1.0 - (-1.0 / (0.015 * sr)).exp();
    let pre_z = 1.0 - (-1.0 / (0.06 * sr)).exp();
    let position_rate = (2.0 * freq / sr).clamp(0.01, 0.1);
    let step = (freq / sr).min(0.24);
    for (i, sample) in out.iter_mut().enumerate() {
        mem[1] += pre_xy * (x_target - mem[1]);
        mem[2] += pre_xy * (y_target - mem[2]);
        mem[3] += pre_z * (z_target - mem[3]);
        let quantization = (mem[3] - 3.0).clamp(0.0, 1.0);
        let x = mem[1].floor() + snap_fraction(mem[1].fract(), quantization);
        let y = mem[2].floor() + snap_fraction(mem[2].fract(), quantization);
        let z = mem[3].floor() + snap_fraction(mem[3].fract(), quantization);
        mem[4] += position_rate * (x - mem[4]);
        mem[5] += position_rate * (y - mem[5]);
        mem[6] += position_rate * (z - mem[6]);
        mem[0] = (mem[0] + step).fract();
        let value = grid(
            mem[0],
            mem[4].clamp(0.0, 6.9999),
            mem[5].clamp(0.0, 6.9999),
            mem[6].clamp(0.0, 6.9999),
        );
        let aux_value = (value * 32.0).trunc() / 32.0;
        *sample = if auxiliary { aux_value } else { value };
        if let Some(aux) = aux_out.as_deref_mut() {
            aux[i] = if auxiliary { value } else { aux_value };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bank_at, grid, wave};

    #[test]
    fn all_192_original_cells_are_finite_and_distinct_across_axes() {
        for bank in 0..3 {
            for x in 0..8 {
                for y in 0..8 {
                    let values = [0.11, 0.29, 0.53, 0.77].map(|phase| wave(phase, x, y, bank));
                    assert!(values.iter().all(|v| v.is_finite()));
                    assert!(values.iter().any(|v| v.abs() > 0.01));
                }
            }
        }
        assert_eq!(
            (0..8).map(bank_at).collect::<Vec<_>>(),
            [0, 1, 2, 2, 2, 2, 1, 0]
        );
        assert!((grid(0.23, 2.0, 3.0, 1.0) - wave(0.23, 2, 3, 1)).abs() < 1e-5);
        assert_ne!(wave(0.23, 0, 3, 0), wave(0.23, 7, 3, 0));
        assert_ne!(wave(0.23, 2, 0, 1), wave(0.23, 2, 7, 1));
        assert_ne!(wave(0.23, 2, 3, 0), wave(0.23, 2, 3, 2));
    }
}
