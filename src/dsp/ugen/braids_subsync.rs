//! Original analytic sub-octave and dual hard-sync oscillator adaptations.
//!
//! Positions 5–8 follow the MIT Braids `RenderSub` / `RenderDualSync`
//! control roles, but use no source oscillator, table, or generated data.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Master, sub/slave, external-sync clock, and strike envelope.
pub const STATE_FLOATS: usize = 4;

fn square(phase: f32, width: f32) -> f32 {
    if phase < width {
        1.0
    } else {
        -1.0
    }
}

fn variable_saw(phase: f32, color: f32) -> f32 {
    let corner = 0.12 + 0.76 * color;
    if phase < corner {
        -1.0 + 2.0 * phase / corner
    } else {
        1.0 - 2.0 * (phase - corner) / (1.0 - corner)
    }
}

fn base(shape: usize, phase: f32, color: f32) -> f32 {
    match shape {
        5 => square(phase, 0.1 + 0.8 * color),
        6 => variable_saw(phase, color),
        7 => square(phase, 0.5),
        _ => 2.0 * phase - 1.0,
    }
}

/// Shape positions 5–8; outside values clamp to the nearest endpoint.
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
    let shape = ins[1].first().round().clamp(5.0, 8.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let step = freq / sr;
    let sync_step = step * (1.0 + 3.0 * sync);
    let slave_step = (step * 2.0f32.powf(2.0 * color)).min(0.45);
    let sub_step = step / if timbre < 0.5 { 4.0 } else { 2.0 };
    let sub_gain = (2.0 * timbre - 1.0).abs() * 0.75;
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    for sample in out.iter_mut() {
        if sync > 0.0 {
            mem[2] += sync_step;
            if mem[2] >= 1.0 {
                mem[2] -= 1.0;
                mem[0] = 0.0;
                mem[1] = 0.0;
            }
        }
        mem[0] += step;
        let wrapped = mem[0] >= 1.0;
        if wrapped {
            mem[0] -= 1.0;
        }
        if shape >= 7 && wrapped {
            mem[1] = 0.0;
        }
        mem[1] = (mem[1] + if shape < 7 { sub_step } else { slave_step }).fract();
        let main = base(shape, mem[0], color);
        let companion = if shape < 7 {
            square(mem[1], 0.5)
        } else {
            base(shape, mem[1], color)
        };
        let blend = if shape < 7 { sub_gain } else { timbre };
        let body = main * (1.0 - blend) + companion * blend;
        let transient = if mem[3] == 0.0 { strike } else { mem[3] };
        mem[3] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        let overtone = (TAU * mem[0] * (2.0 + color)).sin();
        *sample = 0.7 * (body * (1.0 - 0.3 * transient) + 0.3 * transient * overtone);
    }
}
