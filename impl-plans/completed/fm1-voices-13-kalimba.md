# FM1V-13: `kalimba-core` Kernel (Modal Tine Voice)

**Status**: Completed
**Plan ID**: FM1V-13 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` (Part 2 common rules; "Kalimba (`kalimba-core`, template `kalimba`)")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The user asked for a kalimba/mbira voice. Model it as modal synthesis:

- clamped-free beam tine modes, plus a beating twin of the fundamental;
- a half-sine pluck whose width sets the brightness;
- mode damping, two body resonances, and an optional bottle-cap buzz.

The code is original Rust written from the physics, with Fletcher and
Rossing's clamped-free bar modes as the basis. No kalimba implementation
source may be consulted, and no FM-1 firmware (FiMba-1 is GPL).

FM1V-00 pinned the port contract in `src/dsp/ugen/kalimba.rs`. Keep it.

## Non-goals

- No registry, template or metadata work (FM1V-30).
- Do not change `modal_pair.rs` or `effects/resonator_extra.rs`. Copy the
  complex-rotation recurrence idea and write it fresh here.
- No stereo output.

## Dependencies

- **dependsOn**: FM1V-00
- **Blocks**: FM1V-30

## writePaths

- `src/dsp/ugen/kalimba.rs`
- `src/dsp/tests/dsp/kalimba.rs`
- `impl-plans/active/fm1-voices-13-kalimba.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-13` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/effects/resonator_extra.rs:73-99`: the complex-rotation mode
  oscillator. Radius is `exp(-3 ln10 / (sr * T60))`.
- `src/dsp/ugen/modal_pair.rs:115-160`: first-block detection, and seeding
  `prim::Rng` from `kx.seed` into `st.u[0]`.
- `src/dsp/effects/prim.rs`: `Rng`, `Biquad`/`Shape::Bandpass`, `OnePole`.
- `src/dsp/tests/dsp/bass_voice.rs`: the helpers `voice_inputs`,
  `render_voice`, `rms`, `peak`, plus the extreme-grid test pattern.
- `crate::dsp::offline::spectrum(frames, bins)` and
  `crate::dsp::alloc_probe::armed`.

## File-level Changes (`src/dsp/ugen/kalimba.rs`)

All state lives in `mem[..STATE_FLOATS]`. Set the real `STATE_FLOATS` and
keep it at or below 64. The per-sample loop is fixed-bound and allocation
free.

**Modes:**

- Ratios `[1.0, 6.2669, 17.5475, 34.3861]` at `f0 = freq`.
- A twin mode at `f0 + kalimba-beat`, gain 0.6 relative to the fundamental.
- Mode input gains are the tuned constants `[1.0, 0.45, 0.2, 0.1]`, with
  the twin using the fundamental's gain. Document them as original tuning.
- Skip any mode at or above `0.45 * sr`: it contributes exactly 0 and is
  never updated.

**Decay:**

- Fundamental T60 is `kalimba-decay`, clamped to 0.1..=10 s.
- Mode n uses `T60_n = decay / ratio_n^p`, with
  `p = 0.5 + 1.5 * kalimba-damping`.
- The twin uses the fundamental's T60.

**Pluck:**

- At voice start, a half-sine force pulse of width
  `w = exp(ln(6e-3) + hardness * (ln(0.5e-3) - ln(6e-3)))` seconds, with
  hardness clamped to 0..1.
- The pulse amplitude is `velocity`, clamped to 0..1.
- The pulse drives every mode for `ceil(w * sr)` samples. It is generated
  from a sample counter kept in state, so it is block-size independent.

**Body:**

- Two fixed two-pole band-pass resonators, at 220 Hz (Q 6) and 650 Hz
  (Q 4), driven by the sum of the tines.
- `out = tines + kalimba-body * (b220 + b650)`.

**Buzz:**

- Gate: `g = max(0, |x_fund| - 0.25 * x_ref)`, where `x_ref` is the peak of
  the fundamental during the pluck window, held in state.
- Seeded white noise from `prim::Rng` (`kx.seed ^ 0x4B41_4C49`), band-passed
  at 4 kHz with Q 2, times `g * kalimba-buzz * 4`.
- When `kalimba-buzz == 0.0`, skip the buzz path entirely, including the
  RNG draws, so the output is bit-identical to a no-buzz render.

**Output:**

- Scale by a fixed normalization constant chosen so that the default C4
  pluck peaks between 0.4 and 0.9.
- Clamp to `[-1, 1]`.
- If any state value is non-finite, zero that component.

**Lifetime:**

- `kx.gate` is ignored.
- After the pluck window, when the summed squared mode states drop below
  `1e-8`, which is -80 dB, call `st.finish()`.

## Pitfalls

- **Read controls once per block.** Read `freq` and the controls from
  `ins[i].first()` once per block, so that a mid-note change of
  `kalimba-beat` takes effect at the next block.
