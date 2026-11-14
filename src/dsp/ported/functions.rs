//! Coverage for the two published Tides function-generator generations.

use super::{CoverageState, ResourceState};

pub const TIDES_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionSpec {
    pub generation: u8,
    pub mode: u8,
    pub range: u8,
    pub output_mode: u8,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub vactr_template: Option<&'static str>,
    /// Explicitly loaded four-output template; unavailable on stereo hosts.
    pub opt_in_quad_template: Option<&'static str>,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    /// Published audio output lanes versus simultaneously routed Vactr lanes.
    pub source_channels: u8,
    pub simultaneous_channels: u8,
    /// Simultaneous roles are adapted; source timing/numerical parity is open.
    pub simultaneous_output_coverage: CoverageState,
}

const fn first_generation() -> [FunctionSpec; 9] {
    let row = FunctionSpec {
        generation: 1,
        mode: 0,
        range: 0,
        output_mode: 0,
        source_path: "tides/generator.cc",
        source_revision: TIDES_REVISION,
        vactr_template: Some("tidal-voice"),
        opt_in_quad_template: Some("tidal-quad-voice"),
        coverage: CoverageState::Adaptation,
        resources: ResourceState::Replacement,
        source_channels: 2,
        simultaneous_channels: 2,
        simultaneous_output_coverage: CoverageState::Adaptation,
    };
    let mut rows = [row; 9];
    let mut index = 0;
    while index < 9 {
        rows[index].mode = (index / 3) as u8;
        rows[index].range = (index % 3) as u8;
        index += 1;
    }
    rows
}

const fn second_generation() -> [FunctionSpec; 24] {
    let row = FunctionSpec {
        generation: 2,
        mode: 0,
        range: 0,
        output_mode: 0,
        source_path: "tides2/ramp_generator.h",
        source_revision: TIDES_REVISION,
        vactr_template: Some("tidal-poly-voice"),
        opt_in_quad_template: Some("tidal-poly-quad-voice"),
        coverage: CoverageState::Adaptation,
        resources: ResourceState::Replacement,
        source_channels: 4,
        simultaneous_channels: 2,
        simultaneous_output_coverage: CoverageState::Adaptation,
    };
    let mut rows = [row; 24];
    let mut index = 0;
    while index < 24 {
        rows[index].mode = (index / 8) as u8;
        rows[index].output_mode = ((index / 2) % 4) as u8;
        rows[index].range = (index % 2) as u8;
        index += 1;
    }
    rows
}

pub const TIDES1: [FunctionSpec; 9] = first_generation();
pub const TIDES2: [FunctionSpec; 24] = second_generation();

#[must_use]
pub fn functions() -> (&'static [FunctionSpec; 9], &'static [FunctionSpec; 24]) {
    (&TIDES1, &TIDES2)
}

#[must_use]
pub fn functions_coverage_summary() -> &'static str {
    "Tides1: 9/9 AD/loop/AR x high/medium/low combinations are runnable analytic adaptations; default stereo multiplexes EOA level/EOR held-release roles, while opt-in quad routes unipolar, bipolar and both flags together. EOR scales ~1 ms at normal rates/~4 ms in low range below the source-derived threshold and stays high while idle. Optional wavetable hack excluded. Tides2: 24/24 ramp/output/range combinations are runnable analytic adaptations; default stereo selects two lanes and opt-in quad emits four direct-stem lanes. Source fixed-point timing and numerical parity remain open."
}
