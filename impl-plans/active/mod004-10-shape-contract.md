# MOD004-10: Output Shape Contract, Edge Output Index, Selection Wrapper

**Status**: Completed
**Plan ID**: MOD004-10 (wave 1; parallel with MOD004-11 and MOD004-12)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Typed outputs and edges; declaration table; Capacity)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004A contract part)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29

## Intent and Context

Later plans (lowering, codec and runtime) all need one shared definition
of each node kind's outputs, of how shapes propagate along edges, of voice
layout, and of channel-slice counting. If each plan wrote its own copy,
lowering, the decoder and template build could disagree, which would break
native/browser parity. This plan pins these pure, allocation-free
functions and data types. It also adds the `output` field to `Edge` and an
evaluator-only `UGenKind::Output` wrapper, so that dependents compile
against a stable contract. There is **no behavior change** in this plan:
every edge gets `output: 0`, and nothing creates the wrapper yet.

## Non-goals

- No lowering, VM, checker, codec, template or voice logic. Those are
  MOD004-20/21/22.
- No change to `InstDef` or `RawGraph` fields. Shapes are always derived,
  never stored.
- No new `FailCode` or `DiagCode`.

## Dependencies

- **dependsOn**: MOD004-00
- **Blocks**: MOD004-20, MOD004-21, MOD004-22

## writePaths

- `src/dsp/graph.rs`: `Edge.output`, the `UGenKind::Output(u8)` variant,
  `mod shape; pub use shape::*;`
- `src/dsp/graph/shape.rs` (new)
- `src/dsp/ugen/mod.rs`: the `BuildError::TooManyBuffers` variant and its
  message only
- `src/dsp/build.rs`: only `output: 0` in `Edge` literals, plus
  placeholder arms for `UGenKind::Output`
- `src/vm/natives/dsp.rs`: only if a `UGenKind` match must become
  exhaustive
- Every other file that fails to compile only because of the new field or
  variant: add `output: 0` to `Edge { .. }` literals, or add an arm. The
  known set is `src/dsp/arena.rs:585` (decoder literal: `output: 0` for
  now), `src/dsp/ugen/template.rs:34`, `src/host/native/tests/audio.rs`,
  `src/sched/tests/midi.rs`, `src/ns/evaluator.rs:369` and
  `src/ns/journal.rs:430` (only if they are `dsp::graph::Edge`), and the
  about 30 files under `src/dsp/tests/dsp/` that build `Edge` literals.
  Do NOT touch `src/dsp/tests/dsp/kernel_pairs.rs` or the kernel source
  files (MOD004-12 owns them), and do not touch `src/dsp/engine.rs` or
  `src/dsp/engine/` (MOD004-11).
- `src/dsp/tests/dsp/shape_contract.rs` (stub from MOD004-00; fill it)
- this plan's Progress Log

## sharedPaths (read-only)

`src/dsp/ugen/catalog.rs` (`node_of`, i.e. `Node::from_spec`),
`src/dsp/build/names.rs` (the `.vact` names to kind mapping, for example
`wave-grid-core` -> `TableTerrainPair`).

## Contract (pin these signatures exactly)

In `src/dsp/graph/shape.rs`:

```rust
pub const MAX_OUTPUTS_PER_NODE: usize = 4;
pub const MAX_AUDIO_BUFFERS: usize = 512;   // channel slices under the cap
pub const DISCARD_SLICES: usize = 2;        // shared sink for unconsumed outputs >= 1
pub const DISCARD: u16 = u16::MAX;          // slice sentinel for "write to discard"
pub enum AudioOutputShape { Mono, Stereo }  // fn channels(self) -> usize
pub struct NodeAudioShape { /* count: u8 (1..=4), stereo mask: u8 */ }
// NodeAudioShape: const MONO; fn new(count, stereo_mask) -> Option<Self>;
//   fn count(self) -> usize; fn output(self, k: usize) -> Option<AudioOutputShape>;
//   fn to_byte(self) -> u8; fn from_byte(b: u8) -> Option<Self>
pub enum OutputDecl {
    Fixed { shape: NodeAudioShape, names: &'static [&'static str] },
    FollowSubject,   // voice effect node: output 0 takes port-0 input shape
    Elementwise,     // Mul, Add
    Control,         // Const, Param
    Tap { aux: bool }, // AuxOut (aux: true), Out3/Out4 (aux: false); mono input, silent output
}
pub fn decl_for_spec(spec: &UGenSpec) -> OutputDecl;
pub fn decl_for_node(node: &crate::dsp::ugen::Node) -> OutputDecl;
pub enum OutputSelector { Name(KwId), Index(i64) }
pub enum SelectError { SingleOutput, Unknown }
pub fn select_output(kind: &UGenKind, sel: OutputSelector) -> Result<u8, SelectError>;
pub enum ShapeError {
    BadEdge { edge: usize }, BadOutput { edge: usize }, Mismatch { node: u16, port: u8 },
    StereoAuxOut, Cycle, TooManyBuffers { need: usize },
}
pub fn derive_shapes(n: usize, decl: impl Fn(usize) -> OutputDecl, edges: &[Edge],
    out: &mut [NodeAudioShape; NODE_CAP]) -> Result<(), ShapeError>;
pub enum VoiceLayout { Mono, MainAux, Stereo }
pub fn voice_layout(n: usize, decl: impl Fn(usize) -> OutputDecl, shapes: &[NodeAudioShape],
    edges: &[Edge]) -> Result<VoiceLayout, ShapeError>;
pub fn assign_slices(order: &[u16], shapes: &[NodeAudioShape], edges: &[Edge],
    out: &mut [[u16; MAX_OUTPUTS_PER_NODE]; NODE_CAP]) -> Result<usize, ShapeError>;
```

In `src/dsp/graph.rs`: `pub struct Edge { pub from: u16, pub to: u16, pub port: u8, pub output: u8 }`.
Keep the derives. Also add the variant `UGenKind::Output(u8)`, documented
as: evaluator-only; `args[0]` is `(None, UGenInput::Node(source))`; it is
never lowered to a node.

In `src/dsp/ugen/mod.rs`: `BuildError::TooManyBuffers`, with the message
"the instrument needs more than 512 audio channel buffers".

## Rules the Functions Must Implement (from the design)

- **Declaration table:**
  - `Fixed{MONO, &[]}` for every kind not listed below.
  - `Fixed{2 mono outputs, &["main","aux"]}` for `VaFilter`, `FmPair`,
    `AnalogPair`, `ChordPair`, `TableTerrainPair`, `TerrainPair`,
    `StringMachinePair`, `ShapePair` and `StageChain`.
  - `Fixed{[mono, stereo], &["mono","stereo"]}` for `SamplePlay`.
  - `FollowSubject` for `Effect`.
  - `Elementwise` for `Mul` and `Add`.
  - `Control` for `Const` and `Param`.
  - `Tap { aux: true }` for `AuxOut`; `Tap { aux: false }` for `Out3`
    and `Out4`.
  - `VaSource`, `PhasePair` and every other pair kernel stay `Fixed{MONO}`.
- **`to_byte`:** bits 0-1 hold `count - 1`, and bit `2 + k` is the stereo
  flag of output k. `from_byte` rejects bits 6-7 and any stereo flag at or
  above the count.
- **`select_output`:**
  - `UGenKind::Output(_)`, `Effect(_)`, `BusInput`, and `Ugen(spec)` whose
    decl is not `Fixed` with two or more names -> `SingleOutput`.
  - `Name(k)`: the position of `name_of_kw(k)` in `names`, else `Unknown`.
  - `Index(i)`: `0 <= i < count`, else `Unknown`.
