# Structural sampling and unchanged child clocks

**Status**: In Progress (session 258: runs SECOND, after SONG-ISSUED-RESOLUTION is accepted; TASK-016 fixes the format hunks and reruns the gates with zero allowed failures; see "Session 258 amendment")
**Created**: 2026-10-03
**Last Updated**: 2026-10-04
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

The implementation follows the accepted manifest below. Session 255 corrected
the Euclid projection fixtures and unsupported-sampling fixture. Session 257
added the authorized process ID to the export and CLI test temp-directory
names. The acceptance rerun passed the focused, route-regression, harness,
native and WASM gates; full nextest completed with only the six allowed
SONG-ISSUED-RESOLUTION failures. Receipt:
`tmp/song-mode-riela/session249-structural-receipt.json`.

## Proposed bounded manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/pattern/eval/song_clock.rs` | Extract dispatch and register children | Complete |
| `src/pattern/eval/song_clock/dispatch.rs` (new) | Exhaustive dispatch; remove only proven barriers | Complete |
| `src/pattern/combinators/structure.rs` | Genuine Euclid sampling hook and subdivision evidence | Complete |
| `src/pattern/combinators/region.rs` | Chop/Striate/LoopAt/Fit owning behavior | Complete; unchanged-time pass-through evidenced |
| `src/pattern/combinators/music.rs` | Arp owning behavior | Complete; unchanged-time pass-through evidenced |
| `src/pattern/eval/song_clock/structural_tests.rs` (new) | Actual Index clock regressions | Complete; session 257 gates pass |

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

**Status**: Complete (session 257)
**Parallelizable**: No; depends on TASK-001

- [x] Euclid passes its actual step timing event to structural sampling.
- [x] Remove exactly seven dispatch barriers; Chunk remains Unknown.
- [x] Preserve each operator's output wholes, controls and occurrence identities.
- [x] Keep Chunk Unknown until its separate selection proof is implemented.

### TASK-003: Genuine clock and replay evidence

**Status**: Complete (session 257)
**Parallelizable**: No; depends on TASK-002

- [x] Real evaluated/frozen Index fixtures cover all seven operators.
- [x] Cover empty subjects, nested Fast/Rev/weighted steps, half-open boundaries
  and sampling points, and exact partition invariance.
- [x] Callback-denied replay proves no independent ranking/source rereads.
- [x] Full actual operation exact/one-less work and inherited depth failures pass.
- [x] Preserve existing pattern operator, trace-ranking and song geometry tests.
- [x] Independent native/lint/WASM/format and relevant regression gates pass.

## Completion criteria

- [x] All three tasks and actual owning fixtures pass on unchanged held inputs.
- [x] Every removed barrier has actual canonical Index clock evidence.
- [ ] Chunk selection, routing consumption, pre-Reserve admission and useful
  varying seeds remain required for the full goal.

## Progress log

### 2026-10-04 — Session 257 implementation and acceptance rerun

Applied the operator-authorized harness fix in `tests/song_export.rs` and
`tests/song_cli.rs`: `Directory::new` now includes `std::process::id()` before
the existing per-process counter. Each diff is exactly 2 additions/1 deletion;
assertion counts are unchanged (29 and 20), and the files are 197 and 160
lines. The plan-local pre/post hashes and intended hunks are recorded under
`tmp/song-s249/SONG-STRUCTURAL-CLOCK/session257/`.

Session 257 acceptance gates passed: focused structural/combinator/domain
selection 37/37; legacy route regression binaries 105/105; harness binaries
10/10 and five repeated runs 50/50; native build and WASM build exit 0. Full
nextest completed with 2,793 run, 2,787 passed, six failed and three skipped;
the failure set is exactly the six allowed `SONG-ISSUED-RESOLUTION` tests, with
no `song_export` or `song_cli` failure. Strict Clippy exits 101 with no
diagnostic in a 2c path; all diagnostics map to Route8, RES or SW. Structural
rustfmt `--check` reports only unchanged baseline hunks at
`structural_tests.rs:30` and `:95`; both harness files pass `--check`. The
cohort is 953, all Rust files are below 1,000 lines, frozen/unowned path checks
and the prior five-path Rust-diff check pass. The receipt records the new
session-257 fingerprint and full logs.

Key commands and logs:

- `CARGO_TERM_QUIET=true cargo build` — exit 0;
  `tmp/song-mode-riela/session249-structural-build.log`.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` — exit
  101, zero 2c diagnostics;
  `tmp/song-mode-riela/session249-structural-clippy.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast` — exit 100 under the exact six-ID allowance;
  `tmp/song-mode-riela/session249-structural-nextest-full.log`.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit 0;
  `tmp/song-mode-riela/session249-structural-wasm.log`.
- `rustfmt --edition 2021 --check` on declared structural paths — exit 1 on
  unchanged baseline hunks; changed-line comparison passed. Harness-file
  `rustfmt --check` exits 0. Both logs are in
  `tmp/song-mode-riela/session249-structural-fmt.log`.
- Focused, route regression, and harness commands and counts are recorded in
  the receipt and `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session257/`.

TASK-012 through TASK-015 are complete for this plan. Test-integrity,
adversarial and integration review remain workflow-owned; the accepted
manifest's downstream issued-resolution work is not part of this predecessor.

### 2026-10-04 — Session 256 acceptance rerun

Reran the assigned gates on Rust sources identical to `88f5120` (current HEAD
`09fc99f8e4336c2be8db68040279e6cf572d6272` is the committed documentation
checkpoint). No Rust implementation was changed. The focused structural,
combinator and domains selection passed 37/37; native build passed; legacy
route regression binaries passed 105/105; WASM build passed. Clippy exited 101
with no diagnostic in a 2c path; all reported paths map to Route8, RES or SW.
`rustfmt --check` exited 1 only for unchanged hunks at
`structural_tests.rs:30` and `:95`; the `git diff -U0 c7083fb` comparison shows
no overlap with changed lines. File counts are 939, 61, 470, 526, 442, 353,
896 and 675; cohort is 953; frozen/unowned and V11 Rust-diff checks pass.

Full nextest completed with 2,793 run, 2,786 passed, 7 failed and 3 skipped.
Six failures are the exact allowed SONG-ISSUED-RESOLUTION IDs. The additional
`fractional_song_and_tail_write_exact_partial_block` fails in
`tests/song_export.rs:20` with `AlreadyExists`, outside this plan's writePaths
and not in the six-ID allowance. Therefore TASK-012/TASK-013 remain In Progress;
no out-of-scope test/source edit was made. The resume criterion is to assign an
owner to triage the export regression, then rerun full nextest and confirm the
failure set is a subset of the six allowed resolver IDs.

Gate evidence: V1 focused (`exit 0`, 37/37) and V5 route regressions (`exit 0`,
105/105) are in `tmp/song-mode-riela/session249-structural-nextest-focused.log`;
V2 build (`exit 0`) in `session249-structural-build.log`; V3 strict Clippy
(`exit 101`, 0 plan-path diagnostics) in `session249-structural-clippy.log`;
V4 full nextest (`exit 100`, seven failures) in
`session249-structural-nextest-full.log`; V6 WASM (`exit 0`) in
`session249-structural-wasm.log`; V7 rustfmt (`exit 1`, unchanged-only hunks)
in `session249-structural-fmt.log`. V8 line counts, V9 frozen/unowned paths,
V10 cohort, and V11 no-Rust-diff results are in
`tmp/song-s249/SONG-STRUCTURAL-CLOCK/session256/` and the session256 receipt.
Archived session255 artifacts and their hashes remain in
`attempt-session255/`; its original `session255/` directory was preserved.

### 2026-10-04 — Session 255 implementation handoff

Applied the accepted D4 test corrections in `structural_tests.rs`: added the
Euclid-only projection helper; kept unchanged-clock assertions for the six
time-preserving operators; asserted the empty-subject Euclid's exact one row;
and searched the inherited-depth boundary over 32 fresh fixtures, checking B,
B+1 and the `max_depth` refusal. Swapped only the unsupported-sampling test's
Euclid input to the accepted Chunk fixture in `song_clock/tests.rs`; its
assertions and faulted-frame half are unchanged.

The focused structural/combinator/domain filter passes 37/37. The separate
route regression selection passes 105/105. Native and WASM builds exit 0.
Strict Clippy exits 101 with no diagnostic in a 2c path; other diagnostics
are in the Route8/RES/SW disposition paths listed in the receipt. Full
nextest completes with 2,787 passed and six failed. Five are the named
downstream 2a resolver tests. The sixth,
`song::routing::source::issued::tests::genuine_source_contributions_reject_a_foreign_transcript`,
is outside this plan's writePaths and not in the allowed five-test list; the
serial join is blocked until its owner repairs or formally dispositions it.
Formatting reports only pre-existing hunks in `structural_tests.rs` lines 30
and 95, with no hunk on a changed line. Line counts, the 953-file cohort, the
one-line fixture diff, frozen paths and no-new-allow/expect checks pass. Full
logs and source snapshots are under
`tmp/song-s249/SONG-STRUCTURAL-CLOCK/session255/` and
`tmp/song-mode-riela/session249-structural-*`.

### 2026-10-04 — Session 254 implementation handoff

Reviewed the committed partial structural-clock implementation. `dispatch.rs`
returns `true` only for Chunk; Euclid passes its generated timing event, step
whole and part to `with_structural_sample`; the six unchanged-time operators
do not requery per subdivision. The existing Striate fixture uses
`sampling_with_replay`, whose replay phase denies callback reads. No finding
required a production edit.

Rewrote the six parser fixtures to the real Euclid and Chunk arities without
changing existing assertions or test names. Added the inherited-depth test
with a private fixture and VM work cleanup checks. Changed the unsupported
domains fixture from Euclid to Chunk, kept its four refusal checks and the
`range saw` case unchanged, and added positive Euclid consume evidence.

The baseline reproduction exited 100 with 14 selected tests: 7 passed and 7
failed (six fixture arity errors and the stale Euclid refusal expectation).
After the import correction, the focused session-254 confirmation exited 100:
16 tests ran, 12 passed and 4 failed. The failures are:

- `euclid_over_slice_obeys_inherited_depth_boundary_without_refunding_debit`:
  `max_depth - 1` returned `DepthExceeded` (`pattern nesting too deep`) at
  `structural_tests.rs:274`.
- `euclid_empty_subject_has_no_false_index_observation`: the expected empty
  observation set was non-empty at `structural_tests.rs:205`.
- `split_queries_retain_the_single_query_index_rows`: projection ended at
  `1/4`, expected `1/2`, at `structural_tests.rs:180`.
- `nested_fast_rev_weighted_euclid_and_chop_keep_clock_orientation`:
  projection ended at `1/8`, expected `1/2`, at `structural_tests.rs:180`.

The plan directs stopping on any post-arity behavioral failure and forbids
weakening assertions. Therefore build, Clippy, full nextest, focused regression
suite, WASM and rustfmt gates were not run. The 953-file cohort, line budgets,
unowned-path diff, unchanged `tests.rs`/`input.rs`, and no-new-allow/expect
checks passed. Session-254 implementation remains incomplete pending diagnosis
and an authorized correction to the Euclid/depth projection behavior, followed
by the unrun gates.

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

## Session 252 amendment (wave 2c, serial)

Source: the design section "Session 252 resume amendments" (Downstream seams)
and the 2a joint-geometry rule in `song-mode-issued-route-resolution.md`. This
plan starts only after 2b is joined and committed. The six writePaths, the
seven-barrier scope, Chunk staying Unknown and the Euclid depth boundary test
are unchanged. The session-251 partial code at `980083a` in
`song_clock/dispatch.rs` and `structural_tests.rs` is unreviewed. It is
reviewed against the design before the gates run.

### Declared seam (conditional sharedPath)

**`src/song/routing/nested/issued.rs`** (a 2a writePath, 941 lines at
`980083a`). Use it only if the 2a receipt records
`jointGeometryFixture: "euclid-deferred-to-2c"`. Then:

- After Euclid's barrier is removed, append one test,
  `issued_euclid_joint_geometry_resolves_after_structural_clock`, to the
  existing `#[cfg(test)] mod tests`. It reuses the original session-251
  Euclid program:
  `fn indexed p:\n\teuclid {slice {beat -> p} 2 [0 nil]} 1 2` with a
  `base` of duration 4.
