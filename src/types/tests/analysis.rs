//! SS-ANALYSIS: the self-analysis surfaces in the checker (design 14.5.9):
//! `scope`, `spectrum` and `render` in the M1 subject-overload group, and
//! `capture`, `rms`, `peak`.

use super::{assert_clean, assert_has, last_type};
use crate::types::diag::{DiagCode, Severity};
use crate::types::natives::{HostCap, NativeTable};

#[test]
fn every_analysis_overload_types_as_in_the_table() {
    let cases = [
        ("scope :master 512", "[float]"),
        ("let buf render 2\nscope buf 64", "[float]"),
        ("spectrum :master", "[float]"),
        ("spectrum :master bins: 64", "[float]"),
        ("let buf render 2\nspectrum buf bins: 64", "[float]"),
        ("capture :master 1", "sound"),
        ("render 2", "sound"),
        ("let buf render 2\nrms buf", "float"),
        ("let buf render 2\npeak buf", "float"),
        ("let hit capture :master 1\nrms hit", "float"),
    ];
    for (src, want) in cases {
        assert_clean(src);
        assert_eq!(last_type(src), want, "{src:?}");
    }
}

#[test]
fn render_with_a_keyword_or_no_subject_stays_the_visual_setting() {
    assert_clean("render :o0");
    assert_eq!(last_type("render :o0"), "nil");
    // Bare `render` checks clean; as a value it is the overload group.
    assert_clean("render");
    assert_eq!(last_type("render"), "any");
}

#[test]
fn spectrum_in_a_bus_body_stays_the_analyzer_unit() {
    assert_clean("bus :meters:\n\tlevel > spectrum");
    assert_clean("master:\n\tlevel > spectrum");
    assert_eq!(last_type("sin-osc 440 > spectrum"), "ugen");
}

#[test]
fn the_group_is_overloaded_and_capture_is_an_effect_that_needs_analysis() {
    let t = NativeTable::global();
    for name in ["scope", "spectrum", "render", "scale", "shape"] {
        let (_, sig) = t.get(name).expect("an entry");
        assert!(sig.is_overloaded(), "{name}");
    }
    let (_, capture) = t.get("capture").expect("capture");
    assert!(capture.effectful);
    assert_eq!(capture.needs, &[HostCap::Analysis]);
    let (_, render) = t.get("render").expect("render");
    assert!(render.effectful);
    for name in ["rms", "peak", "scope"] {
        let (_, sig) = t.get(name).expect("an entry");
        assert!(!sig.effectful, "{name}");
    }
    let (_, spectrum) = t.get("spectrum").expect("spectrum");
    assert!(spectrum.keywords.contains(&"bins"));
}

#[test]
fn a_rms_of_a_non_sound_is_a_type_mismatch() {
    assert_has("rms 3", DiagCode::TypeMismatch, 1, Severity::Error);
    assert_has(
        "scope :master \"x\"",
        DiagCode::TypeMismatch,
        1,
        Severity::Error,
    );
}

#[test]
fn capture_inside_a_pattern_query_is_effect_in_pattern() {
    assert_has(
        "s :bd > fast {capture :master 1}",
        DiagCode::EffectInPattern,
        1,
        DiagCode::EffectInPattern.default_severity(),
    );
}
