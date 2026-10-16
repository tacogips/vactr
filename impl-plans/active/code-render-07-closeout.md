# Code Render 07: Calibration, Gates, Mutation Evidence, Before/After Screenshots

**Status**: In Progress
**Plan ID**: CR-07
**Wave**: 3 (depends on CR-01, CR-02, CR-03, CR-04, CR-05, CR-06)
**Design Reference**: design-docs/specs/design-code-pane-text-rendering.md sections 5.2 (calibration), 5.3, 5.5, 6; design-docs/specs/design-canvas-editor-evidence.md "Code-pane text rendering gap (2026-10-16)"
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

## Intent and Context

Run every acceptance gate serially on the integrated tree. Prove the new
tests fail on ea51a07 (mutationEvidence). Record before/after
screenshots. Record the results in the evidence document. A sibling run
(`wf/vim-mode`) is active in another worktree, so every full vitest and
every browser run holds the measurement lock. Silent audio only; never
change system volume.

## Non-goals

- No feature code. If a gate fails, identify the owning plan (CR-01 to
  CR-06) and file, record the failing log, and stop with the failure
  reported. Do not fix product code here. The only code edit allowed is
  the P3/P4 calibration constants in `editor/test/e2e/code-pixels.mjs`
  (see step 3).
- Do not commit, push, or move plans to `impl-plans/completed/`; the
  workflow's final integration owns that. Do not regenerate the canonical
  `run-001` evidence (`--write-evidence` is not used).
- Do not loosen any threshold beyond the calibration rule in CR-06.

## Files

| Path | Change |
| --- | --- |
| `editor/test/e2e/code-pixels.mjs` | Calibration constants only (P3 `extentPx`, P4 `ratio`), within the CR-06 rule. |
| `design-docs/specs/design-canvas-editor-evidence.md` | Append a "Results" paragraph at the end of the "Code-pane text rendering gap (2026-10-16)" section: the gate table (command, exit, counts, log path) and the mutationEvidence summary. Do not edit inside `<!-- EVIDENCE:BEGIN -->`/`<!-- EVIDENCE:END -->`. |
| `impl-plans/active/code-render-07-closeout.md` | Progress log with verification and mutation records. |
| `impl-plans/active/code-render-06-pixel-test.md` | sharedPath: append the calibration record (old value, new value, observed range, log path) to its Progress Log only. |

## Write Scope

Mirrors `impl-plans/active/code-render-dispatch.json` plans[CR-07].

- writePaths: `design-docs/specs/design-canvas-editor-evidence.md`, `impl-plans/active/code-render-07-closeout.md`, `tmp/code-render/CR-07`, `tmp/code-render/baseline`, `tmp/code-render/after`, `tmp/code-render/node_modules`, `editor/dist`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`, `tmp/canvas-cutover/evidence`
- sharedPaths: `editor/test/e2e/code-pixels.mjs` (only the P3 `extentPx` and P4 `ratio` constants, within the CR-06 calibration rule), `impl-plans/active/code-render-06-pixel-test.md` (append the calibration record to its Progress Log only)
- artifactRoots: `tmp/code-render/CR-07`, `tmp/code-render/baseline`, `tmp/code-render/after`, `tmp/code-render/node_modules`, `editor/dist`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`, `tmp/canvas-cutover/evidence` (`target`, the grammar wasm and `editor/node_modules` are prebuilt read-only inputs: never rebuild, reinstall or delete them; `tmp/canvas-cutover/evidence` is the diagnostic output directory written by the pre-existing behavior.mjs:120 WebKit touch diagnostic and measure.mjs:24 during the required behavior e2e gate. CR-07 owns it only as an artifact root and must not delete the existing s274-webkit-touch-events.json; added by the owner for integration-review finding CR07-ARTIFACT-ROOT)
- The before-screenshots and `tmp/code-render/repro-shot.mjs` sit directly in `tmp/code-render/`. They are read-only inputs owned by no plan; never modify or delete them.
- Toolchain audit re-check (step 0 below): CR-07 recomputes the digests and requires equality with every plan's recorded digests.

