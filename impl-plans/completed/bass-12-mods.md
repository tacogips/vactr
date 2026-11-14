# BASS-12: Envelopes, Gate Length, Accent, Glide and Tempo-synced LFO

**Status**: Completed (session 232; accepted by fanout review and integration review comm-003043; ready to archive to `impl-plans/completed/`)
**Plan ID**: BASS-12 (session 232 wave 1, runs alone; BASS-10 and BASS-11 re-verify after it)
**Design Reference**: `design-docs/specs/design-bass-voices.md`, sections "Envelopes and timing", "Tempo sync and wobble divisions", "Note length and slide" and "Verification"
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan holds all the per-voice control logic the kernel needs:

- the tempo-relative note length (`gate-length`, in sixteenth-steps of
  `1 / (16 * cps)` seconds);
- the amp and filter envelopes;
- the accent mapping;
- the slide glide;
- the wobble LFO.

The LFO reproduces the digital drums' formulas. Those helpers are
`pub(super)` in `src/dsp/ugen/digital_drum/chain.rs` and must **not** be
imported or edited. The formulas are:

- `rate_hz = sync ? lfo_rate * cps : lfo_rate`;
- initial phase `offset` if retrigger, else `frac(rate_hz * onset)`.

## Non-goals

- No oscillator or filter code, and no registry work.
- No cross-voice state: slide is only the per-event `slide-from` offset.
- Do not edit `digital_drum/` or any file outside writePaths.

## Dependencies

- **dependsOn**: BASS-00 (completed in session 229, so no dispatch dependency in session 232)
- **Blocks**: BASS-20. In session 232 it also blocks the BASS-10 and BASS-11
  re-verification runs, because they share the lib test crate and cannot
  run while `bass_mods.rs` or `mods.rs` fails to compile.

## Session 232 scope (read first)

- Keep every non-LFO item in `mods.rs` and every non-LFO test in
  `bass_mods.rs` as is. Those 15 tests already pass
  (`tmp/bass-voices-229/BASS-12/nextest.log:42`). Change only `Lfo` (the
  struct, `FLOATS`, `load`, `store`, `start`, `next`, the accessors, and the
  new `COUNT_SPLIT`) and the `Lfo` tests.
- `raw_value`, `random_value`, `rate_hz`, `initial_phase`, `LfoShape` and
  `cutoff_octaves` keep their current signatures and behaviour. `raw_value`
  now receives the derived `phase` and `cycle`.
- Update the existing `Lfo` tests: the wrap detection now uses
  `cycles(48_000.0)`, and the round-trip test uses the six-field literal.
  Add the three new tests listed under Tests.
- The `bass_mods.rs` ambiguous-integer error is already fixed
  (`wrap: Option<usize>` at line 170). Do not re-fix it.
- The implementation goes through `mods.rs:Lfo`. Imitate the component
  style already in the same file (`Glide::load`/`store`, and the
  `finite_or`/`finite_clamp` helpers).

## writePaths

- `src/dsp/ugen/bass_voice/mods.rs`
- `src/dsp/tests/dsp/bass_mods.rs`
- `impl-plans/active/bass-12-mods.md`

## writePathNotes

- path: `impl-plans/active/bass-12-mods.md` | intendedEdit: Progress Log only

## sharedPaths

None.

## Read-only References

- `bass_voice::sanitize`.

## Interface Contract (pin exactly)

Timing:

- `pub const CPS_MIN: f32 = 0.03; pub const CPS_MAX: f32 = 50.0;`
- `pub fn clamp_cps(cps: f32) -> f32` maps non-finite to 0.5, then clamps.
- `pub fn gate_samples(gate_length: f32, cps: f32, sr: f32) -> u32` is
  `round(clamp(gate_length, 0.05, 64) / (16 * clamp_cps(cps)) * sr)`. A
  non-finite `gate_length` becomes 1.0.

Settle-to-1% semantics:

- `pub fn settle_coeff(time_s: f32, sr: f32) -> f32` is
  `exp(-ln(100) / (max(time_s, 1e-4) * sr))`.
- `env-decay` and `slide-time` both use it: after `time_s` seconds the
  value has decayed to 1%.

Amp envelope:

- `#[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct AmpEnv`
  holds `level` and `stage`. `stage` is stored as f32:
  0 = attack, 1 = decay/sustain, 2 = release, 3 = done.
  - `FLOATS = 2`, plus `load`/`store`.
