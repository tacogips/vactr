# BASS-20: `bass-core` Kernel Render and Model Tests

**Status**: Completed (session 232; accepted by fanout review and integration review comm-003043; ready to archive to `impl-plans/completed/`)
**Plan ID**: BASS-20 (wave 2)
**Design Reference**: `design-docs/specs/design-bass-voices.md`, sections "Signal flow", "DSP components", "Note length and slide" and "Verification" (kernel unit tests)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan replaces the BASS-00 stub `render` in
`src/dsp/ugen/bass_voice.rs` with the real per-voice kernel. It composes:

- `Ladder`/`Diode` from BASS-10;
- `Osc`, `FmPair`, `Folder`, `quantize` and `click_semis` from BASS-11;
- `AmpEnv`, `FilterEnv`, `accent`, `Glide`, `Lfo` and `gate_samples` from
  BASS-12.

The result is the six models of the design's model table. It stays
unregistered (no `Node` variant) until BASS-30, so every test here renders
by calling `bass_voice::render` directly, exactly like
`src/dsp/tests/dsp/analog_pair.rs:both_paths_reset_and_controls_respond`.

## Non-goals

- No registry, template, `Node`, catalog or codec edits.
- Do not change the component files. If a component bug blocks you, record
  it in the Progress Log and stop; a serial repair fixes it after the join.
- No stereo output and no oversampling.

## Dependencies

- **dependsOn**: BASS-00, BASS-10, BASS-11, BASS-12
- **Blocks**: BASS-30

## writePaths

- `src/dsp/ugen/bass_voice.rs`
- `src/dsp/ugen/bass_voice/voice.rs`
- `src/dsp/tests/dsp/bass_voice.rs`
- `impl-plans/active/bass-20-kernel.md`

## writePathNotes

- path: `src/dsp/ugen/bass_voice.rs` | intendedEdit: replace the stub render body; keep the BASS-00 port contract, `Model` and `sanitize` unchanged
- path: `src/dsp/ugen/bass_voice/voice.rs` | intendedEdit: optional new file, created only if `bass_voice.rs` would exceed its 600-line budget; private module declared from `bass_voice.rs`; no sibling plan touches it
- path: `src/dsp/tests/dsp/bass_voice.rs` | intendedEdit: append the kernel tests; all four BASS-00 tests, including `bass_voice_render_is_finite_and_bounded`, are kept unmodified and must still pass
- path: `impl-plans/active/bass-20-kernel.md` | intendedEdit: Progress Log only

## sharedPaths

None.

## Read-only References

- The five component files under `src/dsp/ugen/bass_voice/` (`ladder.rs`, `diode.rs`, `osc.rs`, `fm.rs`, `mods.rs`) are read-only.

## Kernel Behavior (key decisions)

**State layout.**

- Replace `STATE_FLOATS` with the sum of the component `FLOATS` plus the
  kernel scalars, using named offset constants:
  - `FILTER` (sized `max(Ladder::FLOATS, Diode::FLOATS)`);
  - `OSC_A`, `OSC_B`, `OSC_SUB`, `FM`, `FOLD`, `AMP`, `FENV`, `GLIDE`,
    `LFO`;
  - The `LFO` slot is sized with `Lfo::FLOATS`, which is 6 after the
  session 232 drift-free revision. The fields are `anchor_phase`,
  `anchor_cycles`, `count_lo`, `count_hi`, `rate` and `smooth`. Call
  `Lfo::start` only in the first block. In every later block, `load` it,
  call `next` per sample, then `store` it. Re-calling `start` would reset
  the elapsed count and bring back the beat drift. Always pass the same
  per-block `rate_hz(lfo_rate, lfo_sync, cps)` value, so a constant rate
  never re-anchors.
- `ELAPSED`: samples since note start, as f32;
  - `GATE_LEFT`: remaining gate samples, as f32.
- Imitate the named-offset style of
  `src/dsp/ugen/digital_drum/tonal.rs` (`STATE_FLOATS`, `HIT_ELAPSED`,
  `LFO_PHASE`).