Evidence goes under `tmp/code-render/CR-07/` (gitignored). The baseline
snapshot goes under `tmp/code-render/baseline/`, after-screenshots under
`tmp/code-render/after/`.

## Steps

Run the commands in order, in the foreground. Paths are relative to the
repository root unless the step says `cd editor`. Every log ends with
`exit=<status>`.

0. Toolchain audit re-check: `shasum -a 256 target/wasm32-unknown-unknown/debug/vactr.wasm tree-sitter-vact/tree-sitter-vact.wasm editor/package-lock.json > tmp/code-render/CR-07/toolchain-sha256.txt`.
   The three digests must equal those recorded in every CR-01 to CR-06
   Progress Log (`toolchainAudit` in `impl-plans/active/code-render-dispatch.json`).
   Record any mismatch as a finding.
1. Preconditions: CR-01 to CR-06 progress logs show their completion
   criteria checked. `git status --short` shows changes only in the plans'
   writePaths.
2. Build the fixed app: `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build > ../tmp/code-render/CR-07/build.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-07/build.log`
   -> exit 0. The release wasm does not exist on this host; the debug
   wasm is the declared frontend artifact.
3. Calibrate (CR-06 rule). Under the lock, run
   `cd editor && node test/e2e/code-pixels.mjs --backend all --dpr all --out ../tmp/code-render/CR-07/calibration`.
   If only P3 or P4 fail and the observed values are inside the allowed
   bounds, set the constants, record old value, new value and observed
   range in this progress log and in CR-06's, and re-run. Any P1, P2 or
   P5-P8 failure is a product failure: report it, naming the owning plan.
4. Gates (each lock-held step: `mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`
   before, `rmdir` after; if the directory exists, wait 30 s and retry;
   never remove a lock you did not create):
   - `cd editor && npm run check` -> exit 0
   - `cd editor && npx vitest run test/code/text-cells.test.ts test/code/atlas-cells.test.ts test/code/glyph-placement.test.ts test/code/renderer-layers.test.ts test/code/perf-hook-annotations.test.ts test/visual/panes.test.ts test/e2e/pixel-compare.test.ts` -> exit 0
   - lock: `cd editor && npx vitest run` (full) -> exit 0, record the counts
   - lock: `cd editor && npm run test:style` -> exit 0 (4/4 engine-viewport combinations)
   - lock: `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run e2e -- --browser all --profile behavior` -> exit 0. run.mjs starts its own server on a free 127.0.0.1 port. The Chromium checks must include `code-pixels-dpr1` and `code-pixels-dpr2` as passed. The measurement profile is not part of this issue's acceptance and is not run.
   - lock: `cd editor && node test/e2e/code-pixels.mjs --backend all --dpr all --out ../tmp/code-render/CR-07/pixels-after` -> exit 0; 4 scenarios x 2 DPR x 2 backends, all P1-P8 passing, renderer strings recorded.
   - `wc -l` on every touched source and test file -> each below 1000.
   - `git diff ea51a07 -- editor/src/code/input.ts editor/src/code/keyboard.ts` -> empty. The `render()` cursor block in `editor/src/code/renderer.ts` is byte-identical to ea51a07 (compare the block text from `git show ea51a07:editor/src/code/renderer.ts`).
