//! Allocation-free audio effect building blocks.
#![warn(missing_docs)]
use music_things_dsp::prim::{Biquad, DelayLine, Follower, Lfo, OnePole, Rng};
/// One named effect parameter.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ParamDef {
    /// Parameter name.
    pub name: &'static str,
    /// Default parameter value.
    pub default: f32,
    /// Minimum parameter value.
    pub min: f32,
    /// Maximum parameter value.
    pub max: f32,
    /// Display unit (`"dB"`, `"s"`, `"Hz"`, `""`).
    pub unit: &'static str,
}

impl ParamDef {
    /// A unitless parameter.
    #[must_use]
    pub const fn new(name: &'static str, default: f32, min: f32, max: f32) -> Self {
        Self {
            name,
            default,
            min,
            max,
            unit: "",
        }
    }

    /// A parameter with a display unit.
    #[must_use]
    pub const fn unit(
        name: &'static str,
        default: f32,
        min: f32,
        max: f32,
        unit: &'static str,
    ) -> Self {
        Self {
            name,
            default,
            min,
            max,
            unit,
        }
    }

    /// `v` clamped into the parameter range (NaN -> default).
    #[must_use]
    pub fn clamp(&self, v: f32) -> f32 {
        if v.is_nan() {
            self.default
        } else {
            v.clamp(self.min, self.max)
        }
    }
}

/// The scalar state of one unit: a fixed bank of primitives. Kinds use the
/// entries they need by index.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct FxState {
    /// Biquad filter states.
    pub bq: [Biquad; 16],
    /// One-pole filter states.
    pub op: [OnePole; 8],
    /// Envelope follower states.
    pub env: [Follower; 4],
    /// Low-frequency oscillator states.
    pub lfo: [Lfo; 4],
    /// Delay line states.
    pub dl: [DelayLine; 8],
    /// Scalar state slots.
    pub s: [f32; 32],
    /// Random generator state.
    pub rng: Rng,
}

/// Per-block counters effects report (granular and analyzers, 12.6).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FxStats {
    /// Grain spawns skipped (window violation or pool exhaustion).
    pub grains_skipped: u32,
    /// Controls clamped to a tier cap (density, size, capture depth).
    pub clamped: u32,
    /// Grains spawned.
    pub grains_spawned: u32,
    /// Grains short-gated at an unfreeze.
    pub grains_gated: u32,
    /// Live-buffer reads outside the written, not-yet-overwritten window
    /// (must stay 0; the no-stale-read proof of 12.6).
    pub stale_reads: u32,
}

mod ctx;
mod kind;
mod param_set;
pub use ctx::{FxContext, FxCtx, GrainLimits};
pub use kind::{AnalyzerKind, EffectKind};
pub use param_set::{ParamSet, MAX_PARAMS};

/// Dynamics effects.
pub mod dynamics;

/// Eq effects.
pub mod eq;

/// Music filters effects.
pub mod music_filters;

/// Musicdsp effects.
pub mod musicdsp;

/// Resonator effects.
pub mod resonator;

/// Resonator extra effects.
pub mod resonator_extra;

/// Restoration effects.
pub mod restoration;

/// Utility effects.
pub mod utility;

/// Saturation effects.
pub mod saturation;

/// Delay effects.
pub mod delay;

/// Reverb effects.
pub mod reverb;

/// Modulation effects.
pub mod modulation;

/// Spatial effects.
pub mod spatial;

/// Lofi effects.
pub mod lofi;

/// Composite lofi effects.
pub mod composite_lofi;

/// Creative reverb effects.
pub mod creative_reverb;

/// Creative delay effects.
pub mod creative_delay;
