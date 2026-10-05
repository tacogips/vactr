# Canvas Cutover: Per-Keystroke and Per-Frame Edit Cost Fix Implementation Plan

**Status**: In Progress (Step 6 implementation complete; pending independent review and post-join gates)
**Plan ID**: CANVAS-EVIDENCE-EDITCOST (wave 3, session 267; parallel with CANVAS-EVIDENCE-SILENT, -VIEWPORT, -RUNSTART)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.10 (session 269: bounded edit-path rule, document sync, syntax provider), #15.3.8.3 (text redraw only on dirty revisions; animation independent), #15.3.8.7 (no whole-text transfers per frame), #15.3.8.8 (input latency p95 <= 50 ms, text-dirty p95 <= 16.7 ms, animation-only p50 <= 4 ms), #15.3.8.9 (owner-file defect-fix rule)
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
- (Session 269: the former `sync.ts` exclusion is removed. `sync.ts`, `syntax.ts`,
  `syntax-core.ts`, `history.ts` and `surface.ts` are now owned here; see the Session 269
  Amendment.)
- No change to `language.ts`, `app/main.ts`, `frame.ts`, `view-host.ts` or any `bind/*`,
  `params/*` file.
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
- Session 269 operator authorization 1:
  - `editor/src/code/sync.ts`
  - `editor/src/code/syntax.ts`
  - `editor/src/code/syntax-core.ts`
  - `editor/src/code/history.ts`
  - `editor/src/code/surface.ts`
  - `editor/test/code/sync.test.ts`
  - `editor/test/code/syntax.test.ts`
  - `editor/test/code/syntax-core.test.ts`
  - `editor/test/code/syntax-fallback.test.ts`
  - `editor/test/code/history.test.ts` (the authorization named `editor/test/canvas/history.test.ts`
    "if it exists"; it does not, so the existing history test file is used)
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

## Session 269 Amendment (operator authorization 1; design 15.3.8.10)

This section supersedes the earlier text wherever they conflict. Design reference:
`design-docs/specs/design-implementation.md#15.3.8.10` (bounded edit-path rule, document sync,
syntax provider) and 15.3.8.2(c) (`parser.parse(input, oldTree)`).

### Intent

The work from session 267 (input, layout, accessibility, keyboard, mount, renderer) is kept as
is and is not redone. Two product causes remain, and they are why `edit-cost.test.ts` and
`mount.test.ts:168` still fail:

1. `DocumentSync.apply` (`editor/src/code/sync.ts:50-55`) converts the whole text on every
   keystroke. Both branches do it:
   - the identity-match branch calls `RevisionHistory.index(current)`, which calls
     `new Utf8Index(e.text.toString())` (`history.ts:152-162`), and every new revision is
     uncached;
   - the mismatch branch calls `new Utf8Index(tr.startState.doc.toString())`.
2. The syntax providers (`editor/src/code/syntax.ts:33-79`) compare and reparse whole-document
   strings. `syntax-core.ts:25-30` `pointAt` slices `text.slice(0, pos)`. `FallbackTokenizerCache`
   tokenizes the whole document after every change.

### TASK-101: Mutation baseline first (before any edit; not gating)

Before the first source edit, run the two failing counter tests on the unmodified source and
keep the log:

`cd editor && ./node_modules/.bin/vitest run test/canvas/edit-cost.test.ts test/canvas/mount.test.ts > ../tmp/canvas-cutover/evidence-editcost/logs/s269-mutation-prefix.log 2>&1; echo $? > ../tmp/canvas-cutover/evidence-editcost/logs/s269-mutation-prefix.exit`

Expected result: a nonzero exit, with failures on the `toString` or large-slice counters. Report it
as `mutationEvidence` (command, expected nonzero, actual exit, log path). Never list it in the
gating verification.

### TASK-102: Line-based byte conversion in `DocumentSync` (`sync.ts`, `history.ts`)

Contract (unchanged public API):

- `DocumentSync` constructor, `apply(tr)`, `onChange`, `mapWireSpan`, `toWireSpan`, `bind`,
  `file`, `revision` and `history` keep their signatures and results.
- `RevisionHistory` public members keep their signatures. `index(rev)` stays: `mapWireSpan` and
  `toWireSpan` still use it. That is off the edit path (design 15.3.8.10).
