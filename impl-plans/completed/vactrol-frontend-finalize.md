# Vactrol Front End: Serial Finalization (FE-FINAL) Implementation Plan

**planId**: FE-FINAL (reconciles vactrol-core.md for TASK-001..003)
**Status**: Completed (session 175: reconciliation and final verification done; the commit belongs to the workflow commit step; stays in active/ until the user confirms archiving)
**Design Reference**: design-docs/specs/design-implementation.md 6.5.7; impl-plans/active/vactrol-core.md
**Created**: 2026-09-25
**Last Updated**: 2026-09-25 (session 175, FE-FINAL step 6 implementer)
**Issue**: https://github.com/tacogips/vactrol/issues/1
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
  - Module 1 and Module 2 `**Status**` lines. Set `COMPLETED` for value, reader and expander.
  - TASK-001, TASK-002, TASK-003: set `**Status**` and check each completion-criteria checkbox that has evidence.
    TASK-001's wasm32 checkbox is checked only on the evidence of V9a and V9b below (U1 is answered; a failing
    wasm32 build is a defect to fix serially, not a blocker to record).
  - The Module Status table rows for `src/value/`, `src/reader/` and `src/expand/`: Status, and Tests (the nextest count from the final log).
  - The Dependencies table rows TASK-001..003.
  - Verification table: change the Lint command to `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
    (revision 16, stricter). In the Verification preamble, replace the sentence saying that none of the commands has been executed.
  - Related Plans: list the four FE-* plans.
  - Progress Log: add a `### Session: <date> (revision 16, TASK-001..003 implemented)` entry. It records tasks completed, the
    command outcomes with exit codes and log paths, user-qa U1-U5 as answered (with the U5 checker obligation noted
    under TASK-004 as a one-line pointer in this progress entry only, not in TASK-004's text), and the plan revisions: the
    stricter lint, the FE sub-plans, the `misplaced-arrow` code, and any diagnostic code added during FE-READER or FE-EXPAND.
- `impl-plans/README.md`: Active Plans table statuses for vactrol-core.md and the four FE-* plans.
- The four FE-* plan files: set each header `**Status**` to `Completed`. Leave them in `active/` until the user confirms
  archiving; do not move them in this workflow.
- `design-docs/specs/design-implementation.md`, exactly two edits:
  - 6.5.4: append one sentence naming `misplaced-arrow` as the code for `->` inside `[..]`, plus any code added during
    implementation. It only records an implementation-revealed detail.
  - Section 5.6, the late-binding bullet (line ~425 at plan time): replace the inline example `fn kick-sound: :bd-tek`
    with a block-form reference, for example "re-running the block-form `fn kick-sound:` with body `:bd-tek`". This
    aligns the prose with U4 (Step 3 design-review low finding); it changes no behavior.

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

Run `mkdir -p target/fe-logs` first. Log evidence follows design 6.5.7 ("Verification evidence"):
- `LOG` for a cargo row is a new file `target/fe-logs/final-<check>-s<S>-<n>.log` (`<S>` is the session number, `<n>`
  counts from 1 per check within the session). Never reuse or overwrite an existing file.
- Run every cargo row as `(set -o pipefail; CMD 2>&1 | tee LOG); echo "exit=$?" >> LOG`.
- The progress log cites, per cargo row, the counting log path (the last run after the final code change) and its
  `exit=` value. For V3 it also cites the run and passed counts, and the run count must be non-zero. A missing log, a
  log without `exit=`, or a truncated log fails the row. Other rows record their output inline.
In the tables, `\|` stands for a literal `|`. Rows with `-` in the `<check>` column are run as written.

| # | Command (`CMD`) | `<check>` | Evidence |
|---|---------|-----------|----------|
| V1 | `CARGO_TERM_QUIET=true cargo build` | `build` | `exit=0`, no warnings |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` | `clippy` | `exit=0` |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run` | `nextest` | `exit=0`; record the run and passed counts (run non-zero) |
| V3t | `CARGO_TERM_QUIET=true cargo test` | `cargotest` | exit 0 and a non-zero test count; run in addition to V3 because the workflow gate recognizes `cargo test` but not `cargo nextest run` as behavioral test evidence (2026-09-25) |
| V3f | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `fixtures` | `exit=0`; a non-zero run count and 0 failed (issue acceptance: the spec-fixture harness ran) |
| V4 | `CARGO_TERM_QUIET=true cargo fmt -- --check` | `fmt` | `exit=0` |
| V5 | `find src tests -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every file is under 1000 lines |
| V6 | `ls src` | - | exactly `compile dsp expand lib.rs main.rs ns pattern reader sched tex types value vm` (the section 4 layout subset) |
| V7 | `git diff -U0 -- impl-plans/active/vactrol-core.md \| grep -n 'TASK-00[4-9]\|TASK-010' \|\| echo none` | - | prints `none`, or only Related Plans and Progress Log lines |
| V8 | `git diff 6500f37 -- Cargo.toml` (6500f37 is the session-167 original HEAD; Cargo.toml is unchanged since 8681890) | - | the only change is the added `[features]` table |
| V9a | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown` | `wasm32` | `exit=0` |
| V9b | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` | `wasm32-hostwasm` | `exit=0` |
| V10 | `wc -l impl-plans/active/vactrol-core.md` | - | under 1000 |
| V11 | `grep -n 'fn kick-sound: :bd-tek' design-docs/specs/design-implementation.md \|\| echo none` | - | prints `none` (the 6.5.5/6.5.6 mentions of the inline `:bd-haus` form are intentional and stay) |

## Completion Criteria

- [x] vactrol-core.md: TASK-001..003 status, checkboxes, Module Status rows, Dependencies rows, Verification lint and progress log are updated; no other task changed (V7)
- [x] impl-plans/README.md and the FE-* plan headers are updated
- [x] V1-V6 (including V3t and V3f), V8, V9a, V9b, V10 and V11 pass
- [x] design-implementation.md has only the two edits listed above (`git diff 67ecd5a -- design-docs/specs/design-implementation.md` shows exactly those two hunks)
- [ ] The final commit (one commit on `main`, AGENTS.md six-section message, no AI attribution or Co-Authored-By line) is left to the workflow's commit step. FE-FINAL does not commit. The commit step stages the explicit path list in FE-VALUE Edit Protocol item 5, never `git add -A`/`git add .`, and checks `git status --short` afterwards: no untracked path may remain under `src/` or `tests/`

## Progress Log

### Plan revision: 2026-09-25 (session 167, plan author)
U1-U5 are answered: the wasm32 builds (V9a, V9b) are required, and U1-U5 are recorded as answered. V8 uses base 6500f37.
Added the section 5.6 wording fix (V11) and the explicit line-count check (V10). No other change.

### Plan revision: 2026-09-25 (session 169, plan author)
The verification table follows design 6.5.7: per-run logs `final-<check>-s<S>-<n>.log`, `exit=` recorded inside each
log, and non-zero nextest run counts. V4 now also writes a log (`fmt`). The session-169 6.5.7 "Verification evidence"
bullet is part of the design/plan checkpoint commit, not an FE-FINAL edit. The "exactly two edits" rule is measured
against that commit. No other change.

### Plan revision: 2026-09-25 (session 170, plan author)
V3f (`final-fixtures-s<S>-<n>.log`) is added so the final evidence meets the issue's non-zero `spec_fixtures`
acceptance signal. V3t and V3f are now named in the completion criteria. The two-edit check is pinned to base 67ecd5a,
the last commit to touch design-implementation.md. The commit contract points to the FE-VALUE item 5 staging list
(single front-end commit, including the untracked `src/` directories). No other change.

### Session: 2026-09-25 (session 175, FE-FINAL step 6 implementer)
**Tasks Completed**: every writePaths edit above. No Rust source changed: the 68 `src/`/`tests/` files match
`pre-source-hashes.txt`, and `cargo fmt -- --check` passed, so no crate-wide fmt ran. Intent is in
`tmp/fe-frontend-20260925-s165/FE-FINAL/attempt-1/intent.md`.
- vactrol-core.md: header In Progress; Module 1/2, TASK-001..003, Module Status (value 57, reader 33, expand 55 unit
  tests; spec_fixtures 8 shared) and Dependencies rows COMPLETED; every TASK-001..003 checkbox checked; the Verification
  preamble and the Lint row (`--all-targets`) updated; one Related Plans line; a 6-line revision-16 progress entry. It is
  996 lines because the revision-15 entry was folded into one line of the condensed entry (no other content deleted).
- design-implementation.md, exactly two hunks against 67ecd5a: the 5.6 late-binding bullet now refers to re-running the
  block-form `fn kick-sound:` with body `:bd-tek` (U4), and the 6.5.4 `[..]` bullet gains one sentence naming
  `misplaced-arrow` and the FE-READER code `empty-pipe`.
- README Active Plans statuses; FE-VALUE, FE-READER and FE-EXPAND headers set to Completed; no plan was moved.
**Hashes** (sha256 prefix, before -> after): vactrol-core.md 7d05836d -> 0f4bcf61; README.md 3ce6b575 -> 5cb18791;
design-implementation.md ef7f4af6 -> 704d23f5, then (the 6.5.4 sentence narrowed: the `line.rs:169` branch is unreachable per the FE-READER attempt-2 verdict) the value in `post-edit-hashes.txt`; frontend-value d164aef0 -> 39c6d061; frontend-reader 14032f33 ->
3bd37fcb; frontend-expander 803b6575 -> 0b0df319; this file 7709631e before the edit (after-hash is in the evidence dir).
**Verification evidence** (`target/fe-logs/`, one run each after the final state, every log ends in `exit=0`):
- V1 `final-build-s175-1.log`; V2 `final-clippy-s175-1.log`; V4 `final-fmt-s175-1.log`.
- V3 `final-nextest-s175-1.log`: 155 tests run, 155 passed, 0 skipped. V3t `final-cargotest-s175-1.log`: lib 147 passed,
  spec_fixtures 8 passed, 0 failed. V3f `final-fixtures-s175-1.log`: 8 run, 8 passed.
- V9a `final-wasm32-s175-1.log`; V9b `final-wasm32-hostwasm-s175-1.log`.
- V5: largest `src/reader/line.rs` 741, `src/reader/lexer.rs` 690. V6: `compile dsp expand lib.rs main.rs ns pattern
  reader sched tex types value vm`. V7: only Progress Log lines (the two folded revision-15 lines and the U5 pointer)
  plus a hunk-header context line. V8: only the `[features]` table. V10: 996. V11: `none`.
**Blockers**: none.
**Deferred notes**: the FE-EXPAND request to write the expander depth cap (512, `nesting-too-deep`) into 6.5.5 was not
applied, because this plan allows exactly two design edits; it is recorded in the vactrol-core.md progress entry. The
last criterion (the commit) belongs to the workflow commit step and stays unchecked here.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md
- **Previous**: impl-plans/active/vactrol-frontend-expander.md
