# PLV-10: Shared Voice-Layer DSP (pure functions and state)

**Status**: Completed
**Plan ID**: PLV-10 (wave 1; parallel with PLV-12)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Upstream behavior translated; Control clock and host rate; Audio path per lane)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

All 24 Plaits templates will share one translation of the upstream Plaits
voice layer (`plaits/dsp/voice.cc`, `plaits/dsp/envelope.h`,
`plaits/dsp/fx/low_pass_gate.h`, `stmlib/dsp/limiter.h`). This plan writes
only the pure, allocation-free math and state types. PLV-20 wraps them in
graph nodes. PLV-21's comparison example calls them directly. The API
below is therefore a contract: pin it exactly.

Every upstream per-block constant assumes 12-sample blocks at 48 kHz, so
one control block is 0.25 ms. Vactr keeps that duration at every host rate
with a fractional control clock.

## Non-goals

- No `UGenSpec`, `Node`, catalog, codec, controls, template, manifest or
  voice-runtime change (PLV-12, PLV-20, PLV-30 and PLV-31 own those).
- No lookup tables. Compute `2^x` with `f32::exp2`; do not port
  `SemitonesToRatio` tables or anything from `plaits/resources.cc`.
- No trigger delay and no trigger hysteresis (design divergences 1 and 2).

## Dependencies

- **dependsOn**: none
- **Blocks**: PLV-20, PLV-21

## writePaths

- `src/dsp/ugen/voice_layer.rs` (new)
- `src/dsp/ugen/voice_layer/tests.rs` (new; `#[cfg(test)] mod tests;` in voice_layer.rs)
- `src/dsp/ugen/mod.rs`: exactly one added line, `pub mod voice_layer;`, placed alphabetically among the existing `pub mod` lines. Nothing else in this file.
- `impl-plans/active/plv-10-voice-layer-dsp.md` (this Progress Log only)
- `tmp/plv/PLV-10/hashes.txt`, `tmp/plv/PLV-10/1-fmt.log`, `tmp/plv/PLV-10/2-check.log`, `tmp/plv/PLV-10/3-clippy.log`, `tmp/plv/PLV-10/4-wasm.log`, `tmp/plv/PLV-10/5-lsp.log`, `tmp/plv/PLV-10/6-focused.log`, `tmp/plv/PLV-10/7-nextest.log`, `tmp/plv/PLV-10/8-lines.log`

## sharedPaths (read-only)

- `design-docs/specs/design-mutable-audio.md` (PLV-001 section is the authority)
- `src/dsp/ugen/fm_pair.rs` (pattern: persistent host-rate interpolation clock carried across callbacks)
- `src/dsp/ugen/clock_noise_pair.rs` (pattern: `struct Svf` state kept as plain floats)
- `src/dsp/ugen/shape_pair.rs` (pattern: `STATE_FLOATS`, state in `mem: &mut [f32]`)

## Contract (pin these names and shapes exactly)

```rust
pub const REFERENCE_RATE: f32 = 48_000.0;
pub const REFERENCE_BLOCK: f32 = 12.0;
pub const DONE_FLOOR: f32 = 1.0e-4;
pub enum LpgMode { Off, Ping, Level }            // Clone, Copy, Debug, PartialEq, Eq
impl LpgMode { pub fn from_control(v: f32) -> Self }
pub fn decay_terms(decay: f32, color: f32) -> (f32, f32); // (short_decay, decay_tail)
pub fn compress_level(level: f32) -> f32;
pub fn shaped_amount(amount: f32) -> f32;
pub fn ping_attack(freq_hz: f32) -> f32;
pub fn post_gain(registered: f32) -> f32;
pub fn clip_unit(x: f32) -> f32;
```

Every state type below derives `Clone, Copy, Debug, PartialEq`. Each has
`pub const FLOATS: usize`, `pub fn load(src: &[f32]) -> Self` and
`pub fn store(&self, dst: &mut [f32])`, which serialize to exactly `FLOATS`
floats (booleans as 0.0/1.0), so PLV-20 can keep state in the voice
`mem` region.

