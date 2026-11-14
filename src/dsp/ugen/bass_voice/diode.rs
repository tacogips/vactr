//! Four-pole coupled TPT diode ladder from the topology discussed by
//! Stinchcombe (2008), Zavalishin (2020) and Pirkle (2021). This is an original
//! fixed-size linear solve with one input nonlinearity and no translated code.

use super::sanitize;

/// Maximum feedback used by the coupled diode ladder.
pub const K_DIODE_MAX: f32 = 2.5;
/// Full-resonance output compensation in decibels.
const RESONANCE_COMP_DB: f32 = 22.0;
/// Extreme-signal guard below the component's strict 16.0 output bound.
const MAX_OUTPUT: f32 = 15.0;
/// Cutoff pre-scale compensating the coupled ladder's lower unscaled corner.
pub const DIODE_FC_SCALE: f32 = 2.9;
/// High-pass corner in the resonance feedback path, in hertz.
pub const FB_HP_HZ: f32 = 60.0;

/// Persistent states for four coupled poles and the feedback-path low-pass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Diode {
    stages: [f32; 4],
    feedback_lp: f32,
}

impl Diode {
    /// Number of persistent state floats.
    pub const FLOATS: usize = 5;

    /// Restores all filter states from voice memory.
    pub fn load(mem: &[f32]) -> Self {
        Self {
            stages: [mem[0], mem[1], mem[2], mem[3]],
            feedback_lp: mem[4],
        }
    }

    /// Saves all filter states to voice memory.
    pub fn store(&self, mem: &mut [f32]) {
        mem[..4].copy_from_slice(&self.stages);
        mem[4] = self.feedback_lp;
    }

    /// Clears the four integrators and feedback-path state.
    pub fn reset(&mut self) {
        self.stages = [0.0; 4];
        self.feedback_lp = 0.0;
    }

    /// Flushes denormals; any non-finite state resets the complete filter.
    pub fn flush(&mut self) {
        if !self.feedback_lp.is_finite() || self.stages.iter().any(|s| !s.is_finite()) {
            self.reset();
        } else {
            self.stages.iter_mut().for_each(|s| *s = sanitize(*s));
            self.feedback_lp = sanitize(self.feedback_lp);
        }
    }

    /// Processes one sample through the fixed-size coupled ladder solve.
    #[inline]
    pub fn process(&mut self, x: f32, c: &DiodeCoeffs) -> f32 {
        if !x.is_finite()
            || !c.is_finite()
            || !self.feedback_lp.is_finite()
            || self.stages.iter().any(|s| !s.is_finite())
        {
            self.reset();
            return 0.0;
        }

        let linear_state = [
            (1.0 - c.g) * self.stages[0],
            (1.0 - c.g) * self.stages[1],
            (1.0 - c.g) * self.stages[2],
            (1.0 - c.g) * self.stages[3],
        ];
        let base = solve_coupled(linear_state, c.g);
        let sensitivity = solve_coupled([c.g, 0.0, 0.0, 0.0], c.g);
        let fb_coeff = c.hp_coeff;
        let hp_q = fb_coeff * (base[3] - self.feedback_lp);
        let hp_sensitivity = fb_coeff * sensitivity[3];
        let solved = (c.drive_gain * x - c.k * hp_q) / (1.0 + c.k * hp_sensitivity);
        let driven = solved.tanh();
        let stages = [
            base[0] + sensitivity[0] * driven,
            base[1] + sensitivity[1] * driven,
            base[2] + sensitivity[2] * driven,
            base[3] + sensitivity[3] * driven,
        ];
        let raw_output = stages[3] * c.output_gain;
        if !raw_output.is_finite() || stages.iter().any(|s| !s.is_finite()) {
            self.reset();
            return 0.0;
        }
        for (state, value) in self.stages.iter_mut().zip(stages) {
            *state = sanitize(2.0 * value - *state);
        }
        self.feedback_lp =
            sanitize(self.feedback_lp + (1.0 - fb_coeff) * (stages[3] - self.feedback_lp));
        raw_output.clamp(-MAX_OUTPUT, MAX_OUTPUT)
    }
}

/// Per-sample coefficients for the coupled diode ladder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiodeCoeffs {
    g: f32,
    k: f32,
    drive_gain: f32,
    hp_coeff: f32,
    output_gain: f32,
}

impl DiodeCoeffs {
    /// Builds bounded coefficients for the requested sample rate and controls.
    pub fn new(cutoff_hz: f32, res: f32, drive: f32, sr: f32) -> Self {
        let sr = if sr.is_finite() {
            sr.max(1.0)
        } else {
            48_000.0
        };
        let nominal = if cutoff_hz.is_finite() {
            cutoff_hz.clamp(20.0, 0.45 * sr)
        } else {
            1000.0_f32.min(0.45 * sr)
        };
        let cutoff = (nominal * DIODE_FC_SCALE).clamp(20.0, 0.45 * sr);
        let res = finite_unit(res);
        let drive = finite_unit(drive);
        let g = (std::f32::consts::PI * cutoff / sr).tan();
        let hp_coeff = (-2.0 * std::f32::consts::PI * FB_HP_HZ / sr).exp();
        let k = K_DIODE_MAX * res;
        Self {
            g: g / (1.0 + g),
            k,
            drive_gain: 1.0 + 7.0 * drive,
            hp_coeff,
            output_gain: 10.0_f32.powf(RESONANCE_COMP_DB * res * (1.0 - 0.5 * drive) / 20.0),
        }
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.g.is_finite()
            && self.k.is_finite()
            && self.drive_gain.is_finite()
            && self.hp_coeff.is_finite()
            && self.output_gain.is_finite()
    }
}

/// Solves the four fixed coupled stage equations without allocation.
#[inline]
fn solve_coupled(mut rhs: [f32; 4], g: f32) -> [f32; 4] {
    let coupling = 0.10;
    let upper = -g * coupling;
    let lower = -g;
    let mut diagonal = [1.0; 4];
    let upper_work = [upper; 4];
    let mut rhs_work = rhs;
    for i in 1..4 {
        let factor = lower / diagonal[i - 1];
        diagonal[i] -= factor * upper_work[i - 1];
        rhs_work[i] -= factor * rhs_work[i - 1];
    }
    rhs[3] = rhs_work[3] / diagonal[3];
    rhs[2] = (rhs_work[2] - upper_work[2] * rhs[3]) / diagonal[2];
    rhs[1] = (rhs_work[1] - upper_work[1] * rhs[2]) / diagonal[1];
    rhs[0] = (rhs_work[0] - upper_work[0] * rhs[1]) / diagonal[0];
    rhs
}

#[inline]
fn finite_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
