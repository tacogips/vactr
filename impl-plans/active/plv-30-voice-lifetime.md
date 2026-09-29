# PLV-30: Gate-Elided Seed Order and Layer-Shaped Voice Lifetime

**Status**: Ready
**Plan ID**: PLV-30 (session 209, wave 1 of 3; serial)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Template wiring and seed preservation: "Lowering order is not the seed order" and "Gate-elided seed order (required)"; Voice lifetime; Real-time, capacity and invariance; Test strategy: seed order, lifetime, partition and rate)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30 (session 209 rewrite; supersedes the session-205 version)

## Intent and Context

The shared Plaits voice layer (PLV-10/12/20/21, commit `f5e623b`) is in
place, but no template uses it yet. PLV-31 will append two `vactrol-gate`
nodes to each of the 24 Plaits templates. Two engine changes must land
first, and this plan owns both.

1. **Seed order.** `Template::build` (`src/dsp/ugen/template.rs`) orders
   nodes with Kahn's algorithm, `topo_order` in
   `src/dsp/ugen/build_helpers.rs`. That puts every source node (no
   input edges) first. A kernel's seed is `voice.seed + compiled index`
   (`src/dsp/voice.rs`, the two `Kx { .. seed: ... }` sites near lines 527
   and 570). New gate-only params and constants therefore move every
   kernel to a later compiled index. Session 205 changed 8 golden render
   digests this way: clock-noise, dual-hat, dual-kick, dual-snare,
   particle, speech, string and swarm. The fix is a per-node
   *seed ordinal*: the node's position in `topo_order` of the
   *gate-elided graph*. Both `Kx` sites use it.
2. **Voice lifetime.** When every `vactrol-gate` in a voice has latched
   an active, non-bypassed mode ("layer-shaped"), the implicit
   `release` fade is skipped. The voice then ends when every gate reports
   done. Every other voice behaves bit for bit as before.

After this plan, no template text changes, so every line of
`src/host/tests/e2e/templates/golden_digests.txt`, graph and render,
must be unchanged.

## Non-goals

- No template, editor, golden-fixture, manifest, catalog, codec, control,
  inventory or notice change. PLV-31 and PLV-40 own those.
- No change to `src/dsp/ugen/vactrol_gate.rs` or `src/dsp/ugen/voice_layer.rs`.
  PLV-20 and PLV-10 are accepted and read-only.
- No change to `topo_order` itself, to `assign_mem`, or to the gate's
  memory layout (`GATE_STATE_FLOATS = 16` stays in the mem region).
- `decay-mod` is not elided. Do not special-case it.
- No new abstraction beyond the one helper and the accessor below.

## Dependencies

- **dependsOn**: none in this run. The accepted dependencies PLV-10,
  PLV-12, PLV-20 and PLV-21 are already committed in `f5e623b`.
- **Blocks**: PLV-31, PLV-40

## writePaths

- `src/dsp/ugen/build_helpers.rs`
- `src/dsp/ugen/template.rs`
- `src/dsp/voice.rs`
- `src/dsp/voice/lifetime.rs` (new)
- `src/dsp/tests/dsp/seed_order.rs` (new)
- `src/dsp/tests/dsp/voice_lifetime.rs` (new)
- `src/dsp/tests/dsp.rs` (two `mod` lines only)
- `impl-plans/active/plv-30-voice-lifetime.md` (Status line and Progress Log only)
- Evidence: `tmp/plv/s209/PLV-30/hashes.txt`, `tmp/plv/s209/PLV-30/1-fmt.log`, `tmp/plv/s209/PLV-30/2-check.log`, `tmp/plv/s209/PLV-30/3-clippy.log`, `tmp/plv/s209/PLV-30/4-wasm.log`, `tmp/plv/s209/PLV-30/5-lsp.log`, `tmp/plv/s209/PLV-30/6-focused.log`, `tmp/plv/s209/PLV-30/7-nextest.log`, `tmp/plv/s209/PLV-30/8-lines.log`, `tmp/plv/s209/PLV-30/9-golden-unchanged.log`

## sharedPaths (read-only)

- `src/dsp/ugen/vactrol_gate.rs`: gate port order (0 = subject `in`), and the `NodeState` flags `u[1]` latched, `u[2]` shaping and `u[3]` done (`NodeState::done`).
- `src/dsp/ugen/mod.rs`: `NodeSpec`, `NodeState`, `MAX_EDGES` (1024).
- `src/dsp/graph.rs`: `Node`, `Edge` (`Copy`; fields `from`, `to`, `port`, `output`), `NODE_CAP` (256).
- `src/dsp/tests/dsp/voice_layout.rs`: private `graph`, `graph_with_params`, `edge` and `render` helpers. Copy their pattern locally; do not make them `pub`.
- `src/dsp/tests/dsp/vactrol_gate.rs`: how the gate is driven with constant ports.
- `src/dsp/tests/dsp/clock_noise_pair.rs`: how a seeded kernel with a `freq` input is built as an `InstDef`.
- `src/dsp/tests/dsp/multi_output/invariance.rs`: the rate/partition pattern (blocks 64, 256 and 97 at 44100, 48000 and 96000 Hz).
- `src/dsp/tests/dsp/analog_pair.rs`: the `decoded.build(&raw, &env)` native-versus-browser install pattern.
- `src/dsp/alloc_probe.rs`: `armed(f) -> (R, usize)`.
- `tmp/plv-wave3-saved/src/dsp/voice/lifetime.rs` and `tmp/plv-wave3-saved/src/dsp/tests/dsp/voice_lifetime.rs`: session-205 reference for the lifetime part only. They passed their focused tests. Re-derive the code; do not copy blindly.

## Contract (pinned; PLV-31 relies on it)

- `src/dsp/ugen/build_helpers.rs`:
  `pub(super) fn seed_ordinals(n: usize, nodes: &[Node], edges: &[Edge], order: &[u16]) -> Result<[u16; NODE_CAP], BuildError>`.
  The result is indexed by **compiled** index `k` (position in `order`).
- `src/dsp/ugen/template.rs`:
  - new fields on `Template`: `pub gates: u8` and `seeds: [u16; NODE_CAP]` (private);
  - `pub fn seed_ordinal(&self, i: usize) -> u16`, returning `seeds[i]` for `i < n_nodes`, else `i` saturated to `u16`;
  - `boxed()` initializes `gates: 0` and `seeds: [0; NODE_CAP]`;
  - `build` resets `gates = 0` next to `self.envs = 0; self.players = 0;`, counts `Node::VactrolGate` in `compile_node` (saturating add, in the same match that counts `envs` and `players`), and sets `self.seeds = seed_ordinals(n, &raw.nodes[..n], edges, &order[..n])?` after `topo_order`.
- `src/dsp/voice.rs`: both `Kx` seed expressions become
  `v.seed.wrapping_add(u32::from(t.seed_ordinal(i)))`. Nothing else in the
  node loop changes.
- `src/dsp/voice/lifetime.rs`, all `pub(super)`:
  - `fn layer_shaped(t: &Template, states: &[NodeState]) -> bool`
  - `fn implicit_fade(t: &Template, states: &[NodeState]) -> bool`
  - `fn finished(t: &Template, states: &[NodeState], ienv: f32) -> bool`

## seed_ordinals rules (the design's three steps, exactly)

1. **Fast path.** If no `nodes[..n]` entry is `Node::VactrolGate`, return
   `out[k] = k` for every `k < n`. Every gate-free graph, which includes
   every non-Plaits template, must hit this path.
2. **Elide gates.** Build a keep mask over raw indices.
   - Every `VactrolGate` is dropped.
   - A gate's *subject* is the `from`/`output` of its edge with
     `port == 0`.
   - Every edge whose `from` is a gate is redirected to that gate's
     subject. Follow gate-to-gate chains until `from` is not a gate; the
     chain is bounded by `n`. If a gate has no port-0 edge, drop the
     redirected edge.
   - Redirected edges keep their position in the edge list.
   - Edges whose `to` is a gate are dropped.
3. **Elide gate-only sources.** A *source* is a raw node with no
   incoming edge in the original graph. Drop a source when it has at
   least one out-edge **and** every out-edge has `to` = a gate **and**
   `port >= 1`. A source that feeds any gate's port 0 is a subject and is
   kept, for example `white-noise > vactrol-gate`.
4. **Order the rest.** Compact the kept raw indices in ascending raw
   order to `0..m`. Remap the kept edges in list order into a fixed
   `[Edge; MAX_EDGES]`. Run `topo_order(m, &elided[..e])`.
   - For each kept raw node `r`, its ordinal is its position in that
     order. Store it at compiled index `k`, where `order[k] == r`.
   - Each dropped node keeps `out[k] = k`. Gates and param/const nodes
     never read a seed.

Pitfalls:
- Use fixed stack arrays only: `[bool; NODE_CAP]`, `[u16; NODE_CAP]`,
  `[Edge; MAX_EDGES]`. No `Vec`, no `collect`, no `Box`.
  `engine.rs:509` calls `build` in place on the engine side, and it must
  not allocate.
- Index the result by **compiled** index, not raw index. This is the
  easiest thing to get wrong. The tests below catch it.
- Do not change `topo_order` or its tie-breaking. Every existing seed
  depends on it.
- Collisions between a dropped node's ordinal and a kept node's ordinal
  are harmless and expected. Do not try to make ordinals unique.

## Lifetime rules

- `layer_shaped`: `t.gates > 0`, **and** at least one compiled node is
  `Node::VactrolGate` (non-vacuous), **and** every such node's state has
  `u[1] == 1 && u[2] == 1`.
- `implicit_fade`: `t.envs == 0 && t.players == 0 && !layer_shaped`.
  This replaces `let implicit = t.envs == 0 && t.players == 0;` in
  `render` (near line 659).
- `finished` keeps today's precedence exactly:
  1. `envs > 0`: all env nodes done;
  2. else `players > 0`: all sample players done;
  3. else if `layer_shaped`: all `VactrolGate` nodes `done()`;
  4. else `ienv <= 0.0`.

  `Voice::finished` (near line 364) delegates to it.

Pitfalls:
- `off` voices and voices whose gates are all bypassed must keep their
  fade, gain and end frame bit-identical. Do not reorder the gain loop,
  the `is_finite` guard, fades or `gate` handling.
- `layer_shaped` is evaluated after the block's nodes have run, because
  the gate latches inside its kernel on the first block.
- `Template` is rebuilt in place. Reset `gates` in `build`, or a
  gate-free rebuild would inherit a stale count.
- `src/dsp/voice.rs` is 918 lines. After the change it must be
  `<= 930`. `lifetime.rs` must stay under 200 lines.

## Test Cases

`src/dsp/tests/dsp/seed_order.rs` (hand-built `InstDef`s via local
`graph`/`edge` helpers; `Template::from_inst` at 48 kHz, `voice_mem`
24000):

- A gate-free graph, `white-noise > * 0.5` -> `seed_ordinal(i) == i` for
  every compiled `i`.
- Shift compensation. Plain graph: `[freq Param, seeded kernel K(freq), Const 0.5, Mul(K, Const)]`.
  Gated graph: the same nodes in the same raw order, then gate-only
  `Param(lpg-mode)`, `Const(slot)` and `Const(lane)`, and `VactrolGate`
  with subject `Mul` at port 0.
  - The compiled index of K differs between plain and gated. Assert this,
    so the test proves a shift exists.
  - `gated.seed_ordinal(k_gated) == k_plain`.
  - With `lpg-mode` constant 0 (off), the gated voice renders bitwise
    equal to the plain voice for 1 s (`to_bits` on every L/R sample).
  - Pick K from the clock-noise pair builder in
    `src/dsp/tests/dsp/clock_noise_pair.rs`. It reads `kx.seed`.
- A gate-only source at raw index 0, before `white-noise`: white-noise's
  compiled index shifts, its ordinal equals its plain index, and the
  renders are bitwise equal.
- Subject kept: `white-noise` feeding only a gate's port 0 (the gate is
  the root) plus gate-only consts. White-noise's ordinal equals its
  index in the plain `white-noise` graph, and the off render is bitwise
  equal to plain.
- Gate chain: `Mul > gate > gate` (root). The kernel's ordinal equals the
  plain index.
- Shared source kept: a `freq` Param read by both K and the gate is
  kept, and ordinals equal the plain case.
- In-place rebuild: build the gated graph, then rebuild the same
  `Template` value from the plain raw graph. Every
  `seed_ordinal(i) == i` and `gates == 0`.
- Allocation: `alloc_probe::armed(|| t.build(&raw, &env))` on the gated
  raw graph reports 0 allocations.

`src/dsp/tests/dsp/voice_lifetime.rs`. Build graphs directly. P is
`SinOsc(220)` into `VactrolGate` as the sink, with constant ports
`lpg-mode` 1, `lpg-decay` d, `lpg-color` 0.5, `slot` 0, `lane` 0,
`freq` 220 and `velocity` 1. O is the same with `lpg-mode` 0. N is
`SinOsc(220)` alone.

- O versus N: the whole voice, including the implicit fade and the end
  frame, is bitwise equal at 48 kHz with 256-frame callbacks.
- P, d = 0.8, default event gate: RMS over [0.30 s, 0.35 s) is above 1e-3.
- P, d = 0.2: the voice ends (all later output exactly 0.0 and the slot
  is free) within 3 s.
- Level mode (`lpg-mode` 2) with velocity 0: |output| never exceeds 1e-6,
  and the voice ends within the gate plus one callback.
- Bypassed gate (`slot` 21, `lpg-mode` 1): ends at exactly the same frame
  as O, and its output equals O x 0.8 within `1e-6 + 1e-6*|O|`.
- In-place rebuild P then N: `gates == 0`, the implicit fade is kept, and
  the voice ends at the same frame as a fresh N.
- Cut group: a second event in the same cut group chokes a sounding P
  voice with the existing cut fade.
- Slot reuse: after a P voice ends, an O voice in the same pool renders
  bitwise equal to a fresh O voice.
- Partition: P at 44100, 48000 and 96000 Hz with 64, 256 and 97-frame
  callbacks is bitwise equal across partitions within each rate,
  including the end frame.
- Native versus browser: P built through the decode path renders bitwise
  equal to native for 1 s at 48 kHz.
- Allocation: rendering P through the rig under `alloc_probe::armed`
  shows 0 allocations during `step`.

## Verification Commands

Logs go in `tmp/plv/s209/PLV-30/`. Run each command as
`cmd > log 2>&1; echo "exit=$?" >> log`.

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0. Fix only your own `.rs` files with `rustfmt --edition 2021`.
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run seed_order voice_lifetime vactrol_gate golden multi_output migrated_pairs select_output` -> exit 0
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the pass count. If it fails, rerun with `--no-fail-fast` into `7-nextest-nff.log` before any repair, and list every failing test.
8. `wc -l src/dsp/voice.rs src/dsp/voice/lifetime.rs src/dsp/ugen/template.rs src/dsp/ugen/build_helpers.rs src/dsp/tests/dsp/seed_order.rs src/dsp/tests/dsp/voice_lifetime.rs src/dsp/tests/dsp.rs` -> every file under 1000 lines; voice.rs <= 930.
9. `git diff --exit-code -- src/host/tests/e2e/templates/golden_digests.txt src/prelude/templates.vact` -> exit 0. No fixture or template change.

## Completion Criteria

- [ ] `seed_ordinals`, `Template::seed_ordinal`, `Template.gates` and the `seeds` field exist with the pinned signatures; both `Kx` sites use the ordinal.
- [ ] `lifetime.rs` implements the three functions, and `voice.rs` delegates to them.
- [ ] Every listed test exists and passes. The shift-compensation test asserts that the compiled index really differs.
- [ ] The golden suites pass with `golden_digests.txt` byte-identical (command 9).
- [ ] Commands 1-9 pass with complete logs ending in `exit=0`.
- [ ] The header `**Status**` is set to `Completed` and the Progress Log records the session.

## Execution Protocol

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record `shasum -a 256` and a one-line intent for
   each existing writePath in `tmp/plv/s209/PLV-30/hashes.txt`.
3. Re-read each file just before editing it. If it drifted because of a
   change you did not make, re-read it, re-apply only your own change,
   and record the drift. Record post-edit hashes.
4. Never edit outside writePaths. A needed repair elsewhere is recorded
   as a blocker, with file, cause and evidence, for the orchestrator's
   serial repair.
5. Record every command, its exit status and its log path in the Progress
   Log. A missing or truncated log is not a pass.

## Progress Log

(none yet)
