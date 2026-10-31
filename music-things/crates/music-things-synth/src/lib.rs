//! Allocation-free synthesis building blocks.
#![warn(missing_docs)]
use music_things_dsp::prim::{Biquad, DelayLine};
/// Maximum kernel input count.
pub const MAX_PORTS: usize = 48;
/// Per-node scalar state inside a voice.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct NodeState {
    /// Scalar state slots.
    pub s: [f32; 8],
    /// Biquad filter states.
    pub bq: [Biquad; 2],
    /// Delay line state.
    pub dl: DelayLine,
    /// Integer state slots.
    pub u: [u32; 4],
}

impl NodeState {
    /// Marks the node finished (an envelope or a one-shot player).
    pub fn finish(&mut self) {
        self.u[3] = 1;
    }

    /// True once `finish` ran.
    #[must_use]
    pub fn done(&self) -> bool {
        self.u[3] == 1
    }
}

/// One kernel input: an audio-rate buffer or a block-constant value.
#[derive(Clone, Copy, Debug)]
pub enum Inp<'a> {
    /// Audio-rate values.
    Buf(&'a [f32]),
    /// Block-constant value.
    Val(f32),
}

impl Inp<'_> {
    /// The value at frame `i` (the last frame past the end).
    #[inline]
    #[must_use]
    pub fn at(&self, i: usize) -> f32 {
        match self {
            Inp::Val(v) => *v,
            Inp::Buf(b) => b.get(i).or_else(|| b.last()).copied().unwrap_or(0.0),
        }
    }

    /// The block's first value (control-rate reads).
    #[inline]
    #[must_use]
    pub fn first(&self) -> f32 {
        self.at(0)
    }
}

/// Block-level context for synthesis kernels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KernelCtx {
    /// Sample rate in hertz.
    pub sr: f32,
    /// Frames of this block during which the gate is open (0 = closed).
    pub gate: usize,
    /// Per-voice noise seed.
    pub seed: u32,
}
/// Supplies a kernel context.
pub trait KernelContext {
    /// Returns the context for the current block.
    fn kernel_ctx(&self) -> KernelCtx;
}
impl KernelContext for KernelCtx {
    fn kernel_ctx(&self) -> KernelCtx {
        *self
    }
}
mod ports;
pub use ports::PortValues;

/// Oscillator kernels.
pub mod osc;

/// Filter and delay kernels.
pub mod filter;

/// Envelope kernels.
pub mod env;

/// Additive synthesis kernel.
pub mod additive;

/// Phase pair kernel.
pub mod phase_pair;

/// analog percussion kernel.
pub mod analog_percussion;

/// feedback metal kernel.
pub mod feedback_metal;

/// fm drum kernel.
pub mod fm_drum;

/// fusion drum kernel.
pub mod fusion_drum;

/// rings part kernel.
pub mod rings_part;

/// string choir kernel.
pub mod string_choir;

/// tidal function kernel.
pub mod tidal_function;

/// tidal poly kernel.
pub mod tidal_poly;

/// peak function kernel.
pub mod peak_function;

/// peak pulse kernel.
pub mod peak_pulse;

/// stage segment kernel.
pub mod stage_segment;

/// Polyphonic LFO kernel.
pub mod frame_lfo;

/// Linked stage kernel.
pub mod stage_linked;

/// Keyframe interpolation kernel.
pub mod frame_keyframe;

/// Ominous Elements lookup helpers.
pub mod elements_ominous;

/// Elements internal kernel.
pub mod elements_internal;

/// MusicDSP synthesis kernels.
pub mod musicdsp_synth;

/// FM synthesis kernels.
pub mod fm;

/// va filter kernels.
pub mod va_filter;

/// stage chain kernels.
pub mod stage_chain;

/// Sample playback kernels.
pub mod sample;

/// Wavetable playback kernels.
pub mod wavetable;
