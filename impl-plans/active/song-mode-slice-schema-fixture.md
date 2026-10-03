# Slice schema fixture compatibility implementation plan

**Plan ID**: SONG-SLICE-SCHEMA-FIXTURE
**Status**: In Progress
**Design Reference**: [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and related plans

The additive Slice structure field requires an explicit update to the existing
public source-free hostile-metadata constructor. Own that compile-required caller
separately from `song-mode-slice-timing-authority.md`, whose eight paths remain
unchanged. This fixture cannot prove real timing issuance or maximal geometry.

## Exact future write manifest

| Path | Deliverable | Status |
|---|---|---|
| `tests/song_source_free.rs` | Add only required Slice structure metadata to existing hostile constructor | In Progress |

This document authorizes no Rust or Cargo by itself. Root releases this one-path
companion after the schema author supplies the real declaration. No production,
other test, index or dependency edits. Keep the file below 1000 lines.

## Compatibility contract

Existing test declaration and behavior remain unchanged:

```rust
fn malformed_layout_copy_and_slice_metadata_are_not_hidden_by_dynamic_timing();
```

Its case4 constructs an invalid descending cuts vector [ONE,ZERO] on a copied
public routing DTO. Supply the new declared Subject structure/issuer metadata
using that test's existing node identity. This remains an explicitly hostile
copied DTO, with no manufactured song timing origin or success authority.
Keep all five malformed cases, the original Type refusal, dynamic neighbors,
limits, and assertions byte-for-byte except the required enum constructor field
and any directly needed import. No dropping the Slice case or lint allowances.

## Tasks and dependencies

| Task | Deliverable | Dependency | Status |
|---|---|---|---|
| TASK-001 | Fresh declaration/caller hash audit | Slice schema authored | In Progress |
| TASK-002 | Exact constructor field compatibility | TASK-001 | In Progress |
| TASK-003 | Joined source-free regression and provenance verification | Both authors held | In Progress |

Tasks are sequential. Preserve the original source-free suite's full inventory;
run the actual whole suite during the mandatory joined independent check, along
with the new timing authority and existing source/route regressions. No Cargo
while any source author is active; retain every foreground process to terminal.

## Completion criteria

- [x] Actual additive schema constructor compiles without an unowned caller gap.
- [x] Exact original malformed cuts and all five Type-refusal assertions remain.
- [x] Whole source-free suite executes with nonempty exact inventory/run agreement.
- [x] Joined native/browser, strict Clippy, scoped formatting and line gates pass.
- [x] Independent source hashes remain fixed through checks; no public timing
  authority or complete Slice geometry is inferred from this compatibility case.

## Progress log

### Session: 2026-10-02

Root freshly searched every Slices constructor and found the external literal at
tests/song_source_free.rs222. Existing inventory/density/nested matches use `..`
and need no field edit. This prior companion prevents an implicit ninth write in
the timing authority source wave. No Rust or Cargo was changed.

### ROOT0423 serial schema source release

The source author owns this exact hostile constructor update alongside the primary schema. Existing five cases and Type assertions remain. No Cargo until both source scopes are held.

### Source-ready compatibility checkpoint

Only the existing hostile Slices literal gained the required Subject structure/issuer field; all five malformed cases, cut values and Type-refusal assertions remain. Source is held for joined independent compilation and unfiltered source-free execution; no passing result is claimed.

### ROOT0435 accepted independent verification; ROOT0439 plan reconciliation

The final held Slice source passed the independent matrix recorded in
`tmp/song-mode-riela/ROOT0432-slice-matrix-acceptance.json`: all 48 gates,
270 distinct unit/integration tests and four privacy cases. Native and browser
compilation, strict Clippy, scoped formatting, line limits and diff checks passed.
The whole source-free suite retained all 12 tests, including the original five
malformed metadata cases and Type diagnostics. All 888 frozen inputs remained
unchanged through the checks. Earlier failed attempts remain in the progress log.

The separate nextest run passed all 11 selected Slice tests, with no selected
skips or failures; its original foreground process terminated with exit zero.
ROOT0435 joins that evidence with the matrix acceptance. The nextest selection
is a subset of the matrix tests, not 11 additional distinct tests.

The scoped implementation and verification criteria above are now checked.
This plan remains In Progress solely pending serial archive/index reconciliation;
its source work and independent verification are finished. This evidence proves
immutable genuine Slice timing and schema compatibility. Index occupancy capture,
maximal connected support, dynamic admission bounds, host readiness, automatic
part progression, playback and export remain unfinished under the active full
song-mode goal. ROOT0438 is implementing the separate index timing operand plan.
