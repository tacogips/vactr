//! The chord-quality table (design 7.1.4, widened 2026-09-27).

use crate::pattern::combinators::music::chord_notes;
use crate::types::chords::{chord_intervals, is_chord_quality, CHORD_QUALITIES};
use crate::value::intern::intern_kw;
use crate::value::value::Value;

fn chord(root: &str, quality: &str) -> Vec<i64> {
    let v = Value::list(vec![
        Value::Keyword(intern_kw(root)),
        Value::Keyword(intern_kw(quality)),
    ]);
    chord_notes(&v).expect("a known chord")
}

#[test]
fn names_are_unique_letter_first_keywords() {
    let mut seen = std::collections::BTreeSet::new();
    for (name, intervals) in CHORD_QUALITIES {
        assert!(seen.insert(*name), "duplicate quality {name}");
        // The Decided identifier rule: a letter first, then letters/digits,
        // hyphen-separated segments (never `:7`).
        let mut chars = name.chars();
        assert!(
            chars.next().is_some_and(|c| c.is_ascii_lowercase()),
            "{name}"
        );
        assert!(
            name.split('-')
                .all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric())),
            "{name}"
        );
        // Root first, ascending, within two octaves.
        assert_eq!(intervals.first(), Some(&0), "{name}");
        assert!(intervals.windows(2).all(|w| w[0] < w[1]), "{name}");
        assert!(intervals.iter().all(|i| (0..=24).contains(i)), "{name}");
    }
    assert!(CHORD_QUALITIES.len() >= 45);
}

#[test]
fn covers_the_common_families() {
    for q in [
        "maj", "min", "m", "dim", "aug", "sus2", "sus4", "five", "six", "m6", "six9", "m69",
        "maj7", "dom7", "m7", "mmaj7", "m7f5", "dim7", "aug7", "augmaj7", "m7s5", "dom7f5",
        "dom7sus2", "dom7sus4", "add9", "madd9", "maj9", "dom9", "m9", "mmaj9", "dom7f9", "dom7s9",
        "m7f9", "dom9sus4", "dom9s5", "add11", "maj11", "dom11", "m11", "dom7s11", "maj7s11",
        "add13", "maj13", "dom13", "m13", "major", "minor",
    ] {
        assert!(is_chord_quality(q), "{q}");
    }
    assert!(!is_chord_quality("7"));
    assert!(!is_chord_quality("maj99"));
    assert_eq!(chord_intervals("major"), chord_intervals("maj"));
    assert_eq!(chord_intervals("minor"), chord_intervals("m"));
}

#[test]
fn runtime_expands_to_the_expected_notes() {
    // c5 is note 60 (middle C).
    assert_eq!(chord("c5", "maj"), [60, 64, 67]);
    assert_eq!(chord("a4", "m"), [57, 60, 64]);
    assert_eq!(chord("g4", "dom7"), [55, 59, 62, 65]);
    assert_eq!(chord("b4", "m7f5"), [59, 62, 65, 69]);
    assert_eq!(chord("c5", "dim7"), [60, 63, 66, 69]);
    assert_eq!(chord("f4", "maj9"), [53, 57, 60, 64, 67]);
    assert_eq!(chord("g4", "dom13"), [55, 59, 62, 65, 69, 76]);
    assert_eq!(chord("d5", "sus2"), [62, 64, 69]);
    assert_eq!(chord("e4", "dom7s9"), [52, 56, 59, 62, 67]);
    assert_eq!(chord("c5", "five"), [60, 67]);
}
