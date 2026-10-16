# Code Render 05: Renderer Wiring - Identity Slots, Snapped Glyph Quads, Straight-Alpha Literals (RC3, R5, R7, R8, R11)

**Status**: In Progress
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

- [x] Non-text instances carry slot 0; the text slot is reset after each build
- [x] Mask and run glyph quads use `glyphPhase`/`glyphQuad`, request `phase`, and set `SNAP_GLYPH`
- [x] `u_device` set every frame
- [x] Translucent literals converted to straight alpha
- [x] `renderer-layers.test.ts` passes; `test/canvas` passes; every mechanical change listed
- [x] Line counts below 1000; cursor block and input/keyboard untouched
- [x] Progress log updated

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Starts after CR-01, CR-02 and CR-04 are complete.

### Session: 2026-10-16 CR-05 implementation
**Tasks Completed**: Renderer slot scoping, glyph phase/quad wiring, device uniform, straight-alpha literals, renderer-layer regression test, canvas GPU assertion reconciliation.
**Tasks In Progress**: Resolve the combined canvas suite measurement assertion with its owning upstream plan; rerun the full canvas suite.
**Blockers**: `editor/test/canvas/mount.test.ts:321` is outside CR-05 writePaths and fails in the combined tree with 53 `measureText` calls where the test expects 0. CR-05 does not edit `layout.ts` or invoke layout measurement; request the CR-01/serial integration owner to diagnose and repair within its ownership, then rerun `cd editor && npx vitest run test/canvas`.
**Notes**:
- `editor/src/code/renderer.ts`: `addCommand` now stores slot 0 outside the text layer; gutter and segment builds reset `currentSlot`; both mask and run paths request phase-specific cells, use `glyphQuad`, and set `SNAP_GLYPH`; `u_device` is looked up and uploaded every render; backdrop, selection, playing, eval-error and eval literals are converted to straight alpha. Cursor block remains untouched.
- `editor/test/code/renderer-layers.test.ts`: added DPR 1/2 checks for non-text slots, cursor/diagnostic/backdrop rows, snapped glyph dimensions, gutter/document `1` UV separation, device uniform and backdrop color.
- Mechanical `editor/test/canvas/gpu.test.ts` changes: the first four color expectations and playing/selection expectation changed to straight-alpha RGB; gutter selection changed from `flags === 0` to `(flags & CLIP_GUTTER) === 0` at the cached-frame and scroll cases because glyphs now carry `SNAP_GLYPH`; upload count changed from one cell per unique character (15) to a bound of 1..8 cells per distinct glyph (up to two widths times four phases), with the existing later-frame no-upload, no-eviction and same-texture assertions retained. Observed first upload count is 34.
- Verification: final focused suite passed 20/20 (`tmp/code-render/CR-05/focused-final.log`); `npm run check` exited 0 (`check-final.log`); `test/canvas/gpu.test.ts` passed 62/62 (`gpu-attempt-2.log`); final `test/canvas` exited 1 with 222/223 passed and only the mount measurement failure (`canvas-final.log`); line counts are renderer 673, gpu.test.ts 993, edit-cost.test.ts 346 (`wc.log`).
- Toolchain digests captured before edits in `toolchain-sha256.txt`: vactr.wasm `de9c55f37b4037d73e28f8aaa11e06d3bc87a3a41a3bc95dd8e64ebf02c93210`, tree-sitter-vact.wasm `ae3e5cdf1217e943f19c3ff15f111801e537ba9b7e463f84d6bf8cedd7fd0609`, package-lock `f32e9585c409487e0d74ee9b8dc1880d4e54ff8610ab2cefa529828c53449749`.
- Input and keyboard files have no diff against `ea51a07`; source-level cursor-block byte comparison is recorded in the CR-05 author self-check.

### Session: 2026-10-16 CR-05 continuation verification
**Tasks Completed**: Re-ran CR-05 focused and canvas verification against the combined source after CR-01's accepted additive-run repair; updated completion criteria.
**Tasks In Progress**: None for CR-05. CR-07 owns full vitest, browser e2e/pixel checks, and final integration review.
**Blockers**: None.
**Notes**:
- The previous `canvas-final.log` failure (222/223; `mount.test.ts:321`, 53 `measureText` calls) is superseded by CR-01's accepted `layout.ts` derived-run metadata/additivity fix. The CR-05 rerun passes all 223 canvas tests; the mount test is unchanged.
- Current-source verification: focused suite 22/22 (`tmp/code-render/CR-05/attempt-4/focused.log`); canvas suite 223/223 under the measurement lock (`canvas.log`); `npm run check` exit 0 (`check.log`).
- Line counts: renderer.ts 673, gpu.test.ts 993, edit-cost.test.ts 346 (`tmp/code-render/CR-05/attempt-4/wc.log`); all remain below 1000.
- The earlier 222/223 log is retained as historical evidence. Full vitest and browser pixel/e2e validation remain assigned to CR-07.
