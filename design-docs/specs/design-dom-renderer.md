# DOM Renderer Backend and Canvas-vs-DOM Comparison

**Status:** Proposed (2026-10-07). **Issue reference:** workflowInput issue
"DOM (HTML) rendering backend for the editor and measured canvas-vs-DOM
performance comparison" (workflow execution
`opus-luna-design-and-implement-review-loop-session-296`, dispatch
`comm-004752`). No GitHub issue number exists. **Branch:** `wf/dom-editor`,
based on `wf/canvas` at `ec34f66`.

**User request (verbatim):** "ちなみにcanvasではなく htmlのeditorを使用するversionも作成してperfomanceを比較せよ"
(also build a version that uses an HTML editor instead of canvas, and compare
performance). The user chose a DOM rendering backend inside the same app,
behind a flag, so the comparison is apples-to-apples. Measurements run
serially. The operator will use the result later to write a GPUI-inspired
tuning proposal for the canvas renderer. That proposal is out of scope here.

**Related design.** The canvas editor is specified in
`design-implementation.md` 15.3 through 15.3.8.15. Measurement protocol,
thresholds, the silent virtual sink and the release-wasm rule are in 15.3.8.8,
15.3.8.13 (decision D) and 15.3.8.14. This document does not change any of
them. It reuses them for a second presentation backend. It is a standalone
file so that `design-implementation.md`, which `wf/canvas` is still editing,
is not touched. This keeps the later merge free of conflicts.

---

## 1. Scope

**In scope**

1. Renderer selection: `?renderer=dom` URL flag plus a programmatic mount
   option. Canvas stays the default.
2. A DOM presentation backend (`DomRenderer`) that implements the same render
   contract the mount already drives on `CanvasRenderer`.
3. Unit tests (jsdom) for the DOM renderer and the selection seam.
4. A DOM-mode behavior e2e script.
5. A comparison driver (`compare.mjs`) that measures canvas and DOM in
   Chromium and WebKit on the 1,000-line and 20,000-line documents, 3 runs
   each, with raw data and a report.

**Non-goals**

- No product setting or UI toggle for the renderer. DOM mode is a comparison
  backend reachable by URL flag or mount option only.
- No change to canvas behavior, canvas code paths, thresholds, workload,
  silent sink or wasm build rules.
- No `contenteditable`. Text input stays on the hidden textarea bridge
  (`input.ts`, `accessibility.ts`).
- No CSS Custom Highlight API in this wave (decision DR-6). No Web Worker,
  no native scrolling, no canvas tuning work.
- No changes to `design-implementation.md`, `renderer.ts`, `frame.ts`,
  `layout.ts`, `view-host.ts`, `input.ts`, `pointer.ts` or `stats.mjs`.

## 2. Baseline check (repository state at `ec34f66`)

Facts that this design relies on. Each was read in the current source.

| Fact | Where |
|------|-------|
| `mount()` composes the headless `CodeSurface`, `DocumentSync`, `InputController` (textarea bridge, IME, presentation doc), `PointerController`, `CodeViewHost` (virtual scroll, the only rect read), `FrameScheduler`, `HighlightScheduler`, syntax spans, diagnostics, completion, transport and the visual backdrop. | `editor/src/code/mount.ts:62-352` |
| The mount drives the renderer only through `setText(doc)`, `setViewport(view, dpr)`, `render(feedback)`, `textPending`, `setPalette`, `setPhases`, `dispose`, `status` (via `onStatus`), `stats` and `layout` (perf hook). | `mount.ts:84-88,190,205,234-245,248,260,331,337`; `perf-hook.ts:56-59` |
| The canvas element is the pointer target (pointer controller, pointer forwarding, diagnostic tooltip) and the backdrop is inserted before it. | `mount.ts:107-114,177-183,299-300` |
| `TextLayout` is the geometry authority: shaping, tab stops (4 spaces), `advance`, `coordsAtPos`, `posAtCoords`, `rangeRects`, `boundary`, `geometrySegments` (256-cluster chunks). Hit testing, caret anchoring and completion positioning go through it (`CodeViewHost.attachBridge`). | `layout.ts`; `view-host.ts:24-28` |
| Unchanged lines keep their `ShapedLine.runs` object identity across edits (`setText` moves cache entries). The canvas renderer keys line slots by that identity. | `layout.ts:108-145`; `renderer.ts:309-334` |
| Mount frames: text-dirty frames recompute syntax spans for one viewport above and one below, and pass `staticRows` (syntax, surface annotations, selection, composition) plus `animatedRows` (playing, eval). Animation-only frames pass the cached static rows and new animated rows. `annotationsRevision` increments on any selection change. | `mount.ts:193-246` |
| Theme tokens `--vt-syn-*`, `--vt-selection`, `--vt-playing-bg`, `--vt-flash`, `--vt-flash-error`, `--vt-danger`, `--vt-data-1`, `--vt-text`, `--vt-text-muted`, `--vt-raised` exist. The canvas palette maps token classes `vact-tok-*` to them. | `editor/src/app/theme.css`; `palette.ts:16-30` |
| `code.css` `.vact-tok-*` rules add italic, weight 600 and dotted underline. The canvas draws color only. | `code.css:11-18` |
| Harness: `measure.mjs` calls `createLargeDocument()` itself, reads `.vact-code-canvas` for WebGL info and for the cycle font-size step, and gates with `stats.mjs evaluate(THRESHOLDS)`. `behavior.mjs` is canvas-specific (pixel readback, `WEBGL_lose_context`). `createLargeDocument({lines})` asserts 1 MiB plus or minus 5%, so it cannot produce a 1,000-line document. | `editor/test/e2e/*.mjs`; `fixtures/large-doc.mjs:40-41` |
| Thresholds: input p95 50 / p99 100 ms; animation work p50 4 / p95 8 / p99 16.7 ms; text work p95 8 ms; frame interval p95 20 / p99 50 ms; sync p95 33.4 / p99 50 ms; ledger 96 MiB; heap growth 8 MiB. | `stats.mjs:1-8` |

