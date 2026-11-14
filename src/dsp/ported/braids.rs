//! Published Braids registration order and truthful Vactr coverage.

use super::{CoverageState, ResourceState};

/// Pinned Eurorack source revision used for this inventory.
pub const BRAIDS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

/// One accessible shape before the question-mark sentinel in `settings.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BraidsShapeSpec {
    pub position: u8,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub vactr_template: Option<&'static str>,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    /// The upstream wave-bank family has an unaudited data dependency.
    /// Runnable replacements do not import that data.
    pub upstream_wave_assets: bool,
    pub pitch_control: Option<&'static str>,
    pub color_control: Option<&'static str>,
    pub timbre_control: Option<&'static str>,
    pub strike_control: Option<&'static str>,
    pub sync_control: Option<&'static str>,
}

const NAMES: [&str; 47] = [
    "CSAW",
    "MORPH",
    "SAW_SQUARE",
    "SINE_TRIANGLE",
    "BUZZ",
    "SQUARE_SUB",
    "SAW_SUB",
    "SQUARE_SYNC",
    "SAW_SYNC",
    "TRIPLE_SAW",
    "TRIPLE_SQUARE",
    "TRIPLE_TRIANGLE",
    "TRIPLE_SINE",
    "TRIPLE_RING_MOD",
    "SAW_SWARM",
    "SAW_COMB",
    "TOY",
    "DIGITAL_FILTER_LP",
    "DIGITAL_FILTER_PK",
    "DIGITAL_FILTER_BP",
    "DIGITAL_FILTER_HP",
    "VOSIM",
    "VOWEL",
    "VOWEL_FOF",
    "HARMONICS",
    "FM",
    "FEEDBACK_FM",
    "CHAOTIC_FEEDBACK_FM",
    "PLUCKED",
    "BOWED",
    "BLOWN",
    "FLUTED",
    "STRUCK_BELL",
    "STRUCK_DRUM",
    "KICK",
    "CYMBAL",
    "SNARE",
    "WAVETABLES",
    "WAVE_MAP",
    "WAVE_LINE",
    "WAVE_PARAPHONIC",
    "FILTERED_NOISE",
    "TWIN_PEAKS_NOISE",
    "CLOCKED_NOISE",
    "GRANULAR_CLOUD",
    "PARTICLE_NOISE",
    "DIGITAL_MODULATION",
];

const fn rows() -> [BraidsShapeSpec; 47] {
    let empty = BraidsShapeSpec {
        position: 0,
        source_name: "",
        source_path: "braids/macro_oscillator.cc",
        source_revision: BRAIDS_REVISION,
        vactr_template: None,
        coverage: CoverageState::Pending,
        resources: ResourceState::NeedsAudit,
        upstream_wave_assets: false,
        pitch_control: None,
        color_control: None,
        timbre_control: None,
        strike_control: None,
        sync_control: None,
    };
    let mut result = [empty; 47];
    let mut i = 0;
    while i < 47 {
        result[i].position = i as u8;
        result[i].source_name = NAMES[i];
        if i == 13 || i == 14 || i == 16 || i >= 17 {
            result[i].source_path = "braids/digital_oscillator.cc";
        }
        result[i].upstream_wave_assets = i >= 37 && i <= 40;
        result[i].vactr_template = Some(if i < 5 {
            "macro-five-voice"
        } else if i < 9 {
            "macro-sub-sync-voice"
        } else if i < 13 {
            "macro-triple-voice"
        } else if i < 17 {
            "macro-digital-voice"
        } else if i < 21 {
            "macro-filter-voice"
        } else if i < 25 {
            "macro-formant-voice"
        } else if i < 28 {
            "macro-fm-voice"
        } else if i < 32 {
            "macro-physical-voice"
        } else if i < 34 {
            "macro-struck-voice"
        } else if i < 37 {
            "macro-percussion-voice"
        } else if i < 39 {
            "macro-wave-grid-voice"
        } else if i < 41 {
            "macro-wave-line-voice"
        } else if i < 44 {
            "macro-noise-voice"
        } else {
            "macro-cloud-voice"
        });
        result[i].coverage = CoverageState::Adaptation;
        result[i].resources = ResourceState::Replacement;
        result[i].pitch_control = Some("freq");
        result[i].color_control = Some("macro-color");
        result[i].timbre_control = Some("macro-timbre");
        result[i].strike_control = Some("macro-strike");
        result[i].sync_control = Some("macro-sync");
        i += 1;
    }
    result
}

const SHAPES: [BraidsShapeSpec; 47] = rows();

/// All 47 source positions, in stable published order.
pub const fn braids_shapes() -> &'static [BraidsShapeSpec; 47] {
    &SHAPES
}

/// Concise human-readable status for command-line or editor use.
pub fn braids_coverage_summary() -> String {
    let adapted = SHAPES
        .iter()
        .filter(|row| row.coverage == CoverageState::Adaptation)
        .count();
    format!(
        "Braids: {adapted}/47 runnable analytic adaptations, {} pending; four upstream wave-bank positions retain unaudited source assets; runnable replacements import none. No source-parity ports.",
        47 - adapted
    )
}
