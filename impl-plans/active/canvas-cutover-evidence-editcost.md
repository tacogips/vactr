# Canvas Cutover: Per-Keystroke and Per-Frame Edit Cost Fix Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-EVIDENCE-EDITCOST (wave 3, session 267; parallel with CANVAS-EVIDENCE-SILENT, -VIEWPORT, -RUNSTART)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.3 (text redraw only on dirty revisions; animation independent), #15.3.8.7 (no whole-text transfers per frame), #15.3.8.8 (input latency p95 <= 50 ms, text-dirty p95 <= 16.7 ms, animation-only p50 <= 4 ms), #15.3.8.9 (owner-file defect-fix rule)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-EVIDENCE-EDITCOST`)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

run-001 on the 20,000-line, 1 MiB document measured:

| Browser | Text-dirty work p95 | Input latency |
|---------|---------------------|---------------|
| Chromium | 32.8 ms | p99 250 ms |
| WebKit | 34 ms | p95 265 ms |

Only 253 and 187 editing keys fitted in 60 s. These are product costs, and design 15.3.8.9 says
they must be fixed in the owning source. The cause is whole-document string work on every
keystroke, and even on every animation frame. All sites below were verified at plan time:

1. `editor/src/code/input.ts:61-66`: the `presentation` getter calls
   `this.surface.state.doc.toString()` on every access. `mount.ts` reads it up to four times per
   frame, including animation-only frames (`mount.ts:172,189,191,199`). `publish()` calls it on
   every surface update (`input.ts:54-58,70`).
2. `editor/src/code/mount.ts:143`: compares two 1 MiB strings and then calls
   `layout.setDocument(displayText)`. `mount.ts:170-175` calls `layout.setDocument` and
   `renderer.setDocument` again on the text-dirty frame.
3. `editor/src/code/layout.ts:28-45`: `setDocument` rescans the whole string with a regex and
   allocates about 20,000 line objects per edit. It also compares cached lines with
   `slice` on both texts.
4. `editor/src/code/accessibility.ts:21-30` (`surroundingWindow`) and `:76` (`readSelection`)
   call `doc.toString()` on every refresh and selection read.
5. `editor/src/code/keyboard.ts:45` and `:74` call `doc.toString()` for word navigation and word
   delete.
6. `editor/src/code/renderer.ts:180`: the draw-cache key `JSON.stringify`s every static
   annotation (up to 16,384 syntax spans) on every rendered frame, including animation-only
   frames.

These files are owned by the accepted CANVAS-RENDER and CANVAS-MOUNT plans. This plan is the
authorized owner-file fix (design 15.3.8.9).

## Non-goals

- No change to `frame.ts` (CANVAS-EVIDENCE-VIEWPORT) or `view-host.ts`.
- No change to `sync.ts`. Its `toString` (line 55) runs only after a history reset, and the
  `doc-changed` delta path is already bounded and tested (15.3.8.7).
- No change to visible behavior, the IME model, grapheme rules, the 8,192-unit input window
  (`INPUT_WINDOW_LIMIT`) or resource caps.
- No new dependency. Use `Text` from `@codemirror/state`, which is already the document type.
- No wall-clock assertions in jsdom tests. Use operation counters.

## Ownership

writePaths:

- `editor/src/code/input.ts`
- `editor/src/code/layout.ts`
- `editor/src/code/accessibility.ts`
- `editor/src/code/keyboard.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/renderer.ts`
- `editor/test/canvas/input.test.ts`
- `editor/test/canvas/gpu.test.ts`
- `editor/test/canvas/mount.test.ts`
- `editor/test/canvas/edit-cost.test.ts` (new)
- this plan file
- `tmp/canvas-cutover/evidence-editcost/intent.json` and `tmp/canvas-cutover/evidence-editcost/receipt.json`
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm` (setup builds only), `editor/node_modules/.vite`, `tmp/canvas-cutover/evidence-editcost/logs`

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`, then
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.

sharedPaths: none.

## Contracts (signatures other code relies on)

