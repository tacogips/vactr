//! Chord qualities (design 7.1.4; Decided 2026-09-25: letter-first keywords,
//! never `:7`; widened 2026-09-27 to the full common vocabulary).
//!
//! One table serves the checker (`unknown-keyword` for a quality that is not
//! here) and the runtime (`chord` and `arp` expand a `[root quality]` pair
//! into these intervals). Intervals are semitones above the root, ascending.
//! Naming: `m` = minor, `maj` = major seventh family, `dom` = dominant
//! seventh family, `s` = sharp, `f` = flat, `add` = added tone without the
//! seventh, `sus` = suspended; long aliases (`major`, `minor`, …) are rows too.

/// `(name, intervals)`; adding a quality means adding a row.
pub const CHORD_QUALITIES: &[(&str, &[i64])] = &[
    // triads and dyads
    ("maj", &[0, 4, 7]),
    ("major", &[0, 4, 7]),
    ("min", &[0, 3, 7]),
    ("minor", &[0, 3, 7]),
    ("m", &[0, 3, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("five", &[0, 7]),
    // sixths
    ("six", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("six9", &[0, 4, 7, 9, 14]),
    ("m69", &[0, 3, 7, 9, 14]),
    // sevenths
    ("maj7", &[0, 4, 7, 11]),
    ("dom7", &[0, 4, 7, 10]),
    ("m7", &[0, 3, 7, 10]),
    ("mmaj7", &[0, 3, 7, 11]),
    ("m7f5", &[0, 3, 6, 10]),
    ("dim7", &[0, 3, 6, 9]),
    ("aug7", &[0, 4, 8, 10]),
    ("augmaj7", &[0, 4, 8, 11]),
    ("m7s5", &[0, 3, 8, 10]),
    ("dom7f5", &[0, 4, 6, 10]),
    ("dom7sus2", &[0, 2, 7, 10]),
    ("dom7sus4", &[0, 5, 7, 10]),
    // ninths and added tones
    ("add9", &[0, 4, 7, 14]),
    ("madd9", &[0, 3, 7, 14]),
    ("maj9", &[0, 4, 7, 11, 14]),
    ("dom9", &[0, 4, 7, 10, 14]),
    ("m9", &[0, 3, 7, 10, 14]),
    ("mmaj9", &[0, 3, 7, 11, 14]),
    ("dom7f9", &[0, 4, 7, 10, 13]),
    ("dom7s9", &[0, 4, 7, 10, 15]),
    ("m7f9", &[0, 3, 7, 10, 13]),
    ("dom9sus4", &[0, 5, 7, 10, 14]),
    ("dom9s5", &[0, 4, 8, 10, 14]),
    // elevenths
    ("add11", &[0, 4, 7, 17]),
    ("maj11", &[0, 4, 7, 11, 14, 17]),
    ("dom11", &[0, 4, 7, 10, 14, 17]),
    ("m11", &[0, 3, 7, 10, 14, 17]),
    ("dom7s11", &[0, 4, 7, 10, 18]),
    ("maj7s11", &[0, 4, 7, 11, 18]),
    // thirteenths
    ("add13", &[0, 4, 7, 21]),
    ("maj13", &[0, 4, 7, 11, 14, 21]),
    ("dom13", &[0, 4, 7, 10, 14, 21]),
    ("m13", &[0, 3, 7, 10, 14, 21]),
];

/// The intervals of `name`, or `None` for an unknown quality.
#[must_use]
pub fn chord_intervals(name: &str) -> Option<&'static [i64]> {
    CHORD_QUALITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, i)| *i)
}

/// Whether `name` is a chord quality.
#[must_use]
pub fn is_chord_quality(name: &str) -> bool {
    chord_intervals(name).is_some()
}
