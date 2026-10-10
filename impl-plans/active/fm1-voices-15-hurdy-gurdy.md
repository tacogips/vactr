# FM1V-15: `hurdy-gurdy-core` Kernel (Bowed Waveguide, Drones, Buzz Bridge)

**Status**: Completed
**Plan ID**: FM1V-15 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` (Part 2 common rules; "Hurdy-gurdy")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The user asked for a hurdy-gurdy voice. It needs:

- a bowed melody string (the chanterelle);
- a bourdon drone and a fifth drone;
- a trompette string with a buzzing bridge (chien), whose buzz rhythm
  follows wheel-speed strokes (coup de poignet).

The design chose a **new bowed digital waveguide**. The existing
Elements-style bow is a free-running `tanh` sine, so it cannot produce
Helmholtz motion. The friction model is the McIntyre-Schumacher-Woodhouse
model in the form Smith gives in PASP. Implement the equations directly; do
not use STK or any other code.

## Non-goals

- No registry work. FM1V-30 adds the kernel to `wants_tempo_anchor`.
- No cross-voice drone.
- No thermal friction.
- Do not modify `braids_physical.rs`, `string_pair.rs` or
  `elements_internal.rs`.
- Do not read FM-1 firmware code.

## Dependencies

- **dependsOn**: FM1V-00
- **Blocks**: FM1V-30

## writePaths

- `src/dsp/ugen/hurdy_gurdy.rs`
- `src/dsp/ugen/hurdy_gurdy/bowed.rs`
- `src/dsp/ugen/hurdy_gurdy/buzz.rs`
- `src/dsp/tests/dsp/hurdy_gurdy.rs`
- `impl-plans/active/fm1-voices-15-hurdy-gurdy.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-15` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/ugen/rings_part.rs:17-23`: `line_len`/`mem_len` sizing
  (`sr/20 + guard`).
- `src/dsp/effects/prim.rs`: `DelayLine::carve`/`write`/`read`
  (fractional), `OnePole`, `Rng`.
- `src/dsp/ugen/braids_physical.rs:76-116`: a delay-coupled bow loop. Use it
  for structure only; the friction curve here is different.
- `src/dsp/ugen/bass_voice/mods.rs::gate_samples`: gate-length to samples.
- `src/dsp/ugen/bass_voice/mods.rs::initial_phase`: tempo-synced phase from
  `onset-time`, `cps` and a rate.

## File-level Changes

### `hurdy_gurdy/bowed.rs` (`pub(super)`)

`BowedString` holds:

- a delay line for the string loop, of length `sr / f` with linear
  fractional read and `f >= 20`, split into bridge-side and nut-side
  sections at relative bow position 0.12;
- a one-pole loss filter;
- state floats.

API:

- `fn mem_floats(sr) -> usize`
- `fn tick(&mut self, mem, bow_velocity, bow_force, f, sr) -> (string_velocity_at_bridge: f32)`

Friction:

- `dv = v_bow - v_string_at_bow`.
- Friction force `phi(dv) = force * sign(dv) * (mu_d + (mu_s - mu_d) / (1 + |dv|/v0))`.
  This is the hyperbolic curve.
- Use `mu_s = 0.8`, `mu_d = 0.3` and `v0 = 0.1`, documented as tuned
  constants.
- At `force == 0` or `v_bow == 0`, no energy is injected.
- Solve the coupling with the standard explicit waveguide approximation:
  evaluate `phi` with the previous sample's string velocity. Document that
  choice.

### `hurdy_gurdy/buzz.rs` (`pub(super)`)

- **Stroke envelope.** `fn stroke(phase: f32) -> f32` is a raised-cosine
  pulse train value in 0..1 with a 20% duty cycle.
- **Rattle.** `Rattle` is a one-sided impact model:
  - the bridge contacts when the trompette bridge force exceeds
    `gap = (1 - clamp(w - threshold, 0, 1)) * 0.5`;
  - each contact emits an impulse proportional to the excess;
  - the impulse decays through a 2-pole resonator at 2.5 kHz with Q 3;
  - the output is scaled by `gurdy-buzz`.
  - When `gurdy-buzz == 0`, the rattle is not evaluated (exact bypass).

### `hurdy_gurdy.rs`

