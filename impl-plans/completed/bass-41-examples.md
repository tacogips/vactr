# BASS-41: Techno Example Tracks, Example Render Tests and README

**Status**: Completed
**Plan ID**: BASS-41 (wave 4; parallel with BASS-40)
**Design Reference**: `design-docs/specs/design-bass-voices.md`, sections "Presets and examples", "Verification" (offline render tests) and "Risks" (README overlap)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The user said the techno examples lack bass. This plan adds four example
tracks that combine existing drum templates with the new bass templates.
Each track is rendered offline by a committed test to
`tmp/bass/examples/<name>.wav` and checked objectively. The plan also adds
the README listing.

## Non-goals

- Do not edit `examples/industrial-techno.vact` or any existing example.
- No new effects, templates or kernel changes.
- No master limiter to pass level checks.
- README: one contiguous bullet group only. The concurrent wf/syntax-fmt
  branch may also edit README, so keep the diff small and localized.

## Dependencies

- **dependsOn**: BASS-30
- **Blocks**: none (reconcile)

## writePaths

- `examples/acid-techno.vact`
- `examples/rumble-techno.vact`
- `examples/offbeat-bass-techno.vact`
- `examples/wobble-techno.vact`
- `src/host/tests/e2e/templates/bass_examples.rs`
- `README.md`
- `impl-plans/active/bass-41-examples.md`

## writePathNotes

- path: `examples/acid-techno.vact` | intendedEdit: new file
- path: `examples/rumble-techno.vact` | intendedEdit: new file
- path: `examples/offbeat-bass-techno.vact` | intendedEdit: new file
- path: `examples/wobble-techno.vact` | intendedEdit: new file
- path: `src/host/tests/e2e/templates/bass_examples.rs` | intendedEdit: replace the BASS-30 placeholder
- path: `README.md` | intendedEdit: one new bullet group under `## Status`, next to the existing `src/dsp/` bullet (about lines 63-84)
- path: `impl-plans/active/bass-41-examples.md` | intendedEdit: Progress Log

## sharedPaths

None.

## Read-only References

- `bass_render.rs` helpers.
- The `E2e` rig.
- `examples/industrial-techno.vact` and `examples/digital-kit.vact` for
  style: `use-bpm`, `s :template > note [...] > control value > gain g > dN`.
- `design-docs/specs/design-music.md` section 5 for effect syntax: per-event
  `> room 0.3 > size 0.6`, `> lpf 1200`, and `bus`/`master` chains.

## Example Content (key decisions)

Each file starts with a comment header: the style, the tempo, and which bass
template and techniques it shows. Each ends with `dN` sinks, like the
existing examples. Drums use existing templates only: `phase-drum` kick,
`dual-hat-voice` or `digital-kit` hats.

- `acid-techno.vact` (`use-bpm 132`):
  - four-on-the-floor kick and offbeat hats;
  - a 16-step `acid-bass` line with `accent` and `slide-from` patterns
    under `cut 1`;
  - a slowly moving `cutoff {range sine 250 900}`.
- `rumble-techno.vact` (`use-bpm 128`):
  - kick;
  - a rolling 16th `sub-bass` or `reese-bass` "rumble" (a `reese-rumble`
    style patch) with `room`/`size` and `lpf` from existing per-event
    effects to smear the tail.

  This is the rumble recipe (design user-QA BQ3: an example, not a
  template).
- `offbeat-bass-techno.vact` (`use-bpm 126`): minimal. Kick on the beats,
  `analog-bass` or `sub-bass` on the offbeats
  (`note [nil :c2 nil :c2 ...]`), a sparse hat.
- `wobble-techno.vact` (`use-bpm 136`): kick, and `wobble-bass` with long
  notes (`gate-length 16`) whose `lfo-rate` alternates per bar between 8
  (1/8) and 12 (1/8 triplet), e.g. `> lfo-rate [8 12]` over a slowed
  pattern.

Levels: set `gain` per line so the full mix peaks at 1.0 or below with no
limiter.

## Test Behavior (`bass_examples.rs`)

`bass_examples_render_and_meet_level_targets`: for each of the four files,
via `include_str!("../../../../../examples/<file>")`:

- fresh `E2e::new()`, `eval` the file;
- `run_stereo_for(seconds)`, where `seconds` is 2 cycles at the file's BPM:
  `2 * 240 / bpm`. Use a literal per file;
- `write_wav(&format!("examples/{stem}"), ..)`.

