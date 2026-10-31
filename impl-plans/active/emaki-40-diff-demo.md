# EMK-DIFF-DEMO: realtime split diff demo page and its Playwright e2e

**Status**: Ready (after EMK-API and EMK-DIFF)
**Plan ID**: EMK-DIFF-DEMO (wave 4; parallel with EMK-JSON-DEMO, EMK-SHOWCASE, EMK-VAC-PLUGINS)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 10.1, 11.1, 12.1, 12.3; decision EM-D11
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user asked for a sample that puts two documents side by side, the
master on the left and the edited copy on the right. While you type on the
right, it shows a GitHub-style diff in realtime:
- red and green line backgrounds;
- stronger intra-line word marks;
- aligned filler rows;
- line numbers in both gutters;
- synchronized vertical scrolling.

It must use only the public mark, line-format and widget APIs, proving they
are enough. Its e2e runs in Chromium and WebKit.

## Non-goals

- No engine or diff-module change. If a defect is found, stop and report a
  scope amendment.
- No perf script (EMK-PERF).
- No horizontal scroll sync.

## Dependencies

- **dependsOn**: EMK-API, EMK-DIFF.

## writePaths

Demo page:
- `emaki/examples/diff/index.html`, `emaki/examples/diff/main.ts`,
  `emaki/examples/diff/diff-plugin.ts`, `emaki/examples/diff/hook.ts`

Script:
- `emaki/scripts/e2e.mjs`

Test:
- `emaki/test/examples/diff-plugin.test.ts`

Generated, plus this plan:
- `emaki/dist`, `emaki/node_modules` (artifactRoots)
- `impl-plans/active/emaki-40-diff-demo.md`

## sharedPaths

None. Read-only: `emaki/src/**` (import only through `../../src/index`
and `../../src/diff/index`, which are public entries via relative paths
inside the package), `emaki/scripts/route-server.mjs`,
`emaki/examples/shared/demo.css`.

## Tasks

### DD1: `diff-plugin.ts`

`createDiffPair({ left: EditorHandle; right: EditorHandle; colors }): { dispose(): void; readonly stats: DiffStats }`.
It registers one plugin on each editor (`addPlugin`).

**Recompute**
- A right-editor `docChanged` sets `dirty`. The next `frame` hook of the
  right plugin recomputes once: several edits in one frame coalesce
  (design 10.1).
- Recompute: `diffLines(leftLines, rightLines)`, then
  `alignSplit(hunks, leftLength, rightLength)`.
- Lines come from `state.doc.toJSON()`. Its cost is accepted for the 100 ms
  gate. Record the measured time in the Progress Log.

**Apply.** Use only the plugin host APIs.

Line formats:
- left removed lines:
  `{ background: diff-del, gutterMarker: { text: '-', color } }`;
- right added lines:
  `{ background: diff-add, gutterMarker: { text: '+' } }`;
- modified pairs: the del and add backgrounds on their respective sides.

Word marks (`diffWords` on each modified pair):
- `{ background: diff-del-strong }` on the left;
- `{ background: diff-add-strong }` on the right;
- computed only for pairs whose lines fall in viewport +/- one viewport
  height, updated on `viewportChanged`.

Fillers:
- block widgets `{ key: 'fill:' + side + ':' + afterLine, side: 'below' (or 'above' for afterLine -1 at line 0), height: count x lineHeight, content: canvas }`;
- the canvas draws a flat diagonal hatch with `--emaki-*` token colors
  read once, and returns `false` (static).

**Synced scroll.** `left.onScroll` sets `right.scrollTo({ top })` and the
reverse, with a re-entrancy flag.

**Colors.** Read from CSS custom properties `--emaki-diff-add`,
`--emaki-diff-del`, `--emaki-diff-add-strong` and `--emaki-diff-del-strong`
on `document.documentElement`, parsed to Rgba.

### DD2: Page (`index.html`, `main.ts`)

- Two flexed panes (left label "master (read-only)", right label
  "working copy"). The style is flat, square and spaced, using
  `../shared/demo.css`.
- Query `?lines=N`, default 400. The master text is
  `generateJsonFixture({ lines: N, seed: 1 })` from `../../src/language/json`.
- The working copy starts as master with three deterministic edits (change a
  value on line 10, delete line 30, insert two lines after line 50). The
  demo therefore shows all row kinds on load.
- Both editors use `jsonLanguage`, line numbers on, left `readOnly: true`.

