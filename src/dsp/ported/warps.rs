//! Warps' published `modulation_algorithm` range, in the pinned
//! `warps/dsp/modulator.{h,cc}` `XmodAlgorithm` order, plus the hidden
//! frequency-shifter Easter egg.
//!
//! See `THIRD_PARTY_NOTICES.md`'s "Warps modulation design reference"
//! section for the full per-equation provenance this inventory summarizes.
//! `dual-mod` translates the crossfade, analog/digital ring modulation,
//! XOR and comparator equations and the 96 kHz vocoder topology; the fold
//! equation "remains authored" per that notice, so it stays an
//! architectural adaptation rather than a translated equation. No row is a
//! validated source-equivalent port.

use super::{CoverageState, ResourceState};

pub const WARPS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

/// One position in the source `modulation_algorithm` 0..1 range and its
/// Vactr dual-input modulation effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarpsAlgorithmSpec {
    pub position: u8,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub vactr_effect: &'static str,
    pub coverage: CoverageState,
    pub resources: ResourceState,
}

const fn row(
    position: u8,
    source_name: &'static str,
    source_path: &'static str,
    vactr_effect: &'static str,
    coverage: CoverageState,
    resources: ResourceState,
) -> WarpsAlgorithmSpec {
    WarpsAlgorithmSpec {
        position,
        source_name,
        source_path,
        source_revision: WARPS_REVISION,
        vactr_effect,
        coverage,
        resources,
    }
}

/// `dual-mod`'s XMOD-and-vocoder path (positions 0..6) shares the imported,
/// individually MIT-noticed `warps/resources.cc` `fb_*` 20-band filter rows
/// and `warps/dsp/sample_rate_conversion_filters.h` FIR coefficient halves
/// that carry every algorithm across the host-rate boundary into the
/// translated 96 kHz filter bank. None of the four `ResourceState`
/// variants names "cleared, individually translated filter coefficient
/// data, no oscillator table" directly; `Replacement` ("Vactr uses
/// cleared, original or analytic replacements") is the closest declared
/// fit, so it is used here for positions 0..6. `shift-pair` (position 7)
/// does not share this pipeline: its own 127-tap Hilbert FIR is authored
/// and imports no source lookup, quadrature pole or oscillator waveform,
/// so it is `ResourceState::None`.
const XMOD_RESOURCES: ResourceState = ResourceState::Replacement;

pub const WARPS_ALGORITHMS: [WarpsAlgorithmSpec; 8] = [
    row(
        0,
        "XMOD_ALGORITHM_XFADE",
        "warps/dsp/modulator.h",
        "dual-mod",
        CoverageState::SourceStage,
        XMOD_RESOURCES,
    ),
    // "The fold equation remains authored": unlike its five XMOD siblings,
    // fold is not a translated source equation, so it stays Adaptation.
    row(
        1,
        "XMOD_ALGORITHM_FOLD",
        "warps/dsp/modulator.h",
        "dual-mod",
        CoverageState::Adaptation,
        XMOD_RESOURCES,
    ),
    row(
        2,
        "XMOD_ALGORITHM_ANALOG_RING_MODULATION",
        "warps/dsp/modulator.h",
        "dual-mod",
        CoverageState::SourceStage,
        XMOD_RESOURCES,
    ),
    row(
        3,
        "XMOD_ALGORITHM_DIGITAL_RING_MODULATION",
        "warps/dsp/modulator.h",
        "dual-mod",
        CoverageState::SourceStage,
        XMOD_RESOURCES,
    ),
    row(
        4,
        "XMOD_ALGORITHM_XOR",
        "warps/dsp/modulator.h",
        "dual-mod",
        CoverageState::SourceStage,
        XMOD_RESOURCES,
    ),
    row(
        5,
        "XMOD_ALGORITHM_COMPARATOR",
        "warps/dsp/modulator.h",
        "dual-mod",
        CoverageState::SourceStage,
        XMOD_RESOURCES,
    ),
    // Continuous algorithm region above ~0.7, not a named XmodAlgorithm
    // enumerator: translates the pinned 20-band analysis/synthesis
    // topology, but source-rate parity is not claimed, so this stays
    // SourceStage rather than SourcePort.
    row(
        6,
        "VOCODER",
        "warps/dsp/vocoder.cc",
        "dual-mod",
        CoverageState::SourceStage,
        XMOD_RESOURCES,
    ),
    // Hidden Easter egg, not part of the public modulation_algorithm range.
    row(
        7,
        "EASTER_EGG",
        "warps/dsp/modulator.cc",
        "shift-pair",
        CoverageState::Adaptation,
        ResourceState::None,
    ),
];

#[must_use]
pub fn warps_algorithms() -> &'static [WarpsAlgorithmSpec; 8] {
    &WARPS_ALGORITHMS
}

/// Concise, truthful human-readable status. No position is reported as a
/// source port.
#[must_use]
pub fn warps_coverage_summary() -> String {
    "Warps: 8 published `modulation_algorithm` positions (6 XMOD equations, \
     the 96 kHz vocoder, and the hidden Easter-egg frequency shifter); 2 \
     adaptations (fold, shift-pair), 6 source-stage translations \
     (crossfade, analog/digital ring modulation, XOR, comparator, \
     vocoder), 0 source ports. `dual-mod` imports only the pinned `fb_*` \
     filter rows and sample-rate-conversion FIR coefficient halves; \
     `shift-pair` imports no source table or oscillator waveform."
        .to_string()
}
