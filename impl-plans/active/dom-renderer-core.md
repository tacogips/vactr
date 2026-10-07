# DOM-RENDERER-CORE: Renderer Interface, DOM Renderer, Overlays and Unit Tests Implementation Plan

**Status**: Ready
**Plan ID**: DOM-RENDERER-CORE (wave 1 of 4, serial chain)
**Design Reference**: `design-docs/specs/design-dom-renderer.md`. Cite the document's own numbering: DR-1 (interface), DR-3 (tree, coordinates, CSS rules), DR-4 (window, keys, bounds), DR-5 (frame discipline), DR-6 (highlights), DR-7 (stats) and DR-8 (unit tests).
**Depends On**: none
**Next**: `impl-plans/active/dom-renderer-mount.md`
**Created**: 2026-10-07
**Last Updated**: 2026-10-07

---

## Intent and Context

The user asked for an HTML (DOM) version of the canvas code editor, so that its performance can
be compared with the canvas editor in the same app. This plan builds only the presentation
backend. A later plan (DOM-RENDERER-MOUNT) wires it into `mount.ts`.

Repository facts:

- The mount drives the canvas renderer only through `setText(doc)`, `setViewport(view, dpr)`,
  `render(feedback)`, `textPending`, `setPalette`, `setPhases`, `dispose`, `status`, `stats` and
  `layout` (`editor/src/code/mount.ts:84-88,205,234-245,248,260,331,337`;
  `editor/src/code/perf-hook.ts:56-59`).
- `TextLayout` (`editor/src/code/layout.ts`) is the only geometry authority. It provides
  `shape(n)`, `advance(line, pos)`, `coordsAtPos`, `rangeRects`, `boundary`,
  `geometrySegments(line)` (256-cluster chunks) and `lineCount`. `ShapedRun = {text, from, to, x,
  width}`. Runs are split at tabs (tab stop = 4 spaces), so tabs never appear inside a run.
  Unchanged lines keep their `ShapedLine.runs` array identity across edits (`layout.ts:108-145`).
- Annotation kinds are `syntax | selection | playing | eval | diagnostic | binding | composition |
  call-head` (`editor/src/app/apis.ts:38-42`). Syntax rows carry `className` `vact-tok-*`.
- The canvas token-to-CSS-variable map is `palette.ts:16-25` (`TOKEN_PROPERTIES`; `path` maps to
  `--vt-syn-string`).

## Non-goals

- Do not edit `renderer.ts`, `layout.ts`, `frame.ts`, `view-host.ts`, `input.ts`, `pointer.ts`,
  `palette.ts`, `code.css`, `theme.css` or `mount.ts`. Do not edit any canvas test.
- No CSS Custom Highlight API, no `contenteditable`, no Worker and no native scrolling.
- No text measurement in the DOM renderer. It must never call `measureText`, read layout, or
  call `getComputedStyle`.
- No product toggle UI and no `?renderer` URL parsing at mount (that is the MOUNT plan; this plan
  only provides the pure `selectRendererKind`).

## Ownership

writePaths:

- `editor/src/code/renderer-types.ts` (new)
- `editor/src/code/dom-renderer.ts` (new)
- `editor/src/code/dom-overlay.ts` (new)
- `editor/src/code/dom-renderer.css` (new)
- `editor/test/code/dom-renderer.test.ts` (new)
- `impl-plans/active/dom-renderer-core.md` (checkboxes and progress log only)
- `tmp/dom-renderer/core` (artifact root: logs and hash receipts)
- `editor/node_modules/.vite` (artifact root: vitest cache)

sharedPaths: none.

## Contracts (pinned; later plans depend on these exact names)

### `editor/src/code/renderer-types.ts`

- `export type RendererKind = 'canvas' | 'dom';`
- `export interface CodeRenderer`. Its members are:
  - `readonly layout: TextLayout`
  - `readonly stats: Readonly<Record<string, number>>`
  - `readonly status: GpuStatus`
  - `readonly textPending: boolean`
  - `setText(doc: Text): void`
  - `setViewport(view: LayoutViewport, dpr?: number): void`
  - `render(feedback?: RenderFeedback): boolean`
  - `setPalette(palette: Palette): void`
  - `setPhases(phases: PhaseTimer | null): void`
  - `dispose(): void`

  Use type-only imports. `GpuStatus` and `RenderFeedback` come from `./renderer`,
  `LayoutViewport` and `TextLayout` from `./layout`, `Palette` from `./palette`, `PhaseTimer`
  from `./frame` and `Text` from `@codemirror/state`.