5. Baseline snapshot for mutation evidence (serial, read-only git):
   - `mkdir -p tmp/code-render/baseline && git archive ea51a07 editor | tar -x -C tmp/code-render/baseline`
   - Symlinks: `tmp/code-render/baseline/editor/node_modules` -> the
     repository `editor/node_modules`; `tmp/code-render/baseline/target`
     -> the repository `target`; `tmp/code-render/baseline/tree-sitter-vact`
     -> the repository `tree-sitter-vact`.
   - Copy the new test files (and `editor/test/code/recording-gl.ts`) into
     the same relative paths under `tmp/code-render/baseline/editor/test/`.
   - `cd tmp/code-render/baseline/editor && npx vitest run <the copied test files>`
     -> expected non-zero exit. Record per file which cases fail: U1
     text-cells, U2 atlas width/filter, U3 renderer slots/caret/diagnostic/backdrop/document-1,
     U6 visual context options, and import failures for U4/U5 (missing
     `glyphPhase`/`SNAP_GLYPH`) and `annotations()`.
   - `cd tmp/code-render/baseline/editor && VACTR_WASM=<absolute path to target/wasm32-unknown-unknown/debug/vactr.wasm> npx vite build`
     -> builds the old app into `tmp/code-render/baseline/editor/dist`.
   - lock: `cd editor && node test/e2e/code-pixels.mjs --dist ../tmp/code-render/baseline/editor/dist --backend all --dpr all --out ../tmp/code-render/CR-07/pixels-baseline`
     -> expected exit 1. Required failing checks on the old code: P1
     (S2, S3, S4), P3 (S1, the `1` in `108`), P5 (S3), P6 (S2 or S3), P7
     (S3 or S4), and P8 (blit error). Record any of them that does not fail,
     with its measured values, as a finding. Do not tune thresholds to
     force a failure.
   These runs go only in mutationEvidence, never in the gating list.
6. Before/after screenshots:
   - `ln -s ../../editor/node_modules tmp/code-render/node_modules` (so
     `tmp/code-render/repro-shot.mjs` can resolve `playwright`; skip if
     it exists).
   - lock: from `editor/`, run an inline foreground wrapper that starts
     `startServer()` from `test/e2e/serve.mjs`, runs
     `node ../tmp/code-render/repro-shot.mjs ../tmp/code-render/after <origin>/`
     with `execFileSync`, and closes the server in `finally`. Output:
     `tmp/code-render/after/t-typed-code.png`, `t-typed-two.png`,
     `t-inserted.png`.
   - Before set: the existing `tmp/code-render/t-typed-code.png`,
     `t-typed-two.png`, `t-scroll-redraw.png` (the 3-line insert) and
     `code-gpu2x.png` (kalimba).
   - Review finding DR-L2: also compare the translucent palette quads.
     The pixel-test captures in `tmp/code-render/CR-07/pixels-after` and
     `pixels-baseline` show the diagnostic underline (`--vt-danger`). Add
     one selection capture before and after: a small script under
     `tmp/code-render/CR-07/selection-shot.mjs` (untracked) that inserts
     the S3 text, presses `Shift+ArrowUp` twice, and screenshots the
     canvas at DPR 2 against the fixed and the baseline dist. Name every
     before/after pair in the progress log, and state whether the
     selection and underline composites look unchanged (expected for the
     converted literals) or changed (palette colors, now CSS-correct).
7. Append the results paragraph to `design-canvas-editor-evidence.md`.

## Key Points and Pitfalls

- Hold the lock for the whole command, then remove it. Never run two
  lock-held commands at the same time.
- Never use ports 5173 or 5174; all servers here are `startServer` on
  port 0.
- A missing or truncated log, or a blocked run (exit 2), is not a pass.
- The baseline tree is a read-only copy under `tmp/`. Never run git
  write commands, stash, checkout or worktree.

## Verification Records