**Strings.** Four `BowedString`s:

| String | Frequency | Level control |
|--------|-----------|---------------|
| chanterelle | `freq` | `gurdy-melody` |
| bourdon | MIDI key `gurdy-drone-key` (24..=72) | `gurdy-bourdon` |
| fifth | key + 7 | `gurdy-fifth` |
| trompette | key + 12 | `gurdy-trompette` |

A level of exactly 0 skips that string's tick.

**Wheel.**

- `w = gurdy-wheel * (1 + gurdy-stroke-depth * stroke(phase))`.
- `phase` advances at `gurdy-strokes * cps` Hz. Its initial phase comes from
  `onset-time` the same way bass does, so strokes are tempo-locked.
- `gurdy-strokes == 0` gives a steady wheel.
- The bow velocity of every string is `w * 0.5`. The bow force is
  `gurdy-pressure`.

**Buzz.** The rattle is driven by the trompette's bridge velocity, gated by
`w > gurdy-buzz-threshold`.

**Note length.**

- `gate-length` (clamped 0.05..=64) with `cps` gives the bowing time.
- After it ends, `w` ramps to 0 over 10 ms and the strings decay.
- `st.finish()` is called once the summed string energy is below `1e-8`.

**Output.**

- `velocity * (sum of strings + buzz)`, times a fixed normalization so the
  default note peaks between 0.3 and 0.9.
- Clamp to `[-1, 1]`.
- A non-finite string state resets that string.

**Memory.** `STATE_FLOATS` covers four lines at `line_len(96_000)`, a fixed
bound, plus the scalars. It must stay at or below 24_000 floats at 48 kHz,
so that it fits the 24_000-float test budget. Compute the bound from the
96 kHz worst case at 20 Hz: `96000/20 = 4800` per line, which is 19_200 plus
guards. Document it.

## Pitfalls

- **Bowing must self-oscillate.** Sustained Helmholtz-like motion at
  `freq` with no external periodic drive is the point. Do not cheat with an
  oscillator.
- **Drone keys are MIDI notes**, converted with `440 * 2^((k-69)/12)`.
  They are not affected by note `freq`.
- **Keep the memory bound fixed.** The template build needs a fixed
  `mem_need`.
- **Do not use `kx.gate` for note-off.** `gate-length` owns it.

## Tests (`src/dsp/tests/dsp/hurdy_gurdy.rs`; names contain `gurdy`)

Keep the ports test. Add:

- `gurdy_bowed_chanterelle_pitch_tracks_freq`. Melody only (drones 0) at
  f=330, wheel 0.5, pressure 0.5. The autocorrelation pitch over
  0.3..1.0 s is within 1% of 330.
- `gurdy_no_wheel_no_sustain`. With wheel 0, the RMS over 0.2..1 s is below
  1e-5.
- `gurdy_drones_sound_at_key_and_fifth`. Melody 0, drone key 43, bourdon 1,
  fifth 1, trompette 0. The spectrum has peaks within 1% of 98.0 and
  146.8 Hz.
- `gurdy_buzz_rises_above_threshold`. Trompette only, buzz 1, threshold
  0.5. The 3-8 kHz energy at wheel 0.9 is more than 4 times the energy at
  wheel 0.3.
- `gurdy_strokes_are_tempo_locked`. Use strokes 4, cps 0.5, buzz 1, wheel
  0.6, stroke-depth 1. The autocorrelation of the 5 ms RMS of the
  high-passed (>3 kHz) output peaks at `1/(4*0.5) = 0.5 s` +/- 2%.
- `gurdy_buzz_zero_is_exact_bypass`.
- `gurdy_gate_length_owns_note_off_and_finishes`.
- `gurdy_block_size_independent_and_deterministic`.
- `gurdy_extreme_grid_is_finite_and_bounded`. f in {20, 2000}, every
  control at its range ends, at 44.1, 48 and 96 kHz.
- `gurdy_state_fits_test_budget`: `STATE_FLOATS <= 24_000`.
- `gurdy_render_does_not_allocate`.