- **`derive_shapes`:** runs an internal allocation-free Kahn order over
  `n <= NODE_CAP`, using fixed arrays; a cycle gives `Cycle`. Then:
  - `from`/`to` out of range -> `BadEdge`; `output >= count(from)` -> `BadOutput`.
  - `Fixed`, `Tap` and `Control` accept only mono inputs, else
    `Mismatch`. The shape of `Fixed` is its declaration.
  - `FollowSubject`: the shape is mono with no port-0 edge, otherwise the
    port-0 source's selected output shape. Ports 1 and above must be
    mono.
  - `Elementwise`: stereo if any source whose decl is neither `Control`
    nor `Tap` is stereo. In that case every such source must be stereo,
    else `Mismatch`. `Control` sources (scalar) and `Tap` sources (silent
    mono) are always accepted and broadcast to both channels. This lets
    `(s :stereo) + {x > aux-out}` reach `voice_layout`, which reports
    `StereoAuxOut`.
- **`voice_layout`:** a sink is a node with no outgoing edge.
  - `has_aux` means some node has decl `Tap { aux: true }`.
  - Stereo means some non-`Tap` sink has a stereo output 0. In that case
    every non-`Tap` sink must have a stereo output 0 (else `Mismatch` with
    port 0), and `has_aux` gives `StereoAuxOut`.
  - Otherwise the result is `MainAux` when `has_aux`, else `Mono`.
- **`assign_slices`:** walks `order` sequentially from slice 0. For each
  node:
  - output 0 always gets `channels` consecutive slices;
  - output `k >= 1` gets slices only if some edge has `from == node` and
    `output == k`, otherwise `DISCARD`.
  The return value is the total. A total above `MAX_AUDIO_BUFFERS` gives
  `TooManyBuffers { need }`. Unused array entries stay `DISCARD`.

## Pitfalls

- Do not allocate: no `Vec` or `HashMap`. Template build runs on the audio
  thread for browser installs.
- `Edge` stays `Copy`, `Hash` and `Eq`. Put the new field last.
- In `build.rs`, the placeholder for `UGenKind::Output` in `Graph::node`
  and `lower_bus` returns
  `LowerError::ty("output selection is not supported yet")`. MOD004-20
  replaces it. Do not implement lowering here.
- The existing `EffectKind::bus_port_shape`/`AudioPortShape` stays
  untouched.
- `src/dsp/graph.rs` is 483 lines. Put all new logic in `graph/shape.rs`
  and keep both files under 1000 lines.

## Test Cases (`src/dsp/tests/dsp/shape_contract.rs`)

- For every `UGenSpec` variant constructible in tests (iterate the catalog
  as `src/dsp/tests/dsp/catalog.rs` does): `decl_for_spec(s) == decl_for_node(&Node::from_spec(s))`.
- `NodeAudioShape` byte round trip for all valid (count, mask) pairs; a
  byte with bit 6 set -> `None`; `count = 1` with mask bit 1 -> `None`.
- `select_output` on a `va-filter` kind: `:main` -> 0, `:aux` -> 1,
  `Index(1)` -> 1, `Index(2)` -> `Unknown`, `:left` -> `Unknown`.
  `SinOsc` with `:main` -> `SingleOutput`. `Output(1)` -> `SingleOutput`.
  `sample-play` `:stereo` -> 1.
- `derive_shapes`:
  - a `SamplePlay` output 1 feeding an `Effect` port 0 -> the effect is
    stereo;
  - the same feeding `Lpf` -> `Mismatch`;
  - stereo times a `Param` into `Mul` -> stereo;
  - stereo times mono `SinOsc` into `Mul` -> `Mismatch`;
  - stereo plus an `AuxOut` output into `Add` -> stereo, not an error;
  - an edge output of 2 from `VaFilter` -> `BadOutput`;
  - a two-node cycle -> `Cycle`.
- `voice_layout`:
  - a plain mono chain -> `Mono`;
  - an `AuxOut` graph -> `MainAux`;
  - a stereo effect sink -> `Stereo`;
  - a stereo sink plus `AuxOut` -> `StereoAuxOut`;
  - a stereo sink plus a mono non-tap sink -> `Mismatch`.
- `assign_slices`:
  - 256 mono nodes -> 256;
  - 171 `SamplePlay` nodes each with output 1 consumed -> 513 ->
    `TooManyBuffers`;
  - an unconsumed `VaFilter` aux -> the entry is `DISCARD`;
  - slices increase monotonically along `order`.