- `ControlClock { carry }` (FLOATS = 1) with
  `pub fn next_len(&mut self, sr: f32) -> usize`.
- `DecayEnvelope { value }` (FLOATS = 1) with `trigger()`,
  `process(short_decay)` and `value()`.
- `VactrolEnvelope { state, gain, frequency, hf_bleed, ramp_up }`
  (FLOATS = 5). It has `new()`, `trigger()`,
  `process_ping(attack, short_decay, decay_tail, color)`,
  `process_lp(level, short_decay, decay_tail, color)`, getters, and
  `is_done() -> bool`.
- `LowPassGate { value, increment, g, h, bleed, s1, s2, prev_gain }`
  (FLOATS = 8) with
  `begin(target_gain, frequency, hf_bleed, len: usize, sr)` and
  `tick(x) -> f32`.
- `PostLimiter { peak }` (FLOATS = 1) with `new()` and
  `process(pre_gain, sr, x) -> f32`.

## Required semantics

- `LpgMode::from_control` returns `Off` for NaN or `v < 0.5`, `Ping` for
  `v < 1.5`, and `Level` otherwise.
- `decay_terms` clamps `decay` and `color` to [0, 1]; a non-finite input
  becomes 0.5. It returns `short = 0.05 * exp2(-8 d)` and
  `tail = 0.005 * exp2(-6 d + h) - short`.
- `compress_level(l)` is `clamp(1.3 l / (0.3 + |l|), 0, 1)`; a
  non-finite input returns 0.
- `shaped_amount(a)` clamps `a` to [-1, 1], then returns
  `1.05 * a * max(|a| - 0.05, 0.05)`.
- `ping_attack(f)` is `24 * clamp(f, 1.0, 24_000.0) / 48_000`; a
  non-finite input uses 440.
- `post_gain(g)` returns 1 when `g < 0`, else `g`. `clip_unit` clamps to
  [-1, 1] and maps NaN to 0.
- `ControlClock::next_len`: `ratio = REFERENCE_BLOCK * sr / REFERENCE_RATE`;
  `carry += ratio`; `len = floor(carry)` (at least 1); `carry -= len`.
  At exactly 48 kHz every length is 12; at 96 kHz every length is 24; at
  44.1 kHz lengths are 11 or 12 and sum to `11.025 * n` within 1.
- `DecayEnvelope`: `trigger` sets the value to 1; `process(s)` computes
  `value *= 1 - 2 s`.
- `VactrolEnvelope::new()` matches upstream `Init`: state 0, gain 1,
  frequency 0.5, hf_bleed 0, no ramp. `trigger` sets `ramp_up`.
  `process_ping`: while ramping, `state += attack`, clamp at 1, and clear
  `ramp_up` on reaching 1; then call `process_lp(if ramp_up {state} else {0}, ...)`.
  `process_lp`: `err = level - state`; the coefficient is 0.6 if
  `err > 0`, else `short + (1 - s^4) * tail`; then `state += c * err`,
  `gain = state`, `frequency = 0.003 + 0.3 s^4 + 0.04 h`, and
  `hf_bleed = (t^2 + (1 - t^2) h) h^2` with `t = 1 - s`. Use the
  pre-update state for `s^2`, `s^4` and `t`, exactly as upstream does.
  `is_done()` is `!ramp_up && state < DONE_FLOOR`.
- `LowPassGate::begin`:
  - `increment = (target - prev_gain) / len` and `value = prev_gain`,
    then `prev_gain = target` (upstream `ParameterInterpolator`);
  - `f = frequency * REFERENCE_RATE / sr`, with the multiplier exactly
    1.0 at 48 kHz;
  - `g = f * (PI + 0.3736 * PI^3 * f^2)` (stmlib `FREQUENCY_DIRTY`);
  - `r = 1 / 0.4`, `h = 1 / (1 + r g + g^2)`; store `g`, `h` and
    `bleed`.