- It asserts the same four facts as
  `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`:
  - legacy `prepare_routes` is `Ok`;
  - the legacy `resolve_route` error contains
    `sampled context requires joint mapping geometry`;
  - `resolve_issued_event` is `Ok`;
  - the handle is the same.
- Imitate that test's body. Copy it; do not call into it.
- Change no existing line. The file must stay below 1000 lines, so the new
  test may be at most 50 lines.
- If 2a kept the Euclid fixture, this path stays unedited and the receipt
  records `"nested/issued.rs": "declared, unedited"`.

If it is edited, add it to the focused filter
(`-E 'test(/song_clock|combinators|routing::nested::issued/)'`), the
`rustfmt --check` command and `wc -l`. Never run rustfmt in write mode on it.

### Test case

- 2a deferred Euclid, and this sub-wave removed the Euclid barrier: the Euclid
  joint-geometry program resolves on the issued path, and legacy still refuses
  with the barrier message.
- If it still refuses with `original source membership missing`, the Euclid
  hook is not genuine. Fix the hook in this plan's paths. Do not change the
  resolver or the test.

### Session 252 done criteria

- [ ] The Euclid depth boundary test is present and passes (session 251
  criterion).
- [ ] If the 2a receipt deferred Euclid, `grep -n
  "issued_euclid_joint_geometry_resolves_after_structural_clock"
  src/song/routing/nested/issued.rs` matches and the test passes. Otherwise
  `git diff 980083a -- src/song/routing/nested/issued.rs` shows only 2a's
  committed changes, which means this sub-wave made none.
- [ ] No `ST-` row remains. Strict Clippy has no non-dead-code lint in this
  plan's paths.

## Session 253 note (wave 2c, serial)

Source: the design section "Session 253 resume amendments". The scope, paths,
tests and done criteria of this plan are unchanged. Only these facts change:

- 2a adds the child module `src/song/routing/nested/issued/members.rs`. It is a
  2a file. This plan never edits it.
- 2a leaves `src/song/routing/nested/issued.rs` at no more than 949 lines. The
  conditional Euclid witness (at most 50 lines) must keep the file below 1000.
- Use the conditional seam only if the 2a receipt records
  `"euclid-deferred-to-2c"`. When it is used:
  - The witness copies the restored 2a joint-geometry test: one Slice under
    one sampling combinator, the first event, and the same four assertions.
  - It does not copy the session-251 nested-slice form.
- `rustfmt --check` on `nested/issued.rs` also checks `members.rs`. A `Diff in`
  hunk reported for `members.rs` is recorded, not fixed. It belongs to 2a.
- The cohort is 953 at the start of this sub-wave and stays 953.
- `git diff --quiet` checks for the unowned paths use `6543273` or the 2a join
  commit as the base, not `980083a`.

## Session 254 amendment (runs FIRST; serial, concurrency 1)

The source of truth is the design section "Session 254 resume amendments
(2026-10-04)", subsection "Structural clock (2c) repair". Where this section
conflicts with an earlier amendment of this plan, this section wins.

### Intent and context

- The serial order is now: this plan, then SONG-ISSUED-RESOLUTION, then
  SONG-SHARED-WORK, then SONG-ISSUED-PLAYBACK, then SONG-16.
