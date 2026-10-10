# FM1V-17: `gendyn-core` Kernel (Dynamic Stochastic Synthesis)

**Status**: Ready
**Plan ID**: FM1V-17 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` (Part 2 common rules; "GENDYN")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

This implements Xenakis's dynamic stochastic synthesis, following Formalized
Music and the GENDY3 analyses by Serra and Hoffmann. Each wave period is a
polygon of `P` breakpoints. Every period, each breakpoint's amplitude and
duration take a second-order bounded random walk.

At `gendyn-spread 0`, every period lasts exactly `1/freq`, so the pitch is
playable. A larger spread lets the pitch wander.

## Non-goals

- No registry work.
- No envelope inside the kernel; the template adds `env-adsr`.
- No free-running pitch mode other than through `gendyn-spread`.

## Dependencies

- **dependsOn**: FM1V-00
- **Blocks**: FM1V-30

## writePaths

- `src/dsp/ugen/gendyn.rs`
- `src/dsp/tests/dsp/gendyn.rs`
- `impl-plans/active/fm1-voices-17-gendyn.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-17` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/effects/prim.rs::Rng` (`unit`, `bipolar`).
- `src/dsp/ugen/modal_pair.rs:115-160`: the seeding pattern, with the seed
  stored in `st.u[0]`.

## File-level Changes (`src/dsp/ugen/gendyn.rs`)

**State** (in `mem`):

- per breakpoint: `amp[MAX_POINTS]`, `amp_vel[MAX_POINTS]`,
  `dur[MAX_POINTS]` and `dur_vel[MAX_POINTS]`, with `MAX_POINTS = 32`;
- the current segment index, the segment phase and the latched `P`;
- the DC-blocker state.

`STATE_FLOATS = 4*32 + 8`.

**Init** (first block):

- The RNG is `Rng::new(kx.seed ^ 0x4745_4E44)`.
- Amplitudes start as `bipolar() * 0.5`.
- Durations start equal, at `1/(freq*P)`.
- Velocities start at 0.

**Distributions.** `gendyn-dist` is rounded 0..=3. Each draw maps `u` from
`Rng::unit()`, clamped to `[1e-6, 1 - 1e-6]`, to a step in [-1, 1]:

| Value | Distribution | Mapping |
|------:|--------------|---------|
| 0 | uniform | `2u - 1` |
| 1 | Cauchy | `tan(pi(u - 0.5))`, clamped to +/-8, then divided by 8 |
| 2 | logistic | `ln(u/(1-u))`, clamped to +/-8, divided by 8 |
| 3 | hyperbolic cosine | `ln(tan(pi*u/2))`, clamped to +/-8, divided by 8 |

**Walks.** At each period start, for every breakpoint `i < P`:

- `amp_vel[i] += step * gendyn-amp-step`, mirrored into
  `[-amp-step, +amp-step]`;
- `amp[i] += amp_vel[i]`, mirrored into `[-1, 1]`;
- `dur_vel[i]` and `dur[i]` work the same way with `gendyn-dur-step`. The
  duration is mirrored into `[(1-s), (1+s)] * base`, where
  `base = 1/(freq*P)` and `s = gendyn-spread` clamped 0..=1.

**Pitch lock.** When `s == 0`, every `dur[i]` equals `base` exactly.

**Points.** `P = round(gendyn-points)` is clamped 3..=32 and latched at
period start.

**Rendering.** Interpolate linearly from breakpoint `i` to `i+1`, wrapping
`P-1 -> 0`, over `dur[i]` seconds. Output is DC-blocked at 10 Hz and
clamped.

## Pitfalls

- **Mirror, do not clamp.** The walks must reflect at the barriers, as
  Xenakis's model does. A clamp sticks at the boundary.
- **Pitch-lock exactness.** At `s == 0` the summed segment durations must
  equal `1/freq`. Track fractional sample time with an accumulated phase,
  not integer segment lengths, so there is no drift.
- **Never produce infinity.** `tan` near `pi/2` is the risk, hence the clamp
  on `u` and the +/-8 clamp.

## Tests (`src/dsp/tests/dsp/gendyn.rs`; names contain `gendyn`)

Keep the ports test. Add:

- `gendyn_deterministic_per_seed`: the same seed renders bit-identically,
  and a different seed renders differently.
- `gendyn_spread_zero_locks_period`. At f=200, spread 0 and 48 kHz, take
  the zero-crossing-based period estimate of the DC-blocked output, or the
  autocorrelation over 1 s. It equals 240 samples +/- 1.
- `gendyn_spread_increases_period_variance`: the per-period length variance
  at spread 0.8 is greater than at spread 0.
- `gendyn_every_distribution_is_finite`: dist 0..=3, each with amp-step 1
  and dur-step 1, for 2 s. Every sample is finite and `|y| <= 1`.
- `gendyn_amp_step_zero_freezes_waveform_after_first_period`. With amp-step
  0, dur-step 0 and spread 0, consecutive periods are identical within 1e-6.
- `gendyn_block_size_independent`.
- `gendyn_extreme_grid_is_finite_and_bounded`.
- `gendyn_render_does_not_allocate`.

## Verification (logs under `tmp/fm1-voices/FM1V-17/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/gendyn.rs src/dsp/tests/dsp/gendyn.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/gendyn/)' > tmp/fm1-voices/FM1V-17/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 9 tests run and 0 failed.
3. `git diff --stat` shows only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

You own only your files. If a build fails in another plan's file, wait and
re-run.

## Done Criteria

- [ ] The kernel is implemented.
- [ ] Verification 1-3 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.
