# MOD004-21: Graph Codec Shape Bytes, Edge Output Index, New Record Tag

**Status**: Completed (committed in b6fa077)
**Plan ID**: MOD004-21 (wave 2; parallel with MOD004-20 and MOD004-22)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Codec and compatibility)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004D)
**Baseline**: `85a300a`. MOD004-00/10/11/12 are committed and accepted. At the baseline, `src/dsp/arena.rs:490` still has `const G_INST: u8 = b'I'`; the decoder builds `Edge { .., output: 0 }`; `NodeAudioShape::{to_byte, from_byte}` exist in `src/dsp/graph/shape.rs` (bits 0-1 = count - 1, bits 2-5 = stereo mask, bits 6-7 rejected).
**Created**: 2026-09-29
**Last Updated**: 2026-09-29 (implementation and verification)

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
- `arena.rs` is 780 lines. Keep it under 1000.
- `decode_graph` runs on the native engine install path too
  (`src/dsp/engine.rs:491`, `install_bytes`). Allocation-freedom applies
  to both.
- Tests that install through bytes (for example
  `src/dsp/tests/dsp/stage_linked.rs`, `cross_mod.rs`, `chip_pair.rs`)
  call `encode_inst` and so pick up the new tag automatically. No test or
  example embeds literal `b'I'` payload bytes (checked with
  `grep -rn "b'I'" src`, which finds only `arena.rs:490`), so none needs
  regenerating. If one appears, it is outside writePaths: record it as a
  blocker.

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

- [x] The encoder and decoder implement the pinned layout, and the old `I` tag is rejected with `BadRecord`.
- [x] The decoder cross-checks shapes and edge outputs without allocating.
- [x] Exact byte fixtures plus decode and re-encode round trips pass for mono, multi-mono and stereo graphs.
- [x] Bus and master bytes are unchanged in the focused codec fixtures.
- [x] Local checks 1-4 and 6-7 pass, including focused codec and golden tests and formatting.
- [x] Full nextest check 5 passes; the coordinator runs it outside the sandbox. (2026-09-30: 1582/1582 passed, 2 skipped, exit 0 on the final MOD-004 tree, `tmp/mod004/MOD004-40/review/step7-full-nextest.log`.)

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

### Session: 2026-09-29 — MOD004-21 implementation

**Tasks Completed**: codec implementation, allocation-free decoder shape cross-check, exact fixture tests.

**Changes**:
- `src/dsp/arena.rs`: encoder now emits instrument tag `b'V'`, one derived shape byte after every node spec, and each edge's output index after its port. Decoder uses fixed `NODE_CAP` shape arrays, validates source/output bounds before indexing, rejects malformed shape bytes, and compares encoded shapes with `derive_shapes` results. Bus/master branches remain untouched.
- `src/dsp/tests/dsp/codec_shapes.rs`: added hand-written mono, multi-mono, and stereo wire fixtures with decode equality and deterministic re-encode checks; added old/unknown tag, reserved/mismatched shape, invalid output, stereo-to-mono, and bus/master fixtures. Corrected a one-byte error in the hand-written `Const(440.0)` fixture after the first behavioral run.

**Pre-edit hashes**: `arena.rs` df164e51386c9acf06d61eb118dc03b64dc2fb9e5c34eeecf82027643a4a642d; `codec_shapes.rs` 8df27c83fd7b877ca20465f94050c4602c59bd9f9793a641631d24f6799cbaa8.

**Final source identity**: `git diff --binary | sha256sum` was da148e7ab5327a01a53c9f1f60f42f49217b85c7ede4e49ee83ba4e818ca1d67 both before and after retry-5 verification. Final file hashes: `arena.rs` d6552765dfdb16bf6b8dbfb42d97cd89344db2d2c683349154d5d604adb6314d; `codec_shapes.rs` 4fd1ee7cff55fd072043e10771bf48d3438a78cbb4b5cdf777d79c7b9d83a5e0.

**Final verification** (complete logs under `tmp/mod004/MOD004-21/`; retry-5 source identity stable):
- `CARGO_TERM_QUIET=true cargo check -q --all-targets`: exit 0 (`retry-5-cargo-check-all-targets.log`).
- `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings`: exit 0 (`retry-5-cargo-clippy-all-targets.log`).
- `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib`: exit 0 (`retry-5-cargo-check-wasm-lib.log`).
- Focused nextest command from this plan: exit 0, 28 passed, 0 failed (`retry-5-nextest-focused.log`).
- Full `cargo nextest run`: exit 100, 542 run, 540 passed, 2 failed, 2 skipped (`retry-5-nextest-full.log`). Failures: `dsp::tests::dsp::voice_stereo::browser_codec_preserves_aux_output_marker_and_audio` and `dsp::tests::dsp::voice_stereo::native_split_voice_and_mono_pan_remain_distinct`, both asserting RMS in `src/dsp/tests/dsp/voice_stereo.rs:51`.
- Line-count gate: exit 0; `arena.rs` 814 lines and `codec.rs` 393 lines (`retry-5-line-count-gate.log`).
- `rustfmt --edition 2021 --check src/dsp/arena.rs src/dsp/ugen/catalog/codec.rs src/dsp/tests/dsp/codec_shapes.rs`: exit 0 (`retry-5-rustfmt-check.log`).
- Earlier moving-tree failures and resolved mono fixture failure remain recorded in `cargo-check-all-targets.log`, `retry-1-*`, and `retry-2-*`; the final stable source checks supersede their passing gates, while the retry-5 full-suite failures remain unresolved.

### Review fixes: 2026-09-29

- Updated the codec encoder's shape lookup to use `raw.n_nodes - 1` (the node count) rather than depend on the loop counter.
- The previous full-suite failures were the MOD004-22 pan assertions; those assertions and the stereo tap sink bug have been corrected. The coordinator runs the full suite outside this sandbox.
- Final checks passed: `cargo fmt --check`; `cargo check -q --all-targets`; `cargo clippy -q --all-targets -- -D warnings`; wasm32 lib and LSP feature checks; focused nextest covering `codec_shapes` and `golden` (42 passed, 1,531 skipped across the combined requested filter); all touched Rust files are below 1,000 lines. Cargo checks used `CARGO_TERM_QUIET=true`.
- The full nextest suite was not run in this sandbox because tests bind to `127.0.0.1`; the coordinator runs that gate outside the sandbox.
