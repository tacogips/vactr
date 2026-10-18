# LPI-NS-INDEX: review and prove the session-349 name-space lookup index

**Status**: In Progress (implemented at b327141; review-and-repair rerun pending)
**Plan ID**: LPI-NS-INDEX (wave 1)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 6 (I6, name-space index part), 2 (I9) and 11 (R1-R6)
**Manifest**: `impl-plans/active/live-perf-int-dispatch.json`
**Branch**: `merge/live-perf` (shared working directory; no private branch)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Intent and Context

Session 349 (commit 4ca8edc) added lookup indexes to speed up evaluation.
The full vitest `test/e2e/large-eval.test.ts` scaling check failed at
ratio 7.59-7.83 before them and now passes at 2.61, so they stay. They
were never reviewed. The indexes are:

- `src/ns/depgraph.rs`: `DepGraph::current_by_span`, `claim`,
  `add_current_span`, `remove_current_span`, `current_gen_at_span`.
- `src/ns/tweak.rs`: `TweakTable::by_generation`, `by_span_start`,
  `insert`, `sites_of`, `migration_candidates`,
  `migration_generation_in_span`, `remove_indexes`, `retire`.
- `src/ns/evaluator.rs`: the `migrate_from` lookup in form evaluation
  (around line 395) and `Evaluator::migrate` (around line 882).

The change is meant to be a pure performance change, so every result must
equal the linear lookups it replaced. Show the old code with
`git show 4ca8edc^:src/ns/evaluator.rs`, `.../depgraph.rs` and
`.../tweak.rs`.

**Known divergence candidate.** The old `migrate_from` scanned
`graph.ids()` for the first form with `is_current(id) && node.span ==
form.span`. `is_current` is vacuously true for a form with an empty write
set, even when `claim` never ran for it. The new index registers forms only
inside `claim`. An unclaimed empty-write form could therefore be found by
the old scan and missed by the new one.

## Non-goals

- No new optimization, and no change to the scaling thresholds in
  `src/ns/tests/migrate_scale.rs` or `editor/test/e2e/large-eval*.ts`.
- No behavior change except closing a proven divergence.
- Do not remove the indexes.

## Dependencies

- **dependsOn**: none (wave 1). Runs alongside LPI-WIRE and LPI-VIM-STOP.
  It shares `target/` with LPI-WIRE. On a compile error in a file outside
  writePaths:
  - wait 2 minutes and retry, at most 3 times;
  - then record it as drift and stop;
  - never edit that file.

## writePaths

- `src/ns/depgraph.rs`
- `src/ns/tweak.rs`
- `src/ns/evaluator.rs`
- `src/ns/tests/mod.rs`
- `src/ns/tests/migrate_index_equivalence.rs` (new)
- `impl-plans/active/live-perf-int-ns-index.md` (this plan; Progress Log
  only)
- Artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules`, `tmp/live-perf-int/ns-index`

## sharedPaths

None. Read-only: `src/ns/tests/migrate_scale.rs` (imitate its `Harness`
use), `src/ns/tests/reactive_basic.rs` (`Harness`), `src/ns/journal.rs`
(the second `claim` caller, line about 644).

## Tasks

### TASK-N1: Review and settle the divergence

1. Read the old and new lookups side by side. Write a short equivalence
   argument in the Progress Log for each of:
   - `migrate`: the first matching predecessor by position, for binding
     id or name, and by `(origin, index, ty)`;
   - `sites_of`: ordering by `(index, id)`, which equals a stable sort by
     index over id-ordered values;
   - `insert` and `retire` index consistency;
   - `current_gen_at_span`.
2. Settle the empty-write-set candidate.
   - Check whether `Evaluator` ever calls the migration lookup when an
     earlier form with the same span and an empty write set exists that
     `claim` never registered. Look at all `DepGraph::add` callers and
     both `claim` callers.
   - **If it is reachable:** register a form in `current_by_span` at
     `DepGraph::add` when its write set is empty, since it is then
     vacuously current. Keep the existing `claim` maintenance.
   - **If it is unreachable:** record the proof in the Progress Log and
     change no product code.
3. Also check that no other path changes ownership. Ownership changes only
   through `claim`'s `name_owner.insert` and `bind_owner.insert`; a later
   removal would leave the index stale. Record the result.

### TASK-N2: Differential test (`src/ns/tests/migrate_index_equivalence.rs`, registered in `src/ns/tests/mod.rs`)

- Add a test-local reference function. It reimplements the old linear
  `current_gen_at_span` scan over `h.ev.graph()`, using public
  `DepGraph::ids`, `get`, `is_current` and `node.span`.
- Drive `Harness` (imitate `src/ns/tests/migrate_scale.rs`) through
  evaluation sequences. After each step, compare
  `graph().current_gen_at_span(span)` with the reference, for the span of
  every form in the graph.

**Test cases (situation -> expected outcome):**

- Independent `let valueN 0.5` forms, then re-evaluating one of them ->
  index equals reference for every span.
- A redefinition that supersedes a name (`let a 1` then `let a 2` at a
  different span) -> the superseded form is not current in either.
- A form with an empty write set (for example a bare expression such as
  `(+ 1 2)`, or whatever the reader accepts as a top-level expression with
  no definition or binding), evaluated twice at the same span ->
  identical results.
- A failed evaluation followed by a successful one at the same span ->
  identical results.
- `d1 {late-probe {* 0.5 2}}` with `set_tweak`, then re-evaluated (the
  existing `explicit_same_span_reevaluation_inherits_prior_tweak_override`
  shape) -> `take_migrations()` contains the same `(old, new)` pair the
  old `migrate` would produce. Assert the concrete ids, as the existing
  test does.

Keep the test file under 300 lines. Do not edit `migrate_scale.rs`.

## Invariants

- The existing `src/ns/tests/migrate_scale.rs` tests pass unchanged.
- `src/ns/evaluator.rs` stays under 1000 lines (it is 957 now; prefer
  putting new logic in `depgraph.rs`).
- The `#[cfg(test)]` `migration_lookup_work` instrumentation does not
  change production results.

## Verification (repository root; logs under `tmp/live-perf-int/ns-index/`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (`1-fmt.log`).
2. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` ->
   exit 0 (`2-clippy.log`).
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib ns::`
   -> exit 0, testsRun > 0, failures 0 (`3-nextest-ns.log`). It includes
   `migrate_index_equivalence` and `migrate_scale`.
4. `wc -l src/ns/depgraph.rs src/ns/tweak.rs src/ns/evaluator.rs src/ns/tests/migrate_index_equivalence.rs`
   -> all under 1000 (`4-line-counts.log`).
5. **Mutation evidence** (only as `mutationEvidence`). Temporarily make
   `current_gen_at_span` return the *last* form in the set instead of the
   first. Show that a differential case fails. Then restore it.

## Completion Criteria

- [x] The Progress Log holds the equivalence argument for the four lookups
  and the reachability verdict for the empty-write-set candidate.
- [x] The divergence is closed in `depgraph.rs` if reachable; otherwise no
  product change is made.
- [x] The differential test is added and registered, and passes.
- [x] Verification 1-4 exit 0. Mutation evidence is recorded separately.
- [x] The Progress Log records pre/post sha256 for every edited file.

## Rerun (workflow session 438): verify and review the committed work

TASK-N1 and TASK-N2 are implemented in commit b327141. The work is the
differential test plus a no-product-change verdict. This rerun does
**not** re-implement anything. All logs go to
`tmp/live-perf-int/ns-index/rerun-438/`.

### TASK-NR1: Deliverable check (no edit; `rerun-438/r1-deliverables.log`)

- `grep -n 'mod migrate_index_equivalence' src/ns/tests/mod.rs` [1 match]
- `test -f src/ns/tests/migrate_index_equivalence.rs` [exit 0]
- `git diff 9474491 HEAD -- src/ns/depgraph.rs src/ns/tweak.rs src/ns/evaluator.rs`
  [empty: no product change, matching the recorded verdict]
- `grep -n '!members.is_empty()' src/ns/evaluator.rs` [the guard that
  makes the empty-write-set candidate unreachable; recorded line around
  475]
- `git diff 9474491 HEAD -- src/ns/tests/migrate_scale.rs` [empty]

### TASK-NR2: Verification rerun

Run verification steps 1-4 of this plan again, unchanged, with logs
`rerun-438/1-fmt.log` to `rerun-438/4-line-counts.log`. Step 3 must
include `migrate_index_equivalence` and `migrate_scale` with 0 failures.

