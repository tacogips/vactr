use super::FxStats;
use music_things_dsp::{fft::Fft, SampleSource};
/// Granular resource limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrainLimits {
    /// Maximum grain density per second.
    pub max_grain_density: f32,
    /// Maximum grain size in seconds.
    pub max_grain_size: f32,
    /// Maximum capture duration in seconds.
    pub max_capture_seconds: f32,
}
/// Block context for effect processing.
pub struct FxCtx<'a> {
    /// Sample rate in hertz.
    pub sr: f32,
    /// Sample resources.
    pub samples: &'a dyn SampleSource,
    /// FFT tables.
    pub fft: &'a Fft,
    /// Granular resource limits.
    pub limits: GrainLimits,
    /// Preallocated scratch.
    pub scratch: &'a mut [f32],
    /// Analysis output cells.
    pub analysis: &'a mut [f32],
    /// Per-block statistics.
    pub stats: &'a mut FxStats,
}
/// Supplies an effect context.
pub trait FxContext {
    /// Reborrows the context for processing.
    fn fx_ctx(&mut self) -> FxCtx<'_>;
}
impl FxContext for FxCtx<'_> {
    fn fx_ctx(&mut self) -> FxCtx<'_> {
        FxCtx {
            sr: self.sr,
            samples: self.samples,
            fft: self.fft,
            limits: self.limits,
            scratch: &mut *self.scratch,
            analysis: &mut *self.analysis,
            stats: &mut *self.stats,
        }
    }
}
