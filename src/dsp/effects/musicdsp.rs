//! Original MusicDSP-inspired waveshapers, complex feedback, and nonlinear FIR.
//! Research references: entries 203, 104, 70 and 207; no snippet source imported.
//! The FIR retains each input sample's amplitude region throughout its history.

use super::prim::{db_to_gain, DelayLine};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;
use std::f32::consts::{PI, TAU};

const FOLDBACK: [ParamDef; 4] = [
    ParamDef::new("threshold", 0.45, 0.01, 1.0),
    ParamDef::unit("drive", 12.0, 0.0, 36.0, "dB"),
    ParamDef::unit("output", -6.0, -36.0, 12.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const VARIABLE_CLIP: [ParamDef; 4] = [
    ParamDef::new("hardness", 0.5, 0.0, 1.0),
    ParamDef::unit("drive", 9.0, 0.0, 36.0, "dB"),
    ParamDef::unit("output", -6.0, -36.0, 12.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const ALIEN_WAH: [ParamDef; 6] = [
    ParamDef::unit("rate", 0.35, 0.0, 12.0, "Hz"),
    ParamDef::new("depth", 0.7, 0.0, 1.0),
    ParamDef::new("feedback", 0.75, -0.95, 0.95),
    ParamDef::unit("time", 0.008, 0.0001, 0.04, "s"),
    ParamDef::new("stereo", 0.5, 0.0, 1.0),
    ParamDef::new("mix", 0.65, 0.0, 1.0),
];
const DYNAMIC_CONVOLUTION: [ParamDef; 5] = [
    ParamDef::unit("freq", 1200.0, 40.0, 16000.0, "Hz"),
    ParamDef::new("sweep", 1.5, -0.9, 6.0),
    ParamDef::unit("decay", 0.004, 0.0001, 0.05, "s"),
    ParamDef::unit("drive", 6.0, 0.0, 30.0, "dB"),
    ParamDef::new("mix", 0.7, 0.0, 1.0),
];
/// The bank's fixed CPU bound: eight kernels, 192 taps, independent stereo history.
pub const FIR_BANKS: usize = 8;
pub const FIR_TAPS: usize = 192;
const FIR_TABLE: usize = FIR_BANKS * FIR_TAPS;
/// Includes kernels, two input histories and two amplitude-coordinate histories.
pub const FIR_MEMORY: usize = FIR_TABLE + 4 * FIR_TAPS;

pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    match kind {
        EffectKind::Foldback => &FOLDBACK,
        EffectKind::VariableClip => &VARIABLE_CLIP,
        EffectKind::AlienWah => &ALIEN_WAH,
        EffectKind::DynamicConvolution => &DYNAMIC_CONVOLUTION,
        _ => &[],
    }
}
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    match kind {
        EffectKind::AlienWah => ((0.04 * sr).ceil() as usize + 2) * 4,
        EffectKind::DynamicConvolution => FIR_MEMORY,
        _ => 0,
    }
}
pub(super) fn init(kind: EffectKind, st: &mut FxState, memory: usize, sr: f32) {
    if kind == EffectKind::AlienWah {
        let mut cursor = 0;
        let len = (0.04 * sr).ceil() as usize + 2;
        for line in &mut st.dl[..4] {
            *line = DelayLine::carve(&mut cursor, memory, len);
        }
    }
}

/// Periodic threshold reflection, bounded to `[-threshold, threshold]`.
#[must_use]
pub fn fold_sample(input: f32, threshold: f32) -> f32 {
    let threshold = threshold.max(0.0001);
    let phase = (input + threshold).rem_euclid(4.0 * threshold);
    if phase <= 2.0 * threshold {
        phase - threshold
    } else {
        3.0 * threshold - phase
    }
}

/// Generalized soft sign. Log-safe reciprocal branches approach a hard clip
/// continuously as the exponent rises, with unity small-signal slope.
#[must_use]
pub fn clip_sample(input: f32, hardness: f32) -> f32 {
    let exponent = 1.0 + 63.0 * hardness.clamp(0.0, 1.0).powi(2);
    let amplitude = input.abs();
    let shaped = if amplitude <= 1.0 {
        amplitude / (1.0 + amplitude.powf(exponent)).powf(exponent.recip())
    } else {
        (1.0 + amplitude.recip().powf(exponent)).powf(-exponent.recip())
    };
    shaped.copysign(input)
}

fn complex_wah(
    p: &[f32],
    st: &mut FxState,
    memory: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let feedback = p[2].clamp(-0.95, 0.95);
    let injection = 1.0 - feedback.abs();
    let delay = (p[3] * sr).max(1.0);
    let step = TAU * p[0] / sr;
    for (left, right) in l.iter_mut().zip(r) {
        for (lane, sample) in [left, right].into_iter().enumerate() {
            let phase = st.s[0] + if lane == 1 { p[4] * PI * 0.5 } else { 0.0 };
            let rotation = p[1] * PI * phase.sin();
            let (sin, cos) = rotation.sin_cos();
            let real = st.dl[2 * lane].read(memory, delay);
            let imaginary = st.dl[2 * lane + 1].read(memory, delay);
            let next_real = injection * *sample + feedback * (cos * real - sin * imaginary);
            let next_imaginary = feedback * (sin * real + cos * imaginary);
            st.dl[2 * lane].write(memory, next_real);
            st.dl[2 * lane + 1].write(memory, next_imaginary);
            *sample = next_real;
        }
        st.s[0] = (st.s[0] + step).rem_euclid(TAU);
    }
}

fn prepare_kernels(p: &[f32], st: &mut FxState, memory: &mut [f32], sr: f32) {
    if st.s[..3] == p[..3] {
        return;
    }
    st.s[..3].copy_from_slice(&p[..3]);
    // Rebuild only on coefficient automation, with a fixed 8*192 bound.
    for bank in 0..FIR_BANKS {
        let region = bank as f32 / (FIR_BANKS - 1) as f32;
        let mut phase = 0.0;
        let mut norm = 0.0;
        for tap in 0..FIR_TAPS {
            let age = tap as f32 / sr;
            let chirp = (1.0 + p[1] * region * tap as f32 / FIR_TAPS as f32).max(0.05);
            let frequency = (p[0] * chirp).clamp(10.0, sr * 0.45);
            phase += TAU * frequency / sr;
            let kernel = phase.sin() * (-age / p[2].max(0.0001)).exp();
            memory[bank * FIR_TAPS + tap] = kernel;
            norm += kernel.abs();
        }
        for kernel in &mut memory[bank * FIR_TAPS..(bank + 1) * FIR_TAPS] {
            *kernel /= norm.max(1e-6);
        }
    }
}
fn dynamic_fir(
    p: &[f32],
    st: &mut FxState,
    memory: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    if memory.len() < FIR_MEMORY {
        return;
    }
    prepare_kernels(p, st, memory, sr);
    let drive = db_to_gain(p[3]);
    let mut head = st.s[3] as usize;
    for (left, right) in l.iter_mut().zip(r) {
        for (lane, sample) in [left, right].into_iter().enumerate() {
            let input = (*sample * drive).clamp(-8.0, 8.0);
            let samples = FIR_TABLE + lane * 2 * FIR_TAPS;
            let regions = samples + FIR_TAPS;
            memory[samples + head] = input;
            memory[regions + head] = input.abs().min(1.0) * (FIR_BANKS - 1) as f32;
            let mut output = 0.0;
            for tap in 0..FIR_TAPS {
                let index = (head + FIR_TAPS - tap) % FIR_TAPS;
                // This coordinate belongs to the historical input, not today's input.
                let coordinate = memory[regions + index];
                let bank = coordinate.floor() as usize;
                let fraction = coordinate - bank as f32;
                let a = memory[bank * FIR_TAPS + tap];
                let b = memory[bank.min(FIR_BANKS - 2) * FIR_TAPS + FIR_TAPS + tap];
                let kernel = if bank == FIR_BANKS - 1 {
                    a
                } else {
                    a + fraction * (b - a)
                };
                output += memory[samples + index] * kernel;
            }
            *sample = output;
        }
        head = (head + 1) % FIR_TAPS;
    }
    st.s[3] = head as f32;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    memory: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    match kind {
        EffectKind::Foldback => {
            let drive = db_to_gain(p[1]);
            let output = db_to_gain(p[2]);
            for sample in l.iter_mut().chain(r) {
                *sample = fold_sample(*sample * drive, p[0]) * output;
            }
        }
        EffectKind::VariableClip => {
            let drive = db_to_gain(p[1]);
            let output = db_to_gain(p[2]);
            for sample in l.iter_mut().chain(r) {
                *sample = clip_sample(*sample * drive, p[0]) * output;
            }
        }
        EffectKind::AlienWah => complex_wah(p, st, memory, l, r, ctx.sr),
        EffectKind::DynamicConvolution => dynamic_fir(p, st, memory, l, r, ctx.sr),
        _ => {}
    }
}
