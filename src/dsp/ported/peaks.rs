//! Peaks' twelve registered functions, in `processors.h` order.
//!
//! Runnable entries are Vactrol adaptations or source-stage translations,
//! never validated source ports. The original
//! Number Station digit binary and generated waveform tables are excluded.

use super::{CoverageState, ResourceState};

pub const PEAKS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeaksRole {
    Audio,
    Control,
    Excluded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeaksFunctionSpec {
    pub position: u8,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub vactrol_template: Option<&'static str>,
    pub role: PeaksRole,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    pub alternate: &'static str,
    /// The upstream position references unaudited digit recordings; Vactrol
    /// does not import them, including when an original replacement runs.
    pub upstream_digit_asset: bool,
}

#[allow(clippy::too_many_arguments)] // Literal registry rows have eight auditable fields.
const fn row(
    position: u8,
    source_name: &'static str,
    source_path: &'static str,
    vactrol_template: Option<&'static str>,
    role: PeaksRole,
    coverage: CoverageState,
    resources: ResourceState,
    alternate: &'static str,
) -> PeaksFunctionSpec {
    PeaksFunctionSpec {
        position,
        source_name,
        source_path,
        source_revision: PEAKS_REVISION,
        vactrol_template,
        role,
        coverage,
        resources,
        alternate,
        upstream_digit_asset: position == 11,
    }
}

pub const PEAKS_FUNCTIONS: [PeaksFunctionSpec; 12] = [
    row(
        0,
        "Envelope",
        "peaks/modulations/multistage_envelope.cc",
        Some("peak-motion-voice"),
        PeaksRole::Control,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "AD in half mode; ADSR in full mode",
    ),
    row(
        1,
        "LFO",
        "peaks/modulations/lfo.cc",
        Some("peak-motion-voice"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "seven half-mode presets; full mode shape/parameter/reset",
    ),
    row(
        2,
        "Tap LFO",
        "peaks/modulations/lfo.cc",
        Some("peak-motion-voice"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "half mode shape/parameter; full mode level/shape/parameter/reset",
    ),
    row(
        3,
        "Bass drum",
        "peaks/drums/bass_drum.cc",
        Some("low-drum"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "half/full source controls not yet audited for parity",
    ),
    row(
        4,
        "Snare drum",
        "peaks/drums/snare_drum.cc",
        Some("wire-drum"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "half/full source controls not yet audited for parity",
    ),
    row(
        5,
        "High hat",
        "peaks/drums/high_hat.cc",
        Some("metal-hat"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "source alternate parity pending",
    ),
    row(
        6,
        "FM drum",
        "peaks/drums/fm_drum.cc",
        Some("phase-drum"),
        PeaksRole::Audio,
        CoverageState::SourceStage,
        ResourceState::Replacement,
        "single-phase source stages; source alternate presets and numeric parity pending",
    ),
    row(
        7,
        "Pulse shaper",
        "peaks/pulse_processor/pulse_shaper.cc",
        Some("peak-pulse-voice"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "full delay/duration/interval/repeats; half duration/repeats with derived interval",
    ),
    row(
        8,
        "Pulse randomizer",
        "peaks/pulse_processor/pulse_randomizer.cc",
        Some("peak-pulse-voice"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "full acceptance/repetition/delay/randomness; half forces acceptance and zero randomness",
    ),
    row(
        9,
        "Bouncing ball",
        "peaks/modulations/bouncing_ball.h",
        Some("peak-pulse-voice"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "full gravity/loss/height/velocity; half fixes height and velocity",
    ),
    row(
        10,
        "Mini sequencer",
        "peaks/modulations/mini_sequencer.h",
        None,
        PeaksRole::Excluded,
        CoverageState::Pending,
        ResourceState::None,
        "sequencer-only; audio scope excludes it",
    ),
    row(
        11,
        "Number Station",
        "peaks/number_station/number_station.cc",
        Some("number-station-voice"),
        PeaksRole::Audio,
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "tone and authored ten-digit voice roles; half fixes noise and distortion",
    ),
];

#[must_use]
pub fn peaks_functions() -> &'static [PeaksFunctionSpec; 12] {
    &PEAKS_FUNCTIONS
}

#[must_use]
pub fn peaks_coverage_summary() -> &'static str {
    "Peaks: 12 registered functions; 7 control/audio-rate function adaptations, 3 drum adaptations and 1 FM drum source-stage translation runnable, mini sequencer excluded from audio scope. Number Station runs an original ten-syllable replacement; upstream digits.bin remains unaudited and unused. No source-port parity claimed."
}
