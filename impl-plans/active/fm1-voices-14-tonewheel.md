# FM1V-14: `tonewheel-core` Kernel (Drawbar Tonewheel Organ)

**Status**: In Progress
**Plan ID**: FM1V-14 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` (Part 2 common rules; "Drawbar tonewheel organ")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The user asked for a drawbar tonewheel organ voice with these parts:

- nine drawbars of near-sine partials at equal-tempered ratios, with
  foldback;
- key click;
- single-trigger percussion on the 2nd or 3rd harmonic;
- a vibrato/chorus scanner.

The kernel owns its note length through `gate-length`, in sixteenth-steps
derived from the hidden `cps`. This follows `bass-core`, because `legato`
never reaches audio voices. The rotary sound stays the existing `rotary`
effect on a bus.

The code is original. The facts used are public user-level ones: footages,
foldback, percussion behaviour and the scanner.

## Non-goals

- No registry work. FM1V-30 adds `TonewheelCore` to `wants_tempo_anchor`.
- No rotary/Leslie code.
- No tonewheel leakage or crosstalk.
- Do not use "Hammond" in identifiers.

## Dependencies

- **dependsOn**: FM1V-00
- **Blocks**: FM1V-30

## writePaths

- `src/dsp/ugen/tonewheel.rs`
- `src/dsp/tests/dsp/tonewheel.rs`
- `impl-plans/active/fm1-voices-14-tonewheel.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-14` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/ugen/bass_voice.rs`:
  - the use of `port::GATE_LENGTH`, `port::CPS` and `port::ONSET_TIME`;
  - `src/dsp/ugen/bass_voice/mods.rs::gate_samples`, which turns
    gate-length in sixteenth-steps into samples. Copy the formula; do not
    import the private bass module unless the function is already `pub`.
- `src/dsp/effects/prim.rs`: `DelayLine` (`carve`, `write`, `read`), `Rng`,
  `Biquad`.
- `src/dsp/tests/dsp/bass_voice.rs::bass_voice_gate_length_owns_release_and_finish`:
  the test pattern for note-off timing.

## File-level Changes (`src/dsp/ugen/tonewheel.rs`)

**Partials.** The drawbar ratios are
`[0.5, 2^(7/12), 1, 2, 2^(19/12), 4, 2^(28/12), 2^(31/12), 8]`. Each
partial is an analytic sine with its own phase, kept in state, and all
phases start at 0.

**Foldback.** Fold each partial frequency `fp = freq * ratio`:

- while `fp > 5900.0`, halve it;
- while `fp < 32.70`, double it;
- after folding, skip any partial at or above `0.45*sr`.

**Drawbar levels.**

- Round `d` to 0..=8.
- The gain is 0 when `d == 0`, otherwise `10^(-3*(8-d)/20)`.
- The sum is multiplied by `1/9`.

**Percussion.**

- `organ-perc`, rounded:
  - 0: off;
  - 1: ratio 2;
  - 2: ratio `2^(19/12)`, folded the same way.
- Percussion triggers only if `organ-perc-trigger >= 0.5` at voice start.
- It is a decaying sine with T60 of 0.25 s, or 1.0 s when
  `organ-perc-slow >= 0.5`.
- Level 1.0 normally, or `10^(-10/20)` when `organ-perc-soft >= 0.5`.
- While percussion is active, `drawbar9` is muted. At normal level
  (`organ-perc-soft < 0.5`) the drawbar sum is also scaled by `10^(-3/20)`.

**Key click.**

- At onset, seeded noise (`kx.seed ^ 0x544F_4E45`) with a 4 ms exponential
  decay, band-passed at 3 kHz with Q 1, times `organ-click`.
- When `organ-click == 0.0`, the click branch and its RNG draws are skipped
  entirely, so the click is an exact bypass.

**Scanner.**

- `organ-vibrato` rounded 0..=6: off, V1, V2, V3, C1, C2 or C3.
- A fractional delay over a `DelayLine` carved from `mem`, with a base
  delay of 1.0 ms modulated by a 6.9 Hz sine.
- Peak modulation is 0.25, 0.5 or 1.0 ms for depths 1 to 3.
- V settings output the scanned signal. C settings output
  `0.5 * (dry + scanned)`.
- Off bypasses the line, but still writes into it, so switching on
  mid-note is click-free.
- The delay memory is at most `ceil(0.003 * sr) + 4` floats.

**Note length.**

- Compute `gate_left` from `gate-length` and `cps` at voice start.
- When it reaches 0, run a 5 ms linear release, then call `st.finish()`.
- `kx.gate` is not used for note-off.

