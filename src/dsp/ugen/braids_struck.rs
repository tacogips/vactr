//! Additive struck-voice adaptations of Braids positions 32–33.
//!
//! The small partial, amplitude and decay maps below are translated from
//! Emilie Gillet's MIT-licensed `braids/digital_oscillator.cc` (2012). Analytic
//! sine, host-rate decay and a deterministic noise filter replace source
//! waveform/lookup resources and block-dependent fixed-point processing.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

const BELL_PITCH: [i16; 11] = [
    -1284, -1283, -184, -183, 385, 1175, 1536, 2233, 2434, 2934, 3110,
];
const BELL_GAIN: [u16; 11] = [
    8192, 5488, 8192, 14745, 21872, 13680, 11960, 10895, 10895, 6144, 10895,
];
const BELL_LONG: [u16; 11] = [
    65533, 65533, 65533, 65532, 65531, 65531, 65530, 65529, 65527, 65523, 65519,
];
const BELL_SHORT: [u16; 11] = [
    65308, 65283, 65186, 65123, 64839, 64889, 64632, 64409, 64038, 63302, 62575,
];
const DRUM_PITCH: [i16; 6] = [0, 0, 1041, 1747, 1846, 3072];
const DRUM_GAIN: [u16; 6] = [16986, 2654, 3981, 5308, 3981, 2985];
const DRUM_LONG: [u16; 6] = [65533, 65531, 65531, 65531, 65531, 65516];
const DRUM_SHORT: [u16; 6] = [65083, 64715, 64715, 64715, 64715, 62312];

const PARTIALS: usize = 11;
/// Eleven phases/amplitudes, three noise filters, RNG and trigger state.
pub const STATE_FLOATS: usize = 29;

fn noise(state: &mut f32) -> f32 {
    let mut bits = if *state == 0.0 {
        0xACE1u16
    } else {
        *state as u16
    };
    bits ^= bits << 7;
    bits ^= bits >> 9;
    bits ^= bits << 8;
    *state = f32::from(bits);
    f32::from(bits) / 32767.5 - 1.0
}

fn decay(long: u16, short: u16, color: f32, sr: f32) -> f32 {
    let source_block_factor =
        (f32::from(short) + (f32::from(long) - f32::from(short)) * color * color) / 65536.0;
    // Pinned Braids test renders blocks of 24 at 48 kHz. Preserve that
    // reference duration while removing host block-size dependence.
    source_block_factor.powf(48_000.0 / (24.0 * sr))
}

fn trigger(state: &mut [f32], shape: usize, strike: f32) {
    let gain = 0.2 + strike * 0.8;
    let count = if shape == 32 { 11 } else { 6 };
    for index in 0..count {
        state[index] = 0.25;
        let source_gain = if shape == 32 {
            BELL_GAIN[index]
        } else {
            DRUM_GAIN[index]
        };
        state[PARTIALS + index] = f32::from(source_gain) / 32768.0 * gain;
    }
    state[22..25].fill(0.0);
    state[28] = 1.0;
}

/// Render struck bell or drum with fixed per-voice partial/noise state.
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
    let shape = ins[1].first().round().clamp(32.0, 33.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    if mem[28] == 0.0 {
        trigger(mem, shape, strike);
    }
    let count = if shape == 32 { 11 } else { 6 };
    let sync_step = freq / sr * (0.08 + 0.25 * sync);
    let noise_coeff = (TAU * (80.0 + timbre * freq * 8.0) / sr).min(0.85);
    let mut steps = [0.0; PARTIALS];
    let mut decays = [0.0; PARTIALS];
    for index in 0..count {
        let (pitch, long, short) = if shape == 32 {
            (BELL_PITCH[index], BELL_LONG[index], BELL_SHORT[index])
        } else {
            (DRUM_PITCH[index], DRUM_LONG[index], DRUM_SHORT[index])
        };
        let alternating = if index % 2 == 0 { -1.0 } else { 1.0 };
        let detune = if shape == 32 {
            alternating * timbre * 2.0
        } else {
            0.0
        };
        let semitones = f32::from(pitch) / 128.0 + detune;
        let partial_freq = freq * 2.0f32.powf(semitones / 12.0);
        steps[index] = if partial_freq < sr * 0.45 {
            partial_freq / sr
        } else {
            0.0
        };
        decays[index] = decay(long, short, color, sr);
    }
    for sample in out.iter_mut() {
        if sync > 0.0 {
            mem[27] += sync_step;
            if mem[27] >= 1.0 {
                mem[27] -= 1.0;
                trigger(mem, shape, strike);
            }
        }
        let mut partials = [0.0; PARTIALS];
        let mut sum = 0.0;
        for index in 0..count {
            if steps[index] > 0.0 {
                mem[index] = (mem[index] + steps[index]).fract();
                partials[index] = (TAU * mem[index]).sin() * mem[PARTIALS + index];
                sum += partials[index];
            }
            mem[PARTIALS + index] *= decays[index];
        }
        let body = if shape == 32 {
            sum * 0.17
        } else {
            let random = noise(&mut mem[25]);
            mem[22] += noise_coeff * (random - mem[22]);
            mem[23] += noise_coeff * (mem[22] - mem[23]);
            mem[24] += noise_coeff * (mem[23] - mem[24]);
            let noise_mix = (partials[1] * (1.0 - timbre) + partials[3] * timbre) * mem[24];
            (partials[0] + sum * (0.2 + timbre * 0.3) + noise_mix * 0.6) * 0.38
        };
        *sample = body.clamp(-1.0, 1.0);
    }
}