- This plan depends only on SONG-ROUTE8.
- The base is `a8b8ed2`. The cohort is 953 and stays 953.
- The partial code committed at `1ac457f` already extracted dispatch, removed
  the seven barriers and wired Euclid. That code breaks full nextest in two
  ways. Each failure is quoted from
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/full-nextest-post-helper.log`.
  - The fixtures in `structural_tests.rs` use invalid arities or values:
    - `euclid` takes 3 arguments, found 4;
    - `chunk` takes 3 arguments, found 2;
    - `euclid steps must be between 1 and 4096` (the nested test).
  - `geometry_tests::domains::unsupported_and_continuous_index_evidence_refuse_without_callback_fallback`
    still expects Euclid to refuse. Euclid is now intentionally supported.
- That run stopped at fail-fast after 1229 of 2791 tests. More failures may
  exist, so every full run here uses `--no-fail-fast`.

### Scope decisions (pinned; do not reopen)

- **D3: no new arities.** Rewrite the fixtures to the real builtins:
  - `euclid` takes pattern, pulses and steps, plus the `rotation:` keyword
    (default 0). See `src/types/natives_domain.rs:214` and
    `src/vm/natives/pattern.rs:271-278`.
  - `chunk` takes pattern, divisions and a function. See
    `src/types/natives_domain.rs:198`.
  - Do not edit `natives_domain.rs`, `vm/natives/pattern.rs` or `infer_call.rs`.
- **D4: keep Euclid instrumented.** Do not restore its barrier in
  `dispatch.rs`. Instead, own the `domains.rs` expectation change as a declared
  behavior change, and back it with replacement evidence.
- **The 2c sharedPath `src/song/routing/nested/issued.rs` is removed.** So is
  the conditional `issued_euclid_joint_geometry_resolves_after_structural_clock`
  witness. 2a runs after this plan and keeps its own Euclid fixture. Do not
  touch `nested/issued.rs` or `nested/issued/members.rs`.

### Owned paths for session 254

| Path | Kind | Allowed edit |
|---|---|---|
| `src/pattern/eval/song_clock/structural_tests.rs` | writePath | Fixture strings only (TASK-004), plus the Euclid depth-boundary test (TASK-005), if it is absent |
| `src/song/snapshot/occupancy/geometry_tests/domains.rs` | writePath (new for this plan) | Only the one test named below (TASK-006) |
| `src/pattern/eval/song_clock/dispatch.rs` | writePath | Review only. Edit only if review finds a barrier removed without evidence; then restore that one barrier. |
| `src/pattern/eval/song_clock.rs`, `src/pattern/combinators/structure.rs`, `src/pattern/combinators/region.rs`, `src/pattern/combinators/music.rs` | writePaths | Review only. Edit only to fix a defect that a 2c test proves. |

There are no sharedPaths. These are never edited:

- `src/pattern/eval/song_clock/tests.rs`, which holds the shared harness, the
  `Fixture` type and `sampling_with_replay`;
- `src/pattern/combinators/input.rs`;
- every 2a, 2b or wave-3 path;
- the three unowned paths;
- `.agents/settings.local.json`.

### TASK-004: Reconcile structural fixtures with the real arities

**Status**: Implemented; focused behavioral checks found non-arity failures
**Parallelizable**: No
**Deliverable**: `src/pattern/eval/song_clock/structural_tests.rs`

Make exactly these string rewrites. Each keeps the program's meaning, so no
assertion changes:

| Test | Old body fragment | New body fragment |
|---|---|---|
| `seven_structural_operators_preserve_slice_clocks` | `euclid {slice {beat -> p} 2 [cut nil]} 1 2 0` | `euclid {slice {beat -> p} 2 [cut nil]} 1 2` |
| `euclid_empty_subject_has_no_false_index_observation` | `... 1 2 0` | `... 1 2` |
| `nested_fast_rev_weighted_euclid_and_chop_keep_clock_orientation` | `... 3} 3 0` (3 pulses over 0 steps) | `... 3} 3 8` |
| `split_queries_retain_the_single_query_index_rows` | `... 2} 2 4 0` | `... 2} 2 4` |
| `chunk_slice_clock_stays_unknown` | `chunk {slice {beat -> p} 2 [cut nil]} 2` | `chunk {slice {beat -> p} 2 [cut nil]} 2 {q -> fast q 2}` |
| `euclid_slice_query_charges_exact_and_one_less_work` | `... 3 8 1` | `... 3 8 rotation: 1` |

- Apply the `rotation: 1` form to the depth-boundary program as well
  (TASK-005).
- Imitate these existing forms:
  - `tests/song_source_conditionals.rs:100` for `chunk p 2 {q -> fast q 2}`;
  - `tests/song_source_sampling.rs:80` for `rotation:`.
- Pitfalls:
  - Do not change any `assert!`, helper or test name.
  - Do not add a fourth positional argument anywhere.
  - Do not change the Index lists (`[cut nil]`). This harness evaluates
    through `song_clock`, not through routing preparation, so SM4 does not
    apply here.
- If a rewritten test then fails for any reason other than arity, stop and
  record the exact message in the progress log. Never edit an assertion to
  make it pass.

### TASK-005: Euclid inherited-depth boundary test

**Status**: Implemented; required `max_depth - 1` case fails with `DepthExceeded`. Check with
`grep -n DepthExceeded src/pattern/eval/song_clock/structural_tests.rs`.
**Parallelizable**: No (after TASK-004)

The session 251 contract is unchanged:

- Use the program `euclid {slice {beat -> p} 2 [cut nil]} 3 8 rotation: 1`.
- At depth `max_depth - 1`, the result is `Ok`. Rows are non-empty and have
  no `Unknown` clock.
- At depth `max_depth`, the result fails with `FailCode::DepthExceeded`,
  publishes no rows, and leaves `remaining()` no greater than the start value.
- The session 251 harness rules apply: copy a minimal private fixture into
  `structural_tests.rs`, and do not edit `tests.rs`.
- Never relax a depth check. If `max_depth - 1` refuses on unmodified code,
  stop and report.

### TASK-006: Declared Euclid behavior change in `domains.rs`

**Status**: Implemented; Chunk refusal and positive Euclid evidence passed the focused run
**Parallelizable**: No (after TASK-004)
**Deliverable**: `src/song/snapshot/occupancy/geometry_tests/domains.rs`,
only the test
`unsupported_and_continuous_index_evidence_refuse_without_callback_fallback`
(currently lines 248-320)

1. Keep the `range saw` continuous case byte-identical.
2. Replace the Euclid body in the refusing loop with a still-unsupported
   operator: `chunk {slice {beat -> nil} 1 [cut]} 2 {q -> fast q 2}`. The
   refusing branch keeps all four assertions unchanged:
   - `failure.code == FailCode::Type`, with its message text;
   - `refused > 0`;
   - `Rc::ptr_eq(&prior, snapshot.replay...)`;
   - `reads.get() == 0`.
3. Add the Euclid replacement evidence as a positive branch in the same
   test.
   - Use the body `euclid {slice {beat -> nil} 1 [cut]} 1 2` and the same
     `retain`, `deny`, `request` and `owner_addresses` sequence.
   - For every address whose `owner().revision == request.revision`,
     `bind_index` then `consume` (same arguments as the refusing branch) must
     return `Ok`. Count these as `accepted`, and assert `accepted > 0`.
   - Assert `reads.get() == 0` and that the replay `Rc` is unchanged.
   - Use a separate loop iteration or an explicit `if body.starts_with("euclid")`
     branch. Imitate the existing `if body.contains("range saw")` split.
4. You may rename the test, but only to a name that still says unsupported
   evidence refuses, for example
   `unsupported_index_evidence_refuses_and_euclid_consumes_without_callback_fallback`.
   Record the old and new names in the receipt.
5. Stop conditions. Do not weaken the test to get past either one.
   - Chunk yields `refused == 0`: stop and report the owner addresses seen.
   - Euclid `consume` errors: stop and report the error. This is a genuine 2c
     hook defect. Fix it in this plan's `song_clock`/`combinators` paths, never
     in snapshot or routing code.
6. In the receipt, classify every diff line against `a8b8ed2` with
   `git diff a8b8ed2 -- src/song/snapshot/occupancy/geometry_tests/domains.rs`.
   Allowed classes: `chunk-substitution`, `euclid-positive-branch`,
   `rename`. Any other changed line fails review.

### TASK-007: Review the committed partial 2c code

**Status**: Reviewed; no structural dispatch or sampling defect found in the reviewed partial code
**Parallelizable**: No (before the gates)

Review `git diff 37ea3e8 a8b8ed2 -- <the six original 2c Rust paths>`
against Wave 2c and record findings in the receipt. Check:

- Exactly seven barriers were removed, and Chunk stays `Unknown`
  (`dispatch.rs:11-14`).
- Euclid calls `with_structural_sample` with the actual step timing event,
  the step whole and the part.
- Ply, Arp, Chop, Striate, LoopAt and Fit query their children in unchanged
  time, with no per-subdivision requery.
- Every removed barrier has a passing evaluated Index test with callback
  denial in `structural_tests.rs`. If one has none, restore that barrier and
  record it.

### Invariants

- Chunk stays `Unknown`. Legacy routing `resolve_route` still refuses sampled
  contexts with `sampled context requires joint mapping geometry`.
- `tests.rs` and `input.rs` are unchanged:
  `git diff a8b8ed2 -- src/pattern/eval/song_clock/tests.rs src/pattern/combinators/input.rs`
  is empty.
- No assertion is removed or loosened anywhere. There is no new `#[allow` or
  `#[expect`.
- Every touched file stays below 1000 lines. `domains.rs` is 634 lines at
  `a8b8ed2`.
- Never run rustfmt in write mode on `domains.rs` or any snapshot file. Use
  `--check` only and hand-format.

### Session 254 tests (input -> expected outcome)

- The six rewritten structural tests and the unchanged
  `striate_ranking_reuses_retained_work_without_callback_reads` -> pass.
- The Euclid depth boundary -> `Ok` at `max_depth - 1`; `DepthExceeded` at
  `max_depth`, with no rows and the debit kept.
- `domains` test, Chunk body -> refused with `FailCode::Type`, zero reads and
  the replay `Rc` unchanged.
- `domains` test, Euclid body -> every address consumes `Ok`, with
  `accepted > 0`, zero reads and the replay `Rc` unchanged.
- `domains` test, `range saw` body -> unchanged refusal mentioning
  `first-structure`.
- Full suite with `--no-fail-fast` -> the only failures allowed are the five
  2a-owned resolver tests:
  - `cached_nested_slice_events_resolve_from_issued_transcript`
  - `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`
  - `distinct_equal_handle_invocations_are_all_resolved`
  - `partitioned_nested_issued_queries_equal_the_full_route_set`
  - `discarded_augmented_source_origins_are_resolved`

  Any other failure stops 2c. The receipt lists it with its owner (`ST-`,
  `RES-`, `SW-` or Route8). A failure in a 2c path is fixed here. Any other
  failure is reported to the orchestrator.

### Session 254 execution steps (in order)

1. Copy `tmp/song-mode-riela/session249-structural-*` to
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session253/` and record their
   sha256 values.
2. Rewrite `tmp/song-mode-riela/session249-structural-intent.json` with:
   - the design sha256 and plan sha256;
   - the sha256 and full text of `structural_tests.rs` and `domains.rs`.

   Re-read and hash-check each file before every edit. On drift, stop and
   reconcile from the current file.
3. Reproduce. Run the following, then record its exit status. Exit 100 is
   expected:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib --no-fail-fast -E 'test(/song_clock::structural_tests|geometry_tests::domains/)' > tmp/song-s249/SONG-STRUCTURAL-CLOCK/session254/reproduce.log 2>&1`
4. Do TASK-007, then TASK-004, TASK-005 and TASK-006, then the verification
   below.
5. Write the receipt `tmp/song-mode-riela/session249-structural-receipt.json`
   with:
   - the arity rewrite table;
   - the `domains.rs` diff classification and the test names;
   - the TASK-007 review findings;
   - the clippy disposition;
   - the full-suite failure list with owners;
   - the cohort count (953) and the line counts;
   - the result of the unowned-path `git diff --quiet`;
   - `evidenceFingerprint`, the sha256 of the receipt. It must differ from
     every earlier structural receipt hash recorded in
     `tmp/song-s249/SONG-STRUCTURAL-CLOCK/`.
6. Add one progress-log entry to this plan.

### Session 254 verification (foreground; record the exit status and full log path)

1. Focused:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast -E 'test(/song_clock|combinators|geometry_tests::domains/)' > tmp/song-mode-riela/session249-structural-nextest-focused.log 2>&1`
   It must exit 0 with a nonzero count.
2. `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-structural-build.log 2>&1`
   must exit 0.
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-structural-clippy.log 2>&1`
   - No diagnostic may name a 2c path, `domains.rs` included.
   - Every other diagnostic maps to an `SW-`, `RES-` or Route8 D-row. For
     example, `SW-let_and_return` at `src/song/snapshot/issued.rs:231` is
     expected.
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-mode-riela/session249-structural-nextest-full.log 2>&1`
   - The run must complete (the `Summary` line shows every test run).
   - The failures must be exactly a subset of the five 2a tests above.
   - At least 425 distinct tests must pass.
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker >> tmp/song-mode-riela/session249-structural-nextest-focused.log 2>&1`
   must exit 0.
6. `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-structural-wasm.log 2>&1`
   must exit 0.