Conclusion: the mount already isolates presentation behind a small,
duck-typed renderer surface. A DOM backend needs one interface type, one
selection seam in `mount.ts`, and a type widening in `perf-hook.ts`. No
shared editing, input, clock or syntax code changes.

## 3. Decisions

### DR-1 Renderer selection seam

- New file `editor/src/code/renderer-types.ts` defines:
  - `type RendererKind = 'canvas' | 'dom'`.
  - `interface CodeRenderer`: `layout: TextLayout`, `stats` (readonly
    numeric record), `status: GpuStatus`, `textPending: boolean`,
    `setText(doc: Text)`, `setViewport(view: LayoutViewport, dpr?: number)`,
    `render(feedback?: RenderFeedback): boolean`, `setPalette(palette)`,
    `setPhases(phases)`, `dispose()`. `GpuStatus` and `RenderFeedback` are
    imported as types from `renderer.ts`, which is not edited.
    `CanvasRenderer` satisfies the interface structurally.
  - `selectRendererKind(search: string, requested?: RendererKind): RendererKind`.
    Precedence: explicit `requested` option, then URL `renderer=dom`, then
    `'canvas'`. Any other URL value (including `renderer=canvas`, empty or
    unknown) yields `'canvas'`.
- `MountOptions` gains `renderer?: RendererKind`. This is the programmatic
  option for tests and the harness.
- `mount.ts` edits are limited to these:
  1. Compute the kind once.
  2. Canvas branch: the existing statements, unchanged in order and
     arguments.
  3. DOM branch: create `div.vact-code-dom` (with `aria-hidden="true"`) and
     `new DomRenderer(element, layout, { onStatus })`.
  4. Name the presentation element `surfaceEl`. It replaces `canvas` in the
     pointer controller, pointer forwarding, diagnostic tooltip listeners and
     backdrop insertion.
  5. Type the renderer as `CodeRenderer`.
  6. Set `codePane.dataset.renderer = kind`.

  No other mount logic changes. Frame callbacks, syntax windows, annotation
  assembly and highlight ticking are identical for both kinds.
- `perf-hook.ts` widens `CanvasRenderer` to `CodeRenderer`. `counters()`
  adds `rendererKind` and `domNodes`
  (`document.getElementsByTagName('*').length`, the whole document, the same
  definition for both kinds).

### DR-2 Shared versus presentation boundary

| Concern | Owner in DOM mode |
|---------|-------------------|
| Document authority, revisions, undo/redo, `DocumentSync`, wire-span mapping and pins | shared, unchanged |
| Textarea input bridge, IME preedit (presentation doc), accessibility, keyboard | shared, unchanged |
| Pointer and touch, selection handles logic, long press | shared `PointerController`, attached to `.vact-code-dom` |
| Virtual scroll, viewport, keyboard inset, the one rect read | shared `CodeViewHost` |
| Frame scheduler (text-dirty versus animation-only frames, visibility, DPR, resize) | shared `FrameScheduler` |
| Audible clock, `HighlightScheduler`, transport, eval flash | shared |
| Syntax spans (tree-sitter and fallback), diagnostics, completion popup positioning, format, bind and params | shared |
| Geometry (shaping, advances, hit testing, caret rects) | shared `TextLayout`. The DOM renderer never measures text and never reads layout. |
| Visual backdrop layering | shared `placeBackground`. The backdrop sits under `.vact-code-dom`, whose background is translucent like the canvas clear quad. |
| Glyph rasterization, text painting, overlays, gutter, highlights | `DomRenderer` (new) |

### DR-3 DOM element tree and coordinate model

```
div.vact-code-dom                  root: pointer target, clip, translucent bg,
                                   user-select none, touch-action none,
                                   contain: strict, aria-hidden
  div.vact-dom-gutter              left 0, width = view.gutter (48 px), clip
    div.vact-dom-gutter-rows       transform: translate3d(0, -scrollTop, 0)
      div.vact-dom-num  x N        keyed by line number, translateY(n*lh)
  div.vact-dom-text                left = gutter, right 0, clip
    div.vact-dom-content           transform: translate3d(-scrollLeft, -scrollTop, 0)
                                   font and line-height set once from layout.font
      div.vact-dom-sel             selection rects layer
      div.vact-dom-hl              animated highlight layer (playing, eval)
      div.vact-dom-lines
        div.vact-dom-line  x N     keyed by line identity, translateY(n*lh)
          span.vact-dom-run x R    left = layout x of the run piece
            span.vact-dom-tok-*    token spans; unstyled text is a bare text node
      div.vact-dom-under           diagnostic / composition / call-head bars
      div.vact-dom-labels          binding labels
      div.vact-dom-caret           one element
      div.vact-dom-handle x 2      selection handles
```