- `TextLayout`:
  - Add `setText(doc: Text): void` (from `@codemirror/state`). Keep `setDocument(text: string)`
    as a thin wrapper that converts with `Text.of(text.split(/\r\n|\r|\n/))`. Note that
    `TextLayout` must keep CRLF and CR semantics identical to today. If `Text` normalizes line
    breaks differently, keep the existing string path for `setDocument` and add the `Text` path
    alongside; tests decide.
  - Internally, line lookup uses the stored `Text` (`doc.line(n + 1)`, `doc.lineAt(pos)`,
    `doc.sliceString(from, to)`, `doc.lines`) instead of a full line array and `source`
    string.
  - `get document(): string` stays. It is computed lazily and cached per `Text` identity. Only
    `saveText()` and tests call it.
  - `lineCount`, `shape`, `visible`, `boundary`, `advance`, `offsetInRun`, `coordsAtPos`,
    `posAtCoords` and `rangeRects` keep their signatures and results.
  - The cache-retention rule stays: a cached line survives only if its line number and its text
    are unchanged. Compare only the cached entries (at most the cache), never the whole document.
- `InputPresentation`: becomes `{ doc: Text; cursor: number; annotations: readonly CodeAnnotation[]; readonly text: string }`.
  - `text` is a lazy getter (`doc.toString()`), kept for tests only.
  - When not composing, `doc` is `surface.state.doc` itself (identity).
  - When composing, `doc` is `surface.state.doc.replace(range.from, range.to, Text.of(preedit.split('\n')))`.
  - The presentation object is cached and rebuilt only when the doc identity, composition range,
    preedit, composing flag or selection head changes.
- `CanvasRenderer`:
  - Add `setText(doc: Text): void`, mirroring `setDocument`. Keep `setDocument(text)`.
  - `RenderFeedback` gains optional `annotationsRevision?: number`. When it is present, the
    draw-cache key uses it in place of the stringified static annotations. The key also keeps
    `textRevision`, view, scale, font generation, atlas evictions, cursor and handles.
    Callers that omit it keep today's behavior.
- `surroundingWindow(surface)`: same signature and same returned `InputWindow` values. It must
  slice only a bounded region of `doc`: from the start of the line containing
  `head - INPUT_WINDOW_LIMIT/2`, clamped to `head - INPUT_WINDOW_LIMIT`, to the matching bound
  after the head. Then apply the existing `boundary` logic to that local slice with an offset.
  `readSelection` does the same around the window.
- `KeyboardController` word navigation and word delete: operate on the text of the line(s)
  involved (`doc.lineAt(head)`, plus the adjacent line when crossing a line break) instead of
  `doc.toString()`. Results must be identical.

## Mount wiring (`mount.ts`)

- `onPresentation`: compare `presentation.doc` by identity with the last displayed `Text`. On
  change, call `layout.setText(doc)`, mark display dirty and invalidate text. Never compare
  strings.
- Text-dirty frame: use the cached presentation. Call `renderer.setText(presentation.doc)` only
  when the doc identity changed. Do not call `layout.setDocument` a second time; the renderer
  owns that call through its layout.
- Animation-only frames must not touch `presentation.text` or any whole-document string. Read
  `cursor` and `annotations` from the cached presentation.
- Keep a numeric `staticRevision` that increments whenever `staticAnnotations` is recomputed, and
  pass it as `annotationsRevision`.
- Keep every other line of `mount.ts` unchanged. `mount.ts` must stay under 1,000 lines (it is
  288 today).

## Tests (situation -> expected)

`editor/test/canvas/edit-cost.test.ts` (new; jsdom; imitate the `mount.test.ts` fake GL and deps
setup by copying the minimal helpers). Use a 20,000-line document with about 52 characters per
line, built inline; it does not need the e2e fixture.

- Spy on `Text.prototype.toString`. Also wrap `sliceString` and count calls whose range covers
  more than 64 KiB.
- Mount the document and let the first text frame finish. Then reset the counters and dispatch a
  one-character insert through `InputController.replaceSelection('x', 'input.type')`, then run
  one frame -> `toString` count 0 and large-slice count 0.
