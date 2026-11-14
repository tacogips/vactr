# BASS-11: Band-limited Oscillators, Feedback FM, Folder and Bit Depth

**Status**: Completed (session 232; accepted by fanout review and integration review comm-003043; ready to archive to `impl-plans/completed/`)
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
  `k * 2950` below 24 kHz), divided by total power, is compared with a naive
  saw or triangle generated in the test. The saw share is at most 0.5 times
  the naive share. For the triangle, subtract the same-measurement additive
  band-limited reference share from both values, then require the oscillator's
  excess share to be at most 0.5 times the naive triangle's excess share.
  This keeps the 0.5 alias-reduction requirement above the Hann-window
  leakage floor.
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
- [x] Verification passes, with the exit code and test count recorded.
      (15/15, exit 0, `tmp/bass-voices-232/BASS-11/retry-02/nextest-final.log`.)
- [x] `git diff --stat` touches only writePaths.
      (`osc.rs` and `filter.rs` diffs are empty;
      `tmp/bass-voices-232/reconcile/wave-7/protected-paths-diff.txt` is empty.)

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

- [x] The rustfmt log ends with `exit=0` (`tmp/bass-voices-232/BASS-11/retry-02/rustfmt-final.log`).
- [x] The nextest log ends with `exit=0` and reports at least 10 tests run
      and 0 failed (`tmp/bass-voices-232/BASS-11/retry-02/nextest-final.log`).
- [x] The integrity table covers every "Tests" bullet, with no loosened
      threshold (see Session 232 table below).
- [x] `git diff --stat -- src/dsp/ugen/osc.rs` is empty.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none

### Session: 2026-09-30 (BASS-11 implementation)
**Tasks Completed**: Oscillator, feedback FM, folder, quantizer, and 15 focused behavioral tests implemented in the owned files.
**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/osc.rs src/dsp/ugen/bass_voice/fm.rs src/dsp/tests/dsp/bass_sources.rs` passed (exit 0; `tmp/bass-voices-229/BASS-11/rustfmt.log`). `CARGO_TERM_QUIET=true cargo check --lib` passed (exit 0; `tmp/bass-voices-229/BASS-11/check-lib.log`). Two focused nextest attempts did not reach test execution: attempt 1 exposed compile errors in BASS-10/BASS-12 files; attempt 2 still fails before tests on the BASS-12-owned ambiguous integer type in `src/dsp/tests/dsp/bass_mods.rs:183` (exit 101; `tmp/bass-voices-229/BASS-11/nextest-attempt-02.log`). No sibling files were edited.
**Status**: In Progress; rerun the positive-count BASS-11 nextest gate after sibling compilation blockers are resolved.

### Session: 2026-09-30 (BASS-11 re-verification and repairs)
**Tasks Completed**: Corrected the fixed-width pulse to have zero DC while retaining its 0.3 duty cycle and bounded peak. Updated the zero-index FM test oracle to follow the component's f32 phase accumulator rather than a closed-form multiply; retained the 1e-5 tolerance and 1000 samples. Kept the naive oscillator comparator on the same f32 phase trajectory as the tested oscillator. The triangle alias requirement remains unmet after scoped attempts; its threshold was not changed.
**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/osc.rs src/dsp/ugen/bass_voice/fm.rs src/dsp/tests/dsp/bass_sources.rs` passed (exit 0; `tmp/bass-voices-232/BASS-11/agent-rustfmt-final.log`). `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --no-fail-fast -E 'test(/bass_sources::/)'` failed (exit 100; 15 run, 14 passed, 1 failed, 1658 skipped; `tmp/bass-voices-232/BASS-11/agent-nextest-final.log`). The remaining failure is `bass_sources_polyblamp_triangle_reduces_aliasing_against_naive_triangle`: share 0.0011326671 vs 0.0012024496, while the plan requires <= 0.5 of naive (<= 0.0006012248). Earlier complete attempts are preserved in `nextest-final.log`, `nextest-after-repair.log`, `nextest-polyblamp.log`, `nextest-polyblamp-polarity.log`, `nextest-matched-phase.log`, and `nextest-support-two.log`; no threshold was relaxed. `git diff --stat -- src/dsp/ugen/osc.rs` is empty. Final hashes: `osc.rs` ea9ebb1af00da2be49308ee815e01ad033675a2086f8090313c7f4e28e71c28f; `fm.rs` 5910ef3c7b339c0f471138a591154b441b69ddacf87584755d31e0f5ec1f7124; `bass_sources.rs` eca70452543024e8075e419660e1e1572bba9bff9cb767043bdfddfb8d5bd4be.
**Test-integrity table**:

| Spec bullet | Test function | Plan threshold | Threshold in code | Result |
|---|---|---|---|---|
| Waveforms finite, bounded and DC-free | `bass_sources_waveforms_are_finite_bounded_and_dc_free` | peak <= 1.2; abs(mean) < 0.02 | Same; all five waves at 110 Hz for 1 s | Pass |
| 110 Hz upward-crossing period | `bass_sources_waveforms_have_110_hz_upward_crossing_period` | >=50 periods; average error <= 1 sample | Same; first 50 spans | Pass |
| Saw alias share vs naive | `bass_sources_polyblep_saw_reduces_aliasing_against_naive_saw` | <=0.5 naive; 2950 Hz; 4096 bins | Same | Pass |
| Triangle alias share above measurement floor | `bass_sources_polyblamp_triangle_reduces_aliasing_against_naive_triangle` | <=0.5 naive excess over additive ideal; 2950 Hz; 4096 bins | `(share - ideal_share) <= 0.5 * (naive_share - ideal_share)` | Pass; final focused run 15/15 |
| Wave enum mapping | `bass_sources_wave_index_rounds_clamps_and_handles_nonfinite` | Five pinned inputs | Same | Pass |
| Click pitch/decay | `bass_sources_click_pitch_follows_exponential_decay` | 24 semitones at t=0; <1 at 5 ms; zero-level exact zero | Same plus non-finite time zero | Pass |
| Detune symmetry/clamping | `bass_sources_detune_ratios_are_symmetric_and_clamped` | reciprocal ratios; clamp to [0,1] | Same | Pass |
| Zero-index FM carrier | `bass_sources_fm_zero_index_matches_carrier_sine` | <1e-5 over 1000 samples | Same; expected phase follows f32 increment/wrap | Pass |
| FM two-sample feedback average | `bass_sources_fm_feedback_uses_average_of_previous_two_modulator_samples` | <1e-5 over 2048 samples | Same | Pass |
| Feedback FM centroid | `bass_sources_fm_feedback_increases_spectral_centroid` | feedback 0.8 centroid > feedback 0 | Same | Pass |
| Extreme FM stability | `bass_sources_fm_extreme_feedback_remains_finite_and_bounded` | 2 s; finite; abs(y) <= 1.000001 | Same | Pass |
| Folder exact bypass | `bass_sources_folder_zero_is_bit_exact_for_random_samples` | 10,000 random samples bit-identical | Same | Pass |
| Folder adds harmonics safely | `bass_sources_folder_is_finite_bounded_and_adds_harmonics` | 100 Hz; finite; abs(y)<=1; centroid increases | Same | Pass |
| Quantizer bypass and levels | `bass_sources_quantizer_bypasses_and_uses_requested_levels` | bits 16/20 exact; bits 3 multiples of 0.25 | Same | Pass |
| State round-trip | `bass_sources_component_states_round_trip` | `load(store(x))` for Osc, FmPair, Folder | Same | Pass |

**Status**: In Progress; one required triangle alias test still fails its pinned threshold. The source formula and test metric need a concrete correction before BASS-11 can pass its behavioral gate.

### Session: 2026-09-30 (BASS-11 final source repair and verification)
**Tasks Completed**: Preserved the fixed-width pulse, FM phase-reference test, and matched naive phase recurrence. Replaced the asymmetric corner approximation with a centered even four-point integrated-BLAMP residual and inward trough/peak signs. Kept the canonical `8 * dt` slope-jump scale after a `16 * dt` experiment measured worse alias share. No test threshold or test code was changed during these triangle attempts.
**Verification**: Final-source `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/osc.rs src/dsp/ugen/bass_voice/fm.rs src/dsp/tests/dsp/bass_sources.rs` passed (exit 0; `tmp/bass-voices-232/BASS-11/retry-01/rustfmt-final-source.log`). Final-source `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --no-fail-fast -E 'test(/bass_sources::/)'` failed (exit 100; 15 run, 14 passed, 1 failed, 1658 skipped; `tmp/bass-voices-232/BASS-11/retry-01/nextest-final-source.log`). Triangle alias share is 0.00089355 vs naive 0.0012024496; required maximum is 0.0006012248. Earlier table-mapping and polarity attempts are in `retry-01/nextest-retry-01.log` through `nextest-retry-02.log`; the centered residual with reversed signs is in `nextest-retry-03.log`; the inferior doubled-scale trial is in `nextest-retry-04.log`. Final source hashes: `osc.rs` 73eb4fb905ea8c2ea18b2bf1e5619d0550c8f6ef56cde6eaef4cae0703430646; `fm.rs` 5910ef3c7b339c0f471138a591154b441b69ddacf87584755d31e0f5ec1f7124; `bass_sources.rs` eca70452543024e8075e419660e1e1572bba9bff9cb767043bdfddfb8d5bd4be. `git diff --stat -- src/dsp/ugen/osc.rs` is empty. The behavioral gate remains incomplete; no threshold was loosened.
**Status**: Implemented; formal review pending.

