# FM1V-20: Six-Operator FM Engine and `fm-mod` Algorithm Mode Kernel

**Status**: In Progress
**Plan ID**: FM1V-20 (wave 2)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("`fm` template backward compatibility", "Macro patch", "Operator, EG and scaling behaviour" numeric rules, "`fm6-core` UGen")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

This plan writes the engine that both FM entry points share:

1. **`fm-mod` algorithm mode.** When the voice's `algorithm` control rounds
   to 1..=32, the `fm-mod` node ignores its `in`/`mod` inputs. It renders
   the six-operator macro patch of that topology at the voice's `freq`,
   `ratio`, `index` and `velocity`. When `algorithm` is 0, negative or
   non-finite, the existing `modulate` body runs unchanged, so the `fm`
   golden digests stay identical.
2. **`fm6-core`.** The full engine, with an optional `Fm6Patch`.

FM1V-40 wires both into the graph: the catalog ports, mixer, voice dispatch
and memory.

## Non-goals

- No registry edits. That means none of: `mixer.rs`, `catalog*`, `codec.rs`,
  `build_helpers.rs`, `voice.rs`, `controls.rs`, `templates.vact`.
- No LFO, pitch EG, AMS/PMS or key sync.
- Do not change `fm::op`, `fm::modulate` or `fm::phase_distortion`; they
  must stay byte-identical.

## Dependencies

- **dependsOn**: FM1V-10 (`ALGORITHMS`, `RENDER_ORDER`, `algorithm`,
  `carrier_count`), FM1V-11 (`Eg`, `EG_BLOCK`, `log_to_amp`, the scaling
  functions)
- **Blocks**: FM1V-40

## writePaths

- `src/dsp/ugen/fm/engine.rs`
- `src/dsp/ugen/fm/engine/setup.rs` (create it only if `engine.rs` would
  exceed 400 lines; its `mod setup;` line goes in `engine.rs`)
- `src/dsp/tests/dsp/fm6_engine.rs`
- `impl-plans/active/fm1-voices-20-fm-engine.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-20` (artifact root)

## sharedPaths

- `src/dsp/ugen/fm.rs`

## sharedPathNotes

- `src/dsp/ugen/fm.rs`: append exactly one new public function,
  `modulate_with_algorithm`, after `modulate`. Do not change any existing
  line. FM1V-00 already added the module declarations.

## Read-only References

- `src/dsp/ugen/fm.rs::op`: phase accumulation, increment clamp
  `[-0.49, 0.49]`, wrap.
- `src/dsp/ugen/fm/patch.rs`, `algorithms.rs`, `envelope.rs`, `scaling.rs`.
- `src/dsp/ugen/mod.rs`: `Inp::first/at`, `Kx { gate, sr, .. }`,
  `NodeState::finish`.
- `src/dsp/tests/dsp/bass_voice.rs`: direct-render helpers.

## File-level Changes

### `src/dsp/ugen/fm/engine.rs`

Keep the FM1V-00 contract items: `FM6_PORTS`, `fm6_port`, `fm_mod_port`,
`MACRO_FEEDBACK`, and the `fm6_render` signature. Set the real
`STATE_FLOATS`, keeping it at or below 160.

**Voice state** (in `mem`):

- per operator: phase, the last two outputs, `Eg` (`EG_FLOATS`), the
  current and next block amplitude, and frequency;
- globals: an initialized flag, the frame counter within `EG_BLOCK`, a
  released flag, and the latched algorithm.

**Setup at voice start.** Two modes, both computed into state on the first
block with no allocation:

- **Patch mode** (`Fm6Patch` present):
  - `key = midi_key(freq)` and `vel = velocity_midi(velocity)`;
  - per-operator frequency is `op_freq_hz(transpose_hz(freq, TRANSPOSE), ..)`;
  - per-operator `Eg::new(rates, levels, op_outlevel(..), op_rate_scaling(..), sr)`;
  - the algorithm comes from the patch, and so does the feedback level
    (`feedback_gain(FEEDBACK)`).
- **Macro mode**, `macro_setup(algorithm, ratio, index, feedback, velocity)`:
  - **carriers**: ratio 1, amplitude 1;
  - **modulators**: ratio `clamp(ratio, 0.0625, 32)`. Their peak phase
    deviation is `index * velocity` radians.
  - **Modulator index envelope**: a 10 ms linear attack, then exponential
    decay with τ = 0.8 s. It is computed per `EG_BLOCK` and interpolated
    linearly within the block.
  - **Carrier envelope**: in `fm-mod` mode it is held at 1. In `fm6-core`
    macro mode it is a linear 10 ms attack, held while `kx.gate > 0`, then
    an exponential release reaching -60 dB in 0.3 s.
  - **Feedback**: `feedback_gain(feedback)`.

