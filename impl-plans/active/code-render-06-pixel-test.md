# Code Render 06: Playwright Pixel Test and Harness Gap Closure

**Status**: Completed
**Plan ID**: CR-06
**Wave**: 1 (no dependencies; its gating run happens in CR-07 after CR-05)
**Design Reference**: design-docs/specs/design-code-pane-text-rendering.md sections 5.2 (P1-P8), 5.4; design-docs/specs/design-canvas-editor-evidence.md "Code-pane text rendering gap (2026-10-16)"
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

The existing canvas evidence missed the defects. The only pixel check
(`editor/test/e2e/behavior.mjs` 'canvas-only-text') reads the line-1
rectangle and asserts only "at least 2 colors". DPR 2 is checked by value
only, overlays are never located, and the console is not scanned. This
plan adds a pixel test that compares the canvas code pane against DOM
text in the same font, per character, at DPR 1 and 2, on ANGLE Metal and
SwiftShader. It also locates the caret and the diagnostic underline,
checks backdrop uniformity, and fails on WebGL console errors. It is a
plain Node Playwright-library script like the existing harness (no
`@playwright/test`).

## Non-goals

- No new npm dependency (no PNG decoder package). Decode screenshots in a
  browser page (see "Pixel access").
- Do not change `run.mjs`, `measure.mjs`, `stats.mjs`, or any evidence
  under `design-docs/specs/evidence/`.
- Do not touch the renderer, layout, atlas, geometry or visual sources.
  The only source change is the read-only `annotations()` accessor in
  `perf-hook.ts`.
- Never change system volume. Every browser context installs the silent
  sink (`installSilentSink(context)` from `./silent-sink.mjs`) before
  creating a page.

## Files

| Path | Change |
| --- | --- |
| `editor/test/e2e/pixel-compare.mjs` | New. Pure functions on `{ data: Uint8ClampedArray, width, height }` images. No imports, no top-level side effects, so its source can be injected into a page. |
| `editor/test/e2e/pixel-compare.test.ts` | New vitest unit tests on synthetic images. |
| `editor/test/e2e/code-pixels.mjs` | New driver: `export async function runCodePixels(browser, origin, options)` plus a CLI when run directly. |
| `editor/test/e2e/behavior.mjs` | Add two Chromium-only checks, `code-pixels-dpr1` and `code-pixels-dpr2`, after the existing checks. |
| `editor/test/e2e/README.md` | Document the pixel test: what it checks, the CLI, the backend matrix, and the lock and port rules. |
| `editor/src/code/perf-hook.ts` | Add `annotations(): { kind: string; from: number; to: number }[]` to `VactrPerf`, implemented from `input.surface.annotationRanges()`. |
| `editor/test/code/perf-hook-annotations.test.ts` | New small unit test for `annotations()`. |
| `impl-plans/active/code-render-06-pixel-test.md` | Progress log, including the P3/P4 calibration record. |

Patterns to imitate: `behavior.mjs:runBehavior` (the `check(id, action)`
wrapper, `goto(`${origin}/?perf=1`)`, waiting for `window.__vactrPerf`
and `.vact-code-canvas`); `run.mjs` (chromium launch args, `startServer`
from `./serve.mjs`, the JSON result line, exit codes 0/1/2);
`stats.test.ts` (vitest on pure `.mjs` helpers).

## pixel-compare.mjs contract (pure)

Pin these exports (names and shapes); internals are free:

- `modeColor(img, rect) -> [r, g, b]`: most frequent RGB in the rect (the
  backdrop estimate).
- `cellInk(img, rect, backdrop) -> { ink, inkLeft, inkRight }`. Per
  pixel, coverage is `dist(pixel, backdrop) / dist(brightest pixel in
  rect, backdrop)`, clamped to [0, 1] (Euclidean RGB distance; 0 when the
  rect has no pixel farther than 8 levels from the backdrop). `ink` is the
  coverage sum. `inkLeft`/`inkRight` are the first/last columns with
  coverage of at least 0.25, or `null`.