- `pub struct AmpParams { pub attack: f32, pub decay: f32, pub sustain: f32, pub release: f32 }`
- `pub const LEGATO_RAMP_S: f32 = 0.002;`
- `pub fn start(&mut self, legato: bool)`: level 0, stage attack.
- `pub fn release(&mut self)`: stage release, unless the stage is already
  done.
- `pub fn next(&mut self, p: &AmpParams, legato: bool, sr: f32) -> f32`:
  - Attack is a linear ramp to its target:
    - non-legato: to 1.0 over `max(attack, 1 sample)`;
    - legato: to `sustain` over `LEGATO_RAMP_S`.
  - Decay approaches `sustain` exponentially with `settle_coeff(decay)`.
    Legato notes skip decay.
  - Release decays by `exp(-ln(1e4) / (max(release, 1e-4) * sr))`, so it
    reaches -80 dB after `release` seconds.
  - Once the level is below `1e-4` in release, the stage becomes done and
    the level 0.
- `pub fn done(&self) -> bool`.

Filter envelope:

- `pub struct FilterEnv` holds `value`; `FLOATS = 1`, plus `load`/`store`.
- `start(&mut self, legato: bool)` sets the value to `0.0` when legato and
  `1.0` otherwise.
- `next(&mut self, decay_s: f32, sr: f32) -> f32` returns the current value
  and then multiplies it by `settle_coeff(decay_s)`.

Accent:

- `pub struct AccentOut { pub oct: f32, pub decay: f32, pub gain: f32 }`
- `pub fn accent(a: f32, res: f32, env_decay: f32) -> AccentOut`, with `a`
  and `res` clamped to `[0, 1]`:
  - `oct = a * ACCENT_OCT * (0.5 + 0.5 * res)`, with `ACCENT_OCT = 2.0`;
  - `decay = env_decay + a * (min(env_decay, ACCENT_DECAY) - env_decay)`,
    with `ACCENT_DECAY = 0.2`;
  - `gain = 1 + ACCENT_GAIN * a`, with `ACCENT_GAIN = 0.4`.

Glide:

- `pub struct Glide` holds the offset in semitones; `FLOATS = 1`, plus
  `load`/`store`.
- `start(&mut self, slide_from: f32)` clamps to +/-24; non-finite becomes 0.
- `next(&mut self, slide_time: f32, sr: f32) -> f32` returns the current
  offset, then multiplies it by `settle_coeff(slide_time)`. It snaps to
  exactly 0.0 once `|offset| < 1e-4`.

LFO:

- `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum LfoShape { Sine, Tri, Saw, Ramp, Square, Random }`,
  with `from_index` using the `LFO_WAVES` order of `controls.rs`.
- `pub fn rate_hz(rate: f32, sync: bool, cps: f32) -> f32` clamps `rate` to
  `[0.01, 50]`, then returns `rate * clamp_cps(cps)` when synced and `rate`
  otherwise.
- `pub fn initial_phase(retrigger: bool, offset: f32, rate_hz: f32, onset: f32) -> f32`
  returns:
  - `offset.rem_euclid(1.0)` when retriggering;
  - otherwise `(rate_hz as f64 * onset as f64).rem_euclid(1.0) as f32`,
    **computed in f64**.
