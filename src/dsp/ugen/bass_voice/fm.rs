//! Feedback FM and digital waveshaping components derived from published equations.
//!
//! The operator feedback model follows Chowning (1973) and Tomisawa
//! (US 4,249,447, 1981). The folder uses first-order antiderivative
//! antialiasing as described by Parker, Zavalishin, and Le Bivic (2016) and
//! Bilbao et al. (2017). No source code is consulted or translated.

use std::f32::consts::TAU;

/// Feedback depth at the maximum feedback control value.
pub const FB_MAX: f32 = 1.5;
/// Sine-folder gain added at maximum fold control.
pub const FOLD_GAIN: f32 = 4.0;

/// Two-operator sine FM with two-sample averaged modulator feedback.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FmPair {
    mod_phase: f32,
    car_phase: f32,
    y1: f32,
    y2: f32,
}

impl FmPair {
    /// Number of floating-point values needed to store this pair.
    pub const FLOATS: usize = 4;

    /// Restores operator state from its fixed-size arena slice.
    #[must_use]
    pub fn load(src: &[f32]) -> Self {
        Self {
            mod_phase: wrap_phase(src[0]),
            car_phase: wrap_phase(src[1]),
            y1: finite_or_zero(src[2]),
            y2: finite_or_zero(src[3]),
        }
    }

    /// Saves operator state to its fixed-size arena slice.
    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.mod_phase;
        dst[1] = self.car_phase;
        dst[2] = self.y1;
        dst[3] = self.y2;
    }

    /// Produces one carrier sample and advances both operator phases.
    #[must_use]
    pub fn next(&mut self, car_inc: f32, ratio: f32, index: f32, feedback: f32) -> f32 {
        let carrier_increment = unit_increment(car_inc);
        let ratio = if ratio.is_finite() {
            ratio.clamp(0.0, 32.0)
        } else {
            0.0
        };
        let index = if index.is_finite() {
            index.clamp(0.0, 32.0)
        } else {
            0.0
        };
        let feedback = if feedback.is_finite() {
            feedback.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let modulator =
            (TAU * self.mod_phase + FB_MAX * feedback * (self.y1 + self.y2) * 0.5).sin();
        let carrier = (TAU * self.car_phase + index * modulator).sin();
        self.y2 = self.y1;
        self.y1 = modulator;
        self.mod_phase = (self.mod_phase + carrier_increment * ratio).rem_euclid(1.0);
        self.car_phase = (self.car_phase + carrier_increment).rem_euclid(1.0);
        finite_or_zero(carrier)
    }
}

/// First-order antiderivative sine folder with an exact zero-fold bypass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Folder {
    previous: f32,
}

impl Folder {
    /// Number of floating-point values needed to store this folder.
    pub const FLOATS: usize = 1;

    /// Restores folder state from its fixed-size arena slice.
    #[must_use]
    pub fn load(src: &[f32]) -> Self {
        Self {
            previous: finite_or_zero(src[0]),
        }
    }

    /// Saves folder state to its fixed-size arena slice.
    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.previous;
    }

    /// Folds `x`, using the integral difference when the input changes.
    #[must_use]
    pub fn process(&mut self, x: f32, fold: f32) -> f32 {
        if !x.is_finite() {
            self.previous = 0.0;
            return 0.0;
        }
        let fold = if fold.is_finite() {
            fold.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if fold <= 0.0 {
            self.previous = x;
            return x;
        }

        let previous = self.previous;
        let gain = 1.0 + FOLD_GAIN * fold;
        let result = if (x - previous).abs() < 1.0e-5 {
            fold_shape((x + previous) * 0.5, gain)
        } else {
            let current_integral = fold_integral(x, gain);
            let previous_integral = fold_integral(previous, gain);
            (current_integral - previous_integral) / (x - previous)
        };
        self.previous = x;
        finite_or_zero(result).clamp(-1.0, 1.0)
    }
}

/// Quantizes to a rounded bit depth; 16 bits and above is an exact bypass.
#[must_use]
pub fn quantize(x: f32, bits: f32) -> f32 {
    if !x.is_finite() || !bits.is_finite() || bits >= 16.0 {
        return x;
    }
    let bits = bits.clamp(2.0, 16.0).round();
    let levels = 2.0_f32.powf(bits - 1.0);
    (x * levels).round() / levels
}

#[inline]
fn fold_shape(x: f32, gain: f32) -> f32 {
    (gain * x * std::f32::consts::FRAC_PI_2).sin()
}

#[inline]
fn fold_integral(x: f32, gain: f32) -> f32 {
    -(gain * x * std::f32::consts::FRAC_PI_2).cos() / (gain * std::f32::consts::FRAC_PI_2)
}

#[inline]
fn unit_increment(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 0.25)
    } else {
        0.0
    }
}

#[inline]
fn wrap_phase(value: f32) -> f32 {
    if value.is_finite() {
        value.rem_euclid(1.0)
    } else {
        0.0
    }
}

#[inline]
fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
