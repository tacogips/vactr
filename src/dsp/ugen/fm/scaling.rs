//! Six-operator FM keyboard, velocity, level and frequency scaling.
//!
//! Adapted from music-synthesizer-for-android (Apache-2.0), revision
//! `f67d41d313b7dc85f6fb99e79e515cc9d208cfff`: `app/src/main/jni/dx7note.cc`,
//! `env.cc`, `env.h`, `exp2.h`, and `exp2.cc`. Modified: translated to Rust,
//! floating-point frequency path, no lookup tables beyond the level curve.

use super::patch::{op, op_base, Fm6Patch};

const LEVEL_CURVE: [i32; 20] = [
    0, 5, 9, 13, 17, 20, 23, 25, 27, 29, 31, 33, 35, 37, 39, 41, 42, 43, 45, 46,
];
const EXP_SCALE: [i32; 33] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 14, 16, 19, 23, 27, 33, 39, 47, 56, 66, 80, 94, 110, 126,
    142, 158, 174, 190, 206, 222, 238, 250,
];
const VELOCITY_DATA: [i32; 64] = [
    0, 70, 86, 97, 106, 114, 121, 126, 132, 138, 142, 148, 152, 156, 160, 163, 166, 170, 173, 174,
    178, 181, 184, 186, 189, 190, 194, 196, 198, 200, 202, 205, 206, 209, 211, 214, 216, 218, 220,
    222, 224, 225, 227, 229, 230, 232, 233, 235, 237, 238, 240, 241, 242, 243, 244, 246, 246, 248,
    249, 250, 251, 252, 253, 254,
];

/// Converts a note frequency to its nearest MIDI key; invalid inputs use A4.
#[must_use]
pub fn midi_key(note_hz: f32) -> i32 {
    if !note_hz.is_finite() || note_hz <= 0.0 {
        return 69;
    }
    (69.0 + 12.0 * (note_hz / 440.0).log2())
        .round()
        .clamp(0.0, 127.0) as i32
}

/// Converts normalized velocity to a MIDI velocity in the range 0 through 127.
#[must_use]
pub fn velocity_midi(velocity: f32) -> i32 {
    if !velocity.is_finite() {
        return 127;
    }
    (velocity * 127.0).round().clamp(0.0, 127.0) as i32
}

/// Maps an operator output-level byte through msfa's log-level table.
#[must_use]
pub fn scale_out_level(outlevel: u8) -> i32 {
    let outlevel = i32::from(outlevel.min(99));
    if outlevel >= 20 {
        28 + outlevel
    } else {
        LEVEL_CURVE[outlevel as usize]
    }
}

/// Computes left/right keyboard level scaling with one of msfa's four curves.
#[must_use]
pub fn scale_level(
    key: i32,
    break_point: u8,
    left_depth: u8,
    right_depth: u8,
    left_curve: u8,
    right_curve: u8,
) -> i32 {
    let offset = key.clamp(0, 127) - i32::from(break_point.min(99)) - 17;
    if offset >= 0 {
        scale_curve(offset / 3, right_depth, right_curve)
    } else {
        scale_curve((-offset) / 3, left_depth, left_curve)
    }
}

/// Computes msfa's keyboard rate increment for a MIDI key and sensitivity.
#[must_use]
pub fn scale_rate(key: i32, sensitivity: u8) -> i32 {
    let x = (key.clamp(0, 127) / 3 - 7).clamp(0, 31);
    (i32::from(sensitivity.min(7)) * x) >> 3
}

/// Computes msfa's velocity-dependent log-level offset.
#[must_use]
pub fn scale_velocity(velocity_midi: i32, sensitivity: u8) -> i32 {
    let velocity = velocity_midi.clamp(0, 127);
    let value = VELOCITY_DATA[(velocity >> 1) as usize] - 239;
    ((i32::from(sensitivity.min(7)) * value + 7) >> 3) << 4
}