- Use `NodeState.u[1]` as the init flag and `u[0]` as the seed, as
  `tonal.rs` does with `kx.seed ^ <kernel constant>`.

**Port reads.**

- `freq` is read per sample with `ins[port::FREQ].at(i)`, because it may be
  a connected signal.
- Every other port is read once per block with `.first()`.
- The model comes from `Model::from_port(mode)`.
- Enum ports use `Wave::from_index` and `LfoShape::from_index`. Bool ports
  are `> 0.5`.

**First block (init flag 0).**

- Set the seed.
- Detune pair phases (Wobble and Reese only): `Osc::with_phase` from two
  hashed values of the seed. All other oscillators start at phase 0.
- Compute `legato = slide_from != 0`, then call:
  - `Glide::start(slide_from)`;
  - `AmpEnv::start(legato)`;
  - `FilterEnv::start(legato)`.
- Store `legato` in the state so later blocks know it.
- `Lfo::start(initial_phase(retrigger, lfo_offset, rate_hz, onset), shape, seed)`.
- `GATE_LEFT = gate_samples(gate_length, cps, sr)`.

**Per sample.**

1. `pitch = freq * 2^((glide.next + click_semis(elapsed / sr, click_level)) / 12)`.
   `click_level` applies to all models; only `sub-bass` exposes it.
   Clamp pitch to `[8, sr / 4]`.
2. Oscillator section by model:
   - Analog: `Osc_A(wave)` plus `sub_level * Osc_SUB(Square, inc / 2)`.
   - Acid: `Osc_A(wave)`.
   - Fm: `FmPair.next(inc, ratio, index * (INDEX_FLOOR + (1 - INDEX_FLOOR) * fenv), fm_feedback)`,
     with `INDEX_FLOOR = 0.35`; then `Folder.process(.., fold)`; then
     `quantize(.., bit_depth)`.
   - Wobble and Reese: `(Osc_A(wave, inc * r1) + Osc_B(wave, inc * r2)) / sqrt(2)`
     plus `sub_level * Osc_SUB`, where `(r1, r2) = detune_ratios(detune)`.
   - Sub: `Osc_A(wave)`.
3. `x *= accent.gain`, where `accent = accent(accent_port, res, env_decay)`
   is computed once per block.
4. `fc = cutoff * 2^(env_mod * fenv + accent.oct * fenv + cutoff_octaves(lfo_depth, lfo_u))`.
   - `fenv` is `FilterEnv.next(accent.decay)`.
   - `lfo_u` is `Lfo.next(...)`, with `rate_hz(lfo_rate, lfo_sync, cps)`.
5. Filter: Acid uses `Diode`; all other models use `Ladder`. Coefficients
   are built from `fc`, `res` and `drive` every sample.
6. `y *= amp.next(...) * MODEL_GAIN[model]`. `MODEL_GAIN` is a documented
   per-model constant tuned so each model's default-port render at 55 Hz
   peaks between 0.3 and 0.9.
7. `y = soft_limit(y)`: identity for `|y| <= KNEE` (0.8), above that
   `KNEE + (1 - KNEE) * tanh((|y| - KNEE) / (1 - KNEE))` with the sign.
   Then clamp to `[-1, 1]`, and map non-finite values to 0.
8. Gate: decrement `GATE_LEFT`; when it reaches 0, call `amp.release()`.
   **Ignore `kx.gate`**: the kernel owns note-off, like the drum cores.

**End of block.**

- Call `flush()` on the filter and apply `sanitize` to every stored float.
- Then `store` every component.

**Voice end.**

- When `amp.done()`: zero **all** `mem[..STATE_FLOATS]`, write zeros for the
  rest of the block, and call `st.finish()`.
- Every later block writes zeros and returns immediately.
- Zeroing inside the finish path is what makes the "filter states exactly 0
  after the voice finishes" test pass. The design review noted that the
  denormal flush alone cannot guarantee it.

## Pitfalls

- Do not allocate: no `Vec` and no `Box`. Mem is a slice of length
  `>= STATE_FLOATS`; guard with `debug_assert!` and early-return zeros if
  it is shorter.
