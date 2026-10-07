# DOM-RENDERER-EVIDENCE: DOM Behavior Run, Canvas-vs-DOM Measurement Matrix, Report and Gates Implementation Plan

**Status**: Ready (starts after DOM-RENDERER-HARNESS is accepted)
**Plan ID**: DOM-RENDERER-EVIDENCE (wave 4 of 4, serial chain; final)
**Design Reference**: `design-docs/specs/design-dom-renderer.md` DR-9 (DOM behavior), DR-10 (comparison protocol and report) and DR-12 (gates)
**Depends On**: DOM-RENDERER-HARNESS (`impl-plans/active/dom-renderer-harness.md`), and transitively CORE and MOUNT
**Created**: 2026-10-07
**Last Updated**: 2026-10-07

---

## Intent and Context

The user asked for a DOM version of the editor and a measured performance comparison with the
canvas version. This plan produces the evidence:

- a DOM behavior pass in Chromium and WebKit;
- 24 measured runs: {canvas, dom} x {chromium, webkit} x {1,000, 20,000 lines} x 3;
- raw data under `design-docs/specs/evidence/renderer-comparison/rc-001/`;
- the report `design-docs/specs/design-renderer-comparison.md` with an honest analysis;
- the final gates.

The operator will use the report later to write a GPUI-inspired canvas tuning proposal. This
plan records observations only. It writes no proposal.

**Sandbox boundary.** The Codex sandbox cannot bind localhost and keeps `.git` read-only.
Builds that need no localhost can run in the sandbox. Every browser run, `test:style`, the full
nextest, and commit and push happen outside the sandbox, in the verification, review and final
integration steps. The work is split in two:

1. **Iteration 1 (implementer, sandbox):** `--write-report` from the (still empty) raw
   directory, creating the report skeleton, plus the in-sandbox gates.
2. **Verification 1 (outside):** builds, behavior and the locked measurement matrix.
3. **Iteration 2 (implementer, sandbox):** regenerate the tables and write the analysis from the
   data.
4. **Final gates (outside),** then commit and non-force push by the final integration step.

## Non-goals

- No threshold, workload, timing constant or harness rule changes. No best-of selection and no
  rerun-until-pass. A failing gate is a result.
- No canvas tuning and no tuning proposal text.
- No edit to `editor/src/code/layout.ts`, `renderer.ts`, `frame.ts`, `view-host.ts`,
  `run.mjs`, `behavior.mjs`, `stats.mjs` or the fixture.
- No archiving of plans. Archiving is done at final reconciliation by the integration step.
- No mutation or negative-control commands.

## Ownership

writePaths:

- `design-docs/specs/design-renderer-comparison.md` (new: generated tables plus the written
  analysis)
- `impl-plans/active/dom-renderer-evidence.md` (checkboxes and progress log only)
- `design-docs/specs/evidence/renderer-comparison/rc-001` (artifact root: raw data written by
  `compare.mjs`; committed)
- `tmp/dom-renderer/evidence` (artifact root: logs)
- `target` (artifact root: wasm builds and nextest)
- `editor/dist` (artifact root: frontend build)
- `editor/node_modules/.vite` (artifact root: vitest cache)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root: rebuilt only if missing via
  `mise run ts-build-wasm`)

sharedPaths (repair only when an e2e or gate failure proves a defect in that file; record the
reason and the failing evidence in the progress log):

- `editor/src/code/dom-renderer.ts`
- `editor/src/code/dom-overlay.ts`
- `editor/src/code/dom-renderer.css`
- `editor/test/code/dom-renderer.test.ts`
- `editor/src/code/mount.ts` (seam lines only)
- `editor/src/code/perf-hook.ts`
- `editor/test/e2e/behavior-dom.mjs`
- `editor/test/e2e/compare.mjs`
- `editor/test/e2e/compare-stats.mjs`
- `editor/test/e2e/compare-stats.test.ts`
- `impl-plans/README.md` (add one index entry for the four dom-renderer plans)

## Measurement lock (mandatory for every browser command, `test:style` and the full nextest)

Wrap each locked command exactly like this. Run it in the foreground and release the lock
immediately after:

```
bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done; trap "rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock" EXIT; echo dom-renderer-evidence-s296 > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; <COMMAND>'
```

Never hold the lock while idle or across two cells. Never run vitest, builds or any other heavy
suite while a locked measurement runs.

## Execution Steps

### TASK-E1 (iteration 1, sandbox): Report skeleton and README index
- From `editor/`, run
  `node test/e2e/compare.mjs --run-id rc-001 --write-report > ../tmp/dom-renderer/evidence/write-report-1.log 2>&1`
  -> exit 0. This creates `design-docs/specs/design-renderer-comparison.md` from
  `REPORT_SKELETON`, with `unavailable` tables.