- `pub struct Lfo` (revised in session 232 for a drift-free phase; the
  three-float `phase`/`cycles`/`smooth` layout is retired) has these
  fields, in this order: `pub anchor_phase: f32`, `pub anchor_cycles: f32`,
  `pub count_lo: f32`, `pub count_hi: f32`, `pub rate: f32` and
  `pub smooth: f32`. `pub const FLOATS: usize = 6;`, plus `load`/`store` in
  that slot order.
  - The elapsed count is `n = count_hi * 2^20 + count_lo`, evaluated in
    f64. Define `pub const COUNT_SPLIT: f32 = 1_048_576.0;` (2^20).
  - `load`/`store` clamp the slots. `anchor_phase` is `rem_euclid(1)`.
    `anchor_cycles` and `count_hi` are floored and clamped to
    `[0, 16_777_216]`. `count_lo` is floored and clamped to
    `[0, COUNT_SPLIT)`. `rate` is finite and at least 0. `smooth` is clamped
    to `[0, 1]`. Non-finite values become 0.
  - `pub fn start(&mut self, phase: f32, shape: LfoShape, seed: u32)` does
    the following:
    - sets `anchor_phase = phase.rem_euclid(1)` (non-finite becomes 0);
    - zeroes `anchor_cycles`, the count and `rate`;
    - sets `smooth` to the raw value at that phase, so there is no ramp
      from 0.
  - `pub fn next(&mut self, shape: LfoShape, rate_hz: f32, seed: u32, sr: f32) -> f32`
    does the following:
    1. Sanitize `rate_hz` to a finite value of at least 0, and `sr` to a
       finite value of at least 1 (non-finite becomes 48000).
    2. If `rate_hz.to_bits() != self.rate.to_bits()`, re-anchor at the
       current `n` with the old rate. Set `anchor_cycles += floor(pos)`
       and `anchor_phase = frac(pos)`, zero the count, then set
       `rate = rate_hz`.
    3. Compute `pos = anchor_phase + n * rate / sr` in f64. Then
       `phase = frac(pos)` and `cycle = anchor_cycles + floor(pos)`,
       clamped to 2^24.
    4. Compute the raw shape value at `(phase, cycle)`. Advance the
       one-pole smoother (`LFO_SMOOTH_S = 0.002`) and return the smoothed
       unipolar `u` in `[0, 1]`.
    5. Increment the count: `count_lo += 1`. On reaching `COUNT_SPLIT`, set
       `count_lo = 0` and `count_hi += 1`, saturating at 2^24.
  - The value returned is always the one at the pre-increment position, as
    before. The first call after `start` re-anchors with `n = 0`, which is
    a no-op for phase.
  - Accessors take the sample rate, because the position is derived:
    - `pub fn cycles(&self, sr: f32) -> f32` is
      `anchor_cycles + floor(pos)` at the current count;
    - `pub fn phase(&self, sr: f32) -> f32` is `frac(pos)`.
  - No incremental `phase += inc` accumulation may remain anywhere in
    `Lfo`.
  - Raw shapes:
    - sine: `0.5 - 0.5 * cos(2*pi*p)`;
    - tri: rises 0 to 1 at `p = 0.5`, back to 0;
    - saw: `p`;
    - ramp: `1 - p`;
    - square: 1 for `p < 0.5`, else 0;
    - random: a deterministic integer hash of `(seed, cycles)` mapped to
      `[0, 1]`, constant within a cycle.
- `pub const WOBBLE_OCT: f32 = 5.0;`
  `pub fn cutoff_octaves(depth: f32, u: f32) -> f32` is
  `WOBBLE_OCT * clamp(depth, -1, 1) * u`.

## Pitfalls

- `onset-time` can be up to 3600 s. The phase product must be computed in
  f64, or the phase error exceeds 1%.
- Do not store RNG state in f32 memory; arbitrary bit patterns can be NaN
  and the flush would clobber them. Random uses a hash of `(seed, cycles)`.
- Never accumulate the LFO phase in f32 or f64. Session 229 lost about
  0.08% per cycle that way, because each `phase += rate / sr` rounds. Only
  integers (the count and the cycle base) are stored. The phase is derived
  in f64 every sample.
- Integers are exact in f32 only below 2^24 (about 349 s at 48 kHz), so
  the count is split into `count_lo`/`count_hi`. `anchor_cycles` stays far
  below 2^24 for any voice.
- Evaluate the position as `anchor_phase + (n * rate) / sr` in f64,
  multiply first. `n * (rate / sr)` can round 270.0 down to 269.999...,
  and then `floor` returns 269, which fails the long-playback test. Cast
  each operand to f64 before the arithmetic.
- Re-anchor on a bit-level rate change only. Comparing with a tolerance
  would let tiny port changes silently scale `n` and jump the phase.
- `sanitize` flushes values below 1e-20 to 0. That is harmless for whole
  numbers and for `anchor_phase`.
- Legato semantics: the amp starts at 0 but reaches `sustain` within 2 ms
  and never exceeds it.
- Every `next` must stay finite for non-finite parameters. Clamp or
  substitute defaults.

## Tests (`src/dsp/tests/dsp/bass_mods.rs`)

- `gate_samples`:
  - `(1, 0.5, 48000) == 6000`;
  - `(0.55, 0.5625, 48000) == 2933`;
  - `(1, 0, 48000)` uses `cps = 0.03`;
  - `(1000, 0.5, 48000) == 64 * 6000`.
