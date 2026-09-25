//! Tempo: bpm and beats per cycle; cps is derived (design 11.1).

use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};

/// A tempo. `cps = bpm / 60 / beats_per_cycle`, kept exact.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Tempo {
    pub bpm: Ratio64,
    pub beats_per_cycle: Ratio64,
}

impl Default for Tempo {
    /// 120 bpm, 4 beats per cycle (design-music section 1): cps = 1/2.
    fn default() -> Self {
        Self {
            bpm: Ratio64::from_int(120),
            beats_per_cycle: Ratio64::from_int(4),
        }
    }
}

impl Tempo {
    /// A tempo with positive bpm and beats per cycle.
    ///
    /// # Errors
    /// `Type` when either value is not positive.
    pub fn new(bpm: Ratio64, beats_per_cycle: Ratio64) -> Result<Self, Failure> {
        if bpm <= Ratio64::ZERO || beats_per_cycle <= Ratio64::ZERO {
            return Err(Failure::new(
                FailCode::Type,
                "bpm and beats per cycle must be positive",
            ));
        }
        Ok(Self {
            bpm,
            beats_per_cycle,
        })
    }

    /// Cycles per second, exact.
    ///
    /// # Errors
    /// `Overflow` or `DivisionByZero` from the ratio math.
    pub fn cps(self) -> Result<Ratio64, Failure> {
        self.bpm
            .checked_div(Ratio64::from_int(60))?
            .checked_div(self.beats_per_cycle)
    }

    /// Seconds per cycle as a host float.
    ///
    /// # Errors
    /// As [`Tempo::cps`].
    pub fn cycle_seconds(self) -> Result<f64, Failure> {
        Ok(1.0 / self.cps()?.to_f64())
    }

    /// Beats for a cycle position, exact.
    ///
    /// # Errors
    /// `Overflow` from the ratio math.
    pub fn beats_at(self, cycles: Ratio64) -> Result<Ratio64, Failure> {
        cycles.checked_mul(self.beats_per_cycle)
    }
}
