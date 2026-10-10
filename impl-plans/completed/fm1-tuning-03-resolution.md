# FM1 Tuning 03: Frequency Resolution at Commit, MIDI, Song Freeze and Song Encode

**Status**: Completed
**Plan ID**: fm1-tuning-03-resolution
**Wave**: 2 (depends on fm1-tuning-01-model; parallel with 02 and 04)
**Design Reference**: design-docs/specs/design-tuning-and-strum.md D1, D5, D6, sections 4.6, 4.7 (note names), 4.9
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

When an event carries a recognized `tuning` control (the canonical list
from plan 01), its notes must sound at the tuned frequency. This plan makes
the four scheduler-side sites honor it:

- audio commit
- MIDI commit
- song freezing
- song encode

Keyword notes map through `Tuning::note_key` (design 4.7, review DR-1 and
DR-2). Without a recognized tuning, every line of existing behavior stays
bit-identical (D1).

Current code:

- `src/sched/commit.rs`: `note_to_freq` :43; `note_of` :49; `notes()` :440;
  `audio_events` freq loop :612-:633; generic control loop
  (`declared_param` error "unknown instrument control"); `midi_events` :691.
- `src/song/source.rs:176-182` freezes a keyword note as
  `ResolvedNote::Int(note_number(..))`.
- `src/sched/song/encode.rs`: the control loop :86 and the `row.note` freq
  push :204-:213.

## Non-goals

- Do not change `note_to_freq`, `note_of`'s untuned behavior, `CellMap`,
  `cells.rs`, `MidiEvent`, the MIDI hosts, or OSC.
- No `dsp::controls` row for `tuning`. No `PatNode`, natives or docs.
- No audio-thread code. Everything here runs on the scheduler side.

## Working Rules

Follow the Working Rules in `fm1-tuning-01-model.md`: fresh read plus
sha256 before and after each edit, re-apply on drift, own paths only, no
git writes, no broad formatting, silent tests, no full nextest. Plans 02
and 04 run concurrently on disjoint files.

## Files

| Path | Change |
| --- | --- |
| `src/sched/commit.rs` | tuned branch in `notes`/`audio_events`/`midi_events`; skip recognized `tuning` in the generic loop (873 lines; keep it < 1000) |
| `src/sched/song/encode.rs` | skip recognized frozen `tuning`; tuned `row.note` freq |
| `src/song/source.rs` | keyword note under a recognized tuning -> `note_key` before `ResolvedNote::Int`; skip unmapped tuned notes at freeze (985 lines; keep it < 1000) |
| `src/sched/tests/sched.rs` | add `mod tuning;` to the module list |
| `src/sched/tests/sched/tuning.rs` | new tests |

If `commit.rs` would reach 950+ lines, move the helpers into a new
`src/sched/commit/tuned.rs` (a sibling of `commit/timestamps.rs`). That
path is pre-declared in writePaths.

## Required behavior

1. **Recognition**: `crate::pattern::tuning::Tuning::from_control(&value)`
   on the event's `tuning` control (in commit, via `entry(controls, "tuning").map(current)`).
   - `None` (no control, or not a canonical list): today's path, untouched.
   - `Some(Err(f))`: the event fails with `f` (event-local, like other
     commit failures).
2. **Generic control loop** (audio): if the control name is `tuning` and it
   is recognized, `continue` before the `controls::row` lookup. Place it
   next to the existing `speed-fit` skip. Without that skip, a recognized
   tuning reaches `declared_param` and fails as "unknown instrument
   control". An unrecognized `tuning` value keeps today's handling.
3. **Keyword tones**: under a recognized tuning, a `Value::Keyword` tone
   (scalar or chord list item) becomes `note_key(name)`; `Ok(None)` (not a
   note name) drops that tone exactly like today's `filter_map(note_of)`.
   Numbers are keys as written (fractional allowed).
   - Add a tuned variant of `notes()`, e.g.
     `fn tuned_notes(controls, bank, &Tuning) -> Result<Option<(Vec<f64>, Option<VarSlotRef>)>, Failure>`.
   - Do NOT change `note_of`; `cells.rs:encode_value` uses it.