### TASK-NR3: Mutation evidence (only in `mutationEvidence`)

Record the sha256 before and after each mutation; the restored hash must
match the original.

- M1: `current_gen_at_span` uses `.next_back()` instead of `.next()` ->
  the differential test fails (`rerun-438/m1.log`).
- M2: in `DepGraph::claim`, temporarily skip the `remove_current_span`
  call for superseded forms -> the supersession differential case fails
  (`rerun-438/m2.log`). If it does not fail, record that as a coverage gap
  for the reviewer. Do not add a test unless the review asks for one.
- Afterwards, rerun NR2 step 3 green (`rerun-438/m-restore-3.log`).

When NR1-NR3 are done, set Status to
`Implemented (b327141); awaiting independent review`.

### TASK-NR4: Independent review (performed by the workflow review step; read-only)

- The Progress Log has an equivalence argument for each of the four
  lookups, and it matches the code. Compare with
  `git show 4ca8edc^:src/ns/{depgraph,tweak,evaluator}.rs`.
- The empty-write-set verdict holds: `eval_form_checked` adds a form only
  when `!members.is_empty()`, and the reactive-commit path that empties a
  write set calls `claim`.
- Ownership changes only through `claim`.
- The `#[cfg(test)]` `migration_lookup_work` instrumentation has no
  production effect.
- `evaluator.rs` is at most 957 lines.
- The differential test compares against an independent linear reference.
  It does not compare the index with itself.

### TASK-NR5: Repair only findings

Fix each high or mid finding inside writePaths. A behavior defect gets a
failing-first test. List each fix in `addressedFeedback` with status
`repaired`, then rerun NR2. If a finding needs a file outside writePaths,
stop and report a scope amendment.

### TASK-NR6: Acceptance

After the review step accepts, add a Progress Log entry with the reviewer
step id and verdict. Then set Status to `Completed`.

### Rerun Completion Criteria

- [ ] NR1 log shows every expected result.
- [ ] NR2 steps 1-4 exit 0 with positive test counts.
- [ ] NR3 M1 fails as expected (M2 fails, or is recorded as a coverage
  gap); restore hashes match.
- [ ] Review verdict recorded; every high or mid finding repaired.
- [ ] Status is Completed only after the recorded acceptance.

## Execution Rules (all plans in this manifest)

- Read each file fresh immediately before editing it. Record
  `shasum -a 256 <file>` before and after the edit in the Progress Log.
- Edit only writePaths. Report drift in any other file; never repair it.
- Run commands in the foreground only. Write each to its log with
  `cmd > log 2>&1; echo "exit=$?" >> log`.
- Do not commit, stash, branch or run any mutating git command. Read-only
  `git diff`, `git status` and `git show` are allowed.
- Use English only, with no emojis. Keep every touched source file under
  1000 lines.
