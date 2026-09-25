# Vactrol Middle End: Reconciliation and Plan Bookkeeping (ME-FINAL) Implementation Plan

**planId**: ME-FINAL (reconciles TASK-004..006 for issue #2)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 6.5.7 (evidence rule), 7.1.7 (verification, rollback)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/2
**dependsOn**: ME-INTEGRATE (and transitively every ME plan)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

All code waves are done. This plan verifies the whole tree once more, runs the crate-wide formatter check, and updates
the shared indexes that only a serial final step may touch: `vactrol-core.md` (TASK-004..006 status, criteria, module
status, dependencies, module sketches, progress log), `impl-plans/README.md`, the ME plan headers, the dispatch
manifest, and the carried front-end bookkeeping. It documents the explicit staging list for the workflow commit step.
It does not commit.

## Non-Goals

- No source changes except `cargo fmt` output if `cargo fmt --check` fails (then re-run every row).
- No change to TASK-001..003 or TASK-007..010 text in `vactrol-core.md` except the module-sketch lines listed below.
- No `git mv` of plans in this workflow: plans are marked Completed with archiving noted (acceptance allows this); the
  move to `impl-plans/completed/` happens after the workflow commit, when the user confirms, as for the front-end plans.
- No commit, push or PR (the workflow commit step does that).

## writePaths (exclusive)

- `impl-plans/active/vactrol-core.md`, `impl-plans/README.md`
- `impl-plans/active/me-middle-20260925-s175-dispatch.json` (plan statuses and a `completion` note)
- `impl-plans/completed/fe-frontend-20260925-s165-dispatch.json` (append a `closure` note only)
- `design-docs/specs/design-implementation.md` (only to record implementation differences that the ME plan progress
  logs report as accepted design differences; each edit cited to its progress-log entry; none is expected)
- `impl-plans/active/vactrol-middle-finalize.md`

## sharedPaths

- The seven other ME plan files: header `**Status**` line only (`Completed (archiving to impl-plans/completed/ after the
  workflow commit, on user confirmation)`). Never edit their progress logs.
- Any `.rs` file: only as rewritten by `cargo fmt` (record the file list).

## Tasks

1. Check join integrity: for each ME plan, `shasum -a 256 -c tmp/me-middle-20260925-s175/<planId>/attempt-<n>/final-hashes.txt`
   (the latest attempt). A mismatch is acceptable only when a later plan's progress log records the intended edit of that
   file; list every mismatch and its explanation.
2. Run the verification table below. If V9 fails, run `CARGO_TERM_QUIET=true cargo fmt`, record the changed files, and
   re-run every row with new logs.
3. `vactrol-core.md`:
   - TASK-004, TASK-005, TASK-006: `**Status**: COMPLETED`; check every completion-criteria box, each backed by a named
     test or log in the new progress-log entry. The TASK-006 "tweaked PParam probability ... via staging invalidation and
     re-query (design 11.3)" box is checked with the note that the re-query semantics are tested (ME-INTEGRATE) and the
     11.3 staging invalidation lands with TASK-007. If any criterion is not met, leave it unchecked and report it; never
     check a box without evidence.
   - Module Status rows for Checker, Namespace/compiler/VM and Pattern engine + visuals + clock -> COMPLETED with test counts.
   - Dependencies table rows TASK-004..006 -> COMPLETED.
   - Module sketches: Module 3 `Ty` gains `Path, Url, Sound`; Module 4 `Namespace` comment reads "prelude (read-only) ->
     session -> fn/block scope chain (design 5.6, 20 Q1)" instead of "SINGLE global scope"; Module 5 `PatNode` gains
     `Sound { src, kit }` and `MidiNotes { subject, channel }`, and `Pat` gains `structured`.
   - Verification section: note that the rows were executed for TASK-004..006 (session number) with both wasm32 builds.
   - Progress log: one entry `### Session: <date> (issue #2, TASK-004..006 implemented)` with the per-wave summary, the
     final logs (`final-<check>-s<S>-<n>.log`), nextest/cargo test counts, the largest `.rs` file, the fixture class
     counts, the open user-QA items (M4, M5, M6 followed by recommendation; `fn f a b` and `let`/`upd base` authority
     questions pending), and a correction of the revision-16 notes: 6.5.5 now records the expander `MAX_DEPTH = 512`, and
     the `[:g :7]` conflict is resolved by the letter-first chord decision (`[:g :dom7]`).
   - Related Plans: list the eight ME plans and the dispatch manifest.
4. `impl-plans/README.md`: set the eight ME rows (added at plan creation) to Completed (archiving pending) and update
   the `vactrol-core.md` row (TASK-001..006 completed; TASK-007..010 not started).
5. `impl-plans/completed/fe-frontend-20260925-s165-dispatch.json`: append
   `"closure": {"date": "<date>", "note": "RECON3-1 resolved: FE-READER review verdict accepted (attempt-2, findings []); all FE plans archived in 850c606."}`
   (carried TODO from 0bee1fb); keep the JSON valid (`python3 -m json.tool` or `jq .` exit 0).
6. Dispatch manifest: set each plan's `status` to `completed` and add a `completion` note with the final log names.
7. Record the staging list for the workflow commit step (item "Commit staging" below) in the progress log, with
   `git status --short` output.

## Commit Staging (for the workflow commit step; this plan does not commit)

Stage by explicit path only (never `git add -A` or `git add .`): `src/`, `tests/`, `design-docs/specs/`,
`design-docs/user-qa/`, `impl-plans/active/`, `impl-plans/completed/fe-frontend-20260925-s165-dispatch.json`,
`impl-plans/README.md`. `Cargo.toml` and `Cargo.lock` must be unchanged (V7). `target/` and `tmp/` are ignored and never
staged. The message uses the six CLAUDE.md sections on separate lines and has no AI attribution or Co-Authored-By line.
After staging, `git diff --staged --stat` is shown and `git status --short` must show no untracked path under `src/` or
`tests/`.

## Verification

The ME-MASKS table and log rule with `<plan>` = `final`: V1, V2, V3, V3t, V3f, V6a, V6b, V4 (under 800), V5, V7, plus:

| # | Command | `<check>` | Evidence |
|---|---------|-----------|----------|
| V9 | `CARGO_TERM_QUIET=true cargo fmt --check` | `fmt` | `exit=0` |
| V10 | `git diff -U0 -- impl-plans/active/vactrol-core.md \| grep -nE 'TASK-00[1-37-9]\|TASK-010' \|\| echo none` | - | only lines this plan intended (module sketches, Related Plans); record output |
| V11 | `grep -c 'eval = "unclassified"' tests/fixtures/spec/manifest.toml` | - | `0` |
| V12 | `jq . impl-plans/completed/fe-frontend-20260925-s165-dispatch.json > /dev/null && jq . impl-plans/active/me-middle-20260925-s175-dispatch.json > /dev/null` | - | exit 0 |
| V13 | `git status --short` | - | recorded; no unexpected paths |

## Completion Criteria

- [ ] Join integrity checked; mismatches explained
- [ ] V1-V13 pass (V9 crate-wide fmt) with logs cited
- [ ] vactrol-core.md TASK-004/005/006 COMPLETED with every criterion checked and evidenced (or an unmet criterion reported, not checked)
- [ ] Module Status, Dependencies, module sketches and progress log updated; README index updated
- [ ] ME plans marked Completed with archiving noted; dispatch manifest statuses set; FE dispatch closure note added
- [ ] Commit staging list recorded

## Progress Log

(Implementer: one entry per session: work done, evidence per row, mismatches, formatter changes, blockers.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md
- **Previous**: vactrol-middle-integrate.md
