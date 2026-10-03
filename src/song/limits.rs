//! Construction bounds and exact absolute-position host frame conversion.
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};

/// Bounded symbolic construction and canonical realization policy.
/// Host preparation must supply its actual track/cache/frame capacities;
/// these values do not claim voice count equals available route branches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongLimits {
    pub max_nodes: u32,
    pub max_depth: u32,
    pub max_tracks: u32,
    pub max_cached_events: u32,
    pub max_frames: u64,
}
impl Default for SongLimits {
    fn default() -> Self {
        Self {
            max_nodes: 16_384,
            max_depth: 256,
            max_tracks: 64,
            max_cached_events: 100_000,
            max_frames: (u64::from(u32::MAX) - 36) / 4,
        }
    }
}
impl SongLimits {
    /// Derives admission policy from supplied actual host allocations.
    /// # Errors
    /// Invalid construction policy is rejected; zero musical capacities
    /// deliberately permit only empty tracks/events/zero-frame arrangements.
    pub fn for_capacities(
        max_tracks: u32,
        max_cached_events: u32,
        max_frames: u64,
    ) -> Result<Self, Failure> {
        let limits = Self {
            max_tracks,
            max_cached_events,
            max_frames,
            ..Self::default()
        };
        limits.validate()?;
        Ok(limits)
    }
    /// Validates structural limits against existing depth/work ceilings.
    pub fn validate(&self) -> Result<(), Failure> {
        if self.max_nodes == 0
            || self.max_nodes > 1_000_000
            || self.max_depth == 0
            || self.max_depth > 256
        {
            return Err(Failure::new(
                FailCode::Type,
                "invalid song construction node/depth limits",
            ));
        }
        Ok(())
    }
    pub(crate) fn structure(&self, nodes: u32, depth: u32, tracks: usize) -> Result<(), Failure> {
        self.validate()?;
        if nodes > self.max_nodes {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "song symbolic node limit exceeded",
            ));
        }
        if depth > self.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "song construction depth exceeded",
            ));
        }
        if tracks > self.max_tracks as usize {
            return Err(Failure::new(
                FailCode::BeyondCapability,
                "song track capacity exceeded",
            ));
        }
        Ok(())
    }
    /// Checks canonical output/cache entries without truncating music.
    pub fn check_events(&self, count: usize) -> Result<(), Failure> {
        self.validate()?;
        if count > self.max_cached_events as usize {
            return Err(Failure::new(
                FailCode::BeyondCapability,
                "song canonical event capacity exceeded",
            ));
        }
        Ok(())
    }
    /// Converts an absolute nonnegative second position once, nearest/ties-up.
    /// No accumulating rounded child durations and no floating point math.
    pub fn frames_at(&self, seconds: Ratio64, sample_rate: u32) -> Result<u64, Failure> {
        self.validate()?;
        if seconds < Ratio64::ZERO || sample_rate == 0 {
            return Err(Failure::new(
                FailCode::Type,
                "frame conversion needs nonnegative time and positive rate",
            ));
        }
        let n = u128::try_from(seconds.num())
            .map_err(|_| Failure::new(FailCode::Overflow, "negative frame numerator"))?;
        let d = u128::try_from(seconds.den())
            .map_err(|_| Failure::new(FailCode::Overflow, "frame denominator overflow"))?;
        let scaled = n * u128::from(sample_rate);
        let rounded = scaled / d + u128::from((scaled % d) * 2 >= d);
        let frames = u64::try_from(rounded)
            .map_err(|_| Failure::new(FailCode::Overflow, "song frame count overflow"))?;
        if frames > self.max_frames {
            return Err(Failure::new(
                FailCode::BeyondCapability,
                "song frame capacity exceeded",
            ));
        }
        Ok(frames)
    }
}
/// Absolute arrangement endpoint and absolute tail deadline; tail frame count
/// is their difference, not a separately rounded and accumulated duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameEndpoints {
    arrangement: u64,
    deadline: u64,
}
impl FrameEndpoints {
    /// Constructs ordered nonnegative absolute endpoints.
    pub fn new(arrangement: u64, deadline: u64) -> Result<Self, Failure> {
        if deadline < arrangement {
            return Err(Failure::new(
                FailCode::Type,
                "tail deadline is before the arrangement endpoint",
            ));
        }
        Ok(Self {
            arrangement,
            deadline,
        })
    }
    /// Absolute arrangement end frame.
    #[must_use]
    pub const fn arrangement(self) -> u64 {
        self.arrangement
    }
    /// Absolute end-plus-tail deadline frame.
    #[must_use]
    pub const fn deadline(self) -> u64 {
        self.deadline
    }

    /// Number of frames after the arrangement endpoint.
    #[must_use]
    pub const fn tail_frames(self) -> u64 {
        self.deadline - self.arrangement
    }
}
