//! DSP graph and audio thread (design section 12). TASK-008.
//!
//! The module map is fixed by design 12.8.2: BE-CONTRACTS owns `graph`,
//! `controls`, `cells`, `release`, `caps` and `alloc_probe`; BE-DSP owns the
//! engine, voices, ugens, effects and buses; BE-INST owns `build`.

pub mod arena;
pub mod build;
pub mod bus;
pub mod caps;
pub mod cells;
pub mod controls;
pub mod effects;
pub mod engine;
pub mod fft;
pub mod granular;
pub mod graph;
pub mod meta;
pub mod offline;
pub mod release;
pub mod ring;
pub mod ugen;
pub mod voice;

#[cfg(test)]
pub(crate) mod alloc_probe;

#[cfg(test)]
mod tests;
