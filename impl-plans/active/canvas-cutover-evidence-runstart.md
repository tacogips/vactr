# Canvas Cutover: Large-Document Run Start Stall Fix Implementation Plan

**Status**: Ready (session 271: serial perf-gate split, then gates; the Rust fix is implemented)
**Plan ID**: CANVAS-EVIDENCE-RUNSTART (dispatch wave 3 in session 271; runs alone. CANVAS-EVIDENCE-EDITCOST depends on it.)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.11 (session 271: serial perf gate, operator decision P; supersedes the 15.3.8.10 "Wall-clock vitest gates" paragraph), #15.3.8.10 (mutation evidence), #15.3.8.8 (head-only control attribution; an active 64-voice workload on the 1 MiB document), #15.3.8.9 (owner-file defect-fix rule)
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
  `layout.ts`, `accessibility.ts`, `keyboard.ts`, `mount.ts`, `renderer.ts`, and since session 269
  `sync.ts`, `syntax.ts`, `syntax-core.ts`, `history.ts`, `surface.ts`) or to the EDITCOST test
  files, or to VIEWPORT (`frame.ts`).
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
- Session 269 operator authorization 2: the six Rust files edited in session 267 under the
  escalation rule below are now writePaths. RUNSTART is their only active owner. Their intended
  edits are the notes in the escalation list below, and the edits already exist at `6f6b807`.
  - `src/ns/evaluator.rs`
  - `src/ns/eval_doc.rs`
  - `src/ns/journal.rs`
  - `src/ns/depgraph.rs`
  - `src/ns/insts.rs`
  - `src/compile/compiler.rs`
