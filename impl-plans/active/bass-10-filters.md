# BASS-10: Transistor Ladder and Diode Ladder Filters

**Status**: Ready
**Plan ID**: BASS-10 (wave 1; parallel with BASS-11 and BASS-12)
**Design Reference**: `design-docs/specs/design-bass-voices.md`, sections "DSP components" (transistor ladder, diode ladder), "Intentional simplifications" and "Verification"
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The bass kernel needs two original zero-delay-feedback (ZDF/TPT) low-pass
filters whose cutoff can change every sample:

- a 4-pole transistor ladder, used by the analog, fm, wobble, sub and reese
  models;
- a 4-pole diode ladder with coupled stages, used by the acid model.

The existing `ladder` UGen (`src/dsp/ugen/filter.rs`) is a different
algorithm. It reads cutoff once per block and has a one-sample feedback
delay, so it must not be reused or changed.

The components are plain structs. The kernel (BASS-20) loads them from voice
memory at block start, calls `process` once per sample, and stores them at
block end.

## License Boundary

Write the code from the equations in:

- Zavalishin, *The Art of VA Filter Design* (TPT one-pole, ZDF ladder
  solution, diode ladder);
- Stinchcombe 2008 (diode ladder topology);
- Pirkle's VA diode ladder equations;
- Huovilainen 2004 (input nonlinearity).

Do not consult, translate or paraphrase any source code: no GPL, LGPL or
unknown-license 303 or ladder emulations, and no book or app-note code.
Cite the papers in the module doc comments.

## Non-goals

- No oversampling, no per-stage `tanh`, no Newton iteration.
- No highpass or bandpass outputs. No new `Node` or registry entries.
- Do not edit `bass_voice.rs`, the other component files, `filter.rs`, or
  any file outside writePaths.

## Dependencies

- **dependsOn**: BASS-00
- **Blocks**: BASS-20

## writePaths

- `src/dsp/ugen/bass_voice/ladder.rs`
- `src/dsp/ugen/bass_voice/diode.rs`
- `src/dsp/tests/dsp/bass_filters.rs`
- `impl-plans/active/bass-10-filters.md`

## writePathNotes

- path: `src/dsp/ugen/bass_voice/ladder.rs` | intendedEdit: replace the placeholder
- path: `src/dsp/ugen/bass_voice/diode.rs` | intendedEdit: replace the placeholder
- path: `src/dsp/tests/dsp/bass_filters.rs` | intendedEdit: replace the placeholder
- path: `impl-plans/active/bass-10-filters.md` | intendedEdit: Progress Log only

## sharedPaths

None.

## Read-only References

- `crate::dsp::ugen::bass_voice::sanitize` from BASS-00.
- `crate::dsp::effects::prim::Rng` for the noise used in tests.
- `crate::dsp::offline::{rms, spectrum}`.

## Interface Contract (pin exactly; BASS-20 codes against it)

In `ladder.rs`:

- `#[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct Ladder`
  holds four stage states.
- `impl Ladder`:
  - `pub const FLOATS: usize = 4;`
  - `pub fn load(mem: &[f32]) -> Self` and `pub fn store(&self, mem: &mut [f32])`
    read and write exactly `FLOATS` values.
  - `pub fn reset(&mut self)`.
  - `pub fn flush(&mut self)` applies `sanitize` to every state. If any
    state was non-finite, it resets all of them.
  - `pub fn process(&mut self, x: f32, c: &LadderCoeffs) -> f32`.
- `#[derive(Clone, Copy, Debug, PartialEq)] pub struct LadderCoeffs` with
  `pub fn new(cutoff_hz: f32, res: f32, drive: f32, sr: f32) -> Self`.

In `diode.rs`, the same shape:

- `Diode` has `FLOATS = 5`: four stage states plus the feedback high-pass
  state.
- It has the same `load`, `store`, `reset`, `flush` and `process` methods.
- `DiodeCoeffs::new(cutoff_hz, res, drive, sr)`.

## Behavior to Implement

**Coefficients.** Computed per call; the kernel calls `new` once per sample.

- `cutoff_hz` is clamped to `[20, 0.45 * sr]`. A non-finite value becomes
  1000.
- `res` and `drive` are clamped to `[0, 1]`. Non-finite becomes 0.
- `g = tan(pi * fc / sr)`.

**Transistor ladder.**

- Four identical TPT one-pole low-pass stages.
- Resonance: `k = K_LADDER_MAX * res`, where `K_LADDER_MAX` is a documented
  `const` just above 4.0.
- Solve the zero-delay feedback loop in closed form from the stage states
  and the instantaneous gain `G^4`.
- Apply `tanh(drive_gain * u)` once to the solved input, then run the four
  stages.
- Drive: `drive_gain = 1 + DRIVE_GAIN * drive`. `DRIVE_GAIN` is about 7
  (+18 dB at `drive = 1`). A documented partial output compensation
  `DRIVE_COMP` applies.
- Output is `y4 * (1 + LADDER_COMP * k)`, where `LADDER_COMP` is documented.

**Diode ladder.**

- Four TPT integrators in the coupled diode-ladder form: each stage's input
  depends on its neighbour stage outputs.
- The instantaneous system is tridiagonal. Solve it in closed form per
  sample with fixed-size arithmetic: no loops that depend on data, and no
  allocation.
- One input `tanh` and a fixed one-pole high-pass in the resonance feedback
  path, with a documented corner `FB_HP_HZ` (start about 60 Hz).
- `k = K_DIODE_MAX * res`.
- `DIODE_FC_SCALE`: a documented, measured constant that pre-scales the
  cutoff so that the -3 dB corner at `res = 0` lands near the nominal
  `cutoff_hz`. A faithful coupled model's effective corner sits well below
  nominal (a review note), so measure it, set the constant, and record the
  measured corner in the Progress Log.

**Safety.** `process` never returns NaN. If an input or intermediate value
is non-finite, reset the state and return 0.0.

## Pitfalls

- Use `f32` throughout. `tan` is called per sample; do not cache it across
  samples inside the struct.
- Clamp before `tan`, because `fc` near `sr / 2` explodes.
- Do not add dynamic state beyond `FLOATS`. Everything the struct needs
  must round-trip through `load`/`store`, because the voice memory is the
  only persistence.
- Resonance tests measure steady-state gain at the frequency. Allow the
  filter to settle by discarding the first 0.25 s.
- Keep each file under 400 lines. Keep the tests in `bass_filters.rs`, not
  inline `#[cfg(test)]`.

## Tests (`src/dsp/tests/dsp/bass_filters.rs`)

Every test name starts with `bass_filters_` or lives in this module. Add a
helper that measures `gain_db(filter, coeffs, f_hz)`: a 0.75 s sine at
48 kHz, with the RMS of the last 0.5 s compared to the input RMS.

- Transistor ladder, cutoff 1000, res 0:
  - `|gain(250) - gain(62.5)| <= 3 dB`;
  - `gain(4000) <= gain(62.5) - 30 dB`.
- Diode ladder, cutoff 1000, res 0:
  - `|gain(250) - gain(62.5)| <= 3 dB`;
  - `gain(4000) <= gain(62.5) - 20 dB`.
- Both filters, cutoff 1000, `res` in {0, 0.25, 0.5, 0.75, 1}:
  - `gain(1000)` is strictly increasing;
  - `gain(1000)` at `res = 1` is at least `gain(1000)` at `res = 0` plus
    12 dB.
- Both filters, white noise at amplitude 4.0 (seeded `Rng`) for 2 s, over
  every combination of:
  - cutoff in {20, 20000};
  - res in {0, 1};
  - drive in {0, 1};

  gives all outputs finite and `|y| < 16`.
- Both filters: 0.5 s of noise, then 2 s of zeros at cutoff 100, then
  `flush()` leaves every state exactly `0.0`.
- Both filters: `process(f32::NAN, ..)` returns `0.0`, and the next
  `process(0.1, ..)` is finite.
- Both filters: `load(store(x))` round-trips bit-exactly.
- Both filters: the same input processed twice from `default()` gives
  bit-identical output.

## Verification (evidence required)

1. `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/ladder.rs src/dsp/ugen/bass_voice/diode.rs src/dsp/tests/dsp/bass_filters.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_filters::/)' > tmp/logs/bass-10-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`, with at least 8 tests run and 0 failed.
3. Record the measured diode -3 dB corner, `K_LADDER_MAX`, `K_DIODE_MAX`,
   `DIODE_FC_SCALE` and `FB_HP_HZ` in the Progress Log.

## Concurrency and Drift Protocol

- Only writePaths are edited. Fresh-read before each edit.
- Record `shasum -a 256` of the owned files at start and finish.
- Siblings BASS-11 and BASS-12 edit other files in the same tree at the
  same time. If `cargo` fails in a file you do not own, record it and
  re-run after a short wait. Do not edit that file.
- No git operations other than `status` and `diff`.

## Done Criteria

- [ ] The interface contract matches exactly: `FLOATS`, method names and
      signatures.
- [ ] Verification 1-2 pass, with the exit code and test count recorded.
- [ ] Constants and measurements recorded. No file outside writePaths
      changed (`git diff --stat`).

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none
