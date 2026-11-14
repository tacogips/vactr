# FM1V-16: `vosim-core` Kernel (VOSIM Pulse Trains)

**Status**: Completed
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
- current-period `N` (latched), with `T` recomputed per sample from the
  current formant and clamped to `P/N`;
- the DC-blocker state.

**Per sample:**

1. The period is `P = 1/freq`. Clamp `freq` to 20..=8000 and replace
   non-finite values with 440.
2. At each period start (`p` wraps), latch:
   - `N = round(vosim-pulses)`, clamped 1..=8;
   - initialize `T = 1/vosim-formant`, with the formant clamped 100..=8000;
   - if `N*T > P`, set `T = P/N`. Recompute `T` from the current formant
     each sample, using the latched `N` and the same `P/N` clamp.
3. `t = p * P` is the time into the period. Pulse index `k = floor(t/T)`.
   - If `k < N`, `y = vosim-decay^k * sin^2(pi * (t - k*T)/T)`, with the
     decay clamped 0..=1.
   - Otherwise `y = 0`.
4. `vosim-formant` is re-read every sample for `T` and clamped to `P/N` using
   the latched `N`, preserving `N*T <= P` while allowing smooth sweeps.
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
- `vosim_formant_region_centroid_tracks_control`. With f=110, pulses 8 and
  decay 1, the strongest spectral region above 300 Hz has its centroid within
  15% of the formant. Check formant 800 and formant 1600.
- `vosim_single_pulse_ignores_decay`: pulses 1 is bit-identical for
  decay 0.2 and decay 0.9.
- `vosim_pulse_width_clamps_to_period`: formant 100 at f=8000 is finite and
  periodic at `sr/8000` samples, +/- 1; its settled window (`samples[4_000..]`)
  also has peak-to-peak amplitude greater than 0.3 to detect a missing `N*T <= P`
  width clamp.
- `vosim_dc_blocker_centres_output`: at `[220, 900, 3, 0.7]`, 48 kHz,
  48,000 frames and 256-frame blocks, the settled window (`samples[24_000..]`)
  has absolute mean below `1e-2` and minimum below `-0.05`.
- `vosim_block_size_independent_and_deterministic`.
- `vosim_extreme_grid_is_finite_and_bounded`.
- `vosim_render_does_not_allocate`.

## Verification (logs under `tmp/fm1-voices/FM1V-16/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/vosim.rs src/dsp/tests/dsp/vosim.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/vosim/)' > tmp/fm1-voices/FM1V-16/nextest-ti-repair.log 2>&1; echo "exit=$?"`
   must report `exitStatus=0`, with at least 9 tests run and 0 failed.
3. Inspect `git diff --stat` and the assigned-path status. The shared working
   tree may include other authorized fanout changes; this plan's own changes
   must remain within its declared writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

You own only your files. If a build fails in another plan's file, wait and
re-run.

## Done Criteria

- [x] The kernel is implemented.
- [x] Verification 1-3 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

### Session: 2026-10-10 (FM1V-16 implementation)
**Tasks Completed**: VOSIM kernel and focused tests.

- Implemented the VOSIM pulse train in `src/dsp/ugen/vosim.rs` with four
  floats of state: normalized period phase, period-latched pulse count, and
  the 10 Hz DC blocker input/output state. Pulse width follows the current
  formant each sample and clamps to `P/N`; output is finite and bounded.
- Preserved the port-contract test and added seven `vosim_*` behavior tests,
  including the strongest-region formant centroid check at 800 and 1600 Hz,
  block-size determinism and zero allocations.
- Final verification: `rustfmt --edition 2021 --check
  src/dsp/ugen/vosim.rs src/dsp/tests/dsp/vosim.rs` passed (exit 0; log
  `tmp/fm1-voices/FM1V-16/rustfmt-final-20261010-attempt2.log`). Focused
  `cargo nextest run -E 'test(/vosim/)'` passed (8 run, 8 passed, 0 failed;
  exit 0; `tmp/fm1-voices/FM1V-16/nextest-final-2.log`). `git diff --stat`
  exited 0 and its shared-tree output includes concurrent fanout changes;
  assigned-path status is limited to the VOSIM source/test and this plan.
- Earlier focused attempts are retained: `nextest.log` failed before test
  execution due to concurrent hurdy-gurdy compile errors; `nextest-rerun1.log`
  and `nextest-final.log` each ran 8 tests with 7 passed and one periodicity
  assertion failure while the DC blocker transient was settling. The final
  test renders longer and checks the settled region without relaxing its
  `1e-4` tolerance.

### Session: 2026-10-10 (Step 6 final-source verification)
**Tasks Completed**: confirmed FM1V-16 source contract and reran focused gates.

- Reviewed the kernel and tests against this plan and the accepted VOSIM design; `STATE_FLOATS` is 4, and the implementation retains period-latched pulse count, formant-driven width clamped to `P/N`, geometric decay, 10 Hz DC blocking, finite/bounded output, and zero-allocation coverage.
- Fresh final-source verification: `rustfmt --edition 2021 --check src/dsp/ugen/vosim.rs src/dsp/tests/dsp/vosim.rs` exited 0 (`tmp/fm1-voices/FM1V-16/rustfmt-step6-final.log`); `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/vosim/)'` exited 0 with 8 run, 8 passed, 0 failed (`tmp/fm1-voices/FM1V-16/nextest-step6-final.log`).
- Assigned paths remain the VOSIM kernel, VOSIM tests, and this plan. No Rust source edits were required during this Step 6 verification pass. Downstream FM1V-30 registry/template integration and formal review remain with their owning workflow steps.

### Session: 2026-10-10 (FM1V-16 test-integrity repairs)
**Tasks Completed**: FM1V-16-TI-01 and FM1V-16-TI-02; tests and plan only.

- Added a settled-window peak-to-peak assertion (`> 0.3`) to
  `vosim_pulse_width_clamps_to_period`, preserving its render arguments,
  finite/bounded assertion, and `1e-4` periodicity tolerance.
- Added `vosim_dc_blocker_centres_output`, which checks the settled output mean
  (`abs(mean) < 1e-2`) and negative excursion (`min < -0.05`) for the requested
  48 kHz render. The ports test and `src/dsp/ugen/vosim.rs` are unchanged.
- Fresh verification: `rustfmt --edition 2021 --check
  src/dsp/ugen/vosim.rs src/dsp/tests/dsp/vosim.rs` exited 0
  (`tmp/fm1-voices/FM1V-16/rustfmt-ti-repair.log`). Focused nextest with
  `test(/vosim/)` exited 0, 9 run, 9 passed, 0 failed
  (`tmp/fm1-voices/FM1V-16/nextest-ti-repair.log`).
- These repairs are implemented and verified; independent test-integrity and
  adversarial review remain downstream.
