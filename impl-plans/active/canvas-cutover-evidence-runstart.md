# Canvas Cutover: Large-Document Run Start Stall Fix Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-EVIDENCE-RUNSTART (wave 3, session 267; parallel with CANVAS-EVIDENCE-SILENT, -VIEWPORT, -EDITCOST)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.8 (head-only control attribution; an active 64-voice workload on the 1 MiB document), #15.3.8.9 (owner-file defect-fix rule)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-EVIDENCE-RUNSTART`)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

In run-001 the head-only control (same page, same `.vact-run` toolbar click) produced an onset in
both browsers. On the 20,000-line, 1 MiB document the same click timed out after 10 s in both
browsers (`locator.click: Timeout 10000ms exceeded`), and an earlier keyboard Run attempt
(`Mod-Shift-Enter`) did not return within 120 s (`tmp/canvas-cutover/evidence/` session 266 logs).

Playwright's click waits for the page to acknowledge the input event. The click handler runs
`EvalController.evalAll()` (`editor/src/code/eval.ts:110-113`). That sends the whole document to
the in-page Wasm session (`client.eval(file, doc.toString())`). The page main thread is
therefore blocked for more than 10 s somewhere in:

- the Wasm `eval` of the 1 MiB document; or
- the synchronous handling of its reply in the frontend (eval-result diagnostics, sites, binding
  and params refresh, highlights).

The full-document `session_check` is fast (`editor/test/e2e/large-doc.test.ts` passes under the
default vitest timeout), so parsing and checking alone are not the stall.

The design requires an active 64-voice workload on the 1 MiB document. A startup stall like this
is a product defect and must be fixed in its owning source (15.3.8.9), not recorded as
acceptable.

## Non-goals

- No change to the workload fixture (CANVAS-EVIDENCE-SILENT owns `fixtures/large-doc.mjs`). This
  plan imports `createLargeDocument()` read-only.
- No change to any `editor/src/code/*` file owned by CANVAS-EVIDENCE-EDITCOST (`input.ts`,
  `layout.ts`, `accessibility.ts`, `keyboard.ts`, `mount.ts`, `renderer.ts`) or VIEWPORT
  (`frame.ts`).
- No change to evaluation semantics, diagnostics content, the protocol wire format or audio
  output.
- No generalized background-worker architecture. Fix the measured hot spot with the smallest
  change.

## Ownership

writePaths:

- `editor/test/e2e/large-eval.test.ts` (new)
- `editor/src/code/eval.ts`
- `editor/src/code/diagnostics.ts`
- `editor/src/params/mount.ts`
- `editor/src/bind/mount.ts`
- `src/session/eval.rs`
- `src/session/tests/eval.rs`
- this plan file
- `tmp/canvas-cutover/evidence-runstart/intent.json` and `tmp/canvas-cutover/evidence-runstart/receipt.json`
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`, `tmp/canvas-cutover/evidence-runstart/logs`

sharedPaths: none at authoring time.

Escalation rule: if diagnosis puts the hot spot in a file not listed above, check two things:

- the file is not owned by another wave-3 plan (see the manifest);
- the file is not one of the EDITCOST/VIEWPORT files above.

If both hold, add it to this plan's sharedPaths with a `sharedPathNotes` entry (path and intended
edit) and record that in the progress log before editing it. Also add the file's existing unit
test file if a test change is needed. Otherwise stop and report the file for serial repair. Never
edit a file owned by another wave-3 plan.

## Tasks

### TASK-001: Reproduce and locate (diagnosis first)

`editor/test/e2e/large-eval.test.ts` (`// @vitest-environment node`) uses the real host-wasm
module. Imitate `editor/test/wasm/abi.test.ts:83`: a rig with
`send(rig, 'eval', { file, code, doc_revision, edit_epoch })`. Load helpers through the
non-literal import pattern (`large-doc.test.ts:7-10`).

- Measure the wall time of `eval` for the first 5,000 lines of `createLargeDocument().text`
  (split on `\n` and rejoin) and for the full 20,000-line text: median of 3 runs each, fresh
  `session_init` per run. Do not call `createLargeDocument({ lines: 5000 })`: the generator
  throws outside the 1 MiB +/- 5% byte window (`fixtures/large-doc.mjs:40`).
  Record both numbers and their ratio in the progress log and the test output.
- If the 20,000-line eval in node takes 2 s or more, or the ratio exceeds 8 (linear scaling is
  about 4), the hot spot is in Rust. Profile with `node --cpu-prof` on the same test (record the
  `.cpuprofile` path under `tmp/canvas-cutover/evidence-runstart/logs/`). Then fix it in
  `src/session/eval.rs` (or an escalated Rust file).
- Otherwise the hot spot is in the frontend reply handling. Add a jsdom case in the same test
  file that feeds the recorded eval-result envelope for the 20,000-line document into the
  controllers that consume it:
  - `EvalController` reply path (`eval.ts`);
  - `DiagnosticsController` (`diagnostics.ts`);
  - the params mount refresh on `sites` (`params/mount.ts:141-146,246`);
  - the binding mount refresh on `sites` (`bind/mount.ts:131`).

  Use the existing test fixtures for each module as the pattern. Time each consumer separately,
  then fix the dominant one.

### TASK-002: Fix

Apply the smallest change that removes super-linear or unbounded synchronous work on the eval
path. Examples of acceptable shapes, chosen only by what the profile shows:

- replace a repeated full-text scan per definition with one indexed pass;
- render at most a bounded number of parameter or binding rows synchronously and the rest
  lazily, with behavior unchanged for small documents;
- avoid recomputing diagnostics mapping per site.

Keep the existing outputs byte-identical for the existing tests.

### TASK-003: Regression guard

In `large-eval.test.ts`:

- `eval` of the 20,000-line workload completes with exactly one `eval-result` and zero error
  diagnostics.
- The median 20,000-line / 5,000-line time ratio is <= 8.
- The node wall time for the 20,000-line `eval` (debug wasm) is <= 5,000 ms.
- If the fix was in the frontend: the jsdom consumer time for the 20,000-line reply is
  <= 1,000 ms per consumer.

Each timing test sets an explicit per-test vitest timeout (for example 120,000 ms), because the
vitest default of 5,000 ms would fail for harness reasons. These wall-clock budgets are
deliberately loose (more than 2x the post-fix measurement). Record
the measured post-fix values. If a budget is tighter than 2x the measured value, raise it to 2x
and record why.

If a Rust test is needed (for example a structural complexity guard), add it in
`src/session/tests/eval.rs`, imitating that file's existing rows.

## Pitfalls

- Fixing the harness instead of the product, for example by raising the Playwright click timeout.
  This is forbidden.
- Changing eval semantics or dropping diagnostics to make it faster.
- Editing `fixtures/large-doc.mjs`.
- Rust: `clippy -D warnings` with no new `allow` or `expect`; rustfmt only touched files; files
  stay under 1,000 lines.

## Verification (inside the sandbox; logs under `tmp/canvas-cutover/evidence-runstart/logs/`)

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 (rebuild after any Rust change) |
| `cd editor && ./node_modules/.bin/vitest run test/e2e/large-eval.test.ts` | pass; measured times and ratio printed and recorded |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | all pass; no assertion deleted |
| If Rust changed: `rustfmt --check src/session/eval.rs src/session/tests/eval.rs` (plus any escalated Rust file) | exit 0 |
| If Rust changed: `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| If Rust changed: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run session::` | all pass |

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`.

Mutation evidence (separate list): the regression guard run against the pre-fix source must fail
(expected nonzero exit), recorded with the log path.

Outside the sandbox (wave 4): the large-document toolbar click returns within 10 s and workload
onsets are observed in both browsers. The full nextest suite runs there (timeout >= 1500 s).

## Overwrite and Drift Protocol

Record the sha256 before and after each edit in `intent.json` and `receipt.json`. If a file
drifted before your first edit, stop and report it. Edit only this plan's progress log.

## Completion Criteria

- [ ] Hot spot located with recorded timings or profile
- [ ] Fix applied in the owning file(s); any escalated paths recorded as sharedPaths
- [ ] `large-eval.test.ts` guard passes with recorded post-fix values; pre-fix failure recorded as mutation evidence
- [ ] `npm run check`, full vitest, and (if Rust changed) rustfmt, clippy and focused nextest pass

## Progress Log

### Session: 2026-10-05 (session 267 plan)
**Tasks Completed**: Plan authored; symptom and control attribution taken from run-001 and session 266 logs. Diagnosis pending.
