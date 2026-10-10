# Pending: Microtonal Tuning and Chord Performance Questions

Questions raised by [design-tuning-and-strum.md](../specs/design-tuning-and-strum.md)
(2026-10-10, branch `wf/fm1-tuning`). None blocks implementation; each
recommendation is followed by default until the user decides otherwise.

## TQ1: MIDI output of microtonal notes

- **Question**: How should a tuned note leave through MIDI out?
- **Options**:
  - (a) Nearest 12-TET MIDI note of the tuned frequency, no pitch bend;
    deviations up to 50 cents are lost.
  - (b) Per-note pitch bend with channel rotation (MPE style); needs a new
    `MidiEvent` variant, pitch-bend support in the native and browser MIDI
    hosts, and a bend-range convention.
- **Recommendation**: (a) now (design D6). (b) can be added later without
  changing the tuning model, because the tuned frequency is already known
  at commit.

## TQ2: Default reference pitch

- **Question**: Without `ref-key`/`ref-freq`, which key anchors a tuning?
- **Options**:
  - (a) The root key (default 60) sounds at its 12-TET pitch
    (261.6256 Hz), so switching tunings keeps the root in place.
  - (b) Scala's default: key 69 = 440 Hz, which moves the root in most
    non-12 tunings (in 19-EDO key 60 would be about 317 Hz).
- **Recommendation**: (a) (design D4). `tune .. ref-key: 69 ref-freq: 440`
  gives (b) explicitly.

## TQ3: Harp plate defaults

- **Question**: How many strips and which starting key does `harp` use by
  default?
- **Recommendation**: 12 strips starting one period below the root (key 48
  without a tuning), so a triad spans four periods; both are keyword
  arguments (`strips:`, `base:`).

## TQ4: Live retuning of held var-driven notes

- **Question**: Without a tuning, a `var`-driven `note` travels as a
  `NoteToFreq` cell, so a later `upd` retunes a held voice. Under a tuning
  the frequency is computed at commit and sent as a constant. Is that
  acceptable?
- **Recommendation**: Yes for this change. The value is still read at
  commit (the current var value); only already-sounding voices keep their
  pitch until their next event.