- Do not read `kx.gate` for note-off, and do not call `st.finish()` before
  the release completes.
- The seed only affects Wobble/Reese phases and the random LFO. Acid, Sub,
  Analog and Fm output must be seed-independent (tested).
- Keep `bass_voice.rs` under 600 lines. If the model match grows, move it to
  the private `bass_voice/voice.rs`, which is already declared in
  writePaths. It is a new file no sibling touches, and it is declared as a
  private module from `bass_voice.rs`.

## Tests (append to `src/dsp/tests/dsp/bass_voice.rs`)

Helpers:

- `ports(overrides: &[(usize, f32)]) -> [Inp; MAX_PORTS]` starts from
  `PORTS` defaults.
- `render_voice(ins, seconds, seed) -> Vec<f32>` renders 256-frame blocks
  with `Kx { sr: 48_000.0, gate: 256, .. }`.

Test cases:

- Every model 0..5 with default ports at freq 55 and 110, rendered for 1 s:
  - finite; `rms > 1e-3`; `|y| <= 1`;
  - peak at 55 Hz in `[0.2, 1.0]`.
- Accent (Acid, freq 55): the first 150 ms with `accent = 1` against `0`
  shows a peak at least 2 dB higher and a higher spectral centroid over the
  first 50 ms.
- Slide (Sub, wave sine, cutoff 20000, res 0, freq 110, `slide-from -12`,
  `slide-time 0.06`):
  - the first measured period (between the first two upward zero
    crossings) is greater than 1.3 x and less than 2.05 x the target period
    `48000/110`, i.e. clearly lengthened by the slide but never longer than
    the -12 semitone start period plus 5%. (`slide-time` is the time to
    settle to 1%, so the pitch already rises noticeably during the first
    cycle and the first period cannot equal the full start period.)
  - the mean period over 0.08..0.12 s is `48000/110` +/-1%;
  - `|y|` reaches at least 90% of its steady peak within 2.5 ms.
- Slide does not retrigger the filter envelope (Acid, `env-mod 4`,
  `slide-from -5` against `0`): the legato note's centroid over the first
  30 ms is lower than the non-legato note's.
- Gate length (Sub, sine, `release 0.01`, `gate-length 1`):
  - at `cps 0.5`: nonzero output at frame 5900; all zeros from frame
    `6000 + 480 + 256`; `st.done()` is true;
  - at `cps 0.5625` the note-off is at 5333 +/-1: the release starts within
    one block after it.
- Wobble (model 3, `lfo-depth 1`, `lfo-rate 4`, `lfo-sync 1`, cps 0.5,
  `gate-length 64`, 2 s): the autocorrelation of 10 ms frame RMS peaks at a
  lag of 50 +/-2 frames (0.5 s). With `lfo-sync 0` and `lfo-rate 4` the peak
  is at 25 +/-2 frames.
- Stability grid: every model at `freq` 20 and 2000, combined with the
  extreme sets
  - {cutoff 20, cutoff 20000};
  - {res 1, drive 1, index 32, fm-feedback 1, fold 1, bit-depth 2};
  - {lfo-depth +/-1, slide-from +/-24};

  each rendered for 2 s: finite and `|y| <= 1`.
- Finish zeroes state: after `done()`, `mem[..STATE_FLOATS]` is all `0.0`
  and the next block's output is all zeros.
- Determinism: the same seed gives bit-identical output for every model.
  Seeds 1 and 2 give identical output for Acid and different output for
  Reese.
- No allocation: wrap one render in the existing allocation probe, if
  `src/dsp/alloc_probe.rs` exposes a test helper. Otherwise record that the
  e2e rig in BASS-30 covers it.

## Verification (evidence required)

1. `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice.rs src/dsp/tests/dsp/bass_voice.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_voice::/) | test(/bass_filters::/) | test(/bass_sources::/) | test(/bass_mods::/)' > tmp/logs/bass-20-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`, with at least 45 tests and 0 failed.
3. Record `STATE_FLOATS`, `MODEL_GAIN` and the measured default peaks per
   model in the Progress Log.

## Concurrency and Drift Protocol

