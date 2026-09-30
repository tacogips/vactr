use super::{
    plaits_algorithms, plaits_coverage_summary, plaits_voice, CoverageState, Enveloped,
    ResourceState, VoiceRegistration, PLAITS_REVISION,
};
use crate::dsp::graph::EffectKind;
use crate::ns::insts::TEMPLATE_NAMES;
use crate::types::manifest::HostManifest;

#[test]
fn braids_registry_is_ordered_and_truthful() {
    let rows = super::braids_shapes();
    assert_eq!(rows.len(), 47);
    let manifest = HostManifest::spec_default();
    assert!(TEMPLATE_NAMES.contains(&"macro-five-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-sub-sync-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-triple-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-digital-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-filter-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-formant-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-fm-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-physical-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-struck-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-percussion-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-wave-grid-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-wave-line-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-noise-voice"));
    assert!(TEMPLATE_NAMES.contains(&"macro-cloud-voice"));
    for (position, row) in rows.iter().enumerate() {
        assert_eq!(usize::from(row.position), position);
        assert_eq!(row.source_revision, super::BRAIDS_REVISION);
        if matches!(position, 13 | 14 | 16 | 17..=46) {
            assert_eq!(row.source_path, "braids/digital_oscillator.cc");
        } else {
            assert_eq!(row.source_path, "braids/macro_oscillator.cc");
        }
        let (template, core) = if position < 5 {
            ("macro-five-voice", "macro-five-core")
        } else if position < 9 {
            ("macro-sub-sync-voice", "macro-sub-sync-core")
        } else if position < 13 {
            ("macro-triple-voice", "macro-triple-core")
        } else if position < 17 {
            ("macro-digital-voice", "macro-digital-core")
        } else if position < 21 {
            ("macro-filter-voice", "macro-filter-core")
        } else if position < 25 {
            ("macro-formant-voice", "macro-formant-core")
        } else if position < 28 {
            ("macro-fm-voice", "macro-fm-core")
        } else if position < 32 {
            ("macro-physical-voice", "macro-physical-core")
        } else if position < 34 {
            ("macro-struck-voice", "macro-struck-core")
        } else if position < 37 {
            ("macro-percussion-voice", "macro-percussion-core")
        } else if position < 39 {
            ("macro-wave-grid-voice", "macro-wave-grid-core")
        } else if position < 41 {
            ("macro-wave-line-voice", "macro-wave-line-core")
        } else if position < 44 {
            ("macro-noise-voice", "macro-noise-core")
        } else {
            ("macro-cloud-voice", "macro-cloud-core")
        };
        let editor = manifest.editor_decl(core).expect("registered core editor");
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
        assert_eq!(row.vactr_template, Some(template));
        for name in [
            row.pitch_control,
            row.color_control,
            row.timbre_control,
            row.strike_control,
            row.sync_control,
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                editor.params.iter().any(|param| param.name == name),
                "{name}"
            );
        }
        assert_eq!(row.upstream_wave_assets, (37..=40).contains(&position));
    }
    assert_eq!(rows[0].source_name, "CSAW");
    assert_eq!(rows[46].source_name, "DIGITAL_MODULATION");
    assert!(super::braids_coverage_summary().contains("47/47"));
}

#[test]
fn plaits_registry_is_complete_ordered_and_source_pinned() {
    let rows = plaits_algorithms();
    assert_eq!(rows.len(), 24);
    for (position, row) in rows.iter().enumerate() {
        assert_eq!(usize::from(row.position), position);
        assert_eq!(row.source_revision, PLAITS_REVISION);
        assert!(row.source_path.starts_with("plaits/dsp/engine"));
        assert!(row.source_path.ends_with("_engine.cc"));
        assert_eq!((row.main_outputs, row.aux_outputs), (1, 1));
    }
    assert_eq!(rows[2].source_path, rows[3].source_path);
    assert_eq!(rows[3].source_path, rows[4].source_path);
    assert_eq!(rows[23].source_engine, "hi_hat_engine");
}

