//! Original procedural tone and ten-digit voice station.
//!
//! The published voice mode plays an external digit recording. This module
//! contains no recording, source offset, generated wavetable or phoneme data:
//! ten authored formant trajectories create new syllable-like sounds.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 32;

#[derive(Clone, Copy)]
struct Syllable {
    duration: f32,
    first: (f32, f32),
    last: (f32, f32),
    onset_noise: f32,
}

// Independently authored approximate English digit gestures. Frequencies are
// formant targets, not samples, source offsets or transcribed Peaks data.
fn syllable(digit: u8) -> Syllable {
    match digit {
        0 => Syllable {
            duration: 0.34,
            first: (320.0, 2700.0),
            last: (500.0, 950.0),
            onset_noise: 0.75,
        }, // zero
        1 => Syllable {
            duration: 0.27,
            first: (480.0, 1050.0),
            last: (540.0, 1300.0),
            onset_noise: 0.05,
        }, // one
        2 => Syllable {
            duration: 0.25,
            first: (380.0, 1800.0),
            last: (320.0, 2250.0),
            onset_noise: 0.35,
        }, // two
        3 => Syllable {
            duration: 0.29,
            first: (490.0, 1450.0),
            last: (370.0, 2050.0),
            onset_noise: 0.4,
        }, // three
        4 => Syllable {
            duration: 0.29,
            first: (570.0, 980.0),
            last: (360.0, 1050.0),
            onset_noise: 0.65,
        }, // four
        5 => Syllable {
            duration: 0.31,
            first: (630.0, 1000.0),
            last: (360.0, 2200.0),
            onset_noise: 0.8,
        }, // five
        6 => Syllable {
            duration: 0.30,
            first: (480.0, 1750.0),
            last: (390.0, 1500.0),
            onset_noise: 0.85,
        }, // six
        7 => Syllable {
            duration: 0.35,
            first: (510.0, 1600.0),
            last: (320.0, 1650.0),
            onset_noise: 0.7,
        }, // seven
        8 => Syllable {
            duration: 0.28,
            first: (420.0, 1900.0),
            last: (360.0, 1650.0),
            onset_noise: 0.08,
        }, // eight
        _ => Syllable {
            duration: 0.28,
            first: (560.0, 1750.0),
            last: (320.0, 1850.0),
            onset_noise: 0.12,
        }, // nine
    }
}

fn unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn random(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32) / (u32::MAX as f32)
}

fn formant(input: f32, frequency: f32, sr: f32, mem: &mut [f32], offset: usize) -> f32 {
    let radius = (-TAU * 85.0 / sr).exp();
    let omega = TAU * frequency.min(sr * 0.42) / sr;
    let y = (1.0 - radius) * input + 2.0 * radius * omega.cos() * mem[offset]
        - radius * radius * mem[offset + 1];
    mem[offset + 1] = mem[offset];
    mem[offset] = y.clamp(-8.0, 8.0);
    y
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    if st.u[0] == 0 {
        st.u[0] = kx.seed ^ 0x6d84_7931;
        if st.u[0] == 0 {
            st.u[0] = 1;
        }
    }
    for (i, sample) in out.iter_mut().enumerate() {
        let note = (ins[0].at(i) / 220.0).clamp(0.25, 4.0);
        let voice = ins[1].at(i) >= 0.5;
        let half = ins[2].at(i) >= 0.5;
        let tone = unit(ins[3].at(i));
        let transition = unit(ins[4].at(i));
        let noise = if half { 0.5 } else { unit(ins[5].at(i)) };
        let drive = if half { 0.5 } else { unit(ins[6].at(i)) };
        let selected = ins[7].at(i).round().clamp(0.0, 9.0) as u8;
        let auto = ins[8].at(i) >= 0.5;
        let gate = ins[9].at(i) >= 0.5;
        let trigger = ins[10].at(i) >= 0.5;
        let aux = ins[11].at(i) >= 0.5;
        let edge = (gate && mem[0] < 0.5) || (trigger && mem[1] < 0.5);
        mem[0] = if gate { 1.0 } else { 0.0 };
        mem[1] = if trigger { 1.0 } else { 0.0 };
        if edge {
            mem[4] = if auto && random(&mut st.u[0]) < transition {
                (random(&mut st.u[0]) * if voice { 10.0 } else { 4.0 }).floor()
            } else {
                selected as f32
            };
            mem[3] = 0.0;
            mem[5] = 1.0;
        }
        let digit = (mem[4] as u8).min(9);
        let rnd = random(&mut st.u[0]) * 2.0 - 1.0;
        let frequency = (80.0 + tone * 240.0) * note;
        mem[2] =
            (mem[2] + frequency * if voice { 1.0 } else { f32::from(digit % 4 + 1) } / sr).fract();
        let dry = if voice {
            let recipe = syllable(digit);
            let progress = mem[3] / recipe.duration;
            mem[3] += 1.0 / sr;
            let active = progress < 1.0 && mem[5] > 0.5;
            if !active {
                mem[5] = 0.0;
            }
            let t = progress.clamp(0.0, 1.0);
            let vowel = mem[2].mul_add(TAU, 0.0).sin() + 0.3 * (2.0 * TAU * mem[2]).sin();
            let onset = ((0.18 - t) / 0.18).clamp(0.0, 1.0) * recipe.onset_noise;
            let input = vowel * (1.0 - onset * 0.65) + rnd * onset;
            let f1 = recipe.first.0 + (recipe.last.0 - recipe.first.0) * t;
            let f2 = recipe.first.1 + (recipe.last.1 - recipe.first.1) * t;
            let first = formant(input, f1, sr, mem, 6);
            let second = formant(input, f2, sr, mem, 8);
            let envelope = (t * 30.0).min(1.0) * ((1.0 - t) * 8.0).min(1.0);
            if active {
                (first * 0.6 + second * 0.4) * envelope
            } else {
                0.0
            }
        } else {
            let target = if gate { 1.0 } else { 0.0 };
            mem[5] += (target - mem[5]) * (1.0 - (-1.0 / (0.008 * sr)).exp());
            (TAU * mem[2]).sin() * mem[5]
        };
        mem[10] += (rnd - mem[10]) * (1.0 - (-1.0 / (0.002 * sr)).exp());
        let mixed = dry * (1.0 - noise * 0.7) + mem[10] * noise * mem[5] * 0.25;
        let processed = (mixed * (1.0 + drive * 9.0)).tanh() / (1.0 + drive * 0.5);
        *sample = if aux { dry } else { processed }.clamp(-1.0, 1.0);
    }
}