**Level.** `velocity` multiplies the output. Clamp the output to `[-1, 1]`.

**State.** All state lives in `mem`. Set `STATE_FLOATS` to the fixed part
plus the delay length at 96 kHz (a fixed upper bound), so that `mem_need`
can be a constant.

## Pitfalls

- **Phases stay bounded.** Accumulate in f32 and wrap with
  `ph -= ph.floor()`, as `fm::op` does.
- **Single-trigger** is the per-event control, decided at voice start only
  (design FV5).
- **`cps` reaches the kernel only after FM1V-30** adds the kernel to
  `wants_tempo_anchor`. Unit tests pass `cps` directly through the inputs.
- **Do not let `gate-length` wrap.** Clamp it to 0.05..=64. Clamp `cps` to
  0.03..=50.

## Tests (`src/dsp/tests/dsp/tonewheel.rs`; names contain `tonewheel`)

Keep the ports test. Add:

- `tonewheel_registration_888000000_peaks`. Use f=220 with drawbars 1-3 at
  8 and the rest 0. Spectrum peaks are within 1% of 110, 329.6 and 220 Hz,
  and there is no peak above -40 dB relative at 440 Hz.
- `tonewheel_foldback_caps_partials`. Use f=2093 (C7) with all drawbars 8.
  The spectrum energy above 5950 Hz is below -60 dB relative to the total.
- `tonewheel_drawbar_steps_are_3db`. The fundamental level with `drawbar3`
  at 8 versus 7 differs by 3 dB +/- 0.1.
- `tonewheel_percussion_second_and_third`:
  - perc 1 adds a decaying 2f peak, and perc 2 a 2.9966f peak;
  - its level at 250 ms is about -60 dB of its onset level (fast,
    +/- 6 dB), and slow decays more slowly;
  - with `drawbar9` at 8 and perc on, energy at 8f is absent (muted).
- `tonewheel_perc_trigger_zero_adds_nothing`: perc 1 with trigger 0 is
  bit-identical to perc 0.
- `tonewheel_click_zero_is_exact_bypass`, and click 1 adds 2-5 kHz energy
  in the first 10 ms.
- `tonewheel_scanner_modulates_at_6_9hz`. With V3, the instantaneous
  frequency of a single-drawbar 1 kHz tone, estimated from zero crossings
  over 2 s, oscillates with a dominant period of 1/6.9 s +/- 5%.
- `tonewheel_gate_length_owns_note_off`. With cps 0.5 and gate-length 4,
  sound stops, meaning RMS below 1e-4 for the rest of the render, within
  `4/(16*0.5) + 0.005` s +/- one sample. `st.done()` is then true.
- `tonewheel_block_size_independent_and_deterministic`.
- `tonewheel_extreme_grid_is_finite_and_bounded`: all controls at their
  range ends, f in {20, 8000}.
- `tonewheel_render_does_not_allocate`.

## Verification (logs under `tmp/fm1-voices/FM1V-14/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/tonewheel.rs src/dsp/tests/dsp/tonewheel.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/tonewheel/)' > tmp/fm1-voices/FM1V-14/nextest-final-current.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 12 tests run and 0 failed.
3. The kernel file is under 400 lines and the test file under 1000.
4. `git diff --stat` shows only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

You own only your files. If a build fails in another plan's file, wait and
re-run.

## Done Criteria

- [x] The kernel is implemented, with the real `STATE_FLOATS`.
- [x] Plan-owned tests cover the pinned port contract and tonewheel behavior (13 tonewheel-named tests total).
- [x] `tonewheel_foldback_caps_partials` holds the note and checks unfolded and folded partial frequencies (FM1V-14-TI-01).
- [x] `tonewheel_gate_length_owns_note_off` asserts sound immediately before the expected note-off window (FM1V-14-TI-02).
- [x] The gate release is armed once, persisted across blocks, ramps for 5 ms, and finishes after the release (FM1V-14-ADV-01).
- [x] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