#[test]
fn plaits_voice_registrations_match_all_24_pinned_rows() {
    let expected = [
        (Enveloped::Never, 1.0, 1.0),
        (Enveloped::Never, 0.7, 0.7),
        (Enveloped::Always, 1.0, 1.0),
        (Enveloped::Always, 1.0, 1.0),
        (Enveloped::Always, 1.0, 1.0),
        (Enveloped::Never, 0.7, 0.7),
        (Enveloped::Never, 0.8, 0.8),
        (Enveloped::WhenClocked, 0.5, 0.5),
        (Enveloped::Never, 0.8, 0.8),
        (Enveloped::Never, 0.7, 0.6),
        (Enveloped::Never, 0.6, 0.6),
        (Enveloped::Never, 0.7, 0.6),
        (Enveloped::Never, 0.8, 0.8),
        (Enveloped::Never, 0.6, 0.6),
        (Enveloped::Never, 0.8, 0.8),
        (Enveloped::Never, -0.7, 0.8),
        (Enveloped::Never, -3.0, 1.0),
        (Enveloped::Never, -1.0, -1.0),
        (Enveloped::Never, -2.0, 1.0),
        (Enveloped::Always, -1.0, 0.8),
        (Enveloped::Always, -1.0, 0.8),
        (Enveloped::Always, 0.8, 0.8),
        (Enveloped::Always, 0.8, 0.8),
        (Enveloped::Always, 0.8, 0.8),
    ];
    let rows = plaits_algorithms();
    for (position, (enveloped, out_gain, aux_gain)) in expected.into_iter().enumerate() {
        let registration = VoiceRegistration {
            enveloped,
            out_gain,
            aux_gain,
        };
        assert_eq!(plaits_voice(position), Some(registration));
        assert_eq!(rows[position].voice, registration);
        assert_eq!(rows[position].voice_layer, CoverageState::SourceStage);
        assert_ne!(rows[position].coverage, CoverageState::SourcePort);
    }
    assert_eq!(plaits_voice(24), None);

    assert_eq!(plaits_voice(9).unwrap().gain(0), 0.7);
    assert_eq!(plaits_voice(9).unwrap().gain(1), 0.6);
    assert_eq!(plaits_voice(17).unwrap().gain(0), -1.0);
    assert_eq!(plaits_voice(17).unwrap().gain(1), -1.0);
    assert!(!plaits_voice(7).unwrap().is_enveloped(false));
    assert!(plaits_voice(7).unwrap().is_enveloped(true));
    assert!(plaits_voice(21).unwrap().is_enveloped(false));
    assert!(plaits_voice(21).unwrap().is_enveloped(true));
    assert!(!plaits_voice(0).unwrap().is_enveloped(false));
    assert!(!plaits_voice(0).unwrap().is_enveloped(true));
}

#[test]
fn implemented_rows_have_registered_templates_and_editor_controls() {
    let manifest = HostManifest::spec_default();
    let rows = plaits_algorithms();
    let mut adaptations = 0;
    let mut source_stages = 0;
    for row in rows {
        assert_eq!(row.voice_layer, CoverageState::SourceStage);
        if let Some(name) = row.vactr_template {
            assert!(
                TEMPLATE_NAMES.contains(&name),
                "{name}: prelude registration"
            );
            let editor = manifest.editor_decl(name).expect("editor declaration");
            let controls = row.controls;
            for control in [
                controls.note,
                controls.harmonics,
                controls.timbre,
                controls.morph,
                controls.accent,
                controls.trigger_mode,
            ]
            .into_iter()
            .flatten()
            {
                assert!(
                    editor.params.iter().any(|param| param.name == control),
                    "{name}: {control} editor control"
                );
            }
            assert!(controls.note.is_some());
            assert!(controls.harmonics.is_some());
            assert!(controls.timbre.is_some());
            assert!(controls.morph.is_some());
            match row.coverage {
                CoverageState::Adaptation => adaptations += 1,
                CoverageState::SourceStage => source_stages += 1,
                other => panic!("{name}: invalid implemented status {other:?}"),
            }
            assert_eq!(row.resources, ResourceState::Replacement);
        } else {
            assert!(matches!(
                row.coverage,
                CoverageState::Pending | CoverageState::Unavailable
            ));
        }
    }
    assert_eq!((adaptations, source_stages), (15, 9));
    assert_eq!(rows[10].coverage, CoverageState::SourceStage);
    assert_eq!(rows[10].vactr_template, Some("fm-pair-voice"));
    assert_eq!(rows[11].coverage, CoverageState::SourceStage);
    assert_eq!(rows[11].vactr_template, Some("grain-pair-voice"));
    assert_eq!(rows[17].coverage, CoverageState::SourceStage);
    assert_eq!(rows[17].vactr_template, Some("clock-noise-voice"));
}

