# FM1V-16: `vosim-core` Kernel (VOSIM Pulse Trains)

**Status**: Ready
**Plan ID**: FM1V-16 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` (Part 2 common rules; "VOSIM")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The user asked for experimental oscillators from published papers. VOSIM
(Kaegi and Tempelaars, JAES 1978) builds each fundamental period from:

- `N` sin-squared pulses of width `T = 1/formant`;
- a geometric amplitude decay across those pulses;
- silence for the rest of the period.

This is an original implementation from the paper. It does not replace the
existing Braids-position VOSIM approximation in `macro-formant-voice`.

## Non-goals

- No registry work.
- No envelope inside the kernel: the template adds `env-adsr` (FM1V-30).
- Do not change `braids_formant.rs`.

## Dependencies

- **dependsOn**: FM1V-00
- **Blocks**: FM1V-30

## writePaths

- `src/dsp/ugen/vosim.rs`
- `src/dsp/tests/dsp/vosim.rs`
- `impl-plans/active/fm1-voices-16-vosim.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-16` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/ugen/fm.rs::op`: phase accumulation and clamping style.
- `src/dsp/tests/dsp/bass_voice.rs`: render helpers.

## File-level Changes (`src/dsp/ugen/vosim.rs`)

**State** (in `mem`, `STATE_FLOATS` at most 8):

- period phase `p` in [0,1);
- current-period `N` and `T` (latched);
- the DC-blocker state.

**Per sample:**

1. The period is `P = 1/freq`. Clamp `freq` to 20..=8000 and replace
   non-finite values with 440.
2. At each period start (`p` wraps), latch:
   - `N = round(vosim-pulses)`, clamped 1..=8;
   - `T = 1/vosim-formant`, with the formant clamped 100..=8000;
   - if `N*T > P`, set `T = P/N`.
3. `t = p * P` is the time into the period. Pulse index `k = floor(t/T)`.
   - If `k < N`, `y = vosim-decay^k * sin^2(pi * (t - k*T)/T)`, with the
     decay clamped 0..=1.
   - Otherwise `y = 0`.
4. `vosim-formant` is re-read every sample for `T` only while it does not
   violate `N*T <= P`. This allows smooth sweeps. Pin the behaviour exactly
   as follows:
   - recompute `T` per sample from the current formant;
   - clamp it to `P/N` using the latched `N`.
5. Output `dc_block(y)`: a one-pole high-pass at 10 Hz, clamped to `[-1, 1]`.

## Pitfalls

- **Do not pre-normalize by N.** The level is set by the template's `amp`.
  `sin^2` gives 0..1, and the DC blocker centres it.
- **`vosim-pulses` is a float control.** Round it, and only at period start,
  to avoid mid-period discontinuities.

## Tests (`src/dsp/tests/dsp/vosim.rs`; names contain `vosim`)

Keep the ports test. Add:

- `vosim_fundamental_matches_freq`. At f=220, formant 900 and pulses 3, the
  autocorrelation pitch is within 1% of 220.
- `vosim_formant_peak_tracks_control`. With f=110, pulses 8 and decay 1,
  the strongest spectral region above 300 Hz has its centroid within 15% of
  the formant. Check formant 800 and formant 1600.
- `vosim_single_pulse_ignores_decay`: pulses 1 is bit-identical for
  decay 0.2 and decay 0.9.
- `vosim_pulse_width_clamps_to_period`: formant 100 at f=8000 is finite and
  periodic at `sr/8000` samples, +/- 1.
- `vosim_block_size_independent_and_deterministic`.
- `vosim_extreme_grid_is_finite_and_bounded`.
- `vosim_render_does_not_allocate`.

## Verification (logs under `tmp/fm1-voices/FM1V-16/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/vosim.rs src/dsp/tests/dsp/vosim.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/vosim/)' > tmp/fm1-voices/FM1V-16/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 8 tests run and 0 failed.
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
