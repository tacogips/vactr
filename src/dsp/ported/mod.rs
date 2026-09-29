//! Checked coverage inventory for published Mutable Instruments audio DSP.
//!
//! Inventories are per published module and do not imply unlisted algorithm
//! coverage or source-equivalent audio ports.

mod braids;
mod clouds;
mod elements;
mod frames;
mod functions;
mod manifest;
mod peaks;
mod rings;
mod stages;
mod streams;
mod warps;

pub use clouds::{
    clouds_coverage_summary, clouds_modes, CloudsModeSpec, CLOUDS_MODES, CLOUDS_REVISION,
};
pub use warps::{
    warps_algorithms, warps_coverage_summary, WarpsAlgorithmSpec, WARPS_ALGORITHMS, WARPS_REVISION,
};

pub use frames::{
    frames_coverage_summary, frames_paths, FramesCoverage, FramesPathSpec, EASING_OPTIONS,
    FRAMES_PATHS, FRAMES_REVISION, KEYFRAME_CAPACITY,
};
pub use peaks::{
    peaks_coverage_summary, peaks_functions, PeaksFunctionSpec, PeaksRole, PEAKS_FUNCTIONS,
    PEAKS_REVISION,
};
pub use rings::{
    resonator_models, rings_coverage_summary, string_synth_path, ResonatorSpec, StringSynthSpec,
    RINGS_REVISION,
};
pub use stages::{
    stages_cells, stages_coverage_summary, StageCoverage, StageFunctionSpec, STAGES_CELLS,
    STAGES_OTHER, STAGES_REVISION,
};
pub use streams::{
    streams_coverage_summary, streams_functions, StreamsFunctionSpec, STREAMS_FUNCTIONS,
    STREAMS_REVISION,
};

pub use braids::{braids_coverage_summary, braids_shapes, BraidsShapeSpec, BRAIDS_REVISION};
pub use elements::{
    elements_coverage_summary, resonator_modes, ElementsSpec, ALTERNATE_VOICE, ELEMENTS_REVISION,
};
pub use functions::{
    functions, functions_coverage_summary, FunctionSpec, TIDES1, TIDES2, TIDES_REVISION,
};

pub use manifest::{
    plaits_algorithms, plaits_coverage_summary, plaits_voice, CommonControls, CoverageState,
    Enveloped, PortedAlgorithm, ResourceFlags, ResourceState, VoiceRegistration, PLAITS_REVISION,
};

#[cfg(test)]
mod tests;