**Render loop.** For each sample, for `op` in `RENDER_ORDER` (6..1):

1. The modulation input is the sum of the outputs of `op`'s modulators,
   taken this sample.
2. If `op` is the feedback destination, add
   `feedback_gain * (y1 + y2) / 2` of the source operator's previous two
   outputs.
3. `y_op = amp_op * sin(TAU * (phase + mod_cycles))`.
4. Advance the phase by `clamp(f_op / sr, -0.49, 0.49)` and wrap it.
5. Carriers sum into `out`, then divide by `carrier_count`. The final
   output is clamped to `[-1, 1]`.

**Modulation scale.**

- In patch mode, an operator output of amplitude 1.0 adds 1.0 cycle of
  phase deviation to the operators it modulates. This is msfa's Q24
  convention, where an output of `1<<24` equals one cycle. Document the
  equivalence.
- In macro mode, the modulator amplitude is `index / TAU` cycles, so `index`
  is in radians.

**EG timing.**

- The EG ticks at chunk boundaries of a frame counter that persists across
  blocks. Each tick's amplitude target comes from `log_to_amp`. Within a
  chunk the amplitude is interpolated linearly from the previous target.
- Key-off happens in the first frame where the frame index is at or past
  `kx.gate`, when `kx.gate` is less than the block length. Call `Eg::key_off`
  at that frame's chunk boundary. Pin this rule and test it with a one-line
  comment: a key-off takes effect at the next 64-frame chunk boundary.

**Lifetime.** `fm6_render` calls `st.finish()` once every carrier's EG
reports `released_and_silent`. In macro mode that is when every carrier's
release level is below -80 dB.

**Robustness.** A non-finite value in any state resets the whole voice
state to silence and outputs 0 for the rest of the block.

**Algorithm mode for `fm-mod`.**

```rust
pub fn fm_mod_algorithm(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>)
```

It renders macro mode with `fm_mod_port` indices. The carrier envelope is
held, and the `in` and `mod` inputs are ignored.

### `src/dsp/ugen/fm.rs` (append only)

```rust
pub fn modulate_with_algorithm(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>)
```

1. Read `a = ins[engine::fm_mod_port::ALGORITHM].first()`.
2. If `!a.is_finite() || a.round() <= 0.0`, call `modulate(ins, out)` and
   return. Do not touch `st` or `mem`.
3. Otherwise call `engine::fm_mod_algorithm(..)`.

The algorithm is latched at the first algorithm-mode block. A later change
of the `algorithm` control within the same voice has no effect until the
next note.

## Pitfalls

- **The legacy branch must be byte-identical** to calling `modulate`
  directly. Test it bit for bit.
- **The macro index envelope must not depend on the host block size.** Use
  the same frame counter as the EG.
- **`ins[k]` beyond the connected ports** reads the implicit row control or
  the default. In unit tests, pass the values explicitly.
- **Do not allocate** and do not use `Vec` in either render function.
- **Do not add an LFO or pitch EG**, even though `Fm6Patch` contains them.
- **`Fm6Patch::EMPTY` is silent.** `Fm6Patch::EMPTY` has rates and levels
  at 0, and output level 0. Every patch-mode test fixture must set R1..R4, L1..L4,
  `OUTPUT_LEVEL` and the frequency fields for each audible operator. If a
  patch-mode test is silent or never reaches sustain, fix the fixture to
  match this plan. Never relax the assertion or bend the EG to fit.

## Tests (`src/dsp/tests/dsp/fm6_engine.rs`; names contain `fm6` or `fm_mod`)

Keep `fm6_ports_contract`. Add:

- `fm_mod_legacy_branch_is_bit_identical`: random-ish deterministic `in`
  and `mod` buffers, with algorithm 0, -1 and NaN. The output equals
  `fm::modulate` exactly, and `mem` is untouched (it compares equal to a
  copy).
- `fm_mod_algorithms_are_pairwise_distinct`. Render algorithms 1..=32 with
  freq 220, ratio 14, index 2, velocity 1, for 0.25 s each. All 32 outputs
  are pairwise different (the first divergence exists).
- `fm6_routing_is_observable`. For every algorithm and operator, silence
  that operator by zeroing its amplitude through a test-only setup hook,
  `#[cfg(test)] pub(crate) fn silence_op`. The output changes exactly when
  that operator is a carrier, or reaches a carrier through modulator edges.
- `fm6_feedback_acts_only_through_source`. With feedback 0 against 7 on
  algorithm 32 (all carriers, feedback 6->6), only operator 6's
  contribution differs. Isolate it by silencing operators 1-5.
