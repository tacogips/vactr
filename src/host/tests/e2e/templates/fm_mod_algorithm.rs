//! FM1V-40 coverage for the existing FM template's algorithm control and fm6-core.

use std::collections::HashSet;

use super::super::{all_finite, peak, rms, E2e, SOURCE};
use crate::complete::{complete, Snapshot};

fn render(source: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(source);
    let audio = e.run_for(0.5);
    assert!(e.faults.is_empty(), "render faults: {:?}", e.faults);
    audio
}

fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|sample| sample.to_bits()).collect()
}

#[test]
fn fm_mod_unset_and_zero_are_identical() {
    let unset = render("s :fm > note [:a3] > once");
    let zero = render("s :fm > algorithm 0 > note [:a3] > once");
    assert_eq!(bits(&unset), bits(&zero));
}

#[test]
fn fm_mod_algorithms_render_distinct_and_bounded() {
    let mut renders = Vec::new();
    let mut unique = HashSet::new();
    for algorithm in 1..=32 {
        let audio = render(&format!(
            "s :fm > algorithm {algorithm} > note [:a3] > once"
        ));
        assert!(all_finite(&audio), "algorithm {algorithm} is finite");
        assert!(rms(&audio) > 1.0e-3, "algorithm {algorithm} is audible");
        assert!(peak(&audio) <= 1.0, "algorithm {algorithm} is bounded");
        assert!(
            unique.insert(bits(&audio)),
            "algorithm {algorithm} is distinct"
        );
        renders.push(audio);
    }
}

#[test]
fn fm_mod_user_inst_without_algorithm_stays_legacy() {
    let legacy = render(
        "inst probe:\n\tfm-op freq ratio: 1 index: 1 > fm-mod {fm-op freq ratio: 2} > * amp\ns :probe > note [:a3] > once",
    );
    let zero = render(
        "inst probe:\n\tfm-op freq ratio: 1 index: 1 > fm-mod {fm-op freq ratio: 2} > * amp\ns :probe > algorithm 0 > note [:a3] > once",
    );
    let algorithm = render(
        "inst probe:\n\tfm-op freq ratio: 1 index: 1 > fm-mod {fm-op freq ratio: 2} > * amp\ns :probe > algorithm 5 > note [:a3] > once",
    );
    assert_eq!(bits(&legacy), bits(&zero));
    assert_ne!(bits(&legacy), bits(&algorithm));
}

fn compile_outcome(source: &str) -> String {
    let mut e = E2e::new();
    let source = format!("{source}\n");
    let outcomes =
        e.ev.eval_str(&source, SOURCE)
            .expect("source reads and expands");
    outcomes
        .last()
        .expect("one form")
        .value
        .as_ref()
        .expect_err("definition fails")
        .message
        .clone()
}

fn synthetic_patch() -> String {
    let mut values = [0_u8; 155];
    // One operator at a valid output level and a sustained synthetic envelope.
    values[105 + 16] = 99;
    values[105..109].copy_from_slice(&[99, 99, 99, 99]);
    values[109..113].copy_from_slice(&[99, 99, 99, 99]);
    values[105 + 18] = 1;
    format!(
        "[{}]",
        values
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

#[test]
fn fm6_core_macro_and_constant_patch_lower_and_render() {
    let macro_audio = render("inst m:\n\tfm6-core freq > * amp\ns :m > note [:c4] > once");
    assert!(all_finite(&macro_audio));
    assert!(rms(&macro_audio) > 1.0e-3);
    assert!(peak(&macro_audio) <= 1.0);

    let patch = synthetic_patch();
    let source = format!(
        "let p {patch}\ninst t:\n\tfm6-core freq patch: p > * amp\ns :t > note [:c4] > once",
    );
    let patched = render(&source);
    assert!(all_finite(&patched));
    assert!(rms(&patched) > 1.0e-4);
    assert!(peak(&patched) <= 1.0);

    let bank_source = format!(
        "let p {patch}\nlet bank [p p]\ninst t:\n\tfm6-core freq patch: {{bank 1}} > * amp\ns :t > note [:c4] > once",
    );
    let bank_patch = render(&bank_source);
    assert!(all_finite(&bank_patch));
    assert!(rms(&bank_patch) > 1.0e-4);
    assert!(peak(&bank_patch) <= 1.0);
}

#[test]
fn fm6_core_bad_patch_reports_expected_length_and_index() {
    let short = format!(
        "inst t:\n\tfm6-core freq patch: [{}] > * amp",
        vec!["0"; 154].join(" ")
    );
    let outcome = compile_outcome(&short);
    assert!(outcome.contains("expected 155"), "{outcome}");

    let mut values = vec!["0"; 155];
    values[105 + 16] = "100";
    let invalid = format!(
        "inst t:\n\tfm6-core freq patch: [{}] > * amp",
        values.join(" ")
    );
    let outcome = compile_outcome(&invalid);
    assert!(outcome.contains("index 121"), "{outcome}");
}

#[test]
fn fm6_core_macro_algorithms_and_completion_are_exposed() {
    let first = render("inst m:\n\tfm6-core freq algorithm: 5 > * amp\ns :m > note [:c4] > once");
    let second = render("inst m:\n\tfm6-core freq algorithm: 9 > * amp\ns :m > note [:c4] > once");
    assert_ne!(bits(&first), bits(&second));

    let snapshot = Snapshot::builtin();
    for (prefix, label) in [("fm6-co", "fm6-core"), ("fm6-sy", "fm6-sysex")] {
        let result = complete(prefix, prefix.len(), &snapshot, 100);
        assert!(
            result.items.iter().any(|item| item.label == label),
            "{label}"
        );
    }
}
