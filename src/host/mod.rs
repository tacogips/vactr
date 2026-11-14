//! Capability hosts (design 11.5, 12.7, 12.8.3, 12.8.5, 12.8.10).
//!
//! The core reaches audio, MIDI, OSC, render and sample I/O only through the
//! traits in `caps`. `wire` holds the POD records that cross to the audio
//! side and their byte codec, `noop` the host that does nothing (the dry-run
//! target), and `testing` the recording hosts and mock transports. The
//! native host (cpal, midir) and the wasm ABI are compiled only for their
//! targets, so both wasm32 builds stay free of native crates.

pub mod caps;
pub mod noop;
pub mod song_profile;
pub mod wire;

#[cfg(test)]
pub(crate) mod testing;

#[cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
pub mod native;

#[cfg(all(target_arch = "wasm32", feature = "host-wasm"))]
pub mod wasm;

#[cfg(test)]
mod tests;
