//! Original procedural adaptations of Braids positions 44–46.
//!
//! Four analytic sine grains, three particle resonances, and an I/Q symbol
//! carrier preserve source-level roles without source waves, LUTs or data.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::{PI, TAU};

/// Fixed per-voice RNG, sync, grain, resonator and symbol state.
pub const STATE_FLOATS: usize = 33;

fn random(mem: &mut [f32]) -> f32 {
    let next = ((mem[0] as u32).wrapping_mul(109).wrapping_add(1021)) & 0xffff;
    mem[0] = next as f32;
    next as f32 / 65_535.0
}

fn initialise(mem: &mut [f32], seed: u32, strike: f32) {
    let initial = (seed ^ ((strike * 65_535.0) as u32) ^ 0x6D31) & 0xffff;
    mem[0] = initial as f32;
    mem[1] = (initial.max(1)) as f32;
    for slot in 0..4 {
        mem[5 + 3 * slot] = 1.0;
    }
    mem[16] = 1.0;
    mem[29] = 1.0;
    mem[30] = 1.0;
    mem[31] = 1.0;
}

fn restart(mem: &mut [f32]) {
    mem[0] = mem[1];
    mem[4..].fill(0.0);
    for slot in 0..4 {
        mem[5 + 3 * slot] = 1.0;
    }
    mem[16] = 1.0;
    mem[29] = 1.0;
    mem[30] = 1.0;
    mem[31] = 1.0;
}

fn grain_sample(mem: &mut [f32], freq: f32, color: f32, timbre: f32, sr: f32) -> f32 {
    let density = 8.0 + color * 60.0;
    mem[16] += density / sr;
    if mem[16] >= 1.0 {
        mem[16] -= 1.0;
        let slot = (mem[17] as usize) & 3;
        mem[17] = ((slot + 1) & 3) as f32;
        let spread = random(mem) * 2.0 - 1.0;
        let phase = random(mem);
        let offset = 4 + 3 * slot;
        mem[offset] = phase;
        mem[offset + 1] = 0.0;
        mem[offset + 2] = (freq * 2.0f32.powf(spread * timbre)).min(sr * 0.45) / sr;
    }
    let age_step = 1.0 / ((0.18 - color * 0.15) * sr);
    let mut sum = 0.0;
    for slot in 0..4 {
        let offset = 4 + 3 * slot;
        let age = mem[offset + 1];
        if age < 1.0 {
            let envelope = (PI * age).sin().powi(2);
            sum += (TAU * mem[offset]).sin() * envelope;
            mem[offset] = (mem[offset] + mem[offset + 2]).fract();
            mem[offset + 1] = age + age_step;
        }
    }
    sum * 0.35
}

fn particle_sample(mem: &mut [f32], freq: f32, color: f32, timbre: f32, sr: f32) -> f32 {
    let mut impulse = 0.0;
    if random(mem) < (12.0 + color * 850.0) / sr {
        mem[18] = 1.0;
        let spread = (random(mem) * 2.0 - 1.0) * timbre;
        for i in 0..3 {
            let ratio = (1.0 + i as f32 * 0.61) * 2.0f32.powf(spread * (i as f32 + 1.0));
            let tuned = (freq * ratio).clamp(20.0, sr * 0.42);
            mem[25 + i] = (TAU * tuned / sr).cos();
        }
    }
    impulse += (random(mem) * 2.0 - 1.0) * mem[18] * 0.09;
    mem[18] *= (-1.0 / (0.0025 * sr)).exp();
    let radius = (-1.0 / (0.05 * sr)).exp();
    let mut sum = 0.0;
    for i in 0..3 {
        let index = 19 + i * 2;
        let value = (impulse + 2.0 * radius * mem[25 + i] * mem[index]
            - radius * radius * mem[index + 1])
            .clamp(-4.0, 4.0);
        mem[index + 1] = mem[index];
        mem[index] = value;
        sum += value;
    }
    (sum * 0.13).clamp(-1.0, 1.0)
}

fn symbol_sample(mem: &mut [f32], freq: f32, color: f32, timbre: f32, sr: f32) -> f32 {
    mem[29] += (freq * (0.1 + 2.0 * color)).min(sr * 0.4) / sr;
    if mem[29] >= 1.0 {
        mem[29] -= 1.0;
        let sequence = mem[32] as u32;
        mem[32] = ((sequence + 1) & 255) as f32;
        let candidate = (random(mem) * 4.0).floor() as u32;
        let pilot = ((sequence & 1) << 1) | ((sequence >> 1) & 1);
        let symbol = if random(mem) < timbre {
            candidate
        } else {
            pilot
        };
        mem[30] = if symbol & 1 == 0 { 0.7 } else { -0.7 };
        mem[31] = if symbol & 2 == 0 { 0.7 } else { -0.7 };
    }
    mem[28] = (mem[28] + freq / sr).fract();
    (mem[30] * (TAU * mem[28]).sin() + mem[31] * (TAU * mem[28]).cos()) * 0.55
}

/// Render one source-position role with fixed event-local state.
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
    let freq = ins[0].first().clamp(20.0, sr * 0.22);
    let shape = ins[1].first().round().clamp(44.0, 46.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    if mem[1] == 0.0 {
        initialise(mem, kx.seed, strike);
    }
    let sync_step = freq / sr * (0.2 + 0.7 * sync);
    let strike_decay = (-1.0 / (0.03 * sr)).exp();
    for sample in out.iter_mut() {
        if sync > 0.0 {
            mem[2] += sync_step;
            if mem[2] >= 1.0 {
                mem[2] -= 1.0;
                restart(mem);
            }
        }
        let body = match shape {
            44 => grain_sample(mem, freq, color, timbre, sr),
            45 => particle_sample(mem, freq, color, timbre, sr),
            _ => symbol_sample(mem, freq, color, timbre, sr),
        };
        let transient = if mem[3] == 0.0 { strike } else { mem[3] };
        mem[3] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (body * (0.8 + 0.2 * transient)).clamp(-1.0, 1.0);
    }
}