- This plan runs alone in wave 2. Fresh-read `bass_voice.rs` before editing.
- If its hash differs from the BASS-00 post-hash recorded in
  `bass-00-scaffold.md`, reconcile by keeping the pinned contract.
- No git operations other than `status` and `diff`.

## Done Criteria

- [x] Port contract unchanged: all four BASS-00 tests (including
      `bass_voice_render_is_finite_and_bounded`) are kept unmodified and
      still pass.
- [x] Verification passes, with the exit code and count recorded.
- [x] Only writePaths changed.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none

### Session: 2026-09-30 (session 232 implementation)
**Tasks Completed**: BASS-20 kernel, direct behavioral tests, scoped verification.
**Tasks In Progress**: Formal test-integrity and adversarial review are downstream workflow steps.
**Blockers**: None for the assigned implementation.
**Notes**:
- Replaced the stub render with a six-model real-time-safe kernel, retaining
  the BASS-00 port table, `Model`, and `sanitize` unchanged. Named offsets use
  a shared filter slot sized to five floats and six-float `Lfo` state;
  `STATE_FLOATS = 25`. The private render implementation remains in
  `bass_voice.rs` (388 lines), so the optional `voice.rs` split was unnecessary
  and removed to follow the plan's 600-line condition.
- `MODEL_GAIN = [0.6, 0.12, 0.6, 0.6, 0.6, 0.6]` in model order Analog,
  Acid, FM, Wobble, Sub, Reese. Measured 55 Hz default peaks were 0.558401,
  0.325309, 0.552675, 0.484977, 0.558401, and 0.484977 respectively.
- Added direct tests for all models at 55/110 Hz, accent, slide and no filter
  retrigger, gate release at two cps values, synced/free wobble period,
  two-second stability, finish zeroing, determinism, and no render allocation.
  The allocation probe reported zero allocations. All four original BASS-00
  tests were left unmodified and passed within the focused run.
- The accepted accent signal-flow gain remains before the filter; a second
  output gain preserves the required audible accent level after the diode
  input saturation compressed the pre-filter boost to 0.22 dB. The legato
  flag is passed to `AmpEnv::next` only while stage 0 is ramping, avoiding the
  component's stage-1 catch-all completion behavior without changing the
  component file.
- Wobble uses 10 ms RMS windows at a 200 Hz sine carrier and 20 Hz cutoff.
  Autocorrelation peak search ignores lags below 10 frames to exclude the
  trivial adjacent-window peak; the expected periodic peaks remain at 50 +/-2
  frames for synced and 25 +/-2 for free rate.
- Final source verification:
  - `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice.rs src/dsp/tests/dsp/bass_voice.rs`
    exited 0 (`tmp/bass-voices-232/BASS-20/implementation/rustfmt-final-4.log`).
  - `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_voice::/) | test(/bass_filters::/) | test(/bass_sources::/) | test(/bass_mods::/)'`
    exited 0: 57 run, 57 passed, 0 failed
    (`tmp/bass-voices-232/BASS-20/implementation/nextest-final-4.log`).
  - The separate default-level measurement run exited 0, 1/1 passed, with
    peaks recorded above (`tmp/bass-voices-232/BASS-20/implementation/model-levels-final-4.log`).
  - Earlier failed tuning attempts remain in their original logs; the final
    source-matched focused run is green.

### Session: 2026-09-30 (final-source verification after conformance pass)
**Tasks Completed**: Final control-rate port-read adjustment; reverified the integrated source.
**Tasks In Progress**: Formal test-integrity and adversarial review remain downstream.
**Blockers**: None.
**Notes**:
- `slide-from` is now clamped and read once per block before deriving `legato`.
  Final source: `src/dsp/ugen/bass_voice.rs` SHA-256
  `4dcb750479677a245af1ee766ac9da6c2968a0fa4f3fb258339e4806a44726f4`;
  `src/dsp/tests/dsp/bass_voice.rs` SHA-256
  `31bdfa16b0db0193a319855e87958a217c89f0986b572b30ede4d597898f6840`.
