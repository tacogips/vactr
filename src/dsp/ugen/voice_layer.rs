//! Original fixed-state DSP math for the shared Plaits voice layer.
//!
//! The behavior is informed by Emilie Gillet's MIT-licensed
//! `plaits/dsp/voice.cc`, `plaits/dsp/voice.h`, `plaits/dsp/envelope.h`,
//! `plaits/dsp/fx/low_pass_gate.h`, and `stmlib/dsp/limiter.h`,
//! `stmlib/dsp/filter.h`, `stmlib/dsp/parameter_interpolator.h`.
//! No upstream code, lookup table, sample, preset or resource data is used.

use std::f32::consts::PI;

pub const REFERENCE_RATE: f32 = 48_000.0;
pub const REFERENCE_BLOCK: f32 = 12.0;
pub const DONE_FLOOR: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LpgMode {
    Off,
    Ping,
    Level,
}

impl LpgMode {
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

pub fn decay_terms(decay: f32, color: f32) -> (f32, f32) {
    let d = unit_or(decay, 0.5);
    let h = unit_or(color, 0.5);
    let short = 0.05 * 2.0_f32.powf(-8.0 * d);
    let tail = 0.005 * 2.0_f32.powf(-6.0 * d + h) - short;
    (short, tail)
}

pub fn compress_level(level: f32) -> f32 {
    if !level.is_finite() {
        return 0.0;
    }
    (1.3 * level / (0.3 + level.abs())).clamp(0.0, 1.0)
}

pub fn shaped_amount(amount: f32) -> f32 {
    let a = amount.clamp(-1.0, 1.0);
    1.05 * a * (a.abs() - 0.05).max(0.05)
}

pub fn ping_attack(freq_hz: f32) -> f32 {
    let frequency = if freq_hz.is_finite() {
        freq_hz.clamp(1.0, 24_000.0)
    } else {
        440.0
    };
    24.0 * frequency / REFERENCE_RATE
}

pub fn post_gain(registered: f32) -> f32 {
    if registered < 0.0 {
        1.0
    } else {
        registered
    }
}

pub fn clip_unit(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(-1.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlClock {
    pub carry: f32,
}

impl ControlClock {
    pub const FLOATS: usize = 1;

    pub fn load(src: &[f32]) -> Self {
        Self { carry: src[0] }
    }

    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.carry;
    }

    pub fn next_len(&mut self, sr: f32) -> usize {
        let ratio = REFERENCE_BLOCK * sr / REFERENCE_RATE;
        self.carry += ratio;
        let len = (self.carry.floor() as usize).max(1);
        self.carry -= len as f32;
        len
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecayEnvelope {
    pub value: f32,
}

impl DecayEnvelope {
    pub const FLOATS: usize = 1;

    pub fn load(src: &[f32]) -> Self {
        Self { value: src[0] }
    }

    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.value;
    }

    pub fn trigger(&mut self) {
        self.value = 1.0;
    }

    pub fn process(&mut self, short_decay: f32) {
        self.value *= 1.0 - 2.0 * short_decay;
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VactrolEnvelope {
    pub state: f32,
    pub gain: f32,
    pub frequency: f32,
    pub hf_bleed: f32,
    pub ramp_up: bool,
}

impl VactrolEnvelope {
    pub const FLOATS: usize = 5;

    pub fn load(src: &[f32]) -> Self {
        Self {
            state: src[0],
            gain: src[1],
            frequency: src[2],
            hf_bleed: src[3],
            ramp_up: src[4] != 0.0,
        }
    }

    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.state;
        dst[1] = self.gain;
        dst[2] = self.frequency;
        dst[3] = self.hf_bleed;
        dst[4] = f32::from(self.ramp_up);
    }

    pub fn new() -> Self {
        Self {
            state: 0.0,
            gain: 1.0,
            frequency: 0.5,
            hf_bleed: 0.0,
            ramp_up: false,
        }
    }

    pub fn trigger(&mut self) {
        self.ramp_up = true;
    }

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

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    pub fn hf_bleed(&self) -> f32 {
        self.hf_bleed
    }

    pub fn is_done(&self) -> bool {
        !self.ramp_up && self.state < DONE_FLOOR
    }
}

impl Default for VactrolEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LowPassGate {
    pub value: f32,
    pub increment: f32,
    pub g: f32,
    pub h: f32,
    pub bleed: f32,
    pub s1: f32,
    pub s2: f32,
    pub prev_gain: f32,
}

impl LowPassGate {
    pub const FLOATS: usize = 8;

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
pub struct PostLimiter {
    pub peak: f32,
}

impl PostLimiter {
    pub const FLOATS: usize = 1;

    pub fn load(src: &[f32]) -> Self {
        Self { peak: src[0] }
    }

    pub fn store(&self, dst: &mut [f32]) {
        dst[0] = self.peak;
    }

    pub fn new() -> Self {
        Self { peak: 0.5 }
    }

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
