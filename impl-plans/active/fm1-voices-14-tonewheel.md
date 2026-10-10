# FM1V-14: `tonewheel-core` Kernel (Drawbar Tonewheel Organ)

**Status**: Ready
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
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/tonewheel/)' > tmp/fm1-voices/FM1V-14/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 12 tests run and 0 failed.
3. The kernel file is under 400 lines and the test file under 1000.
4. `git diff --stat` shows only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

You own only your files. If a build fails in another plan's file, wait and
re-run.

## Done Criteria

- [ ] The kernel is implemented, with the real `STATE_FLOATS`.
- [ ] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.
