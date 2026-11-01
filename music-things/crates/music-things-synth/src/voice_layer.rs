//! Original fixed-state DSP math for the shared Plaits voice layer.
//!
//! The behavior is informed by Emilie Gillet's MIT-licensed
//! `plaits/dsp/voice.cc`, `plaits/dsp/voice.h`, `plaits/dsp/envelope.h`,
//! `plaits/dsp/fx/low_pass_gate.h`, and `stmlib/dsp/limiter.h`,
//! `stmlib/dsp/filter.h`, `stmlib/dsp/parameter_interpolator.h`.
//! No upstream code, lookup table, sample, preset or resource data is used.

use std::f32::consts::PI;

/// Reference rate value.
pub const REFERENCE_RATE: f32 = 48_000.0;
/// Reference block value.
pub const REFERENCE_BLOCK: f32 = 12.0;
/// Done floor value.
pub const DONE_FLOOR: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Lpgmode for the voice layer.
pub enum LpgMode {
    /// Off mode.
    Off,
    /// Ping mode.
    Ping,
    /// Level mode.
    Level,
}

impl LpgMode {
    /// Performs the from control operation.
    pub fn from_control(v: f32) -> Self {
        if !v.is_finite() || v < 0.5 {
            Self::Off
        } else if v < 1.5 {
            Self::Ping
        } else {
            Self::Level
        }
    }
}

fn unit_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

/// Performs the decay terms operation.
pub fn decay_terms(decay: f32, color: f32) -> (f32, f32) {
    let d = unit_or(decay, 0.5);
    let h = unit_or(color, 0.5);
    let short = 0.05 * 2.0_f32.powf(-8.0 * d);
    let tail = 0.005 * 2.0_f32.powf(-6.0 * d + h) - short;
    (short, tail)
}

/// Performs the compress level operation.
pub fn compress_level(level: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    (1.3 * level / (0.3 + level.abs())).clamp(0.0, 1.0)
}

/// Performs the shaped amount operation.
pub fn shaped_amount(amount: f32) -> f32 {
    let a = amount.clamp(-1.0, 1.0);
    1.05 * a * (a.abs() - 0.05).max(0.05)
}

/// Performs the ping attack operation.
pub fn ping_attack(freq_hz: f32) -> f32 {
    let frequency = if freq_hz.is_finite() {
        freq_hz.clamp(1.0, 24_000.0)
    } else {
        440.0
    };
    24.0 * frequency / REFERENCE_RATE
}

/// Performs the post gain operation.
pub fn post_gain(registered: f32) -> f32 {
    if registered < 0.0 {
        1.0
    } else {
        registered
    }
}