- `fm6_patch_mode_eg_shapes_output`. Start from `Fm6Patch::EMPTY` and set
  every field below explicitly:
  - `ALGORITHM` = 0 (algorithm 1) and `FEEDBACK` = 0;
  - op1: `OUTPUT_LEVEL` 99, `OSC_MODE` 0, `COARSE` 1, `FINE` 0, `DETUNE` 7,
    R1..R4 = 99/99/70/50 (R3 70, so L2->L3 completes within the window),
    L1..L4 = 99/99/50/0;
  - all other operators: `OUTPUT_LEVEL` 0.

  Render with freq 220 and the gate held for 1.0 s at 48 kHz. The RMS over
  0.5-0.9 s is lower than the RMS over 0-20 ms. Then close the gate and
  render until `st.done()` becomes true, within 5 s.
- `fm6_patch_fixed_frequency_operator`. Start from `Fm6Patch::EMPTY` and
  set:
  - `ALGORITHM` = 31 (algorithm 32) and `FEEDBACK` = 0;
  - op1: `OSC_MODE` 1, `COARSE` 1, `FINE` 0, `DETUNE` 7, `OUTPUT_LEVEL` 99,
    R1..R4 = 99/99/99/99, L1..L4 = 99/99/99/0;
  - all other operators: `OUTPUT_LEVEL` 0.

  With the gate held, a zero-crossing estimate over 1 s (after a 50 ms
  skip) gives 10 Hz +/- 2%. Check this at both freq 220 and freq 440.
- `fm6_block_size_independent`: patch and macro mode at 64-, 128- and
  256-frame blocks are bit-identical.
- `fm6_deterministic`.
- `fm6_stability_grid`: all 32 algorithms, feedback 7, every operator at
  OL 99, freq 20 and 8000. Every sample is finite and `|y| <= 1`.
- `fm6_render_does_not_allocate`, covering both `fm6_render` and
  `modulate_with_algorithm`.

## Verification (logs under `tmp/fm1-voices/FM1V-20/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/fm.rs src/dsp/ugen/fm/engine.rs src/dsp/tests/dsp/fm6_engine.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6|fm_mod|fm_algo/)' > tmp/fm1-voices/FM1V-20/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 11 tests from this plan and 0 failed.
3. `git diff -U0 -- src/dsp/ugen/fm.rs | grep '^-[^-]'` prints nothing:
   there are no removed lines.
4. `wc -l src/dsp/ugen/fm/engine.rs` is below 400. If it is not, split the
   setup into `src/dsp/ugen/fm/engine/setup.rs` and record the split.
   Declare it as a private child module of `engine.rs`, so `fm.rs` does not
   change.
5. `git diff --stat` shows only writePaths and `fm.rs`.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

- Fresh-read `fm.rs` and record its `shasum -a 256` before and after.
- Other wave-2 plans (FM1V-21, FM1V-30) edit other files. If a build fails
  outside your writePaths, wait and re-run.

## Done Criteria

- [x] The engine and `modulate_with_algorithm` are implemented, and the
      legacy branch is bit-identical.
- [ ] Verification 1-5 pass and are recorded.
      Static checks 1, 3 and 4 passed. The canonical focused filter ran with
      an isolated declared target directory and completed 50 tests (49
      passed, 1 failed). The only failure is the FM1V-21
      `fm6_sysex_native_returns_bulk_voices` test, whose fixture uses invalid
      parenthesized Vactr syntax at `src/ns/tests/reactive_basic.rs:81:33`.
      Complete log: `tmp/fm1-voices/FM1V-20/nextest-final-source-03.log`.
      Supplemental plan-module filter passed 11/11 with exit status 0;
      log: `tmp/fm1-voices/FM1V-20/nextest-engine-module-final.log`.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

### Session: 2026-10-10 (FM1V-20 implementation)
**Tasks Completed**: Engine and algorithm-mode wrapper implemented; legacy branch, topology/render, patch EG and fixed-frequency, key-off chunk timing, determinism, stability, and allocation tests added.
**Notes**: `src/dsp/ugen/fm/engine.rs` is 399 lines. `fm.rs` has no removed lines. Final rustfmt passed. `fm6_patch_key_off_waits_for_next_eg_chunk` compares identical output before the next 64-frame boundary and divergence in the boundary chunk. The isolated plan-module filter passed 11/11 with exit status 0. The canonical focused filter ran against `CARGO_TARGET_DIR=target/fm1-voices/FM1V-20/isolated-target` and completed with 50 tests run, 49 passed, and one unrelated FM1V-21 native SysEx test failed because `len (fm6-sysex ./bank.syx)` is invalid Vactr syntax. The failing source path is outside this plan's writePaths. Resume after FM1V-21 repairs its fixture, then rerun the canonical filter. Full logs: `tmp/fm1-voices/FM1V-20/nextest-final-source-03.log` and `tmp/fm1-voices/FM1V-20/nextest-engine-module-final.log`; final rustfmt log: `tmp/fm1-voices/FM1V-20/rustfmt-final-source-03.log`.