- `compareLine(canvasCells, referenceCells, chars, tolerances) -> { failures: string[], inkRatio }`:
  P1 to P4 for one line (definitions below). `tolerances = { minInk: 0.5,
  extentPx: 1, ratio: [0.8, 1.25] }` by default.
- `verticalRuns(img, x, color, tol) -> [top, bottom][]`: runs of pixels at
  column x within `tol` of `color`.
- `bandRows(img, rect, color, tol) -> number[]`: rows in the rect that
  contain at least one pixel within `tol` of `color`.
- `columnUniform(img, x, top, bottom, tol) -> boolean`.

Checks (one line = one document line fully inside the canvas viewport):

- P1: each non-whitespace character cell has canvas `ink` of at least
  `minInk` times its reference `ink`.
- P2: each whitespace cell has canvas `ink` at most 0.15 times the line's
  mean non-whitespace reference ink.
- P3: for each non-whitespace character, `|(canvas inkRight - inkLeft) -
  (reference inkRight - inkLeft)| <= extentPx` device px.
- P4: the line's total canvas ink divided by total reference ink is within
  `ratio`.

## code-pixels.mjs contract

- `runCodePixels(browser, origin, { dpr, expectBackend, scenarios?, outDir? })`
  -> resolves to a detail string when every check passes; throws an
  `Error` whose message lists every failing check (scenario, line, char
  index, check id, measured value) otherwise. `expectBackend` is
  `'metal'` or `'swiftshader'`. Read the WebGL renderer string with
  `WEBGL_debug_renderer_info` on the code canvas page. If it does not
  contain `Metal` (or `SwiftShader`, respectively), throw. A mismatched
  backend never passes.
- CLI: `node test/e2e/code-pixels.mjs [--dist <dir>] [--backend metal|swiftshader|all] [--dpr 1|2|all] --out <dir>`.
  It starts `startServer({ dist })` in-process (port 0 on 127.0.0.1, so
  never 5173 or 5174; default dist `editor/dist`) and closes it in
  `finally`. For each backend it launches Chromium headless:
  - metal: `['--mute-audio', '--use-angle=metal', '--enable-gpu', '--ignore-gpu-blocklist']`
  - swiftshader: `['--mute-audio', '--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader']`

  It runs the DPR matrix and writes `<out>/report.json` (every check with
  pass/fail and measured values, the renderer strings, the P3/P4
  tolerances used), plus canvas and reference captures per scenario and
  DPR as PNG. It prints one JSON line `{ pass, testsRun, testsPassed,
  failureCount, report }`. Exit 0 when all pass, 1 on any failed check, 2
  when blocked (dist missing, browser launch failure, backend
  unavailable). A blocked run is never a pass.
- Each scenario uses a fresh context: viewport 1100x700,
  `deviceScaleFactor: dpr`, silent sink installed. `goto(`${origin}/?perf=1`)`,
  wait for `window.__vactrPerf` and `.vact-code-canvas`, and wait up to
  10 s for `[data-pane="code"]` `dataset.syntax === 'tree-sitter-worker'`
  (record when it times out; do not fail). Click the canvas at
  `(x + 200, y + 10)` as `tmp/code-render/repro-shot.mjs` does. Then
  act, and settle: wait until `__vactrPerf.counters().textPending` is
  false, then 2 animation frames, then 300 ms.
  - S1: `keyboard.type('use-bpm 108', { delay: 20 })`
  - S2: type `# hello`, press `Enter`, type `abc def` (the completion popup
    may be open)
  - S3: `keyboard.insertText('# c\nuse-bpm 108\ns :kalimba > note :c5')`
  - S4: `keyboard.insertText(<contents of examples/kalimba.vact>)`, read
    from the repository root with `node:fs`
- Document truth: `__vactrPerf.doc()` and `__vactrPerf.selection().head`
  (caret), and `__vactrPerf.annotations()` (diagnostics with
  `kind === 'diagnostic'`).
