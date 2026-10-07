# DOM-RENDERER-HARNESS: Comparison Driver, DOM Behavior Checks and measure.mjs Hooks Implementation Plan

**Status**: Completed (accepted in session 296 after the ADV-HARNESS-S296-01 repair; archived 2026-10-07). The `dom-visible-text` token check was later corrected by DOM-RENDERER-EVIDENCE to the string token; see that plan's progress log.
**Plan ID**: DOM-RENDERER-HARNESS (wave 3 of 4, serial chain)
**Design Reference**: `design-docs/specs/design-dom-renderer.md` DR-9 (DOM behavior e2e) and DR-10 (comparison protocol: matrix, conditions, order, lock, `measure.mjs` hooks, first-viewport probe, metric table, raw data, honesty and winner rules, report)
**Depends On**: DOM-RENDERER-MOUNT (`impl-plans/completed/dom-renderer-mount.md`)
**Next**: `impl-plans/active/dom-renderer-evidence.md`
**Created**: 2026-10-07
**Last Updated**: 2026-10-07

---

## Intent and Context

The user wants a measured canvas-versus-DOM comparison. This plan writes the tooling only. The
EVIDENCE plan runs it outside the sandbox under the measurement lock.

The Codex sandbox cannot bind localhost, so the implementer cannot run any browser here. All
logic that can be pure goes into `compare-stats.mjs` and is unit tested.

Repository facts:

- `editor/test/e2e/run.mjs` shows the canonical flow:
  - `startServer()` from `serve.mjs` returns `{origin, close}`.
  - `gatingPreflight({distWasm, releasePath, debugPath, writeEvidence: true})` from
    `wasm-profile.mjs` returns `{wasm, refusal}`; a refusal means exit 2.
  - Chromium launches with `['--mute-audio','--use-angle=metal','--enable-gpu','--ignore-gpu-blocklist']`.
  - WebKit is probed headless for WebGL2 and relaunched headed if WebGL2 is missing
    (`run.mjs:32`).
  - `installSilentSink(context)` comes from `silent-sink.mjs`.
  - The context is 1280x900 with DPR 1.
  - `runMeasurement(page, context, name, {profile:'all', runId, headless})` comes from
    `measure.mjs`.
  - Sample downsampling for written files is in `run.mjs:57`.
- `measure.mjs` reads `.vact-code-canvas` at `:12` (WebGL probe; keep) and `:55` (cycle
  font-size step; widen). It calls `createLargeDocument()` at `:13`. It samples
  `counters()` at `:43` and `:58`, and at the end at `:86`. Its result has
  `{metrics, samples, pass, failures, limitations, renderer, control}`. `samples` holds
  `input-pair` rows with `latencyMs`.
- `stats.mjs` exports `percentile`, `evaluate`, `THRESHOLDS` and `countChecks`. Do not edit
  `stats.mjs`.
- `fixtures/large-doc.mjs createLargeDocument()` returns `{text, controlText, head, lines,
  bytes, seed, voices, visualOutputs, longLines}`. It asserts 1 MiB, so it cannot be called with
  `lines: 1000`. Do not edit it.
- `behavior.mjs runBehavior(browser, origin, name)` returns `{name, checks, passed, total,
  limitations}`. Each check is `{id, pass, detail}` or `{id, status: 'limitation', pass: null,
  detail}`.
- DOM-mode contracts from earlier plans:
  - The URL is `/?perf=1&renderer=dom`.
  - The root is `.vact-code-dom` and lines are `.vact-dom-line`.
  - Gutter numbers are `.vact-dom-num`, with `textContent` = 1-based number.
  - Other elements: caret `.vact-dom-caret`, handles `.vact-dom-handle`, composition bars
    `.vact-dom-bar.vact-dom-comp`, keyword spans `.vact-dom-tok-keyword`.
  - `__vactrPerf.counters()` has `rendererKind`, `domNodes`, `renderer.liveLines`,
    `renderer.windowFrom`, `renderer.windowTo`, `renderer.lineBuilds` and `gpuStatus`.
