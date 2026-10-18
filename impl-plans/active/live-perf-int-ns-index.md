# LPI-NS-INDEX: review and prove the session-349 name-space lookup index

**Status**: Ready
**Plan ID**: LPI-NS-INDEX (wave 1)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` section 6 (I6, name-space index part)
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

- [ ] The Progress Log holds the equivalence argument for the four lookups
  and the reachability verdict for the empty-write-set candidate.
- [ ] The divergence is closed in `depgraph.rs` if reachable; otherwise no
  product change is made.
- [ ] The differential test is added and registered, and passes.
- [ ] Verification 1-4 exit 0. Mutation evidence is recorded separately.
- [ ] The Progress Log records pre/post sha256 for every edited file.

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