- Reference: after capturing the canvas, inject an overlay `div`
  (`position: fixed`, `pointer-events: none`, `aria-hidden="true"`,
  `z-index` max, opaque background `#000`). It is positioned exactly over
  the canvas text area (canvas rect left + 48 CSS px gutter, canvas rect
  top) with one row per document line: `height: 18px`, `line-height:
  18px`, `white-space: pre`, `font: 13px ui-monospace, SFMono-Regular,
  Menlo, monospace` (the constant from `editor/src/code/mount.ts:84`;
  cite it in a comment), `color: #fff`. Capture the same clip, then remove
  the overlay. Character boxes come from `Range.getClientRects()` per
  character on the overlay rows. Map device px as `(cssX - clipX) * dpr`.
- Cell rect for a character: its reference box horizontally, and
  vertically the line row from top to `top + 18 - 2` CSS px (this
  excludes underline overlays). Skip characters whose box ends beyond the
  canvas right edge minus 8 CSS px, and characters intersecting the
  bounding box of `.vact-completion` or `.vact-code-diag-tooltip` when
  those are visible. Record skip counts.
- Backdrop: `modeColor` of each line's row band, separately for the
  canvas capture and the reference capture.
- P5 caret: palette cursor color = the canvas text color. Expected caret
  column = the reference x at the caret's line and column (the end of the
  last character box before the caret, or the row's left when the column
  is 0). Within up to 6 captures 150 ms apart (the caret blinks), pass
  when some capture has a vertical run of at least `0.8 * 18` CSS px in
  the caret line's row band, within 1 device px of the expected column,
  and no capture has such a run at that column in any other row.
- P6 diagnostics: for each diagnostic whose start line is fully visible,
  the rows of `--vt-danger`-colored pixels (sample the color from
  `getComputedStyle(document.documentElement).getPropertyValue('--vt-danger')`;
  tolerance 40) inside the canvas, restricted to the diagnostic's x span
  from the reference boxes, must all lie within the bottom 2 CSS px band
  of that line's row. Across S2 and S3, at least one diagnostic must be
  checked; if none exists, P6 fails ("no diagnostic to check") so it
  cannot pass vacuously.
- P7 backdrop: for 3 columns (canvas right edge minus 8, 40 and 80 CSS
  px, skipping columns covered by the completion popup), `columnUniform`
  from the row below the last document line to the canvas bottom, within
  2 levels.
- P8 console: `page.on('console')` and `page.on('pageerror')` from before
  `goto`. Fail on any message matching `/GL_INVALID|WebGL: INVALID|GL ERROR/`.

## Pixel access (no new dependency)

`page.screenshot({ clip })` returns PNG bytes. Decode in a dedicated
`about:blank` analysis page in the same browser: pass the base64 string,
create an `Image` from a `data:image/png;base64,` URL, `await
img.decode()`, draw it to a 2D canvas, and `getImageData`. Run the
pixel-compare functions inside that page: read `pixel-compare.mjs`
source, strip the `export ` keywords, and inject it with
`page.addScriptTag({ content })` that assigns the functions to
`window.__pixelCompare`. Return only small results (failures, ratios) to
Node. Never return full pixel arrays to Node.

## behavior.mjs integration

After the existing checks, when `name === 'chromium'`:
`await check('code-pixels-dpr1', () => runCodePixels(browser, origin, { dpr: 1, expectBackend: 'metal' }))`,
and the same for `dpr: 2`. These run on the run.mjs browser (metal
flags) and their own contexts. Keep the change under 10 lines; WebKit is
unchanged.

## Calibration rule (P3, P4)

The defaults are `extentPx = 1` and `ratio = [0.8, 1.25]`. The fixed code
does not exist until CR-05. CR-07 performs the single allowed
calibration on the fixed build: P3 may go up to 2 device px; P4 may move
to the observed range widened by 0.05 on each side, but must stay inside
[0.7, 1.4]. The calibrated values become the constants in
`code-pixels.mjs` and are recorded in this plan's and CR-07's progress
logs before any mutation run. P1, P2 and P5-P8 thresholds are fixed.