- `editor/tsconfig.json` has `types: []` and `allowJs: false`. A vitest file loads `.mjs` through
  `const spec: string = '...'; await import(/* @vite-ignore */ spec)` with a typed interface
  (`test/e2e/stats.test.ts:1-27`).

## Non-goals

- No edit to `run.mjs`, `behavior.mjs`, `stats.mjs`, `silent-sink.mjs`, `serve.mjs`,
  `wasm-profile.mjs`, `fixtures/large-doc.mjs`, `package.json` or any product file.
- No threshold, workload, timing constant or Chromium flag differs between renderers.
- No lock acquisition inside the scripts. The caller acquires the lock. The script only refuses
  to run without it.
- No CSS Custom Highlight API probing and no extra metrics beyond DR-10.

## Ownership

writePaths:

- `editor/test/e2e/compare.mjs` (new)
- `editor/test/e2e/compare-stats.mjs` (new)
- `editor/test/e2e/compare-stats.test.ts` (new)
- `editor/test/e2e/behavior-dom.mjs` (new)
- `impl-plans/active/dom-renderer-harness.md` (checkboxes and progress log only)
- `tmp/dom-renderer/harness` (artifact root: logs and receipts)
- `editor/node_modules/.vite` (artifact root: vitest cache)

sharedPaths:

- `editor/test/e2e/measure.mjs`: four additive hooks only (see Contract 3).
- `editor/test/e2e/README.md`: append a "Renderer comparison" section.

## Contracts

### 1. `compare-stats.mjs` (pure ES module; no browser, no fs; imports only `./stats.mjs`)

Exports (exact names):

- `sliceWorkload(workload, lines)`:
  - Returns `{ ...workload, text: firstNRows.join('\n'), lines, bytes: utf8Length,
    sourceLines: workload.lines, longLines: <count of rows over 400 chars> }`.
  - `controlText` and `head` are unchanged.
  - Throws a `RangeError` when `lines < 8` (the head row count), when `lines > workload.lines`,
    or when the input is not an integer.
  - Use `TextEncoder` for the byte count.
- `RENDERERS = ['canvas','dom']` and `BROWSERS = ['chromium','webkit']`, both frozen.
  `DEFAULT_LINES = [1000, 20000]` and `DEFAULT_RUNS = 3`.
- `runOrder(runs, renderers)`: an array of `{run, renderer, position}`. Run k (1-based) uses
  `renderers` order for odd k and reversed order for even k. For 3 runs this gives
  `r1 canvas,dom; r2 dom,canvas; r3 canvas,dom`.
- `runKey({lines, browser, renderer, run})`: `${lines}-${browser}-${renderer}-r${run}`.
- `inputP50(samples)`: `percentile` p50 of `latencyMs` over rows with `phase === 'input-pair'`.
  It returns `null` when there are none.
- `extractRunMetrics(measured, extras)`: a flat object with these keys:
  - Input and frame: `inputP50`, `inputP95`, `inputP99`, `frameP95`, `frameP99`, `textP95`,
    `animP50`, `animP95`.
  - Sync: `syncP95`, `syncP99` (non-stall `metrics.syncAbsMs`), `stallSyncCount`,
    `stallSyncP50`, `stallSyncP95`, `stallSyncMax`, `lateActiveMismatchCount`,
    `replayedFlashCount`.
  - Memory and nodes: `heapGrowthBytes` (null when unavailable), `domNodesPeak`,
    `domNodesFinal`, `ledgerMaxBytes`.
  - First viewport: `mountToFirstFrameMs` and `loadToViewportMs` from `extras`.
  - Gate: `gatePass` (boolean, `measured.pass`) and `failures` (array).
  - Every missing number is `null`, never `0`.
- `METRIC_ROWS`: an ordered array of `{key, label, unit, lowerIsBetter: true}` for the report.
  It covers every numeric key above.
- `aggregateCell(records, key)`: `{values, median, min, max, missing}`. Nulls are excluded and
  counted in `missing`. When every value is null, `median`, `min` and `max` are `null`.