- `CodeSurface` (`surface.ts`) keeps its public API. Touch it only if a test shows that it calls
  `history.index` or converts whole text on dispatch. Expected: no change.

Implementation points:

- Add one module-private helper in `sync.ts`, for example
  `class LineBytes { static of(doc: Text): LineBytes; toByte(doc: Text, pos: number): number; update(changes: ChangeSet, base: Text, next: Text): void }`.
  - It stores one UTF-8 byte length per line (line breaks excluded) plus a prefix-sum array that
    is recomputed from the first changed line onward.
  - Numeric work linear in the line count is allowed. Reading text is not.
- Byte offset of `pos` in `doc` = (sum of the byte lengths of lines `1..n-1`) + (`n-1` bytes for the
  `\n` separators) + (UTF-8 bytes of `line.text` from 0 to `pos - line.from`), where
  `line = doc.lineAt(pos)`.
  - Count the line prefix by iterating `charCodeAt` over `line.text`. Never `slice` it: a single
    line can exceed 64 KiB, as in the 100,000-character line test.
- Match `Utf8Index.toByte` exactly (`editor/src/protocol/utf8.ts:38-70`):
  - 1 byte for code units below 0x80, 2 below 0x800, 4 for a valid surrogate pair, 3 otherwise
    (including a lone surrogate);
  - an offset that falls between the high and low halves of a pair maps to the pair's start byte.
- `apply(tr)`:
  - If `tr.startState.doc` is not the `Text` the table was built for (a history reset or revision
    gap), rebuild the table from `tr.startState.doc` lines (`doc.iterLines()` or `doc.line(n)`).
    Never call `toString`.
  - Compute every change's base byte `from`/`to` against the base table first, then build
    `changes`/`dirty` exactly as `Utf8Index.changes` does (`utf8.ts:103-121`): the same `delta`
    bookkeeping, and `insert_len` from `utf8Length` of the inserted text.
  - For the inserted text, read `inserted` per line (`inserted.iterLines()` or `line(n).text`) and
    sum the byte lengths, plus one per line break. Do not call `inserted.toString()` on large
    inserts.
  - Then update the table to `tr.newDoc`, replacing only the line range each change touches and
    working from the last change to the first.
  - If two changes in one change set touch the same or adjacent lines, rebuild the table from
    `tr.newDoc` lines instead.
  - Do not call `this.history.index(...)` inside `apply`.
- Keep the call order in `apply`: `doc.edit`, then `history.record`, then the listeners.
- Do not touch `editor/src/protocol/utf8.ts` (not owned). Its exported `utf8Length(s)`
  (`utf8.ts:20`) may be used on bounded strings such as one inserted line.

Tests (`editor/test/code/sync.test.ts`, imitating its existing rows; `history.test.ts` only if
history changes):

- Random edit sequences (seeded; at least 200 transactions mixing insert, delete, replace,
  multi-change sets and undo/redo through `CodeSurface`) on documents with ASCII, Japanese
  U+65E5 U+672C U+8A9E, emoji U+1F3B9 (surrogate pair), a 100,000-character line and empty lines
  -> each `doc-changed` byte change and dirty span equals the reference computed by
  `new Utf8Index(startDoc.toString()).changes(list)` in the test.
- A transaction whose start document is not the recorded current text (construct the
  `DocumentSync` with one `Text`, then apply a transaction built from a different `EditorState`)
  -> correct byte changes, and no `Text.prototype.toString` call on a document over 64 KiB
  (spy).
- A position inside a surrogate pair -> the same byte as `Utf8Index.toByte`.
- Existing `sync.test.ts` and `history.test.ts` rows -> unchanged and passing.

### TASK-103: Syntax provider without whole-document strings (`syntax.ts`, `syntax-core.ts`)

Contract change in `syntax-core.ts` (consumed only by `syntax.ts`, `mount.ts` and the three
syntax tests; `app/main.ts` uses `loadVactSyntax` only, and its signature is unchanged):

- `VactSyntax` keeps `parse(text: string): ParsedVact` and the optional `reparse`, for existing
  tests.
