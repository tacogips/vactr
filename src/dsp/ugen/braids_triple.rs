//! Original analytic three-oscillator adaptation of Braids positions 9–12.
//!
//! The published implementation selects intervals from a 65-value array;
//! this one computes a continuous, independently designed curve instead.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Three phases, event-local sync clock and strike envelope.
pub const STATE_FLOATS: usize = 5;

/// Continuous ±24-semitone interval with a ±0.15-semitone fine zone.
pub(crate) fn interval_semitones(value: f32) -> f32 {
    let centered = 2.0 * value.clamp(0.0, 1.0) - 1.0;
    let distance = centered.abs();
    let semitones = if distance <= 0.12 {
        distance * 1.25
    } else {
        0.15 + ((distance - 0.12) / 0.88).powf(1.3) * 23.85
    };
    semitones.copysign(centered)
}

fn waveform(shape: usize, phase: f32) -> f32 {
    match shape {
        9 => 2.0 * phase - 1.0,
        10 => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        11 => 1.0 - 4.0 * (phase - 0.5).abs(),
        _ => (TAU * phase).sin(),
    }
}

/// Render only source positions 9–12, clamping outside selectors to endpoints.
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
    let shape = ins[1].first().round().clamp(9.0, 12.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let root_step = freq / sr;
    let steps = [
        root_step,
        (root_step * 2.0f32.powf(interval_semitones(color) / 12.0)).min(0.45),
        (root_step * 2.0f32.powf(interval_semitones(timbre) / 12.0)).min(0.45),
    ];
    let sync_step = root_step * (1.0 + 3.0 * sync);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    for sample in out.iter_mut() {
        if sync > 0.0 {
            mem[3] += sync_step;
            if mem[3] >= 1.0 {
                mem[3] -= 1.0;
                mem[..3].fill(0.0);
            }
        }
        let mut body = 0.0;
        for (phase, step) in mem[..3].iter_mut().zip(steps) {
            *phase = (*phase + step).fract();
            body += waveform(shape, *phase) * 0.32;
        }
        let transient = if mem[4] == 0.0 { strike } else { mem[4] };
        mem[4] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        let overtone = (TAU * mem[0] * (2.0 + color)).sin();
        *sample = body * (1.0 - 0.25 * transient) + 0.25 * transient * overtone;
    }
}