- **Content coordinates.** Every element inside `.vact-dom-content` is
  positioned in document coordinates: `x = TextLayout advance`,
  `y = lineNumber * lineHeight`. Scrolling writes exactly two transforms
  (`gutter-rows` and `content`) and nothing else, as long as the rendered
  window still covers the viewport (DR-4). Both layers use
  `will-change: transform`, so scroll is compositor-only in both engines.
- **Geometry fidelity.** Each run piece is absolutely positioned at its
  `TextLayout` x (`ShapedRun.x` or `geometrySegments` piece x). Tabs never
  reach the DOM, because `TextLayout` splits runs at tabs, so CSS `tab-size`
  does not matter. Browser text layout drift is therefore bounded within one
  run piece of at most 256 clusters. Caret, selection, handles, underlines
  and highlights are positioned from `TextLayout` (`coordsAtPos`,
  `rangeRects`, `advance`) with a content-coordinate viewport. That viewport
  is `{left: 0, top: W0*lh, gutter: 0, scrollLeft: 0, scrollTop: W0*lh,
  height: (W1-W0)*lh, width: 2^24}` for the rendered window `[W0, W1)`.
  This is the same geometry the canvas uses and the same geometry hit testing
  uses.
- **Font.** `.vact-dom-content` and `.vact-dom-gutter-rows` set
  `font: <layout.font.font>` and `line-height: <lineHeight>px` once at
  construction and on font change. They do not inherit from the root, so the
  harness cycle step that changes the presentation element's `font-size`
  causes only a style recalculation in both modes, never a geometry change.
- **Token classes.** `dom-renderer.css` defines `.vact-dom-tok-comment`,
  `-directive`, `-keyword`, `-number`, `-string`, `-path` (maps to
  `--vt-syn-string`), `-head` and `-bracket`. Each sets only
  `color: var(--vt-syn-*)`, mirroring `palette.ts TOKEN_PROPERTIES`. There is
  no italic, weight or decoration, so DOM glyph advances equal `TextLayout`
  advances and the painting matches canvas, which draws color only. The DOM
  renderer maps `vact-tok-*` annotation class names through a frozen lookup
  table. An unknown class renders as plain text, the same as the canvas
  palette fallback.
- **Colors.** Selection uses `--vt-selection`, playing uses
  `--vt-playing-bg`, eval uses `--vt-flash` and `--vt-flash-error`,
  diagnostics `--vt-danger`, composition `--vt-data-1`, call-head
  `color-mix(in srgb, var(--vt-text-muted) 60%, transparent)`, caret
  `--vt-text`, handles `--vt-data-1`, labels `--vt-raised` on `--vt-syn-head`,
  and the gutter `--vt-syn-comment`. The root background
  `--vt-dom-code-bg: rgb(10 13 18 / 0.85)` is defined only in
  `dom-renderer.css` and mirrors the canvas clear quad (`renderer.ts:233`).
  Pixel-exact parity with canvas is not a goal. Because `setPalette` has
  nothing to read, it is a no-op that only stores the palette. Theme changes
  apply through CSS.
- **CSS rules** (from the WebKit diagnosis in 15.3.8.14):
  - No `opacity`, `filter`, `:has()` or per-element `box-shadow` on lines,
    overlays or highlights.
  - Highlight on and off is a `visibility` toggle through a class, which
    needs paint only and no layout.
  - `border-radius: 0` everywhere, squares only, tokens only (UI style rule).
  - Lines use `position: absolute; white-space: pre; contain: layout style`.
- **Pointer and native behavior.** The root sets `user-select: none`,
  `-webkit-user-select: none`, `-webkit-touch-callout: none`,
  `touch-action: none` and `cursor: text`, so DOM text never starts native
  selection, drag or a callout. Selection remains the editor's own
  (`PointerController`).
- **Accessibility.** The root is `aria-hidden="true"`. The textarea bridge
  remains the only accessible surface, as in canvas mode.

### DR-4 Virtualization, keyed reuse and bounds

- **Rendered window.** `V = [v0, v1)` is the set of visible rows from the
  view. `margin = max(8, ceil(rows / 2))`. The current window is
  `W = [W0, W1)`. If `V ⊆ W`, then `W` is kept and scroll is a transform
  write only. Otherwise `W = [max(0, v0 - margin), min(lineCount, v1 + margin))`.
  `W` normally lies inside the mount's syntax window (one viewport above and
  one below). With very short viewports (for example, under a virtual
  keyboard), hysteresis can leave an edge of `W` slightly outside that window.
  Those off-screen edge lines render without spans until spans arrive, and
  then rebuild once, which is counted in `lineBuilds`. This is accepted. It
  has no visible effect, and the mount's syntax window is not changed.
  Provider truncation behaves the same as in canvas.
- **Line keys.** A `WeakMap<readonly ShapedRun[], LineEntry>` keyed by
  `ShapedLine.runs` identity, which is the same identity the canvas uses for
  slots. `LineEntry` holds the element, the line number it is positioned at,
  a content signature and the visible chunk set. The signature is font
  generation, syntax class runs for the line (relative offsets and class)
  and, for long lines, the visible chunk indices.
  - Hit with an equal signature: reuse. Write the transform only if the line
    number changed.
  - Hit with a different signature: rebuild that line's children with a
    single `replaceChildren`.
  - Miss: take an element from the free pool (or create one) and build it.
  - Lines leaving `W` are detached and returned to a free pool capped at
    `|W|`.
