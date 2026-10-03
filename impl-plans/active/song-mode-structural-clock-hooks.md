# Structural sampling and unchanged child clocks

**Status**: In Progress (wave 2, session 250)
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

The session-250 implementation follows the accepted six-path manifest below.
Post-semantic behavioral gates are pending because the shared tree currently
fails compilation in files owned by other implementation plans.

## Proposed bounded manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/pattern/eval/song_clock.rs` | Extract dispatch and register children | Complete |
| `src/pattern/eval/song_clock/dispatch.rs` (new) | Exhaustive dispatch; remove only proven barriers | In Progress |
| `src/pattern/combinators/structure.rs` | Genuine Euclid sampling hook and subdivision evidence | In Progress |
| `src/pattern/combinators/region.rs` | Chop/Striate/LoopAt/Fit owning behavior | Evidence pending; unchanged-time pass-through |
| `src/pattern/combinators/music.rs` | Arp owning behavior | Evidence pending; unchanged-time pass-through |
| `src/pattern/eval/song_clock/structural_tests.rs` (new) | Actual Index clock regressions | Written; blocked at compile gate |

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

**Status**: Complete
**Parallelizable**: No

- [x] Confirm the accepted manifest and dependency admission from the runtime plan contract.
- [x] Extract dispatch with all existing branches and barriers unchanged first.
- [x] Preserve metering, failure restoration and public query behavior.

### TASK-002: Actual sampling and unchanged child clocks

**Status**: In Progress
**Parallelizable**: No; depends on TASK-001

- [x] Euclid passes its actual step timing event to structural sampling.
- [x] Remove exactly seven dispatch barriers; Chunk remains Unknown.
- [ ] Preserve each operator's output wholes, controls and occurrence identities.
- [ ] Keep Chunk Unknown until its separate selection proof is implemented.

### TASK-003: Genuine clock and replay evidence

**Status**: In Progress
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

### 2026-10-03 — Session 250 implementation and verification handoff

Extracted `QState::with_clock_dispatch` into `song_clock/dispatch.rs`; the
focused extraction checkpoint ran before semantic edits and passed 21/21 with
exit 0. Removed the seven listed dispatch barriers while retaining Chunk's
Unknown barrier. Euclid now passes its generated step event, original step
whole, and inherited part through `with_structural_sample`. Added seven
requirement-level tests in `structural_tests.rs`, including operator Index
projection, empty Euclid, nested timing, split-window replay, Striate callback
denial, Chunk Unknown and exact/one-less Euclid work.

Post-semantic focused nextest exited 101 because the shared tree was being
written by other plan owners: first a missing `src/song/snapshot/issued/`
child/re-export, then unresolved `FrozenIssuedSongEvent` in
`src/song/routing/source/issued.rs`. The current native build also exits 101
with private-field/type errors in `src/song/routing/configuration/index/canonical.rs`
and a missing `issued_leaves` field in `src/song/routing/source/issued.rs`.
These files are outside this plan's six Rust write paths and were not edited.
Resume post-semantic verification after those owners complete their source
seams and the shared tree compiles. The final rustfmt `--check` exits 0. Earlier
failed command attempts remain in the plan-local logs; no gate is marked passed
based on those failures.

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
  "dependsOn": ["SONG-ROUTE8", "SONG-ISSUED-RESOLUTION", "SONG-SHARED-WORK"],
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

## Session 251 amendment (wave 2c, serial)

Source of truth: design section "Session 251 resume amendments". This plan
starts only after 2a and 2b are joined and committed. It runs alone
(concurrency 1).

- Add one test to `structural_tests.rs`: a Euclid-over-Slice query with
  inherited depth `max_depth - 1` succeeds, and the same query with inherited
  depth `max_depth` fails with `FailCode::DepthExceeded`. It publishes no
  events, and the collector keeps the work spent before the refusal. The
  existing exact/one-less work test stays unchanged.
