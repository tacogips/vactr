# BASS-11: Band-limited Oscillators, Feedback FM, Folder and Bit Depth

**Status**: Ready
**Plan ID**: BASS-11 (wave 1; parallel with BASS-10 and BASS-12)
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

- [ ] Interface contract exact.
- [ ] Verification passes, with the exit code and test count recorded.
- [ ] `git diff --stat` touches only writePaths.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none