4. **Audio freq**: per tone, `t.freq(n)?`; `Ok(None)` (unmapped) skips the
   tone without a failure. Cast with `as f32` like today. Under a tuning,
   push `Ctl::Const(hz)` even when `late` is `Some` (no
   `CellMap::NoteToFreq` cell; design TQ4). An explicit `freq` control
   still suppresses notes (`has_freq`), unchanged.
5. **MIDI**: untuned -> unchanged `n.round().clamp(0.0, 127.0) as u8`.
   Tuned: map keywords as in rule 3; `f = t.freq(n)?`; skip unmapped tones;
   note = `(69.0 + 12.0 * (f / 440.0).log2()).round().clamp(0.0, 127.0) as u8`.
   The default note (`vec![60.0]` when there is no note) keeps today's
   behavior.
   - `midi_events` currently returns `Vec`. If it must become `Result`,
     adapt only its call site in `commit_inner`.
6. **Song freeze** (`src/song/source.rs` `expand_event`): before the
   per-tone loop, decode `event.controls.get(intern_kw("tuning"))` with
   `from_control` once. In the `Value::Keyword` arm use
   `t.note_key(name)` when tuned (`Ok(None)` -> the existing
   "invalid song note name" failure), else `note_number` (unchanged).
   Numbers unchanged. After resolving a note, only when tuning is
   recognized, evaluate `t.freq(note.to_f64())?`; `Ok(None)` skips the tone
   at freeze with no row and no fault, matching live commit. Untuned freeze
   remains unchanged.
7. **Song encode**: convert the frozen `tuning` control
   (`FrozenControl::List`) back to a `Value` (extend the local `scalar`
   helper with a recursive list conversion used only for this key) and
   call `from_control`. If recognized, `continue` in the control loop for
   that key, and push `t.freq(note.to_f64())?` for `row.note`
   (`Ok(None)` -> push no freq as a defensive fallback; freeze normally
   drops unmapped tuned tones). If unrecognized, leave today's code path
   alone.

## Pitfalls

- Bit identity: do not reorder or refactor the untuned code. Add the tuned
  path as an `if let Some(t)` branch beside it.
- Do not decode the tuning once per tone; decode once per event.
- Do not use `pattern_of` to build test controls: a list would split into
  steps. Use `crate::sched::tests::sched::with_ctl` (it uses `pure`).

## Tests (`src/sched/tests/sched/tuning.rs`)

Use `Rig`, `pat_of`, `with_ctl`, `cval`, `rig.sent()`, `rig.midi_calls()`
from `src/sched/tests/sched.rs`. Build the control with
`Tuning::edo(19, Ratio64::from_int(2)).to_control()` (or `from_spec` of a
preset).

- untuned `s :analog > note [60 60.5 :d 69]` -> freq ctl bits equal `note_to_freq(n) as f32` for 60, 60.5, 62, 69 (bit equality)
- tuned edo 19: `note 66` -> f32 equals `(note_to_freq(60.0) * (6.0/19.0).exp2()) as f32` within 1 ulp
- tuned edo 19: bare `note :d` -> key 63, freq = `note_to_freq(60) * 2^(3/19)` (DR-1, audio path)
- tuned edo 19 with root 62 mapping: `note :d` -> freq = `note_to_freq(62)` (DR-2)
- tuned `:ji-5`: chord list `[60 64 67]` -> three events at ref, ref*5/4, ref*3/2
- tuned scl with an `x` key -> that tone produces no event and no fault
- tuned + a var-driven note -> the freq ctl is `Ctl::Const` (not a cell)
- MIDI route `s {midi 1}` with an edo 19 tuning, `note :d` -> the midi note is the nearest 12-TET note of `note_to_freq(60)*2^(3/19)`, which is 62; untuned `note :d` -> 62; untuned `note 60.6` -> 61
- an unrecognized `tuning` value (Int 3) on an instrument without that param -> today's "unknown instrument control" failure (unchanged)
- a recognized but malformed list `[:edo]` -> event-local fault

