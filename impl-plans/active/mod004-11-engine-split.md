# MOD004-11: Split the Engine Render Path Out of `engine.rs`

**Status**: Ready
**Plan ID**: MOD004-11 (wave 1; parallel with MOD004-10 and MOD004-12)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Capacity: the engine.rs split)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004B/C prerequisite)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29

## Intent and Context

`src/dsp/engine.rs` is 966 lines. MOD004-22 must change the per-voice mix
(balance pan for two-channel voices) and the node-buffer allocation. The
repository rule is that any touched Rust file stays under 1000 lines, so
the render path moves into a child module first. The move is
behavior-neutral: a pure move with no logic edits, so later diffs are easy
to review.

## Non-goals

- No change to arithmetic, iteration order, buffer sizes or pan law.
- No renames of public API. `Engine`'s public methods and signatures stay
  identical.

## Dependencies

- **dependsOn**: MOD004-00
- **Blocks**: MOD004-22

## writePaths

- `src/dsp/engine.rs`
- `src/dsp/engine/render.rs` (new child module)
- this plan's Progress Log

## File-level Changes

- In `src/dsp/engine.rs`, add `mod render;` and move the whole private
  method `fn render<C: CellRead + ?Sized>(&mut self, ...)` (currently
  about lines 721-860) into `src/dsp/engine/render.rs` as
  `impl Engine { pub(super) fn render ... }`. A child module can read the
  parent struct's private fields, so no visibility change is needed.
- Inside `render.rs`, extract the per-voice accumulation loop (currently
  lines 800-816: the `pan_gains(v.pan)` call, the `has_aux` branch, and
  the bus/orbit/mix_3/mix_4 accumulation) into a private free function,
  for example
  `fn mix_voice(t: &Template, v: &Voice, rc_out: &[f32], rc_out_r: &[f32], rc_out_3: &[f32], rc_out_4: &[f32], off: usize, bus: &mut BusSlot, orbit: &mut OrbitDelay, mix_3: &mut [f32], mix_4: &mut [f32])`.
  Keep exactly the same operations in the same order. Adjust the parameter
  list to whatever the borrow checker needs, but keep it a free function
  in `render.rs`, because MOD004-22 edits only this function for balance
  pan.
- Move only the imports that become unused in `engine.rs`.

Code to imitate: the existing child-module split pattern, for example
`src/dsp/ugen/digital_drum/` (a file module plus a directory), and the
coding standards in `.agents/skills/rust-coding-standards/`.

## Invariants

- The output is bit-identical. The golden test from MOD004-00 is the
  proof.
- No new allocation on the callback path. The `Rig::step` and
  `run_stereo_for` allocation assertions stay green.
- `engine.rs` drops to at most about 850 lines, and `render.rs` stays
  under 300.

## Pitfalls

- Do not make the moved function `pub`; use `pub(super)`.
- Do not reorder the `mix_3`/`mix_4` accumulation or the `ended` handling.
- MOD004-10 runs at the same time and adds `Edge.output`. `engine.rs` does
  not build `Edge` literals, so you should not need to touch it for that.
  If the crate fails to compile in other files, wait and retry per the
  protocol.

## Test Cases

- No new tests. The existing suite plus `golden` prove neutrality:
  - golden renders -> unchanged;
  - `src/dsp/tests/dsp/voice_stereo.rs` and `stereo_contract.rs` -> pass.

## Verification Commands (logs in `tmp/mod004/MOD004-11/`)

1. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
2. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
3. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run golden voice_stereo stereo_contract rate_contract` -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0
6. `wc -l src/dsp/engine.rs src/dsp/engine/render.rs` -> both below 1000 (target: engine.rs at most 850)
7. `rustfmt --edition 2021 --check src/dsp/engine.rs src/dsp/engine/render.rs` -> exit 0

## Completion Criteria

- [ ] `render` lives in `src/dsp/engine/render.rs`, with the `mix_voice` helper extracted.
- [ ] The golden test passes with no diff to the fixture.
- [ ] Checks 1-7 pass, with logs recorded.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff` and `git status` are allowed.
2. Record pre-edit hashes and an intent snapshot in the Progress Log.
3. Re-read each file just before editing it; if it changed in a way you did
   not make, re-read it and re-apply only your own change.
4. Never edit outside writePaths. If cargo fails in another plan's files,
   wait about 60 s and retry, up to 10 times, then record a blocker.
5. Format only your own files.
6. Record post-hashes, exit statuses and log paths in this Progress Log
   only.

## Progress Log

(empty)