- `export function selectRendererKind(search: string, requested?: RendererKind): RendererKind`.
  Precedence: `requested` if it is given; otherwise `new URLSearchParams(search).get('renderer')
  === 'dom'` yields `'dom'`; otherwise `'canvas'`. A `requested` value of `'canvas'` overrides a
  URL `renderer=dom`.
- `CanvasRenderer` must satisfy `CodeRenderer` structurally without edits. Prove it with a
  compile-time assertion in the test (see Tests). Do not add `implements` to `renderer.ts`.

### `editor/src/code/dom-renderer.ts`

- `export const DOM_RENDERER_LIMITS = Object.freeze({ maxLines: 1024, maxSpansPerLine: 512,
  maxOverlayElements: 1024, maxHighlightKeys: 512 });`
- `export function domWindowMargin(rows: number): number` returns `max(8, ceil(rows / 2))`.
- `export interface DomRendererOptions { onStatus?: (status: GpuStatus) => void }`
- `export class DomRenderer implements CodeRenderer` with
  `constructor(readonly element: HTMLElement, readonly layout: TextLayout, options?: DomRendererOptions)`.
- `stats` is a plain object with exactly these numeric fields, all starting at 0: `frames`,
  `lineBuilds`, `lineMoves`, `lineReuses`, `lineReleases`, `gutterWrites`, `overlayWrites`,
  `caretWrites`, `highlightToggles`, `highlightBuilds`, `highlightDropped`, `transformWrites`,
  `windowShifts`, `windowFrom`, `windowTo`, `liveLines`, `liveNodes`, `peakNodes`,
  `spansMerged`, `overlayDropped`.
- `status` is `{ kind: 'ready', message: 'DOM renderer', effectiveDpr: <last dpr>, saveText: () =>
  layout.document }`. `onStatus` fires on construction and whenever `effectiveDpr` changes.
- `textPending` always returns `false`.
- `setPalette(p)` stores `p` only. It performs no DOM write.
- `setPhases(phases)` sets `this.layout.phases = phases`, exactly like `renderer.ts:121`, so
  `TextLayout` shaping spans are recorded in DOM mode. It performs no DOM write.
- `setViewport(view, dpr = 1)` performs the same validation as `renderer.ts:126` (throw
  `RangeError('Invalid viewport')`). It stores the values. It performs no DOM write.
- `setText(doc)` calls `layout.setText(doc)` only when `doc !== ` the last doc, bumps an internal
  text revision, and marks text dirty. The mount calls `layout.setText(presentation.doc,
  changes)` itself before `renderer.setText`; `TextLayout.setText` returns early for the same doc,
  so identity is preserved.

### Element tree (DR-3); class names are a contract for the harness plan

- The root is `element`. The constructor adds class `vact-code-dom` and sets
  `aria-hidden="true"`.
- Children are `div.vact-dom-gutter > div.vact-dom-gutter-rows > div.vact-dom-num*` and
  `div.vact-dom-text > div.vact-dom-content`. `.vact-dom-content` contains, in this order,
  `div.vact-dom-sel`, `div.vact-dom-hl`, `div.vact-dom-lines > div.vact-dom-line*`,
  `div.vact-dom-under`, `div.vact-dom-labels`, `div.vact-dom-caret` and two
  `div.vact-dom-handle`.
- Inside a line: `span.vact-dom-run` (absolutely positioned at its layout x), then
  `span.vact-dom-tok-<name>` for styled text, and bare text nodes for unstyled text.
- The token class map is a frozen object with these entries:
  - `vact-tok-comment -> vact-dom-tok-comment`
  - `directive`, `keyword`, `number`, `string`, `path`, `head` and `bracket` map the same way
  - Unknown classes have no class and render as plain text.
- Highlight rect elements have class `vact-dom-hlr` plus a kind class (`vact-dom-playing`,
  `vact-dom-flash` or `vact-dom-flash-error`). The visible state is the class `on`.
- Underline bars have class `vact-dom-bar` plus `vact-dom-diag`, `vact-dom-comp` or
  `vact-dom-call`. Labels are `vact-dom-label`. Selection rects are `vact-dom-selr`.

### `editor/src/code/dom-overlay.ts`

