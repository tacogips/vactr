# FM1 Tuning 04: Tuning-Aware scale, chord, voicing and Microtonal Scale Presets

**Status**: Ready
**Plan ID**: fm1-tuning-04-music
**Wave**: 2 (depends on fm1-tuning-01-model; parallel with 02 and 03)
**Design Reference**: design-docs/specs/design-tuning-and-strum.md D1, D7, sections 4.7, 4.8
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

`scale`, `chord` and `voicing` must work in non-12 tunings, and the new
microtonal scale presets (`:edo19-major`, `:maqam-rast`, `:slendro`,
`:pelog`, `:bp-lambda`, ...) must be usable through `scale`. Each
combinator reads the event's `tuning` control (set earlier in the chain by
`tune`). Without a recognized tuning, the existing integer arithmetic runs
unchanged (D1). `arp` is order-only and is NOT changed.

All edits are in `src/pattern/combinators/music.rs` (442 lines):

| Symbol | Line |
| --- | --- |
| `SCALES` | :22 |
| `scale` constructor validation | :132 |
| `scale_note` | :174 |
| `scale_event` | :189 |
| `chord_notes` | :96 |
| `query_chord` | :219 |
| `voice` | :234 |
| `query_voicing` | :260 |

## Non-goals

- No change to the 17 existing scales, `CHORD_QUALITIES`, `arp`, `range`,
  or `chord_pattern_of`.
- No `PatNode`, commit, natives or docs changes.
- No change to the `scale` native in `src/vm/natives/music.rs`. It already
  passes the keyword through, and validation lives in `music::scale`.

## Working Rules

Follow the Working Rules in `fm1-tuning-01-model.md`: fresh read plus
sha256 before and after each edit, re-apply on drift, own paths only, no
git writes, no broad formatting, silent tests, no full nextest. Plans 02
and 03 run concurrently on disjoint files.

## Files

| Path | Change |
| --- | --- |
| `src/pattern/combinators/music.rs` | tuned branches; accept preset names; `#[cfg(test)] mod tuning_tests;` |
| `src/pattern/combinators/music/tuning_tests.rs` | new tests |

`music.rs` must stay under 1000 lines (target under 650).

## Required behavior (design 4.7)

Helper: `fn event_tuning(e: &Event) -> Option<Result<Tuning, Failure>>`
reads `e.controls["tuning"]` through `Tuning::from_control`. `Some(Err)` is
an event fault (through `event_fault`, as the existing code does).

1. **`scale` constructor**: accept a name if `scale_steps(name)` (legacy)
   OR `presets::scale_preset(name)` is Some. Both unknown -> the existing
   "unknown scale" error.
2. **`scale_event`**:
   - Legacy name, untuned: the existing `scale_note` call, byte-for-byte
     unchanged.
   - Otherwise determine the tuning T:
     - the event's recognized tuning, or
     - for a microtonal preset on an untuned event:
       `Tuning::edo(preset.size, period)`. Insert its `to_control()` as the
       event's `tuning` control, and only when the event has no
       recognized tuning.
   - Root key: `T.note_key(root name)` (the root keyword is always a note
     name; `None` is the "unknown scale root" error).
   - Preset size P (12 for legacy), steps, and Sc (preset period cents:
     1200, or `1200 * log2(3)` for (3,1)).
   - Degree d: `oct = d.div_euclid(len)`, `i = d.rem_euclid(len)`.
     - If `T.keys_per_period() == P`: `key = root + oct*K + steps[i]`.
     - Else: `key = root + T.nearest(root, oct*Sc + steps[i]*Sc/P)?`.
   - Write the result the same way as today: the `note` control for `n`,
     or `e.value`.