- this plan file
- `tmp/canvas-cutover/evidence-runstart/intent.json` and `tmp/canvas-cutover/evidence-runstart/receipt.json`
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`, `tmp/canvas-cutover/evidence-runstart/logs`, `tmp/canvas-cutover/evidence-runstart/scratch` (session 269 mutation build; gitignored)

sharedPaths (session 269): `editor/test/e2e/large-doc.test.ts`, conditional and limited to
TASK-204 below.

Historical session-267 escalation notes (recorded before the edits; these files are now
writePaths):

- `src/ns/evaluator.rs`: checker environment reconstruction in `Evaluator::eval_form` is the measured hot path. Use an empty environment only for forms with no free session symbols/qualified references; avoid constructing callable certification snapshots when no callables were checked. The bounded snapshot bytecode proof also permits the compiler's `Force` on constant-only definitions.
- `src/ns/eval_doc.rs`: apply the same environment selection to both manifest-specific checks in `eval_form_in` so the caller path does not reconstruct the full namespace for independent scalar definitions. Cache a small number of dynamic manifests keyed by base manifest and shared control-set identity.
- `src/ns/journal.rs`: add an explicit subset snapshot constructor for statically pure constant definitions; the evaluator selects it only for `LoadConst`/`DefGlobal`/`Force`/`Deref`/`Ret` code with no non-definition globals, leaving all other transactions on the existing full rollback snapshot.
- `src/ns/depgraph.rs`: maintain a reverse reader index for committed edges, failed-attempt reads, and recovery subscriptions; use it to avoid scanning every historical form for each newly defined name. Also maintain a generation-to-form index because `Session::note_revision` calls `form_of_gen` for each emitted form and its current full-form scan remains quadratic; journal validation updates that index when a form commits a new generation.
- `src/compile/compiler.rs`: store installed custom-control names as a shared immutable set in `CompileCx`; the profile showed reconstructing the same `BTreeSet` for every independent form.
- `src/ns/insts.rs`: maintain the deduplicated declared-control set when an instrument is installed or replaced, and expose its shared `Rc` so manifest and compiler consumers do not rebuild names from all instruments per form.
- `src/ns/evaluator.rs` and `src/ns/eval_doc.rs`: reuse the already computed default `CheckResult` when `eval_form_in` runs the form, preserving checker diagnostics/callable certification while removing its duplicate default check.

`src/ns/evaluator.rs`, `src/ns/eval_doc.rs`, `src/ns/journal.rs`, `src/ns/depgraph.rs`, `src/compile/compiler.rs`, and `src/ns/insts.rs` are not owned by another wave-3 plan. The initial controlled session is still within the original plan boundary; these files are added under its explicit escalation rule.

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

## Session 269 Amendment (operator authorization 2; design 15.3.8.10)

This section supersedes the earlier text wherever they conflict.

### Intent

The Rust fix from session 267 (in the six writePaths above) is kept. Do not change Rust
source unless a gate below fails because of it. The remaining work is:

- the budget rule;
- the bounded pre-fix mutation evidence;
- the full gates;
- a rule for the `large-doc.test.ts` suite-load timeout (step-3 finding DR-269-L1).

Eval semantics, diagnostics and the wire format stay unchanged.

### TASK-201: Large-eval budget (design 15.3.8.10 "Wall-clock vitest gates")

- Build the host-wasm library on the final source (setup command), then run the focused test
  three times. Each run writes its own log and `.exit` file:
  `cd editor && ./node_modules/.bin/vitest run test/e2e/large-eval.test.ts > ../tmp/canvas-cutover/evidence-runstart/logs/s269-focused-<n>.log 2>&1; echo $? > ../tmp/canvas-cutover/evidence-runstart/logs/s269-focused-<n>.exit`
  for n = 1, 2, 3.
- Take `m` = the median of the three printed `20000=` values. The budget is
  `ceil(2 * m / 100) * 100` ms; the operator expects about 5,200 ms.
- Edit only `editor/test/e2e/large-eval.test.ts`:
  - replace the literal `5_000` at line 51 with a named constant `LARGE_EVAL_BUDGET_MS`, set to
    the computed budget;
  - add a one-line comment citing this plan, the three measured medians and `m`.
- Keep every other assertion, including the 20000/5000 ratio of at most 8, exactly one
  `eval-result` and zero error diagnostics, and the 120,000 ms per-test timeout.
- Record the three medians, `m` and the budget in the progress log.
- Do not raise the budget above `ceil(2 * m / 100) * 100`. Do not change the ratio guard. Do not
  move the measurement to CPU time.

### TASK-202: Bounded pre-fix mutation evidence (not gating)

Build a pre-fix wasm from a scratch copy of the pre-RUNSTART source, then run today's
`large-eval.test.ts` against it. `968028c92aff8442cb87948a09812b617e54ed8e` is the parent state
whose `src/` equals today's except for the six RUNSTART files (verified by
`git diff --stat 968028c 6f6b807 -- src`).

1. `rm -rf tmp/canvas-cutover/evidence-runstart/scratch && mkdir -p tmp/canvas-cutover/evidence-runstart/scratch`
2. `git archive 968028c92aff8442cb87948a09812b617e54ed8e Cargo.toml Cargo.lock src examples tests | tar -x -C tmp/canvas-cutover/evidence-runstart/scratch`
   (`git archive` only reads `.git`, so it works in the read-only sandbox).
3. `cd tmp/canvas-cutover/evidence-runstart/scratch && CARGO_TERM_QUIET=true cargo build --offline --locked --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm > ../logs/s269-mutation-build.log 2>&1; echo $? > ../logs/s269-mutation-build.exit`
   Expected exit: 0. The scratch build writes only inside the scratch artifact root.
4. `cd editor && VACTR_WASM="$PWD/../tmp/canvas-cutover/evidence-runstart/scratch/target/wasm32-unknown-unknown/debug/vactr.wasm" timeout 900 ./node_modules/.bin/vitest run test/e2e/large-eval.test.ts > ../tmp/canvas-cutover/evidence-runstart/logs/s269-mutation-prefix.log 2>&1; echo $? > ../tmp/canvas-cutover/evidence-runstart/logs/s269-mutation-prefix.exit`
   - Expected: vitest exit 1, with a test failure caused by the 120,000 ms per-test timeout or
     the budget assertion.
   - Session 267 measured pre-fix 20,000-line evals of about 60 s, so three runs exceed the test
     timeout.
   - An outer `timeout` kill (exit 124) is neither a pass nor valid mutation evidence. Record it
     and rerun once.
5. Report `mutationEvidence`: `{ command, expectedExit: "nonzero (1)", actualExit, logPath }`.
   Never put it in the gating list.

Pitfalls:

- Do not revert the six files in the shared working tree. Use the scratch copy only.
- Do not use `git worktree`, `git stash` or `git checkout`.
- Do not point the default `target/` wasm at the pre-fix build. Use `VACTR_WASM` only.
- Leave the scratch directory in place. It is an artifact root and is never committed.

### TASK-203: Gates (inside the sandbox; logs under `tmp/canvas-cutover/evidence-runstart/logs/s269-*`, each with an `.exit` file)

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 (final source) |
| `cd editor && ./node_modules/.bin/vitest run test/e2e/large-eval.test.ts` | exit 0; medians printed and recorded |
| `cd editor && ./node_modules/.bin/vitest run test/e2e/large-doc.test.ts` | exit 0; record the test duration |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` three consecutive times (`s269-vitest-full-1..3`) | Each run exits 0 on the joined tree. In the sandbox, before EDITCOST lands, failures only in EDITCOST-owned tests (`test/canvas/edit-cost.test.ts`, `test/canvas/mount.test.ts`, `test/code/sync.test.ts`, `test/code/syntax*.test.ts`, `test/code/history.test.ts`) are reported with log paths and not fixed here. Any failure in `large-eval.test.ts` or `large-doc.test.ts` blocks acceptance. The post-join verification step repeats the three consecutive runs and is authoritative. |
| `rustfmt --check src/ns/evaluator.rs src/ns/eval_doc.rs src/ns/journal.rs src/ns/depgraph.rs src/ns/insts.rs src/compile/compiler.rs` | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0, with no new `allow` or `expect` |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run ns:: compile:: session::` | exit 0 |

Outside the sandbox (verification step, authoritative):

- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run > tmp/canvas-cutover/evidence-runstart/logs/s269-nextest-full.log 2>&1; echo $? > tmp/canvas-cutover/evidence-runstart/logs/s269-nextest-full.exit`
  must exit 0. A timeout kill (124) is neither a pass nor a failure; rerun it.
  `complete::tests::robust::every_prefix_and_mutant_is_panic_free` takes about 500 s.
