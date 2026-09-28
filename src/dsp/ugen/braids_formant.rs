//! Original procedural adaptations of Braids positions 21–24.
//!
//! No source phoneme arrays, formant tables, sine samples or lookup data are
//! imported. In particular, position 23 uses analytic pulse grains rather
//! than the source's five-filter FOF path.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Root, three formants, sync and event-onset transient.
pub const STATE_FLOATS: usize = 6;

fn advance(phase: &mut f32, step: f32) -> bool {
    *phase += step;
    if *phase >= 1.0 {
        *phase -= 1.0;
        true
    } else {
        false
    }
}

fn harmonic_bank(phase: f32, color: f32, timbre: f32, fundamental: f32, sr: f32) -> f32 {
    let peak = 1.0 + 10.0 * color;
    let width = 0.65 + 3.0 * timbre;
    let second_peak = 1.0 + peak * 0.5;
    let mut sum = 0.0;
    let mut weight = 0.0;
    for index in 1..=12 {
        let harmonic = index as f32;
        if harmonic * fundamental >= sr * 0.45 {
            break;
        }
        let distance = (harmonic - peak) / width;
        let secondary = (harmonic - second_peak) / (width * 1.4);
        let gain = 1.0 / (1.0 + distance * distance) + timbre * 0.5 / (1.0 + secondary * secondary);
        sum += gain * (TAU * phase * harmonic).sin();
        weight += gain;
    }
    sum / weight.max(1.0)
}

/// Render one of the four source-ordered formant/harmonic roles.
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
    let shape = ins[1].first().round().clamp(21.0, 24.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let step = freq / sr;
    let sync_step = step * (0.2 + sync * 0.7);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    for sample in out.iter_mut() {
        if sync > 0.0 && advance(&mut mem[4], sync_step) {
            mem[..4].fill(0.0);
        }
        let cycle = advance(&mut mem[0], step);
        if cycle && shape != 24 {
            mem[1..4].fill(0.0);
        }
        let body = match shape {
            21 => {
                advance(&mut mem[1], (step * (1.0 + color * 12.0)).min(0.45));
                advance(&mut mem[2], (step * (1.5 + timbre * 15.0)).min(0.45));
                let bell = (-8.0 * mem[0]).exp();
                bell * ((TAU * mem[1]).sin() * 0.6 + (TAU * mem[2]).sin() * 0.4)
            }
            22 => {
                let trajectory = color * color * (3.0 - 2.0 * color);
                let shift = 0.7 + timbre * 1.7;
                let ratios = [
                    1.4 + trajectory * 2.7,
                    3.3 + (1.0 - trajectory) * 6.0,
                    7.0 + trajectory * 5.0,
                ];
                let mut formants = 0.0;
                for (index, ratio) in ratios.into_iter().enumerate() {
                    advance(&mut mem[index + 1], (step * ratio * shift).min(0.45));
                    let amplitude = 0.56 / (index as f32 + 1.0);
                    formants += amplitude * (TAU * mem[index + 1]).sin();
                }
                formants * (1.0 - mem[0])
            }
            23 => {
                let envelope = (-10.0 * mem[0] * (0.6 + timbre)).exp();
                let mut grain = 0.0;
                for partial in 0..5 {
                    let band = partial as f32 + 1.0;
                    let ratio = (2.0 + band * (1.1 + color * 1.5)) * (0.8 + timbre);
                    if freq * ratio < sr * 0.45 {
                        grain += (TAU * mem[0] * ratio).sin() / (band + 1.0);
                    }
                }
                grain * envelope * 0.8
            }
            _ => harmonic_bank(mem[0], color, timbre, freq, sr),
        };
        let transient = if mem[5] == 0.0 { strike } else { mem[5] };
        mem[5] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (body * (1.0 - 0.2 * transient) + 0.2 * transient * (TAU * mem[0] * 4.0).sin())
            .clamp(-1.0, 1.0);
    }
}