- **Long lines.** Lines with more than one geometry segment (more than 256
  clusters) render only the segments that intersect
  `[scrollLeft - width, scrollLeft + 2*width]`. This is the canvas culling
  rule (`renderer.ts:370-374`). A horizontal scroll that changes the visible
  chunk set rebuilds only those lines.
- **Gutter.** Number elements are keyed by line number, with
  `textContent = n + 1` and `translateY(n*lh)`. They are recycled when `W`
  shifts. Inserting a line adds one number element at the window end. It
  never rewrites the others.
- **Bounds** (constants exported from `dom-renderer.ts` and tested):
  - Live line elements and gutter numbers are each at most `|W|`. `|W|` is
    at most `rows + 2*margin` and at most `MAX_DOM_LINES = 1024`.
  - Token spans per line piece are at most `MAX_SPANS_PER_LINE = 512`.
    Excess styling is merged into plain text and counted.
  - Overlay elements per layer are at most `MAX_OVERLAY_ELEMENTS = 1024`.
    Excess is counted, not drawn.
  - Highlight keys are at most `MAX_HIGHLIGHT_KEYS = 512`, with LRU eviction
    of keys that are off. Keys that are on beyond the cap are counted as
    `highlightDropped`.
  - The free pool is at most `|W|`.
  - The renderer allocates nothing in the `ResourceLedger`, so its GPU ledger
    contribution is 0 by construction and is reported as such.

### DR-5 Frame discipline

`render(feedback)` runs inside the mount's rAF callback. It performs no
layout reads (`getBoundingClientRect`, `offset*`, `client*`,
`getComputedStyle`, `scroll*`) and no text measurement. The only per-frame
layout read in DOM mode is the existing `CodeViewHost.readRect`, at most one
per frame and only when the rect is stale. All DOM writes for a frame happen
in one synchronous pass in this order: window and lines, then gutter, then
static overlays, then caret and handles, then highlights, then transforms.
No read happens between writes, so the browser does one style and layout
pass after the callback.

`render` classifies its input against the previous call:

| Change since last render | DOM writes allowed |
|--------------------------|--------------------|
| `textRevision` or `setText` (document edit) | changed lines only (signature or identity miss), position writes for shifted lines, gutter entries entering or leaving the window, overlays recomputed |
| syntax span set changed | rebuild of lines whose syntax signature changed |
| scroll inside window | two transform writes |
| scroll leaving window | window shift (enter and leave lines and numbers), overlays recomputed for the new window, two transform writes |
| selection or `annotationsRevision` only (caret move, selection drag) | caret, handle and selection-layer elements only; zero writes inside `.vact-dom-lines` and the gutter |
| diagnostics, composition, call-head or binding annotations | the underline and label layers only |
| animated rows only (animation-only frame) | `.vact-dom-hl` only: for range keys already built, only `class` toggles; new keys create elements inside `.vact-dom-hl` |
| DPR change | none, apart from the status update (CSS handles DPR; no layout invalidation in DOM mode) |
| font `loadingdone` | the same as canvas: bump the font generation (`layout.setFont`), which rebuilds the window lines |

- Highlight geometry per key `(kind, className, from, to)` is cached per
  `(textRevision, W)`. It is recomputed lazily when a key turns on after
  either one changed.
- The syntax signature check costs O(spans in window + rows) per text-dirty
  frame. It writes nothing when nothing changed. When the syntax span list
  key (computed as in canvas `syntaxKey`) is unchanged, the per-line check is
  skipped.
- `textPending` is always `false`. There is no atlas, so the window is built
  in one frame.
- `status` is `{kind: 'ready', message: 'DOM renderer', effectiveDpr: dpr}`.
  `onStatus` fires on changes, the same as canvas. There is no context loss
  in DOM mode.
- `setViewport` validates its input the same way canvas does
  (`renderer.ts:126`, `RangeError` on non-finite or non-positive sizes).
- `dispose()` removes the renderer's children and listeners (fonts
  `loadingdone`), clears the maps and pools, and sets live counts to 0. The
  stylesheet link stays, because it is idempotent like the `code.css` link.

### DR-6 Playing highlights: overlay class toggles, no Custom Highlight API

Playing and eval ranges are drawn as pooled rectangle elements in
`.vact-dom-hl`, keyed by range. Turning a range on or off toggles one class.
This is the "class toggle" path the issue requires. It touches no line,
token or text node. The CSS Custom Highlight API exists in both target
engines (Chromium 105 and later, Safari 17.2 and later), but this wave does
not use it, for three reasons:

1. It needs DOM `Range` objects mapped into token spans and text nodes,
   rebuilt whenever lines rebuild, which adds a second geometry path.
2. `::highlight()` cannot use the overlay styling that canvas parity needs.
3. A comparison should have one measured path per renderer.

The issue makes the API optional, so no fallback is needed. It can be
evaluated later as a separate measured variant.

### DR-7 Instrumentation

`DomRenderer.stats` (exposed by `counters().renderer`):

