//! Four-pole TPT transistor ladder following the linear ZDF solution in
//! Zavalishin, *The Art of VA Filter Design* (2020), with input saturation
//! following Huovilainen, DAFx-04 (2004). No reference implementation code is
//! used.

use super::sanitize;

/// Maximum transistor-ladder feedback; slightly above the linear oscillation threshold.
pub const K_LADDER_MAX: f32 = 4.08;
/// Additional input gain at full drive, approximately 18 dB.
const DRIVE_GAIN: f32 = 7.0;
/// Partial compensation for passband loss as resonance increases.
const DRIVE_COMP: f32 = 0.18;
/// Partial output compensation for resonance-dependent passband loss.
const LADDER_COMP: f32 = 0.08;

/// Persistent trapezoidal integrator states for the four ladder poles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ladder {
    stages: [f32; 4],
}

impl Ladder {
    /// Number of persistent state floats.
    pub const FLOATS: usize = 4;

    /// Restores the four stage states from voice memory.
    pub fn load(mem: &[f32]) -> Self {
        Self {
            stages: [mem[0], mem[1], mem[2], mem[3]],
        }
    }

    /// Saves the four stage states to voice memory.
    pub fn store(&self, mem: &mut [f32]) {
        mem[..4].copy_from_slice(&self.stages);
    }

    /// Clears all integrator state.
    pub fn reset(&mut self) {
        self.stages = [0.0; 4];
    }

    /// Flushes denormals; a corrupted stage resets the complete ladder.
    pub fn flush(&mut self) {
        if self.stages.iter().any(|state| !state.is_finite()) {
            self.reset();
        } else {
            self.stages
                .iter_mut()
                .for_each(|state| *state = sanitize(*state));
        }
    }

    /// Processes one sample through the zero-delay-feedback ladder.
    #[inline]
    pub fn process(&mut self, x: f32, c: &LadderCoeffs) -> f32 {
        if !x.is_finite() || !c.is_finite() || self.stages.iter().any(|s| !s.is_finite()) {
            self.reset();
            return 0.0;
        }

        let g = c.g / (1.0 + c.g);
        let g2 = g * g;
        let g4 = g2 * g2;
        let one_minus_g = 1.0 - g;
        let q1 = one_minus_g * self.stages[0];
        let q2 = g * q1 + one_minus_g * self.stages[1];
        let q3 = g * q2 + one_minus_g * self.stages[2];
        let q4 = g * q3 + one_minus_g * self.stages[3];
        let solved = (c.drive_gain * x - c.k * q4) / (1.0 + c.k * g4);
        let input = solved.tanh();

        let mut value = input;
        for stage in &mut self.stages {
            let v = g * (value - *stage);
            value = v + *stage;
            *stage = sanitize(value + v);
        }
        let output = value * c.output_gain;
        if output.is_finite() {
            output
        } else {
            self.reset();
            0.0
        }
    }
}

/// Per-sample coefficients for the transistor ladder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LadderCoeffs {
    g: f32,
    k: f32,
    drive_gain: f32,
    output_gain: f32,
}

impl LadderCoeffs {
    /// Builds bounded coefficients for the requested sample rate and controls.
    pub fn new(cutoff_hz: f32, res: f32, drive: f32, sr: f32) -> Self {
        let sr = if sr.is_finite() {
            sr.max(1.0)
        } else {
            48_000.0
        };
        let cutoff = if cutoff_hz.is_finite() {
            cutoff_hz.clamp(20.0, 0.45 * sr)
        } else {
            1000.0_f32.min(0.45 * sr)
        };
        let res = finite_unit(res);
        let drive = finite_unit(drive);
        let g = (std::f32::consts::PI * cutoff / sr).tan();
        let k = K_LADDER_MAX * res;
        Self {
            g,
            k,
            drive_gain: 1.0 + DRIVE_GAIN * drive,
            output_gain: (1.0 + LADDER_COMP * k) * (1.0 - DRIVE_COMP * drive),
        }
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.g.is_finite()
            && self.k.is_finite()
            && self.drive_gain.is_finite()
            && self.output_gain.is_finite()
    }
}

#[inline]
fn finite_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