- `winner(canvasAgg, domAgg)`:
  - `'unavailable'` if either median is null.
  - Otherwise `'canvas'` or `'dom'` when the lower median differs from the other by more than
    `max(canvas.max - canvas.min, dom.max - dom.min)`.
  - Otherwise `'no clear difference'`.
- `downsampleSamples(samples, stride = 32)`:
  - Keeps every `stride`-th row (index 0, stride, 2*stride, ...) per phase, for phases
    `frame`, `presented`, `onset`, `sync-sample` and `stall-audit`.
  - Keeps `phase` rows whose exclusive-duration sum (`row.row.slice(2)` summed) is at least
    50 ms.
  - Keeps all other rows.
  - Returns `{rows, stride}`.
- `MAX_RUN_JSONL_BYTES = 870_000`.
- `contended(load1, cpus)` returns `load1 > cpus`.
- `checkLock(ownerText)` returns `{ ok: boolean, owner: string | null }`. `ok` is true iff the
  trimmed text is non-empty.
- `COMPARISON_BEGIN = '<!-- RENDERER-COMPARISON:BEGIN -->'` and
  `COMPARISON_END = '<!-- RENDERER-COMPARISON:END -->'`.
- `buildComparison(runRecords, environment, attempts)` returns `comparison.json`:
  - `{runId, environment, cells: [{lines, browser, metrics: {[key]: {canvas: agg, dom: agg,
    winner}}, gate: {canvas: {passed, runs}, dom: {passed, runs}}, runs: [...]}], attempts,
    contendedRuns}`.
- `renderComparison(comparison)` returns markdown with these sections:
  1. Summary: one table per (lines, browser), with rows from `METRIC_ROWS` and columns
     `canvas median | dom median | winner | threshold`.
  2. Method and environment.
  3. Per-run values: every run value in each cell.
  4. Gate results: pass counts and the failures listed per run.
  5. Contended runs and attempts.
  - Null renders as `unavailable`.
  - The threshold column uses `THRESHOLDS` from `stats.mjs` where a matching threshold exists,
    else `-`.
- `replaceMarked(source, section)`: replaces the text between the markers (exclusive). It
  throws if either marker is missing.
- `REPORT_SKELETON`: a full markdown document with these parts:
  - title "Canvas vs DOM Renderer Comparison";
  - a status line and a link to `design-dom-renderer.md`;
  - the markers with an empty generated region between them;
  - three implementer-written headings: `## Analysis`, `## Limitations` and
    `## Observations for the canvas tuning proposal`. Each contains only the line
    `Pending measured data.`

### 2. `compare.mjs` (CLI driver; Node built-ins, `playwright`, sibling modules)

- Flags:

  | Flag | Default |
  |------|---------|
  | `--run-id` | `rc-001` |
  | `--renderers` | `canvas,dom` |
  | `--browsers` | `chromium,webkit` |
  | `--lines` | `1000,20000` |
  | `--runs` | `3` |
  | `--behavior` | off (behavior mode instead of measurement) |
  | `--resume` | off (skip run keys whose `runs/<key>.json` exists with `complete: true`) |
  | `--write-report` | off (only rebuild `comparison.json` and the report from existing raw files; no browser and no lock needed) |
  | `--out` | `<repoRoot>/design-docs/specs/evidence/renderer-comparison/<run-id>` |
  | `--report` | `<repoRoot>/design-docs/specs/design-renderer-comparison.md` |

  Reject unknown flag values with exit 2.
- **Lock guard** (every mode except `--write-report`):
  - Read `${process.env.VACTR_MEASURE_LOCK ?? path.resolve(repoRoot, '..', '.measure-lock')}/owner`.
  - If `checkLock` is not `ok`, print the blocked JSON and exit 2.
  - Record the owner in `environment.json`.
- **Preflight:** `dist/index.html` exists and `gatingPreflight(... writeEvidence: true)` returns
  no refusal; otherwise exit 2. Record `wasm` in `environment.json`.