| Counter | Meaning |
|---------|---------|
| `frames` | number of `render()` calls |
| `lineBuilds` | lines whose children were (re)built |
| `lineMoves` | position-only line writes |
| `lineReuses` | lines kept with no write |
| `lineReleases` | lines released from the window |
| `gutterWrites` | gutter number writes |
| `overlayWrites` | selection, underline and label writes |
| `caretWrites` | caret and handle writes |
| `highlightToggles` | highlight class toggles |
| `highlightBuilds` | highlight elements created or repositioned |
| `highlightDropped` | highlight keys dropped over the cap |
| `transformWrites` | layer transform writes |
| `windowShifts` | rendered-window shifts |
| `windowFrom`, `windowTo` | current rendered window `[W0, W1)` (0-based line indexes), read by the e2e bound check |
| `liveLines` | current line elements |
| `liveNodes` | renderer-owned elements and text nodes, maintained incrementally |
| `peakNodes` | maximum `liveNodes` |
| `spansMerged`, `overlayDropped` | cap counters |

All counters are plain integer increments with no allocation.
The mount's existing `timed('upload', ...)` phase wraps `render()` in both
modes, so `phaseMs.upload` is comparable.

### DR-8 Tests (vitest and jsdom; no wall clock)

New `editor/test/code/dom-renderer.test.ts` drives `DomRenderer` directly
with a `TextLayout` built on a fixed-advance metrics fake. It uses
`MutationObserver.takeRecords()` and the stats counters:

1. **Virtualization bounds.** A 20,000-line document with a 900 px viewport
   (50 rows). After the first render, and after scrolling to the middle and
   the end:
   - `liveLines` is at most `rows + 2*margin`.
   - Gutter elements are at most `|W|`.
   - Every rendered line number is in `W`.
   - `peakNodes` is bounded.
   - A scroll inside `W` produces only the two transform writes.
2. **Keyed reuse.**
   - Editing one character on line k produces `lineBuilds` delta 1 and no
     `childList` records on any other line.
   - Inserting a newline above the viewport produces `lineBuilds` delta at
     most 1 (the new line). Shifted lines get attribute-only (`style`)
     records, counted in `lineMoves`. Gutter writes are at most 1 plus the
     window delta.
3. **Caret move touches no line nodes.** A selection-only change (cursor and
   `annotationsRevision` change, same `textRevision`) produces no mutation
   record whose target is inside `.vact-dom-lines` or `.vact-dom-gutter`.
   `lineBuilds` and `lineMoves` deltas are 0, and only the caret, handle and
   selection layers change.
4. **Animation-only frame touches only highlight classes.** First frame: every
   record's target is inside `.vact-dom-hl`. Repeating the range set (on,
   off, then on again) produces only `attributes` records with
   `attributeName === 'class'` inside `.vact-dom-hl`. Both branches are
   asserted in one test. Highlight keys are capped at `MAX_HIGHLIGHT_KEYS`.
5. **IME and composition rendering.** A presentation doc with preedit text
   and a `composition` annotation renders the preedit text in the line DOM.
   One underline bar is at the `TextLayout` range rect, and the caret is at
   the presentation cursor. Committing replaces it with no composition bar.
6. **Selection rendering.** A 3-line selection produces 3 rects equal to
   `layout.rangeRects` in content coordinates. Changing the selection writes
   only the selection layer.
7. **No layout reads in render.** Spies on
   `Element.prototype.getBoundingClientRect` and `getComputedStyle` record 0
   calls during `render()`.
8. **Long lines.** A 5,000-character line renders only the visible chunks. A
   horizontal scroll that changes the chunk set rebuilds only that line.
9. **Tokens and bounds.** Token classes map to `vact-dom-tok-*`. An unknown
   class gives plain text. Span merging over 512 is counted.

New `editor/test/code/dom-mount.test.ts` (jsdom, rAF fake host, as in
`test/canvas/mount.test.ts`):

- **Selection precedence.** The default and `?renderer=bogus` produce
  `.vact-code-canvas` and no `.vact-code-dom`. `?renderer=dom` and
  `{renderer: 'dom'}` produce `.vact-code-dom` and no canvas. An explicit
  `{renderer: 'canvas'}` overrides the URL.
- **DOM mount wiring.** Typing updates the line DOM. Pointer events on
  `.vact-code-dom` move the selection. `counters().rendererKind === 'dom'`.
  Each frame performs at most one `getBoundingClientRect`. Dispose leaves
  the ledger at 0 and removes `.vact-code-dom`.
- **Canvas unchanged.** All existing `test/canvas/*` and other tests stay
  green with no edits.

### DR-9 DOM-mode behavior e2e (`editor/test/e2e/behavior-dom.mjs`, new)

The script has the same structure and silent sink as `behavior.mjs`. It opens
`/?perf=1&renderer=dom`, uses `hasTouch` contexts, and uses the same check
result format (`pass`, or `limitation`, which is excluded from counts).
`behavior.mjs` is not edited.

