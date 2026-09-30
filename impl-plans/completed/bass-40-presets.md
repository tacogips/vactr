# BASS-40: Bass Preset Library and Preset Render Tests

**Status**: Completed
**Plan ID**: BASS-40 (wave 4; parallel with BASS-41)
**Design Reference**: `design-docs/specs/design-bass-voices.md`, sections "Presets and examples" and "Verification" (offline render tests)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The user asked for named bass patches for techno at 125-140 BPM:

- squelchy and rolling acid;
- deep sub;
- hoover-ish reese;
- wobble at 1/4 and 1/8;
- FM pluck;
- digital grit.

This plan writes `examples/bass-presets.vact` with at least three patches
per template (six template families), and a committed test that renders
every patch offline to `tmp/bass/presets/<name>.wav` and checks it
objectively.

## Non-goals

- No new templates, kernel changes or registry edits.
- No README (BASS-41 owns it).
- Do not edit `src/host/tests/e2e/templates.rs`; BASS-30 already registered
  `mod bass_presets;`.

## Dependencies

- **dependsOn**: BASS-30
- **Blocks**: none (reconcile)

## writePaths

- `examples/bass-presets.vact`
- `src/host/tests/e2e/templates/bass_presets.rs`
- `impl-plans/active/bass-40-presets.md`

## writePathNotes

- path: `examples/bass-presets.vact` | intendedEdit: new file
- path: `src/host/tests/e2e/templates/bass_presets.rs` | intendedEdit: replace the BASS-30 placeholder
- path: `impl-plans/active/bass-40-presets.md` | intendedEdit: Progress Log

## sharedPaths

None.

## Read-only References

- `src/host/tests/e2e/templates/bass_render.rs`: `SR`, `write_wav`,
  `low_band_share`, `check`, `RenderCheck`.
- `src/host/tests/e2e.rs`: `E2e::new`, `eval`, `run_stereo_for`, `faults`,
  `committed`.
- `src/prelude/templates.vact`: the bass headers.

## Preset Format (key decisions)

- The file begins with a comment block:
  - how to use a patch: `acid-squelch [:c2 :c2 :eb2 :c3] > accent [0 1 0 0] > cut 1 > d1`;
  - `slide-from` semantics: semitones from the previous note, and use it
    with `cut 1`;
  - `gate-length` is in sixteenth-steps;
  - the wobble `lfo-rate` division table.

  After that comes `use-bpm 132`.
- Each patch is `fn <family>-<name> notes:` with a body that returns
  `s :<template> > note notes > <control> <value> ...`. `<family>` is one
  of `analog`, `acid`, `fm`, `wobble`, `sub`, `reese`; the family maps to
  the `<family>-bass` template.
- Required patches, at least three per family:

  | Family | Patches |
  |--------|---------|
  | `acid` | `acid-squelch`, `acid-rolling`, `acid-deep` |
  | `sub` | `sub-deep`, `sub-driven`, `sub-long` |
  | `reese` | `reese-hoover`, `reese-dark`, `reese-rumble` |
  | `wobble` | `wobble-quarter` (`lfo-rate 4`), `wobble-eighth` (8), `wobble-triplet` (6), `wobble-free` (`lfo-sync false`, `lfo-rate 3.2`) |
  | `fm` | `fm-pluck`, `fm-digital-grit` (`fold 0.6`, `bit-depth 6`), `fm-metal` (`ratio 2`, high `fm-feedback`) |
  | `analog` | `analog-pluck`, `analog-driven`, `analog-square` (`wave :square`) |

- Every patch sets `gain` so its render peak is at most 1.0.
- The file ends with commented demo lines only. There is no active `d`
  sink at top level, so evaluating the file plays nothing by itself.
- **Checkpoint first.** Write `bass_preset_fn_pipes_to_sink` before any
  presets. It evaluates `fn p notes:\n\ts :sub-bass > note notes\np [:c2] > d1`
  and asserts `committed > 0` with no faults. If the language cannot pipe a
  pattern returned by an `fn`, switch every patch to a zero-argument
  `fn <family>-<name>:` that returns a complete demo pattern, keep the
  family prefix rule, and record the switch in the Progress Log. Do not
  change the language.

## Test Behavior (`bass_presets.rs`)

- `bass_presets_cover_every_family`: collect preset names by scanning the
  file text (`include_str!("../../../../../examples/bass-presets.vact")`)
  for lines starting with `fn `. Assert:
  - at least 18 total;
  - at least 3 per family;
  - every name's prefix is a known family.
- `bass_presets_render_and_meet_level_targets`: for each preset, build a
  fresh `E2e::new()`, evaluate the file, then evaluate
  `<name> <demo phrase> > d1`. Demo phrase per family:

  | Family | Demo phrase |
  |--------|-------------|
  | acid | `[:c2 :c2 :eb2 :c3 :c2 :g1 :c2 :bb1] > accent [1 0 0 1 0 0 1 0] > slide-from [0 0 0 -9 12 0 -7 0] > cut 1` |
  | sub | `[:c1 nil :c1 :g0]` |
  | reese | `[:c1 :c1 :eb1 :c1]` |
  | wobble | `[:c1 :g0]` |
  | fm | `[:c2 :c2 :c3 :c2]` |
  | analog | `[:c2 :c2 :g1 :bb1]` |

  Render 2.0 s with `run_stereo_for` and call
  `write_wav(&format!("presets/{name}"), &l, &r)`. Assert:
  - `e.faults` is empty; `committed > 0`;
  - `check(..).finite`;
  - `rms > 1e-3`; `peak <= 1.0`.

  Low-band share:

  | Family | Minimum `low` |
  |--------|---------------|
  | `sub` | 0.8 (authoritative; never lower) |
  | `analog`, `reese`, `wobble`, `fm` | 0.5 |
  | `acid` | no minimum |

  On failure, the message includes the name and all measured values.
