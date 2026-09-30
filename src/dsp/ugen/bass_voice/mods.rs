//! Per-voice envelopes, note timing, accent, glide and wobble modulation.

use super::sanitize;

pub const CPS_MIN: f32 = 0.03;
pub const CPS_MAX: f32 = 50.0;
pub const LEGATO_RAMP_S: f32 = 0.002;
pub const ACCENT_OCT: f32 = 2.0;
pub const ACCENT_DECAY: f32 = 0.2;
pub const ACCENT_GAIN: f32 = 0.4;
pub const WOBBLE_OCT: f32 = 5.0;
const LFO_SMOOTH_S: f32 = 0.002;
const LN_100: f32 = 4.605_170_2;
const LN_1E4: f32 = 9.210_340_5;

pub fn clamp_cps(cps: f32) -> f32 {
    if cps.is_finite() {
        cps.clamp(CPS_MIN, CPS_MAX)
    } else {
        0.5
    }
}

pub fn gate_samples(gate_length: f32, cps: f32, sr: f32) -> u32 {
    let length = if gate_length.is_finite() {
        gate_length.clamp(0.05, 64.0)
    } else {
        1.0
    };
    let sample_rate = if sr.is_finite() {
        sr.max(1.0)
    } else {
        48_000.0
    };
    (length / (16.0 * clamp_cps(cps)) * sample_rate)
        .round()
        .clamp(1.0, u32::MAX as f32) as u32
}