| Check | Assertion |
|-------|-----------|
| `dom-visible-text` | The pane has `.vact-code-dom` and no `.vact-code-canvas`. Each rendered visible row's text equals the document line (or its rendered chunk slices). A `vact-dom-tok-keyword` span's computed color equals the resolved `--vt-syn-keyword`. The bridge textarea is transparent. |
| `dom-caret-alignment` | For an ASCII line, a Japanese line and an emoji line of the fixture, the caret element's left edge is within 1 CSS px (ASCII) or 2 CSS px (non-ASCII) of a DOM `Range` rect at the same offset. This validates `TextLayout` against browser text layout. A failure is a recorded defect, not a limitation. A fix inside the DOM renderer files is in scope. A fix that needs `layout.ts` (not owned by any DOM plan) requires a scope amendment: stop and report with evidence. |
| `editing-undo-redo-navigation` | Same assertions as the canvas check. |
| `clipboard-round-trip` / `clipboard-synthetic-event` | Same as canvas (the WebKit check is a limitation). |
| `japanese-ime` / `japanese-ime-synthetic` | Chromium CDP composition. While preedit is active, the document and revision are unchanged, the preedit text is visible in the line DOM and a composition bar exists. Commit adds exactly one revision. WebKit is a limitation. |
| `touch-selection` | Long press on `.vact-code-dom` gives a word selection and two `.vact-dom-handle` elements, with presented `handles === 2`. Chromium uses CDP touch. WebKit uses synthetic pointer events (labeled). |
| `dpr-and-resize` | Selection is unchanged. `effectiveDpr === devicePixelRatio` (1 then 2 on Chromium; a DPR-2 context on WebKit). `lineBuilds` delta for a DPR-only change is 0. Bridge bounds stay in the viewport. |
| `visual-viewport-inset`, `background-no-replay` | Same as canvas. |
| `dom-bounded-nodes` | With the 20,000-line fixture loaded and scrolled to the middle and end, `liveLines` is at most `rows + 2*margin` and the rendered numbers are within the window. |
| `dispose-ledger` | The ledger is 0, `.vact-code-dom` is removed and `__vactrPerf` is removed. |

The canvas-only checks (`canvas-only-text`, `context-loss-restore`) do not
apply in DOM mode and are not run there.

### DR-10 Comparison protocol (`editor/test/e2e/compare.mjs`, new)

**Matrix.** Renderers `{canvas, dom}` × browsers `{chromium, webkit}` ×
documents `{1,000, 20,000}` lines × 3 runs = 24 measured runs. The flags
`--renderers`, `--browsers`, `--lines`, `--runs`, `--run-id` (default
`rc-001`), `--behavior` and `--resume` select subsets. `--resume` skips runs
whose raw JSON is complete. This lets the matrix be measured in several
invocations, so the lock is released between them.

**Workload.**

- 20,000 lines: `createLargeDocument()` unchanged (1 MiB, 64 voices,
  Japanese, emoji, long lines).
- 1,000 lines: the first 1,000 rows of the same deterministic document (the
  same head, the same evaluable program, about 45 KB). `compare.mjs` builds
  it with a pure helper, so the fixture file is not edited.
- Both use the identical workload object shape.

**Identical conditions.**

- The same built `dist` (one build; the renderer is chosen by URL only).
- Release wasm enforced by `gatingPreflight` (refusal exits 2).
- Silent sink with `installSilentSink`, plus `--mute-audio` on Chromium.
- The same Chromium launch arguments as `run.mjs`.
- A 1280x900 DPR-1 context.
- The same `runMeasurement` workload: control run, hush, large-document run,
  10 s warmup, 60 s edit loop, 120 s cycle loop with DPR flips, font-size
  step and 250 ms injected stalls.
- The same `evaluate(THRESHOLDS)`.
- WebKit headless or headed mode is decided once per invocation by the
  existing WebGL2 probe and applied to both renderers. It is recorded.
- A fresh browser per run.

**Order.** Within each (lines, browser) cell the runs alternate:
`r1 canvas, dom; r2 dom, canvas; r3 canvas, dom`. Each run records its
position, start and end timestamps, and `os.loadavg()` at start and end.
Runs whose 1-minute load average exceeds the CPU count are flagged
`contended` in the report. They are never dropped.

**Measurement lock.** `compare.mjs` refuses to start (exit 2, blocked) unless
`<VACTR_MEASURE_LOCK or <repoRoot>/../.measure-lock>/owner` exists and is not
empty. It records the owner id. The caller acquires and releases the lock as
the constraint prescribes:

```
until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done
echo <owner> > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner
trap 'rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock' EXIT
```

The recommended granularity is one invocation per (lines, browser) cell
(6 runs, about 25-30 min), with the lock released between cells. No vitest,
build or other heavy suite may run during a measurement.

**`measure.mjs` hooks** (the only shared harness edits, all additive):

1. An optional `workload` option that defaults to `createLargeDocument()`.
2. The cycle font-size step and nothing else selects
   `.vact-code-canvas, .vact-code-dom`. The WebGL info probe is unchanged and
   reports `webgl2: false` in DOM mode.
3. `domNodes` peak and final values, sampled at the existing counter sample
   points, stored in `metrics.domNodes = {peak, final}`.
4. `metrics.rendererStats = counters.renderer` and
   `metrics.rendererKind = counters.rendererKind`.

Canvas default behavior of `run.mjs` and `measure.mjs` is otherwise
unchanged. `run.mjs`, `behavior.mjs`, `stats.mjs`, `silent-sink.mjs`,
`serve.mjs` and `fixtures/large-doc.mjs` are not edited.