- Three consecutive full `vitest run` passes on the joined tree.

### TASK-204: `large-doc.test.ts` suite-load timeout (conditional sharedPath; DR-269-L1)

`editor/test/e2e/large-doc.test.ts` is owned by the accepted CANVAS-EVIDENCE-SILENT plan. Its
test `validates the complete evaluable document with the real host-wasm session_check` has no
wall-clock assertion. It failed once only at the vitest default 5,000 ms timeout under full-suite
load, and passed alone in 1.99 s. A full-document `session_check` is an allowed whole-text,
off-edit-path operation (design 15.3.8.7), and no design budget exists for it.

Edit the file only if both conditions hold:

1. one of the three TASK-203 full runs fails on that test's default timeout;
2. the focused TASK-203 `large-doc.test.ts` run passed in under 5,000 ms.

The edit is limited to an explicit per-test timeout of `60_000` as the third argument of that one
`it(...)`, plus a one-line comment citing this plan and the measured focused duration. Change no
assertion and no other test.

Record the trigger log path, the focused duration and the edit in the progress log, with
fresh-read and post-edit sha256 values.

If the focused run itself takes 5,000 ms or more, this is a product cost. Do not edit the test.
Stop and report it for routing to the owner (design 15.3.8.10, selective redispatch).

### Completion criteria (session 269)

- [x] TASK-201: final-source focused 20,000-line medians 2,372.8, 2,366.7, and 2,382.9 ms; `m=2,372.8`; `LARGE_EVAL_BUDGET_MS=4,800`, exactly `ceil(2*m/100)*100` (`logs/s269d-focused-1..3.log`)
- [x] TASK-202: pre-fix scratch Wasm build exit 0; `s269-mutation-prefix.log` exited 1 on the 4,800 ms budget assertion after a 141,240.7 ms 20,000-line median (outer timeout did not kill the run)
- [ ] TASK-203: build, focused eval, focused large-doc, npm check, rustfmt, strict clippy and focused nextest pass. Three consecutive full-suite runs pass with `--maxWorkers=1`, but the plan-listed default `vitest run` command passed once and failed twice on the eval budget (`logs/s269e-vitest-full-1..3.log`)
- [x] TASK-203 (outside the sandbox): full nextest exit 0, 2,810 passed and 3 skipped (`logs/s269-nextest-full.log`)
- [x] TASK-204: triggered by full-suite default-timeout failures; focused duration 1.99 s; added only the 60-second timeout/comment (`editor/test/e2e/large-doc.test.ts`)

## Session 271 Amendment (operator decision P; design 15.3.8.11)

This section supersedes all earlier text wherever they conflict, including TASK-003, TASK-201
and the TASK-203 full-vitest row.

### Intent and context

Session 269 blocked acceptance for one reason. The absolute 4,800 ms assertion in
`editor/test/e2e/large-eval.test.ts:53` runs inside the default parallel vitest run, and under
worker contention it passes or fails unpredictably:

- default-worker full runs exited 0, 1 and 1, with 20,000-line medians of 4,924.1 and 5,685.6 ms;
- three `--maxWorkers=1` full runs passed 696/696.

The Rust fix is correct and stays as it is. The focused medians were 811.3 and 2,374.2 ms (ratio
2.93). Against the pre-fix Wasm the medians were 20,566.3 and 141,240.7 ms (ratio 6.87).

The work in this amendment:

1. Move the absolute budget into a perf-only file that runs only through a new serial perf gate,
   `npm run test:perf`.
2. Keep deterministic checks in the default suite.
3. Re-measure the budget.
4. Re-run the negative control.
5. Pass the gates.

### Non-goals (session 271)

- No Rust change. The six Rust writePaths, `src/session/eval.rs` and `src/session/tests/eval.rs`
  stay byte-identical. Check with `git diff --stat 6dc176f -- src`, which must be empty.
- No change to the frontend writePaths `editor/src/code/eval.ts`, `editor/src/code/diagnostics.ts`,
  `editor/src/params/mount.ts` or `editor/src/bind/mount.ts`.
- No edit to `editor/test/e2e/large-doc.test.ts`. TASK-204 is done.
- No edit to EDITCOST files or to any other test file.
- No change to `editor/package-lock.json`, to any dependency, or to the `check`, `test`, `build`,
  `dev` or `e2e` scripts.
- No `it.skip`, `it.skipIf`, `describe.runIf`, environment check or early `return` inside a test
  file. Files are selected by vitest config only.
- No assertion is deleted. The ratio limit is tightened from 8 to 6, never loosened.

### Ownership additions (session 271)

New writePaths (all concrete files):

- `editor/test/e2e/large-eval.perf.test.ts` (new)
- `editor/test/e2e/large-eval-shared.ts` (new)
- `editor/vitest.config.ts`
- `editor/package.json`: the `scripts.test:perf` entry only. CANVAS-EVIDENCE keeps this file as
  a sharedPath for `scripts.e2e` only.

### TASK-301: Perf selection in `editor/vitest.config.ts` and `editor/package.json`

- `editor/vitest.config.ts`:
  - Import `configDefaults` from `vitest/config`.
  - Read the environment with the untyped-global idiom at `editor/vite.config.ts:27-28`:
    `(globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env`.
    `editor/tsconfig.json` has `"types": []`, so a bare `process` fails `npm run check`. Do not add
    `@types/node`, and do not add a `types` entry.
  - Set `const perf = env.VACTR_PERF === '1'`. Only the exact string `'1'` enables perf mode.
  - `test.include`: when perf, `['test/**/*.perf.test.ts']`; otherwise the existing
    `['test/**/*.test.ts', 'test/**/*.test.tsx']`, unchanged.
  - `test.exclude`: when perf, `configDefaults.exclude`; otherwise
    `[...configDefaults.exclude, 'test/**/*.perf.test.ts']`. Setting `exclude` replaces the
    vitest defaults, so always spread `configDefaults.exclude`. Dropping it would collect
    `node_modules` tests.
  - Keep `environment: 'jsdom'`, the `solid()` plugin, `resolve.conditions` and the header
    comment. Add one comment line citing design 15.3.8.11.
- `editor/package.json`: add `"test:perf": "VACTR_PERF=1 vitest run --maxWorkers=1"` right after
  the `"test"` entry. Keep 2-space JSON formatting. Change nothing else.

### TASK-302: Split `large-eval`

- `editor/test/e2e/large-eval-shared.ts` (new). The name must not end in `.test.ts`. Move the
  measurement out of `large-eval.test.ts` with unchanged semantics:
  - the non-literal fixture and helper imports (`large-eval.test.ts:7-10`);
  - `evaluate` (fresh `session_init(48_000, 0)`, one `eval` envelope with `doc_revision: 1` and
    `edit_epoch: 1`, record tag `0x71`, `eval-result` with `re === 1`);
  - `median`;
  - the three-iteration 5,000/20,000 loop with its per-run `expect`s (exactly one `eval-result`
    each, zero `error` diagnostics on the 20,000-line run).

  Pinned exports:

  ```ts
  export const LARGE_EVAL_RATIO_LIMIT = 6;
  export interface LargeEvalMeasurement { smallMedian: number; largeMedian: number; ratio: number }
  export async function measureLargeEval(label: string): Promise<LargeEvalMeasurement>
  ```

  `measureLargeEval` prints exactly one line:
  `` `${label} medians: 5000=<x.x>ms 20000=<y.y>ms ratio=<r.rr>` ``.
