# Structural sampling and unchanged child clocks

**Status**: Ready (wave 2, session 249; released after SONG-ROUTE8 joins)
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

## Session 249 executable contract (wave 2)

The source of truth is the design section "Wave 2c: structural clock hooks".
Prerequisites (canonical clock, sampling, query authority) are already
accepted. The plan is scheduled in wave 2, after SONG-ROUTE8, only to keep the
wave-1 cohort exact. Its paths do not overlap with any other plan.

```json
{
  "planId": "SONG-STRUCTURAL-CLOCK",
  "planPath": "impl-plans/active/song-mode-structural-clock-hooks.md",
  "wave": 2,
  "dependsOn": ["SONG-ROUTE8"],
  "writePaths": [
    "src/pattern/eval/song_clock.rs",
    "src/pattern/eval/song_clock/dispatch.rs",
    "src/pattern/combinators/structure.rs",
    "src/pattern/combinators/region.rs",
    "src/pattern/combinators/music.rs",
    "src/pattern/eval/song_clock/structural_tests.rs",
    "impl-plans/active/song-mode-structural-clock-hooks.md",
    "tmp/song-mode-riela/session249-structural-intent.json",
    "tmp/song-mode-riela/session249-structural-receipt.json",
    "tmp/song-mode-riela/session249-structural-build.log",
    "tmp/song-mode-riela/session249-structural-clippy.log",
    "tmp/song-mode-riela/session249-structural-nextest-focused.log",
    "tmp/song-mode-riela/session249-structural-nextest-full.log",
    "tmp/song-mode-riela/session249-structural-wasm.log",
    "tmp/song-mode-riela/session249-structural-fmt.log"
  ],
  "sharedPaths": []
}
```

### Intent and context

`QState::with_clock_dispatch` (`song_clock.rs:885`) sets the canonical clock to
`Unknown` for eight operators (`song_clock.rs:892-899`): Euclid, Ply, Arp, Chop,
Striate, LoopAt, Fit and Chunk. Songs that apply these operators to Index or
Slice music therefore cannot be routed. The aim is to remove the seven barriers
whose real clock behavior can be proved, and keep Chunk.

### Non-goals

- No Chunk support.
- No changes to routing, snapshot or replay files.
- No new public operators.
- No changes to output wholes, controls, occurrence identities or the
  LoopAt/Fit speed-control encoding.
- No `allow`/`expect` attributes.

### File-level changes

1. **TASK-001: extract dispatch, with no behavior change.**
   - Move `with_clock_dispatch` and its helpers into
     `song_clock/dispatch.rs`, keeping the same signature and every branch.
   - Register the module in `song_clock.rs` (994 lines).
   - Run the focused tests before making any semantic change, and record the
     result as the extraction checkpoint in the receipt.
2. **`structure.rs`, Euclid**: pass the actual step timing event, whole and
   part to `QState::with_structural_sample` (`song_clock.rs:751`) instead of a
   bare `sample_child`. Keep the generated event identity, the original source
   whole and the inherited step whole as separate pieces of evidence.
3. **`region.rs` (Chop at line 91, Striate at 98, LoopAt at 141, Fit at 147),
   `music.rs` (Arp at line 160) and `structure.rs` (Ply at line 62)**
   - The child is queried in unchanged time, so the child clock passes through
     unchanged.
   - Never re-query per subdivision, and never retroactively divide inner Index
     observations.
   - Striate's full-cycle ranking query must use the existing replay/work
     accounting. It must not replay the callback independently.
   - If an operator needs no combinator edit, removing its dispatch barrier is
     enough, but its evidence test is still required.
   - `src/pattern/combinators/input.rs` lists these operators at lines 121-130.
     It is not a writePath and must not be edited.
4. **`dispatch.rs`**: remove exactly the seven barriers that have passing
   evidence. Chunk stays `Unknown`.
5. **`structural_tests.rs`**: register it in `song_clock.rs` with
   `#[cfg(test)] mod structural_tests;`.

### Code to imitate

- The existing sampled-source fixtures in `song_clock/tests.rs`.
- The callback-read denial pattern in
  `src/song/snapshot/occupancy/geometry_tests/domains.rs` (`deny(snapshot)`
  plus a `reads.get() == 0` assertion).

### Key pitfalls

- An empty subject sampled by Euclid must still complete honestly: no events
  and no false observation.
- Half-open boundaries: an event at exactly the step end belongs to the next
  step.
- Nested Fast, Rev and weighted steps must keep their orientation.
- Partition invariance: the union of split queries equals a single query.
- Never default an uninstrumented path to identity. It stays `Unknown`.

### Tests to add (in `structural_tests.rs`, with evaluated or frozen Index fixtures)

- For each of Euclid, Ply, Arp, Chop, Striate, LoopAt and Fit applied to a
  Slice source: the canonical clock is not `Unknown`, and the projected source
  START and whole equal the hand-computed values.
- Euclid over an empty subject: zero events and no observation, and the query
  succeeds.
- Fast, Rev and weighted steps nested inside Euclid and Chop: orientation and
  boundaries are correct at the half-open edges.
- A query split at a step boundary equals one query (the partition-invariance
  case).
- Callback-denied replay over Striate's ranking query: zero callback reads.
- Chunk over a Slice source: the clock is still `Unknown`, and it refuses
  truthfully.
- Exact and one-less work, and inherited depth, for a full Euclid-over-Slice
  query: exact succeeds and one less fails.

### Verification (run in the foreground; record the exit status and full log path)

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-structural-build.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-structural-clippy.log 2>&1`
  must report no diagnostic in this plan's paths. Route8 dead-code entries made
  by other plans may remain; list them.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/song_clock|combinators/)' > tmp/song-mode-riela/session249-structural-nextest-focused.log 2>&1`
  must exit 0 with a nonzero count. It must cover every existing pattern
  operator, trace-ranking and song geometry test.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session249-structural-nextest-full.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-structural-wasm.log 2>&1` must exit 0.
- `rustfmt --edition 2021 --check <the six Rust writePaths> > tmp/song-mode-riela/session249-structural-fmt.log 2>&1`
  passes when no `Diff in` line names a touched path.
- `wc -l` on all six paths: each must be below 1000.
  `song_clock.rs` must shrink after the extraction.

### Shared-branch protocol

Same as SONG-ISSUED-RESOLUTION: an intent JSON with the original texts and
hashes, a fresh read and hash check before every edit, no edits to other
workers' files, no crate-wide format, no stash/checkout/reset, and updates to
this plan's log only.

### Done criteria

- [ ] Dispatch is extracted and the extraction checkpoint is recorded.
- [ ] Exactly seven barriers are removed, and Chunk remains.
- [ ] All seven test bullets above are present and pass.
- [ ] The existing operator tests are unchanged and pass.
- [ ] Build, full tests and WASM exit 0.
- [ ] fmt is clean on touched paths and every file is below 1000 lines.
- [ ] No new `allow`/`expect` attributes.
- [ ] The progress log is updated.
