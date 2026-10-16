# Code Pane Text Rendering

Design for fixing the canvas/WebGL code pane's dropped and garbled glyphs,
misplaced overlays, and heavy, soft text, plus the visuals pane MSAA blit
error. Branch `wf/code-render`, base `ea51a07`. Issue record: the
workflow input "Code pane drops and garbles glyphs (missing line segments,
'1' rendered as a bar) and text looks dirty" (no GitHub issue).

## 1. Overview

### 1.1 Scope

In scope:

- Root-cause fixes for the six reported defects (section 3).
- Glyph placement and blending rules that make canvas text match the
  browser's own rendering of the same font at DPR 1 and 2 (section 4).
- Regression coverage: unit tests, and a Playwright pixel test at DPR 1
  and 2 (section 5).
- The evidence-harness gap: why the existing canvas evidence passed these
  defects, and the checks that close it (section 5.4,
  [design-canvas-editor-evidence.md](design-canvas-editor-evidence.md#code-pane-text-rendering-gap-2026-10-16)).
- A user-qa decision on the font stack, weight and font bundling
  ([pending-code-render-questions.md](../user-qa/pending-code-render-questions.md)).

Out of scope:

- The document model, `EditorState` ownership, input and keyboard paths
  (`editor/src/code/input.ts`, `keyboard.ts`). The sibling run
  `wf/vim-mode` owns those and a cursor-shape hook in `renderer.ts`.
- A CodeMirror `EditorView`, a new renderer backend, or a text shaper.
- Complex-script (`isRunLine`) raster changes beyond the shared
  cache-key and placement rules. No reported defect is in that path.
- Gamma or contrast correction, subpixel (LCD) antialiasing, and a
  bundled font (pending CR1).

### 1.2 Baseline data flow (verified at ea51a07)

1. `TextLayout` (`layout.ts`) shapes each document line into `ShapedRun`s
   split at tabs. `ShapedRun.from`/`to` are absolute document offsets.
   `geometrySegments(line)` splits runs into chunks of at most 256
   clusters; chunk runs also carry absolute `from`/`to`.
2. `CanvasRenderer.rebuildTextSegments` (`renderer.ts`) builds one
   retained command list per (line runs identity, chunk) and one gutter
   list per line, keyed in `SegmentCache`. Commands are in CSS pixels
   relative to the line origin (`top = 0`).
3. `drawRun` takes the ASCII/grapheme path (`layout.textCells`) for every
   run unless `isRunLine(run)` (complex scripts only), and asks
   `GlyphAtlas.cell` for one white mask cell per cluster.
4. `GlyphAtlas.cell` (`atlas.ts`) rasterizes the cluster with Canvas 2D
   into a staging canvas at `dpr`, uploads the cell rectangle with
   `texSubImage2D`, and caches it by a string key. The texture uses
   `LINEAR` filtering and the default `UNPACK_PREMULTIPLY_ALPHA_WEBGL`
   (false), so texels are straight alpha: rgb white, a = coverage.
5. Every instance (32 bytes) carries a slot index. The vertex shader
   (`geometry.ts`) adds the slot texel `(x, y)` from `u_slots` to the
   instance rectangle. `writeSlots` assigns slots from 1 upward to text
   lines, with `(x, y) = (-scrollLeft, line.number * lineHeight -
   scrollTop)`. Slot 0 is never assigned and stays `(0, 0)`.
6. The fragment shader outputs `texel * v_color` (or `texel` for
   untinted emoji). Blending is `ONE, ONE_MINUS_SRC_ALPHA`, and the
   canvas context uses the default `premultipliedAlpha: true`.
7. Font: `13px ui-monospace, SFMono-Regular, Menlo, monospace`,
   `lineHeight 18`, `baseline 14` (`mount.ts:84`).
8. Visuals: `visual/mount.ts:71` creates the shared visuals context with
   `getContext('webgl2')` (default `antialias: true`).
   `GlRenderHost.present` (`render-host.ts:226`) blits a single-sample
   texture framebuffer into the default framebuffer.

## 2. Evidence

The repro script `tmp/code-render/repro-shot.mjs` (headless Chromium,
ANGLE Metal on Apple M4, DPR 2) produced `t-typed-code.png`,
`t-typed-two.png`, `t-scroll-redraw.png` and `code-gpu2x.png` (gitignored
under `tmp/code-render/`). The pixel positions in them match the root
causes below exactly:

- `t-typed-code.png`: the gutter `1` is normal; the document `1` in
  `108` is a thin bar.
- `t-scroll-redraw.png` (the 3-line insert): line 2 starts at
  `48 + 4 * advance` CSS px with `bpm`; line 3 starts at
  `48 + 16 * advance` with `e :c5`. 4 and 16 are those lines'
  absolute document offsets. The caret is at the correct x on line 3 but
  two rows low. The diagnostic underline is two rows low.
- `t-typed-two.png`: line 2 (`abc def`, offset 8, length 7) is blank. The
  underline is one row low.
- `code-gpu2x.png` (kalimba): only line 1 (offset 0) renders. A
  background-shade step appears at row 5, the slot offset of the last
  built line.

## 3. Root causes

| ID | Defect | Root cause | Location |
|---|---|---|---|
| RC1 | `1` drawn as a thin bar (case 1) | The atlas cache key leaves out the rasterized cell width. The gutter label run for line 1 is `{ text: '1', width: gutter - 8 }` (`labelRun`), so `textCells` gives `1` a 40 CSS px cluster and the atlas caches a 40 px wide `1` cell. The document `1` (about 7.8 px) gets the same key and reuses that cell, squeezed into a narrow quad. Any cluster width that differs between call sites (label boxes, prefix-measurement rounding) collides the same way. | `atlas.ts:92` (key), `renderer.ts:285-292,409` (label width) |
| RC2 | Leading characters missing on every line after the first; short later lines blank (cases 2, 3, 4) | `TextLayout.textCells` (ASCII path) starts at `offsetInRun(run, visibleFrom, -1)`. That returns an absolute document position (`run.from + boundary`), but it is used as a run-relative index. On a line whose run starts at document offset N, the first N characters are skipped; if N is at least the run length, nothing is drawn. Line 1 (N = 0) is unaffected. The completion popup in case 2 is a coincidence, and so is the single-color comment in case 4: both are line 1. | `layout.ts:68` |
| RC3 | Caret, diagnostic, composition and call-head underlines, handles, binding labels, selection, playing/eval highlights and the viewport backdrop drawn offset by a line position (case 3) | `addCommand` stamps every instance with `this.currentSlot`, which keeps the slot of the last text segment built. Non-text layers then inherit that line's slot offset `(-scrollLeft, n * lineHeight - scrollTop)` in the vertex shader. Their rectangles are already in viewport coordinates, so the offset is applied twice. | `renderer.ts:297` with `408,421` |
| RC4 | Heavy text with bright fringes (case 5) | The fragment shader outputs straight alpha (`rgb = tint`, `a = coverage * tint.a`). The blend (`ONE, ONE_MINUS_SRC_ALPHA`) and the premultiplied canvas expect premultiplied output. Every partly covered edge pixel is therefore drawn at full tint over the background, which thickens strokes. | `geometry.ts:50`, `renderer.ts:210`, `atlas.ts:128` |
| RC5 | Soft text (case 5) | Quads sit at fractional device-pixel positions: glyph x is `gutter + run.x + cluster.x` with fractional advances, and the slot offset includes fractional scroll. The quad width is `cluster.width + 2 * pad / dpr` CSS px, but the cell is `ceil(width * dpr) + 2 * pad` texels wide, so the texture is also rescaled. With `LINEAR` sampling, both effects blur every glyph. | `renderer.ts:578-580`, `atlas.ts:89,155-156` |
| RC6 | `GL_INVALID_OPERATION: glBlitFramebuffer` repeating (case 6) | The visuals context is created with the default `antialias: true`, so the default framebuffer is multisampled. WebGL2/GLES 3.0 forbids `blitFramebuffer` into a draw framebuffer with `SAMPLE_BUFFERS > 0`. Rectangles and formats already match (`RENDER_SIZE` on both sides). | `visual/mount.ts:71`, `render-host.ts:226` |

Checked and ruled out (no change):

- `texSubImage2D` upload rectangles. WebGL2 uploads `[0, width) x [0,
  height)` of the staging canvas with `UNPACK_SKIP_* = 0`, and
  `clearRect` clears the same region first.
- Stale segment-cache invalidation on document change. Segment keys use
  the line runs identity, style hash, DPR and atlas generation, and
  `setText` drops shaped lines in changed ranges. The blank lines are
  explained entirely by RC2.
- The complex-script `run` cell path (`piece`/`chunkX`). It is not taken
  for ASCII or Latin text, so it cannot cause cases 1 to 4.

## 4. Design

### 4.1 Cluster ranges are run-relative (RC2)

- R1. `textCells(run, visibleFrom, visibleTo)` returns clusters whose
  `from`/`to` are offsets into `run.text`, and it starts at the first
  cluster whose right edge is at or after `visibleFrom`, whatever
  `run.from` is. The ASCII path converts `offsetInRun`'s absolute
  result to a run offset (`- run.from`) before indexing. `offsetInRun`
  keeps its absolute contract, which hit testing uses.
- R2. For any run, the clusters `textCells(run)` returns concatenate to
  `run.text`, and cluster `x` equals `runOffset(run, from)`. This holds
  for lines at any document offset and for chunk runs from
  `geometrySegments`.

### 4.2 Atlas cells are keyed by everything that affects their pixels (RC1, RC5)

- R3. The `GlyphAtlas.cell` key includes the cell's texel width
  (`ceil(width * dpr) + 2 * pad`) and the subpixel phase (R6), next to
  the existing kind, font, fallback, font generation, DPR, text, piece
  and chunkX. Two requests share a cell only when they would rasterize
  identical pixels.
- R4. A returned `AtlasCell.width`/`height` is the exact texel extent of
  the uploaded raster, and its `u0..u1`/`v0..v1` span exactly that extent.
- Label runs (gutter numbers, binding labels) keep their box width.
  With R3 their glyph cells no longer collide with document glyphs. No
  label layout change is needed.

### 4.3 Non-text instances use the identity slot (RC3)

- R5. Slot 0 is reserved as the identity offset and is never assigned to
  a line (as today: `nextSlot` starts at 1). Every instance written to
  the `background`, `overlay` and `overlayText` layers carries slot 0.
  Only text-layer segment and gutter commands carry their line's slot.
  The renderer sets the text slot only while it builds a text segment and
  resets it to 0 afterwards, so no other draw path can inherit it. The
  cursor quad code itself is not edited, which keeps the `wf/vim-mode`
  cursor-shape hook region conflict-free.

### 4.4 Glyph quads map 1:1 to device pixels (RC5)

- R6. Subpixel phase. At segment build time, each glyph's origin in
  device pixels (`(gutter + run.x + cluster.x) * scale`, relative to the
  line origin) is split into an integer part and a fractional part. The
  fractional part is quantized to one of 4 phases (0, 0.25, 0.5, 0.75;
  rounding up to the next integer at 1). The atlas rasterizes the glyph
  shifted right by the phase, inside the existing pad. Four phases
  match the subpixel positioning Chromium/Skia uses for DOM text, so
  monospace spacing stays even. Plain integer snapping would make
  `7.8 px` advances alternate between 7 and 8 px.
- R7. Exact quad size. A glyph instance's rectangle is
  `cell.width / scale` by `cell.height / scale` CSS px. Its left edge is
  `(integerPart - cell.pad) / scale`, so it is device-integral relative
  to the line origin.
- R8. Snapped origin. Glyph instances carry a new instance flag
  (`SNAP_GLYPH`). For flagged instances, the vertex shader rounds the
  instance origin (rectangle top-left plus slot offset) to the nearest
  device pixel and keeps the size as is. The device ratio comes from a
  new uniform holding `canvas.width / view.width` and
  `canvas.height / view.height`, which absorbs fractional scroll and the
  `floor` in `effectiveSize`. Unflagged instances (backgrounds, caret,
  underlines, handles) are positioned as today.
- R9. The atlas texture uses `NEAREST` for minification and
  magnification. With R6 to R8 every glyph texel lands on exactly one
  device pixel, so filtering cannot blur. `NEAREST` also keeps
  float error from bleeding neighbor cells. When the canvas ratio is
  not exactly `scale` (budget-reduced DPR), mapping is near 1:1 and
  `NEAREST` stays crisp.

### 4.5 Premultiplied output (RC4)

- R10. The fragment shader always emits premultiplied color:
  `c = untinted ? texel : vec4(v_color.rgb, v_color.a * texel.a)`,
  `out = vec4(c.rgb * c.a, c.a)`. Texture upload stays straight alpha
  (`UNPACK_PREMULTIPLY_ALPHA_WEBGL` false). The blend stays `ONE,
  ONE_MINUS_SRC_ALPHA`. Mask cells use only texel alpha (coverage), so
  the tint is exact.
- R11. Look preservation for solid quads. The renderer's hard-coded
  translucent literals (`renderer.ts`: backdrop `[0.04, 0.05, 0.07,
  0.85]`, selection `[0.12, 0.22, 0.32, 0.55]`, playing/eval colors) are
  converted to straight alpha (`rgb / a`, clamped to 1). Their composite
  then stays unchanged under R10. Palette colors read from CSS are
  straight alpha by definition. With R10 they now composite the way
  CSS does. Before/after screenshots and `npm run test:style` must show
  no style regression.
- No gamma, contrast or weight change. If the fixed rendering still
  looks heavier than DOM text, that goes to CR2.

### 4.6 Visuals context without MSAA (RC6)

- R12. `visual/mount.ts` creates the visuals context with
  `getContext('webgl2', { antialias: false })`. Its outputs are
  full-screen shader passes into single-sample texture framebuffers, so
  MSAA on the default framebuffer has no effect, and turning it off makes
  the `present` blit valid. The blit stays as it is.

### 4.7 Module boundaries and size

| File | Change | Lines at ea51a07 |
|---|---|---|
| `editor/src/code/layout.ts` | R1, R2 | 499 |
| `editor/src/code/atlas.ts` | R3, R4, phase raster (R6), `NEAREST` (R9) | 209 |
| `editor/src/code/geometry.ts` | `SNAP_GLYPH` flag, vertex snap (R8), fragment premultiply (R10), a pure glyph-placement helper (R6, R7) for unit tests | 139 |
| `editor/src/code/renderer.ts` | text-only slot scoping (R5), glyph quad sizing through the helper (R7), device-ratio uniform (R8), straight-alpha literals (R11). The `render()` cursor block is not edited. | 668 |
| `editor/src/visual/mount.ts` | R12 | - |

Every touched file must stay under 1000 lines.
`editor/test/canvas/gpu.test.ts` (990 lines) gets no new tests. New
unit tests go in new files, and shared fake-GL helpers may move into a
test helper module, listed as a mechanical change.

## 5. Validation

### 5.1 Unit tests (vitest; must fail on ea51a07)

New file `editor/test/code/text-render.test.ts` (or split by area if it
grows):

- U1 (RC2). For the document `# c\nuse-bpm 108\ns :kalimba > note :c5`
  and for `examples/kalimba.vact`, the `textCells` texts of every chunk
  run of every line concatenate to the line text. Cell `x` values equal
  `advance(line, pos) - run.x`.
- U2 (RC1). Two `GlyphAtlas.cell` requests for `1` with widths 40 and 8
  return distinct cells whose texel widths are `ceil(w * dpr) + 2 * pad`.
  Same width with different phase also returns distinct cells.
- U3 (RC3). After rendering a 3-line document with a cursor on line 3,
  a selection and a diagnostic, every non-text-layer instance uploaded
  to the fake GL buffers has slot 0, and the cursor rectangle's top equals
  `2 * lineHeight - scrollTop`.
- U4 (R6, R7). The glyph-placement helper returns device-integral quad
  origins, the expected phase for fractional origins (including carry at
  1), and quad sizes equal to the cell texel extent divided by `scale`
  for DPR 1, 1.5 and 2.
- U5 (R8, R9, R10). Shader and texture contract: the exported
  `FRAGMENT_SOURCE` premultiplies, the exported `VERTEX_SOURCE` snaps
  `SNAP_GLYPH` instances, and the fake GL's recorded `texParameteri`
  calls set `NEAREST` on the atlas texture.
- U6 (RC6). With a stub document whose canvas records `getContext`
  arguments, visuals mount requests `webgl2` with `antialias: false`.

U4 and U5 cover new code. Their old-code failures come from the pixel
test (section 5.2) in mutationEvidence.

### 5.2 Playwright pixel test

Files: `editor/test/e2e/code-pixels.mjs` (the driver: `runCodePixels(browser,
origin, options)` plus a CLI), `editor/test/e2e/pixel-compare.mjs` (pure
analysis), `editor/test/e2e/pixel-compare.test.ts` (vitest on synthetic
pixel arrays, like `stats.test.ts`).

- Matrix: DPR 1 and 2 (`deviceScaleFactor`) x two pinned Chromium
  backends: ANGLE Metal (`--use-angle=metal --enable-gpu
  --ignore-gpu-blocklist`, same as `run.mjs`) and SwiftShader
  (`--use-angle=swiftshader`). The WebGL renderer string is recorded in
  each check's detail. A backend that is not the one requested fails
  the check; it never passes silently.
- Scenarios (fresh page each, viewport 1100x700, editor focused by
  clicking the canvas):
  - S1: type `use-bpm 108`.
  - S2: type `# hello`, Enter, `abc def`, with the completion popup open.
    Lines under the popup's DOM bounding box are excluded.
  - S3: `insertText('# c\nuse-bpm 108\ns :kalimba > note :c5')`.
  - S4: `insertText` of the contents of `examples/kalimba.vact`.
- Reference. The page injects a hidden-from-input DOM reference block
  outside the code pane. It has the same font string as `layout.font`,
  white text on the code backdrop color, one row per document line, and
  the same line height. Per-character boxes come from
  `Range.getClientRects`. Canvas and reference are captured with
  `page.screenshot` clips (composited pixels, independent of
  `preserveDrawingBuffer`).
- Coverage metric. For each character cell, coverage per pixel is
  `distance(pixel, backdrop) / distance(brightest pixel in the cell,
  backdrop)`; cell ink is the sum. Because the metric normalizes per
  cell, it does not depend on tint. Checks per line:
  - P1: every non-whitespace character cell has canvas ink at least 0.5
    times its reference ink (catches RC2).
  - P2: whitespace cells have near-zero ink.
  - P3: each character's inked column extent is within 1 device px of
    the reference's (catches RC1).
  - P4: the line's total ink ratio (canvas / reference) is within
    [0.8, 1.25] (catches RC4 and RC5 smear). The tolerance may be
    recalibrated once on the fixed code; the calibrated value is
    recorded in the plan before any mutation run.
- Overlay checks:
  - P5: the caret-colored pixels lie inside the caret line's row band.
  - P6: diagnostic-colored underline pixels lie in the bottom 2 CSS px
    band of the line containing the diagnostic's start. The expected
    line comes from a read-only diagnostic snapshot added to the
    existing `?perf=1` hook (`perf-hook.ts`) if the hook does not
    already expose it.
  - P7: below the last text row, each sampled column is uniform within
    2 levels (catches the RC3 backdrop step).
- Console gate (P8): no console message matching `GL_INVALID|WebGL:
  INVALID|GL ERROR` from page load to the end of the scenario. The page
  is the default browser-tier app with the visuals pane mounted, the
  same setup in which the owner's repro logged the repeating blit error
  (catches RC6).
- Gating: `behavior.mjs` calls `runCodePixels` for Chromium at DPR 1 and
  2 (checks `code-pixels-dpr1`, `code-pixels-dpr2`), so
  `npm run e2e -- --profile behavior` gates them on the run.mjs backend.
  The CLI (`node test/e2e/code-pixels.mjs --origin <url> --backend
  metal|swiftshader`) runs the full matrix against a dev server on a
  free port (never 5173 or 5174) under the measurement lock.
- Old-code requirement: on ea51a07 at least P1, P3, P5, P6, P7 and P8
  fail. The failing outputs are recorded as mutationEvidence, never in
  the gating verification list.

### 5.3 Before/after evidence

`tmp/code-render/repro-shot.mjs` is re-run (with a free-port URL
argument) on the fixed build. After-screenshots go next to the
before-screenshots under `tmp/code-render/` (gitignored), and the plan
progress log names both sets.

### 5.4 Why the evidence harness missed these defects

- The only canvas pixel check (`behavior.mjs`, line-1 glyph rectangle)
  reads line 1 only. RC2 does not affect line 1, because there absolute
  and run-relative offsets coincide.
- That check asserts only "at least 2 distinct colors" and a rectangle
  wider than 4 px. A squashed `1`, heavy fringes and blur all pass it.
- The DPR 2 check only compares `devicePixelRatio` values; no pixels are
  read at DPR 2.
- No check covers overlay positions (caret, underline) after a
  multi-line edit, and the console is not scanned for GL errors.
- The unit tests (`gpu.test.ts`) run `textCells` only on line 0. Their
  fake GL captures instance bytes (`bufferSubData`), but no test checks
  instance slots, overlay positions or atlas-key uniqueness across
  widths. Multi-line geometry tests exercise hit testing (`advance`,
  `posAtCoords`), which uses the absolute `offsetInRun` contract
  correctly. `test:style` measures DOM chrome, not canvas pixels.

The pixel test (P1 to P8) and units U1 to U6 close each gap. The gap
analysis is also recorded in
[design-canvas-editor-evidence.md](design-canvas-editor-evidence.md#code-pane-text-rendering-gap-2026-10-16).

### 5.5 Gates

`cd editor && npm run check`; focused vitest on the new test files;
`npm run build`; and, each under the measurement lock
(`mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`, then
`rmdir` afterwards; wait and retry if it exists): full vitest,
`npm run test:style`, the canvas e2e (`run.mjs` and `behavior.mjs`), and
the pixel-test matrix against a dev server on a free port. Frontend
tests use the prebuilt `target/wasm32-unknown-unknown/debug/vactr.wasm`
and `tree-sitter-vact.wasm`. Artifact roots: `target/`,
`tree-sitter-vact.wasm`, `editor/node_modules`, `tmp/`, Playwright
outputs.

## 6. Rollout constraints

- Diagnosis is complete before code changes (section 2 and 3). The
  implementation must confirm RC1 to RC3 with U1 to U3 failing on
  ea51a07 before fixing them.
- Fix order: RC2, RC1 and RC3 (correctness), then RC4 and RC5
  (quality), then RC6. Each fix lands with its tests. RC4 and RC5 change
  how all text looks and are judged together, against the pixel test and
  screenshots.
- `renderer.ts` edits stay inside `addCommand`/slot scoping, `drawRun`
  quad construction, uniform setup and the literal colors.
  `input.ts`, `keyboard.ts` and the cursor quad block are not touched.
- No bundled font unless CR1 is answered with a bundling option, and
  then only with its license file and notice recorded.

## 7. Open questions

- CR1 (font stack and bundling) and CR2 (weight) in
  [pending-code-render-questions.md](../user-qa/pending-code-render-questions.md).
  Implementation follows the recommendations (keep the system stack,
  weight 400, no bundled font) unless the owner answers otherwise.
