//! Checked inventory of the six published Rings `Part` resonator models.
//!
//! A separate string-synth Part and external audio bus adaptation are listed.

use std::fmt::Write;

use super::{CoverageState, ResourceState};
use crate::dsp::graph::EffectKind;

/// Pinned Mutable Instruments Eurorack revision for the Rings inventory.
pub const RINGS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResonatorSpec {
    pub model: u8,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub voice_template: Option<&'static str>,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    pub main_outputs: u8,
    pub aux_outputs: u8,
    pub external_excitation: bool,
    pub external_effect: Option<EffectKind>,
}

const NAMES: [&str; 6] = [
    "MODAL_RESONATOR",
    "SYMPATHETIC_STRING",
    "STRING",
    "FM_VOICE",
    "SYMPATHETIC_STRING_QUANTIZED",
    "STRING_AND_REVERB",
];

const fn rows() -> [ResonatorSpec; 6] {
    let empty = ResonatorSpec {
        model: 0,
        source_name: "",
        source_path: "rings/dsp/part.cc",
        source_revision: RINGS_REVISION,
        voice_template: Some("resonator-voice"),
        coverage: CoverageState::Adaptation,
        resources: ResourceState::Replacement,
        main_outputs: 1,
        aux_outputs: 1,
        external_excitation: true,
        external_effect: Some(EffectKind::ResonantBank),
    };
    let mut rows = [empty; 6];
    let mut i = 0;
    while i < 6 {
        rows[i].model = i as u8;
        rows[i].source_name = NAMES[i];
        i += 1;
    }
    rows
}

static MODELS: [ResonatorSpec; 6] = rows();

#[must_use]
pub fn resonator_models() -> &'static [ResonatorSpec; 6] {
    &MODELS
}

#[must_use]
pub fn rings_coverage_summary() -> String {
    let mut out = String::from("Rings Part: 6/6 runnable event-local adaptations; 0 source ports; separate string-synth adaptation runnable; external-audio resonant-bank bus adaptation available.\n");
    for row in MODELS {
        let _ = writeln!(
            out,
            "{}: {} -> resonator-voice / resonant-bank (Adaptation, analytic resources)",
            row.model, row.source_name
        );
    }
    out
}

/// Separate source StringSynthPart path, distinct from the six Part models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringSynthSpec {
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub voice_template: &'static str,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    pub fx_roles: [&'static str; 6],
    pub external_excitation: bool,
}

static STRING_SYNTH: StringSynthSpec = StringSynthSpec {
    source_path: "rings/dsp/string_synth_part.cc",
    source_revision: RINGS_REVISION,
    voice_template: "string-choir-voice",
    coverage: CoverageState::Adaptation,
    resources: ResourceState::Replacement,
    fx_roles: [
        "formant",
        "chorus",
        "short reverb",
        "formant 2",
        "ensemble",
        "reverb 2",
    ],
    external_excitation: false,
};

#[must_use]
pub fn string_synth_path() -> &'static StringSynthSpec {
    &STRING_SYNTH
}
