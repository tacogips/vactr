//! Plaits' exact 0–23 registration order at the pinned source revision.

use std::fmt::Write;

/// The upstream Eurorack revision used for every row in this manifest.
pub const PLAITS_REVISION: &str = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4";

/// Fidelity describes Vactr behavior, not the upstream file's license.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageState {
    /// No Vactr renderer yet.
    Pending,
    /// Runnable independent architecture with known source-stage gaps.
    Adaptation,
    /// Source signal stages translated; numerical or host behavior differs.
    SourceStage,
    /// Full source and host behavior validated against upstream.
    SourcePort,
    /// The published resource-backed mode is unavailable in this build.
    Unavailable,
}

/// Asset provenance for the selected position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceState {
    /// No engine-specific external resource identified in the pinned source.
    None,
    /// Vactr uses cleared, original or analytic replacements.
    Replacement,
    /// The engine's exact assets still require a separate provenance audit.
    NeedsAudit,
    /// The published assets are excluded pending independent rights or replacement.
    RightsBlocked,
}

/// Specific asset families that cannot be inferred from an MIT source header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceFlags {
    /// Six-operator banks extracted from Yamaha DX7 SysEx ROM sets.
    pub dx7_rom: bool,
    /// LPC words described by upstream as extracted from TI ROMs.
    pub ti_rom: bool,
    /// Wave/terrain assets whose individual origin is not yet cleared.
    pub wave_assets: bool,
}

impl ResourceFlags {
    const NONE: Self = Self {
        dx7_rom: false,
        ti_rom: false,
        wave_assets: false,
    };
    const DX7: Self = Self {
        dx7_rom: true,
        ..Self::NONE
    };
    const TI: Self = Self {
        ti_rom: true,
        ..Self::NONE
    };
    const WAVES: Self = Self {
        wave_assets: true,
        ..Self::NONE
    };
}

/// Whether an upstream engine already applies its own envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enveloped {
    Never,
    Always,
    WhenClocked,
}

/// Per-position voice-level registration copied from upstream `Voice::Init`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceRegistration {
    pub enveloped: Enveloped,
    pub out_gain: f32,
    pub aux_gain: f32,
}

impl VoiceRegistration {
    /// Returns the registered gain for lane zero (main) or lane one (aux).
    #[must_use]
    pub fn gain(&self, lane: u8) -> f32 {
        if lane == 0 {
            self.out_gain
        } else {
            self.aux_gain
        }
    }

    /// Resolves the registration rule against the engine's clocked state.
    #[must_use]
    pub fn is_enveloped(&self, clocked: bool) -> bool {
        match self.enveloped {
            Enveloped::Never => false,
            Enveloped::Always => true,
            Enveloped::WhenClocked => clocked,
        }
    }
}

static VOICE_REGISTRATIONS: [VoiceRegistration; 24] = [
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 1.0,
        aux_gain: 1.0,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.7,
        aux_gain: 0.7,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: 1.0,
        aux_gain: 1.0,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: 1.0,
        aux_gain: 1.0,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: 1.0,
        aux_gain: 1.0,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.7,
        aux_gain: 0.7,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.8,
        aux_gain: 0.8,
    },
    // The chiptune engine sets its enveloped flag when clocked.
    VoiceRegistration {
        enveloped: Enveloped::WhenClocked,
        out_gain: 0.5,
        aux_gain: 0.5,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.8,
        aux_gain: 0.8,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.7,
        aux_gain: 0.6,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.6,
        aux_gain: 0.6,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.7,
        aux_gain: 0.6,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.8,
        aux_gain: 0.8,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.6,
        aux_gain: 0.6,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: 0.8,
        aux_gain: 0.8,
    },
    // This position intentionally follows the accepted design's Never rule.
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: -0.7,
        aux_gain: 0.8,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: -3.0,
        aux_gain: 1.0,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: -1.0,
        aux_gain: -1.0,
    },
    VoiceRegistration {
        enveloped: Enveloped::Never,
        out_gain: -2.0,
        aux_gain: 1.0,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: -1.0,
        aux_gain: 0.8,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: -1.0,
        aux_gain: 0.8,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: 0.8,
        aux_gain: 0.8,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: 0.8,
        aux_gain: 0.8,
    },
    VoiceRegistration {
        enveloped: Enveloped::Always,
        out_gain: 0.8,
        aux_gain: 0.8,
    },
];

/// Returns the registration tuple for a Plaits position, if it is in range.
#[must_use]
pub fn plaits_voice(slot: usize) -> Option<VoiceRegistration> {
    VOICE_REGISTRATIONS.get(slot).copied()
}

/// Source common-control role mapped to a `.vact` name, when implemented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommonControls {
    pub note: Option<&'static str>,
    pub harmonics: Option<&'static str>,
    pub timbre: Option<&'static str>,
    pub morph: Option<&'static str>,
    pub accent: Option<&'static str>,
    pub trigger_mode: Option<&'static str>,
}

