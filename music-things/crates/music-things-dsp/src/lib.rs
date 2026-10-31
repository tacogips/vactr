//! Shared allocation-free audio processing primitives.
#![warn(missing_docs)]
pub mod fft;
pub mod prim;
mod sample_view;
pub use sample_view::{NoSamples, SampleSource, SampleView};
