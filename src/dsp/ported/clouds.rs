//! Clouds' four published `PlaybackMode` texture modes, in
//! `clouds/dsp/granular_processor.h` registration order.
//!
//! Every mode is a runnable Vactr architectural adaptation; none is a
//! validated source-equivalent port. See `THIRD_PARTY_NOTICES.md`'s four
//! "Clouds ... texture architectural adaptation" sections for the per-mode
//! provenance this inventory summarizes.

use super::{CoverageState, ResourceState};

pub const CLOUDS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

/// One `PlaybackMode` entry and its Vactr stereo texture effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloudsModeSpec {
    pub mode: u8,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub vactr_effect: &'static str,
    pub coverage: CoverageState,
    pub resources: ResourceState,
}

const fn row(
    mode: u8,
    source_name: &'static str,
    source_path: &'static str,
    vactr_effect: &'static str,
) -> CloudsModeSpec {
    CloudsModeSpec {
        mode,
        source_name,
        source_path,
        source_revision: CLOUDS_REVISION,
        vactr_effect,
        // Every mode is a runnable architectural adaptation; none of the
        // four "Clouds ... texture architectural adaptation" notices claims
        // a source-stage translation or source-equivalent port.
        coverage: CoverageState::Adaptation,
        // No generated grain-size, sine, window, crossfade,
        // sample-rate-conversion, spectral or wave table is imported for
        // any mode; every notice states this explicitly.
        resources: ResourceState::Replacement,
    }
}

pub const CLOUDS_MODES: [CloudsModeSpec; 4] = [
    row(
        0,
        "PLAYBACK_MODE_GRANULAR",
        "clouds/dsp/granular_sample_player.h",
        "texture-grain",
    ),
    row(
        1,
        "PLAYBACK_MODE_STRETCH",
        "clouds/dsp/wsola_sample_player.h",
        "texture-stretch",
    ),
    row(
        2,
        "PLAYBACK_MODE_LOOPING_DELAY",
        "clouds/dsp/looping_sample_player.h",
        "texture-loop",
    ),
    row(
        3,
        "PLAYBACK_MODE_SPECTRAL",
        "clouds/dsp/pvoc/phase_vocoder.cc",
        "texture-spectral",
    ),
];

#[must_use]
pub fn clouds_modes() -> &'static [CloudsModeSpec; 4] {
    &CLOUDS_MODES
}

/// Concise, truthful human-readable status. No mode is reported as a
/// source port; the shared `quality` mono/low-fidelity selector is a
/// separate adaptation documented in `THIRD_PARTY_NOTICES.md` and is not
/// itself a fifth mode.
#[must_use]
pub fn clouds_coverage_summary() -> String {
    "Clouds: 4/4 published playback modes (granular, stretch, looping-delay, \
     spectral) are runnable Vactr architectural adaptations; 0 source-stage \
     translations, 0 source ports. No generated grain, window, spectral or \
     sample-rate-conversion table, and no recorded or external audio asset, \
     is imported by any mode."
        .to_string()
}
