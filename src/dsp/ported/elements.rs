//! Checked Elements internal instrument and external-input bus adaptations.

use super::{CoverageState, ResourceState};
use crate::dsp::graph::EffectKind;

pub const ELEMENTS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementsSpec {
    pub mode: u8,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub vactrol_template: Option<&'static str>,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    pub external_blow: bool,
    pub external_strike: bool,
    pub external_effect: Option<EffectKind>,
}

pub const RESONATORS: [ElementsSpec; 3] = [
    ElementsSpec {
        mode: 0,
        source_name: "MODAL",
        source_path: "elements/dsp/voice.cc",
        vactrol_template: Some("elements-voice"),
        coverage: CoverageState::Adaptation,
        resources: ResourceState::Replacement,
        external_blow: true,
        external_strike: true,
        external_effect: Some(EffectKind::ElementsBank),
    },
    ElementsSpec {
        mode: 1,
        source_name: "STRING",
        source_path: "elements/dsp/voice.cc",
        vactrol_template: Some("elements-voice"),
        coverage: CoverageState::Adaptation,
        resources: ResourceState::Replacement,
        external_blow: true,
        external_strike: true,
        external_effect: Some(EffectKind::ElementsBank),
    },
    ElementsSpec {
        mode: 2,
        source_name: "STRINGS",
        source_path: "elements/dsp/voice.cc",
        vactrol_template: Some("elements-voice"),
        coverage: CoverageState::Adaptation,
        resources: ResourceState::Replacement,
        external_blow: true,
        external_strike: true,
        external_effect: Some(EffectKind::ElementsBank),
    },
];

pub const ALTERNATE_VOICE: ElementsSpec = ElementsSpec {
    mode: 3,
    source_name: "OMINOUS_ALTERNATE",
    source_path: "elements/dsp/ominous_voice.h",
    vactrol_template: Some("elements-voice"),
    coverage: CoverageState::Adaptation,
    resources: ResourceState::Replacement,
    external_blow: true,
    external_strike: true,
    external_effect: Some(EffectKind::ElementsBank),
};

#[must_use]
pub fn resonator_modes() -> &'static [ElementsSpec; 3] {
    &RESONATORS
}

#[must_use]
pub fn elements_coverage_summary() -> &'static str {
    "Elements: 3/3 resonator roles and alternate FM/spatial voice runnable as procedural instrument and separate blow/strike bus adaptations; source 8x/FIR/filter parity pending; no GPL samples or generated tables imported."
}