## Verification (logs under `tmp/fm1-voices/FM1V-15/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/hurdy_gurdy.rs src/dsp/ugen/hurdy_gurdy/bowed.rs src/dsp/ugen/hurdy_gurdy/buzz.rs src/dsp/tests/dsp/hurdy_gurdy.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/gurdy/)' > tmp/fm1-voices/FM1V-15/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 12 tests run and 0 failed.
3. Each kernel file is under 400 lines.
4. `git diff --stat` shows only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

You own only your files. If a build fails in another plan's file, wait and
re-run.

## Done Criteria

- [x] The kernel is implemented, with the fixed `STATE_FLOATS`.
- [x] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

### Session: 2026-10-10 (FM1V-15 implementation)
**Tasks Completed**: Four-string bowed waveguide, tempo-locked wheel strokes,
threshold-gated chien impact rattle, gate-length note lifetime, fixed state,
and the plan's behavioral tests.

**Verification**:
- `rustfmt --edition 2021 --check src/dsp/ugen/hurdy_gurdy.rs src/dsp/ugen/hurdy_gurdy/bowed.rs src/dsp/ugen/hurdy_gurdy/buzz.rs src/dsp/tests/dsp/hurdy_gurdy.rs` exited 0 (`tmp/fm1-voices/FM1V-15/rustfmt-final.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/gurdy/)'` exited 0: 12 run, 12 passed, 0 failed (`tmp/fm1-voices/FM1V-15/nextest-attempt-8.log`). Independent rerun also exited 0: 12 run, 12 passed, 0 failed (`tmp/fm1-voices/FM1V-15/nextest-independent-20261010-rerun.log`).
- Kernel line counts: 219, 114, 91; each kernel is under 400 lines. The test file is 468 lines, under the repository's 1000-line Rust source limit.
- `git diff --stat -- src/dsp/ugen/hurdy_gurdy.rs src/dsp/ugen/hurdy_gurdy/bowed.rs src/dsp/ugen/hurdy_gurdy/buzz.rs src/dsp/tests/dsp/hurdy_gurdy.rs` is empty because these source files are new and untracked. Scoped `git status --short` lists only these four source files and this plan; no files were staged.

Attempts 1-7 did not pass during tuning and their logs remain preserved under
`tmp/fm1-voices/FM1V-15/`; attempt 8 and the independent rerun pass on the
final source. Formal review and workflow finalization remain downstream.

### Session: 2026-10-10 (test-integrity repairs)
**Tasks Completed**: Repaired the three falsifiability findings from the
independent test-integrity review.

- F15-TI-1 (`gurdy_strokes_are_tempo_locked`): uses a continuous >3 kHz
  high-pass followed by 5 ms RMS windows, mean-centered autocorrelation over
  0.30..0.70 s, the unchanged +/-2% tolerance, and a `gurdy-strokes 0`
  control requiring over 2x envelope variance.
- F15-TI-2 (`gurdy_buzz_zero_is_exact_bypass`): drives trompette at 1.0 and
  wheel at 0.9; asserts buzz 0 output is identical across thresholds 0 and 1,
  and differs from buzz 0.8 output.
- F15-TI-3 (`gurdy_drones_sound_at_key_and_fifth`): scans 85..115 Hz and
  125..170 Hz at 0.1 Hz increments, retaining the 1% peak tolerances.
  The widened drone test passes without kernel changes.

**Verification**:
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/gurdy/)' > tmp/fm1-voices/FM1V-15/nextest-ti-repair.log 2>&1; result=$?; cat tmp/fm1-voices/FM1V-15/nextest-ti-repair.log; printf 'exit=%s\\n' "$result"; exit "$result"` exited 0: 12 run, 12 passed, 0 failed.
- Targeted repaired tests exited 0: 3 run, 3 passed (`tmp/fm1-voices/FM1V-15/ti-targeted.log`). The widened drone test also passed before other test edits (`tmp/fm1-voices/FM1V-15/drone-wideband-repro.log`).
- `rustfmt --edition 2021 --check src/dsp/ugen/hurdy_gurdy.rs src/dsp/ugen/hurdy_gurdy/bowed.rs src/dsp/ugen/hurdy_gurdy/buzz.rs src/dsp/tests/dsp/hurdy_gurdy.rs` exited 0 after test edits.

The independent reviewer must re-review F15-TI-1, F15-TI-2 and F15-TI-3;
this progress record does not claim review acceptance.