const fn controls(
    harmonics: &'static str,
    accent: bool,
    trigger: Option<&'static str>,
) -> CommonControls {
    CommonControls {
        note: Some("freq"),
        harmonics: Some(harmonics),
        timbre: Some("timbre"),
        morph: Some("morph"),
        accent: if accent { Some("velocity") } else { None },
        trigger_mode: trigger,
    }
}

/// One position in the source `Voice::Init` engine registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortedAlgorithm {
    pub position: u8,
    pub source_engine: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub vactr_template: Option<&'static str>,
    pub controls: CommonControls,
    /// Number of independently emitted upstream main channels (one).
    pub main_outputs: u8,
    /// Number of independently emitted upstream auxiliary channels (one).
    pub aux_outputs: u8,
    pub coverage: CoverageState,
    pub voice: VoiceRegistration,
    pub voice_layer: CoverageState,
    pub resources: ResourceState,
    pub resource_flags: ResourceFlags,
}

const fn implemented(
    position: u8,
    engine: &'static str,
    path: &'static str,
    template: &'static str,
    controls: CommonControls,
    coverage: CoverageState,
    voice: VoiceRegistration,
) -> PortedAlgorithm {
    PortedAlgorithm {
        position,
        source_engine: engine,
        source_path: path,
        source_revision: PLAITS_REVISION,
        vactr_template: Some(template),
        controls,
        main_outputs: 1,
        aux_outputs: 1,
        coverage,
        voice,
        voice_layer: CoverageState::SourceStage,
        resources: ResourceState::Replacement,
        resource_flags: ResourceFlags::NONE,
    }
}

static PLAITS: [PortedAlgorithm; 24] = [
    implemented(
        0,
        "virtual_analog_vcf_engine",
        "plaits/dsp/engine2/virtual_analog_vcf_engine.cc",
        "filter-voice",
        controls("filter-harmonics", false, None),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[0],
    ),
    implemented(
        1,
        "phase_distortion_engine",
        "plaits/dsp/engine2/phase_distortion_engine.cc",
        "phase-pair-voice",
        controls("phase-harmonics", false, None),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[1],
    ),
    {
        let mut row = implemented(
            2,
            "six_op_engine bank A",
            "plaits/dsp/engine2/six_op_engine.cc",
            "six-bank-a-voice",
            controls("six-patch", true, Some("six-sustain")),
            CoverageState::Adaptation,
            VOICE_REGISTRATIONS[2],
        );
        row.resource_flags = ResourceFlags::DX7;
        row
    },
    {
        let mut row = implemented(
            3,
            "six_op_engine bank B",
            "plaits/dsp/engine2/six_op_engine.cc",
            "six-bank-b-voice",
            controls("six-patch", true, Some("six-sustain")),
            CoverageState::Adaptation,
            VOICE_REGISTRATIONS[3],
        );
        row.resource_flags = ResourceFlags::DX7;
        row
    },
    {
        let mut row = implemented(
            4,
            "six_op_engine bank C",
            "plaits/dsp/engine2/six_op_engine.cc",
            "six-bank-c-voice",
            controls("six-patch", true, Some("six-sustain")),
            CoverageState::Adaptation,
            VOICE_REGISTRATIONS[4],
        );
        row.resource_flags = ResourceFlags::DX7;
        row
    },
    {
        // Upstream modes 5-7 read unaudited wav_integrated_waves/waves.bin;
        // Vactr uses original procedural surfaces, retaining source provenance.
        let mut row = implemented(
            5,
            "wave_terrain_engine",
            "plaits/dsp/engine2/wave_terrain_engine.cc",
            "terrain-voice",
            controls("terrain-select", false, None),
            CoverageState::Adaptation,
            VOICE_REGISTRATIONS[5],
        );
        row.resource_flags = ResourceFlags::WAVES;
        row
    },
    // No wav_integrated_waves/waves.bin read. Ensemble SineRaw reads lut_sine;
    // analytic sine replaces that generated LFO table in the adaptation.
    implemented(
        6,
        "string_machine_engine",
        "plaits/dsp/engine2/string_machine_engine.cc",
        "string-machine-voice",
        controls("machine-chord", false, None),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[6],
    ),
    implemented(
        7,
        "chiptune_engine",
        "plaits/dsp/engine2/chiptune_engine.cc",
        "chip-voice",
        controls("chip-chord", false, Some("chip-clocked")),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[7],
    ),
    implemented(
        8,
        "virtual_analog_engine",
        "plaits/dsp/engine/virtual_analog_engine.cc",
        "analog-pair-voice",
        controls("analog-detune", false, None),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[8],
    ),
    implemented(
        9,
        "waveshaping_engine",
        "plaits/dsp/engine/waveshaping_engine.cc",
        "shape-voice",
        controls("shape-harmonics", false, None),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[9],
    ),
    implemented(
        10,
        "fm_engine",
        "plaits/dsp/engine/fm_engine.cc",
        "fm-pair-voice",
        controls("fm-harmonics", false, None),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[10],
    ),
    implemented(
        11,
        "grain_engine",
        "plaits/dsp/engine/grain_engine.cc",
        "grain-pair-voice",
        controls("grain-harmonics", false, None),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[11],
    ),
    implemented(
        12,
        "additive_engine",
        "plaits/dsp/engine/additive_engine.cc",
        "spectrum-voice",
        controls("spectrum-bumps", false, None),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[12],
    ),
    {
        // Upstream reads unaudited waves.bin; the runnable Vactr grid is
        // original procedural audio, with no wave data imported.
        let mut row = implemented(
            13,
            "wavetable_engine",
            "plaits/dsp/engine/wavetable_engine.cc",
            "wave-grid-voice",
            controls("wave-bank", false, None),
            CoverageState::Adaptation,
            VOICE_REGISTRATIONS[13],
        );
        row.resource_flags = ResourceFlags::WAVES;
        row
    },
    {
        // Upstream selects 15 unaudited integrated waves; this renderer
        // replaces them with original procedural timbres.
        let mut row = implemented(
            14,
            "chord_engine",
            "plaits/dsp/engine/chord_engine.cc",
            "chord-layer-voice",
            controls("layer-chord", false, None),
            CoverageState::Adaptation,
            VOICE_REGISTRATIONS[14],
        );
        row.resource_flags = ResourceFlags::WAVES;
        row
    },
    {
        let mut row = implemented(
            15,
            "speech_engine",
            "plaits/dsp/engine/speech_engine.cc",
            "speech-voice",
            controls("speech-harmonics", true, Some("speech-sustain")),
            CoverageState::Adaptation,
            VOICE_REGISTRATIONS[15],
        );
        row.resource_flags = ResourceFlags::TI;
        row
    },
    implemented(
        16,
        "swarm_engine",
        "plaits/dsp/engine/swarm_engine.cc",
        "swarm-voice",
        controls("swarm-spread", false, Some("swarm-continuous")),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[16],
    ),
    implemented(
        17,
        "noise_engine",
        "plaits/dsp/engine/noise_engine.cc",
        "clock-noise-voice",
        controls("noise-harmonics", false, None),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[17],
    ),
    implemented(
        18,
        "particle_engine",
        "plaits/dsp/engine/particle_engine.cc",
        "particle-voice",
        controls("particle-spread", false, None),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[18],
    ),
    implemented(
        19,
        "string_engine",
        "plaits/dsp/engine/string_engine.cc",
        "string-voice",
        controls("string-structure", true, Some("string-sustain")),
        CoverageState::Adaptation,
        VOICE_REGISTRATIONS[19],
    ),
    implemented(
        20,
        "modal_engine",
        "plaits/dsp/engine/modal_engine.cc",
        "modal-voice",
        controls("modal-structure", true, Some("modal-sustain")),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[20],
    ),
    implemented(
        21,
        "bass_drum_engine",
        "plaits/dsp/engine/bass_drum_engine.cc",
        "dual-kick-voice",
        controls("kick-harmonics", true, Some("kick-sustain")),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[21],
    ),
    implemented(
        22,
        "snare_drum_engine",
        "plaits/dsp/engine/snare_drum_engine.cc",
        "dual-snare-voice",
        controls("snare-harmonics", true, Some("snare-sustain")),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[22],
    ),
    implemented(
        23,
        "hi_hat_engine",
        "plaits/dsp/engine/hi_hat_engine.cc",
        "dual-hat-voice",
        controls("hat-harmonics", true, Some("hat-sustain")),
        CoverageState::SourceStage,
        VOICE_REGISTRATIONS[23],
    ),
];