## Test Cases

`editor/test/e2e/pixel-compare.test.ts` (synthetic images):

- Uniform backdrop with a 3x10 white bar -> `cellInk` gives ink about 30,
  `inkLeft`/`inkRight` equal the bar's columns.
- Rect with only backdrop -> ink 0, extents `null`.
- `compareLine` with a missing glyph (canvas cell empty, reference inked)
  -> a P1 failure naming the character index.
- `compareLine` with a squashed glyph (canvas extent 1 px, reference 5
  px) -> a P3 failure.
- `compareLine` with canvas ink 1.6 times the reference -> a P4 failure;
  1.0 times -> none.
- Whitespace cell with ink -> a P2 failure.
- `verticalRuns` finds an 18-px run at the right column; `bandRows`
  returns only the underline rows; with tolerance 2, `columnUniform` is
  false for a column with a 5-level step and true for a uniform column.
- `modeColor` returns the majority color.

`editor/test/code/perf-hook-annotations.test.ts`:

- `installPerfHook` with a stub `surface` whose `annotationRanges()`
  returns `[{ kind: 'diagnostic', from: 4, to: 7, className: 'x' }]` ->
  `annotations()` returns `[{ kind: 'diagnostic', from: 4, to: 7 }]`
  (only those three keys). Other inputs are minimal stubs cast with `as
  never`; call `removePerfHook` afterwards.

## Invariants

- `behavior.mjs` stays under 210 lines. `code-pixels.mjs` and
  `pixel-compare.mjs` each stay under 400 lines; split helpers if needed.
- The existing behavior checks and their order are unchanged.
- No browser ever runs against ports 5173 or 5174.

## Write Scope

Mirrors `impl-plans/active/code-render-dispatch.json` plans[CR-06].

- writePaths: `editor/test/e2e/pixel-compare.mjs`, `editor/test/e2e/pixel-compare.test.ts`, `editor/test/e2e/code-pixels.mjs`, `editor/test/e2e/behavior.mjs`, `editor/test/e2e/README.md`, `editor/src/code/perf-hook.ts`, `editor/test/code/perf-hook-annotations.test.ts`, `impl-plans/active/code-render-06-pixel-test.md`, `tmp/code-render/CR-06`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`
- sharedPaths: none. Later, in wave 3, CR-07 may change only the P3/P4 calibration constants in `editor/test/e2e/code-pixels.mjs` and append a calibration record to this plan's Progress Log (declared as CR-07 sharedPaths).
- artifactRoots: `tmp/code-render/CR-06`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules` (the last three are prebuilt read-only inputs: never rebuild, reinstall or delete them). The optional smoke run uses an existing `editor/dist` read-only and never builds it.
- Toolchain audit: before the first edit, from the repository root run
  `shasum -a 256 target/wasm32-unknown-unknown/debug/vactr.wasm tree-sitter-vact/tree-sitter-vact.wasm editor/package-lock.json > tmp/code-render/CR-06/toolchain-sha256.txt`
  and copy the three digests into the Progress Log.

## Working Rules

Fresh read plus sha256 before and after each edit of a pre-existing file
(`behavior.mjs`, `README.md`, `perf-hook.ts`). Re-read on drift. No git
writes; foreground only. Logs go under `tmp/code-render/CR-06/`.
Wave-1 peers change renderer sources concurrently, so a browser run in
this plan is only a smoke run of the driver; the gating run is in
CR-07.

## Verification

Run from `editor/`.