- `AmpEnv` non-legato, attack 0.002: the level reaches 1.0 at sample 96
  (+/-1). With sustain 0.5 and decay 0.1 it is within 1% of 0.5 after 0.1 s.
  After `release()` with release 0.05: `done()` within `0.05 * 48000 + 2`
  samples, and the output is then 0.
- `AmpEnv` legato, sustain 0.8: at least 0.79 by sample 97, and never above
  0.8.
- `FilterEnv`: starts at 1 (non-legato) or 0 (legato); at `t = 0.2 s` with
  decay 0.2 the value is within [0.0095, 0.0105].
- `accent`:
  - `(0, 1, 0.5) == {0, 0.5, 1}`;
  - `(1, 1, 0.5) == {2, 0.2, 1.4}`;
  - `(1, 0, 0.1)` gives `oct = 1` and `decay = 0.1`.
- `Glide`: `start(-12)` gives a first value of -12, and the value after
  `0.06 s` with `slide_time = 0.06` is within +/-0.12. `start(0)` gives
  exactly 0 for 1000 samples.
- LFO synced, cps 0.5625, `rate` in {1, 2, 4, 8, 3, 6, 12}: the sample
  distance between consecutive wraps equals
  `round(48000 / (rate * 0.5625))` +/-1, checked over at least 3 wraps.
  Wraps are detected with `cycles(48000.0)`. Keep the tolerance at +/-1.
- LFO free, `rate_hz(3.2, false, 0.5) == 3.2`, and the wrap period is
  15000 +/-1 samples. Keep the `Option<usize>` annotation on `wrap`. Session
  229 hit an ambiguous-integer `abs_diff` compile error here, and it is
  already fixed in the tree.
- Drift-free long playback (new test,
  `bass_mods_synced_lfo_stays_drift_free_over_long_playback`): run `next`
  5,760,000 times at `rate_hz(4, true, 0.5625)` (2.25 Hz) and 48 kHz. Then
  `cycles(48000.0) == 270.0`, and the circular distance of
  `phase(48000.0)` from 0 is at most 1e-6.
- Split count (new test): a state loaded with
  `count_hi = 3, count_lo = 12345, rate = 2.25, anchor_phase = 0.1`
  gives a `phase(48000.0)` equal to
  `frac(0.1 + (3 * 2^20 + 12345) * 2.25 / 48000)`, computed in f64 and
  within 1e-6. After `2^20 - 12345` more `next` calls at `rate_hz = 2.25`, so no
  re-anchor happens, `count_hi == 4`
  and `count_lo == 0`.
- Rate change (new test): run 1000 samples at 2 Hz, then switch to 8 Hz.
  `phase(48000.0)` after the switching `next` call equals `phase(48000.0)`
  before it plus one new increment, `8 / 48000`, within 1e-7. Later calls
  keep advancing by `8 / 48000`. `cycles` never decreases.
- `initial_phase`:
  - `(false, 0, 2.0, 10.25) == 0.5`;
  - `(true, 1.25, 2.0, 10.25) == 0.25`;
  - `(false, 0, 50, 3599.99)` is finite and in `[0, 1)`.
- Random shape: the same seed gives an identical sequence; seeds 1 and 2
  differ within the first 8 cycles; values stay in `[0, 1]`.
- Square shape at 8 Hz: the largest per-sample change of the smoothed
  output is below 0.02.
- `cutoff_octaves`: `(0.8, 1.0) == 4.0`; `(-1, 1) == -5`.
- `load(store(x))` round-trips for every struct. The `Lfo` case uses the
  six-field layout with non-zero `count_hi`.

## Verification (evidence required)

Session 232 logs go to `tmp/bass-voices-232/BASS-12/`, using the exact
commands in the manifest.

1. `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/mods.rs src/dsp/tests/dsp/bass_mods.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_mods::/)'`
   must give `exit=0`, with at least 20 tests and 0 failed (the 17
   existing plus 3 new). The two LFO period tests must pass with their
   `<= 1` tolerances unchanged.
3. `grep -n "phase +\|+= rate\|+ rate" src/dsp/ugen/bass_voice/mods.rs`
   must show no incremental phase accumulation left in `Lfo`. Also record
   `wc -l src/dsp/ugen/bass_voice/mods.rs` and confirm it is under 450.
