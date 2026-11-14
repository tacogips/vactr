//! Eight procedural wave terrains traced by an analytic quadrature path.
//!
//! Source Plaits position 5 uses five analytic terrains, three backed by
//! `wav_integrated_waves`, and an optional ninth user-supplied terrain.
//! Modes 5-7 here are original procedural replacements; mode 8 is unavailable.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Quadrature phase and spare state, reserved before callback rendering.
pub const STATE_FLOATS: usize = 4;

#[inline]
fn sine(phase: f32) -> f32 {
    (TAU * phase).sin()
}

#[inline]
fn squash(value: f32, gain: f32) -> f32 {
    let v = value * gain;
    v / (1.0 + v.abs())
}

#[inline]
fn terrain(x: f32, y: f32, mode: usize) -> f32 {
    let xy = x * y;
    match mode {
        0 => (squash(sine(4.0 + 1.273 * x), 2.0) - sine(4.0 + y * (x + 1.571) * 0.637)) * 0.57,
        1 => sine(4.0 + sine(4.0 + (x + y) * 0.637) / (0.2 + xy * xy) * 0.159),
        2 => sine(4.0 + sine(4.0 + 2.387 * xy) / (0.350 + xy * xy) * 0.159),
        3 => sine(4.0 + xy / (2.0 + (5.0 * (x - 0.25) * (y + 0.25)).abs()) * 6.366),
        4 => sine(
            4.0 + 0.159 / (0.170 + (y - 0.25).abs())
                + 0.477 / (0.350 + ((x + 0.5) * (y + 1.5)).abs()),
        ),
        // Original analytic surfaces, not interpolations of upstream waves.
        5 => {
            (TAU * (0.21 * x + 0.43 * y + 0.18 * xy)).sin() * (1.0 - 0.4 * (x * x + y * y)).max(0.0)
        }
        6 => (2.4 * (x * x - y * y) + 0.6 * xy).tanh(),
        _ => {
            let radius = x * x + y * y;
            (TAU * (1.1 * radius + 0.12 * x)).sin() * (-0.8 * radius).exp()
        }
    }
}

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// Mode 0 is the terrain signal; mode 1 is the transformed path/terrain aux.
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
    let freq = ins[0].first().clamp(20.0, sr * 0.24);
    let harmonics = unit(ins[1].first());
    let timbre = unit(ins[2].first());
    let morph = unit(ins[3].first());
    let auxiliary = ins[4].first() >= 0.5;
    let normalized = freq / sr;
    let attenuation = (1.0 - 8.0 * normalized).max(0.0);
    let radius = 0.1 + 0.9 * timbre * attenuation * (2.0 - attenuation);
    let offset = 1.9 * morph - 1.0;
    let position = ((harmonics * 1.05).min(1.0) * 6.9999).min(6.9999);
    let mode = position.floor() as usize;
    let blend = position.fract();
    let step = (normalized * 0.5).min(0.12);
    for sample in out.iter_mut() {
        let mut main = 0.0;
        let mut aux = 0.0;
        for _ in 0..2 {
            mem[0] = (mem[0] + step).fract();
            let x_path = radius * (TAU * mem[0]).cos();
            let y = radius * (TAU * mem[0]).sin();
            let x = x_path * (1.0 - offset.abs()) + offset;
            let z0 = terrain(x, y, mode);
            let z1 = terrain(x, y, mode + 1);
            let z = z0 + blend * (z1 - z0);
            main += z;
            aux += y + z;
        }
        *sample = if auxiliary {
            sine(1.0 + 0.25 * aux)
        } else {
            0.5 * main
        };
    }
}
