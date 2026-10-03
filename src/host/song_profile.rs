//! Bounded song overlap profile; generic Engine constructors retain their defaults.
use crate::dsp::{
    arena::StoreKind,
    caps::CapabilitySet,
    engine::EngineConfig,
    ring::{ConfigError, SongBusMemoryProfile},
};

/// Two measured generated-parts epochs plus the original legacy master.
pub const SONG_BUS_SLOTS: usize = 51;
/// The actual 73-template prelude plus two 22-template epochs and bounded headroom.
pub const SONG_TEMPLATE_SLOTS: usize = 128;

/// Selects actual constructor-owned capacities without reducing any effect budget.
///
/// # Errors
/// Unsupported fractional rate, output channels, block size, or state geometry.
pub fn song_engine_config(
    sample_rate: f32,
    max_block: usize,
    capabilities: CapabilitySet,
    store: StoreKind,
    output_channels: u8,
) -> Result<EngineConfig, ConfigError> {
    if !sample_rate.is_finite() || sample_rate.fract() != 0. {
        return Err(ConfigError::SampleRate);
    }
    let mut config = EngineConfig::new(&capabilities, sample_rate, max_block, store);
    config.bus_slots = SONG_BUS_SLOTS;
    config.template_slots = SONG_TEMPLATE_SLOTS;
    config.output_channels = output_channels;
    config.song_bus_memory = Some(SongBusMemoryProfile {
        full_slots: 6,
        small_chain_seconds: 1.,
    });
    config.validate()?;
    Ok(config)
}