Internal helper used only by `dom-renderer.ts`. It owns pooled overlay elements: selection rects,
underline bars, labels, caret, handles and highlight rects. Suggested export:
`export class DomOverlays`, with methods that take the content-coordinate viewport and the rows
for each layer, and a `dispose()`. The exact methods are free, but every write goes through a
guard that skips writing an unchanged value, and every write increments the matching `stats`
counter.

## Key Points (what a careless implementation gets wrong)

1. **Never use `innerHTML`, `insertAdjacentHTML` or `outerHTML`.** Document text and binding
   labels are user content. Use `textContent` / `createTextNode` only. A test asserts that
   markup-like text such as `<b>x</b>` renders literally.
2. **Content coordinates.** Overlay geometry comes from `TextLayout` with
   `contentView = { left: 0, top: W0*lh, gutter: 0, scrollLeft: 0, scrollTop: W0*lh,
   height: (W1 - W0)*lh, width: 2 ** 24 }`. With this view, `coordsAtPos` and `rangeRects`
   return document coordinates (`top = n*lh`, `left = advance`). Do not pass the mount's
   viewport to `rangeRects`. That would clamp to the visible rows and break the transform-only
   scroll rule.
3. **Scroll is two transforms.** `.vact-dom-content` gets `translate3d(-scrollLeft, -scrollTop,
   0)` (pixel values) and `.vact-dom-gutter-rows` gets `translate3d(0, -scrollTop, 0)`. Write
   each only when its value changed. The gutter width is `view.gutter ?? 48`. The text clip
   starts at that x.
4. **Window rule (DR-4).** `rows = ceil(view.height / lh)`. `v0 = floor(scrollTop / lh)` and
   `v1 = min(lineCount, ceil((scrollTop + height) / lh))`. Keep `W` while `v0 >= W0 && v1 <= W1`.
   Otherwise set `W = [max(0, v0 - margin), min(lineCount, v1 + margin)]`, capped so that
   `W1 - W0 <= maxLines`. Update `stats.windowFrom` and `stats.windowTo`. When the line count
   shrinks below `W1`, clamp.
5. **Line keys.** Use a `WeakMap<readonly ShapedRun[], LineEntry>`. A `LineEntry` holds the
   element, the number it is positioned at, a signature string and an in-window flag. The
   signature is the font generation, the syntax class runs for that line (relative
   offset:length:class), and, for multi-chunk lines, the visible chunk indices.
   - Equal signature: write `transform: translateY(n*lh px)` only if the number changed
     (`lineMoves`). Otherwise there is no write (`lineReuses`).
   - Different signature: rebuild the children with one `replaceChildren(...)` (`lineBuilds`).
   - Miss: reuse an element from the free pool, or create one, and build it.
   - Leaving the window: `remove()` and push to the free pool (cap `|W|`; extra elements are
     dropped) (`lineReleases`).
   - Bucket syntax rows per line in one O(spans) pass over the sorted rows. Do not filter all
     spans per line, which is O(lines x spans).
   - Skip the per-line signature pass when the textRevision, the window and a syntax key string
     (as in canvas `renderer.ts:216`) are all unchanged.
6. **Run pieces.** For each line, use `layout.geometrySegments(line)`. A single-chunk line
   renders all pieces. A multi-chunk line renders only chunks whose x span intersects
   `[scrollLeft - width, scrollLeft + 2*width]` (the canvas rule, `renderer.ts:370-374`). Each
   piece becomes `span.vact-dom-run` with `left: <piece.x>px`. Split the piece text at syntax
   boundaries (clamped to the piece range). There are at most `maxSpansPerLine` token spans per
   line. Merge the remainder into plain text and add to `spansMerged`.
7. **Caret move writes no line nodes.** When only the cursor, selection, handles or
   `annotationsRevision` changed (same textRevision, window and syntax key), write only the caret,
   handle and selection elements. Selection rows are `kind === 'selection'` in
   `feedback.annotations`. The caret is `feedback.cursor` with `cursorVisible !== false`,
   positioned with `coordsAtPos`. It is 1 px wide, one line tall, colored `--vt-text`. Each
   handle is 10x10 px at the caret top (start) or bottom (end), offset like `renderer.ts:272-276`.
