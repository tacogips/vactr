//! Frames firmware control-path inventory; analog mixer/VCA has no DSP port.

pub const FRAMES_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";
pub const KEYFRAME_CAPACITY: usize = 64;
pub const EASING_OPTIONS: [&str; 6] = [
    "step",
    "linear",
    "in-quartic",
    "out-quartic",
    "sine",
    "bounce",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramesCoverage {
    Adaptation,
    Pending,
    Excluded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramesPathSpec {
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub role: &'static str,
    pub vactrol_template: Option<&'static str>,
    /// Four-output template available after explicit `.vact` load.
    pub opt_in_quad_template: Option<&'static str>,
    pub coverage: FramesCoverage,
    pub source_channels: u8,
    pub simultaneous_vactrol_channels: u8,
    pub source_generated_wave_table: bool,
}

pub const FRAMES_PATHS: [FramesPathSpec; 3] = [
    FramesPathSpec {
        source_path: "frames/poly_lfo.cc",
        source_revision: FRAMES_REVISION,
        role: "four DAC poly-LFO lanes; shape/spreads/coupling",
        vactrol_template: Some("frame-lfo-voice"),
        opt_in_quad_template: Some("frame-lfo-quad-voice"),
        coverage: FramesCoverage::Adaptation,
        source_channels: 4,
        simultaneous_vactrol_channels: 2,
        source_generated_wave_table: true,
    },
    FramesPathSpec {
        source_path: "frames/keyframer.cc",
        source_revision: FRAMES_REVISION,
        role: "64 four-channel keyframes; six easing options and response",
        vactrol_template: Some("frame-keyframe-voice"),
        opt_in_quad_template: Some("frame-keyframe-quad-voice"),
        coverage: FramesCoverage::Adaptation,
        source_channels: 4,
        simultaneous_vactrol_channels: 2,
        source_generated_wave_table: false,
    },
    FramesPathSpec {
        source_path: "frames/frames.cc",
        source_revision: FRAMES_REVISION,
        role: "physical analog mixer/VCA audio path",
        vactrol_template: None,
        opt_in_quad_template: None,
        coverage: FramesCoverage::Excluded,
        source_channels: 4,
        simultaneous_vactrol_channels: 0,
        source_generated_wave_table: false,
    },
];

pub const fn frames_paths() -> &'static [FramesPathSpec; 3] {
    &FRAMES_PATHS
}

pub fn frames_coverage_summary() -> String {
    String::from("Frames: poly-LFO and 64-frame keyframer have original analytic digital adaptations; two lanes are selected by default or all four emitted via opt-in quad direct stems; list-editor controls pending; analog mixer/VCA is outside firmware DSP")
}