- **Behavior mode:** for each browser, launch a fresh browser (same args and WebKit probe as
  measurement).
  - Renderer `dom` runs `runDomBehavior`. Renderer `canvas` runs the unchanged `runBehavior`.
  - Write `behavior/<browser>-<renderer>-behavior.json`.
  - Exit 0 iff every non-limitation check passed.
- **Measurement mode**, for each `lines` in `--lines`, then each browser, then each entry of
  `runOrder` (filtered by `--renderers`):
  1. Fresh browser launch. The WebKit headless or headed decision is made once per invocation
     by the WebGL2 probe and recorded.
  2. **First-viewport probe:** a new context with the silent sink, page
     `/?perf=1&renderer=<r>`, wait for `__vactrPerf`.
     - `mountToFirstFrameMs = presented()[0].frameMs`. Wait up to 5 s for at least one row.
     - Read `t0 = performance.now()` in the page, `fill` the workload text into
       `.vact-code-input-bridge textarea`, then poll in the page.
     - Stop at the first `presented()` row with `frameMs > t0`, `revision ===
       __vactrPerf.revision()`, document length equal to the workload length, and
       `counters().textPending === false`.
     - `loadToViewportMs` is that row's `frameMs - t0`. Time out at 60 s with value `null`.
     - Close the context.
  3. **Measured page:** a new context (1280x900, DPR 1, silent sink), page
     `/?perf=1&renderer=<r>`, then `runMeasurement(page, context, browser, {profile: 'all',
     runId, headless, workload})`. The workload is `createLargeDocument()` for 20,000 lines and
     `sliceWorkload(createLargeDocument(), 1000)` for 1,000 lines.
  4. Record `os.loadavg()` and timestamps at start and end, plus `position`.
  5. Write `runs/<key>.json`: `{key, lines, browser, renderer, run, position, complete: true,
     headless, startedAt, endedAt, loadavgStart, loadavgEnd, contended, metrics:
     extractRunMetrics(...), rawMetrics: measured.metrics, rendererStats, gate: {pass, failures,
     limitations}}`. `rawMetrics` drops the bulky arrays `syncSamples`, `lateFrameAudits` and
     `beatResidual.frames`.
  6. Write `runs/<key>.jsonl` from `downsampleSamples`. If the serialized size is over
     `MAX_RUN_JSONL_BYTES`, double the stride until it fits, and record the stride in the JSON.
  7. **Attempts:**
     - Any thrown error is an attempt failure, appended to `attempts.json` with the error text.
     - Retry the same key once. A second failure leaves that key incomplete.
     - A gate failure is not an error and is never retried.
- **Exit and output:** at the end of measurement mode, rewrite `environment.json` and
  `comparison.json` (from all raw run files in `--out`).
  - Print one JSON line `{runId, pass, blocked, testsRun, testsPassed, failureCount,
    gatePassed, incomplete}`, where `testsRun` is the number of attempted keys, `testsPassed`
    the completed keys and `failureCount` the incomplete keys.
  - Exit 0 iff `failureCount === 0`. Gate failures are results, recorded with `gatePassed`.
  - Exit 2 for blocked.
- **`--write-report` mode:**
  - Build `comparison.json` from the raw files.
  - If the report file is missing, create it from `REPORT_SKELETON`.
  - Replace the marked region with `renderComparison`.
  - Never touch text outside the markers.
- Always `await browser.close()` in `finally`. Never leave a server running: `server.close()`
  goes in `finally`.

### 3. `measure.mjs` hooks (additive; canvas behavior unchanged)

1. Destructure `workload: workloadOverride = null` in the options of `runMeasurement`.
   `:13` becomes `profile==='behavior' ? null : (workloadOverride ?? createLargeDocument())`.
2. At `:55`, select `'.vact-code-canvas, .vact-code-dom'`. Leave the `:12` WebGL probe
   unchanged.
3. Declare `let domNodesPeak = 0`. At both existing `counters()` sample points (`:43` and
   `:58`), set `domNodesPeak = Math.max(domNodesPeak, counters.domNodes ?? 0)`. Set
   `metrics.domNodes = { peak: Math.max(domNodesPeak, counters.domNodes ?? 0), final:
   counters.domNodes ?? null }`, using the counters read at `:86`.
