# MOD004-12: Simultaneous Main/Aux Kernels and Stereo Sample Playback

**Status**: Ready
**Plan ID**: MOD004-12 (wave 1; parallel with MOD004-10 and MOD004-11)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (declaration table; output 0 and output 1 semantics; `sample-play`)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004E kernel part)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29

## Intent and Context

Nine kernels become one node with two mono outputs. Output 0 is exactly
today's single output: the path the node's existing selector port picks.
Output 1 is the complementary path, computed in the same sample loop from
the same state. `sample-play` gains a stereo output 1 that shares output
0's playback cursor. This plan adds these kernel entry points and proves,
at kernel level, that they are bit-identical to the legacy functions. The
runtime (MOD004-22) calls the new entry points only when output 1 is
consumed. When it is not, the legacy functions keep running unchanged.

## Non-goals

- No dispatch wiring (`mixer.rs` and `voice.rs` belong to MOD004-22).
- No change to the legacy functions' observable output or state.
- No other kernels. `phase_pair`, `spectrum_pair`, `grain_pair` and every
  seeded kernel are explicitly excluded by the design.

## Dependencies

- **dependsOn**: MOD004-00
- **Blocks**: MOD004-22

## writePaths

- `src/dsp/ugen/va_filter.rs`, `fm_pair.rs`, `analog_pair.rs`,
  `chord_pair.rs`, `table_terrain_pair.rs`, `terrain_pair.rs`,
  `string_machine_pair.rs`, `shape_pair.rs`, `stage_chain.rs`,
  `sample.rs`
- `src/dsp/tests/dsp/kernel_pairs.rs` (stub from MOD004-00; fill it)
- this plan's Progress Log

Do NOT edit the existing per-kernel test files (`src/dsp/tests/dsp/va_filter.rs`
and so on). MOD004-10 edits them at the same time.

## Contract (pin these names and signatures)

Each pair function takes the **legacy function's parameter list, with
`out` replaced by `main, aux`**:

- `va_filter::filter_pair(ins, st, main: &mut [f32], aux: &mut [f32], kx)`
- `fm_pair::render_pair(ins, st, main, aux, kx)`
- `analog_pair::render_pair(ins, st, mem, main, aux, kx)`, and the same
  shape for `chord_pair`, `table_terrain_pair`, `terrain_pair`,
  `string_machine_pair`, `shape_pair` and `stage_chain`
- `sample::play_stereo(bank, ins, st, mono: &mut [f32], left: &mut [f32], right: &mut [f32], kx)`

`main.len() == aux.len()` is the block length, as `out.len()` is today.

## Required Semantics

- `main` is bit-identical to what the legacy function writes to `out` for
  the same inputs and starting state.
- `aux` is bit-identical to what the legacy function would write if the
  selector port had the complementary value. The selector is `ins[4]` for
  all nine. Read it exactly as today: `.first()` per block in
  `analog_pair`, `chord_pair`, `table_terrain_pair`, `terrain_pair`,
  `string_machine_pair` and `shape_pair`, and `.at(i)` per sample in
  `va_filter`, `fm_pair` and `stage_chain`. Check each kernel's current
  code and keep its read point.
- After the call, `st` and `mem` are bit-identical to the state after the
  legacy call. A pair call must not advance state twice.
- `play_stereo`: `mono` equals `play`'s `out`. `left` and `right` use the
  same fractional-position interpolation as `play`, per channel. A 1-channel
  resource writes the same value to both sides. A resource with two or
  more channels uses channels 0 and 1. Every early-return path (missing
  resource, empty region) fills all three outputs with 0.0 and calls
  `st.finish()` exactly as `play` does.

## Implementation Guidance (key points, not code)

- Prefer one private worker per kernel that the legacy function and the
  pair function both call, for example a worker taking
  `aux: Option<&mut [f32]>`. The worker must preserve the exact float
  operation order of today's loop. The complement is just the other
  already-computed value (for example `fm_pair.rs`'s `sub_head` versus
  `carrier_head`, and `va_filter.rs`'s `hp` versus `lp`). Do not
  recompute anything.
- If a kernel computes only the selected path's value and not the other
  (check `terrain_pair.rs:97-101`, where `aux` accumulates
  unconditionally), keep the unconditional accumulations. Do not introduce
  new per-sample state.
- Do not change `pub fn render`/`filter`/`play` signatures. Other code and
  tests call them.
- `fm_pair.rs` is 350 lines and the other files are smaller. All stay
  under 1000.
- Do not read `kx.seed` in any new path. None of these kernels read it
  today.

Code to imitate: the existing loop bodies in each file. For test
scaffolding (building `Inp` arrays, `NodeState::default()` and `Kx`),
imitate `src/dsp/tests/dsp/va_filter.rs` and `src/dsp/tests/dsp/region.rs`
(for `SampleStore`).

## Test Cases (`src/dsp/tests/dsp/kernel_pairs.rs`)

- For each of the nine kernels, with selector 0 and 1 and two
  parameter sets, over 4 consecutive blocks of 64 and 97 frames:
  - pair `main` == legacy `out` with the same selector, bitwise;
  - pair `aux` == legacy `out` with the complementary selector, bitwise;
  - `NodeState` and `mem` after each block are equal between pair and
    legacy, bitwise, using `PartialEq` for `NodeState` and a slice
    comparison for `mem`.
- `va_filter` with a per-sample selector buffer alternating 0/1 -> `main`
  follows the selector per sample, and `aux` is its complement.
- `play_stereo` with a 1-channel ramp -> `mono` == `play` output, and
  `left == right == mono`.
- `play_stereo` with a 2-channel resource (L ramp, R negated ramp) ->
  `mono` == `play` output, `left` == channel 0 interpolation, `right` ==
  channel 1 interpolation, and `left != right`.
- `play_stereo` with a missing resource -> all three outputs are zero, and
  `st.done()` matches `play`.

## Verification Commands (logs in `tmp/mod004/MOD004-12/`)

1. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
2. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
3. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run kernel_pairs golden` -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0
6. `wc -l` of every writePath -> each below 1000
7. `rustfmt --edition 2021 --check <writePath .rs files>` -> exit 0

## Completion Criteria

- [ ] Ten pair entry points exist with the pinned signatures.
- [ ] Legacy functions are behavior-identical: golden passes and existing kernel tests pass.
- [ ] kernel_pairs tests cover every bullet above for all nine kernels and `play_stereo`.
- [ ] Checks 1-7 pass, with logs recorded.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff` and `git status` are allowed.
2. Record pre-edit hashes and an intent snapshot in the Progress Log.
3. Re-read each file just before editing it; re-apply only your own change
   if it drifted.
4. Never edit outside writePaths. If cargo fails in another plan's files,
   wait about 60 s and retry, up to 10 times, then record a blocker.
5. Format only your own files.
6. Record post-hashes, exit statuses and log paths in this Progress Log
   only.

## Progress Log

(empty)
