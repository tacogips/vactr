use std::collections::BTreeMap;

use crate::pattern::tuning::{
    edo_spec, ratios_spec, scala_spec, scale_preset, tuning_preset, Mapping, Tuning,
};
use crate::value::intern::intern_kw;
use crate::value::key::Key;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

fn close(actual: f64, expected: f64) {
    let error = (actual - expected).abs() / expected.abs().max(f64::MIN_POSITIVE);
    assert!(
        error <= 1e-12,
        "relative error {error}: actual {actual}, expected {expected}"
    );
}

fn freq(tuning: &Tuning, key: f64) -> f64 {
    tuning.freq(key).unwrap().unwrap()
}
fn edo(steps: i64) -> Tuning {
    Tuning::edo(steps, Ratio64::from_int(2)).unwrap()
}
#[test]
fn edo_19_and_31_match_analytic_oracle() {
    let anchor = crate::sched::commit::note_to_freq(60.0);
    for steps in [19_i64, 31] {
        let tuning = edo(steps);
        for degree in -steps..=38 {
            close(
                freq(&tuning, (60 + degree) as f64),
                anchor * (degree as f64 / steps as f64).exp2(),
            );
        }
    }
}

#[test]
fn non_octave_edo_matches_analytic_oracle() {
    let tuning = Tuning::edo(13, Ratio64::from_int(3)).unwrap();
    let anchor = crate::sched::commit::note_to_freq(60.0);
    for degree in -13..=38 {
        close(
            freq(&tuning, (60 + degree) as f64),
            anchor * 3_f64.powf(degree as f64 / 13.0),
        );
    }
}

#[test]
fn ji_and_partch_presets_resolve_periods() {
    let ji = Tuning::from_spec(&Value::kw("ji-5")).unwrap();
    let anchor = crate::sched::commit::note_to_freq(60.0);
    assert_eq!(freq(&ji, 60.0), anchor);
    close(freq(&ji, 64.0), anchor * 5.0 / 4.0);
    close(freq(&ji, 72.0), anchor * 2.0);
    close(freq(&ji, 48.0), anchor / 2.0);
    let partch = Tuning::from_spec(&Value::kw("partch-43")).unwrap();
    assert_eq!(partch.keys_per_period(), 43);
    close(freq(&partch, 103.0), 2.0 * anchor);
}

#[test]
fn mapping_overrides_reference_anchor_exactly() {
    let tuning = edo(19)
        .with_mapping(&Mapping {
            root: None,
            ref_key: Some(69),
            ref_freq: Some(440.0),
        })
        .unwrap();
    assert_eq!(freq(&tuning, 69.0), 440.0);

    let ref_key_only = edo(19)
        .with_mapping(&Mapping {
            root: None,
            ref_key: Some(69),
            ref_freq: None,
        })
        .unwrap();
    assert_eq!(
        ref_key_only.freq(69.0).unwrap(),
        Some(crate::sched::commit::note_to_freq(69.0))
    );
}

#[test]
fn inline_scala_parses_cents_and_ratios() {
    let spec = scala_spec(
        "! c\nslendro approx\n5\n240.0\n480.0\n720.0\n960.0\n2/1\n",
        None,
    )
    .unwrap();
    let tuning = Tuning::from_spec(&spec).unwrap();
    close(
        freq(&tuning, 61.0),
        crate::sched::commit::note_to_freq(60.0) * (240.0_f64 / 1200.0).exp2(),
    );
}

#[test]
fn scala_accepts_crlf_comments_and_trailing_text() {
    let spec = scala_spec("! comment\r\nslendro\r\n2 trailing count text\r\n240.0 cents text\r\n2/1 period text\r\nignored after scale\r\n", None).unwrap();
    let tuning = Tuning::from_spec(&spec).unwrap();
    close(
        freq(&tuning, 61.0),
        crate::sched::commit::note_to_freq(60.0) * (240.0_f64 / 1200.0).exp2(),
    );
}

