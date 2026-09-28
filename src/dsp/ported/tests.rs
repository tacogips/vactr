use super::{
    plaits_algorithms, plaits_coverage_summary, CoverageState, ResourceState, PLAITS_REVISION,
};
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
fn implemented_rows_have_registered_templates_and_editor_controls() {
    let manifest = HostManifest::spec_default();
    let rows = plaits_algorithms();
    let mut adaptations = 0;
    let mut source_stages = 0;
    for row in rows {
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
}
