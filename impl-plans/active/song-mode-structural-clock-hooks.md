# Structural sampling and unchanged child clocks

**Status**: Planning
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Immutable consumers and authentic capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose and dependencies

Replace temporary Unknown barriers with actual operator clock evidence needed
for song routing. Euclid samples a subject under a generated timing event;
Ply, Arp, Chop, Striate, LoopAt and Fit query their children in unchanged time.
Do not invent an affine map for either category.

- **Related**: [Issued route resolution](song-mode-issued-route-resolution.md),
  [reconciliation](song-mode-reconciliation.md).
- **Previous**: Accepted canonical clock, sampling and query authority work.
- **Next**: Separate Chunk output-anchor selection proof and production adoption.

No Rust changes are released by this draft. Frozen-event repair is active.
Author must confirm the exact manifest and owning evidence before Ready.

## Proposed bounded manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/pattern/eval/song_clock.rs` | Extract dispatch and register children | Not Started |
| `src/pattern/eval/song_clock/dispatch.rs` (new) | Exhaustive dispatch; remove only proven barriers | Not Started |
| `src/pattern/combinators/structure.rs` | Genuine Euclid sampling hook and subdivision evidence | Not Started |
| `src/pattern/combinators/region.rs` | Chop/Striate/LoopAt/Fit owning behavior | Not Started |
| `src/pattern/combinators/music.rs` | Arp owning behavior | Not Started |
| `src/pattern/eval/song_clock/structural_tests.rs` (new) | Actual Index clock regressions | Not Started |

Six candidate paths. The current clock parent is994 lines and existing tests896;
extract dispatch before additions and put bulk fixtures in the new child. Keep
every touched Rust source below1000. Declare additional paths before edits.
If pass-through operators require no combinator edits, narrow the final source
manifest before release while preserving their required behavioral evidence.

## Existing interfaces to preserve

Move this owning method into the dispatch child without changing its signature:

```rust
impl QState<'_, '_> {
    pub(crate) fn with_clock_dispatch<T>(
        &mut self, node: &PatNode,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure>;
}
```

Euclid must use the existing real sampling interface:

```rust
impl QState<'_, '_> {
    pub(crate) fn with_structural_sample<T>(
        &mut self, child: &Pat, at: Ratio64, ordinal: u32,
        whole: Option<TimeSpan>, part: TimeSpan, timing: Option<&Event>,
        f: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> Result<T, Failure>;
}
```

Preserve generated event identity, original source whole and the inherited step
whole as distinct evidence. Empty subject sampling must still complete honestly.
Subdivision operators must preserve authentic child observation clocks and
returned copy identities; they must not requery per subdivision or retrospectively
divide inner Index observations. Striate's additional full-cycle ranking query
must use genuine replay/work accounting with no independent callback replay.
LoopAt/Fit retain current sample-header speed encoding semantics.

## Tasks

### TASK-001: Extract exhaustive dispatch

**Status**: Not Started
**Parallelizable**: No

- [ ] Confirm manifest, private ownership and test construction with author.
- [ ] Extract dispatch with all existing branches and barriers unchanged first.
- [ ] Preserve metering, failure restoration and public query behavior.

### TASK-002: Actual sampling and unchanged child clocks

**Status**: Not Started
**Parallelizable**: No; depends on TASK-001

- [ ] Euclid passes its actual step timing event to structural sampling.
- [ ] Remove seven barriers only after their actual clock behavior is proved.
- [ ] Preserve each operator's output wholes, controls and occurrence identities.
- [ ] Keep Chunk Unknown until its separate selection proof is implemented.

### TASK-003: Genuine clock and replay evidence

**Status**: Not Started
**Parallelizable**: No; depends on TASK-002

- [ ] Real evaluated/frozen Index fixtures cover all seven operators.
- [ ] Cover empty subjects, nested Fast/Rev/weighted steps, half-open boundaries
  and sampling points, and exact partition invariance.
- [ ] Callback-denied replay proves no independent ranking/source rereads.
- [ ] Full actual operation exact/one-less work and inherited depth failures pass.
- [ ] Preserve existing pattern operator, trace-ranking and song geometry tests.
- [ ] Independent native/lint/WASM/format and relevant regression gates pass.

## Completion criteria

- [ ] All three tasks and actual owning fixtures pass on unchanged held inputs.
- [ ] Every removed barrier has actual canonical Index clock evidence.
- [ ] Chunk selection, routing consumption, pre-Reserve admission and useful
  varying seeds remain required for the full goal.

## Progress log

### 2026-10-03 — Actual operator gap audited

Read-only review locates all eight barriers at song_clock.rs892–899. Euclid
builds step whole/part and producer before bare sample_child. Ply/Arp/Chop split
returned events after unchanged-time child queries. Striate keeps timing but
also ranks via a full-cycle query; LoopAt/Fit only add speed controls. Chunk
queries both branches then selects by returned output anchor, so its discarded
observations require a separate applicability proof rather than an affine map.
Existing pattern tests and grid source-route fixtures supply syntax/regressions,
but do not establish canonical Index support for these operators. This bounded
draft records required work without releasing concurrent source edits.
