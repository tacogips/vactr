# PLV-30: Layer-Shaped Voice Lifetime

**Status**: Ready
**Plan ID**: PLV-30 (wave 3; parallel with PLV-31)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Voice lifetime; Real-time, capacity and invariance; Test strategy: lifetime, partition and rate)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

Today a template without envelopes or sample players plays for its gate
time, and the implicit `release` fade then ends it:
`src/dsp/voice.rs` `render` (the `implicit` flag near line 659) and
`Voice::finished` (near line 364). That would cut off the upstream ping
and level tails. When every `vactrol-gate` in a voice has latched an
active, non-bypassed mode ("layer-shaped"), the vactrol tail must govern
the voice instead:

- no implicit fade is applied;
- the voice ends when every gate reports done.

Every other voice must behave bit for bit as before. `src/dsp/voice.rs`
is 918 lines, so the rule lives in a new submodule.

## Non-goals

- No kernel change: PLV-20 owns `src/dsp/ugen/vactrol_gate.rs` and its
  flags. No template, editor, golden digest, manifest or catalog change.
- No change to envelope-driven templates (`envs > 0`) or sample-player
  templates (`players > 0`). Cut-group and steal fades keep working
  unchanged.

## Dependencies

- **dependsOn**: PLV-20 (`Node::VactrolGate` and the `NodeState` flag contract)
- **Blocks**: PLV-40

## Contract consumed (from PLV-20)

For a `Node::VactrolGate` state: `u[1] == 1` means latched; `u[2] == 1`
means shaping; `done()` (u[3]) means the gate closed and the vactrol
finished. Non-shaping gates never finish.

## writePaths

- `src/dsp/voice/lifetime.rs` (new)
- `src/dsp/voice.rs` (add `mod lifetime;`, call the helpers from `render` and `Voice::finished`, and remove the moved logic)
- `src/dsp/ugen/template.rs` (`pub gates: u8` on `Template`, initialized to 0 in the constructor (near line 268) and reset to 0 in `Template::compile` next to the existing `self.envs = 0; self.players = 0;` resets (near line 396), and counted in `compile_node` for `Node::VactrolGate` with saturating add, beside the `envs`/`players` counting match)
- `src/dsp/tests/dsp/voice_lifetime.rs` (new)
- `src/dsp/tests/dsp.rs` (one `mod voice_lifetime;` line)
- `impl-plans/active/plv-30-voice-lifetime.md` (Progress Log only)
- `tmp/plv/PLV-30/hashes.txt`, `tmp/plv/PLV-30/1-fmt.log`, `tmp/plv/PLV-30/2-check.log`, `tmp/plv/PLV-30/3-clippy.log`, `tmp/plv/PLV-30/4-wasm.log`, `tmp/plv/PLV-30/5-lsp.log`, `tmp/plv/PLV-30/6-focused.log`, `tmp/plv/PLV-30/7-nextest.log`, `tmp/plv/PLV-30/8-lines.log`

## sharedPaths (read-only)

- `src/dsp/ugen/vactrol_gate.rs` (PLV-20 kernel; read for the flags)
- `src/dsp/tests/dsp/voice_layout.rs` (graph builders `graph`, `graph_with_params`, `edge`)
- `src/dsp/tests/dsp/analog_pair.rs` (native vs browser install pattern)
- `src/dsp/tests/dsp/multi_output/invariance.rs` (rate/block partition pattern, blocks 64/256/97 at 44100/48000/96000)
- `src/dsp/alloc_probe.rs` and its existing test users (allocation probe pattern)

## Required functions (`src/dsp/voice/lifetime.rs`)

```rust
pub(super) fn layer_shaped(t: &Template, states: &[NodeState]) -> bool;
pub(super) fn implicit_fade(t: &Template, states: &[NodeState]) -> bool;
pub(super) fn finished(t: &Template, states: &[NodeState], ienv: f32) -> bool;
```

- `layer_shaped`: `t.gates > 0`, at least one node whose spec is
  `Node::VactrolGate` was actually found among the template's nodes
  (non-vacuous), and every such node has `u[1] == 1 && u[2] == 1`.
- `implicit_fade`: `t.envs == 0 && t.players == 0 && !layer_shaped`.
  This replaces the local `implicit` expression in `render`.
- `finished` keeps today's precedence exactly:
  1. `envs > 0`: all env nodes done (unchanged);
  2. else `players > 0`: all sample players done (unchanged);
  3. else if `layer_shaped`: all `VactrolGate` nodes `done()` (non-vacuous:
     at least one gate node must exist, so a gate-free template never
     takes this branch);
  4. else `ienv <= 0.0`.

`Voice::finished` delegates to it.

## Pitfalls

- For off-mode and bypassed-gate voices, the fade, gain and end frame
  must stay bit-identical. Only the `implicit` predicate and `finished`
  may change. Do not reorder the gain loop, guard, post effects or
  `gate_left` handling.
- `layer_shaped` is evaluated after the block's nodes have run. On the
  first block the gate latches inside its kernel before the mix reads
  the predicate, so do not evaluate it at voice start.
- `Template` is recompiled in place, so a `gates` count that is only
  initialized in the constructor would stay stale when a slot goes from a
  gate graph to a gate-free graph. The vacuous "every gate is shaping"
  check would then treat the gate-free template as layer-shaped, drop its
  implicit fade and end its voice after the first block. Reset `gates` in
  `Template::compile` and keep the non-vacuous check.
- Node states are reset when a pool slot is reused. Verify it, and do
  not add a separate reset path.
- No allocation or iteration beyond `t.n_nodes`. Use fixed loops over
  the template's node specs.
- After the change, `src/dsp/voice.rs` must be smaller than or equal to
  918 lines plus 10. `lifetime.rs` must stay small (well under 200 lines).

## Test Cases (`src/dsp/tests/dsp/voice_lifetime.rs`)

Build graphs directly with the `voice_layout.rs` helpers:

- Graph P: `SinOsc(220)` feeding `VactrolGate` (const ports:
  `lpg-mode` 1, `lpg-decay` d, `lpg-color` 0.5, `slot` 0, `lane` 0,
  `freq` 220, `velocity` 1) as the sink.
- Graph O: the same with `lpg-mode` 0.
- Graph N: `SinOsc(220)` alone.

Cases:
- O vs N: every output sample of the whole voice, including the
  implicit fade and the end frame, is bitwise equal (`to_bits`) at 48 kHz
  with 256-frame callbacks.
- P with d = 0.8 and the default event gate (0.11 s): RMS over
  [0.30 s, 0.35 s) > 1e-3, which is past the old gate plus the 0.1 s
  release.
- P with d = 0.2: the voice ends (all later output exactly 0.0, and the
  slot is free for a new event) within 3 s. P with d = 0.8 is still
  sounding at 0.3 s.
- Level mode (`lpg-mode` 2) with velocity 0: output magnitude never
  exceeds 1e-6, and the voice ends within the gate plus one callback.
- Bypassed gate (`slot` 21, `lpg-mode` 1): the voice ends at exactly the
  same frame as graph O with the input scaled by 0.8 (implicit fade
  kept).
- In-place recompile: compile a `Template` from graph P, then recompile
  the same `Template` value in place from graph N; `gates == 0`, and a
  voice rendered with it keeps the implicit fade and ends at the same
  frame as a fresh N template.
- Cut group: a second event in the same cut group chokes a sounding P
  voice with the existing cut fade.
- Slot reuse: after a P voice ends, an O voice in the same pool renders
  bitwise equal to a fresh O voice.
- Partition: P at 44100, 48000 and 96000 Hz, with 64, 256 and 97-frame
  callbacks and the event at an absolute time of 2048/rate, is bitwise
  equal across partitions within each rate, including the end frame.
- Native vs browser: P installed through the browser graph path
  (`browser_with` or the decode path) renders bitwise equal to native
  for 1 s at 48 kHz.
- Allocation: rendering P through the rig with the allocation probe
  shows zero allocations during `step`.

## Verification Commands (logs in `tmp/plv/PLV-30/`, each ending with `exit=<n>`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (fix only your files with `rustfmt --edition 2021`)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run voice_lifetime vactrol_gate golden multi_output` -> exit 0
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the count
8. `wc -l src/dsp/voice.rs src/dsp/voice/lifetime.rs src/dsp/ugen/template.rs src/dsp/tests/dsp/voice_lifetime.rs src/dsp/tests/dsp.rs` -> each below 1000; voice.rs <= 928

## Completion Criteria

- [ ] `lifetime.rs` implements the three functions; `voice.rs` delegates to them; `Template.gates` counts gate nodes.
- [ ] Every listed test passes; the golden and multi_output suites pass unchanged.
- [ ] Commands 1-8 pass with logs.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record `shasum -a 256` of each existing
   writePath and a one-line intent per file in `tmp/plv/PLV-30/hashes.txt`
   and in the Progress Log.
3. Re-read each file just before editing it. If it changed in a way you
   did not make, re-read it and re-apply only your own change, and record
   the drift.
4. Never edit outside writePaths. PLV-31 runs at the same time and owns
   the templates, editor lists and golden digests. If cargo fails only
   inside its files, wait about 60 s and retry, up to 10 times, then
   record a blocker and stop.
5. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log`. Record
   post-edit hashes, exit statuses and log paths in the Progress Log. A
   missing or truncated log is not a pass.

## Progress Log

(none yet)