3. **`query_chord`**: keep `query_mapped`. Note that the map closure only
   sees the chord value, not the event. Implement the tuned path by
   mapping first, then post-processing in the same `query_chord`: for each
   output event with a recognized tuning, recompute the tones from the
   ORIGINAL chord value.
   - Root: a keyword root via `T.note_key`, a number used as is.
   - Each interval s: if `K == 12`, `root + s`; else
     `root + T.nearest(root, 100*s)?`.
   - The simplest correct approach is to query `pair_up` directly (as
     `query_mapped` does) so that each pair has the event. Keep the untuned
     output identical to today, including the cells handling for late
     values.
4. **`voicing`**: untuned -> the existing `voice()`. Tuned:
   `pc = (n - ROOT).rem_euclid(K)`; the first tone is `ROOT + pc`; each
   next tone is the lowest key above the previous one with its pc. Keyword
   tones are mapped with `note_key` first.
5. **`arp`**: no change.

## Pitfalls

- Bit identity: an untuned event must take exactly the current code path.
  Add branches; do not rewrite `scale_note`/`voice`/`chord_notes`.
- A microtonal preset with no `tune` sets the tuning; an existing
  recognized tuning is never overwritten.
- Keep `chord_notes` public and unchanged (other callers may use it).

## Tests (`music/tuning_tests.rs`)

Build tuned events with
`Rc::new(control(intern_kw("tuning"), konst(tuning.to_control()), subject, None))`,
using `konst` from `src/pattern/tests/mod.rs` and the combinator
`crate::pattern::combinators::control::control`. Do NOT
use `ctl(..)`: it calls `pattern_of`, which would split the list into
steps. Query with `run`.

- untuned `n [0 2 4] > scale :c :major` -> notes 60, 64, 67 (unchanged)
- an edo 19 tuning control + `scale :c :edo19-major`, degrees 0..7 -> 60, 63, 66, 68, 71, 74, 77, 79 (K == P)
- an edo 19 tuning + `scale :c :major` (P 12 != K 19) -> degree 2 (400 cents) -> 60 + nearest = 66 (round(400*19/1200)=6)
- an edo 19 tuning with root 62 + `scale :d :edo19-major` degree 0 -> 62 (DR-2)
- untuned `scale :c :maqam-rast` -> notes 60, 64, 67, 70, ... and the event gains the tuning control equal to `Tuning::edo(24, 2).to_control()`
- untuned `scale :c :bp-lambda` -> the tuning control is edo 13 period 3
- `:ji-5` tuning (K 12) + `scale :c :major` -> legacy integer keys 60, 62, 64 (direct)
- edo 19 + `chord [:c :maj]` -> [60, 66, 71] (400 -> 6, 700 -> round(11.08) = 11)
- edo 19 + `chord [:d :m]` -> root 63
- edo 19 + `chord [:c :maj] > voicing` -> close position within [60, 79)
- untuned `chord [:c :maj7] > voicing` -> identical to today's expected values (copy an existing assertion)
- `scale :c :nope` -> the constructor error is unchanged

## Verification

Evidence directory: `tmp/fm1-tuning/p04/`.

| Command | Must show |
| --- | --- |
| `CARGO_TERM_QUIET=true cargo build > tmp/fm1-tuning/p04/build.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib combinators::music pattern::tests > tmp/fm1-tuning/p04/nextest-focused.log 2>&1` | exit 0, failureCount 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib types:: reader:: > tmp/fm1-tuning/p04/nextest-spec.log 2>&1` | exit 0 (spec-fence and checker tests that use scale/chord) |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/fm1-tuning/p04/clippy.log 2>&1` | exit 0 |
| `rustfmt --edition 2021 --check src/pattern/combinators/music.rs src/pattern/combinators/music/tuning_tests.rs` | exit 0 |
| `wc -l src/pattern/combinators/music.rs` | < 1000 |

## Completion Criteria

- [ ] Tuned scale/chord/voicing per design 4.7; presets accepted; untuned paths unchanged
- [ ] All tests pass; existing pattern tests are green
- [ ] clippy, rustfmt and line limits pass; only the two files changed
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Implementation not started