Assert:

- no faults; `committed > 0`;
- `finite`; `rms > 1e-3`; `peak <= 1.0`;
- `low >= 0.3`.

Collect all failures, then assert with a message listing each file's
measurements.

`bass_examples_use_bass_templates`: each file's text contains at least one
of the six `:<family>-bass` template names. The acid file contains
`acid-bass`, `slide-from` and `accent`. The wobble file contains
`wobble-bass` and `lfo-rate`.

## README Bullet (content)

Add a short bullet group, for example "Bass voices (design-bass-voices.md)":

- the six templates and one line each on what they are;
- slide: `slide-from` semitones plus `cut 1`;
- `gate-length` in sixteenth-steps;
- wobble divisions: `lfo-rate` 4 = 1/4, 8 = 1/8, 6 = 1/4T, 12 = 1/8T, with
  `lfo-sync false` for free rate in Hz;
- `examples/bass-presets.vact`;
- the four example files;
- how to render one: the existing `vactr run <file.vact> --cycles N` line
  in README `## Development`.

Plain ASCII, no emojis. `THIRD_PARTY_NOTICES.md` is unchanged. State that
in the Progress Log, citing the design's license boundary.

## Pitfalls

- `use-bpm` must come before patterns in each file. Pick distinct `dN`
  sinks per line.
- Do not rely on `examples/render_track.rs`; it is not on this branch.
- If `low >= 0.3` fails because hats and kick dominate, lower the hat gain
  or raise the bass first. Only relax the threshold with measurements
  recorded in the Progress Log.
- Note vocabulary: prefer octaves 1-3 for bass notes. Check that the note
  names parse.

## Verification (evidence required)

1. `rustfmt --edition 2021 --check src/host/tests/e2e/templates/bass_examples.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_examples::/)' > tmp/logs/bass-41-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`, with at least 2 tests.
3. `ls tmp/bass/examples/*.wav | wc -l` equals 4.
4. `git diff --stat README.md` shows one contiguous hunk. `git diff --stat`
   touches only writePaths.
5. Record each example's `rms`, `peak` and `low` in the Progress Log.

## Concurrency and Drift Protocol

- BASS-40 runs at the same time. Never edit `bass_presets.rs` or
  `examples/bass-presets.vact`.
- Fresh-read README right before editing it. Record its `shasum -a 256`
  before and after. If it drifted, re-read and re-insert only your bullet
  group.

## Done Criteria

- [x] Four example files exist and render; 4 WAVs exist.
- [x] Tests pass with the exit code and count recorded.
- [x] README bullet group added; notices unchanged.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none

### Session: 2026-09-30 (BASS-41 implementation)
**Tasks Completed**: Four techno examples, two focused offline-render tests, the README bass bullet group, and all plan-local verification.

- Added `examples/acid-techno.vact` (132 BPM), `examples/rumble-techno.vact` (128 BPM), `examples/offbeat-bass-techno.vact` (126 BPM), and `examples/wobble-techno.vact` (136 BPM). Existing examples were not edited; no master limiter was added.
- `bass_examples_render_and_meet_level_targets` renders each track for two cycles to `tmp/bass/examples/<stem>.wav` and checks faults, committed events, finite output, RMS > 1e-3, peak <= 1.0 and low-band share >= 0.3. `bass_examples_use_bass_templates` checks bass templates and the acid slide/accent and wobble LFO controls.
- Successful render measurements (committed; finite; RMS; peak; low-band share):
  - acid-techno (132 BPM): 48; true; 0.110741; 0.368705; 0.981867.
  - rumble-techno (128 BPM): 42; true; 0.077585; 0.322180; 0.989055.
  - offbeat-bass-techno (126 BPM): 21; true; 0.068792; 0.199399; 0.989580.
  - wobble-techno (136 BPM): 20; true; 0.084721; 0.337725; 0.967983.
