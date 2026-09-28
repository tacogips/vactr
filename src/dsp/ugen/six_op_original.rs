//! Three original six-operator FM banks for Plaits positions 2–4.
//!
//! The source macro roles are retained, but no DX7 patch bytes, ratios,
//! levels, wave tables, or generated resource data are used here.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Six phases, six operator envelopes, six prior outputs, and event age.
pub const STATE_FLOATS: usize = 19;
/// Original configurations in each bank; none correspond to DX7 patches.
pub const PATCHES_PER_BANK: usize = 32;

#[derive(Clone, Copy)]
struct Configuration {
    ratio: [f32; 6],
    level: [f32; 6],
}

/// Deterministic authored arithmetic generates 32 distinct configurations.
fn configuration(bank: usize, patch: usize) -> Configuration {
    let mut result = Configuration {
        ratio: [0.0; 6],
        level: [0.0; 6],
    };
    for operator in 0..6 {
        let ratio_step = (patch * 7 + operator * 11 + bank * 13) % 17;
        let level_step = (patch * 13 + operator * 5 + bank * 3) % 9;
        result.ratio[operator] = 0.5 + ratio_step as f32 * 0.22 + patch as f32 * 0.003;
        result.level[operator] = 0.45 + level_step as f32 * 0.06;
    }
    result
}

#[inline]
fn unit(input: f32, fallback: f32) -> f32 {
    if input.is_finite() {
        input.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn modulator(bank: usize, op: usize, samples: &[f32; 6], previous: f32) -> f32 {
    match bank {
        0 => {
            if op % 2 == 0 {
                samples[op + 1]
            } else {
                0.0
            }
        }
        1 => match op {
            0 => samples[3],
            1 => samples[2],
            3 => samples[4],
            4 => samples[5],
            _ => 0.0,
        },
        _ => {
            if op == 5 {
                previous * 0.4
            } else {
                samples[op + 1]
            }
        }
    }
}

/// Render one bank. A duplicated node sends the same waveform to source-style
/// main and auxiliary outputs; both copies have identical event-local state.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
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
    let bank = ins[6].first().round().clamp(0.0, 2.0) as usize;
    let mut cached_patch = PATCHES_PER_BANK;
    let mut config = configuration(bank, 0);
    for (frame, output) in out.iter_mut().enumerate() {
        let frequency = ins[0].at(frame).clamp(20.0, sr * 0.22);
        let patch = (unit(ins[1].at(frame), 0.0) * 31.0).round() as usize;
        let brightness = unit(ins[2].at(frame), 0.5);
        let morph = unit(ins[3].at(frame), 0.5);
        let velocity = unit(ins[4].at(frame), 1.0);
        let sustain = unit(ins[5].at(frame), 0.0) >= 0.5;
        if patch != cached_patch {
            config = configuration(bank, patch);
            cached_patch = patch;
        }
        let age = mem[18];
        let mut operators = [0.0; 6];
        for op in (0..6).rev() {
            let step = (frequency * config.ratio[op] / sr).min(0.45);
            mem[op] = (mem[op] + step).fract();
            let decay = (0.16 + morph * morph * 2.2) * (1.0 + op as f32 * 0.17);
            let natural = (-age / decay).exp();
            let target = if sustain && frame < kx.gate {
                natural.max(0.95)
            } else {
                natural
            };
            let attack = 1.0 - (-1.0 / (0.003 * sr)).exp();
            mem[6 + op] += attack * (target - mem[6 + op]);
            let brightness_level = if op % 2 == 0 {
                0.6 + brightness * 0.4
            } else {
                0.1 + brightness * 0.9
            };
            let modulation = modulator(bank, op, &operators, mem[12 + op]);
            operators[op] = (TAU * mem[op] + modulation * (0.5 + brightness * 5.0)).sin()
                * config.level[op]
                * brightness_level
                * mem[6 + op];
        }
        let voice = match bank {
            0 => (operators[0] + operators[2] + operators[4]) * 0.4,
            1 => (operators[0] + operators[1]) * 0.6,
            _ => operators[0],
        };
        for (op, previous) in operators.into_iter().zip(&mut mem[12..18]) {
            *previous = op;
        }
        *output = (voice * velocity).tanh();
        mem[18] = age + 1.0 / sr;
    }
}

#[cfg(test)]
mod tests {
    use super::{configuration, PATCHES_PER_BANK};

    #[test]
    fn all_original_configurations_are_distinct_within_each_bank() {
        for bank in 0..3 {
            let mut keys = Vec::new();
            for patch in 0..PATCHES_PER_BANK {
                let config = configuration(bank, patch);
                let key: Vec<_> = config.ratio.into_iter().map(f32::to_bits).collect();
                assert!(!keys.contains(&key), "bank {bank}, patch {patch}");
                keys.push(key);
            }
        }
    }
}