### Session: 2026-10-10 (FM1V-14 implementation)
**Tasks Completed**: Implemented the nine-partial tonewheel renderer with foldback, drawbar gain steps, onset-latched percussion, seeded filtered click, scanner delay, gate-length release, finite output and fixed state sizing. Added 12 behavioral tests alongside the existing port-contract test. The fixed arena is 301 floats: nine phases plus `ceil(0.003 * 96 kHz) + 4` scanner samples.
**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/tonewheel.rs src/dsp/tests/dsp/tonewheel.rs` passed (exit 0). Final focused nextest passed 13/13, 0 failed, 2,912 skipped at `tmp/fm1-voices/FM1V-14/nextest-retry-2.log` (exit 0). The kernel/tests are 273/456 lines, and the plan's three authored paths are the only paths recorded for FM1V-14.
**Prior attempts**: Initial nextest compilation was blocked by unrelated `src/dsp/tests/dsp/fm6_envelope.rs:74` (`E0689`); after waiting for the shared tree, retry 1 ran 13 tests but exposed a fixture gate-window error in the registration test. The test now holds the note through its one-second analysis window; the final source-matched retry passed. Logs are preserved as `nextest.log` and `nextest-retry-1.log`.
**Next**: Formal combined-tree integration/adversarial review remains downstream.

### Session: 2026-10-10 (FM1V-14 Step 6 final-source re-verification)
**Tasks Completed**: Re-ran the focused tonewheel suite and formatting gate on the current shared-tree source. No Rust edits were required in this Step 6 execution.
**Verification**: `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/tonewheel/)'` exited 0 with 13 tests passed, 0 failures, and 2,913 skipped; complete log: `tmp/fm1-voices/FM1V-14/nextest-final-current.log`. `rustfmt --edition 2021 --check src/dsp/ugen/tonewheel.rs src/dsp/tests/dsp/tonewheel.rs` exited 0; complete log: `tmp/fm1-voices/FM1V-14/rustfmt-final-current.log`. The kernel and tests are 273 and 456 lines, respectively.
**Next**: Formal combined-tree integration/adversarial review remains downstream.

### Session: 2026-10-10 (FM1V-14 test-integrity repairs)
**Tasks Completed**: Repaired FM1V-14-TI-01 by holding the foldback-test note with CPS 0.03 and gate-length 64.0, checking all five unfolded partial frequencies below 1e-3, and adding folded-line positive controls above 0.05 while retaining the prior probes. Repaired FM1V-14-TI-02 by asserting signal above 1e-3 in samples 23,900-23,999 while retaining the existing note-off assertions.
**Kernel Invariant**: `src/dsp/ugen/tonewheel.rs` remains unchanged; SHA-256 `a0870648e33cba6f563af6448b9ee0cc0c917a75965c6695577a05d5d6fbf40a`.
**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/tonewheel.rs src/dsp/tests/dsp/tonewheel.rs` exited 0; complete log `tmp/fm1-voices/FM1V-14/rustfmt-ti-repair.log`. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/tonewheel/)'` exited 0 with 13 tests passed and 0 failures; complete log `tmp/fm1-voices/FM1V-14/nextest-ti-repair.log`. Line counts are 273/475; kernel SHA-256 remains `a0870648e33cba6f563af6448b9ee0cc0c917a75965c6695577a05d5d6fbf40a`.
**Next**: Hand off both repaired findings for independent test-integrity re-review.

### Session: 2026-10-10 (FM1V-14 adversarial repair FM1V-14-ADV-01)
**Tasks Completed**: Fixed the gate/release state machine. The renderer now arms a 5 ms release once when the gate expires, stores the armed marker in `st.s[5]` and remaining samples in `st.s[1]`, linearly scales output throughout the release, and finishes after sample 24,239 (zero-fill begins at 24,240 for the default 48 kHz case). Extended `tonewheel_gate_length_owns_note_off` with release-window signal, late-tail decay, and exact-zero checks from sample 24,241 onward; all earlier assertions remain.
**Negative Control**: The new release-window assertion failed against the old kernel as intended: 1 selected test, 0 passed, 1 failed, exit 100; `tmp/fm1-voices/FM1V-14/negative-control-adv01-old-kernel.log`.
**Verification**: Focused nextest on the repaired source passed 13/13 with 0 failures, exit 0; `tmp/fm1-voices/FM1V-14/nextest-adv01.log`. Independent `check-and-test-after-modify` re-run also passed 13/13, exit 0; `tmp/fm1-voices/FM1V-14/postcheck-adv01-independent-nextest.log`. Rustfmt checks passed with exit 0 at `tmp/fm1-voices/FM1V-14/rustfmt-adv01.log` and `tmp/fm1-voices/FM1V-14/postcheck-adv01-independent-rustfmt.log`. Kernel/tests are 277/488 lines. Final kernel SHA-256: `65c16cfdc4592dab0a3ab455a7d20bf24cf1892f2a885e2daeb1e587d59fa065`.
**Next**: Independent adversarial re-review and serial combined-tree integration review remain downstream.
