# EMK-JSON-DEMO: 10k-line JSON editor page and perf hook

**Status**: Ready (after EMK-API and EMK-LANG)
**Plan ID**: EMK-JSON-DEMO (wave 4; parallel with EMK-DIFF-DEMO, EMK-SHOWCASE, EMK-VAC-PLUGINS)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 11.1, 11.2, 12.2; decision EM-D12
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants a 10,000-line JSON file with syntax highlighting that is
displayed and edited quickly, drawing only the visible range. This page is:
- the demo of that;
- the fixture host for `npm run perf` (EMK-PERF), which measures first
  paint, scroll frame work and keystroke work at 1k, 10k and 100k lines.

This plan builds the page and a demo-only measurement hook. The hook records
samples from `handle.perf` (EMK-API contract).

## Non-goals

- No perf script and no budget enforcement (EMK-PERF).
- No engine change. Report engine defects as a scope amendment.

## Dependencies

- **dependsOn**: EMK-API, EMK-LANG.

## writePaths

Page:
- `emaki/examples/json-10k/index.html`, `emaki/examples/json-10k/main.ts`,
  `emaki/examples/json-10k/perf-hook.ts`

Test:
- `emaki/test/examples/json-perf-hook.test.ts`

Generated, plus this plan:
- `emaki/dist`, `emaki/node_modules` (artifactRoots)
- `impl-plans/active/emaki-41-json-demo.md`

## sharedPaths

None.

## Tasks

### J1: Page

- Header bar: title, line count, and three buttons `1k`, `10k`, `100k`
  (flat, spaced) that reload with `?lines=`.
- The editor fills the rest of the page.
- Query `?lines=N`, default 10000. `N` is restricted to 1000, 10000 or
  100000. Other values fall back to 10000.
- Startup:
  1. Generate `const text = generateJsonFixture({ lines: N, seed: 1 })`
     **before** creating the editor.
  2. `createEditor(host, { language: jsonLanguage })` with an empty doc.
  3. If `?autoload=0` is absent, call `hook.load()`.

### J2: `perf-hook.ts`: `window.__emakiPerf`

**`load(): Promise<{ firstPaintMs: number }>`**
- `t0 = performance.now()`, then `handle.setDoc(text)`.
- Resolve on the first `onPresent` whose `revision` is the setDoc
  revision and whose `visibleComplete === true`.
- `firstPaintMs = frameEnd - t0`.

**`startRecording()`** clears the buffers and subscribes to:
- `onPresent`, recording `{ frameStart, frameEnd, workMs, revision }`;
- `onInput`, recording `{ eventTimeStamp, start, end, workMs, revisionAfter }`.

**`stopRecording()`** returns
`{ presents: PresentSample[]; inputs: InputSample[] }`.

**`scrollBy(px)`** calls `handle.scrollTo({ top: scrollTop + px })`. It
exists for scripted scroll when wheel events are unreliable in headless
WebKit. EMK-PERF uses the wheel first and records which path it used.

**`caretToLine(line, col)`** places the cursor by dispatching a selection
transaction, then calls `focus()`.

**`info()`** returns
`{ lines, bytes, revision, scrollTop, totalHeight }`.

**Derived metrics** are a pure exported function
`keyMetrics(inputs, presents)`. For each `keydown` input, the presenting
frame is the first present with `revision >= revisionAfter` and
`frameStart >= end`. Per key it returns:
- `a = present.frameEnd - eventTimeStamp`;
- `b = inputWork + present.workMs`, where `inputWork` sums the `workMs` of
  the `keydown` and the `beforeinput` sample that share that key's
  `revisionAfter`;
- `nextFrame`: true if no other present started between `end` and the
  presenting present's `frameStart`.

EMK-PERF imports this logic by evaluating it in the page. Document the
return shape in a JSDoc block.

### J3: `test/examples/json-perf-hook.test.ts`

Test `keyMetrics` with synthetic samples:
- one keydown at t=100 (end 101, work 1), a beforeinput (work 0.5) with
  `revisionAfter` 1, and present `{ frameStart 110, frameEnd 112, workMs 2, revision 1 }`
  -> `a = 12`, `b = 3.5`, `nextFrame = true`;
- a present at 105 with revision 0 before it -> skipped. If it started after
  `end`, then `nextFrame` is false when a revision-1 present comes later;
- no presenting frame -> the key is reported as `missing` (counted against
  the >= 95% rule).

## Invariants

- The fixture is generated before measurement starts and is excluded from
  `firstPaintMs`.
- The hook only observes. It never changes editor behavior.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-JSON-DEMO/`.

1. `npm run check`: passes per rule 12b.
2. `./node_modules/.bin/vitest run test/examples/json-perf-hook.test.ts`:
   exit 0. **This is the pass gate.**
3. `./node_modules/.bin/vitest run`: informational per rule 12b.
4. The `dist/json-10k/index.html` existence check runs at the wave-4 join
   (rule 12a). The orchestrator appends it to this log. No per-plan build.
5. `wc -l` on every written file: each under 1000.

**mutationEvidence.** Make `keyMetrics` ignore `frameStart >= end` -> J3
case 2 fails. Restore afterwards.

## Completion Criteria

- [ ] J1-J3 delivered. The `__emakiPerf` shape matches J2 exactly (EMK-PERF
  depends on it).
- [ ] Verification steps pass. Mutation evidence is recorded. Hashes are
  logged.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