7. `rustfmt --edition 2021 --check src/pattern/eval/song_clock.rs src/pattern/eval/song_clock/dispatch.rs src/pattern/combinators/structure.rs src/pattern/combinators/region.rs src/pattern/combinators/music.rs src/pattern/eval/song_clock/structural_tests.rs src/song/snapshot/occupancy/geometry_tests/domains.rs > tmp/song-mode-riela/session249-structural-fmt.log 2>&1`
   passes when no `Diff in` line names a touched path. A hunk in an untouched
   child, such as `song_clock/tests.rs`, is recorded and not fixed.
8. `wc -l` on the seven Rust paths: each must be below 1000.
9. `git diff --quiet a8b8ed2 -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs src/pattern/eval/song_clock/tests.rs src/pattern/combinators/input.rs`
   must exit 0.
10. `git ls-files -co --exclude-standard -- '*.rs' Cargo.toml Cargo.lock | wc -l`
    prints 953.

### Session 254 done criteria (mechanically checkable)

- [ ] `grep -nE "euclid [^\"]* [0-9]+ [0-9]+ [0-9]+\"|chunk \{[^}]*\}[^{]*\} 2\"" src/pattern/eval/song_clock/structural_tests.rs`
  prints nothing.
- [ ] `grep -n "rotation: 1" src/pattern/eval/song_clock/structural_tests.rs`
  matches at least 2 lines: the exact-work test and the depth test.
- [ ] `grep -n "DepthExceeded" src/pattern/eval/song_clock/structural_tests.rs`
  matches.
- [ ] `grep -n "chunk {slice {beat -> nil} 1 \[cut\]} 2 {q -> fast q 2}" src/song/snapshot/occupancy/geometry_tests/domains.rs`
  matches, and the Euclid body is asserted `Ok` with `accepted > 0`.
- [ ] `dispatch.rs` still returns `true` only for `PatNode::Chunk`.
- [ ] Verification 1, 2, 5, 6, 9 and 10 meet their rules. Verification 3 is
  fully dispositioned. Verification 4 completes with only allowed 2a failures.
  Verification 7 and 8 meet their rules.