- `LowPassGate::tick`:
  - `value += increment`; `s = x * value`;
  - `hp = (s - r s1 - g s1 - s2) h`; `bp = g hp + s1`; `s1 = g hp + bp`;
    `lp = g bp + s2`; `s2 = g bp + lp`;
  - return `lp + (s - lp) * bleed`.
- `PostLimiter`: the initial peak is 0.5.
  - `process`: `s = x * pre_gain`; `err = |s| - peak`;
    `peak += (err > 0 ? a : r) * err`; return
    `s * (peak <= 1 ? 1 : 1 / peak) * 0.8`.
  - Constants: `a = 0.05`, `r = 0.00002` at 48 kHz exactly. At other
    rates, `c_host = 1 - (1 - c)^(48000 / sr)`.

## Pitfalls

- No `Vec`, `Box`, `String` or `format!` in non-test code. Everything is
  `Copy` and stack-only, and must behave identically on wasm32.
- Update the envelope and the SVF coefficients only in `begin`, never in
  `tick`. Otherwise the output depends on callback size.
- Do not substitute the exact `tan` or the `pi * f` shortcut for the
  dirty tangent.
- `FLOATS` values and field order in `load`/`store` are a contract with
  PLV-20. Do not reorder them after publishing.
- The module doc may name only these upstream paths:
  `plaits/dsp/voice.cc`, `plaits/dsp/voice.h`, `plaits/dsp/envelope.h`,
  `plaits/dsp/fx/low_pass_gate.h`, `plaits/dsp/engine/engine.h`,
  `stmlib/dsp/limiter.h`, `stmlib/dsp/filter.h` and
  `stmlib/dsp/parameter_interpolator.h`. PLV-40 inventories exactly
  these. Credit "Emilie Gillet, MIT" in one sentence. Do not paste
  upstream code or comments.

## Test Cases (`src/dsp/ugen/voice_layer/tests.rs`)

- `decay_terms(0, 0)` -> (0.05, 0.005 - 0.05); `decay_terms(1, 1)` -> (0.05/256, 0.005*2^-5 - 0.05/256), each within 1e-9 relative; NaN -> same as 0.5.
- `compress_level`: 0 -> 0, 1 -> 1.0 (clamped), 0.5 -> 0.8125 +- 1e-6, -1 -> 0.
- `shaped_amount`: 0 -> 0, 1 -> 0.9975 +- 1e-6, -1 -> -0.9975, 0.03 -> 1.05*0.03*0.05.
- `LpgMode::from_control`: 0 -> Off, 1 -> Ping, 2 -> Level, NaN -> Off, 0.49 -> Off, 1.6 -> Level.
- `ControlClock` at 48000 -> 1000 calls all return 12. At 96000, all return 24. At 44100, 4000 calls sum to 44100 +- 1, and each is 11 or 12.
- A ping from `new()` with attack 0.25 reaches `state == 1` after 4 processed blocks, clears `ramp_up`, then decays monotonically; `is_done()` becomes true only after `state < 1e-4`.
- `process_lp` with constant level 0.8 converges to 0.8 +- 1e-4 within 50 blocks. Level 0 afterwards decays, and a larger `decay` gives a slower decay.
- `LowPassGate`: `begin(1, 0.3, 0, 12, 48000)`, then DC 1.0 for 600 samples, settles to 1.0 +- 1e-3. With bleed 1 the output equals the gained input exactly. Splitting one 12-sample control block into ticks across two calls gives bitwise-equal output to one call.
- `PostLimiter`: input 0.1 with pre-gain 1 gives 0.08 +- 1e-6. A sustained 4.0 input stays bounded by 0.8 * 1.0 + 1e-3 after 20000 samples. At 96 kHz the time to reach half-way matches 48 kHz within 1 ms.
- The `load`/`store` round trip is identity for every type, and `FLOATS` equals the stored length.