- Depth test harness and pitfalls:
  - Harness: copy the minimal `Fixture` setup (`code`, `payload`, `collect`)
    from `src/pattern/eval/song_clock/tests.rs` into `structural_tests.rs` as a
    private test-only helper. It compiles without visibility changes because
    `tests.rs` and `structural_tests.rs` are both direct child modules of
    `song_clock` (`pub(crate) mod tests;` and `mod structural_tests;` in
    `song_clock.rs`), so anything `tests.rs` can name is also nameable from
    `structural_tests.rs`. Copy only the fields and methods the depth test
    needs. Call `collect(TimeSpan::cycle(0).unwrap(), limits,
    limits.max_nodes, depth)` with `limits = SongLimits::default()` and the
    program `indexed_song("euclid {slice {beat -> p} 2 [cut nil]} 3 8 1")`,
    following `sampling_exact_work` (`tests.rs`, depth 0). Use a fresh fixture
    for each depth. Do not use `PreparedSong::query_issued_with_work` for this
    test: its existing callers need private snapshot retention (the private
    `retain` helper in `shared_work_tests.rs`) that is unreachable from this
    module.
  - Do not edit `src/pattern/eval/song_clock/tests.rs`. It is not a writePath,
    and editing it breaks the join hash audit. Do not make its private
    `Fixture` or `Fixture::collect` visible. Keep `structural_tests.rs` below
    1000 lines.
  - Never relax, move or remove any depth check (`depth >= max_depth` or
    `depth > max_depth`, for example in `song_clock.rs` `copy_issued_clock`,
    `src/song/limits.rs` and the `prepare_owner_dependencies` and
    `observe_part` depth paths) to make the boundary pass. Nested evaluation passes `depth + 1` internally, so
    `max_depth - 1` may refuse on the unmodified code. If it does, stop and
    record a blocker in this plan's progress log with the exact failure message
    and depth. Do not change the boundary numbers.
  - Assertions. At depth `max_depth - 1`, the result is `Ok`, and `rows(&work)`
    (the existing helper in `structural_tests.rs`, which filters to the
    genuine target owner) is non-empty with no
    `CanonicalClockProjection::Unknown` clock. At depth `max_depth`, the
    result's error code is `FailCode::DepthExceeded`, `rows(&work)` is empty
    (nothing published), and `work.borrow().remaining()` is at most the
    starting budget (the debit is kept, never refunded). In both cases
    `fixture.evaluator.vm_and_ns().0.song_work().is_none()` holds afterward,
    as in `sampling_exact_work`. If the failing case publishes rows, record a
    blocker in the progress log instead of weakening the assertion.
- The seven barrier removals and the Chunk `Unknown` barrier from session 250
  are kept. Review checks that each removed barrier has its evaluated or frozen
  Index evidence and callback-denied replay. If a removed barrier has none,
  restore it.
- Clippy: no diagnostic in this plan's six paths. After this sub-wave, no
  `ST-` row may remain.
- Before rerunning the gates, the orchestrator copies the existing
  `tmp/song-mode-riela/session249-structural-*` files to
  `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session250/` and records their
  sha256 values. Then rerun the focused, build, Clippy, full nextest, WASM,
  fmt and `wc -l` commands above in the foreground. Record each exit status
  and full log path.

### Session 251 test cases (input -> expected outcome)

- A Euclid-over-Slice query at inherited depth `max_depth - 1` -> `Ok`, with a
  non-`Unknown` clock and observation rows.
- The same query at inherited depth `max_depth` -> `Err` with
  `FailCode::DepthExceeded`, no observation rows, and `remaining()` at most
  the starting budget.
- The seven existing session-250 tests in `structural_tests.rs` -> unchanged
  and passing.

### Session 251 done criteria (mechanically checkable)

- [ ] `grep -n "DepthExceeded" src/pattern/eval/song_clock/structural_tests.rs`
  matches the new test.
- [ ] `grep -n "Chunk" src/pattern/eval/song_clock/dispatch.rs` still shows the
  `Unknown` barrier, and exactly seven operators lost theirs. Check this with
  `git diff 37ea3e8 -- src/pattern/eval/song_clock.rs src/pattern/eval/song_clock/dispatch.rs`.
- [ ] Focused, build, strict clippy (no diagnostic in the six paths, no `ST-`
  row), full nextest and WASM all exit 0. `rustfmt --check` is clean, and each
  of the six paths is < 1000 lines.
- [ ] `git diff 1ac457f -- src/pattern/combinators/input.rs` is empty. No new
  `allow`/`expect`.
- [ ] `git diff 1ac457f -- src/pattern/eval/song_clock/tests.rs` is empty.
- [ ] The receipt and a progress-log entry are written.