- Add an `impl-plans/README.md` entry: "DOM renderer comparison backend (design
  `design-dom-renderer.md`), serial plans core -> mount -> harness -> evidence", with links.
- [ ] The report exists with both markers. The README entry is added.

### TASK-E2 (outside the sandbox): Builds
Run these serially, from the repository root unless noted:

1. `mise run build-wasm-release > tmp/dom-renderer/evidence/wasm-release.log 2>&1` -> exit 0.
2. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build > ../tmp/dom-renderer/evidence/dist-build.log 2>&1`
   -> exit 0.

### TASK-E3 (outside, locked): Behavior
1. DOM (gating):
   `cd editor && node test/e2e/compare.mjs --run-id rc-001 --behavior --renderers dom > ../tmp/dom-renderer/evidence/behavior-dom.log 2>&1`
   inside the lock wrapper -> exit 0, `failureCount 0`, `testsRun > 0`, in both browsers.
2. Canvas regression reference:
   `cd editor && node test/e2e/compare.mjs --run-id rc-001 --behavior --renderers canvas > ../tmp/dom-renderer/evidence/behavior-canvas.log 2>&1`
   inside the lock wrapper.
   - If it is non-zero, inspect the failing check ids. A failure in a check that exercises the
     seam (`mount.ts`, `perf-hook.ts`) is a defect: fix it and rerun.
   - Otherwise, record it in the report's Limitations section as pre-existing canvas state for
     the `wf/canvas` owner, quoting the check id and detail. Do not list it as a verification
     record.
3. **`dom-caret-alignment` failure:** if the fix lies in the DOM renderer files, fix it there.
   If it needs `layout.ts`, stop and report a scope-amendment blocker with the measured deltas.
   Do not edit `layout.ts`.

### TASK-E4 (outside, locked): Measurement matrix
Run four invocations, each in its own lock wrapper, from `editor/`, in this order:

1. `node test/e2e/compare.mjs --run-id rc-001 --resume --lines 1000 --browsers chromium > ../tmp/dom-renderer/evidence/cell-1000-chromium.log 2>&1`
2. `node test/e2e/compare.mjs --run-id rc-001 --resume --lines 1000 --browsers webkit > ../tmp/dom-renderer/evidence/cell-1000-webkit.log 2>&1`
3. `node test/e2e/compare.mjs --run-id rc-001 --resume --lines 20000 --browsers chromium > ../tmp/dom-renderer/evidence/cell-20000-chromium.log 2>&1`
4. `node test/e2e/compare.mjs --run-id rc-001 --resume --lines 20000 --browsers webkit > ../tmp/dom-renderer/evidence/cell-20000-webkit.log 2>&1`

Each must exit 0 (every run key complete). Gate failures are allowed and recorded. If a key
stays incomplete after its single retry, rerun only that cell with `--resume` once. Both
attempts remain in `attempts.json`.

Expected wall clock is about 25-30 min per cell. Post-sink silence (peak 0) is enforced inside
`runMeasurement` for every run.

- [ ] `rc-001/runs/` has 24 complete `.json` files plus `.jsonl` files, and `environment.json`,
      `attempts.json`, `comparison.json` and `behavior/` exist.

### TASK-E5 (iteration 2, sandbox): Report
1. `cd editor && node test/e2e/compare.mjs --run-id rc-001 --write-report > ../tmp/dom-renderer/evidence/write-report-2.log 2>&1`
   -> exit 0. The tables now hold the measured values.
2. Write the three implementer sections outside the markers, from `comparison.json`, the
   per-run `rendererStats`, the `rawMetrics.phaseMs` and the node counts. Every claim cites a
   number from `rc-001`.
   - **Analysis.** Where each renderer wins and why, per browser and document size. It must
     cover:
     1. End-to-end comparators (input latency, frame interval) versus JS-only work metrics. DOM
        style, layout and paint run after the rAF callback and are not in `textWork` or
        `animationWork`; canvas GPU execution is not either.
     2. The typing cost: DOM `lineBuilds` per edit versus canvas `textBuilds` and geometry
        uploads.
     3. Scroll and window shifts: `windowShifts`, `transformWrites`.
     4. The animation path: `highlightToggles` and `highlightBuilds` versus canvas quads.
     5. Memory: heap growth (Chromium only), DOM node peak and the GPU ledger (DOM 0 by
        construction).
     6. First viewport.
     7. 1,000- versus 20,000-line sensitivity.
     8. WebKit versus Chromium.
     9. Run-to-run variance and contended runs.
     State gate failures plainly, with the threshold, for example "DOM WebKit 20,000-line
     frame p95 = X ms exceeds 20 ms in 3/3 runs".
   - **Limitations.**
     - WebKit heap unavailable.
     - Synthetic WebKit touch, IME and clipboard.
     - The WebKit headless or headed mode actually used.
     - `loadToViewportMs` includes Playwright transfer.
     - Contended runs.
     - Any canvas reference behavior failure from TASK-E3.
   - **Observations for the canvas tuning proposal.** Facts only: which DOM mechanisms (for
     example, transform-only scroll, keyed line reuse, class-toggle highlights) beat canvas in
     which metric, and by how much. Write no recommendations.
3. Do not edit the generated region by hand.
- [ ] The report has measured tables, the three written sections, and no `Pending measured
      data.` line.

### TASK-E6 (final gates; sandbox items by the implementer, locked items outside)
Run serially, one heavy suite at a time:

1. `cargo build --lib --target wasm32-unknown-unknown > tmp/dom-renderer/evidence/wasm-debug.log 2>&1`
   -> exit 0.
2. `cd editor && npm run check > ../tmp/dom-renderer/evidence/check.log 2>&1` -> exit 0.
3. `cd editor && ./node_modules/.bin/vitest run > ../tmp/dom-renderer/evidence/vitest-full.log 2>&1`
   -> exit 0. The default suite includes the dom-renderer, dom-mount and compare-stats tests.
4. `cd editor && npm run test:perf > ../tmp/dom-renderer/evidence/test-perf.log 2>&1` -> exit 0.
5. Locked: `cd editor && npm run test:style > ../tmp/dom-renderer/evidence/test-style.log 2>&1`
   -> exit 0.
6. `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/dom-renderer/evidence/clippy.log 2>&1`
   -> exit 0.
7. Locked, alone, timeout of at least 1500 s:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run > tmp/dom-renderer/evidence/nextest.log 2>&1`
   -> exit 0.
