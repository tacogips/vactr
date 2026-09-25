# Vactrol Session Layer: Reconciliation and Bookkeeping (SS-FINAL) Implementation Plan

**planId**: SS-FINAL (issue #4 serial reconciliation: join integrity, fixture reclassification, final-tree checks, vactrol-core.md TASK-009 bookkeeping, README, commit staging list and message)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 14.5.12 (FINAL row, verification), 14.5.3 (ownership), 6.5.7 (evidence rule), 7.1.7 (fixture classes); design-docs/user-qa/pending-session-questions.md (S1-S6 carried as residual)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: SS-CLI, SS-LSP
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json (never edited by this plan)

---

## Intent and Context

Every implementation wave has joined. This plan runs alone and is the last serial repair point. It:
- reclassifies the one spec fixture block still deferred to TASK-009 (lang-reference.md ordinal 5, "State and
  modules": `import`, `pads.warm`, `load ./soundpack/..`);
- runs every issue #4 check on the whole tree, including `--features lsp` and the LSP smoke test;
- does the cross-plan bookkeeping: vactrol-core.md TASK-009 checkboxes with evidence and status COMPLETED, with the
  audible gate "pending user confirmation" and its automated proxy; SS plan statuses; the README; the commit staging
  list; and the six-section commit message for the workflow's step 9.

## Non-Goals

- No new features. A defect is repaired serially in the smallest edit, with an intent snapshot, and recorded.
- Never edit the dispatch manifest. Never commit (the workflow commit step does, or the operator does if the
  git-commit node rejects the multi-line message).
- Never move plan files to `completed/` during the run; archiving is a separate docs commit after the workflow
  commit, as in issues #2 and #3.
- The audible REPL gate stays "pending user confirmation".

## writePaths (exclusive in wave 5)

- `tests/fixtures/spec/manifest.toml` (lang-reference ordinal 5, plus any multiset repin the joined tree needs)
- `tests/support/eval.rs` (a `Session`-based evaluation path for blocks that need package loading), `tests/spec_fixtures.rs`
  (ONLY if the runner must dispatch that path)
- `impl-plans/active/vactrol-core.md`, `impl-plans/README.md`, `impl-plans/active/vactrol-session-finalize.md`

## sharedPaths (serial; an intent snapshot before every edit)

- Status lines and a closing note only: `impl-plans/active/vactrol-session-{contracts,pkg,directives,analysis,core,cli,lsp}.md`
- Serial repair and `rustfmt` only, when a check on the joined tree fails in that file: every file in any SS plan's
  writePaths, as enumerated for SS-FINAL in the dispatch manifest `sharedPaths`. Each repaired file and its reason is
  recorded.

## File-Level Changes

1. **Join integrity.** Before any edit, run `shasum -a 256 -c` against every SS plan's `final-hashes.txt`. Explain each
   mismatch (a later plan's legitimate edit, or drift) in
   `tmp/ss-session-20260925-s183/SS-FINAL/attempt-<n>/join-integrity.txt`.
2. **`tests/support/eval.rs` + `tests/fixtures/spec/manifest.toml`.**
   - Evaluate lang-reference ordinal 5 through `vactrol::session::Session` with `NoopHost` and no lock.
   - Move it from `deferred` (`deferred_to = "TASK-009"`) to `diagnostic`, with exact `check_diags`/`run_fails`
     multisets (7.1.7). Expected: `package-not-locked` at the `import` line, `pads.warm` undefined, `load ./soundpack/..`
     `host-unavailable`, and the existing pins.
   - Its `note` says why.
   - `grep -c 'deferred_to = "TASK-009"'` must print 0.
3. **`impl-plans/active/vactrol-core.md`, TASK-009.** Each checkbox is checked ONLY with cited evidence: the plan, the
   test file and test names, the log path and its `exit=`.
   - Criterion 1: `src/session/tests/{eval,publish}.rs`.
   - Criterion 2: `src/session/tests/packages.rs` + `src/pkg/tests/proxy.rs`.
   - Criterion 3: `src/pkg/tests/{validate,zip,digest,cache}.rs`.
   - Criteria 4-5: `src/directives/tests/*` + `tests/directive_fixtures.rs` + `src/session/tests/directives.rs`.
   - Criterion 6: `src/directives/tests/{key,persist}.rs`.
   - Criterion 7: `src/session/tests/{authority,tiers}.rs`.
   - Criterion 8, the audible gate: "PENDING USER CONFIRMATION (manual: `vactrol repl` on a real output device, then
     `s :analog > note [:a4] > d1`). Automated proxy: `src/session/tests/repl.rs` audible-gate proxy". Per issue #4
     this does not block acceptance.
   - Criterion 9: `src/session/tests/repl.rs` + `tests/cli.rs`.
   - Criterion 10: `tests/lsp_smoke.rs`, log `ss-final-lsp-smoke`.
   - Criterion 11: the final build, build-lsp and nextest logs.

   Also update this file:
   - TASK-009 status: COMPLETED, only when every box except criterion 8 is checked.
   - Amend the deliverable text with the accepted divergences of design 14.5.1: offline `render` through
     `NativeAudioHost::headless` instead of `Engine::render`; the browser store's `fetch()`/OPFS backend in TASK-010;
     the session socket in `src/cli/ws.rs`.
   - Update the Module 8 status line and the Module Status table, and add a progress-log entry.
4. **`impl-plans/README.md`.** Update the vactrol-core.md row and the eight SS plan rows (Completed) and the manifest
   row.
5. **Plan statuses.** Set the SS plan status lines to Completed, with a closing note citing the final logs.
6. **Commit staging list and message**, recorded in this plan's progress log for step 9.
   - Explicit paths only: `Cargo.toml`, `Cargo.lock`, `src/`, `tests/`, `design-docs/`, `impl-plans/`.
   - The message uses the six CLAUDE.md sections on separate lines, with no AI attribution and no Co-Authored-By line.
   - The operator commits and pushes if the git-commit node rejects the multi-line message.

## Invariants

- A checkbox is checked only with evidence. An unmet criterion is reported, never checked.
- Crate-wide `cargo fmt --check` passes. If it does not, `cargo fmt` is run once and every touched file is listed.
- No `.rs` file reaches 800 lines. No gated crate reaches wasm32.

## Edit Protocol

The common protocol in `vactrol-session-contracts.md`, except rule 5: this plan may run crate-wide `cargo fmt` once,
recorded. Evidence goes under `tmp/ss-session-20260925-s183/SS-FINAL/attempt-<n>/`.

## Verification (the issue's final-tree contract; `<wave>` = `final`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, all on the final tree after the last
repair, plus:

| # | Command | Evidence |
|---|---------|----------|
| F1 | LOG(`ss-final-lsp-smoke`): `CARGO_TERM_QUIET=true cargo test --features lsp --test lsp_smoke` | `exit=0` |
| F2 | LOG(`ss-final-cli`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(cli)'` | `exit=0`, run > 0 |
| F3 | LOG(`ss-final-session`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests/) \| test(/pkg::tests/) \| test(/directives::tests/) \| binary(directive_fixtures)'` | `exit=0`, run > 0 |
| F4 | `grep -c 'deferred_to = "TASK-009"' tests/fixtures/spec/manifest.toml \|\| true` | prints `0` |
| F5 | `CARGO_TERM_QUIET=true cargo build --example beep` (LOG `ss-final-example`) | `exit=0` |
| F6 | `jq . impl-plans/active/ss-session-20260925-s183-dispatch.json > /dev/null` | exit 0 (read-only) |
| F7 | `git status --short` and `git diff --stat` | recorded |

## Completion Criteria

- [ ] Join integrity checked and explained
- [ ] Lang-reference ordinal 5 reclassified (F4 = 0); the fixtures are green
- [ ] V1-V9 and F1-F7 pass on the final tree with logs cited
- [ ] vactrol-core.md TASK-009 bookkeeping done with evidence (criterion 8 pending user confirmation with its proxy);
      status COMPLETED; the README and SS plan statuses updated
- [ ] Commit staging list and six-section message (no attribution lines) recorded; the dispatch manifest is not edited

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-FINAL)` entry. Edit only this log, plus the status lines
and closing notes allowed above.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-cli.md, vactrol-session-lsp.md


### STEP6 OUTPUT NOTE (operator, 2026-09-25, after the SS-ANALYSIS attempt-1 failure)

- The step6-implement output contract requires `changedFiles` to be an ARRAY of
  path strings (SS-ANALYSIS attempt 1 failed with "$.changedFiles must be of type
  array"). Carry `planId`; leave `verificationGaps` empty when every automated
  command passed (manual checks go under `residualRisks`). Crate-wide test
  failures caused only by a sibling branch's in-progress files or by a
  pre-existing test outside every plan's ownership are reported in the
  progress log as a dependency blocker for the operator, never fixed by
  editing unowned files.
