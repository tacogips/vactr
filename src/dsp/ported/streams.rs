//! Streams' six ordered firmware CV processors and Vactr audio adaptations.

use super::{CoverageState, ResourceState};

pub const STREAMS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamsFunctionSpec {
    pub position: u8,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    /// All six upstream processors emit gain/frequency control voltages.
    pub firmware_output: &'static str,
    /// Vactr-authored digital stereo audio effect, when runnable.
    pub digital_effect: Option<&'static str>,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    pub alternate: &'static str,
}

const fn row(
    position: u8,
    name: &'static str,
    path: &'static str,
    effect: Option<&'static str>,
    coverage: CoverageState,
    resources: ResourceState,
    alternate: &'static str,
) -> StreamsFunctionSpec {
    StreamsFunctionSpec {
        position,
        source_name: name,
        source_path: path,
        source_revision: STREAMS_REVISION,
        firmware_output: "gain/frequency CV to analog VCA/VCF",
        digital_effect: effect,
        coverage,
        resources,
        alternate,
    }
}

pub const STREAMS_FUNCTIONS: [StreamsFunctionSpec; 6] = [
    row(
        0,
        "Envelope",
        "streams/envelope.cc",
        Some("stream-envelope"),
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "AD / AR",
    ),
    row(
        1,
        "Vactrol",
        "streams/vactrol.cc",
        Some("stream-vactr"),
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "damped / plucked",
    ),
    row(
        2,
        "Follower",
        "streams/follower.cc",
        Some("stream-follower"),
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "normal gain+frequency / filter-only",
    ),
    row(
        3,
        "Compressor",
        "streams/compressor.cc",
        Some("stream-compressor"),
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "hard / soft knee",
    ),
    row(
        4,
        "Filter controller",
        "streams/filter_controller.h",
        Some("stream-filter"),
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "alternate ignored by source Configure",
    ),
    row(
        5,
        "Lorenz generator",
        "streams/lorenz_generator.cc",
        Some("stream-lorenz"),
        CoverageState::Adaptation,
        ResourceState::Replacement,
        "alternate ignored by source Configure",
    ),
];

#[must_use]
pub fn streams_functions() -> &'static [StreamsFunctionSpec; 6] {
    &STREAMS_FUNCTIONS
}

#[must_use]
pub fn streams_coverage_summary() -> &'static str {
    "Streams: six firmware gain/frequency CV processors; Vactr provides original stereo digital adaptations for all six control roles, including filter controller and Lorenz. Alternate/linked settings are ignored by the source's final two Configure methods and are not placebo controls. No source audio port, two-pair hardware I/O or analog hardware equivalence is claimed."
}
