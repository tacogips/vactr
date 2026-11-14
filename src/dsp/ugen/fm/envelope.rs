//! Six-operator FM envelope generator.
//!
//! Adapted from music-synthesizer-for-android (Apache-2.0), revision
//! `f67d41d313b7dc85f6fb99e79e515cc9d208cfff`: `app/src/main/jni/env.cc`,
//! `env.h`, `synth.h`, `dx7note.cc`, `exp2.h`, and `exp2.cc`. Modified:
//! translated to Rust, floating-point frequency path, no lookup tables
//! beyond the level curve.

/// Number of output frames represented by one envelope tick.
pub const EG_BLOCK: usize = 64;

/// Number of `f32` slots used by [`Eg::store`] and [`Eg::load`].
pub const EG_FLOATS: usize = 20;

/// The log-domain level corresponding to envelope level 99 and output level 99.
///
/// msfa computes `(scaleoutlevel(99) >> 1) * 64 + (scaleoutlevel(99) << 5) - 4256`,
/// then stores that value in Q16. `log_to_amp` treats this value as unity gain.
pub const FULL_SCALE_LEVEL: i32 = 3_840 << 16;

const LEVEL_CURVE: [i32; 20] = [
    0, 5, 9, 13, 17, 20, 23, 25, 27, 29, 31, 33, 35, 37, 39, 41, 42, 43, 45, 46,
];
const NOMINAL_SAMPLE_RATE: f32 = 48_000.0;
const JUMP_TARGET: i32 = 1_716 << 16;
const ATTACK_LIMIT: i32 = 17 << 24;

/// Four-stage, log-domain operator envelope.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Eg {
    rates: [i32; 4],
    levels: [i32; 4],
    outlevel: i32,
    rate_scaling: i32,
    sample_rate: f32,
    level: i32,
    target_level: i32,
    increment: i32,
    segment: i32,
    rising: i32,
    key_down: i32,
}