8. **Animation-only frame.** Animated rows are `feedback.animated` plus any `playing` or `eval`
   in `feedback.annotations`. The key is `${kind}:${className ?? ''}:${from}-${to}`. Keep a
   `Map<key, {els, on, geomRevision}>`.
   - Turning a key on whose geometry is current: `classList.toggle('on', true)` only if it was
     off (`highlightToggles`).
   - A new key, or stale geometry: create or reposition its rects inside `.vact-dom-hl`
     (`highlightBuilds`).
   - Keys not in the set: toggle off once.
   - LRU-evict keys that are off beyond `maxHighlightKeys`. Count keys that are on beyond the
     cap in `highlightDropped`.
   - **Pitfall:** `classList.add` on an already present class still queues a mutation record.
     Track state and skip no-op toggles.
9. **Underlines and labels.** `diagnostic` and `composition` rows draw 2 px bars at the rect
   bottom. `call-head` rows draw a 1 px bar. `binding` rows with `label` draw a label element at
   `coordsAtPos(to)`, positioned like `renderer.ts:262-269`. Truncate labels over 64 chars to 61
   plus an ellipsis. Width is `min(256, max(16, len*8+8))`. Cap each layer at
   `maxOverlayElements` and count the excess in `overlayDropped`.
10. **Write order and reads.** One synchronous pass, in this order: window and lines, gutter,
    static overlays, caret and handles, highlights, transforms. Do not read any layout property.
    Do not call `getBoundingClientRect` or `getComputedStyle`.
11. **Font.** Set `font` and `line-height` on `.vact-dom-content` and `.vact-dom-gutter-rows`
    from `layout.font` at construction, and again only when `layout.font` changes. Listen to
    `document.fonts` `loadingdone` (if present) exactly like `renderer.ts:103-107`: bump the
    generation through `layout.setFont`, which rebuilds the window lines. Remove the listener on
    dispose.
12. **DPR.** `setViewport(view, dpr)` with a new dpr updates `status.effectiveDpr` and fires
    `onStatus`. It must not call `layout.invalidate()` and must not rebuild lines.
13. **Node accounting.** `liveNodes` counts renderer-created elements and text nodes currently
    attached. Update it incrementally on build, release, create and remove. Never walk the tree
    to count. `peakNodes` is the running maximum. `liveLines` is the line-element count in the
    window.
14. **Stylesheet.** Link `dom-renderer.css` once per document, using the `mount.ts:52-57`
    pattern (`new URL('./dom-renderer.css', import.meta.url).href`, marker
    `data-vact-dom-css`). Keep the link on dispose.
15. **Dispose.** Remove all renderer children, clear the maps and pools, reset `liveLines`,
    `liveNodes` and the window to 0, and remove the font listener. It is idempotent. After
    dispose, `render` returns `false`.
16. **File size.** Each `.ts` file stays under 600 lines (policy limit 1000).
17. **`setPhases` is not a no-op.** The mount hands the perf `PhaseTimer` to the layout only
    through `renderer.setPhases` (`mount.ts:260`). `TextLayout` records the `shaping` phase only
    through `layout.phases` (`layout.ts:302,350`). Phase spans are exclusive (`frame.ts:18-45`).
    A no-op or store-only `setPhases` would hide DOM shaping inside `upload`, `caret` and
    `input`, and skew the canvas-vs-DOM `phaseMs` comparison (design DR-7, DR-10). Do not
    "fix" this in `mount.ts`.

## CSS (`dom-renderer.css`)

- Root `.vact-code-dom`:
  - `position: relative; z-index: 1; display: block; width: 100%; height: 100%;
    overflow: hidden; contain: strict`
  - `background: var(--vt-dom-code-bg)`, with `--vt-dom-code-bg: rgb(10 13 18 / 0.85)` declared
    on the root
  - `user-select: none; -webkit-user-select: none; -webkit-touch-callout: none;
    touch-action: none; cursor: text`
- `.vact-dom-gutter` is absolute, `left: 0`, `width: 48px`, `overflow: hidden`, colored
  `--vt-syn-comment`. `.vact-dom-text` is absolute, `left: 48px; right: 0; top: 0; bottom: 0;
  overflow: hidden`.
- `.vact-dom-content` and `.vact-dom-gutter-rows` are absolute at `0,0` with
  `will-change: transform`.
- `.vact-dom-line` and `.vact-dom-num` use `position: absolute; left: 0; top: 0;
  white-space: pre; contain: layout style`.
- `.vact-dom-num` has `padding-left: 4px`. `.vact-dom-run` is `position: absolute; top: 0`.
- Token classes set only `color: var(--vt-syn-*)`, with `path` mapped to `--vt-syn-string`.
- Selection uses `--vt-selection`.
- Highlights: `.vact-dom-hlr { position: absolute; visibility: hidden }` and
  `.vact-dom-hlr.on { visibility: visible }`, with backgrounds `--vt-playing-bg`, `--vt-flash`
  and `--vt-flash-error`.