- **Do not reseed per block.** The RNG state persists in `st.u[0]`.
- **Leave unused fields alone.** `NodeState` fields other than `u[0]`,
  `u[1]` and `u[3]` (finish) must not be touched, because seeds stay stable.
- **Sample rates.** The kernel must work at 44.1, 48 and 96 kHz. All time
  constants are in seconds.

## Tests (`src/dsp/tests/dsp/kalimba.rs`; names contain `kalimba`)

Keep the FM1V-00 ports test. Add:

- `kalimba_mode_peaks_follow_beam_ratios`. Render at f=440, hardness 1,
  decay 4, damping 0, body 0, beat 0, at 48 kHz for 1 s. The spectrum has
  local maxima within 1% of 440, 2757.4 and 7720.9 Hz, and of 15130 Hz too
  because it is below Nyquist.
- `kalimba_beat_period_matches_control`. Use beat 2.0 and damping 1. The
  autocorrelation peak lag of the 10 ms RMS envelope over 0.5..3 s is
  0.5 s +/- 5%.
- `kalimba_damping_shortens_upper_modes`. Compare damping 0 with damping 1.
  The energy in the 2.5-3 kHz band at 300 ms, relative to 0-20 ms, is
  lower with damping 1.
- `kalimba_hardness_brightens`: the spectral centroid of the first 50 ms
  is higher with hardness 1 than with hardness 0.
- `kalimba_buzz_zero_is_exact_bypass_and_buzz_adds_highs`:
  - buzz 0 is bit-identical to a second render with buzz 0;
  - buzz 0 is bit-identical to a render built without the buzz branch;
    compare against a render with body and buzz both 0 under the same seed;
  - buzz 1 has more 3-6 kHz energy over 0-200 ms than buzz 0.
- `kalimba_finishes_and_is_deterministic`:
  - `st.done()` becomes true within `decay * 3` seconds at decay 1;
  - two renders with the same seed are bit-identical.
- `kalimba_block_size_independent`: 64-frame and 256-frame block renders
  are bit-identical.
- `kalimba_extreme_grid_is_finite_and_bounded`. Cover f in
  {20, 440, 8000, 20000}, beat {0, 8}, hardness {0, 1}, decay {0.1, 10},
  damping {0, 1}, body {0, 1}, buzz {0, 1} and velocity {0, 1}. Every
  sample is finite with `|y| <= 1`.
- `kalimba_render_does_not_allocate`: `alloc_probe::armed` around one
  256-frame render gives 0 allocations.

## Verification (logs under `tmp/fm1-voices/FM1V-13/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/kalimba.rs src/dsp/tests/dsp/kalimba.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/kalimba/)' > tmp/fm1-voices/FM1V-13/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 10 tests run and 0 failed.
3. `wc -l` of both files is below 400 and 1000 respectively.
4. `git diff --stat` shows only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

You own only your files. If a build fails in another plan's file, wait and
re-run; never edit it. Fresh-read before each edit.

## Done Criteria

- [x] The kernel is implemented to the model above, with the real
      `STATE_FLOATS`.
- [x] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

### Session: 2026-10-10 (FM1V-13 implementation)
**Tasks Completed**: Implemented the five-mode kalimba kernel and all assigned behavior tests.
**Notes**: `STATE_FLOATS` is 24; modes use the specified beam ratios, beating twin, half-sine pluck, mode-dependent T60, two body band-pass resonators, seeded optional buzz, per-sample finite-state cleanup, and block-independent finish detection. Preserved the pinned ports test. No registry or template work was performed.
**Verification**:
- `rustfmt --edition 2021 --check src/dsp/ugen/kalimba.rs src/dsp/tests/dsp/kalimba.rs` — exit 0; `tmp/fm1-voices/FM1V-13/rustfmt-final.log`.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/kalimba/)'` — exit 0, 11 run / 11 passed / 0 failed; `tmp/fm1-voices/FM1V-13/nextest-final.log`.
- Assigned-path scope and size audit — exit 0; only this plan's two Rust files are changed in its source paths; kernel 211 lines, tests 366 lines; `tmp/fm1-voices/FM1V-13/scope-and-lines-final.log`. The shared worktree also contains other plans' concurrent changes.
- Earlier failed focused attempts are retained in `tmp/fm1-voices/FM1V-13/nextest.log` and `nextest-attempt-02.log` through `nextest-attempt-05.log`; the final current-tree run passes after the corrections.

