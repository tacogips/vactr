# MOD004-21: Graph Codec Shape Bytes, Edge Output Index, New Record Tag

**Status**: Ready
**Plan ID**: MOD004-21 (wave 2; parallel with MOD004-20 and MOD004-22)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Codec and compatibility)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004D)
**Created**: 2026-09-29
**Last Updated**: 2026-09-29

## Intent and Context

Native and browser peers built from the same revision exchange instrument
graphs as bytes (`src/dsp/arena.rs:encode_inst`/`decode_graph`). The
browser installs from these bytes on the audio side. The owner decision
(2026-09-29) says there is no legacy decoder. This plan:
- changes the instrument record tag, so that old payloads are rejected
  with `BadRecord` rather than misparsed;
- adds one shape byte after each node record;
- adds an `output` byte to each edge record;
- cross-checks the encoded shapes against the derived shapes on decode.

## Non-goals

- No bus/master record change, and no version negotiation or
  mixed-revision support.
- No template build or voice changes (MOD004-22).
- No new `FaultCode`. Use `BadRecord` and `GraphTooLarge`.

## Dependencies

- **dependsOn**: MOD004-10
- **Blocks**: MOD004-30, MOD004-40

## writePaths

- `src/dsp/arena.rs`
- `src/dsp/ugen/catalog/codec.rs`: only if a helper is needed; prefer
  keeping changes in `arena.rs`
- `src/dsp/tests/dsp/codec_shapes.rs` (stub from MOD004-00; fill it)
- this plan's Progress Log

## sharedPaths (read-only)

`src/dsp/graph/shape.rs` (`NodeAudioShape::{to_byte, from_byte}`,
`derive_shapes`, `decl_for_spec`, `decl_for_node`), `src/dsp/ugen/template.rs`
(`RawGraph`).

## Wire Layout (pin exactly)

- **Instrument record**:
  - `tag u8`: the new constant `G_INST = b'V'`, replacing `b'I'`. `b'I'`
    is now an unknown tag and must fall into the existing `_ => Err(FaultCode::BadRecord)` arm.
  - `id u32`, then `params` (unchanged).
  - `len16 nodes`, then for each node: the `put_spec` bytes (unchanged),
    then `shape u8`.
  - `len16 edges`, then for each edge: `from u16, to u16, port u8, output u8`.
  - `node_params` (unchanged).
- `G_BUS = b'B'` and `G_MASTER = b'M'` records are byte-for-byte
  unchanged.

## File-level Changes (`src/dsp/arena.rs`)

- **`encode_inst`**:
  - derive shapes with
    `derive_shapes(def.nodes.len(), |i| decl_for_spec(&def.nodes[i]), &def.edges, &mut shapes)`;
  - map an error to `BuildError::BadEdge`;
  - write each node's `shapes[i].to_byte()` right after its `put_spec`,
    and write `e.output` after `e.port`.
  - Use a stack array `[NodeAudioShape; NODE_CAP]`. The encoder runs on
    the evaluator side, so it may allocate, but it should not need to.
- **`decode_graph`**: keep it allocation-free. Use a stack array for the
  decoded shape bytes.
  - After each `get_node`, read `u8` -> `NodeAudioShape::from_byte`;
    `None` -> `BadRecord`.
  - For each edge, read `output`. `output >= encoded[from].count()` ->
    `BadRecord`. An out-of-range `from` also gives `BadRecord`; check it
    before indexing.
  - After all edges: `derive_shapes(raw.n_nodes, |i| decl_for_node(&raw.nodes[i]), &raw.edges[..raw.n_edges], &mut derived)`.
    Any `Err` -> `BadRecord`. `derived[i] != encoded[i]` for any node ->
    `BadRecord`.
- `SLICE_BYTES`/`GraphTooLarge` handling is unchanged.

Code to imitate: the existing `Out`/`In` helpers in
`src/dsp/ugen/catalog/codec.rs` (`o.u8`, `o.u16`, `i.u8()?`) and the
decoder's `map_err(|_| FaultCode::GraphTooLarge)` style.