### DD3: Test hook (`hook.ts`, demo-only contract for EMK-PERF and e2e)

`window.__emakiDiff` provides:
- `stats(): { diffRevision: number; rightRevision: number; removed: number; added: number; modified: number; fillersLeft: number; fillersRight: number; fillerRowsLeft: number; fillerRowsRight: number; wordMarksLeft: number; wordMarksRight: number }`
- `scrollTops(): [number, number]`
- `onApplied(cb: (s: { rightRevision: number; presentEnd: number }) => void): () => void`

`onApplied` fires after the first `onPresent` (from `handle.perf`) of
**both** editors whose frames rendered the decorations for that
`rightRevision`. Implement it by tagging the revision when decorations are
set, then waiting for each editor's next `onPresent`.

### DD4: `scripts/e2e.mjs`

**Run setup**
- Arguments: `--browser chromium|webkit|all` (default `all`) and
  `--out <dir>` (default `perf-results/e2e`).
- Exit 2 if `dist/diff/index.html` is missing (`distReady`) or a browser
  cannot launch.
- Launch with the Playwright library: Chromium gets `--mute-audio`.
- `routeDist(context, 'dist')` (`route-server.mjs`), then go to
  `https://emaki.test/diff/index.html?lines=400`.

**Checks per browser** (wait for `__emakiDiff` with `stats().diffRevision > 0`):
1. Initial stats: `removed >= 1`, `added >= 2`, `modified >= 1`, and
   `fillerRowsLeft + leftLines === fillerRowsRight + rightLines`.
2. Focus the right editor: click inside its canvas on line 5. Type
   `"zzz"` at the end of the line using `page.keyboard.type`. Wait for
   `onApplied`. Then `modified` has increased by 1 and `wordMarksRight >= 1`.
3. Press Enter and type `new line`. Then `added` has increased by 1 and
   `fillerRowsLeft` has increased by 1.
4. Mouse wheel on the left pane by 600 px, wait 2 frames. Then
   `scrollTops()[0] === scrollTops()[1]`, and both are greater than 0.
5. No `console.error` or `pageerror` during the run.

**Output**
- Write `result.json`:
  `{ browser, checks: [{ name, ok, detail }], testsRun, testsPassed, failureCount }`
  per browser plus totals.
- Exit 0 only when all checks pass.

### DD5: `test/examples/diff-plugin.test.ts` (jsdom, recording GL via `createEditor` options)

- left `'a\nb\nc'`, right `'a\nx\nc\nd'` -> the left line 1 format has the
  del background, the right line 1 add background (modified pair), and the
  right line 3 is added;
- the left filler after line 2 has a count of 1;
- two edits in one frame -> exactly one recompute (`stats` counter);
- the scroll sync re-entrancy flag prevents a loop: one `scrollTo` per side.

## Invariants

- The example imports emaki only through `src/index.ts`,
  `src/diff/index.ts` and `src/language/json.ts`, the same modules the
  package exports.
- No vactr strings (boundary test).

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-DIFF-DEMO/`.

1. `npm run check`: passes per rule 12b. Sibling `examples/*` and
   `editor/*` errors are drift.
2. `./node_modules/.bin/vitest run test/examples/diff-plugin.test.ts`:
   exit 0, testsRun > 0. **This is the pass gate.**
3. `./node_modules/.bin/vitest run`: informational per rule 12b.
4. `node --check scripts/e2e.mjs`: exit 0.
5. **Diff e2e (G4) is produced at the wave-4 join** (rule 12a), not here.
   Sibling demo pages may be mid-edit, and the multi-page `vite build`
   compiles all of them. The orchestrator runs
   `cd emaki && npm run build && npm run e2e -- --browser all --out ../tmp/emaki/wave-4-join/e2e`
   outside the sandbox and appends the result to this Progress Log. The
   expected result is exit 0 and `result.json` failureCount 0 for Chromium
   and WebKit. A failing check routes back to this plan.
6. `wc -l` on every written file: each under 1000.

**mutationEvidence.** Disable the synced scroll -> e2e check 4 fails (or the
unit test fails). Restore afterwards.

## Completion Criteria

- [ ] DD1-DD5 delivered, using only public APIs (grep shows no
  `../../src/render` or `../../src/layout` imports in `examples/diff`).
- [ ] Verification steps 1-4 and 6 pass on this plan's scope. Step 5 (G4
  in both browsers) is green at the wave-4 join and appended here. Mutation
  evidence is recorded. Hashes are logged.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
