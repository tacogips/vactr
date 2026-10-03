//! Frozen song timing/seed settings and exact arrangement/tail endpoints.
use super::{FrameEndpoints, Part, SongLimits};
use crate::clock::tempo::Tempo;
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};
use std::rc::Rc;

/// Construction settings. A Song validates and takes a private copy; changing
/// a caller's settings afterwards never mutates that Song's timing authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongSettings {
    pub bpm: Ratio64,
    pub cycle_beats: Ratio64,
    pub meter: [u32; 2],
    pub seed: u64,
    pub tail_seconds: Ratio64,
}
impl Default for SongSettings {
    fn default() -> Self {
        Self {
            bpm: Ratio64::from_int(120),
            cycle_beats: Ratio64::from_int(4),
            meter: [4, 4],
            seed: 0,
            tail_seconds: Ratio64::from_int(8),
        }
    }
}
impl SongSettings {
    /// Validates constant positive tempo, meter and nonnegative finite tail.
    pub fn validate(&self) -> Result<(), Failure> {
        Tempo::new(self.bpm, self.cycle_beats)?;
        if self.meter[0] == 0 || !self.meter[1].is_power_of_two() {
            return Err(Failure::new(
                FailCode::Type,
                "song meter needs a positive numerator and power-of-two denominator",
            ));
        }
        if self.tail_seconds < Ratio64::ZERO {
            return Err(Failure::new(
                FailCode::Type,
                "song tail must be nonnegative",
            ));
        }
        self.seconds_per_cycle()?;
        Ok(())
    }
    /// Frozen existing Tempo contract.
    pub fn tempo(&self) -> Result<Tempo, Failure> {
        Tempo::new(self.bpm, self.cycle_beats)
    }
    /// Exact seconds per cycle, before conversion to host frames.
    pub fn seconds_per_cycle(&self) -> Result<Ratio64, Failure> {
        Ratio64::ONE.checked_div(self.tempo()?.cps()?)
    }
}
/// An immutable finite Song, authoritative for both arrangement and tail.
#[derive(Clone, Debug)]
pub struct Song {
    part: Rc<Part>,
    settings: SongSettings,
    arrangement_seconds: Ratio64,
    deadline_seconds: Ratio64,
}
impl Song {
    /// Freezes validated timing without assuming a host sample rate.
    /// # Errors
    /// Invalid tempo/meter/tail, construction limits or exact time overflow.
    pub fn new(part: Rc<Part>, settings: SongSettings) -> Result<Self, Failure> {
        Self::with_limits(part, settings, &SongLimits::default())
    }
    /// Freezes using an explicitly supplied host construction policy.
    pub fn with_limits(
        part: Rc<Part>,
        settings: SongSettings,
        limits: &SongLimits,
    ) -> Result<Self, Failure> {
        settings.validate()?;
        part.validate_limits(limits)?;
        let arrangement_seconds = part.duration().checked_mul(settings.seconds_per_cycle()?)?;
        let deadline_seconds = arrangement_seconds.checked_add(settings.tail_seconds)?;
        Ok(Self {
            part,
            settings,
            arrangement_seconds,
            deadline_seconds,
        })
    }
    /// Immutable finite arrangement, shared without expanding music.
    #[must_use]
    pub fn part(&self) -> &Rc<Part> {
        &self.part
    }
    /// Frozen timing/seed settings.
    #[must_use]
    pub const fn settings(&self) -> &SongSettings {
        &self.settings
    }
    /// Exact arrangement duration in cycles.
    #[must_use]
    pub fn duration(&self) -> Ratio64 {
        self.part.duration()
    }
    /// Arrangement endpoint in exact seconds, independent of tail.
    #[must_use]
    pub const fn arrangement_seconds(&self) -> Ratio64 {
        self.arrangement_seconds
    }
    /// Absolute end plus tail deadline, in exact seconds.
    #[must_use]
    pub const fn deadline_seconds(&self) -> Ratio64 {
        self.deadline_seconds
    }
    /// Preflights absolute endpoints against actual rate/frame capacity.
    /// Rejects overflow/capacity instead of clamping or accumulating rounding.
    pub fn frame_endpoints(
        &self,
        sample_rate: u32,
        limits: &SongLimits,
    ) -> Result<FrameEndpoints, Failure> {
        self.part.validate_limits(limits)?;
        FrameEndpoints::new(
            limits.frames_at(self.arrangement_seconds, sample_rate)?,
            limits.frames_at(self.deadline_seconds, sample_rate)?,
        )
    }
}