1. `npx vitest run test/e2e/pixel-compare.test.ts test/code/perf-hook-annotations.test.ts > ../tmp/code-render/CR-06/focused.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-06/focused.log` -> exit 0.
2. `node --check test/e2e/code-pixels.mjs > ../tmp/code-render/CR-06/syntax.log 2>&1; node --check test/e2e/pixel-compare.mjs >> ../tmp/code-render/CR-06/syntax.log 2>&1; node --check test/e2e/behavior.mjs >> ../tmp/code-render/CR-06/syntax.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-06/syntax.log` -> exit 0.
3. `npm run check > ../tmp/code-render/CR-06/check.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-06/check.log` -> exit 0 (or only cross-plan errors recorded).
4. Smoke (optional, informational, not gating): when `editor/dist` exists,
   take the measurement lock (`mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`;
   if it exists, wait and retry; never remove someone else's lock), run
   `node test/e2e/code-pixels.mjs --backend metal --dpr 1 --out ../tmp/code-render/CR-06/smoke > ../tmp/code-render/CR-06/smoke.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-06/smoke.log`,
   then `rmdir` the lock. This shows that the driver runs end to end and
   writes `report.json`. Pass or fail of the checks is expected to vary
   while peers are mid-change.

## Completion Criteria

- [x] `pixel-compare.mjs` exports the pinned functions; unit tests pass
- [x] `code-pixels.mjs` implements S1-S4, P1-P8, the backend check, the CLI and exit codes 0/1/2
- [x] `behavior.mjs` gates `code-pixels-dpr1` and `code-pixels-dpr2` for Chromium
- [x] `perf-hook.ts` exposes `annotations()`; its unit test passes
- [x] README documents the pixel test
- [x] Progress log updated

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Ready for wave 1.

### Session: 2026-10-16 implementation
**Tasks Completed**: Pure comparison helpers and synthetic tests; S1-S4/P1-P8 pixel driver with Metal/SwiftShader CLI and Chromium DPR gates; perf annotations accessor and test; README update.
**Tasks In Progress**: None for CR-06. CR-07 owns the serialized browser matrix and single P3/P4 calibration opportunity.
**Blockers**: None.
**Artifact SHA-256**: `target/wasm32-unknown-unknown/debug/vactr.wasm` de9c55f37b4037d73e28f8aaa11e06d3bc87a3a41a3bc95dd8e64ebf02c93210; `tree-sitter-vact/tree-sitter-vact.wasm` ae3e5cdf1217e943f19c3ff15f111801e537ba9b7e463f84d6bf8cedd7fd0609; `editor/package-lock.json` f32e9585c409487e0d74ee9b8dc1880d4e54ff8610ab2cefa529828c53449749.
**Verification**: Focused Vitest passed (2 files, 4 tests); Node syntax checks passed; `npm run check` initially found TS7016 on the pure `.mjs` import, corrected with a file-local `@ts-expect-error`, final rerun recorded below. Logs are under `tmp/code-render/CR-06/`.
**Browser smoke**: Not run; it is optional here, and CR-07 owns the serialized gating browser run after renderer integration.
**Final Verification Logs**: `tmp/code-render/CR-06/focused-final.log` (exit 0; 2 files, 4 tests passed); `tmp/code-render/CR-06/syntax-final.log` (exit 0; all three `node --check` commands passed); `tmp/code-render/CR-06/check-final.log` (exit 0; `npm run check`). The initial type-check failure is preserved in `tmp/code-render/CR-06/check.log` and was resolved before this final-source rerun.
**Current-Source Rerun**: After refining diagnostic span geometry, final evidence is `tmp/code-render/CR-06/focused-current.log` (exit 0; 2 files, 4 tests passed), `tmp/code-render/CR-06/syntax-current.log` (exit 0), and `tmp/code-render/CR-06/check-current.log` (exit 0).
**Integrity Review Repairs (comm-006552)**: CR06-TI-01 guards P2/P4 when no non-whitespace cell is comparable, reports a null ink ratio for those lines, adds empty/whitespace-only assertions, and records `comparedCells` for every line. CR06-TI-02 samples the clamped `caretX-1..caretX+1` columns for both caret presence and row exclusion, preserving run-length and color tolerances; `caretCheck` records `caretX` and its columns. Repair verification: `tmp/code-render/CR-06/focused-repair.log` (exit 0; 2 files, 5 tests passed), `syntax-repair.log` (exit 0), `check-repair.log` (exit 0). Direct empty-line reproduction now prints `{"failures":[],"inkRatio":null}`. Independent re-review remains pending.
**Integrity Repair Redispatch (INT-377-04)**: Fresh SHA-256 identities match the review brief: `pixel-compare.mjs` 84d0bf85b8b74b1d9950df764c3e82e54088c095c3f3e8599d1fbdb998f04c11, `pixel-compare.test.ts` e79ee536fab4e3788e8c112e7d0166a0641f684712e043e45137af4f787e8a24, and `code-pixels.mjs` c4dab2409c789136ce48924699386cbb747ccd22f38c192e36cf60172eccde6c. Re-ran on these sources: `attempt-redispatch/focused.log` (exit 0; 2 files, 5 tests passed), `syntax.log` (exit 0; all three Node syntax checks passed), and `check.log` (exit 0; `npm run check`). Inspection confirms P2/P4 empty-line handling and clamped caret columns remain present; thresholds and P1-P8 definitions are unchanged. CR06-TI-01 and CR06-TI-02 are reported repaired against these hashes; independent test-integrity and adversarial decisions remain downstream.
### Session: 2026-10-17 implementation redispatch
**Tasks Completed**: Re-read CR-06 contract and review history; confirmed both integrity repairs remain present; no additional source correction was required.
**Verification**: Current-source focused Vitest passed (2 files, 5 tests) in `tmp/code-render/CR-06/handoff-2026-10-17-focused.log`; syntax checks passed in `handoff-2026-10-17-syntax.log`; `npm run check` passed in `handoff-2026-10-17-check.log` (all exit 0).
**Pending**: Independent test-integrity and adversarial re-review remain downstream; this session does not claim their acceptance.
**Downstream**: Independent integrity/adversarial review, CR-07 browser gating/calibration, review-dependent docs, commit and push remain owned by later workflow steps.

### Session: 2026-10-17 INT-380 repair redispatch
**Tasks Completed**: INT-380-P6a now waits up to 5 seconds for diagnostic annotations in S2/S3, records `diagnosticWaitMs`/`diagnosticWaitTimedOut`, and settles again before capture while retaining per-scenario and global non-vacuous diagnostic assertions. INT-380-P6b selects only visible diagnostic spans, clamps partially clipped ends to the last visible character box, and reports `diagnosticSkipped` per scenario. INT-380-P3 now uses a single DOM text node per line with per-code-point `Range` boxes; P3 measures the connected ink run nearest each cell center while P1/P2/P4 continue to use total ink. Added synthetic coverage for diagnostic clipping, neighbor bleed, and narrow-bar detection.
**Verification**: `tmp/code-render/CR-06/repair-380/focused-final.log` (exit 0; 2 files, 7 tests passed), `syntax-final.log` (exit 0), and `check-final.log` (exit 0). Locked Metal/SwiftShader × DPR 1/2 matrix passed 4/4 rows in `tmp/code-render/CR-06/repair-380/after-repair-matrix.log`; detailed per-backend reports are under `tmp/code-render/CR-06/after-repair/`. Two consecutive locked DPR-1 reruns passed 2/2 backend rows each in `dpr1-repeat-1.log` and `dpr1-repeat-2.log`. S2/S3 each had diagnostics and passing visible checks in every matrix row; S4 recorded two skipped clipped spans on each of kalimba lines 4 and 5. Screenshots are under the matching `after-repair` artifact directories.
**Resolved Attempt**: The first focused repair run failed only because the synthetic neighbor-bleed fixture added 1.5x total ink and correctly tripped the unchanged P4 threshold; the fixture was narrowed to 1.25x, and the final focused run passes. The initial complete matrix and both repeated DPR-1 runs pass on the final source.
**Pending**: Independent review of these repairs and CR-07 closeout remain downstream-owned. No renderer source or threshold was changed.