/// All 24 published Plaits positions in their upstream registry order.
#[must_use]
pub fn plaits_algorithms() -> &'static [PortedAlgorithm] {
    &PLAITS
}

/// A concise, public human-readable status. No entry is called complete
/// unless it is explicitly `SourcePort`.
#[must_use]
pub fn plaits_coverage_summary() -> String {
    let mut counts = [0usize; 5];
    let mut voice_pending = 0;
    let mut voice_source_stage = 0;
    let mut dx7 = 0;
    let mut ti = 0;
    let mut wave_audit = 0;
    for row in plaits_algorithms() {
        dx7 += usize::from(row.resource_flags.dx7_rom);
        ti += usize::from(row.resource_flags.ti_rom);
        wave_audit += usize::from(row.resource_flags.wave_assets);
        let index = match row.coverage {
            CoverageState::Pending => 0,
            CoverageState::Adaptation => 1,
            CoverageState::SourceStage => 2,
            CoverageState::SourcePort => 3,
            CoverageState::Unavailable => 4,
        };
        counts[index] += 1;
        match row.voice_layer {
            CoverageState::Pending => voice_pending += 1,
            CoverageState::SourceStage => voice_source_stage += 1,
            _ => {}
        }
    }
    let mut summary = String::new();
    let _ = write!(summary,
        "Plaits: {} positions; {} pending, {} adaptations, {} source-stage translations, {} source ports, {} unavailable (DX7 ROM: {}; TI ROM: {}; upstream wave assets unaudited: {}). Voice layer: {} pending, {} source-stage.",
        PLAITS.len(), counts[0], counts[1], counts[2], counts[3], counts[4], dx7, ti, wave_audit, voice_pending, voice_source_stage);
    summary
}