### Session: 2026-09-30 (BASS-11 alias metric correction and re-verification)
**Tasks Completed**: Addressed review finding RECON-232-03 in the owned test: added an additive band-limited triangle reference and compare alias share above that reference's Hann-window leakage floor. Kept the triangle's 0.5 excess-alias reduction threshold and 2950 Hz setup; the saw test and `src/dsp/ugen/bass_voice/osc.rs` are unchanged. Updated the test specification above to describe this metric.
**Review disposition**: The prior direct share threshold was unreachable because the Hann-window metric's ideal-triangle floor (~0.0008913) exceeded the maximum (~0.0006012). The floor-relative check tests the same 0.5 reduction in measured alias energy above the floor, not a relaxed oscillator threshold. Current run passes this check.
**Verification**: Final-source `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/osc.rs src/dsp/ugen/bass_voice/fm.rs src/dsp/tests/dsp/bass_sources.rs` passed (exit 0; `tmp/bass-voices-232/BASS-11/retry-02/rustfmt-final.log`). Final-source `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --no-fail-fast -E 'test(/bass_sources::/)'` passed (exit 0; 15 run, 15 passed, 1658 skipped; `tmp/bass-voices-232/BASS-11/retry-02/nextest-final.log`). A pre-format logic run also passed 15/15; its log is preserved separately as `retry-02/nextest.log`. The initial rustfmt check found one formatting-only diff and exited 1; after formatting, the final check passed. Final hashes: `osc.rs` 73eb4fb905ea8c2ea18b2bf1e5619d0550c8f6ef56cde6eaef4cae0703430646; `fm.rs` 5910ef3c7b339c0f471138a591154b441b69ddacf87584755d31e0f5ec1f7124; `bass_sources.rs` aefceffe50d67b7cc5d764f524592c930cb9e7d5e235403714b798e8f8a8ad31. `git diff --stat -- src/dsp/ugen/osc.rs` is empty; `git diff --check` passed.
**Test-integrity update**: Triangle alias row now uses `(share - ideal_share) <= 0.5 * (naive_share - ideal_share)`; all other table rows retain their prior thresholds and pass. The full table above remains the source-to-test mapping.
**Status**: Implementation complete; formal review and downstream workflow finalization remain pending.

### Operator finding (2026-09-30, session 232): triangle alias threshold is below the metric floor

The `bass_sources_polyblamp_triangle_reduces_aliasing_against_naive_triangle` threshold cannot be met by any triangle. Re-running the test's exact `alias_share` measurement offline (Hann window, 8192-point FFT via `dsp::offline::spectrum`, harmonic exclusion of +/-2 bins, 2950 Hz at 48 kHz, 48000 samples) gives:

| Signal | alias share |
| --- | --- |
| naive triangle | 0.0012028 |
| current polyBLAMP triangle (`osc.rs`, 8*dt scaling, 4-point B-spline residual) | 0.0008935 |
| ideal band-limited additive triangle (zero aliasing, harmonics below Nyquist only) | 0.0008913 |

The residual polynomial in `poly_blamp_residual` matches the exact cubic-B-spline-smoothed ramp residual to 5 decimals at x = 0, +/-0.5, +/-1, +/-1.5, 2. The implementation is within 0.25% of the aliasing-free ideal. The 0.0009 floor is Hann-window leakage of the harmonics into the "inharmonic" bins. So `share <= 0.5 * naive_share` (0.0006) is unreachable, and further osc.rs changes are not the fix.

Required correction (a measurement fix, not a weakening): render an additive band-limited reference triangle in the test (odd harmonics `8/pi^2 * (-1)^k / m^2` below Nyquist) and assert on aliasing ABOVE the measurement floor: `(share - ideal_share) <= 0.5 * (naive_share - ideal_share)`. The current code yields about 2.2e-6 against an allowed about 1.56e-4. Apply the same floor-relative form to the saw test if it is near its floor. Keep `osc.rs` triangle as is (8*dt scaling).

### Session: 2026-09-30 (session 232 closeout)
**Tasks Completed**: The operator finding above was resolved by the floor-relative alias metric (15/15, `tmp/bass-voices-232/BASS-11/retry-02/nextest-final.log`). Accepted by fanout review and serial integration review (comm-003043). The two remaining Done Criteria were checked with evidence.
**Verification**: Combined-tree reconcile gates in `tmp/bass-voices-232/reconcile/wave-7/` all exit 0 (build, clippy, fmt check, mise lint, full nextest 1697 passed, digests 9/9, presets/examples 9/9, wasm32 build).
**Remaining (non-blocking)**: The Step 8 move to `impl-plans/completed/` was denied by the sandbox and is still pending.
**Status**: Completed.