## Pitfalls

- `decode_graph` runs inside the browser audio callback. There must be no
  `Vec`, no `Box` and no formatting.
- Check that `derive_shapes` ignores `n_nodes` beyond `raw.n_nodes`. Pass
  exactly `raw.n_nodes`.
- `FrameKeyframe`/`StageLinked` payload records inside `get_node` stay
  unchanged. The shape byte comes after the whole node record, including
  its payload.
- Do not change `encode_bus`.
- `arena.rs` is 778 lines. Keep it under 1000.

## Test Cases (`src/dsp/tests/dsp/codec_shapes.rs`)

Imitate `src/dsp/tests/dsp/arena.rs` for building `InstDef` and calling
`encode_inst`/`decode_graph`.

Write the exact expected bytes in the test by hand from the layout above.
Do not paste encoder output.

- **Mono fixture**: `InstDef` with `Const(440)` -> `SinOsc` port 0. The
  exact bytes include tag `b'V'`, a shape byte `0x00` per node, and an
  edge `output` byte `0x00`.
- **Multi-mono fixture**: `VaSource` -> `VaFilter` whose output 0 and
  output 1 both feed an `Add`. The `VaFilter` shape byte is `0x01`, and
  there are edges with output bytes `0x00` and `0x01`.
- **Stereo fixture**: `SamplePlay(bank)` output 1 -> `Effect(gain)` port
  0. The `SamplePlay` shape byte is `0x09` (count 2, stereo flag on
  output 1); the effect shape byte is `0x04` (count 1, output 0 stereo).
  Confirm these byte values from the bit layout in
  `src/dsp/graph/shape.rs` and the MOD004-10 plan.
- **Round trip, for each fixture**: `decode_graph(encode(def))` gives the
  same `RawGraph` as `RawGraph::load(def)` (nodes, edges including
  `output`, node_params and params), and a second `encode_inst(def)` is
  byte-identical to the first (deterministic re-encode).
- Old tag `b'I'` with an otherwise valid body -> `Err(BadRecord)`.
- A shape byte with bit 6 set -> `BadRecord`.
- A shape byte that disagrees with the kind (`VaFilter` encoded as
  `0x00`) -> `BadRecord`.
- Edge `output = 2` from `VaFilter` -> `BadRecord`.
- Stereo into `Lpf` (hand-built bytes) -> `BadRecord`.
- Bus and master encodings are unchanged against a literal fixture of one
  existing bus.
- The existing arena and browser-install tests still pass, which proves
  the browser path installs with the new tag.

## Verification Commands (logs in `tmp/mod004/MOD004-21/`)

1. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
2. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
3. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run codec_shapes arena stage_linked golden` -> exit 0
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0
6. `wc -l src/dsp/arena.rs src/dsp/ugen/catalog/codec.rs` -> each below 1000
7. `rustfmt --edition 2021 --check <changed .rs files>` -> exit 0

## Completion Criteria

- [ ] The encoder and decoder implement the pinned layout, and the old `I` tag is rejected with `BadRecord`.
- [ ] The decoder cross-checks shapes and edge outputs without allocating.
- [ ] Exact byte fixtures plus decode and re-encode round trips pass for mono, multi-mono and stereo graphs.
- [ ] Bus and master bytes are unchanged, and checks 1-7 pass with logs recorded.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch or
   worktree. Read-only `git diff` and `git status` are allowed.
2. Record pre-edit hashes and an intent snapshot in the Progress Log.
3. Re-read each file just before editing it; re-apply only your own change
   if it drifted.
4. Never edit outside writePaths. If cargo fails in other plans' files,
   wait about 60 s and retry, up to 10 times, then record a blocker.
5. Format only your own files.
6. Record post-hashes, exit statuses and log paths in this Progress Log
   only.

## Progress Log

(empty)