#[test]
fn resource_blocked_positions_are_not_reported_as_ports() {
    let rows = plaits_algorithms();
    assert_eq!(
        rows.iter().filter(|row| row.resource_flags.dx7_rom).count(),
        3
    );
    assert_eq!(
        rows.iter().filter(|row| row.resource_flags.ti_rom).count(),
        1
    );
    assert_eq!(rows[5].coverage, CoverageState::Adaptation);
    for row in &rows[2..=4] {
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
        assert!(row.resource_flags.dx7_rom);
        assert!(row.vactr_template.is_some());
    }
    assert_eq!(rows[5].resources, ResourceState::Replacement);
    assert!(rows[5].resource_flags.wave_assets);
    assert_eq!(rows[13].coverage, CoverageState::Adaptation);
    assert_eq!(rows[13].resources, ResourceState::Replacement);
    assert!(rows[13].resource_flags.wave_assets);
    assert_eq!(rows[14].coverage, CoverageState::Adaptation);
    assert_eq!(rows[14].resources, ResourceState::Replacement);
    assert!(rows[14].resource_flags.wave_assets);
    assert_eq!(rows[15].coverage, CoverageState::Adaptation);
    assert_eq!(rows[15].resources, ResourceState::Replacement);
    assert!(rows[15].resource_flags.ti_rom);
    assert_eq!(rows[15].vactr_template, Some("speech-voice"));
    assert!(rows
        .iter()
        .all(|row| row.coverage != CoverageState::SourcePort));
    for row in rows
        .iter()
        .filter(|row| row.resources == ResourceState::RightsBlocked)
    {
        assert_eq!(row.coverage, CoverageState::Unavailable);
        assert!(row.vactr_template.is_none());
        assert!(row.resource_flags.dx7_rom || row.resource_flags.ti_rom);
    }
    let summary = plaits_coverage_summary();
    assert!(summary.contains("24 positions"));
    assert!(summary.contains("0 pending"));
    assert!(summary.contains("15 adaptations"));
    assert!(summary.contains("9 source-stage translations"));
    assert!(summary.contains("0 source ports"));
    assert!(summary.contains("0 unavailable"));
    assert!(summary.contains("upstream wave assets unaudited: 3"));
    assert!(summary.ends_with(" Voice layer: 0 pending, 24 source-stage."));
}

// --- MOD-002: cross-family neutral-naming and template/effect matrix ---
//
// design-docs/specs/design-mutable-audio.md's "Scope and license boundary"
// requires every user-facing instrument/effect/control to carry a neutral
// Vactr name; upstream Mutable Instruments module names are attribution
// and engineering references only, never user-facing tokens.

/// Upstream Mutable Instruments module identifiers. No user-facing name
/// may contain one of these as a whole hyphen-separated token. Singular
/// common words such as "stream", "peak", "stage", "tidal" and "frame" are
/// not module identifiers and stay allowed.
const BANNED_MODULE_TOKENS: &[&str] = &[
    "plaits", "braids", "clouds", "rings", "elements", "warps", "tides", "tides2", "peaks",
    "streams", "stages", "frames", "marbles", "beads", "blades", "edges", "yarns", "grids",
    "shelves", "ripples", "veils", "links", "kinks", "branches", "shades", "volts", "ears", "el",
];