**First-viewport probe** (per run, in a separate fresh context with the
silent sink and no audio started, before the measured page):

- `mountToFirstFrameMs`: `presented()[0].frameMs`, measured from navigation
  start (`performance.now()` origin), for `/?perf=1&renderer=<r>`.
- `loadToViewportMs`: `t0 = performance.now()` read in-page immediately
  before `textarea.fill(workload.text)`, then the first presented frame with
  `revision` equal to the loaded revision and `counters().textPending ===
  false`, minus `t0`. This includes Playwright transfer of the document text,
  which is identical for both renderers. It is a relative comparison, not an
  absolute product latency.

**Metric definitions** (identical code for both renderers):

| Metric | Source | Note for DOM |
|--------|--------|--------------|
| input latency p50, p95, p99 | `pairInputLatency` over the frame and key rings. p50 is computed by `compare.mjs` from the in-memory `input-pair` samples. p95 and p99 come from `metrics`. | Includes the browser style, layout and paint the next frame waits on. This is the end-to-end comparator. |
| frame interval p95, p99 | `metrics.frameIntervalMs` | End-to-end comparator. |
| text work p95 | JS time inside rAF callbacks on text-dirty frames | DOM style, layout and paint run after the callback and are **not** included. Canvas GPU execution is also not included. The report states this. |
| animation work p50, p95 | JS time on animation-only frames | Same caveat. |
| sync (non-stall) p95, p99 | `metrics.syncAbsMs` | Same clock and highlight path. |
| stall-window recovery | `metrics.stallWindowSync` (count, p50, p95, max), `lateActiveMismatchCount`, `replayedFlashCount` | Same as canvas. |
| JS heap growth | `metrics.heapGrowthBytes` (Chromium CDP) | WebKit: reported as `unavailable`, never 0. |
| DOM node count | `metrics.domNodes` peak and final (whole document) | The same definition in canvas mode (the canvas page also has bind panels and other DOM). |
| GPU ledger | `metrics.ledgerMaxBytes` | DOM: 0 by construction, reported with that note. |
| first viewport | `mountToFirstFrameMs`, `loadToViewportMs` | Defined above. |
| renderer counters | `metrics.rendererStats` | Used to explain results, not to gate. |
| gate | `evaluate(THRESHOLDS)` pass and failures per run | Unchanged thresholds. DOM is gated exactly as canvas. |

**Raw data** under `design-docs/specs/evidence/renderer-comparison/<run-id>/`
(committed; `rc-001` for the canonical run):

- `environment.json`: host, OS, Node, browser versions, launch modes, wasm
  profile, bytes and sha256, dist build command, lock owner, commands, CPU
  count.
- `runs/<lines>-<browser>-<renderer>-r<k>.json`: all scalar metrics, gate,
  first-viewport, `rendererStats`, `domNodes`, load average, order and
  timestamps.
- `runs/<lines>-<browser>-<renderer>-r<k>.jsonl`: samples downsampled by the
  `run.mjs` rule (every 32nd frame, presented, onset, sync-sample and
  stall-audit row; phase spans of at least 50 ms; everything else kept). All
  scalar metrics are computed from the full in-memory samples before
  downsampling.
- `behavior/<browser>-<renderer>-behavior.json`.
- `attempts.json`: every attempt, including harness-crash attempts with
  their error.
- `comparison.json`: per-cell aggregates.
- Directory budget: 20 MiB. If it is exceeded, the downsampling stride for
  the bulky phases is doubled and the stride is recorded.

**Honesty rules.**

- Every completed run is reported. There is no best-of selection and no
  rerun-until-pass.
- A run may be retried only after a harness-level failure (browser launch,
  workload start, or a crash before the edit loop). Both attempts stay in
  `attempts.json` and are listed in the report.
- A failing gate is a result, not a retry reason.
- Unavailable values are written as `unavailable`.
- No threshold, workload or timing constant differs between renderers.

**Aggregation and "winner" rule** (pure functions in
`editor/test/e2e/compare-stats.mjs`, unit tested):

- Per cell and metric, report the 3 run values, their median and their
  min-max range.
- A renderer "wins" a metric when its median is lower and the medians differ
  by more than the larger of the two renderers' run ranges. Otherwise the
  result is "no clear difference".
- Gate results are counted as runs passed out of 3.

**Report** `design-docs/specs/design-renderer-comparison.md`:

- The generated sections between `<!-- RENDERER-COMPARISON:BEGIN -->` and
  `<!-- RENDERER-COMPARISON:END -->` are rewritten by
  `compare.mjs --write-report` from `comparison.json`. They contain:
  1. A summary table of medians for canvas versus DOM per browser and size,
     with winners.
  2. Method and environment.
  3. Per-cell tables with every run value.
  4. Gate results and failures per run.
  5. Contended runs and attempts.
- Sections written by the implementer from the data, outside the markers:
  - **Analysis**: where each renderer wins and why. Explanations must cite
    `rendererStats`, `phaseMs` and node counts. Required points:
    - JS work versus end-to-end metrics. DOM pays style, layout and paint
      outside the measured callback.
    - Scroll and window-shift cost.
    - Typing cost (single-line rebuild versus geometry segment rebuild).
    - Animation path (class toggles versus quads).
    - The 1,000-line versus 20,000-line sensitivity.
    - WebKit versus Chromium.
  - **Limitations**: WebKit heap unavailable, synthetic WebKit touch, IME
    and clipboard, headless or headed WebKit mode, contention.
  - **Observations for the canvas tuning proposal**: facts only, no
    proposal.