### Session: 2026-10-10 (FM1V-13 test-integrity repair)
**Finding Repaired**: FM1V-13-TI-01 — `kalimba_default_c4_peak_is_normalized` now renders C4 at 261.6256 Hz; the default controls and 0.4..=0.9 assertion are unchanged. Lowered `NORMALIZATION` to 160.0; a standalone call to the production renderer measured C4 peak 0.752829.
**Verification**:
- `rustfmt --edition 2021 --check src/dsp/ugen/kalimba.rs src/dsp/tests/dsp/kalimba.rs` — exit 0; `tmp/fm1-voices/FM1V-13/rustfmt-repair.log`.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/kalimba/)'` — exit 0, 11 run / 11 passed / 0 failed; `tmp/fm1-voices/FM1V-13/nextest-repair.log`.
- `CARGO_TERM_QUIET=true cargo build --lib` — exit 0; `tmp/fm1-voices/FM1V-13/cargo-build-repair.log`.
- `rustc --edition 2021 tmp/fm1-voices/FM1V-13/peak_probe.rs --extern vactr=target/debug/deps/libvactr.rlib -L dependency=target/debug/deps -o tmp/fm1-voices/FM1V-13/peak_probe` and the probe — both exit 0; `tmp/fm1-voices/FM1V-13/peak-probe-build.log` and `peak-probe.log`.
- Line limits and preservation of the pinned `PORTS` table/ports test verified; `tmp/fm1-voices/FM1V-13/line-limits-repair.log`.
- Byte-equivalence audit confirms the pinned `PORTS` table and port test are unchanged from their FM1V-00 scaffold; `tmp/fm1-voices/FM1V-13/port-preservation-repair-02.log`.
**Review Status**: Repair evidence is complete; independent re-review remains pending.

### Session: 2026-10-10 (FM1V-13 adversarial repair)
**Finding Repaired**: FM1V-13-ADV-01 — finish detection now compares mode energy with `FINISH_ENERGY = 1.0e-8 / (NORMALIZATION * NORMALIZATION)`, keeping the pluck-window guard. Added `kalimba_finish_is_below_minus_80_db`; no existing tests, constants or ports were changed.
**Measured Output**: Default C4, seed 33, 48 kHz: peak 0.752829; finish at frame 150272 (3.130667 s); maximum absolute output in the final 480 samples 0.000072633 (<1e-4).
**Verification**:
- `rustfmt --edition 2021 --check src/dsp/ugen/kalimba.rs src/dsp/tests/dsp/kalimba.rs` — exit 0; `tmp/fm1-voices/FM1V-13/rustfmt-adv01-verifier.log`.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/kalimba/)'` — exit 0, 12 run / 12 passed / 0 failed; `tmp/fm1-voices/FM1V-13/nextest-adv01.log`.
- Production renderer measurement — exit 0; `tmp/fm1-voices/FM1V-13/peak-probe-build-adv01.log` and `tmp/fm1-voices/FM1V-13/peak-probe-adv01.log`.
- Kernel and test file sizes are 213 and 407 lines; pinned ports table and test remain byte-identical; `tmp/fm1-voices/FM1V-13/adv01-progress.md` and `tmp/fm1-voices/FM1V-13/port-preservation-repair-02.log`.
**Review Status**: Implementation repair evidence is complete; independent adversarial re-review remains pending.
**Post-format Verification**: Check-and-test-after-modify confirmed the probe formatting, rebuild and measurement, and reran the current-source kalimba suite. A first exact-file probe rustfmt check failed on formatting only; formatting the probe resolved it.
- `rustfmt --edition 2021 --check tmp/fm1-voices/FM1V-13/peak_probe.rs` — exit 0; `tmp/fm1-voices/FM1V-13/rustfmt-adv01-probe-final.log`.
- `rustc --edition=2021 tmp/fm1-voices/FM1V-13/peak_probe.rs --extern vactr=target/debug/deps/libvactr.rlib -L dependency=target/debug/deps -o target/debug/peak_probe` — exit 0; `tmp/fm1-voices/FM1V-13/peak-probe-build-adv01-final.log`.
- `target/debug/peak_probe` — exit 0; `c4_peak=0.752829`, finish at frame 150272 (3.130667 s), final-480 peak 0.000072633; `tmp/fm1-voices/FM1V-13/peak-probe-adv01-final.log`.
- Fresh source-linked rebuild and measurement — `CARGO_TERM_QUIET=true cargo build --lib`, probe recompilation against the rebuilt `target/debug/deps/libvactr.rlib`, and rerun all exit 0; measurements are unchanged; `tmp/fm1-voices/FM1V-13/cargo-build-adv01-final.log`, `peak-probe-build-adv01-final2.log`, and `peak-probe-adv01-final2.log`.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/kalimba/)'` — exit 0, 12 run / 12 passed / 0 failed; `tmp/fm1-voices/FM1V-13/nextest-adv01-final-verifier.log`.
- Rechecked current line limits and byte-identical ports contract; `tmp/fm1-voices/FM1V-13/line-limits-adv01.log` and `tmp/fm1-voices/FM1V-13/port-preservation-adv01.log`.
**Review Status**: Implementation repair evidence is complete; independent adversarial re-review remains pending.