4. Set `metrics.rendererStats = counters.renderer ?? null` and `metrics.rendererKind =
   counters.rendererKind ?? 'canvas'`.

Do not reorder or remove anything else.

### 4. `behavior-dom.mjs`

`export async function runDomBehavior(browser, origin, name)` returns the same shape as
`runBehavior`. It uses the same context options (1280x900, DPR 1, `hasTouch`, and Chromium
clipboard permissions) and `installSilentSink`. It navigates to `/?perf=1&renderer=dom` and
waits for `.vact-code-dom`.

Checks (ids exact), each implementing DR-9:

- **`dom-visible-text`:**
  - `fill` a known multi-line source containing `let`, a comment and a Japanese line.
  - Wait 2 rAFs.
  - Assert `.vact-code-dom` exists and `.vact-code-canvas` does not.
  - For each `.vact-dom-num` visible in the viewport, the matching line element's text
    (`.vact-dom-line` whose top equals the number's top; compare `getBoundingClientRect().top`
    within 1 px) equals the source line.
  - A `.vact-dom-tok-keyword` computed color equals the computed `--vt-syn-keyword` resolved
    through a probe element.
  - The bridge textarea has opacity 0 or a transparent color, as in `behavior.mjs:46`.
- **`dom-caret-alignment`:**
  - For the ASCII, Japanese and emoji lines of the source, press `ControlOrMeta+Home`, then
    `ArrowDown` to the line, then `End`, then wait 2 rAFs.
  - Compare `.vact-dom-caret` left with the `right` of a DOM `Range` that ends at the last text
    node of that line element.
  - Tolerance is 1 px (ASCII) or 2 px (non-ASCII). Report the deltas in `detail`.
- **`editing-undo-redo-navigation`:** copy the assertions of `behavior.mjs:62-84` exactly.
- **`clipboard-round-trip` (Chromium) and `clipboard-synthetic-event` limitation (WebKit):**
  copy them as they are in `behavior.mjs`.
- **`japanese-ime` (Chromium):**
  - Use the CDP flow of `behavior.mjs:96-108`.
  - After `imeSetComposition`, additionally assert that some `.vact-dom-line` text contains
    `にほんご` and that a `.vact-dom-bar.vact-dom-comp` element exists.
  - On WebKit, record the `japanese-ime-synthetic` limitation.
- **`touch-selection`:**
  - Target `.vact-code-dom`.
  - Take the touch point from a DOM `Range` rect over the first word of the first rendered line
    (not hard-coded advances).
  - Chromium uses CDP touch, as in `behavior.mjs:114`. WebKit uses synthetic pointer events, as
    in `behavior.mjs:115`, with the same limitation note.
  - Assert a non-empty selection, `presented().at(-1).handles === 2` and two
    `.vact-dom-handle` elements.
- **`dpr-and-resize`:**
  - Selection is unchanged after `setViewportSize(1100, 760)`.
  - Then wait 100 ms and read `lineBuilds`.
  - Chromium: apply `Emulation.setDeviceMetricsOverride` with the same size and DPR 2. Assert
    `devicePixelRatio === 2`, `gpuStatus.effectiveDpr === 2` and a `lineBuilds` delta of 0.
  - WebKit: in a separate DPR-2 context, assert `effectiveDpr === 2`.
  - Assert the bridge bounds stay inside the viewport.
- **`visual-viewport-inset` and `background-no-replay`:** copy them from `behavior.mjs`.
- **`dom-bounded-nodes`:**
  - `fill` `createLargeDocument().text`.
  - `mouse.wheel(0, 180000)`, then wait 2 rAFs.
  - `rows = ceil(viewportHeight / 18)` and `margin = max(8, ceil(rows / 2))`.
  - Assert `renderer.liveLines <= rows + 2*margin`. Every `.vact-dom-num` value `n` satisfies
    `windowFrom < n <= windowTo`.
  - Repeat after `ControlOrMeta+End`.