- It adds two required members:
  - `parseDoc(doc: Text, old: ParsedVact | null): ParsedVact`. This calls
    `parser.parse(read, oldTree)` with
    `read = (index) => index < doc.length ? doc.sliceString(index, Math.min(doc.length, index + 16384)) : undefined`.
    Chunks are at most 16 KiB. Pin web-tree-sitter 0.27.0 `ParseCallback`
    (`node_modules/web-tree-sitter/web-tree-sitter.d.ts:30,193`).
  - `edit(parsed: ParsedVact, edit: SyntaxTreeEdit): void`. It calls `tree.edit(new Edit(...))`
    on the parsed tree.
- `SyntaxTreeEdit` = `{ startIndex, oldEndIndex, newEndIndex, startPosition, oldEndPosition, newEndPosition }`,
  in the same UTF-16 index units as today's `treeEdits`.
- Add `treeEditFromDocs(base: Text, next: Text, fromA: number, toA: number, fromB: number, toB: number): SyntaxTreeEdit`.
  It computes points with `Text.lineAt` (`row = line.number - 1`, `column = pos - line.from`).
  Keep `treeEdits` and `pointAt` for the string path, which existing tests use.
- `ParsedVact.captures(from, to)` uses `query.captures(tree.rootNode, { startIndex: from, endIndex: to })`
  (`QueryOptions`, d.ts:751-765). The existing filter stays, so results are identical for the
  visible range.

`syntax.ts`:

- `SyntaxSpans`:
  - Keep `lastDoc: Text | null`.
  - `noteChanges(changes, state)` calls `syntax.edit(parsed, ...)` for each change, from the last
    to the first, using `lastDoc` as the base and `state.doc` as the next document, then sets
    `lastDoc = state.doc` and marks the provider dirty. With no parsed tree yet, it only
    records `lastDoc` and marks the provider dirty.
  - `spans(state, ...)`:
    - if `state.doc !== lastDoc` (a change was missed), do a full `parseDoc(state.doc, null)`;
    - else if dirty, call `parseDoc(state.doc, parsed)` and delete the old tree after success;
    - any thrown error falls back to `parseDoc(state.doc, null)`.
  - No `toString`, no string comparison and no `nextText`/`text` string fields.
- `FallbackTokenizerCache`:
  - Keep a per-line start-state array (`{ inString: boolean }` copies) valid up to line `k`.
  - `noteChanges` truncates it at `min(k, base.lineAt(fromA).number - 1)` over all changes.
  - `spans(state, from, to)` extends the start states from `k` to the first visible line
    (tokenizing each line's `line.text` with `LineStream`/`vactParser`, as `tokenizerSpans` does
    at `language.ts:170-185`), then tokenizes only the lines from `from` to `to`.
  - No `toString`, no string comparison and no whole-document `tokenizerSpans` call.
  - Results for any range must equal `tokenizerSpans(doc)` filtered to that range.
- `bounded()`, the 16,384 cap and `truncated` stay unchanged.

`mount.ts` (already owned): no change is expected. The call sites `syntaxProvider.spans(surface.state, from, to, 16384)`
and `noteChanges(update.changes, update.state)` stay.

Tests:

- `syntax-fallback.test.ts`:
  - 20,000-line document, visible range at line 10,000, then a one-character edit at
    line 10,001 and `spans` again -> spans equal `tokenizerSpans(doc)` filtered to the range;
    no `Text.prototype.toString` call; no `sliceString` over 64 KiB.
  - Opening a string on an earlier line changes later spans after truncation -> equals the
    reference.
- `syntax-core.test.ts`:
  - `treeEditFromDocs` equals `treeEdits(oldText, edits)` for ASCII, Japanese, emoji and
    multi-line inserts and deletes;
  - `parseDoc` on a 200,000-character document never requests a chunk over 16 KiB (wrap the
    `read` via a `Text` `sliceString` spy);
  - `parseDoc(doc, null)` captures equal `parse(doc.toString())` captures.
- `syntax.test.ts`:
  - port the stubbed `VactSyntax` fixtures to provide `parseDoc` and `edit`;
  - an edit sequence through `noteChanges` then `spans` -> the same spans as a fresh full parse;
  - a missed change (state doc not `lastDoc`) -> a full parse, not an error;
  - no `toString` of the document.
