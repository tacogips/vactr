# BASS-11: Band-limited Oscillators, Feedback FM, Folder and Bit Depth

**Status**: Ready (session 232 re-verification; implementation done in session 229)
**Plan ID**: BASS-11 (session 232 wave 2, parallel with BASS-10, after BASS-12)
**Design Reference**: `design-docs/specs/design-bass-voices.md`, sections "DSP components" (oscillators, feedback FM) and "Verification"
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan provides the kernel's sound sources:

- polyBLEP saw, pulse and square; a polyBLAMP triangle; an analytic sine;
- the attack "click" pitch blip;
- 2-operator FM whose modulator feeds back on itself;
- a sine wavefolder with first-order ADAA;
- a bit-depth quantizer.

`src/dsp/ugen/osc.rs` has a private `blep`. Do not change `osc.rs`. Write
local helpers instead.

## License Boundary

Write the code from:

- Valimaki/Huovilainen 2007 and Valimaki/Pekonen/Nam 2012 (polyBLEP);
- Esqueda/Valimaki/Bilbao 2016 (polyBLAMP);
- Chowning 1973 and Tomisawa US 4,249,447 (feedback FM);
- Parker/Zavalishin/Le Bivic 2016 and Bilbao et al. 2017 (ADAA);
- Esqueda et al. 2017 (folding).

No source code is consulted. Cite the papers in the module docs.

## Non-goals

- No oversampling, no wavetables, no new registry entries.
- No detune or sub logic beyond the helpers below; BASS-20 composes them.
- Do not edit files outside writePaths.

## Dependencies

- **dependsOn**: BASS-00
- **Blocks**: BASS-20

## writePaths

- `src/dsp/ugen/bass_voice/osc.rs`
- `src/dsp/ugen/bass_voice/fm.rs`
- `src/dsp/tests/dsp/bass_sources.rs`
- `impl-plans/active/bass-11-sources.md`

## writePathNotes

- path: `impl-plans/active/bass-11-sources.md` | intendedEdit: Progress Log only

## sharedPaths

None.

## Read-only References

- `bass_voice::sanitize`.
- `crate::dsp::offline::spectrum`, which averages Hann-window FFT magnitudes
  with `n = 2 * bins` rounded up to a power of two, so bin `k` is at
  `k * sr / n` Hz.
- `crate::dsp::effects::prim::Rng`.

## Interface Contract (pin exactly)

In `osc.rs`:

- `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Wave { Saw, Pulse, Square, Tri, Sine }`.
  - `pub fn from_index(v: f32) -> Wave` rounds and clamps to 0..=4 and maps
    non-finite values to `Saw`.
  - The order matches the `WAVES` enum in `src/dsp/controls.rs`.
- `pub const PULSE_WIDTH: f32 = 0.3;`
- `#[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct Osc` holds
  one phase in `[0, 1)`.
  - `FLOATS = 1`, plus `load`/`store`.
  - `pub fn with_phase(p: f32) -> Self` wraps `p` into `[0, 1)`; non-finite
    becomes 0.
  - `pub fn next(&mut self, wave: Wave, inc: f32) -> f32`. `inc` is
    `freq / sr`, clamped internally to `[0, 0.25]`. It returns the sample
    at the current phase and then advances the phase.
- `pub fn click_semis(t_seconds: f32, level: f32) -> f32` is
  `level * CLICK_SEMIS * exp(-t / CLICK_TAU)`, with `CLICK_SEMIS = 24.0` and
  `CLICK_TAU = 0.0015`. When `level <= 0` or `t` is non-finite it returns
  exactly `0.0`.
- `pub fn detune_ratios(detune: f32) -> (f32, f32)` returns
  `(2^(-d/24), 2^(d/24))` with `d = clamp(detune, 0, 1)`.

In `fm.rs`:

- `#[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct FmPair`
  holds the modulator phase, carrier phase and the last two modulator
  outputs.
  - `FLOATS = 4`, plus `load`/`store`.
  - `pub fn next(&mut self, car_inc: f32, ratio: f32, index: f32, feedback: f32) -> f32`.
  - `pub const FB_MAX: f32 = 1.5;`
  - Modulator: `sin(2*pi*mod_phase + FB_MAX * feedback * (y1 + y2) / 2)`.
  - Carrier: `sin(2*pi*car_phase + index * modulator)`.
  - `ratio` is clamped to `[0, 32]`, `index` to `[0, 32]`, `feedback` to
    `[0, 1]`.
- `#[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct Folder`
  holds the previous input.
  - `FLOATS = 1`, plus `load`/`store`.
  - `pub fn process(&mut self, x: f32, fold: f32) -> f32`.
  - When `fold <= 0`: return `x` bit-exactly, but still store `x` as the
    previous input.
  - Otherwise use first-order ADAA of `f(x) = sin(g * x * pi / 2)`, with
    `g = 1 + FOLD_GAIN * clamp(fold, 0, 1)` and `FOLD_GAIN` about 4. When
    `|x - x1| < 1e-5`, fall back to `f((x + x1) / 2)`.
- `pub fn quantize(x: f32, bits: f32) -> f32`:
  - `bits >= 16`, or non-finite: return `x` bit-exactly;
  - otherwise `b = round(clamp(bits, 2, 16))`, `L = 2^(b - 1)`, and return
    `round(x * L) / L`.

## Pitfalls

- polyBLEP corrections apply at the saw wrap and at both pulse and square
  edges. polyBLAMP corrections apply at the triangle's two corners (phase 0
  and 0.5), scaled by the slope change.
- Triangle and sine start at phase 0 with no DC offset. Keep the waves
  bipolar in about `[-1, 1]`.