- Bars: diag `--vt-danger`, comp `--vt-data-1`, call
  `color-mix(in srgb, var(--vt-text-muted) 60%, transparent)`.
- Caret: `--vt-text`. Handles: `--vt-data-1`. Labels: background `--vt-raised`, color
  `--vt-syn-head`.
- **Forbidden:** `opacity`, `filter`, `:has(`, `box-shadow` and any non-zero `border-radius`
  (the WebKit diagnosis in design 15.3.8.14 and the UI style rule).

## Patterns to imitate

- `editor/src/code/renderer.ts:CanvasRenderer.render` for feedback parsing, `styleHash` (:353)
  for the syntax signature, and `renderer.ts:370-374` for horizontal chunk culling.
- `editor/src/code/mount.ts:addStylesheet` for the stylesheet link.
- `editor/test/canvas/mount.test.ts:setup` for a jsdom harness, and `mount.ts:81-82` for a
  `TextLayout` with a fixed-advance fake (`measureText: (t) => ({ width: t.length * 8 })`,
  `lineHeight 18`).

## Tests (`editor/test/code/dom-renderer.test.ts`, jsdom)

Build `TextLayout` with the fixed-advance fake. Use a 900x900 viewport (`rows = 50`, margin 25),
`gutter 48`. Use `new MutationObserver(() => {})` with `observe(root, {subtree: true,
childList: true, attributes: true, characterData: true})` and `takeRecords()` around each
action.

- **Type check:** `type _C = CanvasRenderer extends CodeRenderer ? true : never;` with a `const`
  of that type, so `npm run check` fails if the canvas renderer drifts.
- `selectRendererKind('', undefined)` -> `'canvas'`; `('?renderer=dom')` -> `'dom'`;
  `('?renderer=bogus')` -> `'canvas'`; `('?renderer=dom', 'canvas')` -> `'canvas'`;
  `('', 'dom')` -> `'dom'`.
- **Virtualization:** 20,000-line text, first render -> `liveLines <= 50 + 2*25`, gutter
  `.vact-dom-num` count `<= windowTo - windowFrom`, all gutter numbers in
  `[windowFrom + 1, windowTo]`. Scroll to the middle and the end -> same bounds,
  `windowShifts` increments, and `peakNodes` stays below a fixed bound computed from the
  limits.
- **Scroll inside the window** (scrollTop + 18) -> the records are exactly 2 attribute (`style`)
  records, on `.vact-dom-content` and `.vact-dom-gutter-rows`. `lineBuilds` delta is 0.
- **Single-char edit on visible line k** (new doc via `Text.replace`, `layout.setText(doc,
  changes)` then `setText(doc)` then `render`) -> `lineBuilds` delta 1, and there are no
  `childList` records targeting any other `.vact-dom-line` subtree.
- **Newline inserted above the viewport** -> `lineBuilds` delta <= 1. Shifted lines produce only
  `style` attribute records (`lineMoves` > 0). Gutter writes <= 1 + window delta.
- **Caret move** (cursor changes, `annotationsRevision` + 1, same textRevision) -> no record
  whose target is inside `.vact-dom-lines` or `.vact-dom-gutter`. `lineBuilds` and `lineMoves`
  deltas are 0. At least one record targets `.vact-dom-caret`.
- **Animation-only frame** (same text, annotations and viewport; new `animated` with 3 playing
  ranges):
  - first time -> every record's target is inside `.vact-dom-hl`;
  - then off, then the same 3 on again -> every record is `attributes` with
    `attributeName === 'class'` inside `.vact-dom-hl`, and `highlightBuilds` delta is 0.
  - Both branches go in one test.
- **Highlight cap:** 600 distinct ranges on -> `highlightDropped > 0` and highlight keys
  <= 512.
- **IME:** a doc whose line 0 contains preedit `にほんご`, plus a `composition` annotation over
  it and `cursor` at its end -> the line text includes the preedit, one `.vact-dom-comp` bar
  whose left/top equal `layout.coordsAtPos` in content coordinates, and the caret at the
  cursor. After commit (no composition row) -> zero visible `.vact-dom-comp` bars.