- Existing rows in all three syntax test files -> kept, with no assertion deleted.

### TASK-104: Gates (inside the sandbox; logs under `tmp/canvas-cutover/evidence-editcost/logs/`, each with an `.exit` file)

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/edit-cost.test.ts test/canvas/mount.test.ts` | exit 0 (edit-cost.test.ts:99 and mount.test.ts:168 pass) |
| `cd editor && ./node_modules/.bin/vitest run test/code/sync.test.ts test/code/history.test.ts test/code/syntax.test.ts test/code/syntax-core.test.ts test/code/syntax-fallback.test.ts` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/canvas` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/canvas/no-editor-view.test.ts` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0. Total at least 688 plus the new rows, with no assertion deleted. A failure only in a RUNSTART-owned file (`test/e2e/large-eval.test.ts`, `test/e2e/large-doc.test.ts`) is reported with its log path and not fixed here; the post-join run is authoritative. |
| `grep -rn "presentation\.text" editor/src` | no matches |
| `grep -n "toString()" editor/src/code/sync.ts editor/src/code/syntax.ts` | no matches (exit 1 from grep is the expected result; record the output) |

Outside the sandbox, by the verification step: full nextest (timeout >= 1500 s), strict clippy
and the wasm build, all with exit 0 and log paths. This plan touches no Rust.

### Pitfalls (session 269)

- Rebuilding a whole-text `Utf8Index` anywhere in `apply`, including via `history.index`.
- Calling `inserted.toString()` or `line.text.slice` on unbounded input.
- Surrogate pair splits and the `\n` byte between lines. The reference oracle must be
  `Utf8Index` on `doc.toString()` in the test, never a second copy of the new algorithm.
- Changing `doc-changed` bytes, revisions or edit epochs. `test/code/sync.test.ts` and the
  protocol tests must pass unchanged.
- Re-introducing string comparison for change detection in either provider.
- Returning tree-sitter chunks over 64 KiB, or returning `''` instead of `undefined` past the
  end. Follow the d.ts `ParseCallback` contract (`string | undefined`).
- Editing `language.ts`, `utf8.ts`, `bind/*`, `params/*` or `app/main.ts`. They are not owned.
- Growing any touched file past 1,000 lines.

## Completion Criteria

- [x] Session 269: `s269-mutation-prefix.log` recorded before edits with exit 1 and the expected three operation-counter failures (mutationEvidence; not gating)
- [x] Session 269: `DocumentSync.apply` has no `history.index` call and no whole-text conversion; 200 seeded edit transactions plus undo/redo match the `Utf8Index` byte-change and dirty-span oracle, including Unicode, emoji, empty lines and a 100,000-character line
- [x] Session 269: `SyntaxSpans` and `FallbackTokenizerCache` use `Text` identity, 16 KiB chunked `parseDoc` and line-local tokenizing; parse and fallback equivalence tests pass
- [x] Session 269: TASK-104 gates are recorded. Focused gates pass. Full vitest has one `test/e2e/large-eval.test.ts` threshold failure owned by CANVAS-EVIDENCE-RUNSTART; its plan requires the post-join full-suite run.
- [x] No whole-document `toString` on keystroke, presentation or animation-only frames; edit-cost/mount operation-counter tests pass and both source greps find no prohibited production calls
- [x] `TextLayout.setText` and `CanvasRenderer.setText` are in place and used by mount; the cache keeps unchanged lines
- [x] `surroundingWindow`, `readSelection` and word navigation/delete are line-local, with equivalence tests; focused edit-cost/mount tests pass
- [x] The renderer cache key uses `annotationsRevision` when given
- [x] Existing tests are ported without lost assertions; `npm run check` passes. Full vitest reports 695 passed and one RUNSTART-owned large-eval threshold failure; no EDITCOST test fails.

## Progress Log

### Session: 2026-10-05 (session 267 plan)
**Tasks Completed**: Plan authored; hot paths identified at `input.ts:61-66`, `mount.ts:143,170-199`, `layout.ts:28-45`, `accessibility.ts:22,76`, `keyboard.ts:45,74`, `renderer.ts:180`.

### Session: 2026-10-05 (Step 6 implementation)
**Tasks Completed**: Added Text-backed layout and renderer APIs; cached lazy `InputPresentation`; identity-based mount wiring and annotation revisions; bounded accessibility and line-local keyboard algorithms; ported composition assertion without dropping checks; added edit-cost, line-window/selection equivalence, cache-retention, renderer parity and annotation-revision tests. Ten animation-only mounted frames pass with no stringify, large slice or text rebuild.
**Verification**: `cd editor && npm run check` exit 0 (`tmp/canvas-cutover/evidence-editcost/logs/npm-check-final.log`); input/GPU tests 73/73 passed (`vitest-input-gpu.log`); no-EditorView test 1/1 passed (`vitest-no-editor-view.log`). Focused canvas: 164 passed, 3 failed; full vitest: 684 passed, 3 failed in 89 files (`vitest-full.log`). The failures are the required operation counters: document edit and word delete call `Text.toString` on a 1,148,889-character receiver through `editor/src/code/sync.ts:55`; mounted edit also performs a >64 KiB slice in fallback syntax work (`editor/src/code/syntax.ts`).
**Blocker (resolved in session 269)**: `sync.ts` and `syntax.ts` are not in this plan's declared `writePaths`; the plan explicitly excludes `sync.ts`, and the fanout write contract forbids editing either file. Resume after an authorized plan/write-path amendment assigns these product defects to an owner; then rerun the exact edit-cost and full vitest gates. No baseline comparison exists, so the failed aggregate is not a pass or baseline candidate.

### Session: 2026-10-05 (session 269 plan amendment)
**Tasks Completed**: Applied operator authorization 1 and design 15.3.8.10. Added the ten authorized writePaths (`editor/test/code/history.test.ts` replaces the non-existent `editor/test/canvas/history.test.ts`), removed the `sync.ts` non-goal, and added TASK-101 (pre-edit mutation baseline), TASK-102 (line-based byte conversion in `DocumentSync`), TASK-103 (Text-identity, chunked tree-sitter and line-local fallback syntax) and TASK-104 (gates). The manifest entry changed with it.

### Session: 2026-10-05 (Step 6 implementation, session 269)
**Tasks Completed**: Implemented cached per-line UTF-8 byte accounting in `sync.ts`; removed history index construction and full-document conversion from `DocumentSync.apply`; added seeded byte-equivalence coverage, a reset-identity operation-counter test, and explicit surrogate-interior mapping coverage. Reworked tree-sitter syntax tracking to use `Text` identity, transactional tree edits and chunked `parseDoc`; replaced fallback whole-document tokenization with cached per-line string state and visible-line tokenizing. Added large-document syntax equivalence and no-conversion tests. Existing canvas behavior and assertions remain in place.
**Verification**: Pre-edit mutation run exited 1 as expected (16 passed, 3 failed): `logs/s269-mutation-prefix.log` and `.exit`. Final focused edit-cost/mount: 19 passed (`logs/vitest-editcost-mount-final.log`, exit 0). Sync/history/syntax focused: 32 passed (`logs/vitest-code-final.log`, exit 0). Canvas suite: 167 passed (`logs/vitest-canvas-final-step6.log`, exit 0). `npm run check` and no-EditorView: exit 0 (`logs/npm-check-final-step6.log`, `logs/vitest-no-editor-view-final-step6.log`). Full vitest: 695 passed, 1 failed (`logs/vitest-full-final-step6.log`, exit 1); the sole failure is `test/e2e/large-eval.test.ts`, the downstream RUNSTART-owned 5,000 ms threshold (measured median 8,267.3 ms, focused 5,000-line median 1,947.0 ms). No EDITCOST-owned test fails. `grep -n "toString()" editor/src/code/sync.ts editor/src/code/syntax.ts` and `grep -rn "presentation.text" editor/src` produced no matches (expected grep exit 1).
**Compatibility note**: The installed web-tree-sitter 0.27.0 range query produced incomplete captures when `startIndex`/`endIndex` were supplied alongside the current UTF-16 offsets. The provider uses the equivalent `startPosition`/`endPosition` QueryOptions range, with UTF-16 row/column points; targeted indexed-option attempts and the passing position-range suite are retained in `logs/syntax-iter5.log` and `logs/syntax-iter6.log`.