## Verification Commands (logs in `tmp/plv/PLV-10/`, each ending with `exit=<n>`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (if it fails only in your files, run `rustfmt --edition 2021 src/dsp/ugen/voice_layer.rs src/dsp/ugen/voice_layer/tests.rs` and rerun)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run voice_layer` -> exit 0; every test above listed as passed
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the pass count
8. `wc -l src/dsp/ugen/voice_layer.rs src/dsp/ugen/voice_layer/tests.rs src/dsp/ugen/mod.rs` -> each below 1000

## Completion Criteria

- [x] The contract items exist with the exact names, fields, `FLOATS` values and semantics above.
- [x] `src/dsp/ugen/mod.rs` differs from its pre-edit version by exactly one `pub mod voice_layer;` line.
- [x] Every listed test exists and passes; commands 1-8 pass with logs.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record `shasum -a 256` of each existing
   writePath and a one-line intent per file in `tmp/plv/PLV-10/hashes.txt`
   and in the Progress Log.
3. Re-read each file just before editing it. If it changed in a way you
   did not make, re-read it and re-apply only your own change, and record
   the drift.
4. Never edit outside writePaths. PLV-12 runs at the same time. If cargo
   fails only inside its files, wait about 60 s and retry, up to 10
   times, then record a blocker and stop.
5. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log`. Record
   post-edit hashes, exit statuses and log paths in the Progress Log. A
   missing or truncated log is not a pass.

## Progress Log

### Session: 2026-09-30 — PLV-10 implementation

**Tasks Completed**: Pure voice-layer math/state types, public module export,
contract tests, and commands 1-8.

**Files**: Added `src/dsp/ugen/voice_layer.rs` and
`src/dsp/ugen/voice_layer/tests.rs`; added exactly one public module line in
`src/dsp/ugen/mod.rs`. The module hash is
`f2b3fc20f4a6ba8fd9d795be47351b1ed37516d39bd99589e4c985763ed29d22`; the
test hash is `0761825ebd54df77019454c214108947f5041966863352354e2ebe580b10cc9e`.
The module-export diff is exactly one insertion.

**Verification**: All final-source commands passed. Focused nextest passed 10
tests; full nextest passed 1,593 tests with 2 skipped. Rust file line counts
are 371, 239 and 436, respectively. Complete logs are
`tmp/plv/PLV-10/1-fmt.log` through `tmp/plv/PLV-10/8-lines.log`.

**Attempt Notes**: The initial formatting check also saw a concurrent,
unformatted edit in `src/dsp/ported/manifest.rs`; only the assigned voice-layer
files were formatted, and the check passed after that shared file settled. The
first focused test run exposed an assertion checking limiter output during its
initial transient rather than after the specified 20,000 samples. The test was
corrected to match the plan, and both the focused and full final-source runs
passed. The 48 kHz limiter now uses the exact specified coefficients.

**Handoff**: PLV-10 owns only pure DSP types and tests. Graph nodes and source
comparison remain with downstream PLV-20 and PLV-21; no template, controls,
manifest, provenance, or host integration work was pulled into this plan.

### Session: 2026-09-30 test-integrity repair

**Change**: `VactrolEnvelope::process_ping` in `src/dsp/ugen/voice_layer.rs`
now clears `ramp_up` on reaching 1.0 and only then selects the `process_lp`
level (`state` while ramping, otherwise 0), matching upstream Plaits
`LPGEnvelope::ProcessPing` and the pinned plan semantics. Previously the level
was captured before clearing, holding the state at 1.0 one extra block and
shifting the whole decay by one control block. The test
`ping_reaches_full_opening_then_decays_to_done` in
`src/dsp/ugen/voice_layer/tests.rs` now asserts state 0.75 with ramp_up set
after 3 blocks, and after the 4th block ramp_up cleared, state == 1 - short_decay
and gain == state.

**Verification**: `tmp/plv/PLV-10/repair-1-fmt.log` exit=0;
`tmp/plv/PLV-10/repair-3-clippy.log` exit=0;
`tmp/plv/PLV-10/repair-6-focused.log` exit=0 (10 tests run, 10 passed).
Pre/post-edit hashes are recorded in `tmp/plv/PLV-10/hashes.txt` (REPAIR-1).