Every gating command above is recorded as `{command, exitStatus: 0,
testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
Step 5 runs are recorded as `mutationEvidence` entries `{command,
exitStatus, failingChecks, log, notes}`.

## Completion Criteria

- [x] Calibration recorded: defaults kept; the matrix exposed P3/P6 failures requiring upstream disposition.
- [ ] check, focused vitest, full vitest, test:style, build, e2e behavior (Chromium + WebKit, with code-pixels-dpr1/dpr2) and the full pixel matrix pass
- [ ] Mutation evidence shows the required old-code failures
- [ ] Before/after screenshots, including the DR-L2 selection pair, named in the progress log
- [ ] Line counts below 1000; input.ts, keyboard.ts and the cursor block untouched
- [x] Results appended to design-canvas-editor-evidence.md

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Runs after all other code-render plans.

### Session: 2026-10-17 (CR-07 attempt 1)
**Tasks Completed**: Toolchain digest re-check; debug-WASM app build; `npm run check`; focused CR-07 vitest suite; calibration matrix captured.
**Tasks In Progress**: Remaining full gates, baseline mutation evidence and before/after screenshots are stopped pending upstream P3/P6 disposition.
**Blockers**: Calibration command failed on the combined tree. DPR-2 P3 character extents differed by up to 3 device pixels (S4, versus the permitted maximum of 2), so the CR-06 calibration rule does not permit changing the threshold. P6 failed with no diagnostic in S2 at DPR 1 and empty underline rows for some S4 diagnostics at DPR 1/2. P3 pixel-width mismatch is assigned for CR-04/CR-02 investigation; P6 diagnostic behavior and/or assertion is assigned for CR-05/CR-06 investigation. No product code is changed by CR-07.
**Notes**: Toolchain SHA-256 values match the CR-01–CR-06 progress records: `vactr.wasm` `de9c55f37b4037d73e28f8aaa11e06d3bc87a3a41a3bc95dd8e64ebf02c93210`, `tree-sitter-vact.wasm` `ae3e5cdf1217e943f19c3ff15f111801e537ba9b7e463f84d6bf8cedd7fd0609`, `package-lock.json` `f32e9585c409487e0d74ee9b8dc1880d4e54ff8610ab2cefa529828c53449749` (`tmp/code-render/CR-07/toolchain-sha256.txt`). `npm run build` exit 0 (`build.log`); `npm run check` exit 0 (`check.log`); focused suite 7 files / 34 tests passed (`focused.log`). Calibration command `node test/e2e/code-pixels.mjs --backend all --dpr all --out ../tmp/code-render/CR-07/calibration` exited 1: four backend/DPR rows, zero passed. Full details and screenshots are in `calibration.log` and `calibration/{metal,swiftshader}-dpr{1,2}/`. Existing P3/P4 defaults remain `extentPx: 1`, `ratio: [0.8, 1.25]`; no calibration edit is justified. The plan's stop-on-product-failure rule applies; full vitest, style, behavior e2e, final pixel matrix, mutation baseline and after/DR-L2 screenshots were not run. The implementation remains incomplete pending upstream repair and a fresh CR-07 run.

### Session: 2026-10-17 (CR-07 attempt 2)
**Tasks Completed**: Rechecked toolchain digests; rebuilt the editor with the declared debug WASM; reran check, focused and full Vitest, test:style, pixel calibration, and Chromium + WebKit behavior profile.
**Tasks In Progress**: Full CR-07 pixel matrix, baseline mutation evidence, after/DR-L2 screenshots and final documentation/evidence records.
**Blockers**: Stopped after the behavior profile wrote `tmp/canvas-cutover/evidence/s274-webkit-touch-events.json`, outside this plan's declared artifactRoots. The runtime contract requires a checkpoint amendment before continuing commands that generate outputs outside declared artifact roots. Resume after `tmp/canvas-cutover/evidence/` is admitted as an artifact root (or the e2e harness is configured to write the diagnostic under `tmp/code-render/CR-07/`).
**Notes**: `tmp/code-render/CR-07/toolchain-sha256-rerun.txt` matches the recorded wasm and package-lock digests. `build-rerun.log`, `check-rerun.log`, `focused-rerun.log` (7 files / 36 tests), `vitest-full-rerun.log` (101 files / 871 tests), and `test-style-rerun.log` (4 engine/viewport combinations) all exit 0. `calibration-rerun.log` and its `report.json` pass 4/4 rows (Metal and SwiftShader at DPR 1 and 2); P3/P4 defaults remain `extentPx: 1`, `ratio: [0.8, 1.25]`. `e2e-behavior-rerun.log` exits 0 with 22/22 checks: Chromium 13/13, including `code-pixels-dpr1` and `code-pixels-dpr2`; WebKit 9/9. The WebKit synthetic clipboard and IME limitations remain excluded by the runner. The diagnostic artifact is timestamped Oct 17 02:09:44 and hashes to `2a50eb17571f1c516c2ce0d4ca7ac59eef272c0187724160932e6addfa86c211`. CR-07 remains incomplete; no thresholds or product sources were changed.
