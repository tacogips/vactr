# EMK-PERF: emaki Playwright perf script, budget gates in WebKit and Chromium, perf repairs

**Status**: Ready (after EMK-DIFF-DEMO and EMK-JSON-DEMO)
**Plan ID**: EMK-PERF (wave 5; runs alone)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 9.2, 12.1, 12.2, 18 (risks); decisions EM-D9, EM-D11, EM-D12; user-QA default EQ1
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants a 10,000-line JSON file with syntax highlighting to be
displayed and edited quickly, drawing only the visible range. The design
turns that into these gates, measured by emaki's own Playwright perf script
in **both** WebKit (the primary target) and Chromium:

| Gate | Budget |
|------|--------|
| First paint, 10k lines | every one of 5 loads <= 200 ms |
| Scroll frame work | p95 <= 16.7 ms |
| Keystroke work (input + presenting frame) | p95 <= 16.7 ms, with >= 95% of keys presented on the next frame |
| Raw keydown-to-present | recorded only |
| Diff visible latency, 10k lines | p95 <= 100 ms |
| Size scaling | p95(100k) <= max(2 x p95(1k), p95(1k) + 2 ms) |

This plan writes `scripts/perf.mjs`, runs it, and, when a gate fails,
diagnoses and repairs the engine without relaxing any gate.

## Non-goals

- Never change a budget, a metric definition (design 12.2) or the fixture.
- No Chromium-only fast path. Every repair must apply to both browsers.
- No vactr edit. vactr re-verification is in EMK-CLOSEOUT.

## Dependencies

- **dependsOn**: EMK-DIFF-DEMO (`__emakiDiff`, diff page),
  EMK-JSON-DEMO (`__emakiPerf`, `keyMetrics`), EMK-SHOWCASE.
- EMK-SHOWCASE is included so no wave-4 plan is still writing `emaki/dist`.

## writePaths

- `emaki/scripts/perf.mjs`
- `emaki/dist`, `emaki/node_modules` (artifactRoots)
- `impl-plans/active/emaki-50-perf.md`

## sharedPaths

Perf repair only (see sharedPathNotes):
- `emaki/src/render/renderer.ts`, `emaki/src/render/text-pass.ts`,
  `emaki/src/render/decoration-pass.ts`, `emaki/src/render/segments.ts`,
  `emaki/src/render/atlas.ts`
- `emaki/src/layout/layout.ts`, `emaki/src/layout/advances.ts`,
  `emaki/src/layout/heightmap.ts`
- `emaki/src/language/incremental.ts`, `emaki/src/language/json-tokenizer.ts`
- `emaki/src/decor/store.ts`, `emaki/src/view/view-host.ts`,
  `emaki/src/input/input.ts`
- `emaki/src/editor.ts`, `emaki/src/editor-frame.ts`
- `emaki/examples/diff/diff-plugin.ts`

**sharedPathNotes (the same note for every path above).** Edit only to fix a
measured gate failure. Before editing:
- record the failing summary values;
- name the hot path with evidence: a Performance trace, or `PhaseTimer`
  phases logged through a temporary page flag that is removed afterwards.

Every repair keeps every emaki test green and changes no test expectation.
Record before/after numbers in the Progress Log. No other plan writes these
files in wave 5.

## Tasks

### PF1: `scripts/perf.mjs`

**CLI.**
`node scripts/perf.mjs [--browser chromium|webkit|all] [--sizes 1000,10000,100000] [--loads 5] [--keys 200] [--edits 100] [--out <dir>] [--self-test]`.
- Defaults: `all`, all three sizes, 5 loads, 200 keys, 100 edits, and
  `--out perf-results/<iso-timestamp>`.
- Exit codes: 0 when all gates pass in every requested browser; 1 when a
  gate fails; 2 when `dist` is missing, a browser cannot launch, or a hook
  is absent.

**Launch.** Use the Playwright library (not `@playwright/test`).
- Chromium args: `['--mute-audio', '--use-angle=metal', '--enable-gpu', '--ignore-gpu-blocklist']`.
  This literal is copied from the editor harness. Do not import from
  outside `emaki/`.
- WebKit: defaults.
- Viewport 1280x800, `deviceScaleFactor` 2.
- `routeDist(context, 'dist')` from `scripts/route-server.mjs`.

**Measurements.** Per browser and size, each in a fresh page:

1. **First paint.** `--loads` times: open
   `json-10k/index.html?lines=N&autoload=0`, call
   `await page.evaluate(() => __emakiPerf.load())`, and record
   `firstPaintMs`.
2. **Scroll.** After a load:
   - `startRecording()`;
   - 300 steps of `page.mouse.wheel(0, 120)` over the editor, each followed
     by a page-side wait for 1 rAF;
   - `stopRecording()`;
   - frame work = the `workMs` of the presents in the window;
   - rAF interval = the `frameStart` deltas (informational).

   If `scrollTop` did not advance after 10 wheel steps (a headless WebKit
   quirk), switch to `__emakiPerf.scrollBy(120)` and record `scrollPath`.
3. **Keystrokes.**
   - `startRecording()`;
   - for each of `--keys` keys: choose a seeded line inside the visible
     range (mulberry32 seed 7), call `caretToLine(line, 2)`, wait 2 rAFs,
     then `page.keyboard.press(<seeded printable char a-z>)`, then wait
     3 rAFs;
   - `stopRecording()`;
   - compute `keyMetrics(...)` in the page: `a`, `b`, `nextFrame`,
     `missing`.
4. **Diff latency** (size 10000 only).
   - Open `diff/index.html?lines=10000` and wait for
     `stats().diffRevision > 0`.
   - Install a page-side capture `keydown` listener on the right editor
     host that stores `event.timeStamp` per resulting revision.
   - For `--edits`: place the caret on a seeded right-editor visible line
     (click), type one char, and await
     `__emakiDiff.onApplied(rightRevision)`.
   - Latency = `presentEnd - keydownTimeStamp`.

**Output** goes to `--out`:
- `raw-<browser>-<size>.jsonl`: one line per sample with `{metric, value, ...context}`;
- `diff-<browser>.jsonl`;
- `env.json`: OS, CPU, the `loadavg` before and after, browser versions,
  `dist` sha256 of `json-10k/index.html`, the git HEAD read with
  `git rev-parse` via `child_process.execFileSync`, and the `scrollPath`s;
- `summary.json`: `{ gates: [{ browser, name, value, budget, pass }], allPass }`.

The p95 is the nearest-rank (`ceil(0.95 n)`-th) of sorted values. Document
this in the script header.

**`--self-test`** runs the stats and gate functions on synthetic arrays. For
example, it checks that the p95 of 1..100 is 95, and that the scaling gate
fails for `1k = 2 ms`, `100k = 6 ms`. Exit 0 or 1, with no browser.

### PF2: Run, diagnose, repair

0. Re-confirm the joined tree before measuring:
   - cite the green wave-4 join gate (`tmp/emaki/wave-4-join/`, which
     includes G4 and smoke);
   - run `cd emaki && npm run check && ./node_modules/.bin/vitest run`,
     which must exit 0. This plan runs alone in wave 5, so these are full
     pass gates.
1. `npm run build` under the lock.
2. Full run under the lock:
   `npm run perf -- --browser all --out ../tmp/emaki/EMK-PERF/run-1`.
3. For each failing gate:
   - diagnose the hot path (see sharedPathNotes);
   - repair;
   - rerun `npm test`;
   - rebuild;
   - rerun only the failing browser and metric, with `--out run-k`;
   - finally do a full both-browser run, with `--out run-final`.

Typical suspects to check first, with each claim backed by evidence:
- a full-document scan in `TextLayout.setText`, the widest-line tracking,
  `IncrementalHighlighter.applyChanges` (the checkpoint splice), or the
  `decorationsKey` building;
- atlas uploads on scroll;
- segment cache misses.

If a gate still fails after diagnosis and repair attempts, set Status to
`Blocked` with the evidence and the remaining gap. Do not relax anything.
The review step decides.

## Invariants

- The metric definitions equal design 12.2, and the raw numbers are always
  written.
- All emaki tests pass after every repair, and the boundary test stays
  green.
- The vactr editor compiles: `cd editor && npm run check` after any repair.
  It consumes the emaki sources.

## Verification

Logs go to `tmp/emaki/EMK-PERF/`.

1. `cd emaki && node --check scripts/perf.mjs && node scripts/perf.mjs --self-test`:
   exit 0.
2. `cd emaki && npm run check && ./node_modules/.bin/vitest run`: exit 0.
3. `cd editor && npm run check`: exit 0 (only if a sharedPath changed;
   otherwise record it as not needed).
4. `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts test/code/renderer-layers.test.ts test/canvas/mount.test.ts`:
   exit 0, only if a render or layout sharedPath changed.
5. Under the lock: `cd emaki && npm run build && npm run perf -- --browser all --out ../tmp/emaki/EMK-PERF/run-final`.
   Expect exit 0, and `summary.json` `allPass: true` with every gate listed
   for both browsers. Record the raw numbers per gate in the Progress Log as
   a table. Inside the sandbox, record blocked-in-sandbox (rule 11).

**mutationEvidence.** In a mutation run only, temporarily set
`maxLinesPerFrame` to `Infinity` and tokenize from line 0 on every
`tokens()` call. The 100k scaling or first-paint gate must fail. Restore
afterwards and verify the hash.

## Completion Criteria

- [ ] `perf.mjs` is delivered per PF1. Self-test passes.
- [ ] `run-final/summary.json` `allPass` is true for Chromium and WebKit.
  The raw files are present. The table is recorded.
- [ ] Every sharedPath edit is justified with before/after numbers and
  hashes.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