4. `git diff --stat` touches only this plan's writePaths, compared with
   the start-of-run snapshot.

## Concurrency and Drift Protocol

- Owned files only; fresh reads.
- Record `shasum -a 256` at start and finish.
- Wait out and record sibling compile failures; never edit a sibling's
  file.

## Done Criteria

- [x] Interface contract exact, including the session 232 six-float `Lfo`;
      `mods.rs` under 450 lines. The cap was raised from 400 for the
      drift-free LFO.
- [x] Verification passes, with the exit code and test count recorded.
- [x] Only writePaths changed.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none

### Session: 2026-09-30 (BASS-12 implementation)
**Tasks Completed**: Implemented the modulation component API and 17 focused tests for gate timing, envelopes, accent, glide, LFO behavior, serialization, and non-finite controls. `mods.rs` is 381 lines.
**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/mods.rs src/dsp/tests/dsp/bass_mods.rs` exited 0 (`tmp/bass-voices-229/BASS-12/rustfmt-final.log`). The scoped nextest run exited 100: 17 tests ran, 15 passed, and 2 LFO period tests failed (`tmp/bass-voices-229/BASS-12/nextest.log`).
**Findings**: The synced 0.5625 Hz case measured wraps of 85265, 85266, and 85265 samples against 85333 expected; the free 3.2 Hz case did not wrap by sample 15002. The pinned `Lfo` contract stores phase/cycles/smoothing in 3 floats; a precision repair must preserve or explicitly revise that state contract before BASS-12 can pass its behavioral gate.
**Remaining**: Fix the LFO period error within the accepted state contract, rerun scoped nextest to green, then mark completion criteria.

### Session: 2026-09-30 (session 232 design revision)
**Tasks Completed**: Revised the `Lfo` contract. The design and this plan now specify a six-float, drift-free layout: the phase is derived in f64 from an exact split sample count, and the LFO re-anchors on rate change. This replaces the three-float accumulating LFO. Added the long-playback, split-count and rate-change test specs. The `mods.rs` cap is now 450 lines.
**Findings**: The ambiguous-integer `abs_diff` error that blocked the BASS-11 nextest (`tmp/bass-voices-229/BASS-11/nextest.log:57-60`, old line 183) is already fixed in the tree: `wrap` is annotated as `Option<usize>` at `bass_mods.rs:170`. The later BASS-12 run compiled and executed 17 tests (`tmp/bass-voices-229/BASS-12/nextest.log:42`).
**Remaining**: Implement the revised `Lfo` in `mods.rs`, update the `Lfo` tests in `bass_mods.rs` (the `cycles(sr)` accessor, the six-field round-trip, three new tests), then run both verification commands.

### Session: 2026-09-30 (session 232 implementation)
**Tasks Completed**: Replaced incremental phase updates with the six-float LFO state and f64 sample-count-derived phase, including split-count carry and bitwise rate-change re-anchoring. Updated wrap/accessor and round-trip tests; added long-playback, split-count and rate-change tests. The `Option<usize>` wrap annotation remains in place. `mods.rs` is 432 lines.
**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/mods.rs src/dsp/tests/dsp/bass_mods.rs` exited 0 (`tmp/bass-voices-232/BASS-12/rustfmt-final.log`). `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_mods::/)'` exited 0 with 20 tests passed and 0 failed (`tmp/bass-voices-232/BASS-12/nextest.log`). The first format check exited 1 only for formatting and was resolved by rustfmt (`tmp/bass-voices-232/BASS-12/rustfmt.log`). The no-incremental-phase search returned no matches and `wc -l` reported 432 (`no-incremental-phase.log`, `line-count.log`). Final source SHA-256 values are recorded in `final-source-hashes.txt`.
**Findings**: None in the assigned implementation scope.
**Remaining**: Formal implementation review and workflow finalization are downstream steps.

### Session: 2026-09-30 (session 232 closeout)
**Tasks Completed**: Accepted by fanout review and serial integration review (comm-003043). The drift-free LFO is in place.
**Verification**: Combined-tree reconcile gates in `tmp/bass-voices-232/reconcile/wave-7/` all exit 0 (build, clippy, fmt check, mise lint, full nextest 1697 passed, digests 9/9, presets/examples 9/9, wasm32 build).
**Remaining (non-blocking)**: The Step 8 move to `impl-plans/completed/` was denied by the sandbox and is still pending.
**Status**: Completed.