8. `mise run build-wasm-release > tmp/dom-renderer/evidence/wasm-release-final.log 2>&1` ->
   exit 0.
9. `git diff --stat ec34f66 -- editor/src/code/renderer.ts editor/src/code/layout.ts editor/src/code/frame.ts editor/test/e2e/run.mjs editor/test/e2e/behavior.mjs editor/test/e2e/stats.mjs editor/test/e2e/fixtures/large-doc.mjs`
   -> empty. Canvas code and the shared harness are untouched.

If a gate fails, fix it within writePaths or sharedPaths and rerun. Report only the final
passing run.

### TASK-E7 (final integration step, outside the sandbox): Commit and push
- Stage only the files changed by the four dom-renderer plans, the design docs, the report,
  `rc-001` and the README.
- Commit with a structured message per `AGENTS.md`, with no tool attribution.
- Then `git push origin wf/dom-editor` (non-force). Never push to main and never open a PR.

## Key Points

- **Honesty.** Every completed run appears in the report. Unavailable values appear as
  `unavailable`. Retries appear in attempts. No threshold is changed, and DOM is gated exactly
  as canvas.
- **Silence.** Every run uses the silent sink (and Chromium `--mute-audio`). Never change the
  macOS volume. A sink violation fails the run and must be investigated, not suppressed.
- **Lock discipline.** One lock wrapper per command. Release it between cells.
- **Raw data size.** Check the size of `rc-001` (`du -sh`) and record it in the progress log.
  The design budget is 20 MiB. `compare.mjs` raises the downsampling stride automatically.
- **Product defects.** If a behavior check fails because of the product, fix it in the
  sharedPaths. The fix must not change canvas mode. Add or adjust a unit test in
  `dom-renderer.test.ts` that reproduces the failure deterministically.

## Verification Record Requirements

Use the mandatory record format `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0,
outcome: "passed", log}` for every gating command above, with timings, gate pass counts per cell
and the raw data size in `notes`.

- Measurement-cell records use `testsRun` = keys attempted and `testsPassed` = keys completed.
- Behavior records use the check counts from the JSON line.
- The canvas reference behavior result is described in `notes` and the report, not as a
  verification record.

Do not run mutation or negative-control commands.

## Completion Criteria

- [ ] DOM behavior passes in Chromium and WebKit (TASK-E3.1).
- [ ] 24 complete runs exist with raw data under `rc-001` (TASK-E4).
- [ ] The report is complete, with tables and analysis (TASK-E5).
- [ ] Gates 1-9 pass (TASK-E6).
- [ ] The commit is pushed non-force to `origin wf/dom-editor` (TASK-E7, final integration
      step).

## Progress Log

### Session: (implementer fills in)
**Tasks Completed**:
**Verification**:
**Notes**:
