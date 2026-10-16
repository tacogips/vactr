# Code Render 05: Renderer Wiring - Identity Slots, Snapped Glyph Quads, Straight-Alpha Literals (RC3, R5, R7, R8, R11)

**Status**: Ready
**Plan ID**: CR-05
**Wave**: 2 (depends on CR-01, CR-02, CR-04)
**Design Reference**: design-docs/specs/design-code-pane-text-rendering.md sections 3 (RC1, RC3, RC5), 4.2-4.5 (R3, R5, R6, R7, R8, R11), 4.7, 5.1 (U3), 6
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

`editor/src/code/renderer.ts` has three remaining defects:

1. RC3: `addCommand` (line 297) stamps every instance with
   `this.currentSlot`. That field is set while building text segments
   (lines 408, 421) and never reset, so caret, underlines, handles,
   binding labels, selection, playing/eval highlights and the viewport
   backdrop inherit the last text line's slot offset `(-scrollLeft,
   n * lineHeight - scrollTop)` in the vertex shader. Evidence: the caret
   two rows low in `tmp/code-render/t-scroll-redraw.png`, the backdrop
   step in `code-gpu2x.png`.
2. RC5: `drawRun` (lines 561-611) sizes glyph quads as `cluster.width +
   2 * cssPad` at fractional positions, which rescales and blurs cells.
3. R11: the hard-coded translucent colors assume straight-alpha output.
   Under CR-04's premultiplied shader they must be converted to keep
   their look.

This plan wires CR-02 (`CellRequest.phase`) and CR-04 (`SNAP_GLYPH`,
`u_device`, `glyphPhase`, `glyphQuad`) into the renderer, adds U3, and
reconciles existing canvas tests that encode the old behavior.

## Non-goals

- Do not edit the cursor block in `render()` (the
  `if (overlayDirty && feedback.cursorVisible !== false && feedback.cursor != null)`
  statement and its quad call). The sibling `wf/vim-mode` run adds a
  cursor-shape hook there. The RC3 fix goes in `addCommand` and the
  segment-build scope, not in the cursor code.
- Do not touch `editor/src/code/input.ts`, `keyboard.ts`, `layout.ts`,
  `atlas.ts`, `geometry.ts`, or the palette/CSS colors (`palette.ts`).
- No change to the blend function, segment cache keys, slot allocation,
  block packing, or label layout (`labelRun` keeps its box width).

## Files

| Path | Change |
| --- | --- |
| `editor/src/code/renderer.ts` | Changes A-E below. |
| `editor/test/code/renderer-layers.test.ts` | New unit test file (U3 plus renderer-level RC1 and R7 checks). |
| `editor/test/canvas/gpu.test.ts` | Mechanical reconciliation only (see below). Must stay under 1000 lines (990 at ea51a07). |
| `editor/test/canvas/edit-cost.test.ts` | Mechanical reconciliation only, if needed. |
| `impl-plans/active/code-render-05-renderer.md` | Progress log. |

### renderer.ts changes

- A (R5). `addCommand`: record `slot: this.currentLayer === 'text' ?
  this.currentSlot : 0`. After each text segment or gutter build in
  `rebuildTextSegments` (where `this.collecting = null` is set), reset
  `this.currentSlot = 0`. Slot 0 stays reserved: `nextSlot` keeps
  starting at 1, and never push 0 into `freeSlots`.
- B (R6, R7, R8, mask path). In `drawRun` for non-`isRunLine` runs:
  compute `glyphX = x + cluster.x` as today, and keep the existing
  visibility culling on `glyphX`. Then `const { devicePixel, phase } =
  glyphPhase(glyphX, this.scale)`, request the cell with `phase`, and
  build the quad from `glyphQuad(devicePixel, this.scale, cell)`:
  `{ left, right: left + width, top: y, bottom: y + height }`. Set
  `flags | SNAP_GLYPH`.
- C (run path, review finding DR-L1). For `isRunLine` pieces, use the
  same helpers on the piece origin `x + offset`: pass `phase` in the
  `kind: 'run'` request, size the quad with `glyphQuad`, and set
  `SNAP_GLYPH`. `piece`, `chunkX` and the piece loop stay unchanged. This
  keeps every atlas cell drawn 1:1 under `NEAREST` sampling.
- D (R8). In `render()`, next to the existing `gl.uniform2f(this.locations.u_view, ...)`:
  `gl.uniform2f(this.locations.u_device, this.canvas.width / this.view.width, this.canvas.height / this.view.height)`.
  Add `'u_device'` to the uniform-location lookup list in `initialize()`
  (the `for (const name of ['u_view', 'u_texture'])` loop).
- E (R11). Convert the hard-coded translucent literals to straight alpha
  (`rgb / a`, rounded to 3 decimals, clamped to 1) so their composite is
  unchanged: backdrop `[0.04, 0.05, 0.07, 0.85]` -> `[0.047, 0.059,
  0.082, 0.85]`; selection `[0.12, 0.22, 0.32, 0.55]` -> `[0.218, 0.4,
  0.582, 0.55]`; playing `[0.35, 0.29, 0.13, 0.5]` -> `[0.7, 0.58, 0.26,
  0.5]`; eval error `[0.4, 0.12, 0.16, 0.5]` -> `[0.8, 0.24, 0.32, 0.5]`;
  eval `[0.2, 0.32, 0.36, 0.5]` -> `[0.4, 0.64, 0.72, 0.5]`. Leave palette
  colors (`this.palette.*`) unchanged.

Imitate the existing patterns: `renderer.ts:render` uniform setup (line
212), `drawRun`'s cell-miss handling (it must stay the same: on a null
cell, set `pendingText`/`currentSegmentPending` and continue).

## Key Points and Pitfalls

- The text layer is repacked with `this.lineSlots.get(identity)` (lines
  451-453, 464-466). That path already uses the right slot; do not change
  it. Only the recorded `slot` of non-text commands changes.
- `labelRun` binding labels draw in `overlayText`. After change A they
  carry slot 0, which is correct because their `x, y` are already
  viewport coordinates.
- Gutter commands get `+ scrollLeft` added when packed (lines 449, 463).
  Keep that. The shader snap absorbs fractional scroll.
- Quad height becomes `cell.height / scale`, which is at least
  `lineHeight`. Do not clamp it back to `lineHeight`.
- Cache-key stability: `glyphPhase` depends only on `glyphX` (relative to
  the line origin) and `scale`, so retained segments stay valid across
  vertical scroll. Do not mix `scrollLeft` or `scrollTop` into
  `glyphX`.
- File size: `renderer.ts` is 668 lines. Changes A-E add only a few
  lines. If the file would reach 1000 lines, stop and record it; do not
  split the file in this plan.

### Mechanical test reconciliation (gpu.test.ts, edit-cost.test.ts)

Allowed only where an assertion encodes RC1, RC3 or RC5 behavior, or the
old flags or colors. Expected cases:

- Filters such as `draw.flags === 0` meant "gutter glyph" (gpu.test.ts
  around line 647). Glyphs now also carry `SNAP_GLYPH`, so use
  `(draw.flags & CLIP_GUTTER) === 0`.
- Exact glyph rect widths or lefts, or `fillText` x values (for example
  gpu.test.ts line 276: `raster.text[0]!.x).toBe(-30)`), that change by
  the quantized phase or the texel-exact width. Update them to the new
  exact values computed from the helpers.
- Non-text draws expected with a nonzero slot or offset.
- Handoff notes from CR-01, CR-02 and CR-04 progress logs.

Never weaken an assertion to `toBeDefined`/`toBeTruthy`, delete a test,
or add `.skip`. List every changed assertion (file, line, old value, new
value, reason) in the progress log. gpu.test.ts must end under 1000
lines (`wc -l`); if a new helper import is needed, import from
`../code/recording-gl` only if that reduces lines, otherwise leave the
local helpers.

## Test Cases (`editor/test/code/renderer-layers.test.ts`)

Use `recordingGL`, `rasterizer`, `width` from `./recording-gl` (created
by CR-02) and a renderer built as in gpu.test.ts (`new
CanvasRenderer(canvas, layout, { gl, createCanvas, ledger })` with the
fixture view `{ width: 320, height: 100, scrollLeft: 0, scrollTop: 0,
left: 10, top: 20, gutter: 48 }`).

- 3-line document `# c\nuse-bpm 108\ns :kalimba > note :c5`, render with
  `cursor = doc length`, a selection annotation `0..3` and a diagnostic
  annotation over `note` on line 3 -> every recorded draw whose layer is
  not `'text'` has `slot === 0`.
