# BASS-41: Techno Example Tracks, Example Render Tests and README

**Status**: Ready
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

- [ ] Four example files exist and render; 4 WAVs exist.
- [ ] Tests pass with the exit code and count recorded.
- [ ] README bullet group added; notices unchanged.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none