/// Composes output level, keyboard level scaling and velocity sensitivity.
///
/// `operator` uses the published DX7 numbering, 1 through 6.
#[must_use]
pub fn op_outlevel(patch: &Fm6Patch, operator: usize, key: i32, velocity_midi: i32) -> i32 {
    let base = op_base(operator);
    if base >= patch.params.len() {
        return 0;
    }
    let params = &patch.params[base..base + 21];
    let level = scale_out_level(params[op::OUTPUT_LEVEL])
        + scale_level(
            key,
            params[op::BREAK_POINT],
            params[op::LEFT_DEPTH],
            params[op::RIGHT_DEPTH],
            params[op::LEFT_CURVE],
            params[op::RIGHT_CURVE],
        );
    let level = level.clamp(0, 127) << 5;
    (level + scale_velocity(velocity_midi, params[op::VELOCITY_SENS])).max(0)
}

/// Computes the operator's keyboard rate-scaling contribution.
#[must_use]
pub fn op_rate_scaling(patch: &Fm6Patch, operator: usize, key: i32) -> i32 {
    let base = op_base(operator);
    if base >= patch.params.len() {
        return 0;
    }
    scale_rate(key, patch.params[base + op::RATE_SCALING])
}

/// Computes an operator frequency from its ratio/fixed mode and detune.
#[must_use]
pub fn op_freq_hz(note_hz: f32, osc_mode: u8, coarse: u8, fine: u8, detune: u8) -> f32 {
    let coarse = coarse.min(31);
    let fine = fine.min(99);
    let detune = detune.min(14);
    let result = if osc_mode == 0 {
        if !note_hz.is_finite() || note_hz <= 0.0 {
            return 0.0;
        }
        let ratio =
            if coarse == 0 { 0.5 } else { f32::from(coarse) } * (1.0 + f32::from(fine) / 100.0);
        let detune_steps = i32::from(detune) - 7;
        note_hz * ratio * 2.0_f32.powf((12_606.0 * detune_steps as f32) / 16_777_216.0)
    } else {
        let coarse_hz = f32::from(coarse & 3) + f32::from(fine) / 100.0;
        let positive_detune = i32::from(detune.saturating_sub(7));
        10.0_f32.powf(coarse_hz) * 2.0_f32.powf((13_457.0 * positive_detune as f32) / 16_777_216.0)
    };
    if result.is_finite() {
        result
    } else {
        0.0
    }
}

/// Applies the patch transpose setting; 24 leaves the note unchanged.
#[must_use]
pub fn transpose_hz(note_hz: f32, transpose: u8) -> f32 {
    if !note_hz.is_finite() {
        return 0.0;
    }
    let semitones = i32::from(transpose.min(48)) - 24;
    let result = note_hz * 2.0_f32.powf(semitones as f32 / 12.0);
    if result.is_finite() {
        result
    } else {
        0.0
    }
}

/// Converts msfa's feedback right-shift into a phase-cycle multiplier.
///
/// msfa sets `fb_shift = 8 - feedback` for nonzero feedback. Each step therefore reduces the
/// right shift by one and doubles the multiplier; feedback zero disables the path.
#[must_use]
pub fn feedback_gain(feedback: u8) -> f32 {
    let feedback = feedback.min(7);
    if feedback == 0 {
        0.0
    } else {
        2.0_f32.powi(i32::from(feedback) - 8)
    }
}

fn scale_curve(group: i32, depth: u8, curve: u8) -> i32 {
    let depth = i32::from(depth.min(99));
    let curve = curve.min(3);
    let group = group.max(0);
    let scale = if curve == 0 || curve == 3 {
        (group * depth * 329) >> 12
    } else {
        let raw = EXP_SCALE[group.min((EXP_SCALE.len() - 1) as i32) as usize];
        (raw * depth * 329) >> 15
    };
    if curve < 2 {
        -scale
    } else {
        scale
    }
}