- **Selection over 3 lines** -> 3 `.vact-dom-selr` elements matching `layout.rangeRects` with
  the content view. Changing the selection -> records only inside `.vact-dom-sel`.
- **No layout reads:** `vi.spyOn(Element.prototype, 'getBoundingClientRect')` and
  `vi.spyOn(window, 'getComputedStyle')` -> 0 calls across construction plus 10 renders.
- **Long line** (5,000 ASCII chars) -> only chunks intersecting the horizontal window are
  rendered. A horizontal scroll by 2 viewport widths -> `lineBuilds` delta 1, for that line
  only.
- **Tokens:** a `vact-tok-keyword` row -> `span.vact-dom-tok-keyword`. Unknown class -> plain
  text. 600 token rows on one line -> `spansMerged > 0` and at most 512 token spans.
- **Literal text:** a line `<b>x</b>` -> its text content equals the source and there is no `b`
  element.
- **DPR:** `setViewport(view, 2)` + render -> `status.effectiveDpr === 2`, `onStatus` called,
  `lineBuilds` delta 0.
- **setPhases:** construct a `DomRenderer`, then `setPhases(new PhaseTimer(fakeNow))` ->
  `layout.phases` is that same timer instance. `setPhases(null)` -> `layout.phases === null`.
  There are no mutation records in either case.
- **Dispose:** root has no children, `liveLines === 0`, `liveNodes === 0`, and `render()`
  returns `false`.
- **CSS lint:** read `src/code/dom-renderer.css` through a dynamically imported
  `node:fs/promises` (the `test/e2e/stats.test.ts:26-27` dynamic-import pattern; tsconfig has
  `types: []`) -> no `opacity`, `filter`, `:has(` or `box-shadow`; every `border-radius` value
  is `0`.

## Tasks

### TASK-C1: Interface and selector
**Deliverables**: `renderer-types.ts`, plus the selector and type-check tests.
- [ ] `RendererKind`, `CodeRenderer` and `selectRendererKind` exported exactly as pinned
- [ ] Compile-time `CanvasRenderer extends CodeRenderer` assertion passes `npm run check`

### TASK-C2: Lines, gutter, window and transforms
**Deliverables**: `dom-renderer.ts` (root tree, window, keyed lines, run pieces, tokens, gutter,
font, status, stats, dispose), `dom-renderer.css`.
- [ ] Virtualization, scroll, edit, newline, long-line, token, literal-text, DPR and dispose
      tests pass
- [ ] `setPhases` sets `layout.phases` (same semantics as `renderer.ts:121`), and the setPhases
      test passes

### TASK-C3: Overlays and highlights
**Deliverables**: `dom-overlay.ts` (selection, caret, handles, bars, labels, highlight pool).
- [ ] Caret-move, animation-only, highlight-cap, IME, selection and no-layout-read tests pass

### TASK-C4: Verification and progress log
- [ ] All verification commands below exit 0. Logs are under `tmp/dom-renderer/core/`. The
      sha256 of each written file is recorded in the progress log.

## Verification (implementer, in the sandbox; no browser and no lock needed)

Run from `editor/`. Run them serially. Do not start the full vitest suite here.

1. `npm run check > ../tmp/dom-renderer/core/check.log 2>&1` -> exit 0.
2. `./node_modules/.bin/vitest run test/code/dom-renderer.test.ts > ../tmp/dom-renderer/core/vitest-dom.log 2>&1`
   -> exit 0. testsRun > 0, all pass.
3. `./node_modules/.bin/vitest run test/canvas > ../tmp/dom-renderer/core/vitest-canvas.log 2>&1`
   -> exit 0. This proves the canvas suite is untouched.
4. `git diff --stat -- editor/src/code/renderer.ts editor/src/code/layout.ts editor/src/code/mount.ts editor/src/code/code.css editor/src/app/theme.css`
   -> empty output (exit 0).

Report verification records only in the mandatory format: `{command, exitStatus: 0, testsRun,
testsPassed, failureCount: 0, outcome: "passed", log}`, with details in `notes`. Do not run
mutation or negative-control commands.

## Completion Criteria

- [ ] The five new files exist. Each `.ts` file is under 600 lines.
- [ ] Every pinned export and class name exists with the exact spelling.
- [ ] Verification 1-4 pass.
- [ ] No file outside writePaths changed (`git status --short` shows only the writePaths).

## Progress Log

### Session: (implementer fills in)
**Tasks Completed**:
**Verification**:
**Notes**:
