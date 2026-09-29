# MOD004-22: Dense Channel Buffers, Multi-output Dispatch, Stereo Voices, Balance Pan

**Status**: Ready
**Plan ID**: MOD004-22 (wave 2; parallel with MOD004-20 and MOD004-21)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Voice buffers and channel mapping; Capacity, real-time, and invariance)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004B, MOD-004C)
**Baseline**: `85a300a`. MOD004-00/10/11/12 are committed and accepted.
**Created**: 2026-09-29
**Last Updated**: 2026-09-29 (refined for session 192: pinned MOD004-12 entry points and current line references)

## Baseline Facts at 85a300a (read before editing)

- The pair entry points committed by MOD004-12 (in `src/dsp/ugen/`):
  - `(ins, st, mem, main, aux, kx)`: `analog_pair::render_pair`,
    `chord_pair::render_pair`, `table_terrain_pair::render_pair`,
    `terrain_pair::render_pair`, `string_machine_pair::render_pair`,
    `shape_pair::render_pair` and `stage_chain::render_pair`;
  - `(ins, st, main, aux, kx)` with no `mem`: `fm_pair::render_pair` and
    `va_filter::filter_pair`.
  Re-read each signature before calling it, and pass exactly what the
  legacy call for the same `Node` passes today.
  - `sample::play_stereo(bank, ins, st, mono, left, right, kx)`.
  Map node kinds to these through the same `Node` match that
  `ugen::run` (= `mixer::run`, `src/dsp/ugen/mod.rs:230`) uses for the
  legacy calls.