- [ ] `git diff a8b8ed2 | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
- [ ] The receipt is written with a new fingerprint, and the progress-log
  entry is added.

## Session 255 amendment (runs FIRST; serial, concurrency 1)

The source of truth is the design section "Session 255 resume amendments
(2026-10-04)": "Implementer authority", "D4: Euclid sampling projection
semantics" and "Structural clock (2c) test corrections". The operator
diagnosis is `tmp/song-mode-riela/session254-structural-diagnosis.md`. This
section wins over every earlier amendment of this plan where they conflict.
The base is `c7083fb`.

### Intent and context

- Session 254 implemented TASK-004 to TASK-007. The focused run then failed
  four tests:
  - `split_queries_retain_the_single_query_index_rows`;
  - `nested_fast_rev_weighted_euclid_and_chop_keep_clock_orientation`;
  - `euclid_empty_subject_has_no_false_index_observation`;
  - `euclid_over_slice_obeys_inherited_depth_boundary_without_refunding_debit`.
- The pre-existing test
  `song_clock::tests::unsupported_sampling_is_unknown_and_faulted_frame_restores_sibling`
  also still expects Euclid to be `Unknown`.
- The production code is correct. These are wrong expectations for a
  sampling Euclid, as D4 requires. At a sample boundary,
  `song_clock_projection.rs:102-109` sets the projected whole to the step
  whole, `sample_start` to the step begin and orientation to `At`.
  `assert_unchanged_projection` (`structural_tests.rs:168-185`) assumes the
  Index whole is unchanged. That is true only for the six time-preserving
  operators.
- This session corrects the plan's own tests and swaps one fixture string in
  `tests.rs`, then runs every gate. The `domains.rs` work (TASK-006) passed in
  session 254 and is kept as it is.

### Implementer authority (replaces every earlier stop clause in this plan)

- Diagnose each failing gate and fix it inside this plan's writePaths, then
  rerun the focused gate until it passes. Record every fix in the receipt
  under `fixes[]` with: the test, the exact message, the root cause, the
  edited paths, and the class (`production-defect` or `own-test-corrected`,
  naming the design rule).
- Stop only for:
  1. a fix that needs a path outside the writePaths below. This includes
     `src/pattern/eval/song_clock.rs`, `src/pattern/eval/song_clock/dispatch.rs`
     and `src/pattern/combinators/structure.rs`, which are frozen in this
     session;
  2. a change to an assertion in a test that exists at `37ea3e8`, other than
     the single authorized fixture swap (TASK-011);
  3. relaxing a production depth check or any identity check.
- `structural_tests.rs` did not exist at `37ea3e8`, so every test in it is an
  own test. Correcting it toward D4 is allowed. Every proof obligation stays,
  either as the same assertion or as an equal-or-stronger replacement named in
  the receipt. Deleting a test, or reducing it to `is_ok()`, is weakening and
  is forbidden.

### Owned paths for session 255

```json
{
  "planId": "SONG-STRUCTURAL-CLOCK",
  "planPath": "impl-plans/active/song-mode-structural-clock-hooks.md",
  "dependsOn": ["SONG-ROUTE8"],
  "writePaths": [
    "src/pattern/eval/song_clock/structural_tests.rs",
    "src/pattern/eval/song_clock/tests.rs",
    "src/song/snapshot/occupancy/geometry_tests/domains.rs",
    "src/pattern/combinators/region.rs",
    "src/pattern/combinators/music.rs",
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

- `tests.rs` is owned only for the one string in TASK-011.
- `domains.rs` is owned only to keep its session 254 test passing. A change
  there must stay within the session 254 classes (`chunk-substitution`,
  `euclid-positive-branch`, `rename`).
- `region.rs` and `music.rs` may change only to fix a failing time-preserving
  operator test (Ply, Arp, Chop, Striate, LoopAt, Fit). None is expected.
- Frozen (stop condition 1 if a fix needs them): `song_clock.rs`,
  `song_clock/dispatch.rs`, `combinators/structure.rs`,
  `combinators/input.rs`, every 2a, 2b and wave-3 path, the three unowned
  paths, and `.agents/settings.local.json`.
- Scratch logs go under `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session255/`.

### TASK-008: Euclid-only projection helper

**Status**: Complete (session 255)
**Parallelizable**: No
**Deliverable**: `structural_tests.rs`

- Add the private helper `fn assert_euclid_sampled_projection(work: &SharedIndexWork)`
  next to `assert_unchanged_projection`. Imitate that function's loop. For
  every row in `rows(work)` it asserts:
  1. `row.clock` matches `CanonicalClockProjection::Known(_)`;
  2. `row.clock.sampling_evidence()` (`song_clock.rs:902`, which returns
     `(whole, part, point)` tuples) has `len() == 1`, and its `whole` is
     `Some(step)`;
  3. `row.clock.project_for(&row.owner, row.event.whole.expect(..), row.issuer_sample_start, work)`
     returns `Ok(Some(footprint))`;
  4. `footprint.whole == step`;
  5. `footprint.sample_start == step.begin`;
  6. `footprint.orientation == ClockOrientation::At`.

  After the loop it asserts that the rows are non-empty and that at least one
  footprint has `applicable == true`. Do not assert that every row is
  applicable: a Rev under a point sample is legitimately non-applicable.
- Use the helper in exactly these places:
  - the `"euclid {slice {beat -> p} 2 [cut nil]} 1 2"` entry of
    `seven_structural_operators_preserve_slice_clocks`. Split the Euclid entry
    out of the loop, or branch on `body.starts_with("euclid")`. The other six
    entries keep `assert_unchanged_projection`;
  - `split_queries_retain_the_single_query_index_rows`;
  - `nested_fast_rev_weighted_euclid_and_chop_keep_clock_orientation`, in
    place of its `assert_unchanged_projection` call. Keep its existing extra
    check that some `Known` row has non-empty `sampling_evidence()`;
  - the accepted case of the depth test (TASK-010).
- Do not change `assert_unchanged_projection`.
  `striate_ranking_reuses_retained_work_without_callback_reads` and the
  time-preserving entries still use it.
- Pitfalls:
  - Compare `footprint.sample_start` with the step begin. Never compare it
    with `row.issuer_sample_start`, which is the original issuer START and is
    kept separately.
  - Do not filter out non-applicable rows before the per-row assertions.

### TASK-009: Empty-subject exact rows

**Status**: Complete (session 255)
**Parallelizable**: No (after TASK-008)

- `euclid_empty_subject_has_no_false_index_observation` keeps its program
  `euclid {slice {beat -> nil} 2 [cut nil]} 1 2` and its name.
- Replace `assert!(rows(work).is_empty())` with:
  - `assert_eq!(rows(work).len(), 1)`, with a comment: one pulse of 2 steps
    sampled in cycle 0, and an empty-subject Slice still records its Index
    row;
  - `assert_euclid_sampled_projection(work)`.
- The exact count replaces emptiness as the stronger evidence. The diagnosis
  measured exactly one row (Index whole `0..1/2`, value 0). If the measured
  count differs, recompute it by hand from the Euclid pulse layout. Record
  the derivation in `fixes[]`. Never drop the count assertion.

### TASK-010: Searched inherited-depth boundary

**Status**: Complete (session 255)
**Parallelizable**: No (after TASK-008)

Rewrite
`euclid_over_slice_obeys_inherited_depth_boundary_without_refunding_debit`
(currently `structural_tests.rs:265-291`). Keep the program
`euclid {slice {beat -> p} 2 [cut nil]} 3 8 rotation: 1`, `DepthFixture` and
`SongLimits::default()`.

- **Search.** For `depth` from `limits.max_depth - 1` down to
  `limits.max_depth - 32` (inclusive, at most 32 probes):
  - build a fresh `DepthFixture::new(body)` and call
    `collect(limits, depth)`;
  - if the result is `Err`, assert `code == FailCode::DepthExceeded`. Any
    other code fails the test. Keep the refused fixture and work of the most
    recent probe, because that probe is B + 1 when the next one succeeds;
  - on the first `Ok`, set `B = depth` and stop.

  Assert that B was found, with the message
  `inherited depth boundary found within 32 probes`. Never hard-code 247.
- **At B.**
  - `result` is `Ok`;
  - `assert_euclid_sampled_projection(&work)` passes;
  - the fixture's `evaluator.vm_and_ns().0.song_work().is_none()` holds.
- **At B + 1** (the refused probe just above B):
  - `FailCode::DepthExceeded`;
  - `work.borrow().remaining() < limits.max_nodes`, with the message
    `failed inherited-depth query keeps its collector debit`;
  - the VM `song_work()` is `None`.
  - Do not assert that `observations` is empty. Collector rows are scratch,
    not publication (D4). The measurement at 248 holds one row.
- **At `max_depth`** (extra refusal, fresh fixture):
  - `FailCode::DepthExceeded`;
  - `work.borrow().observations.is_empty()` (measured 0, because the topology
    check at `song_replay.rs:782` fails before any collection);
  - the VM `song_work()` is `None`.
- Write B in the receipt as `depthBoundary`. The scratch measurement was 247,
  but the test must not depend on it.
- Pitfalls:
  - Never change a production depth check (`vm.rs:367`, `eval.rs:370`,
    `song_replay.rs:782`, `song/limits.rs`).
  - Reuse one fixture per probe. A reused evaluator keeps VM state.
  - Do not borrow `work` mutably while asserting.
  - The test must stay fast. About 10 builds is the expected cost.

### TASK-011: Authorized fixture swap in `song_clock/tests.rs`

**Status**: Complete (session 255)
**Parallelizable**: No

- In `unsupported_sampling_is_unknown_and_faulted_frame_restores_sibling`
  (`src/pattern/eval/song_clock/tests.rs:478`), replace exactly the string
  `"euclid {slice {beat -> p} 2 [0]} 2 2"` with
  `"chunk {slice {beat -> p} 2 [0]} 2 {q -> fast q 2}"`.
- Change nothing else in the file. The variable name `unknown`, every
  assertion (non-empty `outer_rows`, all `Unknown`), the faulted-frame half
  and the test name all stay.
- Edit the file by hand. Never run rustfmt in write mode on it or on
  `song_clock.rs`.
- Check: `git diff --numstat c7083fb -- src/pattern/eval/song_clock/tests.rs`
  prints `1	1	src/pattern/eval/song_clock/tests.rs`, and `wc -l` stays 896.
- If Chunk somehow yields an empty or non-`Unknown` row set, that is stop
  condition 2. Report the rows. Do not change the assertions.

### TASK-012: Gates, receipt and progress log

**Status**: Complete (session 257; the export harness collision is fixed and the six-ID rule passes)
**Parallelizable**: No (last)

Run the steps and the verification below.

### Session 255 execution steps (in order)

1. Copy `tmp/song-mode-riela/session249-structural-*` to
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session254/`, and record the
   sha256 values in `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session254/sha256.txt`.
2. Rewrite `tmp/song-mode-riela/session249-structural-intent.json` with:
   - the design and plan sha256 values;
   - the sha256 and full text of `structural_tests.rs` and `tests.rs`.

   Re-read and hash-check each file before every edit. On drift, stop the
   edit and reconcile from the current file. Never restore a stale copy.
3. Reproduce. Run the following and expect exit 100 with four failures in
   `structural_tests` plus the `tests.rs` test:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib --no-fail-fast -E 'test(/song_clock::structural_tests|song_clock::tests::unsupported_sampling/)' > tmp/song-s249/SONG-STRUCTURAL-CLOCK/session255/reproduce.log 2>&1`
4. Do TASK-008, TASK-009, TASK-010 and TASK-011. After each, rerun the
   step-3 command into `.../session255/focused-<task>.log`, and fix any
   failure under the authority rule.
5. Run Session 255 verification commands 1-10.
6. Write the receipt `tmp/song-mode-riela/session249-structural-receipt.json`
   with:
   - `fixes[]`;
   - `depthBoundary`;
   - the `tests.rs` numstat;
   - the frozen-file diff result;
   - the clippy disposition;
   - the full-suite failure list with owners;
   - the cohort (953);
   - the line counts;
   - the unowned-path result;
   - `evidenceFingerprint`, the sha256 of the receipt. It must differ from
     every hash in `tmp/song-s249/SONG-STRUCTURAL-CLOCK/`.
7. Add one progress-log entry to this plan.

### Session 255 tests (input -> expected outcome)

- The Euclid `1 2` entry of `seven_structural_operators...` -> the Euclid
  helper passes. The other six entries -> `assert_unchanged_projection`
  passes.
- `split_queries...` (`2 4`) -> footprints are 1/4-wide step wholes, with
  `sample_start` equal to each step begin, and the helper passes.
- `nested_fast_rev_weighted...` (`3 8`) -> the helper passes. At least one row
  is applicable, and at least one row carries sampling evidence. A Rev row may
  be non-applicable.
- `euclid_empty_subject...` -> `Ok`, exactly one row, and the helper passes.
- Depth test -> B is found. B is `Ok` with the helper passing and the VM
  cleared. B + 1 is `DepthExceeded` with the debit kept and the VM cleared.
  `max_depth` is `DepthExceeded` with no observations and the VM cleared.
- `unsupported_sampling_is_unknown_and_faulted_frame_restores_sibling`
  (Chunk) -> non-empty rows, all `Unknown`. The faulted-frame half is
  unchanged and passing.
- `chunk_slice_clock_stays_unknown`, `striate_ranking...`,
  `euclid_slice_query_charges_exact_and_one_less_work` and the `domains` test
  -> unchanged and passing.

### Session 255 verification (foreground; record the exit status and full log path)

1. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast -E 'test(/song_clock|combinators|geometry_tests::domains/)' > tmp/song-mode-riela/session249-structural-nextest-focused.log 2>&1`
   must exit 0 with a nonzero count.
2. `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-structural-build.log 2>&1`
   must exit 0.
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-structural-clippy.log 2>&1`.
   No diagnostic may name a path in this session's writePaths. Every other
   diagnostic is mapped to `RES-`, `SW-` or a Route8 D-row in the receipt.
   `SW-let_and_return` at `src/song/snapshot/issued.rs:231` is expected.
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-mode-riela/session249-structural-nextest-full.log 2>&1`
   - The run completes, and the `Summary` line shows every test run.
   - The failures are a subset of these five 2a tests:
     `cached_nested_slice_events_resolve_from_issued_transcript`,
     `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`,
     `distinct_equal_handle_invocations_are_all_resolved`,
     `partitioned_nested_issued_queries_equal_the_full_route_set` and
     `discarded_augmented_source_origins_are_resolved`.
   - At least 425 distinct tests pass.
   - Any other failure in this plan's paths is fixed here. Any other failure
     outside them is stop condition 1 and is reported with its owner.
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker >> tmp/song-mode-riela/session249-structural-nextest-focused.log 2>&1`
   must exit 0.
6. `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-structural-wasm.log 2>&1`
   must exit 0.
7. `rustfmt --edition 2021 --check src/pattern/eval/song_clock/structural_tests.rs src/song/snapshot/occupancy/geometry_tests/domains.rs src/pattern/combinators/region.rs src/pattern/combinators/music.rs src/pattern/eval/song_clock.rs > tmp/song-mode-riela/session249-structural-fmt.log 2>&1`.
   It passes when no `Diff in` hunk covers a line changed since `c7083fb`.
   A hunk on unchanged lines (for example in `tests.rs`, which rustfmt
   reaches through `song_clock.rs`) is recorded and not fixed.
8. `wc -l` on the five writePath Rust files and `song_clock.rs`: each is
   below 1000, and `tests.rs` is 896.
9. `git diff --quiet c7083fb -- src/pattern/eval/song_clock.rs src/pattern/eval/song_clock/dispatch.rs src/pattern/combinators/structure.rs src/pattern/combinators/input.rs src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs`
   must exit 0.
10. `git ls-files -co --exclude-standard -- '*.rs' Cargo.toml Cargo.lock | wc -l`
    prints 953.

### Session 255 done criteria (mechanically checkable)

- [x] `grep -n "fn assert_euclid_sampled_projection" src/pattern/eval/song_clock/structural_tests.rs`
  matches, and `grep -c "assert_euclid_sampled_projection" src/pattern/eval/song_clock/structural_tests.rs`
  is at least 6 (the definition plus five uses).
- [x] `grep -n "max_depth - 1" src/pattern/eval/song_clock/structural_tests.rs`
  shows only the search start, and `grep -n "within 32 probes"` matches.
- [x] `grep -n "rows(work).is_empty()" src/pattern/eval/song_clock/structural_tests.rs`
  prints nothing.
- [x] `git diff --numstat c7083fb -- src/pattern/eval/song_clock/tests.rs`
  is `1	1`, and `grep -n "chunk {slice {beat -> p} 2 \[0\]} 2 {q -> fast q 2}" src/pattern/eval/song_clock/tests.rs`
  matches.
- [x] Verification 9 exits 0, which means the production files are
  unchanged.
- [ ] Verification 1, 2, 5, 6 and 10 exit 0 or meet their rule. Verification
  3 has no diagnostic in a 2c path. Verification 4 has the five allowed 2a
  failures plus the unclassified `genuine_source_contributions_reject_a_foreign_transcript`
  failure owned outside this plan; serial acceptance waits for its owner.
  Verification 7 has no hunk on changed lines, and 8 meets the line budget.
- [x] `git diff c7083fb | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
- [x] The receipt has `fixes[]`, `depthBoundary` and a new fingerprint, and
  one progress-log entry is added.

## Session 256 amendment (acceptance rerun only; serial, concurrency 1)

The source of truth is the design section "Session 256 resume amendments
(2026-10-04)" > "2c acceptance: exact allowed failures". This section wins over
every earlier amendment of this plan where they conflict. Every session 255
rule not changed here stays in force: the owned paths, the frozen files, the
implementer authority and the done criteria. The implementation base stays
`c7083fb`. The final tree is `88f5120`.

### Intent and context

- Session 255 completed TASK-008 to TASK-011. Their done criteria are ticked
  above. The receipt records `depthBoundary: 247` and the fingerprint
  `391d811a5189a3a4433c71e4f77eacfc1137c38aa25c5d70cf81cfae4ac98818`.
- TASK-012 blocked on one thing only. The full run
  (`tmp/song-mode-riela/session249-structural-nextest-full.log`: 2793 run,
  2787 passed, 6 failed, 3 skipped) failed
  `song::routing::source::issued::tests::genuine_source_contributions_reject_a_foreign_transcript`,
  which the session 255 allowed list did not name.
- That test fails identically at `1ac457f` and `c7083fb`. Its fixture has no
  Euclid or child-sampling operator. Operator decision 1 adds it to the 2c
  allowed list, and SONG-ISSUED-RESOLUTION fixes it.
- This session reruns the 2c gates once on the final tree. No new
  implementation is expected.

### Non-goals

- No Rust edit. Every 2c Rust writePath must equal `88f5120` at the end of
  this session. The only exception is a fix forced by a gate failure in a 2c
  path, made under the session 255 authority rule and recorded in `fixes[]`.
  None is expected.
- Do not touch `src/song/routing/source/issued.rs` or any other 2a path. The
  sixth failure belongs to SONG-ISSUED-RESOLUTION.

### Allowed full-suite failures (exact; replaces session 255 verification 4)

Only these six test IDs may fail. All are owned by SONG-ISSUED-RESOLUTION.

1. `song::routing::nested::issued::tests::cached_nested_slice_events_resolve_from_issued_transcript`
2. `song::routing::nested::issued::tests::partitioned_nested_issued_queries_equal_the_full_route_set`
3. `song::routing::nested::issued::tests::issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`
4. `song::routing::nested::issued::tests::distinct_equal_handle_invocations_are_all_resolved`
5. `song::routing::nested::issued::tests::discarded_augmented_source_origins_are_resolved`
6. `song::routing::source::issued::tests::genuine_source_contributions_reject_a_foreign_transcript`

- A failure outside this list fails 2c. If it is in a 2c path, fix it under
  the authority rule. Otherwise it is stop condition 1: report it with its
  owner.
- If a listed test passes, that is not an error. Record it in the receipt
  under `allowedFailuresAbsent`.
- Clippy disposition rows stay as declared in session 255. `SW-let_and_return`
  at `src/song/snapshot/issued.rs:231` is owned by SONG-SHARED-WORK, and
  `RES-` rows are owned by SONG-ISSUED-RESOLUTION or wave 3.

### TASK-013: Session 256 gate rerun, receipt and progress log

**Status**: Complete (superseded by session 257 acceptance rerun)
**Parallelizable**: No (only 2c task this session)
**Deliverables**: the eight `tmp/song-mode-riela/session249-structural-*`
files and a progress-log entry in this plan.

Steps, in order:

1. Copy every `tmp/song-mode-riela/session249-structural-*` file to
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session255/`. Write their
   sha256 values to `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session255/sha256.txt`.
   Do not delete, move or overwrite anything in
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session255/`. Scratch logs for this
   session go to `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session256/`.
2. Record `git rev-parse HEAD` (expected `88f5120...`) and
   `git status --porcelain=v1` (expected clean apart from tmp/ and the
   excluded `.agents/settings.local.json`) into
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session256/tree.log`.
3. Run Session 255 verification commands 1, 2, 3, 5, 6, 7, 8, 9 and 10
   unchanged, with the same log paths. Run command 4 unchanged, but judge it by
   the six-ID rule above.
4. Rewrite `tmp/song-mode-riela/session249-structural-receipt.json`. Keep the
   session 255 fields (`fixes[]`, `depthBoundary`, the `tests.rs` numstat,
   the frozen-file diff result). Update or add:
   - `session: 256`;
   - `treeHead`;
   - every gate's exit status and full log path;
   - `fullSuite`: the summary counts, `failures[]` (each with test ID and
     owner `SONG-ISSUED-RESOLUTION`), `allowedFailures` (the six IDs) and
     `allowedFailuresAbsent`;
   - the clippy disposition;
   - the cohort (953);
   - the line counts;
   - the unowned-path result;
   - a new `evidenceFingerprint`. It is the sha256 of the receipt computed
     with this field empty. It must differ from `391d811a...` and from every
     hash in `tmp/song-s249/SONG-STRUCTURAL-CLOCK/`.
5. Set TASK-012 and TASK-013 to Complete only if every check below passes.
   Add one progress-log entry giving the commands, exit codes and log paths.

Pitfalls:

- Do not "fix" the six failures here. They are 2a-owned.
- Do not run `cargo fmt` or `rustfmt` without `--check`.
- Full nextest takes about 13 minutes. Run it in the foreground, poll until
  it exits, and treat a truncated log (no `Summary` line) as a failure.

### Session 256 verification (foreground; record the exit status and full log path)

- V1 (focused): Session 255 command 1 exits 0 with a nonzero count.
- V2 (build): command 2 exits 0.
- V3 (clippy): command 3 shows no diagnostic in a 2c path, and every
  diagnostic is mapped.
- V4 (full): command 4 (`cargo nextest run --no-fail-fast` into
  `tmp/song-mode-riela/session249-structural-nextest-full.log`) completes with
  a `Summary` line. Check it with
  `grep -E '^\s+FAIL' tmp/song-mode-riela/session249-structural-nextest-full.log | awk '{print $NF}' | sort -u`,
  which must print a subset of the six IDs and nothing else. At least 425
  distinct tests pass.
- V5 (legacy binaries): command 5 exits 0.
- V6 (WASM): command 6 exits 0.
- V7 (fmt): command 7 shows no `Diff in` hunk on a line changed since
  `c7083fb`.
- V8 (lines): command 8 shows every file below 1000 and `tests.rs` at 896.
- V9 (frozen and unowned): command 9 exits 0.
- V10 (cohort): command 10 prints 953.
- V11 (no Rust change this session):
  `git diff --quiet 88f5120 -- src/pattern/eval/song_clock/structural_tests.rs src/pattern/eval/song_clock/tests.rs src/song/snapshot/occupancy/geometry_tests/domains.rs src/pattern/combinators/region.rs src/pattern/combinators/music.rs`
  exits 0, unless `fixes[]` records a session 256 fix.

### Session 256 done criteria (mechanically checkable)

- [x] V1, V2, V5, V6, V9 and V11 exit 0, and V10 prints 953.
- [x] Session 256's out-of-policy export failure was diagnosed and fixed by
  TASK-014; session 257 V4 now has only the six allowed IDs and a `Summary`.
- [x] V3 has no 2c-path diagnostic, V7 has no hunk on changed lines, and V8
  meets the line budget.
- [x] `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session255/sha256.txt` exists,
  and `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session255/` is unchanged.
- [x] The receipt has `session: 256`, `allowedFailures` with six IDs, and a
  fingerprint different from `391d811a...`.
- [x] One progress-log entry is added. Test-integrity, adversarial and
  integration review follow; they are owned by the workflow, not by this
  task.

## Session 257 amendment (harness fix plus acceptance rerun; serial, concurrency 1)

The source of truth is the design section "Session 257 resume amendments
(2026-10-04)" > "2c: temp-directory collision fix (operator authorization)" and
"2c acceptance rerun". This section wins over every earlier amendment of this
plan where they conflict. Every session 255 and 256 rule not changed here stays
in force: the frozen files, the implementer authority, the six-ID allowed list
and the done criteria. The Rust tree base is `259f0b9`, whose Rust sources
equal `88f5120`. The run starts on the commit that records this amendment
(`<s257-plan-commit>`); that commit changes documentation only.

### Intent and context

- Session 256 reran the 2c gates with no Rust edits. Full nextest
  (`tmp/song-mode-riela/session249-structural-nextest-full.log`) reported 2793
  run, 2786 passed, 7 failed and 3 skipped. Six failures are the allowed 2a
  IDs. The seventh was
  `song_export fractional_song_and_tail_write_exact_partial_block`, which
  panicked at `tests/song_export.rs:20` with
  `Os { code: 17, kind: AlreadyExists }`.
- Root cause: `Directory::new` in `tests/song_export.rs` (lines 10-22) names
  its directory `vactr-song-export-{stamp}-{NEXT}`. `stamp` is
  `SystemTime` nanoseconds, and `NEXT` is a static `AtomicU64`. nextest runs
  each test in its own process, so `NEXT` is 0 in every process. macOS clocks
  are microsecond-granular, so two tests starting in the same microsecond
  build the same name. `std::fs::create_dir` then fails for the second test.
- `tests/song_cli.rs` `Directory::new` (lines 11-21) has the identical pattern
  (`vactr-song-cli-{stamp}-{NEXT}`). It has not failed yet, but it would block
  the zero-failure gates of 2a, 2b, wave 3 and SONG-16.
- `tests/cli.rs` (line 23), `tests/song_assets.rs` (line 436) and
  `tests/song_candidate.rs` (line 372) already include `std::process::id()`.
  They are not touched.
- Operator authorization adds `tests/song_export.rs` and `tests/song_cli.rs`
  to this plan's writePaths for this fix only.

### Non-goals

- No change to any 2c Rust writePath from earlier sessions. The paths are
  `structural_tests.rs`, `song_clock/tests.rs`, `geometry_tests/domains.rs`,
  `combinators/region.rs` and `combinators/music.rs`. They stay equal to
  `88f5120`. The only exception is a fix forced by a gate failure in one of
  those paths, made under the session 255 authority rule and recorded in
  `fixes[]`. None is expected.
- No change to any test body, assertion, test name, `Drop` impl, import or
  other helper in `tests/song_export.rs` or `tests/song_cli.rs`.
- Do not replace `std::fs::create_dir` with `create_dir_all`. A real collision
  must still fail loudly.
- Do not touch `tests/cli.rs`, `tests/song_assets.rs`,
  `tests/song_candidate.rs` or any 2a path, including
  `src/song/routing/source/issued.rs`.
- Do not "fix" the six allowed 2a failures here.
- Do not run `cargo fmt` or `rustfmt` in write mode on any file.

### TASK-014: Temp-directory name gains the process id

**Status**: Complete (authorized `Directory::new` process-ID fix)
**Parallelizable**: No (it runs before TASK-015 in the same serial sub-wave)
**Deliverables**: one edited `format!` call in `Directory::new` of
`tests/song_export.rs` and of `tests/song_cli.rs`.

- Before each edit, re-read the file, record its sha256 in
  `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session257/pre-edit.sha256`, and check
  it against `git show 259f0b9:<path> | shasum -a 256`. On a mismatch
  (drift), stop and report it. Do not merge the drift.
- In each file, change only the `format!` call inside `Directory::new`
  (`tests/song_export.rs` lines 16-19, `tests/song_cli.rs` lines 16-19):
  - The literal gains a leading `{}` for the process id:
    `"vactr-song-export-{}-{stamp}-{}"` and
    `"vactr-song-cli-{}-{stamp}-{}"`.
  - Insert one argument line, `std::process::id(),`, before the existing
    `NEXT.fetch_add(...)` argument, with the same 12-space indentation.
  - The resulting name shape is `vactr-song-export-{pid}-{stamp}-{n}`.
- Edit by hand. The expected per-file numstat is `2	1` (2 added lines,
  1 deleted line).
- Record the post-edit sha256 values in
  `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session257/post-edit.sha256`.
- In the receipt, add one `fixes[]` entry per file with:
  - `test`: for song_export,
    `song_export fractional_song_and_tail_write_exact_partial_block`; for
    song_cli, `latent: same pattern`;
  - `message`: `AlreadyExists` at `tests/song_export.rs:20`;
  - `rootCause`: per-process `NEXT` restart plus microsecond clock;
  - `paths`: the edited file;
  - `class`: `harness-collision`;
  - `designRule`: `Session 257 > 2c: temp-directory collision fix`.
- Pitfalls:
  - Do not use `{pid}` as an inline capture unless a `let pid` binding is
    added. The pinned form is a positional argument, which keeps the diff at
    `2	1`.
  - Any further edit to either file is stop condition 1.

Tests (input -> expected outcome):

- `cargo nextest run --test song_export --test song_cli`, with each test in
  its own process -> every test passes, and no `AlreadyExists` appears.
- The same command run 5 times in a row -> every run exits 0.
- Full nextest -> no `song_export` or `song_cli` failure.

### TASK-015: Session 257 gate rerun, receipt and progress log

**Status**: Complete (session 257 acceptance gates, receipt and progress log recorded)
**Parallelizable**: No (depends on TASK-014)
**Deliverables**: the eight `tmp/song-mode-riela/session249-structural-*`
files, the session 257 scratch logs and one progress-log entry in this plan.

Steps, in order:

1. Copy every `tmp/song-mode-riela/session249-structural-*` file to
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session256/`. Write their
   sha256 values to `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session256/sha256.txt`.
   Do this before TASK-014. Never delete, move or overwrite anything in
   `attempt-session255/`, `session255/` or `session256/`. Scratch logs for
   this session go to `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session257/`.
2. Record `git rev-parse HEAD` and `git status --porcelain=v1` in
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session257/tree.log`. The expected
   status is the two test files only, apart from `tmp/` and the excluded
   `.agents/settings.local.json`.
3. Run session 255 verification commands 1-10 unchanged, with the same log
   paths. Judge command 4 by the six-ID rule of the session 256 amendment.
   Then run S257-V1 to S257-V6 below.
4. Rewrite `tmp/song-mode-riela/session249-structural-receipt.json`. Keep the
   session 255 and 256 fields. Update or add:
   - `session: 257`;
   - `treeHead`;
   - every gate's exit status and full log path;
   - `fixes[]` with the two TASK-014 entries;
   - `harnessDiff`: the `git diff --numstat 259f0b9` output for the two files;
   - `fullSuite`: the counts, `failures[]` with owners, `allowedFailures`
     (six IDs) and `allowedFailuresAbsent`;
   - the clippy disposition, the cohort (953), the line counts and the
     unowned-path result;
   - a new `evidenceFingerprint`. It is the sha256 of the receipt computed
     with this field empty. It must differ from `391d811a...` and from every
     hash under `tmp/song-s249/SONG-STRUCTURAL-CLOCK/`.
5. Set TASK-012, TASK-013, TASK-014 and TASK-015 to Complete only if every
   check below passes. Add one progress-log entry with the commands, exit
   codes and log paths. Test-integrity, adversarial and integration review
   follow. The workflow owns those reviews, not this task.

Pitfalls:

- Full nextest takes about 13 minutes. Run it in the foreground and poll it
  until it exits. A log with no `Summary` line counts as a failure.
- A failure outside the six IDs in a 2c path is fixed under the authority
  rule. Any other such failure is stop condition 1: report it with its owner.
  Do not edit tests to make it pass.

### Session 257 verification (foreground; record the exit status and full log path)

Session 255 commands 1-10 and session 256 V1-V11 apply unchanged. V4 must
print a subset of the six IDs, and specifically no `song_export` or
`song_cli` test. These checks are added:

- S257-V1 (harness binaries):
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast --test song_export --test song_cli >> tmp/song-mode-riela/session249-structural-nextest-focused.log 2>&1`
  exits 0 with a nonzero count.
- S257-V2 (repeat): run the S257-V1 command 5 times in the foreground, sending
  output to `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session257/harness-repeat.log`.
  All 5 runs exit 0.
- S257-V3 (fmt):
  `rustfmt --edition 2021 --check tests/song_export.rs tests/song_cli.rs >> tmp/song-mode-riela/session249-structural-fmt.log 2>&1`.
  No `Diff in` hunk may cover a line changed since `259f0b9`.
- S257-V4 (diff shape):
  - `git diff --numstat 259f0b9 -- tests/song_export.rs tests/song_cli.rs`
    prints `2	1` for each file.
  - `git diff -U0 259f0b9 -- tests/song_export.rs tests/song_cli.rs` shows
    hunks only within lines 16-20 of each file.
  - `grep -c 'std::process::id()' tests/song_export.rs tests/song_cli.rs`
    prints 1 for each file.
  - `grep -c 'create_dir_all' tests/song_export.rs tests/song_cli.rs` prints
    0 for each file.
- S257-V5 (assertions unchanged): for each of the two files,
  `git show 259f0b9:<path> | grep -c assert` equals `grep -c assert <path>`.
- S257-V6 (lines): `wc -l tests/song_export.rs tests/song_cli.rs` prints 197
  and 160.

### Session 257 done criteria (mechanically checkable)

- [x] S257-V1, S257-V2 and S257-V5 pass, and S257-V4 and S257-V6 print the
  expected values.
- [x] Session 256 V1, V2, V5, V6, V9 and V11 exit 0, and V10 prints 953.
- [x] The V4 failure set is a subset of the six IDs, with no `song_export` or
  `song_cli` test, and the log has a `Summary` line.
- [x] V3 has no 2c-path diagnostic. V7 and S257-V3 have no hunk on changed
  lines.
- [x] `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session256/sha256.txt`
  exists, and `attempt-session255/`, `session255/` and `session256/` are
  unchanged.
- [x] The receipt has `session: 257`, the two `harness-collision` fixes, and a
  fingerprint different from every earlier one.
- [x] One progress-log entry is added.

## Session 258 amendment (format fix plus acceptance rerun; runs SECOND; serial, concurrency 1)

The source of truth is the design section "Session 258 resume amendments
(2026-10-04)": "Serial order and dependency edge" and "2c: formatting fix and
review on the green tree". This section wins over every earlier amendment of
this plan where they conflict. The session 255 to 257 rules not changed here
stay in force: the frozen files, the implementer authority, the owned paths
and TASK-014.

### Intent and context

- The 2c implementation is complete at `e71d726`. Session 257 V4 had exactly
  the six 2a-owned failures. 2c was not accepted, because the workflow
  progress gate rejects every non-zero full-suite exit.
- The new order runs SONG-ISSUED-RESOLUTION first. This plan's `dependsOn`
  becomes `["SONG-ROUTE8", "SONG-ISSUED-RESOLUTION"]`, so 2c cannot start
  while 2a is running. 2c starts from the commit that records 2a acceptance,
  written `<2a-accepted>`. On that tree the full suite must have zero
  failures.
- `tmp/song-mode-riela/session249-structural-fmt.log` shows two `rustfmt
  --check` hunks in this plan's own writePath
  `src/pattern/eval/song_clock/structural_tests.rs`:
  - **Hunk A (about line 33).** The four-line
    `DecodedSongAssetFactory::new(` call with three `BTreeMap::new(),`
    argument lines becomes rustfmt's two-line form: `let factory =` on one
    line, then the call with its three arguments on the next, indented one
    more level.
  - **Hunk B (about line 98).** The
    `use crate::pattern::eval::song_observation::{CanonicalIndexCollector, CanonicalIndexTarget};`
    line is wrapped over three lines (open brace, the two names on one
    indented line with a trailing comma, closing `};`). The
    `use crate::vm::query_vm::MeteredSongQuery;` line moves after
    `use crate::value::intern::intern_kw;`.
- `structural_tests.rs` is declared at `src/pattern/eval/song_clock.rs:892`
  and has no child modules.
- **Reviews.** The test-integrity, adversarial and integration reviews cover
  `git diff 1ac457f <2c-accepted> -- <2c writePaths>`, which is all of the 2c
  work since SONG-ROUTE8 acceptance, not only the format hunks.

### Non-goals

- No token change in `structural_tests.rs` other than whitespace, line breaks
  and the position of one `use` line. No test, assertion, fixture string or
  name changes.
- No edit to any other 2c path (`song_clock/tests.rs`, `domains.rs`,
  `region.rs`, `music.rs`, `tests/song_export.rs`, `tests/song_cli.rs`).
  These must equal `<2a-accepted>`, unless a gate failure in one of them
  forces a fix under the session 255 authority, recorded in `fixes[]`. None is
  expected.
- No edit to any 2a, 2b, wave-3 or unowned path.
- Do not run `rustfmt` or `cargo fmt` in write mode on any file, including
  `structural_tests.rs`. Edit by hand.
- No `--retries`. Do not rerun the full suite until it happens to pass.
- The six-ID allowed list from sessions 256 and 257 is withdrawn. No failure
  is allowed.

### TASK-016: Format-only fix, gate rerun, receipt and progress log

**Status**: Not Started
**Parallelizable**: No (only 2c task this session; it starts after
SONG-ISSUED-RESOLUTION is accepted)
**Deliverables**: two hand-edited hunks in
`src/pattern/eval/song_clock/structural_tests.rs`, the eight
`tmp/song-mode-riela/session249-structural-*` files, the session 258 scratch
logs and one progress-log entry in this plan.

Steps, in order:

1. Copy every `tmp/song-mode-riela/session249-structural-*` file to
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session257/` and write their
   sha256 values to `sha256.txt` there. Never delete, move or overwrite
   `attempt-session253/` to `attempt-session256/` or `session254/` to
   `session257/`. Scratch logs go to
   `tmp/song-s249/SONG-STRUCTURAL-CLOCK/session258/`.
2. Record `git rev-parse HEAD` (`<2a-accepted>`) and
   `git status --porcelain=v1` in `.../session258/tree.log`. The status must
   be clean apart from `tmp/` and the excluded `.agents/settings.local.json`.
3. Pre-edit check:
   - record the sha256 of `structural_tests.rs` in
     `.../session258/pre-edit.sha256`;
   - confirm `git diff --quiet e71d726 -- src/pattern/eval/song_clock/structural_tests.rs`
     exits 0 (2a did not touch it). On drift, stop and report it.
   - Run `rustfmt --edition 2021 --check src/pattern/eval/song_clock/structural_tests.rs > tmp/song-s249/SONG-STRUCTURAL-CLOCK/session258/fmt-before.log 2>&1`.
     It must exit 1 and show exactly hunks A and B.
4. Hand-edit hunks A and B to match the `+` lines in `fmt-before.log`
   exactly, including indentation. Record the post-edit sha256 in
   `.../session258/post-edit.sha256`.
5. Run `rustfmt --edition 2021 --check src/pattern/eval/song_clock/structural_tests.rs > tmp/song-s249/SONG-STRUCTURAL-CLOCK/session258/fmt-after.log 2>&1`.
   It must exit 0 with an empty log. If a hunk remains, adjust only
   whitespace until it is clean.
6. Run session 255 verification commands 1 to 10, with the same log paths and
   the changes below, then S257-V1, S257-V3 and S257-V5, then S258-V1 to
   S258-V5.
7. Rewrite `tmp/song-mode-riela/session249-structural-receipt.json`. Keep the
   session 255 to 257 fields. Update or add:
   - `session: 258`, `treeHead` and `baseCommit` (`<2a-accepted>`);
   - every gate's exit status and full log path;
   - a `fixes[]` entry for TASK-016 with class `format-only`, the two hunks,
     path `src/pattern/eval/song_clock/structural_tests.rs` and design rule
     `Session 258 > 2c: formatting fix and review on the green tree`;
   - `formatDiff`: the `git diff --numstat <2a-accepted>` output for
     `structural_tests.rs`;
   - `fullSuite` with the counts and `failures: []`. Remove
     `allowedFailures` and `allowedFailuresAbsent`, or set them to empty with
     the note `withdrawn in session 258`;
   - `reviewDiffRange`: `git diff 1ac457f <final tree> -- <2c writePaths>`;
   - the clippy disposition, the cohort (953), the line counts and the
     unowned-path result;
   - a new `evidenceFingerprint`: the sha256 of the receipt with this field
     empty. It must differ from every hash under
     `tmp/song-s249/SONG-STRUCTURAL-CLOCK/` and from `391d811a...`.
8. Set TASK-016 to Complete only if every check below passes. Add one
   progress-log entry with the commands, exit codes and log paths.

Pitfalls:

- Never use `rustfmt` without `--check`. `structural_tests.rs` is a child of
  `song_clock.rs`, and a write-mode run on `song_clock.rs` would also rewrite
  `tests.rs`.
- Do not reorder `use crate::pattern::eval::InputCells;`. Only
  `MeteredSongQuery` moves.
- A full-suite failure in a 2c path is fixed under the authority rule. Any
  other failure is stop condition 1, reported with its owner. Do not edit
  tests to make it pass.

### Session 258 changes to session 255 verification

- Command 3 (clippy): no diagnostic names a 2c path. Every other diagnostic is
  mapped to a declared disposition row. `SW-let_and_return` at
  `src/song/snapshot/issued.rs:231` is still expected, and 2b owns it.
- Command 4 (full, `--no-fail-fast`): exit 0 with a `Summary` line, at least
  2793 tests run and 0 failed.
  `grep -cE '^\s+FAIL' tmp/song-mode-riela/session249-structural-nextest-full.log`
  prints 0. Run it in the foreground and poll it until it exits.
- Command 7 (fmt): still judged by "no hunk on a changed line" for the other
  paths. `structural_tests.rs` must have no hunk at all. A pre-existing
  `tests.rs` hunk on unchanged lines is recorded, not fixed.
- Command 9: the base stays `c7083fb` (those frozen files are unchanged
  since then).
- Session 256 V11 (`git diff --quiet 88f5120 -- ...` on the five earlier 2c
  Rust writePaths) is withdrawn for session 258, because TASK-016 edits
  `structural_tests.rs`. S258-V2 (format-only diff of `structural_tests.rs`)
  and S258-V3 (the other 2c paths equal `<2a-accepted>`) replace it.

### Session 258 verification additions (foreground; record the exit status and full log path)

- S258-V1 (structural_tests format):
  `rustfmt --edition 2021 --check src/pattern/eval/song_clock/structural_tests.rs >> tmp/song-mode-riela/session249-structural-fmt.log 2>&1`
  exits 0 and adds nothing to the log.
- S258-V2 (format-only diff):
  - `git diff --numstat <2a-accepted> -- src/pattern/eval/song_clock/structural_tests.rs`
    prints at most 7 added and 7 deleted lines (expected 6 added and 7
    deleted: hunk A is 2 for 5, hunk B is 3 for 1, and the moved `use` is 1
    for 1);
  - `git diff -w --word-diff=porcelain <2a-accepted> -- src/pattern/eval/song_clock/structural_tests.rs`
    shows only the `MeteredSongQuery` line moving and the brace or line-break
    changes;
  - `git show <2a-accepted>:src/pattern/eval/song_clock/structural_tests.rs | grep -c assert`
    equals `grep -c assert src/pattern/eval/song_clock/structural_tests.rs`.
- S258-V3 (other 2c paths unchanged):
  `git diff --quiet <2a-accepted> -- src/pattern/eval/song_clock/tests.rs src/song/snapshot/occupancy/geometry_tests/domains.rs src/pattern/combinators/region.rs src/pattern/combinators/music.rs tests/song_export.rs tests/song_cli.rs`
  exits 0, unless `fixes[]` records a forced fix.
- S258-V4 (unowned and other plans unchanged):
  `git diff --quiet <2a-accepted> -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs src/song/routing src/song/snapshot/occupancy/lookup src/song/snapshot/issued.rs src/pattern/eval/song_replay.rs`
  exits 0.
- S258-V5 (lines): `wc -l src/pattern/eval/song_clock/structural_tests.rs`
  prints a value below 1000 (353 before the edit, about 352 after).

### Session 258 done criteria (mechanically checkable)

- [ ] `fmt-before.log` shows hunks A and B, and `fmt-after.log` is empty with
  exit 0. S258-V1 exits 0.
- [ ] S258-V2 to S258-V5 pass.
- [ ] Session 255 commands 1, 2, 5, 6, 9 and 10 exit 0 (10 prints 953).
  Command 4 exits 0 with a `Summary` line and 0 failures. Command 3 has no
  2c-path diagnostic, and command 7 meets its rule.
- [ ] S257-V1 exits 0, and S257-V5 assert counts are unchanged.
- [ ] `tmp/song-s249/SONG-STRUCTURAL-CLOCK/attempt-session257/sha256.txt`
  exists. The receipt has `session: 258`, the TASK-016 `format-only` fix,
  `reviewDiffRange`, `failures: []` and a new fingerprint.
- [ ] One progress-log entry is added. Test-integrity, adversarial and
  integration review over `reviewDiffRange` follow; the workflow owns them.
