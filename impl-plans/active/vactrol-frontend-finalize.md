# Vactrol Front End: Serial Finalization (FE-FINAL) Implementation Plan

**planId**: FE-FINAL (reconciles vactrol-core.md for TASK-001..003)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 6.5.7; impl-plans/active/vactrol-core.md
**Created**: 2026-09-25
**Last Updated**: 2026-09-25
**dependsOn**: FE-VALUE, FE-READER, FE-EXPAND (wave 4; serial, and the only plan that edits shared indexes)

---

## Intent and Context

Workflow acceptance requires `impl-plans/active/vactrol-core.md` to show TASK-001..003 complete, with their
criteria checked, the module status rows updated, and a progress-log entry for this session. No other task
may be touched. This plan performs those shared-index edits once, after all three implementation plans are finished and
verified, and it runs the full verification suite as final evidence. It also records the small design and plan
revisions that implementation exposed.

## Non-Goals

- No Rust source changes, except a fix required by a failing final verification. Such a fix is made serially and recorded.
- No edits to TASK-004..010, their rows, or the plan's overall Completion Criteria checkboxes.
- No archiving: `vactrol-core.md` stays in `active/` because TASK-004..010 remain.
- No `cargo fmt` across the crate unless `cargo fmt -- --check` fails. If it fails, run `cargo fmt` once and record it.

## writePaths (serial, and exclusive in this wave)

- `impl-plans/active/vactrol-core.md`:
  - Header: `**Status**: In Progress`, `**Last Updated**: <session date>`.
  - Module 1 and Module 2 `**Status**` lines. Set `COMPLETED` for value, reader and expander. When U1 is still open, use the value text `COMPLETED (wasm32 build pending user-qa U1)`.
  - TASK-001, TASK-002, TASK-003: set `**Status**` and check each completion-criteria checkbox that has evidence.
    TASK-001's wasm32 checkbox stays unchecked, with an inline reason, unless V6 of FE-VALUE actually ran and passed.
  - The Module Status table rows for `src/value/`, `src/reader/` and `src/expand/`: Status, and Tests (the nextest count from the final log).
  - The Dependencies table rows TASK-001..003.
  - Verification table: change the Lint command to `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
    (revision 16, stricter). In the Verification preamble, replace the sentence saying that none of the commands has been executed.
  - Related Plans: list the four FE-* plans.
  - Progress Log: add a `### Session: <date> (revision 16, TASK-001..003 implemented)` entry. It records tasks completed, the
    command outcomes with exit codes and log paths, the open user-qa items U1-U5, and the plan revisions: the stricter lint,
    the FE sub-plans, the `misplaced-arrow` code, and any diagnostic code added during FE-READER or FE-EXPAND.
- `impl-plans/README.md`: Active Plans table statuses for vactrol-core.md and the four FE-* plans.
- The four FE-* plan files: set each header `**Status**` to `Completed`. Leave them in `active/` until the user confirms
  archiving; do not move them in this workflow.
- `design-docs/specs/design-implementation.md` 6.5.4: append one sentence naming `misplaced-arrow` as the code for `->` inside `[..]`,
  plus any code added during implementation. This is the only design edit allowed, and it only records an implementation-revealed detail.

## Invariants

- The text of TASK-004..010 is byte-identical before and after this plan. Check it with the diff command V7.
- Only criteria with recorded evidence are checked. Anything blocked stays unchecked with its reason.
- The `vactrol-core.md` file stays under 1000 lines, per the README size rule. It is 992 lines at plan time.
  Keep the new progress-log entry to at most 6 lines, and put the FE-* plans on a single Related Plans line. If
  `wc -l` would still reach 1000, shorten the revision-15 session entry to one summary line inside the condensed
  "Sessions: 2026-09-24/25" entry. Do not delete any other content.

## Edit Protocol

For each file: read it fresh, record `shasum -a 256` in the FE-FINAL progress log, edit, and record the new hash. If an
unexpected change appears (the hash does not match the value recorded at plan start), stop, diff it, and reapply the intent.

## Verification (foreground; logs under target/fe-logs/)

In the tables, `\|` stands for a literal `|`, and the exit status is `${PIPESTATUS[0]}`.

| # | Command | Evidence |
|---|---------|----------|
| V1 | `CARGO_TERM_QUIET=true cargo build 2>&1 \| tee target/fe-logs/final-build.log` | exit 0, no warnings |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings 2>&1 \| tee target/fe-logs/final-clippy.log` | exit 0 |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run 2>&1 \| tee target/fe-logs/final-nextest.log` | exit 0; record the passed count |
| V4 | `cargo fmt -- --check` | exit 0 |
| V5 | `find src tests -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | every file is under 1000 lines |
| V6 | `ls src` | exactly `compile dsp expand lib.rs main.rs ns pattern reader sched tex types value vm` (the section 4 layout subset) |
| V7 | `git diff -U0 -- impl-plans/active/vactrol-core.md \| grep -n 'TASK-00[4-9]\|TASK-010' \|\| echo none` | prints `none`, or only Related Plans and Progress Log lines |
| V8 | `git diff 8681890 -- Cargo.toml` (8681890 is the base commit before this run) | the only change is the added `[features]` table |
| V9 | `rustup target list --installed` | wasm32 present: run the wasm build and record the result; absent: record "blocked on U1" |

## Completion Criteria

- [ ] vactrol-core.md: TASK-001..003 status, checkboxes, Module Status rows, Dependencies rows, Verification lint and progress log are updated; no other task changed (V7)
- [ ] impl-plans/README.md and the FE-* plan headers are updated
- [ ] V1-V6 and V8 pass; V9 is recorded
- [ ] The final commit (one commit on `main`, AGENTS.md six-section message, no AI attribution or Co-Authored-By line) is left to the workflow's commit step

## Progress Log

### Session: (implementer fills in)
**Tasks Completed**:
**Hashes**:
**Verification evidence**:
**Blockers**:

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md
- **Previous**: impl-plans/active/vactrol-frontend-expander.md
