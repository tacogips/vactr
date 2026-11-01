use crate::dsp::graph::{AnalyzerKind, EffectKind};
#[test]
fn every_effect_kind_maps_to_same_named_crate_kind() {
    for &kind in EffectKind::ALL {
        let expected = if kind == EffectKind::StreamVactr {
            "StreamOpto".to_owned()
        } else {
            format!("{kind:?}")
        };
        assert_eq!(
            format!("{:?}", music_things_fx::EffectKind::from(kind)),
            expected
        );
    }
}
#[test]
fn every_analyzer_kind_maps_to_same_named_crate_kind() {
    for &kind in AnalyzerKind::ALL {
        assert_eq!(
            format!("{:?}", music_things_fx::AnalyzerKind::from(kind)),
            format!("{kind:?}")
        );
    }
}
