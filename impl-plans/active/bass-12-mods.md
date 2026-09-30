# BASS-12: Envelopes, Gate Length, Accent, Glide and Tempo-synced LFO

**Status**: Ready
**Plan ID**: BASS-12 (wave 1; parallel with BASS-10 and BASS-11)
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

- **dependsOn**: BASS-00
- **Blocks**: BASS-20

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
- `pub struct Lfo` holds `phase`, `cycles` (a whole-number count stored as
  f32) and `smooth`; `FLOATS = 3`, plus `load`/`store`.
  - `pub fn start(&mut self, phase: f32, shape: LfoShape, seed: u32)` sets
    the phase, `cycles = 0`, and `smooth` to the raw value at that phase,
    so there is no ramp from 0.
  - `pub fn next(&mut self, shape: LfoShape, rate_hz: f32, seed: u32, sr: f32) -> f32`
    returns the smoothed unipolar value `u` in `[0, 1]`, with a one-pole
    smoother of `LFO_SMOOTH_S = 0.002`. It then advances the phase and
    increments `cycles` on wrap.
  - `pub fn cycles(&self) -> f32`.
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
- `cycles` stays exact in f32 up to 2^24. That is far beyond any single
  voice lifetime.
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
- LFO free, `rate_hz(3.2, false, 0.5) == 3.2`, and the wrap period is
  15000 +/-1 samples.
- `initial_phase`:
  - `(false, 0, 2.0, 10.25) == 0.5`;
  - `(true, 1.25, 2.0, 10.25) == 0.25`;
  - `(false, 0, 50, 3599.99)` is finite and in `[0, 1)`.
- Random shape: the same seed gives an identical sequence; seeds 1 and 2
  differ within the first 8 cycles; values stay in `[0, 1]`.
- Square shape at 8 Hz: the largest per-sample change of the smoothed
  output is below 0.02.
- `cutoff_octaves`: `(0.8, 1.0) == 4.0`; `(-1, 1) == -5`.
- `load(store(x))` round-trips for every struct.

## Verification (evidence required)

1. `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice/mods.rs src/dsp/tests/dsp/bass_mods.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_mods::/)' > tmp/logs/bass-12-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`, with at least 12 tests and 0 failed.

## Concurrency and Drift Protocol

- Owned files only; fresh reads.
- Record `shasum -a 256` at start and finish.
- Wait out and record sibling compile failures; never edit a sibling's
  file.

## Done Criteria

- [ ] Interface contract exact; `mods.rs` under 400 lines.
- [ ] Verification passes, with the exit code and test count recorded.
- [ ] Only writePaths changed.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none
