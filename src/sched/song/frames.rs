//! Absolute rational musical positions on the observed host timebase.
use crate::song::routing::SongEndpoints;
use crate::song::{SnapshotEpoch, SongLimits, SongSettings};
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};

pub(super) struct FrameMap {
    activation: u64,
    sample_rate: u32,
    seconds_per_cycle: Ratio64,
    tail: Ratio64,
    duration: Ratio64,
    limits: SongLimits,
}
impl FrameMap {
    pub(super) fn new(
        settings: SongSettings,
        duration: Ratio64,
        activation: u64,
        sample_rate: u32,
        limits: SongLimits,
    ) -> Result<Self, Failure> {
        settings.validate()?;
        limits.validate()?;
        if duration < Ratio64::ZERO || sample_rate == 0 {
            return Err(Failure::new(
                FailCode::Type,
                "invalid finite song frame map",
            ));
        }
        let seconds_per_cycle = settings.seconds_per_cycle()?;
        let map = Self {
            activation,
            sample_rate,
            seconds_per_cycle,
            tail: settings.tail_seconds,
            duration,
            limits,
        };
        map.deadline(duration)?;
        Ok(map)
    }
    pub(super) fn at(&self, cycle: Ratio64) -> Result<u64, Failure> {
        let seconds = cycle.checked_mul(self.seconds_per_cycle)?;
        self.absolute(seconds)
    }
    pub(super) fn deadline(&self, cycle: Ratio64) -> Result<u64, Failure> {
        let seconds = cycle
            .checked_mul(self.seconds_per_cycle)?
            .checked_add(self.tail)?;
        self.absolute(seconds)
    }
    fn absolute(&self, seconds: Ratio64) -> Result<u64, Failure> {
        self.activation
            .checked_add(self.limits.frames_at(seconds, self.sample_rate)?)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "song absolute host frame overflow"))
    }
    pub(super) fn endpoints(&self, epoch: SnapshotEpoch) -> Result<SongEndpoints, Failure> {
        Ok(SongEndpoints {
            epoch,
            arrangement: self.at(self.duration)?,
            tail_deadline: self.deadline(self.duration)?,
        })
    }
}