pub fn settle_coeff(time_s: f32, sr: f32) -> f32 {
    let rate = if sr.is_finite() {
        sr.max(1.0)
    } else {
        48_000.0
    };
    let time = if time_s.is_finite() {
        time_s.max(1.0e-4)
    } else {
        1.0e-4
    };
    (-LN_100 / (time * rate)).exp()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AmpEnv {
    pub level: f32,
    pub stage: f32,
}

impl AmpEnv {
    pub const FLOATS: usize = 2;

    pub fn load(src: &[f32]) -> Self {
        Self {
            level: sanitize(src.first().copied().unwrap_or(0.0)).clamp(0.0, 1.0),
            stage: sanitize(src.get(1).copied().unwrap_or(3.0))
                .round()
                .clamp(0.0, 3.0),
        }
    }

    pub fn store(&self, dst: &mut [f32]) {
        if let Some(value) = dst.get_mut(0) {
            *value = sanitize(self.level).clamp(0.0, 1.0);
        }
        if let Some(value) = dst.get_mut(1) {
            *value = sanitize(self.stage).round().clamp(0.0, 3.0);
        }
    }

    pub fn start(&mut self, _legato: bool) {
        self.level = 0.0;
        self.stage = 0.0;
    }

    pub fn release(&mut self) {
        if self.stage != 3.0 {
            self.stage = 2.0;
        }
    }

    pub fn next(&mut self, p: &AmpParams, legato: bool, sr: f32) -> f32 {
        let rate = if sr.is_finite() {
            sr.max(1.0)
        } else {
            48_000.0
        };
        let sustain = finite_clamp(p.sustain, 0.0, 1.0, 1.0);
        let level = sanitize(self.level).clamp(0.0, 1.0);
        match self.stage.round() as u8 {
            0 => {
                let target = if legato { sustain } else { 1.0 };
                let seconds = if legato {
                    LEGATO_RAMP_S
                } else {
                    finite_clamp(p.attack, 0.0, f32::MAX, 0.0)
                };
                let step = target / (seconds * rate).max(1.0);
                self.level = (level + step).min(target);
                if self.level >= target {
                    self.stage = 1.0;
                }
            }
            1 if !legato => {
                let coeff = settle_coeff(p.decay, rate);
                self.level = sustain + (level - sustain) * coeff;
            }
            2 => {
                let seconds = finite_clamp(p.release, 1.0e-4, f32::MAX, 1.0e-4);
                self.level = level * (-LN_1E4 / (seconds * rate)).exp();
                if self.level < 1.0e-4 {
                    self.level = 0.0;
                    self.stage = 3.0;
                }
            }
            3 => self.level = 0.0,
            _ => {
                self.level = 0.0;
                self.stage = 3.0;
            }
        }
        self.level = sanitize(self.level).clamp(0.0, 1.0);
        self.level
    }

    pub fn done(&self) -> bool {
        self.stage.round() == 3.0
    }
}

pub struct AmpParams {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FilterEnv {
    pub value: f32,
}

impl FilterEnv {
    pub const FLOATS: usize = 1;

    pub fn load(src: &[f32]) -> Self {
        Self {
            value: sanitize(src.first().copied().unwrap_or(0.0)).clamp(0.0, 1.0),
        }
    }

    pub fn store(&self, dst: &mut [f32]) {
        if let Some(value) = dst.first_mut() {
            *value = sanitize(self.value).clamp(0.0, 1.0);
        }
    }

    pub fn start(&mut self, legato: bool) {
        self.value = if legato { 0.0 } else { 1.0 };
    }

    pub fn next(&mut self, decay_s: f32, sr: f32) -> f32 {
        let current = sanitize(self.value).clamp(0.0, 1.0);
        self.value = sanitize(current * settle_coeff(decay_s, sr));
        current
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccentOut {
    pub oct: f32,
    pub decay: f32,
    pub gain: f32,
}

pub fn accent(a: f32, res: f32, env_decay: f32) -> AccentOut {
    let accent = finite_clamp(a, 0.0, 1.0, 0.0);
    let resonance = finite_clamp(res, 0.0, 1.0, 0.0);
    let decay = finite_clamp(env_decay, 0.0, f32::MAX, 0.2);
    AccentOut {
        oct: accent * ACCENT_OCT * (0.5 + 0.5 * resonance),
        decay: decay + accent * (decay.min(ACCENT_DECAY) - decay),
        gain: 1.0 + ACCENT_GAIN * accent,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Glide {
    pub offset: f32,
}

impl Glide {
    pub const FLOATS: usize = 1;

    pub fn load(src: &[f32]) -> Self {
        Self {
            offset: finite_clamp(src.first().copied().unwrap_or(0.0), -24.0, 24.0, 0.0),
        }
    }

    pub fn store(&self, dst: &mut [f32]) {
        if let Some(value) = dst.first_mut() {
            *value = finite_clamp(self.offset, -24.0, 24.0, 0.0);
        }
    }

    pub fn start(&mut self, slide_from: f32) {
        self.offset = finite_clamp(slide_from, -24.0, 24.0, 0.0);
    }

    pub fn next(&mut self, slide_time: f32, sr: f32) -> f32 {
        let current = sanitize(self.offset);
        self.offset = current * settle_coeff(slide_time, sr);
        if self.offset.abs() < 1.0e-4 {
            self.offset = 0.0;
        }
        current
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LfoShape {
    Sine,
    Tri,
    Saw,
    Ramp,
    Square,
    Random,
}

impl LfoShape {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Tri,
            2 => Self::Saw,
            3 => Self::Ramp,
            4 => Self::Square,
            5 => Self::Random,
            _ => Self::Sine,
        }
    }
}

pub fn rate_hz(rate: f32, sync: bool, cps: f32) -> f32 {
    finite_clamp(rate, 0.01, 50.0, 0.01) * if sync { clamp_cps(cps) } else { 1.0 }
}

pub fn initial_phase(retrigger: bool, offset: f32, rate_hz: f32, onset: f32) -> f32 {
    if retrigger {
        return finite_or(offset, 0.0).rem_euclid(1.0);
    }
    let rate = finite_or(rate_hz, 0.0) as f64;
    let start = finite_or(onset, 0.0) as f64;
    (rate * start).rem_euclid(1.0) as f32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lfo {
    pub anchor_phase: f32,
    pub anchor_cycles: f32,
    pub count_lo: f32,
    pub count_hi: f32,
    pub rate: f32,
    pub smooth: f32,
}

impl Default for Lfo {
    fn default() -> Self {
        Self {
            anchor_phase: 0.0,
            anchor_cycles: 0.0,
            count_lo: 0.0,
            count_hi: 0.0,
            rate: 0.0,
            smooth: 0.0,
        }
    }
}

impl Lfo {
    pub const FLOATS: usize = 6;
    pub const COUNT_SPLIT: f32 = 1_048_576.0;

    pub fn load(src: &[f32]) -> Self {
        Self {
            anchor_phase: finite_or(src.first().copied().unwrap_or(0.0), 0.0).rem_euclid(1.0),
            anchor_cycles: finite_clamp(src.get(1).copied().unwrap_or(0.0), 0.0, 16_777_216.0, 0.0)
                .floor(),
            count_lo: finite_clamp(
                src.get(2).copied().unwrap_or(0.0),
                0.0,
                Self::COUNT_SPLIT - 1.0,
                0.0,
            )
            .floor(),
            count_hi: finite_clamp(src.get(3).copied().unwrap_or(0.0), 0.0, 16_777_216.0, 0.0)
                .floor(),
            rate: finite_clamp(src.get(4).copied().unwrap_or(0.0), 0.0, f32::MAX, 0.0),
            smooth: finite_clamp(src.get(5).copied().unwrap_or(0.0), 0.0, 1.0, 0.0),
        }
    }

    pub fn store(&self, dst: &mut [f32]) {
        let values = [
            finite_or(self.anchor_phase, 0.0).rem_euclid(1.0),
            finite_clamp(self.anchor_cycles, 0.0, 16_777_216.0, 0.0).floor(),
            finite_clamp(self.count_lo, 0.0, Self::COUNT_SPLIT - 1.0, 0.0).floor(),
            finite_clamp(self.count_hi, 0.0, 16_777_216.0, 0.0).floor(),
            finite_clamp(self.rate, 0.0, f32::MAX, 0.0),
            finite_clamp(self.smooth, 0.0, 1.0, 0.0),
        ];
        for (slot, value) in dst.iter_mut().zip(values) {
            *slot = value;
        }
    }

    pub fn start(&mut self, phase: f32, shape: LfoShape, seed: u32) {
        self.anchor_phase = finite_or(phase, 0.0).rem_euclid(1.0);
        self.anchor_cycles = 0.0;
        self.count_lo = 0.0;
        self.count_hi = 0.0;
        self.rate = 0.0;
        self.smooth = raw_value(self.anchor_phase, shape, seed, self.anchor_cycles);
    }

    pub fn next(&mut self, shape: LfoShape, rate_hz: f32, seed: u32, sr: f32) -> f32 {
        let rate = finite_clamp(rate_hz, 0.0, f32::MAX, 0.0);
        let sample_rate = if sr.is_finite() {
            sr.max(1.0)
        } else {
            48_000.0
        };
        if rate.to_bits() != self.rate.to_bits() {
            let position = self.position(sample_rate);
            self.anchor_cycles =
                (self.anchor_cycles as f64 + position.floor()).clamp(0.0, 16_777_216.0) as f32;
            self.anchor_phase = position.rem_euclid(1.0) as f32;
            self.count_lo = 0.0;
            self.count_hi = 0.0;
            self.rate = rate;
        }
        let position = self.position(sample_rate);
        let phase = position.rem_euclid(1.0) as f32;
        let cycles = (self.anchor_cycles as f64 + position.floor()).clamp(0.0, 16_777_216.0) as f32;
        let raw = raw_value(phase, shape, seed, cycles);
        let coeff = (-1.0 / (LFO_SMOOTH_S * sample_rate)).exp();
        self.smooth = sanitize(self.smooth + (raw - self.smooth) * (1.0 - coeff)).clamp(0.0, 1.0);
        self.count_lo += 1.0;
        if self.count_lo >= Self::COUNT_SPLIT {
            self.count_lo = 0.0;
            self.count_hi = (self.count_hi + 1.0).min(16_777_216.0);
        }
        self.smooth
    }

    pub fn cycles(&self, sr: f32) -> f32 {
        let sample_rate = if sr.is_finite() {
            sr.max(1.0)
        } else {
            48_000.0
        };
        (self.anchor_cycles as f64 + self.position(sample_rate).floor()).clamp(0.0, 16_777_216.0)
            as f32
    }

    pub fn phase(&self, sr: f32) -> f32 {
        let sample_rate = if sr.is_finite() {
            sr.max(1.0)
        } else {
            48_000.0
        };
        self.position(sample_rate).rem_euclid(1.0) as f32
    }

    fn position(&self, sr: f32) -> f64 {
        let count = self.count_hi as f64 * Self::COUNT_SPLIT as f64 + self.count_lo as f64;
        self.anchor_phase as f64 + count * self.rate as f64 / sr as f64
    }
}

pub fn cutoff_octaves(depth: f32, u: f32) -> f32 {
    WOBBLE_OCT * finite_clamp(depth, -1.0, 1.0, 0.0) * finite_clamp(u, 0.0, 1.0, 0.0)
}

fn raw_value(phase: f32, shape: LfoShape, seed: u32, cycles: f32) -> f32 {
    match shape {
        LfoShape::Sine => 0.5 - 0.5 * (core::f32::consts::TAU * phase).cos(),
        LfoShape::Tri => 1.0 - (2.0 * phase - 1.0).abs(),
        LfoShape::Saw => phase,
        LfoShape::Ramp => 1.0 - phase,
        LfoShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                0.0
            }
        }
        LfoShape::Random => random_value(seed, cycles),
    }
}

fn random_value(seed: u32, cycles: f32) -> f32 {
    let mut value = seed ^ (cycles.max(0.0) as u32).wrapping_mul(0x9E37_79B9);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    (value >> 8) as f32 / 16_777_215.0
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn finite_clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    finite_or(value, fallback).clamp(min, max)
}