- `ugen::Src::Node(u16)` is used only in `src/dsp/ugen/template.rs:496`
  and `src/dsp/voice.rs:450`. `src/dsp/build.rs` has its own private
  `Src` (MOD004-20's), which is unrelated. Do not touch it.
- `src/dsp/voice.rs::render` (line 417):
  - the per-node loop splits `rc.bufs` at `i * mb` (line 442);
  - the voice-local effect copies and averages at lines 513-531 (keep
    that exact code for mono subjects);
  - the sink sum is at lines 543-549, over `t.sinks[..t.n_sinks]`, which
    is used only here;
  - `post_r` runs only when `t.has_aux` (line 576).
- `src/dsp/engine.rs:149` allocates `bufs` as `NODE_CAP * mb`.
- `src/dsp/engine/render.rs::mix_voice` (line 146) computes
  `pan_gains(v.pan)` and bypasses it when `t.has_aux`.
- `src/dsp/effects/spatial.rs:108` already has a *private*
  `balance_gains(b)` over `-1..1`. Do not reuse, rename or change it. The
  new public `prim::balance_gains(pan)` takes `0..1`.
  `spatial.rs` imports named items from `prim`, not a glob, so there is
  no name clash.

## Intent and Context

Compiled templates today give each node one mono buffer at index
`node * max_block`, and an effect node used inside a voice downmixes its
stereo result. This plan makes the audio side follow the design:
- a dense, bounded arena of channel slices;
- edges that read the selected output's slice or slices;
- the nine dual kernels and `sample-play` writing both outputs when
  output 1 is consumed;
- `Mul`/`Add` and voice effects running per channel on stereo input;
- stereo voices;
- unity-center balance pan for two-channel voices (main/aux and stereo),
  while mono voices keep equal-power pan.

Every unmigrated template must stay bit-identical, and the MOD004-00
golden test is the gate.

## Non-goals

- No lowering, VM or codec (MOD004-20/21). No template edits (MOD004-30).
- No change to bus/master effect processing, to `out3`/`out4` stems, or
  to mono pan.
- No new kernels. Use the MOD004-12 pair functions only.

## Dependencies

- **dependsOn**: MOD004-10, MOD004-11, MOD004-12
- **Blocks**: MOD004-30, MOD004-40

## writePaths

- `src/dsp/ugen/mod.rs`: `Src`, `NodeSpec`
- `src/dsp/ugen/template.rs`, and optionally a new `src/dsp/ugen/template/slices.rs`
  if the file would pass about 900 lines
- `src/dsp/ugen/mixer.rs`: pair and stereo dispatch
- `src/dsp/ugen/build_helpers.rs`: only if `topo_order` visibility or
  `mem_need` needs touching; `mem_need` itself stays per node
- `src/dsp/voice.rs`, and optionally a new `src/dsp/voice/route.rs`
  submodule for the per-node dispatch, if `voice.rs` would pass about 900
  lines
- `src/dsp/engine.rs`: only the `bufs` allocation size
- `src/dsp/engine/render.rs`: only `mix_voice`, for balance pan
- `src/dsp/effects/prim.rs`: add `balance_gains`
- `src/dsp/tests/dsp/voice_layout.rs` (stub from MOD004-00; fill it)
- Existing test files that fail to compile only because `Src::Node`
  changed shape. Only mechanical updates are allowed, and each must be
  listed in the Progress Log.
- this plan's Progress Log

## sharedPaths (read-only)

`src/dsp/graph/shape.rs` (`derive_shapes`, `voice_layout`,
`assign_slices`, `decl_for_node`, `DISCARD`, `DISCARD_SLICES`,
`MAX_AUDIO_BUFFERS`), the kernel pair functions from MOD004-12.

## Contract Changes (pin)

- `pub enum Src { Default(f32), Node { slice: u16, stereo: bool }, Const(f32), Param(u8), Cell(CellId) }`:
  `slice` is the first channel slice of the selected output, and a stereo
  output's right channel is `slice + 1`.
- `NodeSpec` gains `pub outs: [u16; MAX_OUTPUTS_PER_NODE]` (first slice
  per output, or `DISCARD`) and `pub shape: NodeAudioShape`.
- `Template` gains `pub stereo: bool` (the voice layout is `Stereo`) and
  `pub n_slices: usize`. Keep `has_aux` with its current meaning (an
  `aux-out` node is present).
- `pub fn balance_gains(pan: f32) -> (f32, f32)` in `effects/prim.rs`: `p`
  is clamped exactly as `pan_gains` clamps it, and the result is
  `(min(1, 2*(1-p)), min(1, 2*p))`. At `p = 0.5` the result must be
  exactly `(1.0, 1.0)`.
- The node arena in `engine.rs` becomes
  `(MAX_AUDIO_BUFFERS + DISCARD_SLICES) * max_block` floats. The discard
  slices sit at indexes `MAX_AUDIO_BUFFERS` and `MAX_AUDIO_BUFFERS + 1`.
  Update the `RenderCtx::bufs` doc comment.

## File-level Changes

- **`template.rs::build`**, after `topo_order`:
  - call `derive_shapes(n, |i| decl_for_node(&raw.nodes[i]), edges, &mut shapes)`;
  - call `voice_layout(...)`;
  - call `assign_slices(&order[..n], &shapes, edges, &mut outs)`, using
    the same `order` that places nodes;
  - map errors: `TooManyBuffers` -> `BuildError::TooManyBuffers`, `Cycle`
    -> `BuildError::Cycle`, everything else -> `BuildError::BadEdge`;
  - set `stereo`, `n_slices`, and each `NodeSpec.outs` and `shape`.
  - In `compile_node`, an edge sets
    `inputs[port] = Src::Node { slice: outs[from][output], stereo: shape_of(from, output) == Stereo }`.
  - Sinks keep today's rule (a node with no outgoing edge).
- **`voice.rs::render`**:
  - a node reads its inputs from the part of the arena below its first
    output slice (`outs[0]`), using the existing `split_at_mut` idea
    keyed by `outs[0] * mb` instead of `i * mb`;
  - a node's output slices are contiguous from `outs[0]`;
  - unconsumed outputs 1 and above write into the discard slices, which
    are split off the tail of the arena;
  - `Inp::Buf` for a mono-only port is `slice`.
  - Dispatch:
    - A dual kind (`decl_for_node` is `Fixed` with names `["main","aux"]`)
      with `outs[1] != DISCARD` -> the MOD004-12 `*_pair(…, main, aux, …)`.
      Otherwise the legacy `ugen::run`, which is unchanged.
    - `SamplePlay` with `outs[1] != DISCARD` -> `sample::play_stereo(mono, left, right)`.
      Otherwise the legacy path.
    - `Mul`/`Add` with a stereo output -> run the existing mono formula
      once per channel. The per-channel `Inp` for each port is: the left
      or right slice for stereo sources, and the single mono buffer
      (`Control`/`Tap` sources) for both channels.
    - An `Effect` whose port-0 source is stereo -> copy left/right into
      the node's two output slices, call `unit.run(mem, out_l, out_r, rc.dry, &mut rc.fx)`,
      and do **not** average. A mono subject keeps today's
      copy-and-average code exactly.
  - Sink summing:
    - mono layout: sum each sink's `outs[0]` into `rc.out`, as today;
    - stereo layout: sum sink left slices into `rc.out` and right slices
      into `rc.out_r`.
  - The gain/fade/nonfinite loop already covers `rc.out_r`. Change
    `if t.has_aux { v.post_r.run(..) }` to `if t.has_aux || t.stereo`.
  - The per-node `seed` stays `v.seed.wrapping_add(i)`, with `i` the node
    index (unchanged).
- **`engine/render.rs::mix_voice`**:
  - `t.has_aux || t.stereo` -> `(gl, gr) = balance_gains(v.pan)`, then
    `left = y * gl` and `right = out_r * gr`;
  - otherwise use today's `pan_gains` branch, unchanged.
  - Orbit and bus accumulation keep using `left`/`right`.

## Invariants

- Mono and legacy templates are bit-identical, and golden passes unchanged.
  A mono node's output lands in slice k instead of buffer `node`; the
  values are unchanged.
- `aux-out` templates are bit-identical at center pan, because the gains
  are exactly 1.0.
- No allocation, lock, graph traversal or shape inference in `render`.
  The only per-block work is reading `outs`, `shape` and `Src` data
  decided at build time.
- Every loop bound comes from compiled descriptors (`n_nodes`, `shape`,
  `m`).
- `mem_need` is still counted once per node.

## Pitfalls

- The per-node `NodeState` index (`v.nodes[i]`) and the mem region stay
  keyed by node index, not by slice.
- A stereo output's two slices must be adjacent. Never assume a
  one-to-one mapping between slice and node.
- Split borrows: the inputs region (`..outs[0]*mb`), the own-output region
  and the discard tail must be disjoint `&mut` splits. Do not use
  `unsafe`.
- `HostInputL`, `HostInputR`, `AuxOut`, `Out3`, `Out4`, `FrameKeyframe`
  and `StageLinked` keep their special cases. Write to `outs[0]`.
- Do not call a pair kernel when `outs[1] == DISCARD`. The legacy path is
  what makes unmigrated templates bit-identical by construction.
- `engine.rs` edits are limited to the allocation line. `render.rs` edits
  are limited to `mix_voice`.
- `voice.rs` is 718 lines. Move the dispatch into `voice/route.rs` if
  needed, and keep every file under 1000 lines.

## Test Cases (`src/dsp/tests/dsp/voice_layout.rs`)

Imitate `src/dsp/tests/dsp/voice_stereo.rs` and `stereo_contract.rs` for
native `Rig` setup (install through `Template::from_inst`, i.e. the native
path), hand-built `InstDef`s with `Edge.output`, and the
allocation-probed `Rig::step`.

- Mono graph (`SinOsc` -> `Mul` by `amp`) at pan 0.2 -> L/R equal today's
  `pan_gains` result bitwise (compare with a reference computed from
  `pan_gains`).
- `aux-out` graph at pan 0.5 -> L == main and R == aux, bitwise, the same
  as before. At pan 0.2 -> L == main * 1.0 and R == aux * 0.4, bitwise.
  At pan 0.8 -> L == main * 0.4 and R == aux.
- A `VaFilter` with outputs 0 and 1 both consumed (the main path to the
  sink, the aux path through `AuxOut`) -> L/R equal a two-`VaFilter`
  graph with `mode` 0 and 1, bitwise, on the native `Rig`. Browser
  byte-path coverage waits for MOD004-21 and belongs to MOD004-40.
- `SamplePlay` output 1 (stereo resource with L != R) -> `Effect(gain)`
  sink -> a stereo voice. L and R differ and each equals the gain applied
  to its own channel, with no averaging.
- A stereo `Mul` with a `Param` operand -> both channels are scaled.
- `SamplePlay` with only output 0 consumed -> bit-identical to the legacy
  mono path (the downmix).
- 103 `SamplePlay` nodes whose output 1 (`:stereo`) edges are summed by a
  binary tree of 102 stereo `Add` nodes (each `Add` consumes two stereo
  sources on ports 0 and 1; edge `output` 1 from each `SamplePlay`,
  output 0 from each `Add`) = 205 nodes and 3\*103 + 2\*102 = 513 slices ->
  `Template::build` returns `BuildError::TooManyBuffers`, and
  `BuildError::message` names the 512-buffer cap. The same shape with 102
  `SamplePlay` nodes and 101 `Add` nodes (508 slices) -> the build
  succeeds as a stereo voice. Keep the count under `NODE_CAP` = 256, and
  route every consumed stereo output into a stereo-accepting port (an
  `Effect` port 0 or an `Add`/`Mul` port), not several edges into one
  port.
- 256 mono nodes -> the build succeeds.
- An unconsumed `VaFilter` aux -> the legacy path is used (verify through
  identical output), with no panic.
- Every render above runs under `Rig::step`'s allocation assertion.
- `balance_gains(0.5) == (1.0, 1.0)`, `(0.0) == (1.0, 0.0)`,
  `(1.0) == (0.0, 1.0)`, and a nonfinite pan is handled exactly as
  `pan_gains` handles it.

## Verification Commands (logs in `tmp/mod004/MOD004-22/`)

1. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
2. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
3. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run voice_layout golden voice_stereo stereo_contract quad_outputs rate_contract` -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0
6. `wc -l` of every touched `.rs` file -> each below 1000
7. `rustfmt --edition 2021 --check <changed .rs files>` -> exit 0

## Completion Criteria

- [ ] Dense slice assignment, validated capacity, and `TooManyBuffers` exist, with no callback shape inference or allocation.
- [ ] Pair and stereo dispatch are used only when output 1 is consumed; stereo `Mul`/`Add` and stereo voice effects do not downmix.
- [ ] Stereo and main/aux voices use `balance_gains`, and mono voices keep `pan_gains`.
- [ ] Golden passes unchanged, and every voice_layout test passes on the native `Rig`.
- [ ] Checks 1-7 pass, with logs recorded.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff` and `git status` are allowed.
2. Record pre-edit hashes and an intent snapshot in the Progress Log.
3. Re-read each file just before editing it; re-apply only your own change
   if it drifted.
4. Never edit outside writePaths. MOD004-20 edits `src/dsp/build.rs` and
   MOD004-21 edits `src/dsp/arena.rs`; do not touch either. If cargo
   fails in their files, wait about 60 s and retry, up to 10 times, then
   record a blocker.
5. Format only your own files.
6. Record post-hashes, exit statuses and log paths in this Progress Log
   only.

## Progress Log

(empty)
