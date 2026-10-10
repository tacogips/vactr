# FM1V-18: `scanned-core` Kernel (Scanned Synthesis)

**Status**: Completed (accepted at 25d3e80; header corrected in session 352)
**Plan ID**: FM1V-18 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` (Part 2 common rules; "Scanned synthesis")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

This implements scanned synthesis (Verplank, Mathews and Shaw, ICMC 2000):

- A closed ring of 64 masses with springs evolves slowly, at haptic rate.
- Its shape is read around the ring at audio rate, once per period at
  `freq`.

The pitch comes from the scan rate. The timbre evolves as the ring
relaxes after the onset excitation.

## Non-goals

- No registry work.
- No envelope inside the kernel; the template adds `env-adsr`.
- No user-drawn excitation, and no non-circular scan paths.

## Dependencies

- **dependsOn**: FM1V-00
- **Blocks**: FM1V-30

## writePaths

- `src/dsp/ugen/scanned.rs`
- `src/dsp/tests/dsp/scanned.rs`
- `impl-plans/active/fm1-voices-18-scanned.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-18` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/ugen/fm.rs::op`: phase accumulation.
- `src/dsp/tests/dsp/bass_voice.rs`: render helpers, and the centroid
  helper.

## File-level Changes (`src/dsp/ugen/scanned.rs`)

**Constants and state.** `pub const MASSES: usize = 64;`. State in `mem`:

- `pos[64]`, `vel[64]` and `prev[64]` (the shape at the previous update);
- the scan phase;
- the update countdown in samples;
- the onset peak;
- the DC-blocker state.

`STATE_FLOATS = 3*64 + 8`.

**Onset** (first block): `pos[i] = 0.5 * (1 + cos(pi * d / w))` when
`d < w`, else 0, where:

- `d` is the circular distance in masses from `scan-position * 64`;
- `w = scan-hammer * 32`, clamped to at least 1.

All velocities are 0. `prev = pos`. The onset peak is the maximum `|pos|`.

**Dynamics.** Every `round(sr / scan-update)` samples, with `scan-update`
clamped 50..=2000:

1. Copy `pos` into `prev`.
2. For each mass `i`:
   - `a = k*(pos[i-1] + pos[i+1] - 2 pos[i]) - c*pos[i] - d*vel[i]`, with
     indices taken modulo 64;
   - `k = 0.05 + 0.55 * scan-stiffness`, so `k <= 0.60`;
   - `c = 0.2 * scan-centering`, so `c <= 0.2`;
   - `d = 0.01 + 0.5 * scan-damping`, so `d <= 0.51`.

   Clamp each control to 0..=1 first.
3. Apply semi-implicit Euler with `dt = 1` per update:
   `vel += a; pos += vel`.
   - Compute every mass's `a` from the old `pos` before updating any mass.
     This is a Jacobi update, not an in-place Gauss-Seidel sweep.
4. **Stability inequality** (pinned): `w2_max = 4k + c < 4 - 2d` must hold
   at every control combination.
   - Derivation: per ring mode with `w2` = (Laplacian eigenvalue times `k`)
     + `c`, the update matrix has trace `2 - w2 - d` and det `1 - d`.
   - On a 64-mass ring the largest Laplacian eigenvalue is 4, from the
     alternating N/2 mode.
   - Under the mapping above, the worst corner is stiffness 1, centering 1,
     damping 1: `4*0.60 + 0.2 = 2.6 < 4 - 2*0.51 = 2.98`, a margin of at
     least 0.1 at every corner.
   - Bounding `k` alone (for example `k <= 0.95`) is NOT sufficient. Do not
     use the old mapping.
   - Expose the mapping as
     `pub(crate) fn coefficients(stiffness: f32, centering: f32, damping: f32) -> (f32, f32, f32)`,
     returning `(k, c, d)`, so the bound test can check it.

**Scan.**

- The phase advances by `freq/sr`, with `freq` clamped 20..=8000.
- The sample is a cubic (Catmull-Rom) interpolation around the ring at
  `phase * 64`. It reads a blend of `prev` and `pos` with
  `t = 1 - countdown/interval`, so the shape crossfades between updates.

**Output.** `dc_block(y / max(onset_peak, 1e-6))`, clamped to `[-1, 1]`.

## Pitfalls

- **Block-size independence.** The update countdown must persist across
  blocks.
- **Keep the integration stable.** At stiffness 1, damping 0 and centering 0,
  a 10 s render must stay finite and bounded. Test this.
- **The bound covers every control corner, not just the defaults.** A narrow
  hammer (`scan-hammer` near 0.02, about 1 mass wide) strongly excites the
  alternating N/2 mode, the one with Laplacian eigenvalue 4. So
  `4k + c < 4 - 2d` must hold for every combination of stiffness, centering
  and damping. Keep the margin of at least 0.1. Never retune `k` upward
  without re-checking it.
- **No per-sample `Vec`.** Work in place on `mem` slices.

## Tests (`src/dsp/tests/dsp/scanned.rs`; names contain `scanned`)

Keep the ports test. Add:

- `scanned_pitch_matches_freq`: at f=220, the autocorrelation pitch over
  0.1..0.5 s is within 1% of 220.
- `scanned_timbre_evolves`: the spectral centroid of 0-100 ms differs from
  that of 400-500 ms by more than 10%.
- `scanned_stiffness_speeds_evolution`: the centroid change over the first
  200 ms is larger at stiffness 0.9 than at 0.1.
- `scanned_update_rate_changes_output`: renders at update 100 and 1000
  differ.
- `scanned_coefficients_satisfy_stability_bound`: for every corner of
  stiffness, centering and damping in {0, 1}, `coefficients(..)` returns
  `(k, c, d)` with `4k + c < 4 - 2d - 0.1`.
- `scanned_stability_grid_is_finite_and_bounded`: every control at its
  range ends, 10 s at stiffness 1 and damping 0, at 44.1, 48 and 96 kHz.
- `scanned_block_size_independent_and_deterministic`.
- `scanned_render_does_not_allocate`.

## Verification (logs under `tmp/fm1-voices/FM1V-18/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/scanned.rs src/dsp/tests/dsp/scanned.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/scanned/)' > tmp/fm1-voices/FM1V-18/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 9 tests run and 0 failed.
3. `git diff --stat` shows only writePaths.

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

### Session: 2026-10-10 (FM1V-18 implementation)
**Tasks Completed**: Implemented the 64-mass scanned-synthesis kernel and its eight behavior tests, preserving the FM1V-00 port contract.
**Implementation**: `STATE_FLOATS = 3 * 64 + 8`; raised-cosine onset; Jacobi semi-implicit Euler updates at rounded, clamped scan-update intervals; Catmull-Rom circular scan with previous/current shape crossfade; persistent phase/countdown and normalized DC-blocked output. The coefficient corner test proves `4k + c < 4 - 2d - 0.1`. The timbre test uses hammer `0.02`, centering `0.0`, and default stiffness/damping, retaining the exact 0–100 ms vs 400–500 ms windows and >10% centroid criterion. An RMS guard requires the late window to remain at least `1e-3` of the early window.
**Verification**:
- `rustfmt --edition 2021 --check src/dsp/ugen/scanned.rs src/dsp/tests/dsp/scanned.rs` — exit 0; log `tmp/fm1-voices/FM1V-18/rustfmt-final.log`.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/scanned/)'` — exit 0, 9 run / 9 passed / 0 failed; log `tmp/fm1-voices/FM1V-18/nextest-verified.log`.
- Scoped status showed only `src/dsp/ugen/scanned.rs`, `src/dsp/tests/dsp/scanned.rs`, and this plan file within FM1V-18 write paths; both Rust files are under 1000 lines.
**Superseded attempts**: `nextest.log` (exit 101) and `nextest-retry.log` (exit 101) stopped at compile errors in concurrent FM1V-11/12/15/16/17 files; owning branches repaired them. `nextest-final.log` (exit 100, 7 passed / 2 failed) exposed test-window/excitation issues; corrected as described above. `spectral-rerun.log` (exit 100, 1 passed / 1 failed) confirmed default and high-stiffness/zero-damping settings did not meet the requested timbre threshold. `timbre-probe-2.log` then passed the corrected timbre scenario (1/1); the final FM1V-18 filter passed all nine tests.

### Session: 2026-10-10 (test-integrity repairs F18-TI-1 and F18-TI-2)
**Tasks Completed**: Repaired the two mid-severity findings without changing the scanned kernel or weakening plan thresholds/windows.
**F18-TI-1**: Replaced the high damping/centering controls with hammer `0.02`, centering `0.0`, and default stiffness/damping. Added the late/early RMS guard (`late_rms >= 1e-3 * early_rms`) before the unchanged >10% centroid comparison. The integrity review's independent replica reported early/late centroids of approximately 652/38 bins and late/early RMS ratio approximately `1.2e-2`; the current-source test passes both guards.
**F18-TI-2**: Widened the pitch helper's lag search to the prescribed +/-50% range and made it return the best normalized correlation with the estimate. The test retains the 1% frequency tolerance and now requires best correlation >= 0.8 and half-period correlation < best - 0.3. The integrity review's independent replica reported lag 218 (220.18 Hz), best correlation 0.953, and half-period correlation -0.044; the current-source test passes these assertions.
**Verification**:
- `rustfmt --edition 2021 --check src/dsp/ugen/scanned.rs src/dsp/tests/dsp/scanned.rs` — exit 0; complete log `tmp/fm1-voices/FM1V-18/rustfmt-repair.log`.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/scanned/)' > tmp/fm1-voices/FM1V-18/nextest-repair.log 2>&1; echo "exit=$?"` — exit 0, 9 run / 9 passed / 0 failed; complete log `tmp/fm1-voices/FM1V-18/nextest-repair.log`.
- `git diff -- src/dsp/ugen/scanned.rs` — empty; kernel mtime remains `16:47:26`.
**Review Decision**: F18-TI-1 and F18-TI-2 are implemented and locally verified; independent integrity re-review remains pending.