- Keep one test per concern. The render test may loop over presets, but it
  must report every failing preset (collect failures, then assert) rather
  than stopping at the first.

## Pitfalls

- Note names: check that `:g0`, `:bb1` and `:c1` parse in the note
  vocabulary. If octave 0 is unsupported, use `:c1`/`:g1`.
- Patterns loop. The 2 s render at 132 BPM covers about 1.1 cycles, so
  every patch sounds at least once.
- Do not use a master `limiter` to pass the peak check. Tune `gain` instead,
  because the design has no default limiter.
- If a threshold fails, tune the preset (cutoff, drive, gain) first.
  Changing the thresholds is only allowed with the measured values recorded
  in the Progress Log, and never for `sub` below 0.8.
- `tmp/` is gitignored. Never add the WAVs to git.

## Verification (evidence required)

1. `rustfmt --edition 2021 --check src/host/tests/e2e/templates/bass_presets.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_presets::/)' > tmp/logs/bass-40-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`, with at least 3 tests run.
3. `ls tmp/bass/presets/*.wav | wc -l` is at least 18.
4. Record in the Progress Log each preset's measured `rms`, `peak` and
   `low`, for the listening pass at review.

## Concurrency and Drift Protocol

- BASS-41 runs at the same time. It owns different files, so never edit
  `bass_examples.rs`, `README.md` or the example `.vact` files.
- Fresh-read before editing, and record `shasum -a 256` of owned files at
  start and finish.

## Done Criteria

- [x] At least 18 presets, with at least 3 per family.
- [x] All render assertions pass and at least 18 WAVs exist.
- [x] Only writePaths changed.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none

### Session: 2026-09-30 (BASS-40 implementation)
**Tasks Completed**: Added 19 named patches and three focused tests; rendered all 19 patches for 2 seconds to `tmp/bass/presets/`.
**Checkpoint**: `bass_preset_fn_pipes_to_sink` passes using the plan's parameterized returned-pattern syntax. `E2e::committed` increments during `run_stereo_for`, so the checkpoint ticks 0.1 seconds before asserting committed events. No zero-argument fallback was needed.
**Verification**: `rustfmt --edition 2021 --check src/host/tests/e2e/templates/bass_presets.rs` exit 0; `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --no-capture -E 'test(/bass_presets::/)'` exit 0, 3/3 tests passed; 19 WAV files present. The earlier rustfmt check failed on one line wrap before formatting; no behavior changed, and the final-source checks are rerun after formatting.
**Metrics** (rms / peak / low-band share): analog-pluck 0.072539 / 0.269405 / 0.986032; analog-driven 0.079841 / 0.212686 / 0.994952; analog-square 0.113583 / 0.242978 / 0.985831; acid-squelch 0.138512 / 0.365538 / 0.983554; acid-rolling 0.128286 / 0.382868 / 0.984333; acid-deep 0.241844 / 0.396500 / 0.986457; fm-pluck 0.038508 / 0.167749 / 0.875685; fm-digital-grit 0.035208 / 0.128111 / 0.684065; fm-metal 0.027094 / 0.103190 / 0.753174; wobble-quarter 0.089669 / 0.194657 / 0.945180; wobble-eighth 0.081466 / 0.171958 / 0.950798; wobble-triplet 0.096051 / 0.190007 / 0.942395; wobble-free 0.077216 / 0.174397 / 0.952744; sub-deep 0.124793 / 0.260407 / 0.999914; sub-driven 0.088564 / 0.204060 / 0.999111; sub-long 0.164349 / 0.267683 / 0.999996; reese-hoover 0.105741 / 0.168135 / 0.976274; reese-dark 0.132607 / 0.204131 / 0.990323; reese-rumble 0.124046 / 0.220210 / 0.997363.
**Drift evidence**: At start the test placeholder SHA256 was `b90cbef95f0861c577838ffc50553ef21b34a4c3866b6bf62531c5e017c15fe4`; `examples/bass-presets.vact` was absent. See `tmp/bass-voices-232/BASS-40/` for edit intents and complete verification logs.
**Final owned-path SHA256**: `examples/bass-presets.vact` `5ac2c9a50bc84a679ad7757ea9c94dc92bc186c6bb080bf366dfa98bf7c4d114`; `src/host/tests/e2e/templates/bass_presets.rs` `5a5faac46453013383056b3dc56cbcfbbf5504c3d4c99db5deee57aa98207eb1`; `impl-plans/active/bass-40-presets.md` `1106bb3fb08e8865155a7bac82a19659278c51ee75df5112ed809ef002e35f6a` (before this final evidence-line append). Final gates: `tmp/bass-voices-232/BASS-40/rustfmt-final.log`, `tmp/bass-voices-232/BASS-40/nextest-final.log`.

### Session: 2026-09-30 (session 232 closeout)
**Tasks Completed**: Accepted by test-integrity review, adversarial review and serial integration review (comm-003043).
**Verification**: Combined-tree reconcile gates in `tmp/bass-voices-232/reconcile/wave-7/` all exit 0 (full nextest 1697 passed, presets/examples 9/9).
**Remaining (non-blocking)**: Manual listening pass on `tmp/bass/presets/*.wav` (19 files). The Step 8 move to `impl-plans/completed/` was denied by the sandbox and is still pending.
**Status**: Completed.