- Same render -> the caret draw (palette cursor color, width 1) has
  `absTop === 2 * font.lineHeight`; the diagnostic underline draw has
  `absTop === 3 * font.lineHeight - 2`.
- Same render -> the background draw covering the viewport has
  `absTop === 0`.
- Text-layer glyph draws have `(flags & SNAP_GLYPH) !== 0`; at DPR 1 and
  2 (`setViewport(view, dpr)`), `rect[2] * dpr` is an integer, and
  `(rect[0] - slotX) * dpr` is an integer for document glyphs.
- Document `use-bpm 108` (line 1, gutter `1` drawn first) -> the
  text-layer instance for the document `1` has a CSS width below 20
  (not the 40 px label width), and differs in `uv` from the gutter `1`.
- `render()` records a `uniform2f` call on location `{ name: 'u_device' }`
  with `(canvas.width / 320, canvas.height / 100)`.
- The backdrop draw color equals `[0.047, 0.059, 0.082, 0.85]` within
  1/255.

The slot, caret, diagnostic, backdrop and document-`1` cases fail on
ea51a07. CR-07 records that as mutationEvidence.

## Invariants

- Retained text segments are still reused across scroll (the existing
  gpu.test.ts reuse and upload-count tests pass unmodified, unless they
  are listed as mechanical changes with a reason).
