//! Procedural replacements for Braids WAVE_LINE and WAVE_PARAPHONIC (39–40).
//!
//! No upstream `wave_line`, `mini_wave_line`, chord index array, `wt_waves`,
//! `waves.bin`, `map.bin`, generated resource or LXR data is imported.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Four phases, generated sync, strike envelope and scan smoother.
pub const STATE_FLOATS: usize = 7;

fn advance(phase: &mut f32, step: f32) -> bool {
    *phase += step;
    if *phase >= 1.0 {
        *phase -= 1.0;
        true
    } else {
        false
    }
}

fn partial(phase: f32, harmonic: usize, freq: f32, sr: f32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let multiple = harmonic as f32;
    if freq * multiple >= sr * 0.45 {
        0.0
    } else {
        (TAU * phase * multiple).sin()
    }
}

/// One computed periodic node on an original 64-step line.
pub(crate) fn line_cell(index: usize, phase: f32, freq: f32, sr: f32) -> f32 {
    let first = 1 + (index * 3 + index / 7) % 6;
    let second = 2 + (index * 5 + index / 11) % 9;
    let bright = index as f32 / 63.0;
    (partial(phase, first, freq, sr) * (0.7 - bright * 0.25)
        + partial(phase, second, freq, sr) * (0.2 + bright * 0.35)
        + partial(phase, 1, freq, sr) * 0.2)
        * 0.72
}

/// Smooth adjacent-node interpolation or a progressively rough stepped scan.
pub(crate) fn line_wave(scan: f32, roughness: f32, phase: f32, freq: f32, sr: f32) -> f32 {
    let coordinate = scan.clamp(0.0, 1.0) * 63.0;
    let left = coordinate.floor() as usize;
    let right = (left + 1).min(63);
    let fraction = coordinate - left as f32;
    let smooth = line_cell(left, phase, freq, sr) * (1.0 - fraction)
        + line_cell(right, phase, freq, sr) * fraction;
    let stepped = line_cell(left, phase, freq, sr);
    smooth * (1.0 - roughness) + stepped * roughness
}

/// Three original companion intervals; upper half places one note below root.
pub(crate) fn chord_offsets(value: f32) -> [f32; 3] {
    const FAMILIES: [[f32; 3]; 3] = [[2.0, 9.0, 14.0], [5.0, 10.0, 17.0], [3.0, 8.0, 15.0]];
    let segment = (value.clamp(0.0, 1.0) * 6.0).floor().min(5.0) as usize;
    let mut offsets = FAMILIES[segment % 3];
    if segment >= 3 {
        offsets[0] -= 12.0;
    }
    offsets
}

fn voice_wave(phase: f32, color: f32, freq: f32, sr: f32) -> f32 {
    let sine = partial(phase, 1, freq, sr);
    let bright = partial(phase, 3, freq, sr) * 0.55
        + partial(phase, 5, freq, sr) * 0.3
        + partial(phase, 7, freq, sr) * 0.15;
    sine * (1.0 - color * 0.55) + bright * color * 0.55
}

/// Render source position 39 line scan or 40 four-voice chord.
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
    let shape = ins[1].first().round().clamp(39.0, 40.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let root_step = freq / sr;
    let sync_step = root_step * (0.2 + 0.7 * sync);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    let scan_smoothing = 1.0 - (-1.0 / (0.0007 * sr)).exp();
    let chord = chord_offsets(timbre);
    for sample in out.iter_mut() {
        if sync > 0.0 && advance(&mut mem[4], sync_step) {
            mem[..4].fill(0.0);
        }
        let body = if shape == 39 {
            mem[6] += scan_smoothing * (color - mem[6]);
            advance(&mut mem[0], root_step);
            line_wave(mem[6], timbre, mem[0], freq, sr)
        } else {
            let mut mix = 0.0;
            advance(&mut mem[0], root_step);
            mix += voice_wave(mem[0], color, freq, sr);
            for (index, semitones) in chord.into_iter().enumerate() {
                let voice_freq = freq * 2.0f32.powf(semitones / 12.0);
                advance(&mut mem[index + 1], (voice_freq / sr).min(0.45));
                mix += voice_wave(mem[index + 1], color, voice_freq, sr);
            }
            mix * 0.25
        };
        let transient = if mem[5] == 0.0 { strike } else { mem[5] };
        mem[5] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (body * (1.0 - 0.2 * transient) + 0.2 * transient * partial(mem[0], 4, freq, sr))
            .clamp(-1.0, 1.0);
    }
}