- **`dispose-ledger`:** as in `behavior.mjs:180-183`, plus assert that no `.vact-code-dom`
  remains.

Do not run `canvas-only-text` or `context-loss-restore` in DOM mode.

## Key Points

- **Identical conditions are the point.** Reuse `runMeasurement` and the silent sink. Use the
  same launch args and the same `evaluate` (called inside `runMeasurement`). Never pass
  renderer-specific thresholds or options to `runMeasurement` other than the workload.
- **Honest unavailability.** WebKit heap is `null` and renders as `unavailable`. Never write 0
  for a missing value.
- **Contended runs** are kept and flagged, never dropped. Every completed run is reported.
- **Path safety.** Build every output path with `path.join(out, ...)` from validated keys. Do
  not interpolate raw CLI strings into paths. `--run-id` must match `^[a-z0-9-]+$`.
- **Write order.** Write each run's JSON as soon as the run finishes, so `--resume` works after
  an interruption. Write via a temp file plus rename so a partial JSON never has
  `complete: true`.
- **Ignored inputs.** `--write-report` must work in the sandbox with no browser. It must ignore
  `.jsonl` files and must not throw when a cell is incomplete; such cells render as
  `unavailable`.
- **Process control.** Never spawn background processes. Never call `process.exit` before
  `finally` cleanup completes.

## Patterns to imitate

- `editor/test/e2e/run.mjs` for launch, probe, preflight, server, the result JSON line and exit
  codes.
- `editor/test/e2e/behavior.mjs` for check structure and the CDP and touch flows.
- `editor/test/e2e/stats.test.ts:1-27` for the vitest import of `.mjs` with a typed interface.

## Tests (`editor/test/e2e/compare-stats.test.ts`, `// @vitest-environment node`)

Import `compare-stats.mjs` and `fixtures/large-doc.mjs` through the dynamic-import pattern.

- `sliceWorkload(createLargeDocument(), 1000)` -> `lines === 1000`;
  `text.split('\n').length === 1000`; `text.startsWith(head)`; `controlText` equal to the
  source; `bytes` equal to the UTF-8 length; `sourceLines === 20000`.
- `sliceWorkload(w, 7)`, `sliceWorkload(w, 20001)` and `sliceWorkload(w, 1.5)` -> throw
  `RangeError`.
- `runOrder(3, ['canvas','dom'])` -> renderer sequence
  `canvas, dom, dom, canvas, canvas, dom`, with positions 0 and 1 in each run.
- `runKey({lines:1000, browser:'webkit', renderer:'dom', run:2})` -> `'1000-webkit-dom-r2'`.
- `inputP50` over input-pair rows `[10, 20, 30]`, mixed with other phases -> `20`. No rows ->
  `null`.
- `aggregateCell` on values `[3, 1, null, 2]` -> `median 2, min 1, max 3, missing 1`.
- `winner`:
  - medians 10 vs 20, ranges 2 and 3 -> the lower one wins;
  - medians 10 vs 11, ranges 3 and 1 -> `'no clear difference'`;
  - a null median -> `'unavailable'`.
- `downsampleSamples` over 100 `frame` rows, 3 `phase` rows (exclusive sums 10, 50 and 70) and
  5 `input-pair` rows -> 4 frame rows (indexes 0, 32, 64, 96), 2 phase rows and 5 input-pair
  rows.
- `extractRunMetrics` with `heapGrowthBytes: null` and a missing `domNodes` -> both `null`, not
  `0`.
- `renderComparison` on a 1-cell comparison with a null heap median -> contains `unavailable`
  and the canvas and dom columns.
- `replaceMarked`:
  - source without markers -> throws;
  - source with markers -> only the inner region is replaced, and the outer text is
    byte-identical.
- `contended(9, 8)` -> `true`, and `contended(8, 8)` -> `false`.
- `checkLock('')` -> `ok: false`, and `checkLock('owner-x\n')` -> `{ok: true, owner: 'owner-x'}`.
- `REPORT_SKELETON` contains both markers and the three implementer headings.

## Tasks

### TASK-H1: `compare-stats.mjs` and its tests
- [x] All exports exist as pinned. Every test bullet passes.

### TASK-H2: `measure.mjs` hooks
- [x] Only the four additive hooks. `git diff editor/test/e2e/measure.mjs` shows no other change.

### TASK-H3: `behavior-dom.mjs`
- [x] Every DR-9 check id is implemented. `node --check` passes.

### TASK-H4: `compare.mjs`
- [x] Flags, lock guard, preflight, behavior and measurement modes, raw files, attempts,
      `--resume`, `--write-report` and exit codes, all as pinned.

### TASK-H5: README and verification
- [x] `editor/test/e2e/README.md` gets a "Renderer comparison" section. It covers the lock
      wrapper command, the per-cell invocations, `--resume`, `--write-report` and the output
      layout.
- [x] The commands below pass. Logs are under `tmp/dom-renderer/harness/`.

## Verification (implementer, in the sandbox; serial)

From `editor/`:

1. `node --check test/e2e/compare.mjs && node --check test/e2e/compare-stats.mjs && node --check test/e2e/behavior-dom.mjs && node --check test/e2e/measure.mjs > ../tmp/dom-renderer/harness/node-check.log 2>&1`
   -> exit 0.
2. `npm run check > ../tmp/dom-renderer/harness/check.log 2>&1` -> exit 0.
3. `./node_modules/.bin/vitest run test/e2e/compare-stats.test.ts test/e2e/stats.test.ts > ../tmp/dom-renderer/harness/vitest-e2e.log 2>&1`
   -> exit 0, testsRun > 0.
4. `node test/e2e/compare.mjs --run-id harness-dry --write-report --out ../tmp/dom-renderer/harness/dry --report ../tmp/dom-renderer/harness/dry/report.md > ../tmp/dom-renderer/harness/write-report.log 2>&1`
   -> exit 0. The created report contains both markers and `unavailable` cells (no raw data).
   This proves `--write-report` works without a browser or lock and never touches the real
   report.
5. `git diff --stat -- editor/test/e2e/run.mjs editor/test/e2e/behavior.mjs editor/test/e2e/stats.mjs editor/test/e2e/fixtures/large-doc.mjs editor/test/e2e/silent-sink.mjs`
   -> empty.

Use the mandatory record format, with details in `notes`. Browser runs are out of scope here;
the EVIDENCE plan runs them outside the sandbox.

## Completion Criteria

- [x] All pinned exports, flags and check ids exist.
- [x] Verification 1-5 pass.
- [x] Only writePaths and the two sharedPaths changed.

## Progress Log

### Session: 2026-10-07 13:45 JST
**Tasks Completed**: TASK-H1 through TASK-H5. Added pure comparison statistics helpers/tests, serial comparison and report driver, DOM behavior checks, the four `measure.mjs` hooks, and README instructions.
**Verification**: `node --check test/e2e/compare.mjs && node --check test/e2e/compare-stats.mjs && node --check test/e2e/behavior-dom.mjs && node --check test/e2e/measure.mjs` (exit 0); `npm run check` (exit 0); Vitest focused e2e statistics suite (56/56); report-only dry run (exit 0; generated markers and unavailable metrics); protected-path diff stat empty.
**Notes**: Browser behavior and full comparison measurements are assigned to DOM-RENDERER-EVIDENCE. No changes to `run.mjs`, `behavior.mjs`, `stats.mjs`, `silent-sink.mjs` or `fixtures/large-doc.mjs`. Prior accepted mount files remained intact.
**Review repair (ADV-HARNESS-S296-01)**: `firstViewport` now selects the latest presented row at the loaded revision during the first rAF poll with `textPending === false`; fresh repair checks passed in `tmp/dom-renderer/harness/node-check-repair.log`, `check-repair.log`, `vitest-e2e-repair.log` (56/56), `write-report-repair.log`, and empty `protected-paths-repair.log`. The 60 s timeout-to-null and `mountToFirstFrameMs` definition are unchanged.