/// `"stages"` is a coincidental collision: the `phaser` effect's `stages`
/// control (`src/dsp/effects/modulation.rs`) counts allpass filter stages,
/// ordinary audio-engineering vocabulary predating and unrelated to any
/// Mutable Instruments module; it names no upstream source and needs no
/// MOD-002 rename. Braids' five user-facing controls were renamed to
/// `macro-*` (matching the existing `macro-*-voice` templates), so they no
/// longer need an exception here.
const KNOWN_PRE_EXISTING_EXCEPTIONS: &[&str] = &["stages"];

fn assert_neutral_name(name: &str) {
    if KNOWN_PRE_EXISTING_EXCEPTIONS.contains(&name) {
        return;
    }
    for token in name.split('-') {
        assert!(
            !BANNED_MODULE_TOKENS.contains(&token),
            "{name}: contains banned upstream module token {token:?}"
        );
    }
}

#[test]
fn no_user_facing_name_carries_an_upstream_module_token() {
    for name in TEMPLATE_NAMES {
        assert_neutral_name(name);
    }
    for kind in EffectKind::ALL {
        assert_neutral_name(kind.name());
    }
    assert_neutral_name("vactrol-gate");
    assert_neutral_name("decay-mod");
    let manifest = HostManifest::spec_default();
    for decl in manifest.editor_decls() {
        assert_neutral_name(decl.name);
        for param in decl.params.iter() {
            assert_neutral_name(param.name);
        }
    }
}

/// `.vact` sources of templates that are registered only after an explicit
/// opt-in load (quad-stem, external-voice and linked-segment templates),
/// so they are absent from the default `TEMPLATE_NAMES`. Ported inventory
/// rows may still name them; this checks the name is real, not a typo,
/// without duplicating the full `E2e`/registry load these examples' own
/// end-to-end tests already perform.
const OPT_IN_TEMPLATE_SOURCES: &[&str] = &[
    include_str!("../../../examples/quad-stems.vact"),
    include_str!("../../../examples/live-external-voices.vact"),
    include_str!("../../../examples/stage-linked.vact"),
];

fn is_opt_in_template(name: &str) -> bool {
    let needle = format!("inst {name} ");
    OPT_IN_TEMPLATE_SOURCES
        .iter()
        .any(|source| source.contains(needle.as_str()))
}

fn resolves_to_template_or_effect(name: &str) -> bool {
    TEMPLATE_NAMES.contains(&name)
        || EffectKind::from_name(name).is_some()
        || is_opt_in_template(name)
}

#[test]
fn every_ported_family_template_or_effect_name_resolves() {
    for row in plaits_algorithms() {
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "plaits: {name}");
        }
    }
    for row in super::braids_shapes() {
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "braids: {name}");
        }
    }
    for row in super::resonator_modes() {
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "elements: {name}");
        }
    }
    let alternate = super::ALTERNATE_VOICE.vactr_template.expect("named");
    assert!(
        resolves_to_template_or_effect(alternate),
        "elements alternate: {alternate}"
    );
    for row in super::frames_paths() {
        if let Some(name) = row.vactr_effect {
            assert!(
                resolves_to_template_or_effect(name),
                "frames effect: {name}"
            );
        }
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "frames: {name}");
        }
        if let Some(name) = row.opt_in_quad_template {
            assert!(
                resolves_to_template_or_effect(name),
                "frames opt-in: {name}"
            );
        }
    }
    assert_eq!(
        EffectKind::from_name("keyframe-mixer"),
        Some(EffectKind::KeyframeMixer)
    );
    let (tides1, tides2) = super::functions();
    for row in tides1.iter().chain(tides2.iter()) {
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "tides: {name}");
        }
        if let Some(name) = row.opt_in_quad_template {
            assert!(resolves_to_template_or_effect(name), "tides opt-in: {name}");
        }
    }
    for row in super::peaks_functions() {
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "peaks: {name}");
        }
    }
    for row in super::resonator_models() {
        if let Some(name) = row.voice_template {
            assert!(resolves_to_template_or_effect(name), "rings: {name}");
        }
    }
    let string_synth = super::string_synth_path().voice_template;
    assert!(
        resolves_to_template_or_effect(string_synth),
        "rings string synth: {string_synth}"
    );
    for row in super::stages_cells() {
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "stages: {name}");
        }
    }
    for row in super::STAGES_OTHER {
        if let Some(name) = row.vactr_template {
            assert!(resolves_to_template_or_effect(name), "stages other: {name}");
        }
    }
    for row in super::streams_functions() {
        if let Some(name) = row.digital_effect {
            assert!(resolves_to_template_or_effect(name), "streams: {name}");
        }
    }
    for row in super::clouds_modes() {
        assert!(
            resolves_to_template_or_effect(row.vactr_effect),
            "clouds: {}",
            row.vactr_effect
        );
    }
    for row in super::warps_algorithms() {
        assert!(
            resolves_to_template_or_effect(row.vactr_effect),
            "warps: {}",
            row.vactr_effect
        );
    }
}

#[test]
fn clouds_modes_are_ordered_pinned_and_never_source_port() {
    let rows = super::clouds_modes();
    assert_eq!(rows.len(), 4);
    for (mode, row) in rows.iter().enumerate() {
        assert_eq!(usize::from(row.mode), mode);
        assert_eq!(row.source_revision, super::CLOUDS_REVISION);
        assert_ne!(row.coverage, CoverageState::SourcePort);
        assert!(resolves_to_template_or_effect(row.vactr_effect));
        assert_eq!(row.coverage, CoverageState::Adaptation);
        assert_eq!(row.resources, ResourceState::Replacement);
    }
    assert_eq!(rows[0].vactr_effect, "texture-grain");
    assert_eq!(rows[1].vactr_effect, "texture-stretch");
    assert_eq!(rows[2].vactr_effect, "texture-loop");
    assert_eq!(rows[3].vactr_effect, "texture-spectral");
    assert_eq!(rows[0].source_path, "clouds/dsp/granular_sample_player.h");
    assert_eq!(rows[1].source_path, "clouds/dsp/wsola_sample_player.h");
    assert_eq!(rows[2].source_path, "clouds/dsp/looping_sample_player.h");
    assert_eq!(rows[3].source_path, "clouds/dsp/pvoc/phase_vocoder.cc");
    let summary = super::clouds_coverage_summary();
    assert!(summary.contains("4/4"));
    assert!(summary.contains("0 source-stage translations, 0 source ports"));
}

#[test]
fn warps_algorithms_are_ordered_pinned_and_never_source_port() {
    let rows = super::warps_algorithms();
    assert_eq!(rows.len(), 8);
    for (position, row) in rows.iter().enumerate() {
        assert_eq!(usize::from(row.position), position);
        assert_eq!(row.source_revision, super::WARPS_REVISION);
        assert_ne!(row.coverage, CoverageState::SourcePort);
        assert!(resolves_to_template_or_effect(row.vactr_effect));
    }
    for row in &rows[0..7] {
        assert_eq!(row.vactr_effect, "dual-mod");
    }
    assert_eq!(rows[7].vactr_effect, "shift-pair");
    // "The fold equation remains authored": fold is the one XMOD row that
    // is an adaptation rather than a translated source equation.
    assert_eq!(rows[1].coverage, CoverageState::Adaptation);
    for position in [0usize, 2, 3, 4, 5, 6] {
        assert_eq!(
            rows[position].coverage,
            CoverageState::SourceStage,
            "position {position}"
        );
    }
    assert_eq!(rows[7].coverage, CoverageState::Adaptation);
    assert_eq!(rows[7].resources, ResourceState::None);
    for row in &rows[0..=6] {
        assert_eq!(row.resources, ResourceState::Replacement);
    }
    let summary = super::warps_coverage_summary();
    assert!(summary.contains("6 source-stage translations"));
    assert!(summary.contains("0 source ports"));
}