- No change to `input.ts`, `keyboard.ts` or the cursor block: CR-07
  checks `git diff ea51a07 -- editor/src/code/input.ts editor/src/code/keyboard.ts`
  is empty, and the cursor block text is byte-identical.

## Write Scope

Mirrors `impl-plans/active/code-render-dispatch.json` plans[CR-05].

- writePaths: `editor/src/code/renderer.ts`, `editor/test/code/renderer-layers.test.ts`, `editor/test/canvas/gpu.test.ts`, `editor/test/canvas/edit-cost.test.ts`, `impl-plans/active/code-render-05-renderer.md`, `tmp/code-render/CR-05`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`
- sharedPaths: none (`editor/test/code/recording-gl.ts` is read-only here; CR-02 owns it)
- artifactRoots: `tmp/code-render/CR-05`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules` (the last three are prebuilt read-only inputs: never rebuild, reinstall or delete them)
- Toolchain audit: before the first edit, from the repository root run
  `shasum -a 256 target/wasm32-unknown-unknown/debug/vactr.wasm tree-sitter-vact/tree-sitter-vact.wasm editor/package-lock.json > tmp/code-render/CR-05/toolchain-sha256.txt`
  and copy the three digests into the Progress Log.

## Working Rules

Fresh read plus sha256 before and after every edit. Re-read on drift.
CR-05 runs alone in wave 2, but on the same working tree; never overwrite
changes outside its writePaths. No git writes; foreground only. Logs go
under `tmp/code-render/CR-05/`.

## Verification

Run from `editor/`.

1. `npx vitest run test/code/renderer-layers.test.ts test/code/text-cells.test.ts test/code/atlas-cells.test.ts test/code/glyph-placement.test.ts > ../tmp/code-render/CR-05/focused.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-05/focused.log` -> exit 0.
2. `npx vitest run test/canvas > ../tmp/code-render/CR-05/canvas.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-05/canvas.log` -> exit 0 after reconciliation.
3. `npm run check > ../tmp/code-render/CR-05/check.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-05/check.log` -> exit 0.
4. `wc -l src/code/renderer.ts test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts > ../tmp/code-render/CR-05/wc.log` -> every count below 1000.

Full vitest, the browser e2e and pixel runs happen in CR-07 under the
measurement lock.

## Completion Criteria

- [ ] Non-text instances carry slot 0; the text slot is reset after each build
- [ ] Mask and run glyph quads use `glyphPhase`/`glyphQuad`, request `phase`, and set `SNAP_GLYPH`
- [ ] `u_device` set every frame
- [ ] Translucent literals converted to straight alpha
- [ ] `renderer-layers.test.ts` passes; `test/canvas` passes; every mechanical change listed
- [ ] Line counts below 1000; cursor block and input/keyboard untouched
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Starts after CR-01, CR-02 and CR-04 are complete.