#[test]
fn kbm_unmapped_entries_skip_notes_and_reference_must_map() {
    let scl = "test\n3\n100.0\n200.0\n2/1\n";
    let kbm = "3\n0\n127\n60\n60\n440.0\n3\n0\nx\n2\n";
    let tuning = Tuning::from_spec(&scala_spec(scl, Some(kbm)).unwrap()).unwrap();
    assert_eq!(tuning.freq(61.0).unwrap(), None);
    assert!(tuning.freq(60.0).unwrap().is_some());
    let bad = "2\n0\n127\n60\n61\n440.0\n3\n0\nx\n";
    assert_eq!(
        Tuning::from_spec(&scala_spec(scl, Some(bad)).unwrap())
            .unwrap_err()
            .code,
        FailCode::Type
    );
    let mut fields = match scala_spec(scl, Some(kbm)).unwrap() {
        Value::Dict(fields) => (*fields).clone(),
        _ => unreachable!(),
    };
    fields.insert(Key::Kw(intern_kw("root")), Value::Int(63));
    let rooted = Tuning::from_spec(&Value::dict(fields)).unwrap();
    assert_eq!(
        Tuning::from_control(&rooted.to_control()).unwrap().unwrap(),
        rooted
    );
}

#[test]
fn malformed_scala_and_spec_bounds_are_type_failures() {
    for (scl, kbm) in [
        ("scl\n2\n100.0\n", None),
        ("scl\n1\nabc\n", None),
        ("scl\n0\n", None),
        ("scl\n1\n0.0\n", None),
    ] {
        assert_eq!(
            Tuning::from_spec(&scala_spec(scl, kbm).unwrap_or(Value::Nil))
                .unwrap_err()
                .code,
            FailCode::Type
        );
    }
    assert_eq!(
        edo_spec(&Value::Int(0), None).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(
        edo_spec(&Value::Int(1201), None).unwrap_err().code,
        FailCode::Type
    );
    assert_eq!(
        edo_spec(&Value::Int(12), Some(&Value::Int(1)))
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert_eq!(
        ratios_spec(&Value::list(vec![Value::Ratio(Ratio64::from_int(-1))]))
            .unwrap_err()
            .code,
        FailCode::Type
    );
}

#[test]
fn control_round_trips_edo_degrees_and_keymap() {
    let tunings = [
        edo(19),
        Tuning::from_spec(&ratios_spec(&Value::list(vec![Value::Int(2)])).unwrap()).unwrap(),
        Tuning::from_spec(
            &scala_spec(
                "test\n3\n100.0\n200.0\n2/1\n",
                Some("2\n0\n127\n60\n60\n440.0\n3\n0\n2\n"),
            )
            .unwrap(),
        )
        .unwrap(),
    ];
    for tuning in tunings {
        assert_eq!(
            Tuning::from_control(&tuning.to_control()).unwrap().unwrap(),
            tuning
        );
    }
}

#[test]
fn control_recognition_is_narrow_and_malformed_known_lists_fail() {
    assert!(Tuning::from_control(&Value::Int(3)).is_none());
    assert!(Tuning::from_control(&Value::list(vec![Value::Int(1), Value::Int(2)])).is_none());
    assert_eq!(
        Tuning::from_control(&Value::list(vec![Value::kw("edo")]))
            .unwrap()
            .unwrap_err()
            .code,
        FailCode::Type
    );
}

#[test]
fn note_key_obeys_root_and_twelve_key_fast_path() {
    assert_eq!(edo(19).note_key("d").unwrap(), Some(63));
    let rooted = edo(19)
        .with_mapping(&Mapping {
            root: Some(62),
            ref_key: None,
            ref_freq: None,
        })
        .unwrap();
    assert_eq!(rooted.note_key("d").unwrap(), Some(62));
    assert_eq!(
        rooted.freq(62.0).unwrap(),
        Some(crate::sched::commit::note_to_freq(62.0))
    );
    let mut fields = BTreeMap::new();
    fields.insert(Key::Kw(intern_kw("kind")), Value::kw("edo"));
    fields.insert(Key::Kw(intern_kw("steps")), Value::Int(19));
    fields.insert(Key::Kw(intern_kw("period")), Value::Int(2));
    fields.insert(Key::Kw(intern_kw("root")), Value::Int(62));
    let root_spec = Value::dict(fields);
    let from_spec = Tuning::from_spec(&root_spec).unwrap();
    assert_eq!(
        from_spec.freq(62.0).unwrap(),
        Some(crate::sched::commit::note_to_freq(62.0))
    );
    assert_eq!(rooted, from_spec);
    assert_eq!(
        Tuning::from_control(&rooted.to_control()).unwrap().unwrap(),
        rooted
    );

    let mut ji_fields = match tuning_preset("ji-5").unwrap() {
        Value::Dict(fields) => (*fields).clone(),
        _ => unreachable!(),
    };
    ji_fields.insert(Key::Kw(intern_kw("root")), Value::Int(62));
    let ji_root_spec = Value::dict(ji_fields);
    let ji_from_spec = Tuning::from_spec(&ji_root_spec).unwrap();
    let ji_rooted = Tuning::from_spec(&Value::kw("ji-5"))
        .unwrap()
        .with_mapping(&Mapping {
            root: Some(62),
            ref_key: None,
            ref_freq: None,
        })
        .unwrap();
    assert_eq!(
        ji_rooted.freq(62.0).unwrap(),
        Some(crate::sched::commit::note_to_freq(62.0))
    );
    assert_eq!(ji_rooted, ji_from_spec);
    assert_eq!(
        Tuning::from_spec(&Value::kw("ji-5"))
            .unwrap()
            .note_key("d")
            .unwrap(),
        Some(62)
    );
    assert_eq!(edo(19).note_key("not-a-note").unwrap(), None);
}

#[test]
fn nearest_ties_choose_smallest_offset_and_fractional_keys_interpolate() {
    let tuning = edo(19);
    let half_step = 1200.0 / 19.0 / 2.0;
    assert_eq!(tuning.nearest(60, half_step).unwrap(), 0);
    close(
        freq(&tuning, 60.5),
        crate::sched::commit::note_to_freq(60.0) * (0.5_f64 / 19.0).exp2(),
    );
}

#[test]
fn octave_edo_uses_exact_exp2_closed_form() {
    let tuning = edo(19);
    let key = 61.5;
    let anchor = crate::sched::commit::note_to_freq(60.0);
    assert_eq!(freq(&tuning, key), anchor * ((key - 60.0) / 19.0).exp2());
}

#[test]
fn fractional_degrees_interpolate_in_log_frequency() {
    let tuning = Tuning::from_spec(&Value::kw("ji-5")).unwrap();
    let lower = freq(&tuning, 60.0);
    let upper = freq(&tuning, 61.0);
    close(
        freq(&tuning, 60.5),
        (lower.ln() + 0.5 * (upper.ln() - lower.ln())).exp(),
    );
}

#[test]
fn zero_size_kbm_maps_linearly_with_inclusive_bounds() {
    let scl = "test\n3\n100.0\n200.0\n2/1\n";
    let kbm = "0\n58\n62\n60\n60\n440.0\n0\n";
    let tuning = Tuning::from_spec(&scala_spec(scl, Some(kbm)).unwrap()).unwrap();
    assert_eq!(tuning.keys_per_period(), 3);
    assert!(tuning.freq(58.0).unwrap().is_some());
    assert!(tuning.freq(59.0).unwrap().is_some());
    assert!(tuning.freq(62.0).unwrap().is_some());
    assert_eq!(tuning.freq(57.0).unwrap(), None);
    assert_eq!(tuning.freq(63.0).unwrap(), None);
}

#[test]
fn period_size_and_preset_lookups_match_contract() {
    assert_eq!(edo(19).keys_per_period(), 19);
    assert_eq!(
        Tuning::from_spec(&Value::kw("partch-43"))
            .unwrap()
            .keys_per_period(),
        43
    );
    let kbm = "7\n0\n127\n60\n60\n440.0\n7\n0\n1\n2\n3\n4\n5\n6\n";
    let scl = "seven\n7\n100.0\n200.0\n300.0\n400.0\n500.0\n600.0\n2/1\n";
    assert_eq!(
        Tuning::from_spec(&scala_spec(scl, Some(kbm)).unwrap())
            .unwrap()
            .keys_per_period(),
        7
    );
    assert_eq!(scale_preset("maqam-rast").unwrap().size, 24);
    assert_eq!(
        scale_preset("maqam-rast").unwrap().steps,
        &[0, 4, 7, 10, 14, 18, 21]
    );
    assert!(tuning_preset("nope").is_none());
}

#[test]
fn mapping_fields_are_keyword_based() {
    let mut fields = BTreeMap::new();
    fields.insert(Key::Kw(intern_kw("kind")), Value::kw("edo"));
    fields.insert(Key::Kw(intern_kw("steps")), Value::Int(19));
    fields.insert(Key::Kw(intern_kw("period")), Value::Int(2));
    let parsed = Tuning::from_spec(&Value::dict(fields)).unwrap();
    assert_eq!(parsed.root(), 60);
}
