//! Five-slot chord/inversion voice with original procedural wave layer.
//!
//! The chord intervals and inversion/fade roles adapt MIT Plaits code.
//! Fifteen analytic timbres replace source integrated-wave selections;
//! no upstream wave or lookup table is imported.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

const CHORDS: [[f32; 4]; 11] = [
    [0.0, 0.01, 11.99, 12.0],
    [0.0, 7.0, 7.01, 12.0],
    [0.0, 5.0, 7.0, 12.0],
    [0.0, 3.0, 7.0, 12.0],
    [0.0, 3.0, 7.0, 10.0],
    [0.0, 3.0, 10.0, 14.0],
    [0.0, 3.0, 10.0, 17.0],
    [0.0, 2.0, 9.0, 16.0],
    [0.0, 4.0, 11.0, 14.0],
    [0.0, 4.0, 7.0, 11.0],
    [0.0, 4.0, 7.0, 12.0],
];
const FADE: [f32; 5] = [0.55, 0.47, 0.49, 0.51, 0.53];
/// Five divide-down phases, five procedural-wave phases and spare state.
pub const STATE_FLOATS: usize = 16;

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[inline]
fn divide_down(phase: f32, registration: f32) -> f32 {
    let saw = 2.0 * phase - 1.0;
    let square = if phase < 0.5 { 1.0 } else { -1.0 };
    let upper = 2.0 * (2.0 * phase).fract() - 1.0;
    let brightness = registration.clamp(0.0, 1.0);
    let blend = saw + (square - saw) * (1.0 - brightness);
    0.8 * blend + 0.2 * brightness * upper
}

#[inline]
fn procedural_wave(phase: f32, index: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let position = index as f32 / 14.0;
    let angle = TAU * phase;
    match index / 5 {
        0 => {
            let harmonic = 2.0 + 4.0 * position;
            (angle.sin() + 0.3 * (harmonic * angle).sin()) / 1.3
        }
        1 => {
            let modulation = 0.3 + 1.7 * position;
            (angle + modulation * ((2.0 + 3.0 * position) * angle).sin()).sin()
        }
        _ => {
            let drive = 1.5 + 4.0 * position;
            (drive * (angle.sin() + 0.25 * (3.0 * angle).sin())).tanh() * 0.8
        }
    }
}

#[inline]
fn wave_layer(phase: f32, waveform: f32) -> f32 {
    let position = (waveform * 14.0).clamp(0.0, 14.0);
    let index = position.floor() as usize;
    if index == 14 {
        return procedural_wave(phase, index);
    }
    let fraction = position.fract();
    let a = procedural_wave(phase, index);
    a + (procedural_wave(phase, index + 1) - a) * fraction
}

#[inline]
fn inversion(chord: [f32; 4], timbre: f32) -> ([f32; 5], [f32; 5], u8) {
    let mut ratios = [1.0; 5];
    let mut amplitudes = [0.0; 5];
    let position = (timbre * 19.999).clamp(0.0, 19.999);
    let integral = position.floor() as usize;
    let fraction = position.fract();
    let rotations = integral / 4;
    let rotated_note = integral % 4;
    let mut mask = 0u8;
    for (note, interval) in chord.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let transposition = 0.25 * 2.0_f32.powi(((3 + integral - note) / 4) as i32);
        let target = (note + 5 - rotations) % 5;
        let previous = (target + 4) % 5;
        let ratio = 2.0_f32.powf(*interval / 12.0) * transposition;
        match note.cmp(&rotated_note) {
            std::cmp::Ordering::Equal => {
                ratios[target] = ratio;
                ratios[previous] = ratio * 2.0;
                amplitudes[target] = 0.25 * (1.0 - fraction);
                amplitudes[previous] = 0.25 * fraction;
            }
            std::cmp::Ordering::Less => {
                ratios[previous] = ratio;
                amplitudes[previous] = 0.25;
            }
            std::cmp::Ordering::Greater => {
                ratios[target] = ratio;
                amplitudes[target] = 0.25;
            }
        }
        if note == 0 {
            if note >= rotated_note {
                mask |= 1 << target;
            }
            if note <= rotated_note {
                mask |= 1 << previous;
            }
        }
    }
    (ratios, amplitudes, mask)
}

/// Mode 0 sums the full chord; mode 1 emphasizes inversion-selected notes.
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
    let freq = ins[0].first().clamp(20.0, sr * 0.12);
    let chord = CHORDS[(unit(ins[1].first()) * 10.999) as usize];
    let timbre = unit(ins[2].first());
    let morph = unit(ins[3].first());
    let auxiliary = ins[4].first() >= 0.5;
    let (ratios, amplitudes, mask) = inversion(chord, timbre);
    let registration = (1.0 - morph * 2.15).max(0.0);
    let waveform = ((morph - 0.535) * 2.15).max(0.0);
    for sample in out.iter_mut() {
        let mut full = 0.0;
        let mut selected = 0.0;
        for voice in 0..5 {
            let hz = (freq * 0.998 * ratios[voice]).min(sr * 0.24);
            let step = hz / sr;
            mem[voice] = (mem[voice] + step).fract();
            mem[voice + 5] = (mem[voice + 5] + (step * 1.004).min(0.24)).fract();
            let wave_amount = (50.0 * (morph - FADE[voice])).clamp(0.0, 1.0);
            let divide_amount = (1.0 - wave_amount) * (4.0 - 32.0 * step).clamp(0.0, 1.0);
            let tone = amplitudes[voice]
                * (divide_amount * divide_down(mem[voice], registration)
                    + wave_amount * wave_layer(mem[voice + 5], waveform));
            full += tone;
            if mask & (1 << voice) != 0 {
                selected += tone;
            }
        }
        *sample = if auxiliary { 3.0 * selected } else { full };
    }
}

#[cfg(test)]
mod tests {
    use super::{inversion, procedural_wave, CHORDS};

    #[test]
    fn fifteen_original_timbres_and_inversion_slots_are_finite() {
        for index in 0..15 {
            let samples = [0.13, 0.29, 0.47, 0.71].map(|phase| procedural_wave(phase, index));
            assert!(samples.iter().all(|sample| sample.is_finite()));
            assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        }
        for value in [0.0, 0.2, 0.5, 0.8, 1.0] {
            let (ratios, amplitudes, mask) = inversion(CHORDS[4], value);
            assert!(ratios.iter().all(|ratio| ratio.is_finite() && *ratio > 0.0));
            assert!((amplitudes.iter().sum::<f32>() - 1.0).abs() < 1e-5);
            assert_ne!(mask, 0);
        }
    }
}