- Run 10 animation-only frames (playing owner active, no doc change) -> `toString` count 0,
  large-slice count 0, and `renderer.stats.textBuilds` unchanged.
- Word navigation (`Ctrl+ArrowRight`) and word delete (`Ctrl+Backspace` or the platform
  equivalent used in `keyboard.ts`) on the large document -> `toString` count 0 and the same
  resulting selection or document as a reference computed on the small-document path.
- After the one-character edit at line 10,000, `layout` shaped-cache entries for lines 0-9,999
  are retained (`layout.stats.builds` does not grow when re-shaping a cached line from before
  the edit).

Existing tests (port, do not delete assertions):

- `input.test.ts:62`: `toEqual({ text, cursor, annotations })` becomes explicit field assertions
  (`text`, `cursor`, `annotations`) plus `doc.toString()` equality. `:101` keeps its `text`
  assertion through the lazy getter.
- `gpu.test.ts`: add rows. `setText(Text)` and `setDocument(string)` render the same draw
  commands. CRLF and CR documents keep the same `lineCount` and `coordsAtPos`. With
  `annotationsRevision` present, an unchanged revision reuses the cache (no `textBuilds`
  increment) and a changed revision rebuilds.
- `input.test.ts`: `surroundingWindow` on a document with the caret in the middle of a
  100,000-character single line and on a 20,000-line document returns the same window as the
  previous whole-string implementation. Keep a private reference copy of the old algorithm
  inside the test to compare.
- `mount.test.ts`: every existing assertion passes unchanged.

## Pitfalls

- Making `presentation.text` the hot path again by reading it anywhere in production code. Grep
  must show no `presentation.text` outside tests.
- Changing CRLF handling. `TextLayout` treats `\r\n`, `\r` and `\n` as breaks today; `Text.of`
  takes pre-split lines. Normalize identically, and keep the tests that cover this.
- Breaking the IME rule that preedit never changes the document. The composing presentation is
  a derived `Text`, never dispatched.
- Letting the draw-cache key ignore annotation changes. Callers that omit `annotationsRevision`
  must keep the stringified key.
- Growing any touched file past 1,000 lines. Split it if needed (none is close today).

## Verification (inside the sandbox; logs under `tmp/canvas-cutover/evidence-editcost/logs/`)

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas` | all pass, including `edit-cost.test.ts` |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | all pass; total >= 665 plus the new tests; no assertion deleted |
| `cd editor && ./node_modules/.bin/vitest run test/canvas/no-editor-view.test.ts` | pass |
| `grep -rn "presentation\.text" editor/src` | no matches |

Mutation evidence (separate list): `edit-cost.test.ts` run against the pre-change `input.ts`
must fail on the `toString` count (expected nonzero exit), recorded with the log path.

Outside the sandbox (wave 4): text-dirty work p95 and input latency are re-measured. A remaining
miss is triaged by the wave-4 review: a product cost is repaired in this plan's files, and a
software-GL environment limitation is recorded, never relabeled.

## Overwrite and Drift Protocol

Record the sha256 before and after each edit in `intent.json` and `receipt.json`. If a file
drifted before your first edit, stop and report it. Edit only this plan's progress log.

## Completion Criteria

- [ ] No whole-document `toString` on keystroke, presentation or animation-only frames (counter tests pass)
- [ ] `TextLayout.setText` and `CanvasRenderer.setText` are in place and used by mount; the cache keeps unchanged lines
- [ ] `surroundingWindow`, `readSelection` and word navigation/delete are line-local, with equivalence tests
- [ ] The renderer cache key uses `annotationsRevision` when given
- [ ] Existing tests are ported without lost assertions; `npm run check` and full vitest pass

## Progress Log

### Session: 2026-10-05 (session 267 plan)
**Tasks Completed**: Plan authored; hot paths identified at `input.ts:61-66`, `mount.ts:143,170-199`, `layout.ts:28-45`, `accessibility.ts:22,76`, `keyboard.ts:45,74`, `renderer.ts:180`.
