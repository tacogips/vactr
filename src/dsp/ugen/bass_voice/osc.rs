//! Band-limited bass oscillators based on published polynomial correction methods.
//!
//! The saw, pulse, and square discontinuities use polyBLEP corrections
//! (Valimaki and Huovilainen, 2007; Valimaki, Pekonen, and Nam, 2012). The
//! triangle corners use an integrated polynomial correction (Esqueda,
//! Valimaki, and Bilbao, 2016). No source code is consulted or translated.

use std::f32::consts::TAU;

/// The oscillator waveform, in the order used by the `wave` control row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Saw,
    Pulse,
    Square,
    Tri,
    Sine,
}

impl Wave {
    /// Maps the numeric enum port to a waveform.
    #[must_use]
    pub fn from_index(value: f32) -> Self {
        if !value.is_finite() {
            return Self::Saw;
        }
        match value.round().clamp(0.0, 4.0) as u8 {
            0 => Self::Saw,
            1 => Self::Pulse,
            2 => Self::Square,
            3 => Self::Tri,
            _ => Self::Sine,
        }
    }
}

/// Fixed width of the pulse waveform.
pub const PULSE_WIDTH: f32 = 0.3;
/// Click pitch peak in semitones.
pub const CLICK_SEMIS: f32 = 24.0;
/// Click pitch decay time constant, seconds.
pub const CLICK_TAU: f32 = 0.0015;

/// One phase accumulator shared by all waveform shapes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Osc {
    phase: f32,
}

impl Osc {
    /// Number of floating-point values needed to store this oscillator.
    pub const FLOATS: usize = 1;

    /// Creates an oscillator at a wrapped phase.
    #[must_use]
    pub fn with_phase(phase: f32) -> Self {
        Self {
            phase: if phase.is_finite() {
                phase.rem_euclid(1.0)
            } else {
                0.0
            },
        }
    }

    /// Restores oscillator state from its fixed-size arena slice.
    #[must_use]
    pub fn load(src: &[f32]) -> Self {
        Self::with_phase(src[0])
    }

    /// Saves oscillator state to its fixed-size arena slice.
    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.phase;
    }

    /// Produces one sample and advances the phase by `inc` cycles.
    #[must_use]
    pub fn next(&mut self, wave: Wave, inc: f32) -> f32 {
        let dt = if inc.is_finite() {
            inc.clamp(0.0, 0.25)
        } else {
            0.0
        };
        let phase = self.phase;
        let sample = match wave {
            Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
            Wave::Pulse => {
                let low = -PULSE_WIDTH / (1.0 - PULSE_WIDTH);
                let raw = if phase < PULSE_WIDTH { 1.0 } else { low };
                let edge = 0.5 / (1.0 - PULSE_WIDTH);
                raw + edge * poly_blep(phase, dt)
                    - edge * poly_blep((phase - PULSE_WIDTH).rem_euclid(1.0), dt)
            }
            Wave::Square => {
                let raw = if phase < 0.5 { 1.0 } else { -1.0 };
                raw + poly_blep(phase, dt) - poly_blep((phase - 0.5).rem_euclid(1.0), dt)
            }
            Wave::Tri => {
                let raw = if phase < 0.5 {
                    -1.0 + 4.0 * phase
                } else {
                    3.0 - 4.0 * phase
                };
                raw + 8.0 * dt * poly_blamp_residual(phase, 0.0, dt)
                    - 8.0 * dt * poly_blamp_residual(phase, 0.5, dt)
            }
            Wave::Sine => (TAU * phase).sin(),
        };
        self.phase = (phase + dt).rem_euclid(1.0);
        if sample.is_finite() {
            sample
        } else {
            0.0
        }
    }
}

#[inline]
fn poly_blep(phase: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if phase < dt {
        let t = phase / dt;
        t + t - t * t - 1.0
    } else if phase > 1.0 - dt {
        let t = (phase - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[inline]
fn poly_blamp_residual(phase: f32, corner: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    let offset = (phase - corner + 0.5).rem_euclid(1.0) - 0.5;
    let x = offset / dt;
    let abs_x = x.abs();
    if abs_x < 1.0 {
        let x2 = abs_x * abs_x;
        let x4 = x2 * x2;
        let x5 = x4 * abs_x;
        7.0 / 30.0 - abs_x / 2.0 + x2 / 3.0 - x4 / 12.0 + x5 / 40.0
    } else if abs_x <= 2.0 {
        let tail = 2.0 - abs_x;
        tail * tail * tail * tail * tail / 120.0
    } else {
        0.0
    }
}

/// Pitch offset for the short attack click, in semitones.
#[must_use]
pub fn click_semis(t_seconds: f32, level: f32) -> f32 {
    if !t_seconds.is_finite() || !level.is_finite() || level <= 0.0 {
        0.0
    } else {
        level * CLICK_SEMIS * (-t_seconds / CLICK_TAU).exp()
    }
}

/// Symmetric oscillator frequency multipliers for a detuned pair.
#[must_use]
pub fn detune_ratios(detune: f32) -> (f32, f32) {
    let d = if detune.is_finite() {
        detune.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (2.0_f32.powf(-d / 24.0), 2.0_f32.powf(d / 24.0))
}
