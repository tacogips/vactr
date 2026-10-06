# Canvas Cutover OPT-RENDER-A: Cell Atlas, Instanced Layers and One Text Draw Implementation Plan

**Status**: Completed
**Plan ID**: CANVAS-OPT-RENDER-A (session 291, wave 13; runs alone after CANVAS-OPT-BACKDROP, which is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 4 ("Glyph atlas", "Layers and draw calls", "GL hygiene", "Context loss, DPR and disposal", "Ported tests") and section 8 ("OPT-RENDER split (session-291 amendment)", block "CANVAS-OPT-RENDER-A")
**Parent plan**: impl-plans/active/canvas-cutover-opt-render.md (status `Split`; its "Contracts and Key Points" sections 1-3 are the reference contract this plan implements in part)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-RENDER-A`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

The user asked for a fast canvas editor, with WebKit as the primary target. Design 15.3.8.14
section 4 replaces the code renderer's one-`drawArrays`-per-quad path with three pieces: an
append-only cell atlas, instanced quads, and four layers with at most 4 draw calls per frame.
CANVAS-OPT-RENDER ran out of continuation attempts, so the operator split it into A, B and C,
which run strictly one after another. This plan, A, lands the final GPU structure. B (the
per-line geometry cache) and C (overlay-only frames and the badge fix) build on it and do not
change it.

**State at BASE = 4262255** (operator WIP checkpoint, already pushed):

- `editor/src/code/atlas.ts` (156 lines): `GlyphAtlas` with one texture, shelf packing,
  `cell(request)`, growth and reset that bump `stats.generation`, and a persistent staging
  canvas. It also has these defects, all to be fixed here:
  - `beginFrame` zeroes `stats.uploads` and `stats.newCells`, but renderer rows need
    cumulative counters;
  - each cell upload reads the pixels back with `getImageData`;
  - growth and reset run in the middle of a frame, which leaves stale UVs on instances
    already written that frame;
  - the temporary adapters `tile()`, `tilePixels` and `supportsRunRaster()` are still there.
- `editor/src/code/renderer.ts` (376 lines) still draws one `drawArrays` per quad with the
  `u_rect`, `u_color` and `u_uv` uniforms. It toggles `scissor` for the gutter clip and keys
  its rebuild on `JSON.stringify([... cursor, handles])` (`renderer.ts:191`).
- `editor/src/code/geometry.ts` (26 lines) has `INSTANCE_BYTES = 32`,
  `INSTANCES_PER_BLOCK = 64`, `MAX_TEXT_BLOCKS = 1536`, the `GeometryLayer` type, and
  `packInstances()`, which writes rect, uv and color but not `a_meta`.
- `npm run check` exits 0. `editor/test/canvas/gpu.test.ts` passes 33 of 49 rows; the
  16 failing rows are listed under "Ported rows". Log:
  `tmp/canvas-cutover/opt-render/focused-atlas-2.log`.

## Non-goals

- No per-line segment cache, block free list, compaction or incremental slot table. Those
  belong to CANVAS-OPT-RENDER-B. In A, a text-dirty frame may rebuild every visible line's
  text instances and rewrite the whole slot table.
- No `mount.ts` feedback changes: no `staticRevision` removal and no selection-only frame
  path. Those belong to CANVAS-OPT-RENDER-C. Do not edit `editor/src/code/mount.ts` unless the
  renderer API change forces a compile fix; record any such edit.
- No `code.css` edit (the badge fix is C's).
- No change to hit testing, caret or boundary semantics (`layout.ts` `coordsAtPos`,
  `posAtCoords`, `boundary`, `rangeRects`).
- No change to the 15.3.5 caps (atlas 16 MiB, geometry and staging 8 MiB, layout 8 MiB,
  canvas 4 M pixels, total 96 MiB), to the 1 MiB per-frame upload budget, to `GpuStatus`, or
  to the `CodeSurface` and `CodeApi` contracts.
- No WebGL extension. Core WebGL2 only: `drawArraysInstanced`, `vertexAttribDivisor`,
  `vertexAttribIPointer`, `texelFetch` and `RG32F`.
- No Rust, no dependency, no threshold, no harness assertion change.

## Ownership

writePaths (concrete files; the same set for A, B and C, per design section 8):

- `editor/src/code/renderer.ts`
- `editor/src/code/geometry.ts`
- `editor/src/code/atlas.ts`
- `editor/src/code/layout.ts` (additive accessors only)
- `editor/src/code/advances.ts` (additive raster helpers only, if needed)
- `editor/src/code/resources.ts`
- `editor/src/code/mount.ts` (compile fixes only in A)
- `editor/src/code/perf-hook.ts` (additive stats fields only)
- `editor/src/code/code.css` (no edit in A)
- `editor/src/code/segments.ts` (reserved; create only if `renderer.ts` or `geometry.ts`
  would otherwise reach 1,000 lines)
- tests: `editor/test/canvas/gpu.test.ts`, `editor/test/canvas/mount.test.ts`,
  `editor/test/canvas/edit-cost.test.ts`, `editor/test/canvas/first-viewport.test.ts`,
  `editor/test/canvas/contracts.test.ts`, `editor/test/support/gl.ts`,
  `editor/test/support/canvas.ts`
- `impl-plans/active/canvas-cutover-opt-render-a.md` (checkboxes and progress log only)
- artifact roots: `tmp/canvas-cutover/opt-render-a`, `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`, `editor/dist`,
  `tmp/ui-style`

sharedPaths:

- `editor/src/code/frame.ts`: no edit expected.
- **Harness sharedPaths** (session-288 rule): edit only to align sampling to presented
  frames, and record the sha256 values:
  - `editor/test/e2e/behavior.mjs`
  - `editor/test/e2e/measure.mjs`
  - `editor/test/e2e/run.mjs`
  - `editor/test/e2e/stats.mjs`
  - `editor/test/e2e/stats.test.ts`
  - `editor/test/e2e/ios-sim.mjs`
  - `editor/test/e2e/README.md`
  - `editor/test/style/ui-style.mjs`
- **Session-290 sharedPaths**: edit only if the layer change breaks them. Keep each row's
  intent and every other assertion byte-identical, and record the sha256 values:
  - `editor/src/visual/panes.ts`
  - `editor/src/visual/render-host.ts`
  - `editor/src/visual/frame.ts`
  - `editor/src/app/theme.css`
  - `editor/test/canvas/frame.test.ts`
  - `editor/test/visual/frame.test.ts`
  - `editor/test/visual/meters.test.ts`
  - `editor/test/visual/panes.test.ts`
  - `editor/test/visual/render-host.test.ts`
  - `editor/test/visual/scopes.test.ts`
  - `editor/test/visual/text-asset.test.ts`
  - `editor/test/visual/video.test.ts`

**Harness rules.** Never change a check name, assertion, expected value, `THRESHOLDS`,
`TARGETS`, the workload, the fixtures, the silent sink, the `run.mjs` exit codes, the
`ios-sim.mjs` predicate or the `ui-style.mjs` checks. If a probe reads a renderer stat that
this plan would remove, keep that stat; do not edit the probe.

## Invariants (must hold after A)

- **Draw count.** At most 4 draw calls per frame, all `drawArraysInstanced`. No `drawArrays`
  call and no `scissor` or `SCISSOR_TEST` toggling on the code canvas.
- **Text draw.** For a 60-line viewport the text layer is exactly 1 draw.
- **No GPU queries on the hot path.** No `getError`, `getParameter` or `isTexture` in
  `render()`, in a cell upload, or in a layer rewrite. `getError` runs only:
  - in `initialize`;
  - on restore;
  - after a (re)allocation: the canvas backing store, the atlas texture, the slot-table
    texture, or a layer buffer whose capacity grows (`bufferData`).

  `getParameter(MAX_TEXTURE_SIZE)` runs only in `initialize` and on restore. The atlas may
  read it in its constructor (the session-290 allowed init read) or receive it from the
  renderer; either way a restore makes at least 1 fresh read.
- **Ledger.** Every GPU and staging allocation is reserved before it is made, and
  `dispose()` brings `ledger.usedBytes` back to 0.
- **Rebuild key.** The text layer is rebuilt only when one of these changes: the text
  revision, the syntax rows of `feedback.annotations` (compared by per-line style hash), the
  visible window (scroll, size), the font generation, the DPR, or the atlas generation.
  `cursor`, `handles`, `cursorVisible` and the non-syntax annotations never trigger a text
  rebuild.
- **Feedback shape.** `RenderFeedback` keeps its shape. Syntax rows stay in
  `feedback.annotations` (`kind === 'syntax'`). `mount.test.ts:264-291` passes unchanged.

## Contracts and Key Points

### 1. Atlas (`editor/src/code/atlas.ts`, `class GlyphAtlas`)

Finish the parent plan's contract section 1 on top of the WIP code.

- **API.** It stays as below. Delete `tile()`, `tilePixels` and `supportsRunRaster()`, and
  update every reader (renderer and tests).
  - `cell(request: CellRequest): AtlasCell | null`
  - `beginFrame(budgetBytes?: number)`, `endFrame()`, `invalidate()`, `contextLost()`,
    `dispose()`, `get texture()`, `get size()`
  - `stats = { uploads, newCells, hits, resets, growths, generation }`. Every counter is
    **cumulative** and never zeroed by `beginFrame`. Keep a separate private per-frame byte
    counter. `evictions` may stay as an alias equal to `resets`, if a reader needs it.
- **Uploads.** An upload rasterizes into the persistent staging canvas and calls
  `texSubImage2D(TEXTURE_2D, 0, x, y, w, h, RGBA, UNSIGNED_BYTE, stagingCanvas)` with the cell
  rectangle at the staging origin. That is one call per new cell, so `uploads <= newCells`.
  - Do not call `getImageData`: it is a synchronous readback and is slow in WebKit.
  - If the staging canvas is larger than the cell, use the WebGL2 unpack sub-rectangle with
    `UNPACK_SKIP_PIXELS`/`UNPACK_SKIP_ROWS` at 0 and `UNPACK_ROW_LENGTH` at 0, which takes
    the top-left w x h rectangle. Never leave a skip value non-zero.
- **Mask cells are white.** `fillStyle = '#ffffff'`, and the tint is per-instance color. A
  mask or color cell's key does not include a color. Run cells are also rasterized white,
  clipped to their piece, and tinted per instance with the piece's style color. This is how
  syntax color works without per-color cells.
- **Growth and reset happen only between frames.** When a cell does not fit during a frame:
  1. the atlas stops allocating for the rest of that frame (`cell()` returns `null`, and the
     renderer marks `text-pending`);
  2. it records `pendingGrowOrReset`;
  3. at the next `beginFrame` it grows (1024x1024 -> 2048x1024 -> 2048x2048, bounded by
     `MAX_TEXTURE_SIZE` and the 16 MiB atlas cap, reserving before allocating), or resets at
     the maximum size, and bumps `generation`.

  Never reset or grow in the middle of a frame: instances already written that frame would
  point at freed texels.
- **Allocation failure.** If the texture allocation in the constructor or in growth fails
  (`createTexture` returns null, or `getError` after `texImage2D` is non-zero), release every
  reservation made for that allocation and delete the new texture. A constructor failure
  throws `Error('Text atlas allocation failed')`. A growth failure falls back to a reset at
  the current size.
- **White texels.** The 2x2 white texels at (0, 0) stay. Rect instances sample their center.
- **Staging canvas.** `createCanvas()` is called once per atlas lifetime. The canvas is
  resized only when the maximum cell height grows, and its reservation is in the `geometry`
  kind. `endFrame()` frees nothing.

### 2. Geometry (`editor/src/code/geometry.ts`)

- **Instance format** (32 bytes, little-endian), fixed from here on:

  | Attribute | Offset | Type |
  |---|---|---|
  | `a_rect` | 0 | f32 x4 (x, y, w, h) |
  | `a_uv` | 16 | u16 x4, normalized |
  | `a_color` | 24 | u8 x4, normalized |
  | `a_meta` | 28 | u16 x2 (slot, flags), integer attribute via `vertexAttribIPointer` |

  Flags: `CLIP_GUTTER = 1`, `UNTINTED = 2`, `TEXT_SPACE = 4`, `NO_HSCROLL = 8`. Export the
  flag constants.
- **Packing.** Replace `packInstances` with a writer into a reusable scratch
  `ArrayBuffer`/`DataView` that grows by doubling, so a frame does not allocate one
  `ArrayBuffer` per call. Pin it as:
  `class InstanceWriter { clear(): void; push(rect, uv, color, slot, flags): void; get count(): number; bytes(): Uint8Array }`.
- **Shaders.** Export `VERTEX_SOURCE` and `FRAGMENT_SOURCE` for one program:
  - The vertex shader expands a unit quad (6 vertices of a static `a_corner` buffer,
    divisor 0) by the instance rect.
  - When `TEXT_SPACE` is set:
    - `y = texelFetch(u_slots, ivec2(slot, 0), 0).r + rect.y - u_scroll.y`;
    - `x = u_origin.x + rect.x - u_scroll.x`, where `u_origin.x` is the gutter width in CSS
      px.
  - When `NO_HSCROLL` is set, x is the rect x as given (gutter digits).
  - View-space instances, which have neither flag, use the rect as given.
  - The fragment shader samples `u_atlas` at the interpolated UV, and:
    - outputs the texel when `UNTINTED` is set;
    - otherwise outputs `vec4(color.rgb * color.a, color.a) * texel.a`;
    - discards when `CLIP_GUTTER` is set and `gl_FragCoord.x < u_gutterPx` (the gutter in
      device px).
  - Blending stays `ONE, ONE_MINUS_SRC_ALPHA`.
- **Slot table.** An `RG32F` texture of `MAX_SLOTS = 1024` x 1, with `NEAREST` filtering (a
  float texture must not use `LINEAR`), read only through `texelFetch`. In A, a text rebuild
  assigns slot i to the i-th visible line, writes `y = line.number * lineHeight` for each
  slot, and uploads the whole used prefix with one `texSubImage2D` of a `Float32Array`.
  Count the bytes in `stats.slotTableBytes`.
  - When the window has more lines than `MAX_SLOTS`, draw at most `MAX_SLOTS` lines this
    frame and set `text-pending`. Never index past 1023.
- **Layers.** Export `class LayerBuffer` (one per layer: `background`, `text`, `overlay`,
  `overlayText`). Each owns:
  - one `ARRAY_BUFFER`, one VAO bound to the shared `a_corner` buffer and its own instance
    buffer (divisor 1 for the four instance attributes);
  - a ledger reservation in the `geometry` kind equal to its byte capacity.

  `write(writer)`:
  - uses `bufferSubData` when the bytes fit;
  - otherwise reserves the new capacity (doubling, minimum 64 instances), calls
    `bufferData` once, checks `getError` once (an allowed allocation check), and releases the
    old reservation;
  - on failure, releases the new reservation and throws.

  `draw(gl)` issues one `drawArraysInstanced(TRIANGLES, 0, 6, count)` and is skipped when
  `count === 0`. Count text-layer writes in `stats.bufferUploads` (rename semantics: text
  buffer writes) and every draw in `stats.drawCalls` (total), plus `stats.lastFrameDraws`.

### 3. Renderer (`editor/src/code/renderer.ts`)

- **Kept API.** The constructor, `status`, `textPending`, `atlasStats`, `setPhases`,
  `setDocument`, `setText`, `setPalette`, `setViewport`, `render(feedback)`, `dispose`, and
  the loss and restore handlers. `stats` gains `textBuilds` (one per text-layer rebuild in
  A), `slotTableBytes`, `geometryBytes` (text-layer bytes written) and `lastFrameDraws`.
- **`initialize`.** Compiles the single program and creates the shared corner buffer, the
  four `LayerBuffer`s (empty, no capacity yet), the slot-table texture and the atlas. Cache
  the uniform locations. Allocation order and rollback follow today's
  `renderer.ts:initialize`/`releaseGpu`.
- **`render()` passes**, in this order:
  1. `atlas.beginFrame()`. If the atlas generation changed since the last text build, treat
     the text as dirty.
  2. *Text layer,* only when the rebuild key changed. For each visible line, starting at the
     `pendingStartLine` rotation as today, and for each run, the line takes one of two paths:
     - *Run line:* `TextLayout.isRunLine(line)` is true for non-additive fonts and
       complex-script lines. The line emits run cells per style piece, splitting a piece
       wider than the atlas at cluster boundaries via `offsetInRun`.
     - *Additive line:* the line uses `TextLayout.clusters(...)` and emits mask or color
       cells per grapheme. `\p{Extended_Pictographic}` clusters are color cells with
       `UNTINTED`.

     Every instance has `TEXT_SPACE | CLIP_GUTTER`, its slot, line-local x, y = 0, and a
     color from the palette token of the covering syntax row. Gutter numbers are mask cells
     per digit with `TEXT_SPACE | NO_HSCROLL`, right-aligned at `gutter - 8`, with no
     `CLIP_GUTTER`.

     If `cell()` returns `null`, set `textPending`, remember the rotation start, and continue
     with the other lines' already-resident cells; never omit a line silently. Then write the
     slot table and the text layer once.
  3. *Background layer* (view space): the scrim, the selection rects, then the playing and
     eval rects. `animatedRects` finds the first visible line by binary search over
     `visibleLines`; there is no loop over ranges times lines. It is rewritten every frame in
     A (it is small).
  4. *Overlay layer* (view space): in order:
     1. the diagnostic and composition underlines;
     2. the call-head underlines;
     3. the caret, when `cursor != null` and `cursorVisible !== false`;
     4. the binding-label boxes;
     5. the handles.

     Rect instances use the white texel. It is rewritten every frame in A.
  5. *Overlay text layer:* the binding-label glyphs, as mask cells tinted `labelText`.
  6. Draw, in this order: background, text, overlay, overlay text. Set the uniforms once per
     frame: `u_view`, `u_scroll`, `u_origin`, `u_gutterPx`, `u_atlas = 0` and
     `u_slots = 1`, with both textures bound to units 0 and 1.
- **Status and DPR.**
  - RTL degraded reporting is unchanged.
  - `resize()` keeps its `getError` (allowed: canvas backing allocation).
  - A DPR change invalidates the atlas (reset next frame) and the layout, as today.
- **Context loss and restore.** On loss, release every reservation, drop the layers and
  atlas, and draw nothing until restore. On restore, re-run `initialize` (a fresh
  `MAX_TEXTURE_SIZE` read) and mark the text dirty.

### 4. Layout accessors (`editor/src/code/layout.ts`, additive only)

- `isRunLine(line: ShapedLine): boolean`: true when the line's run width index is not
  additive (`RunWidthIndex.additive === false`) or the line is complex-script.
- `clusters(line: ShapedLine, from: number, to: number, visit: (text: string, x: number, width: number) => void): void`:
  visits the grapheme clusters of an additive line in `[from, to)` with their line-local x and
  width from the OPT-TEXT advance table. It makes no `measureText` call.

Do not change any existing method's results.

### Patterns to imitate

- `renderer.ts:initialize`/`releaseGpu`: allocation order and rollback on failure.
- `resources.ts` `ResourceLedger.allocate`: reserve before allocating.
- `gpu.test.ts:55` `recordingGL()` and `gpu.test.ts:298` `fixture()`.

## Test fake extension (`gpu.test.ts` `recordingGL()`, and `editor/test/support/gl.ts` where the mount tests need it)

Add these members, keeping every existing member:

- `createBuffer` plus `bindBuffer`, `bufferData` and `bufferSubData` that store the bytes per
  buffer object (a `Uint8Array` copy).
- `vertexAttribPointer`, `vertexAttribIPointer` and `vertexAttribDivisor`, recorded per bound
  VAO (the attribute index, the buffer bound to `ARRAY_BUFFER` at the time, the offset and
  the divisor).
- `uniform2f`/`uniform1f`/`uniform1i` values kept by location name.
- `texImage2D`/`texSubImage2D` on the slot texture: keep a `Float32Array` source, so the fake
  knows the slot y values.
- `drawArraysInstanced(mode, first, count, instances)`: decodes the bound VAO's instance
  buffer into the existing `draws` record shape, one record per instance with non-zero w
  and h, in buffer order:
  - `rect`: the view-space `[x, y, w, h]`, resolving `TEXT_SPACE` through the slot y, the
    origin and the scroll as the shader does;
  - `color`: the u8 values / 255;
  - `texture`: the texture bound to unit 0;
  - `textureLiveAtDraw`;
  - `scissor`: `(flags & CLIP_GUTTER) !== 0`;
  - extra fields `layer` (the draw index within the frame mapped to `background`, `text`,
    `overlay` or `overlayText`, from a `drawCalls` record), `flags` and `uv`.

  Also push one `drawCalls` record `{ name: 'drawArraysInstanced', instances }`.
- Constants: `RG32F`, `RG`, `FLOAT`, `UNSIGNED_SHORT`, `INT`, `UNPACK_SKIP_PIXELS`,
  `UNPACK_SKIP_ROWS`, `UNPACK_ROW_LENGTH`, `DYNAMIC_DRAW`, `TEXTURE1`.
- `failNextAllocation()`: the next `bufferData` or `texImage2D` sets a GL error. Keep
  `failNextUpload` as an alias.

Colors are u8-quantized. **Do not loosen comparisons to `toBeCloseTo`.** Compare against a
`q(color)` helper that applies the same rounding (`Math.round(c * 255) / 255`).

## Ported rows (the 16 red rows at BASE)

Port each row in cell and instance terms. The row name stays the same, and no `expect` is
deleted without an equivalent assertion of the same intent. Record a before/after note per
row in the progress log.

| Row (gpu.test.ts) | Port |
|---|---|
| "reuses one ledger-accounted raster canvas until disposal" | `createCanvas` called once across several `cell()` calls and frames. The geometry reservation includes the staging canvas and is **unchanged** by `endFrame` (staging is persistent per design section 4). `dispose` returns `usedBytes` 0. |
| "crops the whole run and masks syntax without reshaping substrings" | Run cells: every `fillText` gets the full run text `'ffi日本'`; the piece is clipped by a `rect` crop at the piece offset; dispose returns 0 and `live` is empty. |
| "rasterizes a styled multi-tile run once and releases its bounded raster" | A 32-char run split into pieces: one `fillText` of the full run per new cell, all with `text === run.text`; repeating the same requests gives hits (no new `fillText`); `createCanvas` called once; dispose returns 0. |
| "caches by text/font/fallback/DPR/style and deletes evicted tiles" | Use a ledger that fits the atlas. Distinct font, fallback, DPR or piece makes a new cell; a repeat is a hit; a color-only variant of a mask cell is a hit (tint is instance data). `invalidate()` then `beginFrame()` gives `resets` 1, a bumped `generation` and the per-cell reservations released. Dispose returns 0 and `live` is empty. |
| "rolls back reservations and textures on upload failure" | `failNextAllocation()` before construction: the `GlyphAtlas` constructor throws, `usedBytes` 0, `live.size` 0. |
| "bounds atlas misses per frame while allowing resident hits" | With a budget of exactly one cell's bytes, the first cell is created. With `beginFrame(0)` a resident cell is a hit (not `null`), and a new cell is `null`. `uploads` 1. |
| "draws all required feedback in order on GPU and clips source against gutter" | The decoded order is background (scrim, selection, playing, eval: the first 4 colors equal `q()` of today's values), then text, then overlay (diagnostic before caret, caret before the label box 0.15, the handles as the last two overlay instances with `q(handle)`), then overlay text (label glyphs). The gutter clip is `u_gutterPx === 48 * dpr`, source instances have `CLIP_GUTTER` and gutter instances do not. The `fillText` of `'let x = 1'` is the full run, and the instance covering `[0, 3)` is tinted `q(token head)`. A gutter `'1'` cell is rasterized. Dispose returns 0 and `live` is empty. |
| "idle frames reuse text uploads, layout and static geometry; edits rebuild only dirty tiles" | After the first render, take baselines for `atlasStats.uploads`, `l.stats.builds` and `stats.bufferUploads`. 4 cursor-only frames leave all three unchanged. `setDocument('let x = 2\n日本')` plus a render gives `uploads + 1`. |
| "animation-only frames replay cached text without shaping or uploads" | The same assertions with `bufferUploads` meaning text-layer writes. |
| "does no liveness, error or segmentation probes on animation-only frames" | Unchanged intent: `getError` is called by the `setViewport` reallocation. 10 animation frames give 0 `isTexture`, 0 `getError`, unchanged segmentations, `textBuilds` and uploads, and more decoded draws. |
| "keeps textPending until a bounded per-frame atlas upload completes" | The first frame has `textPending` true and `uploads` less than the total cells needed for the visible window. Within at most `ceil(totalCellBytes / 1 MiB) + 1` frames (bound computed in-test and recorded), `textPending` turns false, `uploads` equals the total, and every one of the 35 lines has at least one text instance. Dispose returns 0. |
| "retains text and save access after allocation/upload failure without DOM fallback" | `failNextAllocation()` before the first `render()`, so a layer buffer allocation fails: `render()` returns false, status `unavailable`, `saveText()` retained, `usedBytes` 0, `live.size` 0, and `setDocument` still updates `saveText()`. |
| "retains every unchanged text tile across animation frames without backdrop uploads" | The first-revision uploads equal the computed distinct cells (40 run cells plus the distinct gutter digit cells). The 40 `'line '` runs are rasterized once each. Later frames give unchanged uploads, an identical decoded text-layer texture list, unchanged `usedBytes`, `resets` 0 and backdrop 0. `texImage2D` is called only for allocations (the atlas and slot table), and `texSubImage2D` calls equal the cell uploads plus the slot-table writes. |
| "geometry pressure during animation does not evict text or allocate backdrop storage" | Under geometry pressure an animation frame gives backdrop 0, unchanged uploads and `resets` 0. The background layer must fit its existing capacity (no growth on animation-only frames with an unchanged animated range count). |
| "dense offscreen syntax on a 1MiB line cannot overflow visible-tile metadata" | Geometry bytes after the dense render minus geometry bytes after the same render with no annotations is below 10,000 (the persistent staging reservation is excluded by the difference), and crops are below 32. Dispose returns 0. |
| "keeps rendering all visible lines when atlas working set exceeds available budget" | Keep the 2,100,000-byte ledger, and use `recordingGL(N)` with N the smallest power of two whose atlas fits beside the canvas (record N), so the atlas, not the ledger, is the binding constraint. `resets > 0`. Every frame's newly rasterized lines have a same-frame text instance with a live texture. Every one of the 50 lines is drawn within the loop bound (raise the bound from 10 only to the computed `ceil(cells / cellsPerAtlas) + 2`, recorded). `usedBytes` stays within 2,100,000, and dispose returns 0. |

Ported rows must also stay green when this plan's change breaks a currently passing row. A
known case is "updates DPR when CSS viewport changes but backing dimensions stay equal": its
`scissor` arguments become `u_gutterPx === 48 * effectiveDpr`. Another is "replays gutter
numbers unscissored and source text clipped on cached frames": select text instances by
`layer === 'text'`, not by white color. No other assertion changes.

## New rows (`gpu.test.ts`, additive stub `measureText: t => ({ width: t.length * 8 })`)

- 60-line viewport -> `lastFrameDraws <= 4`, exactly 1 draw for the text layer, and more
  than 60 decoded text instances (the in-test control: the old per-quad path would have
  issued over 60 draws).
- Atlas growth: with `recordingGL(2048)`, fill the 1024x1024 atlas with distinct clusters
  over frames -> `growths` 1, `generation` bumped, and the text still has instances for
  every line after the bounded frames, with `text-pending` until done. Then, with
  `recordingGL(1024)` (maximum = current size), fill again -> `resets` >= 1. No frame
  writes an instance whose cell generation differs from the atlas generation at draw time;
  assert through the decoded UVs being within the current allocation.
- An emoji cluster gives one color cell, drawn with `UNTINTED`. A Hebrew line gives run
  cells, and `rtlUnsupported` reporting is unchanged.
- 100 animation frames with playing ranges -> 0 `getError`, 0 `getParameter`, 0
  `isTexture`, 0 cell uploads, 0 `createCanvas`, 0 `measureText` on any 2D context, and
  `lastFrameDraws <= 4` every frame. The in-test control is one text edit in the same test,
  which must upload at least 1 cell.

## Pitfalls

- Use `vertexAttribIPointer` for `a_meta`, not `vertexAttribPointer`, or the slot and flags
  arrive as floats.
- A float texture must use `NEAREST` filtering and be read through `texelFetch` only.
- Keep the glyph pad in both the raster and the quad. Do not crop the overhang.
- Run cells are clips of the full run. Never rasterize a substring of a shaped run.
- Do not reset or grow the atlas in the middle of a frame (see "Atlas").
- `stats` counters are cumulative. Do not zero them in `beginFrame`.
- Do not free the staging canvas in `endFrame`.
- Do not keep `drawArrays` or scissor toggling "for the gutter". The flag replaces it.
- Do not change `mount.ts` feedback; C owns it.

## Verification

**Setup** (not gating):

- `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`

Record `BASE=4262255`, `START` (HEAD at dispatch) and the fresh-read sha256 values in
`tmp/canvas-cutover/opt-render-a/intent.json`.

**Inside the sandbox:**

| Command | Required evidence |
|---|---|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts` | exit 0; at least 53 tests (49 ported or kept, plus the new rows), 0 failed |
| `cd editor && ./node_modules/.bin/vitest run test/canvas` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0; every file passes |
| `cd editor && npm run check` | exit 0 |
| `wc -l editor/src/code/renderer.ts editor/src/code/geometry.ts editor/src/code/atlas.ts editor/src/code/layout.ts` | each below 1000 (notes only) |
| `grep -n "getImageData\|JSON.stringify\|getError\|drawArrays(\|scissor" editor/src/code/atlas.ts editor/src/code/renderer.ts editor/src/code/geometry.ts` | no `getImageData`, no `JSON.stringify`, no `drawArrays(`, no `scissor`; `getError` only in allocation paths (notes only) |

**Outside the sandbox** (verification or review step; run each heavy suite alone):

| Command | Required evidence |
|---|---|
| `mise run build-wasm-release`, then `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 |
| `cd editor && npm run e2e -- --browser all --profile behavior --run-id s291-opt-render-a --out ../tmp/canvas-cutover/opt-render-a/behavior` | exit 0; canvas-only text, line-1 readback, IME, context loss and restore, DPR and resize pass in Chromium and WebKit |
| `cd editor && npm run test:style` | exit 0 |
| `cd editor && npm run test:perf` (alone) | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` (alone) | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |

**Record format.** Test commands use
`{command, exitStatus: 0, testsRun > 0, testsPassed, failureCount: 0, outcome: "passed", log}`;
other commands use `{command, exitStatus: 0, outcome: "passed", log}`. Details go in `notes`.
Run no mutation or negative-control command. Repaired findings go only into
`addressedFeedback`/`resolvedFindings` with status `"repaired"`. Use severity `low` or none for
residual notes.

## Overwrite and Drift Protocol

- Read each file fresh before editing it, and record its sha256 in `intent.json`. After the
  last edit, record the post-edit sha256 in
  `tmp/canvas-cutover/opt-render-a/receipt.json`.
- If a file changed without an edit from this plan, stop editing that file and report it.
- Edit only this plan's progress log. Do not edit the parent, B or C plans.

## Completion Criteria

- [x] `GlyphAtlas`:
  - [x] mask, color and run cells;
  - [x] cumulative stats;
  - [x] growth and reset only between frames, with a generation bump;
  - [x] `texSubImage2D` from the staging canvas, with no `getImageData` and no per-upload
    `getError`;
  - [x] `tile`, `tilePixels` and `supportsRunRaster` removed.
- [x] `geometry.ts`:
  - [x] 32-byte writer with slot and flags metadata;
  - [x] the flag constants;
  - [x] `InstanceWriter`;
- [x] one program with the slot-table `texelFetch` path and the gutter discard;
- [x] `LayerBuffer` with ledger-reserved capacity.
- [x] Renderer: four layers, at most 4 `drawArraysInstanced` per frame, exactly 1 text draw,
  no `drawArrays` or scissor, and a text rebuild only on the rebuild key.
- [x] The 16 rows above are ported, plus any row this change broke. `gpu.test.ts` passes
  every row (at least 53).
- [x] New rows: 60-line draws, growth and reset, emoji and Hebrew, 100 animation frames.
- [x] Full default vitest and `npm run check` pass. The behavior e2e, `test:style` and
  `test:perf` pass. Rust gates were not run per the task instruction that Rust is untouched.
- [x] Harness and session-290 sharedPaths are unedited.
- [x] Progress log updated.
- [ ] Session 292: final-source gates rerun on `START=35140bd` with logs (section below).
- [ ] Session 292: Rust carry-forward records added with the empty-diff proof.
- [ ] Session 292: test-integrity, adversarial and integration review accepted.

## Session 292 Resume (re-gate on 35140bd; no reimplementation)

**State.** A is implemented at `35140bd` by an operator-directed Codex gpt-6-luna pass
(progress log entry "CANVAS-OPT-RENDER-A completion"). `git diff --name-only 4262255 35140bd`
lists only `atlas.ts`, `geometry.ts`, `layout.ts`, `renderer.ts`, `first-viewport.test.ts`,
`gpu.test.ts`, `mount.test.ts`, `editor/test/support/gl.ts` and plan/design files, all
inside this plan's writePaths. Record `BASE=4262255` and `START=35140bd` in
`tmp/canvas-cutover/opt-render-a/intent.json`.

**What the step-6 implementer does.** Rerun the gates below once on `START`, serially, each
heavy suite alone. Do not rewrite source. Change source only when a gate fails or a review
finding requires it; then fix it inside this plan's writePaths, rerun the affected gates and
report only the final passing runs.

**Gates on START** (logs under `tmp/canvas-cutover/opt-render-a/`, prefix `s292-`):

- inside the sandbox: `vitest run test/canvas/gpu.test.ts` (>= 53 tests), `vitest run
  test/canvas`, full `vitest run` (>= 785), `npm run check`, the host-wasm wasm32 build,
  strict clippy and the src-tauri cargo check;
- outside the sandbox (verification step): the behavior e2e with
  `--run-id s292-opt-render-a --out ../tmp/canvas-cutover/opt-render-a/s292-behavior`,
  `npm run test:style` and `npm run test:perf` (alone).

**Rust carry-forward.** Rust has not changed since `5e58d04`. Proof (notes, not a gate):
`git diff --name-only 5e58d04 HEAD -- src Cargo.toml Cargo.lock build.rs editor/src-tauri`
prints nothing. The only `mise.toml` change since then is the `build-wasm-release` task
(2e3361d), which does not affect the debug or test builds. Full nextest is therefore carried
forward from the last green full-suite run,
`tmp/canvas-cutover/opt-backdrop/nextest-final-source.log` (2816 run, 2816 passed,
3 skipped, exit 0), as one record:
`{command, exitStatus: 0, testsRun: 2816, testsPassed: 2816, failureCount: 0,
outcome: "passed", log, notes: "carry-forward: Rust unchanged since 5e58d04"}`.
If a fresh clippy, wasm32 or src-tauri run cannot run in the sandbox, carry it forward the
same way from `clippy-final-source.log`, `wasm-final.log` or `tauri-check-final.log` in the
same directory. The closeout in CANVAS-EVIDENCE reruns every Rust gate fresh.

**Test-integrity review brief** (review step; read-only).
`git diff 4262255 35140bd -- editor/test/canvas/gpu.test.ts` removes 37 `expect` lines and
adds 57. `mount.test.ts` and `first-viewport.test.ts` remove none. Two row titles changed:

- "caches by text/font/fallback/DPR/style and deletes evicted tiles" became "caches mask
  cells by text/font/fallback/DPR while tint stays per instance";
- "keeps rendering all visible lines when atlas working set exceeds available budget" became
  "keeps rendering every visible line while a small atlas working set stays pending".

For every removed `expect`, the reviewer names the HEAD assertion that keeps its intent,
using the term map of the parent plan's "Ported rows" table: tile -> cell, per-quad
`drawArrays` -> decoded instances, `scissor` -> `CLIP_GUTTER` and the gutter uniform,
`bufferUploads` -> text-layer buffer writes, evicted tile deletion -> reset or generation
bump releasing cells, style in the tile key -> per-instance tint with the syntax color still
asserted. A removed intent with no counterpart is a finding. The A implementer repairs it by
restoring the assertion in cell or instance terms. It is never accepted as an exemption.

**Deadline.** If the step deadline cuts a repair short, keep the partial work in the tree
and report it as a severity-low note.

## Progress Log

### Session: 2026-10-06 (session 291 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 8 ("OPT-RENDER split
(session-291 amendment)").
**Notes**:
- BASE=4262255.
- The 16 red rows come from `tmp/canvas-cutover/opt-render/focused-atlas-2.log`.
- A fixes the instance format, the program and the layer set, and B and C must not change
  them.

### Session: 2026-10-06 (Step 6 implementation pass)
**Tasks Completed**: Added the reusable 32-byte instance writer and migrated `drawRun()` to
the cell-based atlas API. Reworked `GlyphAtlas` to use persistent staging-canvas uploads,
cumulative counters and deferred generation-changing growth/reset.
**Verification**:
- `cd editor && npm run check`: exit 0 (`tmp/canvas-cutover/opt-render-a/check-final.log`).
- `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts`: exit 1, 49 tests,
  36 passed, 13 failed (`tmp/canvas-cutover/opt-render-a/gpu-final.log`). Failures show the
  remaining port work for legacy atlas expectations and the unimplemented instanced renderer.
**Remaining**: Layer shaders/buffers and four-layer instanced batching; syntax/emoji cell
color handling; 60-line, growth/reset and 100-frame cases; port all failing rows; full
vitest, browser/style/perf and Rust gates. This assigned implementation is incomplete.

### Session: 2026-10-06 (CANVAS-OPT-RENDER-A completion)
**Tasks Completed**: Replaced per-quad rendering with four WebGL2 instanced layers and an
RG32F slot table; added the 32-byte instance format, ledger-backed layer buffers, shader
gutter discard, cluster mask/color cells, shaped-script run cells, deferred atlas growth and
reset, and text-only cache invalidation. Ported the GPU rows to cell/instance semantics and
added deterministic batching, atlas lifecycle, emoji/Hebrew and animation-counter cases.
No new source files were needed.
**Verification** (all commands were run serially; each exited 0):
- `cd editor && npm run check`: exit 0; `tmp/canvas-cutover/opt-render-a/check.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts`: 53/53 passed;
  `tmp/canvas-cutover/opt-render-a/gpu.test.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas`: 205/205 passed across 11 files;
  `tmp/canvas-cutover/opt-render-a/canvas.test.log`.
- `cd editor && ./node_modules/.bin/vitest run`: 785/785 passed across 93 files;
  `tmp/canvas-cutover/opt-render-a/vitest-full.log`.
- `cd editor && npm run test:style`: exit 0, all four engine/viewport combinations passed;
  `tmp/canvas-cutover/opt-render-a/style.log`.
- `cd editor && npm run e2e -- --browser all --profile behavior --run-id s291-opt-render-a --out ../tmp/canvas-cutover/opt-render-a/behavior`:
  exit 0, 18/18 behavior checks passed across Chromium and WebKit;
  `tmp/canvas-cutover/opt-render-a/behavior.log`.
- `cd editor && npm run test:perf`: exit 0, 1/1 passed;
  `tmp/canvas-cutover/opt-render-a/perf.log`.
- Forbidden-path scan found no `getImageData`, `JSON.stringify`, `drawArrays(`, or scissor
  use in `atlas.ts`, `renderer.ts` or `geometry.ts`. Remaining `getError`/`getParameter`
  calls are initialization, backing-store allocation, atlas allocation or reset paths.
- Source line counts: renderer.ts 423, geometry.ts 111, atlas.ts 197, layout.ts 458.
**Not Run**: Cargo clippy, nextest and src-tauri Cargo checks, as the task states Rust is
untouched and those gates are not required. E2E did run its required release wasm/editor
build prerequisites. No harness or session-290 sharedPath was edited.