- `rustfmt --edition 2021 --check src/host/tests/e2e/templates/bass_examples.rs`: exit 0 (`tmp/bass-voices-232/BASS-41/rustfmt-metrics.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_examples::/)' --no-capture`: exit 0, 2 run, 2 passed, 0 failed (`tmp/bass-voices-232/BASS-41/nextest-metrics.log`).
- WAV count: 4. `git diff -U0 README.md` has one hunk. `THIRD_PARTY_NOTICES.md` is unchanged; the accepted design's license boundary says no MIT code was adapted.
- README SHA-256: before `7dec92fa574a3da83ea1bec22c3ee36f046ab9a9d8c0c1f744e5b458e69c2c4a`; after `79301ce363f1ace424249fc97805fd6adec524b635b99882a5f0eba3192b5dce`.
- Downstream workflow steps retain formal review, serial integration gates, and any review-dependent finalization.

### Session: 2026-09-30 (test-integrity self-repair)
**Tasks Completed**: Repaired one mid-severity test-integrity finding in the wobble-techno example and its guarding test.

- Finding: `examples/wobble-techno.vact` used `lfo-rate [8 12]`. Per the design's structure rule the first list step (`note [:c1]`) defines structure and later list controls are sampled at existing onsets, so the single onset at cycle position 0 always sampled `8`; the 1/8-triplet value `12` was never played. The test only checked `wobble.contains("lfo-rate")`, so it could not detect this.
- Edit 1: `examples/wobble-techno.vact` line 9 changed `lfo-rate [8 12]` to `lfo-rate {alt 8 12}` (per-cycle alternation). The line-8 comment already read "alternate 1/8 and 1/8-triplet wobble divisions" and was left unchanged.
- Edit 2: `src/host/tests/e2e/templates/bass_examples.rs` (`bass_examples_use_bass_templates`) now asserts `wobble.contains("lfo-rate {alt 8 12}")` with message "wobble-techno alternates 1/8 and 1/8T lfo-rate per cycle"; no other assertion or the render test was changed.
- New render measurements (committed; finite; RMS; peak; low-band share):
  - acid-techno (132 BPM): 48; true; 0.110741; 0.368705; 0.981867 (unchanged).
  - rumble-techno (128 BPM): 42; true; 0.077585; 0.322180; 0.989055 (unchanged).
  - offbeat-bass-techno (126 BPM): 21; true; 0.068792; 0.199399; 0.989580 (unchanged).
  - wobble-techno (136 BPM): 20; true; 0.084804; 0.337731; 0.969096 (was 0.084721; 0.337725; 0.967983, confirming the second cycle now differs).
- `rustfmt --edition 2021 --check src/host/tests/e2e/templates/bass_examples.rs`: exit 0 (`tmp/bass-voices-232/BASS-41/test-integrity/rustfmt.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_examples::/)' --no-capture`: exit 0, 2 run, 2 passed, 0 failed (`tmp/bass-voices-232/BASS-41/test-integrity/nextest.log`).
- Intent recorded in `tmp/bass-voices-232/BASS-41/intents/test-integrity-repair.md`.

### Session: 2026-09-30 (adversarial review repair: acid-techno slide)
**Tasks Completed**: Repaired one mid-severity adversarial finding in `examples/acid-techno.vact` line 10.

- Adversarial review repair: acid-techno slide-from corrected to the design mapping note(N) - note(N+1) = +8 (eb2 -> g1) on steps 3 and 11, and gate-length 1 added on the preceding eb2 steps 2 and 10 so the slides are legato; render metrics from the rerun below.
- acid-techno (132 BPM): committed=48; finite=true; rms=0.111997; peak=0.368705; low=0.982285 (was rms=0.110741; peak=0.368705; low=0.981867).
- Other examples unchanged (rumble 0.077585 / 0.322180 / 0.989055; offbeat 0.068792 / 0.199399 / 0.989580; wobble 0.084804 / 0.337731 / 0.969096).
- `cargo nextest run -E 'test(/bass_examples::/)' --no-capture`: exit 0, 2 passed (`tmp/bass-voices-232/BASS-41/adversarial/nextest-repair.log`).
- Intent recorded in `tmp/bass-voices-232/BASS-41/adversarial/intent-acid-slide.md`.

### Session: 2026-09-30 (session 232 closeout)
**Tasks Completed**: Accepted by test-integrity review, adversarial review and serial integration review (comm-003043). The Step 8 documentation refresh reviewed the README hunk and kept it as the single bass hunk.
**Verification**: Combined-tree reconcile gates in `tmp/bass-voices-232/reconcile/wave-7/` all exit 0 (full nextest 1697 passed, presets/examples 9/9). `golden-readme-diff.log` shows 1 README hunk.
**Remaining (non-blocking)**: Manual listening pass on `tmp/bass/examples/*.wav` (4 files). The Step 8 move to `impl-plans/completed/` was denied by the sandbox and is still pending.
**Status**: Completed.