- Final `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice.rs src/dsp/tests/dsp/bass_voice.rs`
  exited 0 (`tmp/bass-voices-232/BASS-20/implementation/rustfmt-final-5.log`).
- Final focused nextest command exited 0: 57 run, 57 passed, 0 failed
  (`tmp/bass-voices-232/BASS-20/implementation/nextest-final-5.log`).
- Final default-peak measurement test exited 0: 1 run, 1 passed, 0 failed;
  the six measured peaks match the values in the prior entry
  (`tmp/bass-voices-232/BASS-20/implementation/model-levels-final-5.log`).

### Session: 2026-09-30 (final source hash correction)
**Tasks Completed**: Recorded the source identity after the final one-read port adjustment.
**Tasks In Progress**: Formal test-integrity and adversarial review remain downstream.
**Blockers**: None.
**Notes**: The final `src/dsp/ugen/bass_voice.rs` hash is
`b03ae1c7a914b23a41d41b175e0ca90ef8591a1989581d346154fd2816a281ac`,
superseding the earlier pre-adjustment hash. The final focused gates and
peak measurements are the source-matched `*-final-5.log` evidence above.

### Session: 2026-09-30 (review repairs and serial reconcile)
**Tasks Completed**: Recorded the test-integrity and adversarial repairs, which the
entries above predate, plus one serial-reconcile lint repair.
- Test-integrity (Sonnet subagent, assertions added, none changed): gate note-off timing
  is now checked through the first divergence between short-gate and long-gate renders
  (exact at 6000 and 5333), and the slide no-retrigger test gained a pitch-controlled
  comparison. Both new assertions fail under their mutations
  (`tmp/bass-voices-232/BASS-20/test-integrity/mutation-gate.log`,
  `mutation-fenv.log`); 57/57 passed (`test-integrity/nextest.log`).
- Adversarial finish-path repair: the finish block used to zero the whole final output
  block, which dropped audio rendered before the release ended. It now writes
  `out[rendered..].fill(0.0)` before zeroing state and calling `st.finish()`
  (`src/dsp/ugen/bass_voice.rs:312`). The new regression test
  `bass_voice_finish_keeps_audio_rendered_before_release_end` fails before the fix
  (`tmp/bass-voices-232/BASS-20/adversarial/pre-fix-test.log`, exit 100) and passes after
  it (`adversarial/reviewer/nextest.log`, 58/58).
- Serial reconcile repair: `cargo clippy --all-targets -- -D warnings` failed on
  `clippy::needless_range_loop` in the wobble autocorrelation loop of
  `src/dsp/tests/dsp/bass_voice.rs`. The loop now iterates
  `correlations.iter_mut().enumerate().skip(1)`, which covers the same lags 1..70 with the
  same sums, so behavior is unchanged
  (`tmp/bass-voices-232/reconcile/wave-5/repair-01-diff.txt`).
**Verification** (`tmp/bass-voices-232/reconcile/wave-5/`): rustfmt on the owned files
exit 0, clippy -D warnings exit 0, `test(/bass_voice::/)` 14/14 exit 0, and the
four-module bass filter 58/58.
**Hashes**: `src/dsp/ugen/bass_voice.rs`
`5b5f121e6b75707b9ec9aa93f448070a18b76a80e45ca4df0b4205989cf84ce2`;
`src/dsp/tests/dsp/bass_voice.rs` `758d0369ece913c9a7a79419618d48435a53d822983fda4837ed91d9f85fc9cc`.
These supersede the `4dcb7504`, `b03ae1c7` and `31bdfa16` hashes above.

### Session: 2026-09-30 (session 232 closeout)
**Tasks Completed**: Accepted by fanout review and serial integration review (comm-003043).
**Verification**: Combined-tree reconcile gates in `tmp/bass-voices-232/reconcile/wave-7/` all exit 0 (build, clippy, fmt check, mise lint, full nextest 1697 passed, digests 9/9, presets/examples 9/9, wasm32 build).
**Remaining (non-blocking)**: Manual listening pass on `tmp/bass/`. The Step 8 move to `impl-plans/completed/` was denied by the sandbox and is still pending.
**Status**: Completed.