- The FM feedback must use the average of the **previous two** modulator
  outputs. Update `y2 = y1; y1 = m` after computing `m`.
- The ADAA divide must never hit 0/0. Check the fallback threshold before
  dividing.
- Keep the phases wrapped with `rem_euclid` so long renders do not lose
  precision.

## Tests (`src/dsp/tests/dsp/bass_sources.rs`)

- For each `Wave`, 110 Hz for 1 s at 48 kHz:
  - all finite; `peak <= 1.2`;
  - `|mean| < 0.02`;
  - the upward zero-crossing period is `48000/110` within 1 sample
    (averaged over at least 50 periods).
- Aliasing: saw and triangle at 2950 Hz for 1 s, `spectrum` with 4096 bins.
  The inharmonic power share (bins more than 2 bins from every harmonic
  `k * 2950` below 24 kHz), divided by total power, is at most 0.5 times
  that of a naive saw or triangle generated in the test.
- `Wave::from_index`:
  - 0.0 to Saw, 3.6 to Sine, 9.0 to Sine;
  - -1.0 to Saw, NaN to Saw.
- `click_semis`:
  - `(0.0, 1.0) == 24.0`;
  - `(0.005, 1.0) < 1.0`;
  - level 0 gives exactly `0.0` for t in {0, 0.001, 1}.
- `FmPair` with index 0 and feedback 0 matches `sin(2*pi*phase)` within
  1e-5 over 1000 samples.
- `FmPair` with ratio 1 and index 2: the spectral centroid with feedback 0.8
  is greater than with feedback 0.
- `FmPair` with index 32, feedback 1, ratio 7 for 2 s: all finite, and
  `|y| <= 1.0 + 1e-6`.
- `Folder` with fold 0: 10k random samples pass through bit-identical.
- `Folder` with fold 1 on a 100 Hz sine at amplitude 1: finite,
  `|y| <= 1`, and a higher centroid than the input.
- `quantize`:
  - bits 16 and bits 20 are bit-identical to the input;
  - with bits 3, every output is a multiple of 0.25.
- `load(store(x))` round-trips for `Osc`, `FmPair` and `Folder`.

## Verification (evidence required)

1. `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/osc.rs src/dsp/ugen/bass_voice/fm.rs src/dsp/tests/dsp/bass_sources.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_sources::/)' > tmp/logs/bass-11-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`, with at least 10 tests and 0 failed.

## Concurrency and Drift Protocol

- Same as BASS-10: owned files only, and fresh reads.
- Record `shasum -a 256` at start and finish.
- A compile failure in a sibling's file is waited out and recorded, never
  edited.

## Done Criteria

- [x] Interface contract exact.
- [ ] Verification passes, with the exit code and test count recorded.
- [ ] `git diff --stat` touches only writePaths.

## Session 232 Re-verification (dispatch task)

**Intent.** BASS-11 was implemented in session 229, with 15 tests in
`bass_sources.rs` and `cargo check --lib` exit 0. Its focused nextest never
ran, because the sibling test file had compile errors. The ambiguous
`abs_diff` in `bass_mods.rs` is now fixed, and BASS-12 runs first in
session 232. This task runs the focused gate for the first time and
records the integrity table.

**Order.** This task runs after BASS-12. If cargo fails in a file this
plan does not own, follow the manifest retry policy (wait about 60 s, up
to 10 times) and record the failure. Never edit that file.

**Non-goals.**

- Do not rewrite the oscillators or FM.
- Do not touch `src/dsp/ugen/osc.rs`, which is the existing crate
  oscillator and not this plan's file.
- Do not change any threshold in "Tests".

**Steps.**

1. Record `shasum -a 256` of the three owned files.
2. Run the two manifest commands, writing logs to
   `tmp/bass-voices-232/BASS-11/`.
3. If a `bass_sources` test fails, find the concrete defect in
   `bass_voice/osc.rs` or `bass_voice/fm.rs` and fix it against the pinned
   contract. A defect in the test itself may be fixed only when the test
   contradicts the plan text. Record the reasoning, then re-run.
4. Write a test-integrity table in the Progress Log, one row per bullet
   in "Tests", with these columns:
   `spec bullet | test fn name | plan threshold | threshold in code | result`.
5. Record the end-of-run hashes, the exit codes, the test count and the
   log paths.

**Done criteria (session 232).**

- [ ] The rustfmt log ends with `exit=0`.
- [ ] The nextest log ends with `exit=0` and reports at least 10 tests run
      and 0 failed.
- [ ] The integrity table covers every "Tests" bullet, with no loosened
      threshold.
- [ ] `git diff --stat -- src/dsp/ugen/osc.rs` is empty.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none

### Session: 2026-09-30 (BASS-11 implementation)
**Tasks Completed**: Oscillator, feedback FM, folder, quantizer, and 15 focused behavioral tests implemented in the owned files.
**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/osc.rs src/dsp/ugen/bass_voice/fm.rs src/dsp/tests/dsp/bass_sources.rs` passed (exit 0; `tmp/bass-voices-229/BASS-11/rustfmt.log`). `CARGO_TERM_QUIET=true cargo check --lib` passed (exit 0; `tmp/bass-voices-229/BASS-11/check-lib.log`). Two focused nextest attempts did not reach test execution: attempt 1 exposed compile errors in BASS-10/BASS-12 files; attempt 2 still fails before tests on the BASS-12-owned ambiguous integer type in `src/dsp/tests/dsp/bass_mods.rs:183` (exit 101; `tmp/bass-voices-229/BASS-11/nextest-attempt-02.log`). No sibling files were edited.
**Status**: In Progress; rerun the positive-count BASS-11 nextest gate after sibling compilation blockers are resolved.