- Verification record shape is exactly `{command, exitStatus: 0, testsRun,
  testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
  - Negative-control and mutation runs go only in `mutationEvidence`.
  - Repaired review findings go only in `addressedFeedback`, with status
    `repaired`.
- Edit only this plan's own Progress Log.

## Progress Log

### Session: 2026-10-18 (plan authored, workflow session 433 step 4)
**Tasks Completed**: none. Plan authored from the accepted design.
**Notes**: Ready for wave 1.

### Session: 2026-10-18 (workflow session 433, step 6, LPI-NS-INDEX)
**Tasks Completed**: TASK-N1 and TASK-N2.
**Notes**:

- Equivalence review:
  - `current_gen_at_span`: the old `ids()` scan selects the lowest current
    `FormId` at an exact span; `current_by_span` is a `BTreeSet<FormId>` and
    `.next()` selects that same form. `claim` removes superseded forms and
    adds each current form.
  - `migrate`: old `sites_of` ordering is index order with `TweakId` order
    preserved for ties. The indexed maps record the first matching position;
    binding matches choose the earlier of slot-id and name matches, while
    non-binding matches use `(origin, index, type)`, preserving the old
    `.find` result.
  - `sites_of`: the prior `BTreeMap<TweakId, _>` iteration followed by stable
    sort on index is equivalent to the new explicit `(index, TweakId)` sort.
  - Tweak `insert` removes a replaced site's old generation/span membership
    before adding the new membership. `retire` removes each site's primary,
    generation, and span entries.
  - `migration_generation_in_span`: the span-key range includes starts within
    the query span; its containment and prior-generation filter matches the
    old full scan, and max `(generation, id)` preserves latest-generation
    tie selection.
- Empty-write-set verdict: standalone forms with no writes are not added to
  the graph (`Evaluator::eval_form_checked` guards `DepGraph::add` with
  `!members.is_empty()`). Every add is followed by `claim`. A reactive commit
  may replace a recorded form's write set with an empty set, but then also
  calls `claim`, whose vacuous `is_current` check inserts the form into the
  span index. Thus no graph record with an empty write set is left unclaimed
  and absent from the index; no product change was needed. Ownership changes
  only in `DepGraph::claim` through `name_owner.insert` and `bind_owner.insert`;
  no owner removal path exists.
- Added and registered `src/ns/tests/migrate_index_equivalence.rs`. It compares
  the index to the old linear reference after each evaluation step and covers
  independent forms and reevaluation, supersession, twice-evaluated empty
  expression, failed then successful same-span evaluation, and concrete tweak
  migration ids. The empty expression evaluates to `3` and is absent from the
  graph, matching the reachability finding.
- Final source hashes: `src/ns/tests/mod.rs` pre
  `fc5a0a6a0568d72efdf628416bf302881312baeb90fa5acbf64bf000930f7e3f`, post
  `14258cfe84b2b57934209655428516f7eaa59303fe11e8ecb21e3707ecb0b45e`;
  `src/ns/tests/migrate_index_equivalence.rs` absent before creation, post
  `d8cb4f0b765c6154664ac1831253401b30a1b39ed4f343eb20c7ef6264419ccc`.
  `src/ns/depgraph.rs` was temporarily mutated for the negative control and
  restored byte-for-byte (`9088d5bc5b1284d6890ac9bbdc776963ffb7de6c5ffb095ecefcc4e9dd0cfa5b`
  before and after); `src/ns/tweak.rs` and `src/ns/evaluator.rs` were unchanged.
  This plan's pre-edit SHA256 was
  `03e8f21a2cf863bc705aca27810016c242147980aac8c0911bc24294a8c52934`; its
  final post-edit digest is recorded in
  `tmp/live-perf-int/ns-index/final-sha256.txt` to avoid self-referential
  checksum text.
- Final verification (source after mutation restoration):
  - `CARGO_TERM_QUIET=true cargo fmt --check` — exit 0,
    `tmp/live-perf-int/ns-index/1-fmt-final.log`.
  - `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` — exit 0,
    `tmp/live-perf-int/ns-index/2-clippy-final.log`.
  - `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib ns::`
    — exit 0; 125 passed, 0 failed, 2316 skipped,
    `tmp/live-perf-int/ns-index/3-nextest-ns-final.log`.
  - `wc -l src/ns/depgraph.rs src/ns/tweak.rs src/ns/evaluator.rs src/ns/tests/migrate_index_equivalence.rs`
    — exit 0; 598, 220, 957, and 101 lines,
    `tmp/live-perf-int/ns-index/4-line-counts-final.log`.
- Mutation evidence only: selecting `.next_back()` failed the differential
  test as expected (1 run, 0 passed, 1 failed; expected index `FormGen(2)` vs
  linear `FormGen(1)`), log
  `tmp/live-perf-int/ns-index/mutation-current-span-last.log` (exit 100).
- Earlier failed attempts were corrected and retained: initial invalid
  expression syntax (`3-nextest-ns.log`, exit 100), then invalid identifier
  and formatting (`1-fmt-retry1.log`, exit 1;
  `3-nextest-ns-retry1.log`, exit 100); legal syntax and formatting passed in
  final verification.
- No changes were made to `depgraph.rs`, `tweak.rs`, or `evaluator.rs` in the
  final tree. Formal review and any workflow-finalization tasks remain with
  their downstream steps.

### Session: 2026-10-18 (plan revised, workflow session 438 step 4)
**Tasks Completed**: none (plan revision only).
**Notes**: Converted to a review-and-repair rerun (design section 11). Added TASK-NR1 to TASK-NR6 and the Rerun Completion Criteria. Status is In Progress until the independent review verdict is recorded.