impl Eg {
    /// Creates an envelope at the msfa key-on starting level.
    #[must_use]
    pub fn new(
        rates: [u8; 4],
        levels: [u8; 4],
        outlevel: i32,
        rate_scaling: i32,
        sample_rate: f32,
    ) -> Self {
        let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            NOMINAL_SAMPLE_RATE
        };
        let mut eg = Self {
            rates: rates.map(|rate| i32::from(rate.min(99))),
            levels: levels.map(|level| i32::from(level.min(99))),
            outlevel: outlevel.clamp(0, 4_500),
            rate_scaling: rate_scaling.clamp(0, 63),
            sample_rate,
            level: 0,
            target_level: 0,
            increment: 0,
            segment: 0,
            rising: 0,
            key_down: 1,
        };
        eg.advance(0);
        eg
    }

    /// Advances the envelope by one 64-frame block and returns its Q24 log level.
    pub fn tick(&mut self) -> i32 {
        if self.segment < 3 || (self.segment < 4 && self.key_down == 0) {
            if self.rising != 0 {
                if self.level < JUMP_TARGET {
                    self.level = JUMP_TARGET;
                }
                let approach = (ATTACK_LIMIT.saturating_sub(self.level)) >> 24;
                let delta = i64::from(approach) * i64::from(self.increment);
                self.level = (i64::from(self.level) + delta)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                    as i32;
                if self.level >= self.target_level {
                    self.level = self.target_level;
                    self.advance(self.segment + 1);
                }
            } else {
                self.level = self.level.saturating_sub(self.increment);
                if self.level <= self.target_level {
                    self.level = self.target_level;
                    self.advance(self.segment + 1);
                }
            }
        }
        self.level
    }

    /// Starts the fourth envelope segment, matching msfa's key-up transition.
    pub fn key_off(&mut self) {
        if self.key_down != 0 {
            self.key_down = 0;
            self.advance(3);
        }
    }

    /// Returns true after key-off has reached the L4 target.
    #[must_use]
    pub fn released_and_silent(&self) -> bool {
        self.key_down == 0 && self.segment >= 4
    }

    /// Stores the envelope into exactly representable scalar `f32` slots.
    pub fn store(&self, dst: &mut [f32]) {
        if dst.len() < EG_FLOATS {
            return;
        }
        dst[..EG_FLOATS].fill(0.0);
        for (slot, value) in dst[..4].iter_mut().zip(self.rates) {
            *slot = value as f32;
        }
        for (slot, value) in dst[4..8].iter_mut().zip(self.levels) {
            *slot = value as f32;
        }
        dst[8] = self.outlevel as f32;
        dst[9] = self.rate_scaling as f32;
        dst[10] = self.sample_rate;
        store_i32(&mut dst[11..13], self.level);
        store_i32(&mut dst[13..15], self.target_level);
        store_i32(&mut dst[15..17], self.increment);
        dst[17] = self.segment as f32;
        dst[18] = self.rising as f32;
        dst[19] = self.key_down as f32;
    }

    /// Loads a value previously written by [`Eg::store`].
    #[must_use]
    pub fn load(src: &[f32]) -> Self {
        if src.len() < EG_FLOATS {
            return Self::default();
        }
        let scalar = |index: usize| exact_i32(src[index]).unwrap_or(0);
        let rates = std::array::from_fn(|index| scalar(index).clamp(0, 99));
        let levels = std::array::from_fn(|index| scalar(index + 4).clamp(0, 99));
        let sample_rate = if src[10].is_finite() && src[10] > 0.0 {
            src[10]
        } else {
            NOMINAL_SAMPLE_RATE
        };
        Self {
            rates,
            levels,
            outlevel: scalar(8).clamp(0, 4_500),
            rate_scaling: scalar(9).clamp(0, 63),
            sample_rate,
            level: load_i32(&src[11..13]),
            target_level: load_i32(&src[13..15]),
            increment: load_i32(&src[15..17]).max(0),
            segment: scalar(17).clamp(0, 4),
            rising: scalar(18).clamp(0, 1),
            key_down: scalar(19).clamp(0, 1),
        }
    }

    fn advance(&mut self, segment: i32) {
        self.segment = segment;
        if !(0..4).contains(&segment) {
            return;
        }

        let level = self.levels[segment as usize];
        let actual_level = (output_level_curve(level) >> 1) * 64 + self.outlevel - 4_256;
        let actual_level = actual_level.max(16);
        self.target_level = actual_level.saturating_mul(1 << 16);
        self.rising = if self.target_level > self.level { 1 } else { 0 };

        let qrate = ((self.rates[segment as usize] * 41) >> 6)
            .saturating_add(self.rate_scaling)
            .clamp(0, 63);
        let raw_increment = (4 + (qrate & 3)) << (8 + (qrate >> 2));
        let sample_scale = (NOMINAL_SAMPLE_RATE / self.sample_rate).clamp(0.125, 8.0);
        self.increment = ((raw_increment as f32 * sample_scale).round() as i32).max(1);
    }
}

/// Converts a DX7 envelope level to its log-domain output-level offset.
#[must_use]
fn output_level_curve(outlevel: i32) -> i32 {
    let outlevel = outlevel.clamp(0, 99);
    if outlevel >= 20 {
        28 + outlevel
    } else {
        LEVEL_CURVE[outlevel as usize]
    }
}

/// Converts the Q24 log-level difference from full scale to linear amplitude.
#[must_use]
pub fn log_to_amp(level: i32) -> f32 {
    2.0_f32.powf((level.saturating_sub(FULL_SCALE_LEVEL) as f32) / 16_777_216.0)
}

fn exact_i32(value: f32) -> Option<i32> {
    (value.is_finite()
        && value.fract() == 0.0
        && value >= i32::MIN as f32
        && value <= i32::MAX as f32)
        .then_some(value as i32)
}

fn store_i32(dst: &mut [f32], value: i32) {
    let bits = value as u32;
    dst[0] = (bits & 0xffff) as f32;
    dst[1] = (bits >> 16) as f32;
}

fn load_i32(src: &[f32]) -> i32 {
    let low = exact_i32(src[0]).unwrap_or(0).clamp(0, 0xffff) as u32;
    let high = exact_i32(src[1]).unwrap_or(0).clamp(0, 0xffff) as u32;
    ((high << 16) | low) as i32
}