/// Performs the clip unit operation.
pub fn clip_unit(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(-1.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Controlclock for the voice layer.
pub struct ControlClock {
    /// Carry state.
    pub carry: f32,
}

impl ControlClock {
    /// Floats value.
    pub const FLOATS: usize = 1;

    /// Performs the load operation.
    pub fn load(src: &[f32]) -> Self {
        Self { carry: src[0] }
    }

    /// Performs the store operation.
    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.carry;
    }

    /// Performs the next len operation.
    pub fn next_len(&mut self, sr: f32) -> usize {
        let ratio = REFERENCE_BLOCK * sr / REFERENCE_RATE;
        self.carry += ratio;
        let len = (self.carry.floor() as usize).max(1);
        self.carry -= len as f32;
        len
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Decayenvelope for the voice layer.
pub struct DecayEnvelope {
    /// Value state.
    pub value: f32,
}

impl DecayEnvelope {
    /// Floats value.
    pub const FLOATS: usize = 1;

    /// Performs the load operation.
    pub fn load(src: &[f32]) -> Self {
        Self { value: src[0] }
    }

    /// Performs the store operation.
    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.value;
    }

    /// Performs the trigger operation.
    pub fn trigger(&mut self) {
        self.value = 1.0;
    }

    /// Performs the process operation.
    pub fn process(&mut self, short_decay: f32) {
        self.value *= 1.0 - 2.0 * short_decay;
    }

    /// Performs the value operation.
    pub fn value(&self) -> f32 {
        self.value
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Optoenvelope for the voice layer.
pub struct OptoEnvelope {
    /// State state.
    pub state: f32,
    /// Gain state.
    pub gain: f32,
    /// Frequency state.
    pub frequency: f32,
    /// Hf bleed state.
    pub hf_bleed: f32,
    /// Ramp up state.
    pub ramp_up: bool,
}

impl OptoEnvelope {
    /// Floats value.
    pub const FLOATS: usize = 5;

    /// Performs the load operation.
    pub fn load(src: &[f32]) -> Self {
        Self {
            state: src[0],
            gain: src[1],
            frequency: src[2],
            hf_bleed: src[3],
            ramp_up: src[4] != 0.0,
        }
    }

    /// Performs the store operation.
    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.state;
        dst[1] = self.gain;
        dst[2] = self.frequency;
        dst[3] = self.hf_bleed;
        dst[4] = f32::from(self.ramp_up);
    }

    /// Performs the new operation.
    pub fn new() -> Self {
        Self {
            state: 0.0,
            gain: 1.0,
            frequency: 0.5,
            hf_bleed: 0.0,
            ramp_up: false,
        }
    }

    /// Performs the trigger operation.
    pub fn trigger(&mut self) {
        self.ramp_up = true;
    }

    /// Performs the process ping operation.
    pub fn process_ping(&mut self, attack: f32, short_decay: f32, decay_tail: f32, color: f32) {
        if self.ramp_up {
            self.state = (self.state + attack).min(1.0);
            if self.state >= 1.0 {
                self.ramp_up = false;
            }
        }
        let level = if self.ramp_up { self.state } else { 0.0 };
        self.process_lp(level, short_decay, decay_tail, color);
    }

    /// Performs the process lp operation.
    pub fn process_lp(&mut self, level: f32, short_decay: f32, decay_tail: f32, color: f32) {
        let s = self.state;
        let s2 = s * s;
        let s4 = s2 * s2;
        let t = 1.0 - s;
        let t2 = t * t;
        let err = level - s;
        let coefficient = if err > 0.0 {
            0.6
        } else {
            short_decay + (1.0 - s4) * decay_tail
        };
        self.state += coefficient * err;
        self.gain = self.state;
        self.frequency = 0.003 + 0.3 * s4 + 0.04 * color;
        self.hf_bleed = (t2 + (1.0 - t2) * color) * color * color;
    }

    /// Performs the gain operation.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Performs the frequency operation.
    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    /// Performs the hf bleed operation.
    pub fn hf_bleed(&self) -> f32 {
        self.hf_bleed
    }

    /// Performs the is done operation.
    pub fn is_done(&self) -> bool {
        !self.ramp_up && self.state < DONE_FLOOR
    }
}

impl Default for OptoEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Lowpassgate for the voice layer.
pub struct LowPassGate {
    /// Value state.
    pub value: f32,
    /// Increment state.
    pub increment: f32,
    /// G state.
    pub g: f32,
    /// H state.
    pub h: f32,
    /// Bleed state.
    pub bleed: f32,
    /// S1 state.
    pub s1: f32,
    /// S2 state.
    pub s2: f32,
    /// Prev gain state.
    pub prev_gain: f32,
}

impl LowPassGate {
    /// Floats value.
    pub const FLOATS: usize = 8;

    /// Performs the load operation.
    pub fn load(src: &[f32]) -> Self {
        Self {
            value: src[0],
            increment: src[1],
            g: src[2],
            h: src[3],
            bleed: src[4],
            s1: src[5],
            s2: src[6],
            prev_gain: src[7],
        }
    }

    /// Performs the store operation.
    pub fn store(&self, dst: &mut [f32]) {
        dst.copy_from_slice(&[
            self.value,
            self.increment,
            self.g,
            self.h,
            self.bleed,
            self.s1,
            self.s2,
            self.prev_gain,
        ]);
    }

    /// Performs the begin operation.
    pub fn begin(&mut self, target_gain: f32, frequency: f32, hf_bleed: f32, len: usize, sr: f32) {
        let samples = len.max(1) as f32;
        self.increment = (target_gain - self.prev_gain) / samples;
        self.value = self.prev_gain;
        self.prev_gain = target_gain;

        let f = frequency * REFERENCE_RATE / sr;
        let g = f * (PI + 0.3736 * PI * PI * PI * f * f);
        let r = 1.0 / 0.4;
        self.h = 1.0 / (1.0 + r * g + g * g);
        self.g = g;
        self.bleed = hf_bleed;
    }

    /// Performs the tick operation.
    pub fn tick(&mut self, x: f32) -> f32 {
        self.value += self.increment;
        let s = x * self.value;
        let r = 1.0 / 0.4;
        let hp = (s - r * self.s1 - self.g * self.s1 - self.s2) * self.h;
        let bp = self.g * hp + self.s1;
        self.s1 = self.g * hp + bp;
        let lp = self.g * bp + self.s2;
        self.s2 = self.g * bp + lp;
        lp + (s - lp) * self.bleed
    }
}

impl Default for LowPassGate {
    fn default() -> Self {
        Self {
            value: 0.0,
            increment: 0.0,
            g: 0.0,
            h: 1.0,
            bleed: 0.0,
            s1: 0.0,
            s2: 0.0,
            prev_gain: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Postlimiter for the voice layer.
pub struct PostLimiter {
    /// Peak state.
    pub peak: f32,
}

impl PostLimiter {
    /// Floats value.
    pub const FLOATS: usize = 1;

    /// Performs the load operation.
    pub fn load(src: &[f32]) -> Self {
        Self { peak: src[0] }
    }

    /// Performs the store operation.
    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.peak;
    }

    /// Performs the new operation.
    pub fn new() -> Self {
        Self { peak: 0.5 }
    }

    /// Performs the process operation.
    pub fn process(&mut self, pre_gain: f32, sr: f32, x: f32) -> f32 {
        let (attack, release) = if sr == REFERENCE_RATE {
            (0.05, 0.000_02)
        } else {
            let rate_scale = REFERENCE_RATE / sr;
            (
                1.0 - (1.0_f32 - 0.05).powf(rate_scale),
                1.0 - (1.0_f32 - 0.000_02).powf(rate_scale),
            )
        };
        let s = x * pre_gain;
        let err = s.abs() - self.peak;
        self.peak += (if err > 0.0 { attack } else { release }) * err;
        s * if self.peak <= 1.0 {
            1.0
        } else {
            1.0 / self.peak
        } * 0.8
    }
}

impl Default for PostLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
