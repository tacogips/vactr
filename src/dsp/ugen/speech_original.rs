//! Procedural Plaits position 15 speech architecture adaptation.
//!
//! Naive formant, SAM-like reset formants, and LPC-like excitation/filter
//! roles are independent implementations. Upper-range words are four original
//! Vactrol syllable tokens, not TI ROM words or upstream phoneme data.

use std::f32::consts::{PI, TAU};

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Oscillator, deterministic excitation, three resonators and lattice state.
pub const STATE_FLOATS: usize = 20;
/// Original upper-range Vactrol syllable tokens: ava, omi, era, unu.
pub const ORIGINAL_WORDS: usize = 4;

fn unit(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn model_and_word(group: f32) -> (usize, usize) {
    let group = unit(group, 0.0);
    if group <= 1.0 / 3.0 {
        ((group * 6.0).floor().min(2.0) as usize, 0)
    } else {
        let word = (((group - 1.0 / 3.0) * 6.0).floor() as usize).min(ORIGINAL_WORDS - 1);
        (3, word)
    }
}

fn random(state: &mut f32) -> f32 {
    let next = ((*state as u32).wrapping_mul(157).wrapping_add(199)) & 0xffff;
    *state = next as f32;
    next as f32 / 32_767.5 - 1.0
}

fn vowel_position(word: usize, age: f32, morph: f32) -> f32 {
    let segment = (age * (1.0 + morph * 4.0)).floor().min(2.0);
    ((word as f32 + 1.0) * 0.173 + segment * 0.291 + word as f32 * segment * 0.067).fract()
}

fn resonator(excitation: f32, frequency: f32, sr: f32, history: &mut [f32]) -> f32 {
    let radius = (-1.0 / (0.004 * sr)).exp();
    let coefficient = 2.0 * radius * (TAU * frequency.min(sr * 0.4) / sr).cos();
    let value = (excitation * 0.06 + coefficient * history[0] - radius * radius * history[1])
        .clamp(-4.0, 4.0);
    history[1] = history[0];
    history[0] = value;
    value
}

fn formant_centers(vowel: f32, timbre: f32) -> [f32; 3] {
    let register = 0.75 + timbre * 0.5;
    [
        (280.0 + vowel * 780.0) * register,
        (1_000.0 + (1.0 - vowel) * 1_300.0) * register,
        (2_100.0 + (PI * vowel).sin() * 650.0) * register,
    ]
}

/// Render source-style formant main or excitation auxiliary into fixed state.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
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
    let auxiliary = ins[6].first() >= 0.5;
    if st.u[0] == 0 {
        mem[2] = ((kx.seed ^ 0x5A31) & 0xffff).max(1) as f32;
        st.u[0] = 1;
    }
    for (frame, output) in out.iter_mut().enumerate() {
        let frequency = if ins[0].at(frame).is_finite() {
            ins[0].at(frame).clamp(20.0, sr * 0.3)
        } else {
            220.0
        };
        let group = unit(ins[1].at(frame), 0.0);
        let timbre = unit(ins[2].at(frame), 0.5);
        let morph = unit(ins[3].at(frame), 0.5);
        let accent = unit(ins[4].at(frame), 1.0);
        let sustain = unit(ins[5].at(frame), 0.0) >= 0.5;
        let (model, word) = model_and_word(group);
        let age = mem[1];
        let vowel = if model == 3 {
            vowel_position(word, age, morph)
        } else {
            morph
        };
        let centers = formant_centers(vowel, timbre);
        let phase_step = frequency / sr;
        mem[0] += phase_step;
        let pulse = if mem[0] >= 1.0 {
            mem[0] -= 1.0;
            1.0
        } else {
            0.0
        };
        let noise = random(&mut mem[2]);
        let excitation = (pulse * 0.85 + noise * (0.02 + vowel * 0.14)) * 0.7;
        let mut naive = 0.0;
        for (index, center) in centers.into_iter().enumerate() {
            naive += resonator(
                excitation,
                center,
                sr,
                &mut mem[4 + index * 2..6 + index * 2],
            );
            mem[14 + index] = (mem[14 + index] + center / sr).fract();
        }
        naive *= 0.16;
        let sam = ((TAU * mem[14]).sin() * 0.5
            + (TAU * mem[15]).sin() * 0.3
            + (TAU * mem[16]).sin() * 0.2)
            * (1.0 - mem[0]).powi(2)
            * 0.45;
        let mut lpc = excitation;
        for stage in 0..4 {
            let reflect =
                (0.18 + stage as f32 * 0.09 + vowel * 0.12 - timbre * 0.08).clamp(-0.8, 0.8);
            let prior = mem[10 + stage];
            let next = (lpc + reflect * prior).clamp(-2.0, 2.0);
            mem[10 + stage] = (prior + 0.12 * (lpc - prior)).clamp(-2.0, 2.0);
            lpc = next;
        }
        let mix = (group * 6.0).fract();
        let voiced = match model {
            0 => naive + (sam - naive) * mix,
            1 => sam + (lpc - sam) * mix,
            2 => lpc,
            _ => lpc * (0.75 + 0.25 * (PI * vowel).sin()),
        };
        let decay = 0.12 + morph * 1.3;
        let target = if sustain && frame < kx.gate {
            0.9
        } else {
            (-age / decay).exp()
        };
        mem[3] += (1.0 - (-1.0 / (0.004 * sr)).exp()) * (target - mem[3]);
        let signal = if auxiliary { excitation * 0.3 } else { voiced };
        *output = (signal * mem[3] * accent).tanh();
        mem[1] = age + 1.0 / sr;
    }
}