**Wall clock.** About 3.8 min per Chromium run and about 4.8 min per WebKit
run (the extra headless control loop). That is about 100-110 min for 24 runs,
plus about 5 min of behavior, spread over 4-5 locked invocations.

### DR-11 Files and ownership

| Path | Kind | Change |
|------|------|--------|
| `editor/src/code/renderer-types.ts` | new | DR-1 |
| `editor/src/code/dom-renderer.ts` | new | window, lines, gutter, transforms, stats, status, dispose (under 600 lines) |
| `editor/src/code/dom-overlay.ts` | new | selection, caret, handles, underlines, labels, highlight pools (under 600 lines) |
| `editor/src/code/dom-renderer.css` | new | DR-3 styles, linked by `dom-renderer.ts` only when constructed (`new URL('./dom-renderer.css', import.meta.url)`, the same pattern as `code.css`) |
| `editor/src/code/mount.ts` | shared | DR-1 seam only |
| `editor/src/code/perf-hook.ts` | shared | DR-1 type widening and two counters |
| `editor/test/code/dom-renderer.test.ts` | new | DR-8 |
| `editor/test/code/dom-mount.test.ts` | new | DR-8 |
| `editor/test/e2e/behavior-dom.mjs` | new | DR-9 |
| `editor/test/e2e/compare.mjs` | new | DR-10 driver |
| `editor/test/e2e/compare-stats.mjs` | new | DR-10 pure helpers (workload slice, ordering, aggregation, winner rule, report rendering, downsampling) |
| `editor/test/e2e/compare-stats.test.ts` | new | unit tests for the helpers |
| `editor/test/e2e/measure.mjs` | shared | DR-10 four additive hooks |
| `editor/test/e2e/README.md` | shared | a short "Renderer comparison" section |
| `design-docs/specs/design-renderer-comparison.md` | new | report |
| `design-docs/specs/evidence/renderer-comparison/rc-001/` | artifact root | raw data (committed) |
| `design-docs/specs/architecture.md` | shared | one decision-log row |

Artifact roots (gitignored or generated): `target/`, `editor/dist/`,
`tmp/dom-renderer/`,
`design-docs/specs/evidence/renderer-comparison/rc-001/`,
`tree-sitter-vact/tree-sitter-vact.wasm`.

Unchanged and verified by the existing suites: `renderer.ts`, `frame.ts`,
`layout.ts`, `view-host.ts`, `input.ts`, `pointer.ts`, `accessibility.ts`,
`highlight.ts`, `syntax*.ts`, `palette.ts`, `code.css`, `theme.css`,
`run.mjs`, `behavior.mjs`, `stats.mjs` and the fixture. Every touched or new
TS or JS file stays under 1,000 lines.

### DR-12 Plan decomposition (serial)

1. `impl-plans/active/dom-renderer-core.md`: `renderer-types.ts`,
   `dom-renderer.ts`, `dom-overlay.ts`, `dom-renderer.css` and
   `dom-renderer.test.ts`.
2. `impl-plans/active/dom-renderer-mount.md`: the `mount.ts` seam,
   `perf-hook.ts` and `dom-mount.test.ts`.
3. `impl-plans/active/dom-renderer-harness.md`: `compare.mjs`,
   `compare-stats.mjs` and its test, `behavior-dom.mjs`, the `measure.mjs`
   hooks and the README.
4. `impl-plans/active/dom-renderer-evidence.md`: DOM and canvas behavior
   runs, the 24 measured runs (locked), the report with analysis, the
   architecture row, gates, then commit and non-force push to
   `origin wf/dom-editor`.

**Gates** (EVIDENCE plan, final source):

- `cd editor && npm run check`
- `./node_modules/.bin/vitest run`
- `npm run test:perf`
- `npm run test:style` (locked)
- `cargo clippy --locked --all-targets -- -D warnings`
- The full `cargo nextest run`, alone and locked, with a timeout of at least
  1500 s.
- `mise run build-wasm-release`
- `VACTR_REQUIRE_SESSION_ABI=1 npm run build`
- `node test/e2e/compare.mjs --behavior` (locked; DOM checks must pass in
  both browsers)
- The comparison matrix (locked).

Canvas behavior (`behavior.mjs` via `compare.mjs --behavior --renderers
canvas`) is recorded as a regression reference. A canvas failure in a check
that exercises this branch's diff (the mount seam or perf hook) is a defect
to fix. Any other canvas failure is recorded with evidence and left to the
`wf/canvas` owner.

## 4. Risks

- **DOM 20,000-line WebKit runs may miss frame or input thresholds.** This is
  a reported result, not a blocker. Thresholds are unchanged.
- **Measurement-lock contention stretches wall-clock time.** Mitigated by
  per-cell invocations and `--resume`.
- **Browser text layout may differ from `TextLayout` for fallback glyphs.**
  Bounded by per-run-piece positioning and detected by `dom-caret-alignment`.
- **Merge friction with `wf/canvas` on `mount.ts`, `perf-hook.ts`,
  `measure.mjs` and `architecture.md`.** Edits are minimal and additive.
