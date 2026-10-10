# FM1V-11: Operator Envelope and Keyboard/Velocity/Frequency Scaling

**Status**: Ready
**Plan ID**: FM1V-11 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("Operator, EG and scaling behaviour", "License boundary")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The extended FM engine (FM1V-20) needs the per-operator behaviour of the
classic six-operator voice:

- the 4-rate/4-level envelope generator (EG);
- the output-level curve;
- keyboard level scaling and keyboard rate scaling;
- velocity sensitivity;
- ratio and fixed operator frequency with detune;
- feedback scaling;
- transpose.

The semantics come from msfa (Apache-2.0). msfa is the only permitted code
reference. Formulas are adapted into f32 Rust and the adaptation is recorded
in a doc comment. FM1V-51 copies that record into `THIRD_PARTY_NOTICES.md`.

## Non-goals

- No operator rendering, routing or state layout (FM1V-20).
- No LFO, pitch EG, AMS/PMS or oscillator key sync (design "Intentional
  simplifications").
- No registry edits.
- Do not read Dexed beyond msfa, or any GPL source.

## Dependencies

- **dependsOn**: FM1V-00 (module tree, `patch.rs`, msfa checkout)
- **Blocks**: FM1V-20

## writePaths

- `src/dsp/ugen/fm/envelope.rs`
- `src/dsp/ugen/fm/scaling.rs`
- `src/dsp/tests/dsp/fm6_envelope.rs`
- `impl-plans/active/fm1-voices-11-eg-scaling.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-11` (artifact root)

## sharedPaths

None.

## Read-only References

- msfa at FM1V-00's recorded SHA:
  - `env.cc`/`env.h`: `Env::init`, `Env::getsample`, `Env::keydown`,
    `Env::advance`, the `levellut` table and the `sr_multiplier` handling;
  - `dx7note.cc`: `ScaleLevel`, `ScaleCurve`, `ScaleRate`, `ScaleVelocity`,
    `scaleoutlevel`, `osc_freq`, the feedback shift, and the outlevel
    composition in `Dx7Note::init`;
  - `exp2.h`/`exp2.cc` for the log-to-linear meaning of levels.
- `src/dsp/ugen/fm/patch.rs` (FM1V-00) for the parameter ranges.

## File-level Changes

### `src/dsp/ugen/fm/envelope.rs`

Contract. FM1V-20 depends on it. Keep the names exactly.

- `pub const EG_BLOCK: usize = 64;`. The EG advances once per 64 output
  frames, as msfa's `getsample` does once per N=64 block.
- `#[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct Eg { /* private fields: f32/i32 only */ }`
- `pub const EG_FLOATS: usize`: the number of f32 slots that `store` and
  `load` use.
- `impl Eg`:
  - `pub fn new(rates: [u8; 4], levels: [u8; 4], outlevel: i32, rate_scaling: i32, sr: f32) -> Eg`
    starts in key-on, at msfa's initial level.
  - `pub fn tick(&mut self) -> i32` advances one `EG_BLOCK` and returns the
    level in msfa's Q24 log units, as `Env::getsample` does.
  - `pub fn key_off(&mut self)`.
  - `pub fn released_and_silent(&self) -> bool` is true after `key_off` once
    the release segment has reached its target.
  - `pub fn store(&self, dst: &mut [f32])` and
    `pub fn load(src: &[f32]) -> Eg` round-trip exactly.
    - Integers are stored as f32 only when they are exactly representable,
      i.e. `|v| < 2^24`. If a Q24 value can exceed that, split it into two
      slots.
- `pub const FULL_SCALE_LEVEL: i32` is the steady level for L=99, OL=99,
  scaling 0, velocity sensitivity 0.
- `pub fn log_to_amp(level: i32) -> f32` returns
  `2^((level - FULL_SCALE_LEVEL) / 2^24)`. `log_to_amp(FULL_SCALE_LEVEL)` is
  exactly 1.0 and the function is monotonic.

### `src/dsp/ugen/fm/scaling.rs`

Contract:

- `pub fn midi_key(note_hz: f32) -> i32` returns `round(69 + 12*log2(f/440))`,
  clamped to 0..=127. A non-finite or non-positive input gives 69.
- `pub fn velocity_midi(velocity: f32) -> i32` returns `round(v * 127)`,
  clamped to 0..=127. Non-finite gives 127.
- `pub fn scale_out_level(ol: u8) -> i32` (msfa `scaleoutlevel`).
- `pub fn scale_level(key: i32, break_point: u8, left_depth: u8, right_depth: u8, left_curve: u8, right_curve: u8) -> i32`
  (msfa `ScaleLevel`, curves 0 -LIN, 1 -EXP, 2 +EXP, 3 +LIN).
- `pub fn scale_rate(key: i32, sensitivity: u8) -> i32` (msfa `ScaleRate`).
- `pub fn scale_velocity(velocity_midi: i32, sensitivity: u8) -> i32`
  (msfa `ScaleVelocity`).
- `pub fn op_outlevel(patch: &Fm6Patch, op: usize, key: i32, velocity_midi: i32) -> i32`
  composes the operator outlevel the way `Dx7Note::init` does. FM1V-20
  passes it to `Eg::new`.
- `pub fn op_rate_scaling(patch: &Fm6Patch, op: usize, key: i32) -> i32`.
- `pub fn op_freq_hz(note_hz: f32, osc_mode: u8, coarse: u8, fine: u8, detune: u8) -> f32`:
  - **Ratio mode.** Coarse 0 means 0.5, otherwise the coarse value, times
    `(1 + fine/100)`. Detune is msfa's log-frequency offset per step from
    centre 7, converted to a frequency multiplier.
  - **Fixed mode.** `10^((coarse mod 4) + fine/100)` Hz, as msfa computes
    it. `note_hz` is ignored.
  - Using `note_hz`, not a key number, keeps microtonal tuning exact.
- `pub fn transpose_hz(note_hz: f32, transpose: u8) -> f32` returns
  `note_hz * 2^((t-24)/12)`.
- `pub fn feedback_gain(fb: u8) -> f32` is the multiplier applied to the
  average of the source operator's last two outputs, in cycles of phase per
  unit of output. It is 0.0 for fb 0. Derive it from msfa's shift
  (`FEEDBACK_BITDEPTH - fb`), so each step doubles the gain. Document the
  derivation.

Doc comment in both files: "Adapted from music-synthesizer-for-android
(Apache-2.0), revision `<sha>`: <file list>. Modified: translated to Rust,
floating-point frequency path, no lookup tables beyond the level curve."

## Pitfalls

- **Use the right reference.** Use msfa's integer formulas for level and
  rate math, then convert at the boundary. Do not use rough exponential
  approximations from memory. A formula that is not in msfa needs a
  Progress Log note.
- **Sample-rate scaling.** The EG rate is scaled for sample rate as msfa
  does. At 44.1, 48 and 96 kHz the same segment must take the same
  wall-clock time within 1 ms (+/- one block).
- **Attack segments.** In msfa a rising segment moves along a different
  curve, the "jump + exponential approach". Implement it. A purely linear
  log rise fails the timing test.
- **Out-of-range bytes.** Treat values above the max as the max, without
  panicking. FM1V-12 already clamps, but be defensive.
- No allocation and no `Vec`. These are `fn`s over scalars and small arrays.

## Tests (`src/dsp/tests/dsp/fm6_envelope.rs`; names contain `fm6`)

- `fm6_eg_settles_to_l3_while_held`: rates 99/99/99/99 and levels
  99/80/60/0, held for 2 s at 48 kHz. The last `tick` level equals the
  L3-derived level exactly (as computed by the msfa formula).
- `fm6_eg_releases_to_l4_after_key_off`: after `key_off`,
  `released_and_silent()` becomes true within 2 s at R4 99. Before
  `key_off` it never becomes true.
- `fm6_eg_higher_rate_is_shorter`: for rates {25, 50, 75, 99} on R2, the
  number of ticks to reach the L2 target strictly decreases.
- `fm6_eg_timing_matches_msfa_formula`. Choose rates {0, 25, 50, 75, 99}
  and compute the expected tick counts by hand from the msfa formula. Put
  those integers in the test with a comment that shows the derivation, and
  check the implementation within +/- 1 tick at 48 kHz. Rate 0 is checked as
  "has not reached the target after 1000 ticks".
- `fm6_eg_store_load_round_trips` mid-segment.
- `fm6_eg_sample_rate_independent`: the same segment's duration in seconds
  agrees within 1 block at 44.1, 48 and 96 kHz.
- `fm6_scale_level_curves`:
  - depth 0 gives 0 for every key;
  - for `-LIN` and `-EXP`, the offset does not increase moving away from the
    break point;
  - for `+LIN` and `+EXP`, it does not decrease;
  - this holds on both sides, with the left/right depth and curve chosen
    independently.
- `fm6_scale_rate_increases_with_key` for sensitivity 7. Sensitivity 0 is
  constant.
- `fm6_velocity_sensitivity_zero_is_flat`: with sensitivity 0,
  `op_outlevel` is equal for velocity 0, 64 and 127. With sensitivity 7,
  velocity 127 > velocity 0.
- `fm6_op_freq_modes`:
  - ratio coarse 0, fine 0, detune 7 gives exactly `0.5 * f`;
  - coarse 2, fine 50 gives `3 * f`;
  - fixed coarse 1, fine 0 gives 10 Hz independent of `note_hz`;
  - detune 14 > detune 7 > detune 0, and the multipliers stay within 1%.
- `fm6_feedback_gain_doubles`: `feedback_gain(0) == 0`, and
  `feedback_gain(k+1) == 2 * feedback_gain(k)` for k in 1..7.
- `fm6_log_to_amp_full_scale_is_one`, and monotonicity.

## Verification (logs under `tmp/fm1-voices/FM1V-11/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/fm/envelope.rs src/dsp/ugen/fm/scaling.rs src/dsp/tests/dsp/fm6_envelope.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6_eg|fm6_scale|fm6_velocity|fm6_op_freq|fm6_feedback|fm6_log/)' > tmp/fm1-voices/FM1V-11/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 12 tests run and 0 failed.
3. `git diff --stat` shows only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

Same as FM1V-10. You own only your files. If a build fails in another
plan's file, wait and re-run.

## Done Criteria

- [ ] The envelope and scaling contracts are implemented with msfa
      attribution doc comments.
- [ ] Verification 1-3 pass and are recorded.
- [ ] If msfa is recorded as unavailable, mark this plan blocked instead.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.