The song freeze and encode paths are verified end to end in plan 05
(`tests/song_tuning.rs`), because that needs the `tune` native. Here, the
song edits must compile, and the existing song suites must stay green.

## Verification

Evidence directory: `tmp/fm1-tuning/p03/`.

| Command | Must show |
| --- | --- |
| `CARGO_TERM_QUIET=true cargo build > tmp/fm1-tuning/p03/build.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib sched::tests > tmp/fm1-tuning/p03/nextest-sched.log 2>&1` | exit 0, failureCount 0 (new and existing scheduler tests) |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib song:: > tmp/fm1-tuning/p03/nextest-song-lib.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_end_to_end --test song_issued_events --test song_frozen_cells > tmp/fm1-tuning/p03/nextest-song-int.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/fm1-tuning/p03/clippy.log 2>&1` | exit 0 |
| `rustfmt --edition 2021 --check src/sched/commit.rs src/sched/song/encode.rs src/song/source.rs src/sched/tests/sched.rs src/sched/tests/sched/tuning.rs` | exit 0 |
| `git diff --exit-code src/host/tests/e2e/templates/golden_digests.txt` | exit 0 |
| `wc -l src/sched/commit.rs src/song/source.rs src/sched/song/encode.rs` | each < 1000 |

## Completion Criteria

- [x] All four sites are tuned under a recognized tuning; the untuned code is unchanged
- [x] The listed tests pass; the existing scheduler and song suites are green
- [x] Golden digests are untouched; clippy, rustfmt and line limits pass
- [x] Progress log updated

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Implementation not started

### Session: 2026-10-10 implementation
**Tasks Completed**: Audio commit, MIDI commit, song freeze and song encode tuning resolution; nine scheduler tuning tests; final verification
**Notes**: Added recognized-control handling while retaining the untuned note and MIDI paths; moved commit tuning helpers to `src/sched/commit/tuned.rs` to keep `commit.rs` below 950 lines. Final-source logs: `tmp/fm1-tuning/p03/build-final.log`, `nextest-sched-final.log` (89 passed), `nextest-song-lib-final.log` (278 passed), `nextest-song-int-final.log` (13 passed), `clippy-final.log`, `rustfmt-final.log`, `golden-digests.log` and `wc-lines-final.log` (913/980/262). Earlier test setup failures and a strict-clippy type-complexity finding were corrected and superseded by final passing runs. Plan 05 owns song tuning end-to-end assertions; formal integrity/adversarial/combined-tree review and later workflow finalization remain downstream.

### Session: 2026-10-10 P03-TI-1 repair
**Tasks Completed**: Added a discriminating tuned-MIDI regression assertion for 19-EDO key 66 -> nearest 12-TET MIDI note 64; preserved tuned/untuned `:d` and untuned `60.6` assertions.
**Notes**: `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib sched::tests` passed (89/89, exit 0; `tmp/fm1-tuning/p03/nextest-sched-ti1.log`); `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` passed (exit 0; empty `clippy-ti1.log`); `rustfmt --edition 2021 --check src/sched/tests/sched/tuning.rs` passed (exit 0; empty `rustfmt-ti1.log`). The correction awaits independent review.

### Session: 2026-10-10 P03-ADV-1 repair
**Tasks Completed**: `expand_event` now drops an unmapped numeric tone at song freeze only when a recognized tuning is present, matching live commit without changing the untuned path. Kept song encode's no-frequency branch as a defensive fallback.
**Notes**: Current-source verification passed: build (`build-adv1.log`); scheduler tests 89/89 (`nextest-sched-adv1.log`); song library tests 278/278 (`nextest-song-lib-adv1.log`); song integration tests 13/13 (`nextest-song-int-adv1.log`); clippy (`clippy-adv1.log`); rustfmt (`rustfmt-adv1.log`); unchanged golden digests (`golden-digests-adv1.log`); line counts 913/985/262 (`wc-lines-adv1.log`). The end-to-end unmapped-key song assertion belongs to plan 05 `tests/song_tuning.rs`; this is recorded for the integration reviewer, and no plan-05 files were edited.
