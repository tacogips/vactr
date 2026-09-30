# BASS-20: `bass-core` Kernel Render and Model Tests

**Status**: Ready
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

- [ ] Port contract unchanged: all four BASS-00 tests (including
      `bass_voice_render_is_finite_and_bounded`) are kept unmodified and
      still pass.
- [ ] Verification passes, with the exit code and count recorded.
- [ ] Only writePaths changed.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none