- The existing suite still passes unchanged, which shows that
  `output: 0` everywhere is behavior-neutral.

## Verification Commands (logs in `tmp/mod004/MOD004-10/`)

1. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
2. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
3. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --features lsp` -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run shape_contract golden` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the count
7. `wc -l src/dsp/graph.rs src/dsp/graph/shape.rs src/dsp/build.rs src/dsp/ugen/mod.rs` -> each below 1000
8. `rustfmt --edition 2021 --check <changed .rs files>` -> exit 0

## Completion Criteria

- [x] The contract items exist with the exact signatures above (reviewed against this plan).
- [x] Every existing `Edge` literal carries `output: 0`; there is no behavior change and golden passes.
- [x] The shape_contract tests cover every rule bullet above.
- [x] Checks 1-8 pass, with logs recorded.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff` and `git status` are allowed.
2. Record the pre-edit `shasum -a 256` of each writePath and an intent
   snapshot in the Progress Log.
3. Re-read each file just before editing it. If it changed in a way you did
   not make, re-read it and re-apply only your own change.
4. Never edit outside writePaths. MOD004-11 and MOD004-12 run at the same
   time in the same directory. If cargo fails inside their files, wait
   about 60 s and retry, up to 10 times. If it still fails, record a
   blocker and stop.
5. Format only your own files: `rustfmt --edition 2021 <files>`.
6. Record post-hashes, exit statuses and log paths in this Progress Log
   only.

## Progress Log

### Session: 2026-09-29 — Step 6 implementation

**Tasks completed**: MOD004-10 contract types/functions, edge output field and
zero defaults, placeholder selection handling, and shape-contract tests.

**Notes**:
- The runtime pre-edit snapshot SHA-256 manifest is
  `tmp/mod004/MOD004-10/pre-edit-snapshot-sha256.txt`; per-edit intents are in
  `tmp/mod004/MOD004-10/edit-intent-*.txt` and `edge-intent-*.txt`.
- Post-edit hashes for this plan's formatted Rust files are recorded in
  `tmp/mod004/MOD004-10/post-edit-hashes-final2.txt`. The full-suite source
  identity is confirmed by
  `tmp/mod004/MOD004-10/full-final2-source-identity.txt` (`identical=true`,
  648 Rust files).
- Final-source verification passed: all-target check
  (`check-all-targets-final2.exit`), clippy (`clippy-final2.exit`), wasm32
  (`check-wasm-final2.exit`), LSP (`check-lsp-final2.exit`), focused
  shape-contract/golden tests (11 passed), full nextest (1549 passed, 2
  skipped), the 1000-line gate (`line-counts-plan-final3.exit`), rustfmt
  (`rustfmt-check-final2.exit`) and Edge-field audit (`edge-audit-final.exit`).
  Complete logs and exit files are in `tmp/mod004/MOD004-10/`.
- Superseded attempts are retained: the first all-target check failed in the
  downstream-owned `src/dsp/tests/dsp/kernel_pairs.rs` (`check-all-targets.exit
  = 101`); after the plan's 60-second retry it passed, and final2 also passed.
  Initial clippy found one needless range loop in `shape_contract.rs`
  (`clippy.exit = 101`); it was fixed and final2 strict clippy passed.
- An auxiliary all-touched-file line-count wrapper returned 1 because it
  included `wc`'s aggregate `total` row (`line-counts-final2.exit = 1`). The
  exact plan gate excludes that row and passed (`line-counts-plan-final3.exit
  = 0`); every listed Rust source is below 1000 lines.
- The mandatory `check-and-test-after-modify` agent `/root/verify_after_modify`
  independently reran all six build/test gates on the same tree; each exited
  0, with focused tests 11/11 and full nextest 1549/1549 (2 skipped). Logs are
  `tmp/mod004/MOD004-10/agent-verify-*`; the verifier made no source, plan, or
  Git changes.
- Formal review and serial integration remain downstream workflow steps.