- `editor/test/e2e/large-eval.test.ts` (default suite):
  - Keep `// @vitest-environment node`.
  - One test: `measureLargeEval('large-eval node')`, then
    `expect(ratio).toBeLessThanOrEqual(LARGE_EVAL_RATIO_LIMIT)`, with the `120_000` per-test
    timeout.
  - No `LARGE_EVAL_BUDGET_MS`, no other ms constant, and no comparison of an elapsed value with a
    number.
- `editor/test/e2e/large-eval.perf.test.ts` (perf gate only):
  - Start with `// @vitest-environment node`.
  - One test: `measureLargeEval('large-eval perf')`, then assert the ratio is at most
    `LARGE_EVAL_RATIO_LIMIT`, then `expect(largeMedian).toBeLessThanOrEqual(LARGE_EVAL_BUDGET_MS)`,
    with the `120_000` per-test timeout.
  - `LARGE_EVAL_BUDGET_MS` lives here, with a one-line comment citing this plan, the three TASK-304
    medians and `m`.

Pitfalls:

- Running `./node_modules/.bin/vitest run test/e2e/large-eval.perf.test.ts` without
  `VACTR_PERF=1` finds no test file and exits 1. This is expected. Focused perf runs always use
  `npm run test:perf` or
  `VACTR_PERF=1 ./node_modules/.bin/vitest run --maxWorkers=1 test/e2e/large-eval.perf.test.ts`.
- Do not import a `*.test.ts` file from another test file. Import only `large-eval-shared.ts`.
- Do not switch to CPU time, `process.hrtime` or a work counter. The ratio of medians is the
  default-suite guard (design 15.3.8.11).

### TASK-303: Wall-clock audit on the final source

Run each command and record its output in the progress log:

- `grep -rnE "performance\.now\(\)|Date\.now\(\)|hrtime" editor/test --include='*.ts' --include='*.tsx'`
  (quote the globs; zsh expands them otherwise).
  Expected: only `large-eval-shared.ts` measures elapsed time. The other hits are clock injection:
  `test/app/main.test.ts`, and the `Client` `now` in `test/code/sync.test.ts` and
  `test/canvas/{state,edit-cost,input}.test.ts`.
- `grep -nE "BUDGET|_MS\b" editor/test/e2e/large-eval.test.ts`. Expected: no match (grep exit 1).
- `cd editor && ./node_modules/.bin/vitest list --filesOnly`. Expected: no `.perf.test.ts` path,
  and `test/e2e/large-eval.test.ts` is present.
- `cd editor && VACTR_PERF=1 ./node_modules/.bin/vitest list --filesOnly`. Expected: exactly
  `test/e2e/large-eval.perf.test.ts`.

If the first grep finds a new elapsed-time assertion in a default-suite file, stop and report it
for routing (design 15.3.8.11). Do not edit that file.

### TASK-304: Budget re-measurement (design 15.3.8.11 Budget)

1. Rebuild the host-wasm library on the final source (setup command).
2. With no other vitest, nextest, cargo or browser process running, run three times:
   `cd editor && npm run test:perf > ../tmp/canvas-cutover/evidence-runstart/logs/s271-perf-measure-<n>.log 2>&1; echo $? > ../tmp/canvas-cutover/evidence-runstart/logs/s271-perf-measure-<n>.exit`
   for n = 1, 2, 3.
3. These are measurement runs, not gates. Record their exit codes either way.
4. Take `m` = the median of the three printed `20000=` values. The budget is
   `ceil(2 * m / 100) * 100` ms.
5. If the budget equals 4,800, keep the constant and record that it was confirmed. Otherwise set
   `LARGE_EVAL_BUDGET_MS` to the computed value. Never exceed `ceil(2 * m / 100) * 100`.
6. Record the three medians, `m` and the budget in the progress log.

### TASK-305: Negative control (mutationEvidence, never gating)

Use the existing pre-fix scratch build at
`tmp/canvas-cutover/evidence-runstart/scratch/pre-fix/target/wasm32-unknown-unknown/debug/vactr.wasm`.
Note the `pre-fix/` segment; the TASK-202 text omitted it. If the file is absent, rebuild it per
TASK-202 into `scratch/pre-fix`.

Run each command alone. Each takes about 490 s.

- Default file:
  `cd editor && VACTR_WASM="$PWD/../tmp/canvas-cutover/evidence-runstart/scratch/pre-fix/target/wasm32-unknown-unknown/debug/vactr.wasm" timeout 900 ./node_modules/.bin/vitest run test/e2e/large-eval.test.ts > ../tmp/canvas-cutover/evidence-runstart/logs/s271-negative-default.log 2>&1; echo $? > ../tmp/canvas-cutover/evidence-runstart/logs/s271-negative-default.exit`
- Perf file:
  `cd editor && VACTR_PERF=1 VACTR_WASM="<same path>" timeout 900 ./node_modules/.bin/vitest run --maxWorkers=1 test/e2e/large-eval.perf.test.ts > ../tmp/canvas-cutover/evidence-runstart/logs/s271-negative-perf.log 2>&1; echo $? > ../tmp/canvas-cutover/evidence-runstart/logs/s271-negative-perf.exit`

Expected result for each: exit 1. Record which assertion failed (ratio, budget or test timeout).
An outer `timeout` kill (124) is not valid evidence; record it and rerun once.

If the default file passes against pre-fix Wasm (its ratio came out at 6 or below), record that
honestly as a finding for review. Do not change the limit.

Report `{ command, expectedExit: "1", actualExit, failingAssertion, logPath }` per file.

### TASK-306: Gates (inside the sandbox; logs `tmp/canvas-cutover/evidence-runstart/logs/s271-*` with `.exit` files)

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| TASK-303 audit commands | outputs exactly as stated in TASK-303 |
| `cd editor && ./node_modules/.bin/vitest run test/e2e/large-eval.test.ts` | exit 0; the `large-eval node medians:` line is recorded |
| `cd editor && npm run test:perf` (run alone) | exit 0; 1 test; medians at or below the budget |
| `cd editor && ./node_modules/.bin/vitest run`, three consecutive times (`s271-vitest-full-1..3`) | each exits 0 with at least 696 passed and 0 failed. Record the `ratio=` value from each run's `large-eval node medians:` line (step-3 review finding). |
| `git diff --exit-code 6dc176f -- editor/package-lock.json src` | exit 0 |
| `rustfmt --check src/ns/evaluator.rs src/ns/eval_doc.rs src/ns/journal.rs src/ns/depgraph.rs src/ns/insts.rs src/compile/compiler.rs` | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 |

Outside the sandbox (verification step, authoritative):

- three consecutive default `./node_modules/.bin/vitest run`, each exit 0;
- `npm run test:perf` alone, exit 0;
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run > tmp/canvas-cutover/evidence-runstart/logs/s271-nextest-full.log 2>&1`,
  exit 0. A timeout kill is neither a pass nor a failure.

### Invariants (session 271)

- Eval semantics, diagnostics, the wire format and Rust source are unchanged.
- The default suite contains no absolute ms budget. The perf gate contains every assertion the
  default file has, plus the budget.
- The test count of the default full suite does not drop: the default `large-eval.test.ts` still
  contributes one test.

### Completion criteria (session 271)

- [ ] TASK-301: `vitest.config.ts` perf/default selection and the `test:perf` script are in place;
  `npm run check` exits 0; `package-lock.json` is unchanged
- [ ] TASK-302: `large-eval-shared.ts`, the ratio-only default file (limit 6) and the perf file with
  `LARGE_EVAL_BUDGET_MS` exist; no assertion is deleted
- [ ] TASK-303: audit outputs recorded; the `vitest list` default and perf sets are disjoint
- [ ] TASK-304: three serial perf medians, `m` and the budget recorded
- [ ] TASK-305: both negative-control runs recorded with exit code, failing assertion and log path
  (not gating)
- [ ] TASK-306: all gates exit 0 with log paths; three default full runs and their ratios recorded

## Completion Criteria

- [x] Hot spot located with recorded timings and CPU profiles
- [x] Fix applied in the owning files; escalated paths recorded as sharedPaths
- [x] `large-eval.test.ts` passes on final source: focused 5,000-line median 811.3 ms, 20,000-line median 2,374.2 ms, ratio 2.93, one eval-result and zero error diagnostics (`logs/s269e-large-eval-focused.log`); separate three-run budget sample is recorded under TASK-201
- [x] Controlled pre-fix mutation failure: scratch build exit 0; Vitest exit 1 on the budget assertion at 141,240.7 ms (`logs/s269-mutation-prefix.log`)
- [x] `npm run check`, touched-file rustfmt, strict clippy, focused nextest and full nextest pass; full nextest passed 2,810 tests with 3 skipped
- [x] Full Vitest passes three consecutive times with `--maxWorkers=1` (696/696 each; `logs/s269f-vitest-full-1..3.log`)
- [ ] The plan-listed default `cd editor && ./node_modules/.bin/vitest run` gate passes three consecutive times. After TASK-204 it passed once, then exceeded the 4,800 ms eval budget at 4,924.1 and 5,685.6 ms (`logs/s269e-vitest-full-1..3.log`). Session 271: this is now met through the TASK-306 gates once the budget has moved to the perf gate.
- [ ] Session 271: `npm run test:perf` passes alone on the final source

## Progress Log

### Session: 2026-10-05 (session 267 implementation)
**Tasks Completed**: Reproduced an unbounded host-wasm eval stall; profile localized repeated checker namespace BTreeMap reconstruction in `src/ns/evaluator.rs` via `Namespace::check_env` and `checked_inputs`. The follow-up source inspection found `Snapshot::take` copied every bound session slot and `DepGraph::dependents` scanned every prior form before each form completed. Recorded the explicit plan escalation to `src/ns/evaluator.rs`, `src/ns/eval_doc.rs`, and `src/ns/journal.rs` before source edits. The pre-fix benchmark was interrupted after 5.5 minutes at 100% CPU (log under `tmp/canvas-cutover/evidence-runstart/logs/large-eval-diagnosis.log`, exit 130); the sampled Wasm worker profile is `tmp/canvas-cutover/evidence-runstart/logs/CPU.20261005.133929.80426.1.002.cpuprofile` (39,980 samples).
**Follow-up**: After the checker, snapshot and reverse-reader fixes, the measured 5k/20k medians remained 12,874.1/59,633.0 ms (ratio 4.63), exceeding the 5,000 ms 20k limit. Source tracing found another per-form full scan in `DepGraph::form_of_gen`, called by `Session::note_revision`; this authorized `depgraph.rs` escalation was recorded before changing source. Implementing a generation index and keeping it synchronized on journal commits.
**Profile follow-up**: The generation-index revision still measured 5,000/20,000 medians 14,449.9/64,704.3 ms (ratio 4.48; `large-eval-final.log`). A fresh worker profile (`CPU.20261005.143324.16025.1.002.cpuprofile`, 49.75 sampled seconds) located repeated full `Snapshot::take` (14.79 s) and `dynamic_manifest` control-set construction (18.60 s) per-form costs. Adding the compiler-emitted `Force` to the bounded constant-definition proof and caching dynamic manifests by base and current control names.
**Control-set follow-up**: After those changes, medians were 3,537.9/9,761.0 ms (ratio 2.76). The next profile (`CPU.20261005.144743.26560.1.002.cpuprofile`, 19.76 sampled seconds) showed controls being collected and cloned repeatedly; cache a deduplicated `Rc` set at instrument install/replacement and share it into manifests and compiler contexts.
**Check-result follow-up**: Registry-owned control caching reduced the medians to 2,171.7/5,747.8 ms (ratio 2.65; current log `large-eval-final.log`). `eval_form_in` still performs the default check to compute its diagnostic split, then invokes `eval_form` which repeats that same check. Reuse that existing default `CheckResult` in the evaluator execution path and perform only the custom-manifest check separately.
**Final-source verification**: `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`, `rustfmt --check` on all six touched Rust files, and `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` passed. Focused `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run session::` passed 149 tests (2,664 skipped). `cd editor && npm run check` passed. Focused `large-eval.test.ts` passed one test at 811.7/2,559.3 ms (ratio 3.15), and focused `large-doc.test.ts` passed both tests in 1.99 s.
**Shared-suite disposition**: `cd editor && ./node_modules/.bin/vitest run` exited 1 with 683 passed and 5 failed. Three failures are in parallel-plan `test/canvas/edit-cost.test.ts` and `test/canvas/mount.test.ts`; the full-suite `test/e2e/large-doc.test.ts` timed out at its existing 5 s limit but passed in isolation; this plan's benchmark measured 5,033.5 ms under suite load but passed in isolation at 2,559.3 ms. Full Vitest remains unresolved for serial integration. The pre-edit stall run was manually interrupted after 5.5 minutes (exit 130), so controlled mutation evidence remains outstanding.

### Session: 2026-10-05 (session 269 plan amendment)
**Tasks Completed**: Applied operator authorization 2 and design 15.3.8.10. The six session-267 Rust files are now writePaths with their escalation notes kept. Added the scratch artifact root, the conditional `editor/test/e2e/large-doc.test.ts` sharedPath (DR-269-L1) and tasks TASK-201 (2x focused-median budget), TASK-202 (pre-fix scratch mutation run), TASK-203 (gates: three consecutive full vitest, rustfmt on the six files, strict clippy, focused and outside-sandbox full nextest) and TASK-204 (bounded `large-doc.test.ts` timeout rule). The manifest entry changed with it.

### Session: 2026-10-05 (session 269 implementation gates)
**TASK-201**: On the final measurement logic, three focused runs printed 20,000-line medians 2,372.8, 2,366.7, and 2,382.9 ms (5,000-line values 813.0, 810.7, and 811.7 ms). Thus `m=2,372.8` and the exact budget is `ceil(2*m/100)*100=4,800` ms. `large-eval.test.ts` keeps the <=8 ratio, one result, zero error diagnostics, and 120,000 ms test timeout. The final focused check printed 811.3/2,374.2 ms and passed (`logs/s269e-large-eval-focused.log`).
**TASK-202**: Built the 968028c pre-fix source in `scratch/pre-fix` (exit 0; `logs/s269-mutation-build.log`). The controlled run exited 1 on the budget assertion with medians 20,566.3/141,240.7 ms and ratio 6.87 (`logs/s269-mutation-prefix.log`, 488.38 s); it was not killed by the 900-second outer timeout.
**TASK-203 focused and Rust gates**: The final host-wasm build passed (`logs/s269-wasm-build.log`); rustfmt check on all six Rust files, strict clippy, and focused nextest (283 passed, 2,530 skipped) passed (`logs/s269-rustfmt-check.log`, `s269-clippy.log`, `s269-nextest-focused.log`). `npm run check` passed after the TypeScript edits (`logs/s269f-npm-check.log`). The post-TASK-204 focused large-doc suite passed both tests in 1.95 s (`logs/s269f-large-doc-focused.log`).
**TASK-203 full gates**: Full nextest exited 0: 2,810 passed and 3 skipped in 1,002.044 s (`logs/s269-nextest-full.log`). Default-parallel Vitest attempts are preserved in `s269-vitest-full-*`, `s269b-vitest-full-*`, `s269c-vitest-full-*`, `s269d-vitest-full-*`, and `s269e-vitest-full-*`; after TASK-204, one such run passed and two hit the eval wall-clock budget under worker contention (`s269e-vitest-full-1..3.log`). The joined tree passes three consecutive full runs with supported `--maxWorkers=1`, each 696/696 in 66.79, 68.05, and 66.32 s (`logs/s269f-vitest-full-1..3.log`). This is alternate full-suite evidence, but the plan-listed default command remains unresolved pending acceptance that the worker cap is valid for TASK-203. No test assertion was removed or changed.
**TASK-204**: The default large-doc timeout failed in `logs/s269d-vitest-full-1.log` and `-3.log`; the post-join focused run passed in 1.99 s (`logs/s269d-large-doc-focused.log`). Added only a 60,000 ms per-test timeout and a plan citation. The pre-edit SHA-256 was `144a59a394e90b824db34df5efdc1c99dc2d311874a6c50d58fd0b706bc42516`; final SHA-256 is recorded in `tmp/canvas-cutover/evidence-runstart/receipt.json`.

### Session: 2026-10-05 (session 271 plan amendment)
**Tasks Completed**: Applied operator decision P and design 15.3.8.11. Added four writePaths: `editor/test/e2e/large-eval.perf.test.ts`, `editor/test/e2e/large-eval-shared.ts`, `editor/vitest.config.ts`, and `editor/package.json` (only the `scripts.test:perf` entry). Added these tasks:

- TASK-301: `VACTR_PERF` include/exclude selection and the `test:perf` script.
- TASK-302: split `large-eval` into a ratio-only default file (limit 6) and a perf file holding `LARGE_EVAL_BUDGET_MS`.
- TASK-303: wall-clock audit.
- TASK-304: re-measure the serial budget.
- TASK-305: negative control against the pre-fix Wasm at `scratch/pre-fix`.
- TASK-306: gates.

No Rust or product source change. CANVAS-EVIDENCE-EDITCOST now depends on this plan, so the perf gate runs alone. The manifest entry was changed with this amendment.
