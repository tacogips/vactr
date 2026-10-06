# Canvas Cutover OPT-RENDER: Append-Only Cell Atlas, Instanced Per-Line Geometry and Four Draw Calls Implementation Plan

**Status**: In Progress
**Plan ID**: CANVAS-OPT-RENDER (session 286, wave 13; runs alone after CANVAS-OPT-BACKDROP is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 4 ("Glyph atlas", "Per-line geometry cache", "Layers and draw calls", "GL hygiene", "Context loss, DPR and disposal", "Ported tests", "Proof") and section 6; 15.3.3 (as amended: cluster rasterization, run cells for complex lines); 15.3.5 caps; 15.3.8.7 "GPU per frame" (as amended)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-RENDER`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

The canvas renderer issues one `drawArrays` per quad, with a `u_rect` uniform each:
`renderer.ts:275-284`, about 103 draws per frame. It also:

- rebuilds the whole draw list on every text-dirty frame, including caret moves, because
  `cursor` and `annotationsRevision` are in the cache key (`renderer.ts:191`);
- rasterizes per-run tiles (`atlas.ts:117-185`), each its own texture with a
  `JSON.stringify` identity;
- reallocates a staging canvas every frame (`atlas.ts:57-59,71-75`);
- calls `measureText` for color masks (`atlas.ts:104-105,163-164`);
- calls `getError` after every upload (`atlas.ts:178`).

The design replaces this with three pieces:

- an append-only cell atlas in one texture;
- a persistent instanced geometry buffer per visible line segment, updated with
  `bufferSubData` for changed lines only;
- four layers with at most four draw calls.

Caret, selection and handles live in overlay layers, so they never rebuild text geometry.

CANVAS-OPT-TEXT (accepted) provides the advance table: additive lines are placed by summed
ASCII and cluster advances, and non-additive fonts or complex-script lines keep run
measurement. CANVAS-OPT-BACKDROP (accepted) removed the backdrop upload, cached
`MAX_TEXTURE_SIZE` and added `palette.ts`.

## Non-goals

- No change to hit testing, caret or boundary semantics (`layout.ts` `coordsAtPos`,
  `posAtCoords`, `boundary`, `rangeRects` keep their results).
- No change to the 15.3.5 caps (atlas 16 MiB, geometry and staging 8 MiB, layout 8 MiB,
  canvas pixels 4 M, total 96 MiB) or to the per-frame upload budget (1 MiB) and its
  `text-pending` rule.
- No change to `GpuStatus`, the GPU-unavailable path, context-loss semantics or the
  `CodeSurface`/`CodeApi` contracts.
- No backdrop work (BACKDROP owns it). No syntax, layout-measurement, completion, bind or
  history work.
- No WebGL extension dependency (`WEBGL_multi_draw` and similar). Core WebGL2 only:
  `drawArraysInstanced`, `vertexAttribDivisor`, `texelFetch` and RG32F textures are core.
- No Rust, no dependency, no threshold change.

## Ownership

writePaths:

- `editor/src/code/renderer.ts`
- `editor/src/code/geometry.ts` (new: buffers, blocks, slot table, layers and shaders)
- `editor/src/code/atlas.ts`
- `editor/src/code/layout.ts` (additive cluster-position accessor only)
- `editor/src/code/advances.ts` (additive raster helpers only, if needed)
- `editor/src/code/resources.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/perf-hook.ts` (additive stats fields only)
- tests:
  - `editor/test/canvas/gpu.test.ts`
  - `editor/test/canvas/mount.test.ts`
  - `editor/test/canvas/edit-cost.test.ts`
  - `editor/test/canvas/first-viewport.test.ts`
  - `editor/test/canvas/contracts.test.ts`
  - `editor/test/support/gl.ts`
  - `editor/test/support/canvas.ts`
- `impl-plans/active/canvas-cutover-opt-render.md` (checkboxes and progress log only)
- `tmp/canvas-cutover/opt-render` (artifact root; also receives the behavior e2e output)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules/.vite`, `editor/dist`, `tmp/ui-style`

sharedPaths:

- `editor/src/code/frame.ts`: no edit expected (the dirty reasons exist since OPT-TEXT).
- Harness sharedPaths (session-288 ownership amendment; design 15.3.8.14 section 8). These
  are edited only for frame alignment. The readback probe (15.3.8.12) in `behavior.mjs` must
  keep its assertions unchanged:
  - `editor/test/e2e/behavior.mjs`
  - `editor/test/e2e/measure.mjs`
  - `editor/test/e2e/run.mjs`
  - `editor/test/e2e/stats.mjs`
  - `editor/test/e2e/stats.test.ts`
  - `editor/test/e2e/ios-sim.mjs`
  - `editor/test/e2e/README.md`
  - `editor/test/style/ui-style.mjs`
- Backdrop and render sharedPaths (session-290 ownership amendment; design 15.3.8.14
  section 8). These are the 15-file list minus the three already in writePaths here
  (`atlas.ts`, `gpu.test.ts`, `mount.test.ts`). They are edited only if the atlas, geometry
  or layer change breaks them. Keep the intent of every row, keep every other assertion
  byte-identical, and record each edit with sha256 values:
  - `editor/src/visual/panes.ts`
  - `editor/src/visual/render-host.ts`
  - `editor/src/visual/frame.ts`
  - `editor/src/app/theme.css` (existing token values never change)
  - `editor/test/canvas/frame.test.ts`
  - `editor/test/visual/frame.test.ts`
  - `editor/test/visual/meters.test.ts`
  - `editor/test/visual/panes.test.ts`
  - `editor/test/visual/render-host.test.ts`
  - `editor/test/visual/scopes.test.ts`
  - `editor/test/visual/text-asset.test.ts`
  - `editor/test/visual/video.test.ts`

### Harness sharedPaths (session 288)

The rules are identical to the "Harness sharedPaths (session 288)" section of
`impl-plans/active/canvas-cutover-opt-dom.md`. They are restated here so this plan stands
alone.

**Allowed edit.** Only frame alignment of harness sampling (wait for a presented frame,
then a bounded poll; reference 842cf6e), or documentation of that alignment.

**Never change:**
- a check name, assertion, expected value or comparison;
- `THRESHOLDS` or `TARGETS`;
- the workload, the fixtures or the silent sink;
- `run.mjs` exit codes or its gating refusal;
- the `ios-sim.mjs` predicate;
- the `ui-style.mjs` checks.

**Removed renderer stats.** This replaces the session-286 exception: if a probe reads a
renderer stat this plan would remove, keep that stat (the `RenderFeedback` shape is
already kept). Do not edit the probe. A non-alignment harness change is a blocker for a
serial plan-author amendment.

**Recording.** Record the path, reason and sha256 values in the progress log and in
`tmp/canvas-cutover/opt-render/intent.json` and `receipt.json`.

**Check.** `git diff <START> -- <path>` shows no removed or changed line containing
`expect(`, `assert`, `THRESHOLDS`, `TARGETS` or a check comparison.

## Contracts and Key Points

### 1. Cell atlas (`atlas.ts`, `class GlyphAtlas`, same file and class name)

- **Texture.** One RGBA8 `TEXTURE_2D` that holds every cell.
  - It starts at 1024x1024 (4 MiB `atlas` ledger reservation, made before allocation).
  - When it is full, it grows to 2048x1024, then 2048x2048, bounded by the cached
    `MAX_TEXTURE_SIZE` and the 16 MiB atlas cap. Growth reallocates and clears, bumps
    `generation`, and every segment re-requests its cells (re-rasterized under the per-frame
    budget).
  - At maximum size, a full atlas resets at the same size (bump `generation`).
  - A white 2x2 texel block is reserved at (0, 0). Rect layers sample it.
- **Packing.** Shelves, append-only; there is no per-cell eviction. Cell height is
  `ceil(lineHeight * dpr)`. Cell width is `ceil(advance * dpr) + 2 * pad`, with
  `pad = ceil(2 * dpr)` for glyph overhang. The quad is expanded by the same pad.
- **Cell kinds and keys** (plain strings; no `JSON.stringify`):
  - `m|<gen>|<dpr>|<cluster>`: a mask cell, rasterized in white with
    `fillStyle = '#ffffff'`, tinted per instance.
  - `c|<gen>|<dpr>|<cluster>`: a color cell for `\p{Extended_Pictographic}` clusters. It is
    drawn untinted (instance flag), with alpha from the glyph.
  - `r|<gen>|<dpr>|<runText>|<pieceFrom>|<pieceTo>|<chunkX>`: a run cell for non-additive or
    complex-script lines. It is the full run shaped by `fillText` and clipped to one
    style piece (today's clip method, `atlas.ts:162-168`), so contextual shaping and
    ligatures are preserved. Its width is limited to the atlas width; a longer run splits
    at cluster boundaries into consecutive cells (reuse `layout.ts` `MAX_RUN_CHARS`
    chunking and `offsetInRun` for the x positions).
- **Staging.** One persistent staging canvas (OffscreenCanvas when available, else
  `createCanvas()`). Its font, baseline and fill are set once per (font, generation, dpr). It
  is sized to `atlasWidth x maxCellHeight` and resized only when `maxCellHeight` grows.
  `endFrame` no longer frees it. It is reserved in the `geometry` ledger kind.
- **Uploads.** In-frame only. Each new cell is rasterized into the staging strip at its
  atlas x. At `endFrame`, or at the end of the text pass, each touched shelf uploads its
  dirty x-range with the WebGL2 sub-rectangle form
  (`texSubImage2D(TEXTURE_2D, 0, x, y, w, h, RGBA, UNSIGNED_BYTE, staging)` with
  `UNPACK_SKIP_PIXELS = x`, then reset to 0). If a test shows the sub-rectangle form is
  unreliable, use one `texSubImage2D` per new cell with a cell-sized source region. Either
  way `stats.uploads <= stats.newCells`.
  - The 1 MiB per-frame byte budget applies to rasterized cell bytes. Cells over budget are
    not created this frame; the segment is marked pending and `textPending` is true.
  - No `getError` on uploads.
- **API.** `cell(request: { kind: 'mask' | 'color' | 'run'; text: string; font: LayoutFont; dpr: number; width: number; piece?: { from: number; to: number }; run?: ShapedRun; chunkX?: number }): AtlasCell | null`,
  where `AtlasCell = { u0, v0, u1, v1, width, height, pad }` in texels.
  - `stats = { uploads, newCells, hits, resets, growths, generation }`.
  - `beginFrame(budget)`, `endFrame()`, `invalidate()` (reset), `contextLost()`,
    `dispose()`.
  - `tilePixels` and `supportsRunRaster` go away. Update every reader.

### 2. Geometry (`geometry.ts`)

- **Instance format**, stride 32 bytes:
  - `a_rect`: 4 x f32 (x, y, w, h). For text layers, x and y are line-local CSS px with y
    relative to the line top. For view layers, they are CSS view px.
  - `a_uv`: 4 x u16, normalized texel coordinates.
  - `a_color`: 4 x u8, normalized.
  - `a_meta`: 2 x u16 (slot, flags).
- **Flags**, bit values fixed:
  - `CLIP_GUTTER = 1`: discard fragments with `gl_FragCoord.x` below `gutter * dpr`;
  - `UNTINTED = 2`: output the premultiplied texel as is;
  - `TEXT_SPACE = 4`: y comes from the slot table and the scroll uniform;
  - `NO_HSCROLL = 8`: gutter digits.
- **Shaders.** One program.
  - The vertex shader reads the unit-quad corner (per-vertex) and the instance attributes
    (divisor 1). Text space uses
    `y = slotY(slot) - u_scroll.y` with
    `slotY = texelFetch(u_slots, ivec2(slot, 0), 0).r`, and `x - u_scroll.x` unless
    `NO_HSCROLL` is set.
  - The fragment shader computes `texel = texture(u_atlas, uv)`. The result is `texel` when
    `UNTINTED` is set, else `vec4(color.rgb * color.a, color.a) * texel.a`.
  - Blending stays `ONE, ONE_MINUS_SRC_ALPHA`.
- **Slot table.** An RG32F texture, one row, capacity `MAX_SLOTS = 1024`. Slot i holds the
  document y of its line (`lineNumber * lineHeight`). Changed slots are written with
  `texSubImage2D`, and the bytes go to `stats.slotTableBytes`. A slot maps to one line
  number. On a line-count change, re-assign y for shifted lines only (slot table bytes), not
  their segments.
- **Text buffer.** One `ARRAY_BUFFER` of 64-instance blocks (2 KiB each). It holds at most
  `MAX_TEXT_BLOCKS = 1536` blocks (3 MiB), reserved in the `geometry` ledger kind; it grows
  by `bufferData` reallocation, at most once per frame.
  - A segment, (line, 256-cluster chunk, horizontal window), owns a list of blocks. Unused
    instances in a block are written as zero-size.
  - A segment rebuild writes its blocks with `bufferSubData` (`stats.geometryBytes += bytes`).
  - A free puts its blocks on a free list and zero-writes them.
  - When `highWater > 2 * usedBlocks`, compact with one `bufferData` rewrite of the live
    segments (`stats.compactions += 1`, at most once per frame).
- **Segment key.** `(lineText identity or hash, fontGeneration, dpr, styleHash of the line's
  syntax spans, atlas generation)`.
  - Only lines in `[visibleFirst - overscanLines, visibleLast + overscanLines]` get segments.
    `overscanLines` is one viewport, the same window that `mount.ts` uses for syntax.
  - Only chunks whose x-range intersects `[scrollLeft - width, scrollLeft + 2*width]` get
    segments.
  - Segments outside the window are freed when the window moves past them.
- **Gutter segments.** One per visible line, keyed by line-number string and slot. Digits
  are mask cells. A gutter segment is rewritten only when its line number changes.
- **Layers.** Each layer has its own small dynamic `ARRAY_BUFFER` and VAO, and one
  `drawArraysInstanced` call, skipped when empty:
  1. *Background* (view space): the scrim first, then the selection rects, then the playing
     and eval rects. It is rewritten when the selection, scroll, viewport or animated set
     changes. `animatedRects` finds the first visible line by binary search.
  2. *Text* (text space): every live block, `count = highWater * 64`. One draw.
  3. *Overlays* (view space): the diagnostic, composition and call-head underlines,
     binding-label boxes, caret and handles. It is rewritten on selection, scroll,
     annotation or caret-blink changes.
  4. *Overlay text* (view space): the binding-label glyphs.
- **Draw count.** At most 4 per frame (`stats.drawCalls` is the last frame's count). No
  `scissor` toggling: the `CLIP_GUTTER` flag replaces it.
- **Ledger.** The text buffer, layer buffers, slot table and staging canvas are all reserved
  before allocation. Every reservation is released on dispose, with `usedBytes` back to 0.

### 3. Renderer orchestration (`renderer.ts`)

- *Public API kept:* constructor, `status`, `textPending`, `atlasStats` (now cell stats),
  `setPhases`, `setDocument`, `setText`, `setViewport`, `setPalette`, `render(feedback)`,
  `dispose`, and the context-loss and restore handlers.
- *`RenderFeedback`.* Its shape is unchanged; no `syntax` field is added. Syntax rows stay
  inside `feedback.annotations` (`kind === 'syntax'`), as today. The renderer derives the
  per-line style hashes from those rows. `editor/test/canvas/mount.test.ts:264-291` reads
  the syntax rows from `feedback.annotations` and must pass unchanged.
- *`render()` passes:*
  1. If (text revision, the syntax rows of `annotations` (compared per line by style hash),
     visible window, font generation, dpr, atlas generation) changed, rebuild the segments
     of changed lines only. A line is changed
     when its text identity, its style hash or its slot changed (`stats.textBuilds += 1`
     per segment build).
  2. Update the slot table for shifted lines.
  3. Rewrite the background and overlay layers when their inputs changed.
  4. Issue at most 4 draws.

  `cursor`, `handles`, `annotationsRevision` and `cursorVisible` affect only the overlay
  layers. A caret move gives `textBuilds` 0.
- *GL calls.* No `getError` or `getParameter` per frame: they run in `initialize`, restore
  and allocations only. No `isTexture`. Session-290 wording (design 15.3.8.14 section 4,
  "GL hygiene"): 0 `getParameter` over 100 `setViewport`/animation frames and on edits,
  at least 1 fresh `MAX_TEXTURE_SIZE` read on restore, and the init reads (renderer
  `initialize` and the atlas constructor) allowed. The BACKDROP `getParameter` row stays
  green. If the new atlas takes its limit from the renderer instead of its own read, that is
  allowed, provided restore still makes a fresh read.
- *Restore.* Rebuild the atlas, buffers and slot table from CPU state (layout cache and
  spans) and mark every segment dirty. Context loss releases reservations as today.
- *DPR change.* Reset the atlas and every segment; the existing `resize()` path calls
  `layout.invalidate()`.
- *File size.* Keep `renderer.ts`, `geometry.ts` and `atlas.ts` each under 1000 lines.

### 4. Layout accessor (`layout.ts`, additive)

`TextLayout.clusters(line: ShapedLine, from: number, to: number, visit: (text: string, x: number, width: number, color: boolean) => void): void`
visits the graphemes of an additive line in `[from, to)` with their line-local x and
width, from the OPT-TEXT advance table (no `measureText`). For non-additive or complex
lines, `TextLayout.isRunLine(line): boolean` is true and the renderer uses run cells.

### 5. `mount.ts`

Keep the syntax rows (`result.spans`, reused when `spans()` was skipped since OPT-TEXT)
inside `annotations`, as today, so `mount.test.ts:264-291` passes unchanged. Drop the
`staticRevision++` per text frame (`mount.ts:195`).
`annotationsRevision` changes only when surface annotations, selection or presentation
annotations change.

### Patterns to imitate

- `renderer.ts` `initialize`/`releaseGpu` for allocation order and rollback on failure.
- `resources.ts` `ResourceLedger.allocate`.
- `gpu.test.ts` `recordingGL()` and `fixture()`.

## Tasks

### TASK-R1: Cell atlas
### TASK-R2: `geometry.ts` (instance format, shaders, blocks, slot table, layers)
### TASK-R3: Renderer orchestration, layout accessor and mount feedback
### TASK-R4: Test-fake extension and ported `gpu.test.ts` rows
### TASK-R5: New counter rows, behavior e2e, verification and progress log

Each task's completion criterion is its contract plus its test rows, passing.

## Test Fakes and Ported Rows

**Test fakes.** `gpu.test.ts` `recordingGL()` and `test/support/gl.ts` gain these members:

- `bufferData`/`bufferSubData` that keep the bytes per buffer;
- `vertexAttribPointer`, `vertexAttribIPointer` and `vertexAttribDivisor` recorded per VAO;
- `drawArraysInstanced`, which decodes the bound VAO's instance buffer into the existing
  `draws` record shape: `{ rect, color, texture: atlas, textureLiveAtDraw, scissor: (flags & CLIP_GUTTER) !== 0 }`,
  one record per non-zero-size instance, in buffer order.

This keeps the order and clip assertions of the existing rows meaningful.
`texSubImage2D` records (x, y, w, h). Count calls to `getParameter` and `getError`.

**Ported rows** (`gpu.test.ts`). Keep each row's intent. The `ffi` stub font is
non-additive, so these rows exercise run cells:

| Existing row | Port |
|---|---|
| "reuses one ledger-accounted raster canvas until disposal" (`:190`) | one persistent staging canvas created once across frames; released on dispose |
| "crops the whole run and masks syntax without reshaping substrings" (`:205`) | run cells: `fillText` always gets the full run text; pieces clipped by `rect` crops |
| "rasterizes a styled multi-tile run once and releases its bounded raster" (`:213`) | a long run splits into consecutive run cells; each piece is rasterized once; the staging canvas is not reallocated |
| "caches by text/font/fallback/DPR/style and deletes evicted tiles" (`:225`) | cell keys include font, generation, DPR and piece; an atlas reset frees everything; the ledger returns |
| "reuses syntax tiles when an unchanged run shifts in document offsets" (`:235`) | a shifted unchanged line rebuilds 0 cells and only the slot table |
| "rolls back reservations and textures on upload failure" (`:245`) | allocation failure (texture or buffer) rolls back reservations; `getError` checked only at allocation |
| "bounds atlas misses per frame while allowing resident hits" (`:250`) | 1 MiB per-frame cell budget; `textPending` until done |
| "draws all required feedback in order on GPU and clips source against gutter" (`:275`) | the decoded `draws` order (scrim, selection, playing, text, underline, label box, caret, handles, label text) and gutter-clip flags |
| "idle frames reuse ..." (`:292`), "animation-only frames replay cached text ..." (`:299`), "does no liveness, error or segmentation probes on animation-only frames" (`:309`) | the same assertions on the new stats (`textBuilds`, `uploads`, `getError` 0, `isTexture` 0, segmentations 0) |
| "does not draw commands that reference textures deleted by context loss" (`:324`) | after loss, no draw until restore; after restore, a fresh atlas |
| "uses annotation revisions to reuse and invalidate static draw commands" (`:333`) | an `annotationsRevision` change rebuilds overlays and 0 text segments |
| "replays gutter numbers unscissored and source text clipped" (`:342`) | gutter instances have `NO_HSCROLL` and no `CLIP_GUTTER`; source text has `CLIP_GUTTER` |
| "keeps textPending until a bounded per-frame atlas upload completes" (`:370`) | unchanged intent |
| "dense offscreen syntax on a 1MiB line ..." (`:490`), "atlas does not retain historical full source strings ..." (`:497`), "keeps rendering all visible lines when atlas working set exceeds available budget" (`:503`), "releases initialization resources ..." (`:529`) | same assertions in cell and segment terms (a reset instead of an eviction) |

The resource-admission and layout describes (`:76-187`) are unchanged.

## New Test Cases

**`editor/test/canvas/gpu.test.ts`**, with an additive stub (`width = length * 8`):

- A 60-line viewport -> exactly 1 text draw call and at most 4 draws in total. Control: the
  old per-quad count would be over 60; assert `drawCalls <= 4` and that the decoded `draws`
  count is greater than 60.
- An edit of one line -> `geometryBytes` delta at most that line's segment block bytes
  (`ceil(clusters / 64) * 2048`), `textBuilds` delta 1, uploads at most the new cells.
- Enter on line 30 -> the segments of lines 31 and later are not rebuilt (`textBuilds`
  delta at most 2: the split line and the new line). `slotTableBytes` grows. Gutter
  segments are rebuilt only for renumbered lines.
- A caret move or selection change -> `textBuilds` delta 0; overlay rewritten.
- 100 animation-only frames with playing ranges -> 0 uploads, 0 `textBuilds`, 0
  `createCanvas`, 0 `getError`, 0 `getParameter`, 0 `measureText` on any 2D context.
- An emoji cluster -> a color cell with the `UNTINTED` flag. A Hebrew line -> a run cell
  (`rtlUnsupported` reporting unchanged).
- An atlas filled to capacity -> growth to 2048x1024 (`growths === 1`, generation bumped);
  at maximum -> `resets === 1`; text still renders after the frames needed to fill the
  budget.

**`editor/test/canvas/edit-cost.test.ts`** or **`mount.test.ts`**, mounted 20,000-line
document with the fake GL. A single ASCII edit, keystroke plus next frame:

- `getError` 0, `getParameter` 0;
- atlas uploads at most the new cells;
- geometry bytes at most the changed line segment;
- draw calls at most 4;
- `measureText` 0 on every 2D context (layout and atlas).

A caret move gives `textBuilds` 0. An animation-only frame gives 0 atlas uploads, 0 layout
builds, 0 staging canvases and at most 4 draws.

## Pitfalls

- *Premultiplied alpha.* Mask cells are white with alpha, uploaded with
  `UNPACK_PREMULTIPLY_ALPHA_WEBGL` true. Tint in the shader. Do not upload colored masks per
  token; colors are instance data now.
- *Integer attributes.* `a_meta` uses `vertexAttribIPointer`, not `vertexAttribPointer`.
- *Slot table bounds.* Never index past `MAX_SLOTS`. When the window has more lines than
  slots, render in bounded batches across frames with `text-pending`, never silently omit
  (15.3.5).
- *Atlas generation.* Every segment's key includes the atlas generation. Otherwise a reset
  leaves stale UVs.
- *Glyph padding.* Do not crop glyph overhang: keep `pad` in both the raster and the quad.
- *Run cells.* Never split a run cell at a point that breaks shaping. Pieces are clips of
  the full run.
- *Staging lifetime.* Do not free the staging canvas in `endFrame`; that would repeat the
  per-frame allocation bug.
- *Scope.* Do not reintroduce scissor toggling or any per-frame `getError`.
- *Behavior checks.* `behavior.mjs` readback must see glyph pixels on line 1. Verify
  canvas-only text and readback in both browsers with the behavior e2e.

## Verification

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`;
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
Record `BASE` and the fresh-read sha256 values in `tmp/canvas-cutover/opt-render/intent.json`.

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas` | exit 0; the ported and new rows are listed |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `wc -l editor/src/code/renderer.ts editor/src/code/geometry.ts editor/src/code/atlas.ts editor/src/code/layout.ts` | each under 1000 (notes only) |
| `grep -n "JSON.stringify\|getError" editor/src/code/atlas.ts` | no `JSON.stringify`; no `getError` outside allocation (notes only) |

Outside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `mise run build-wasm-release` then `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 |
| `cd editor && npm run e2e -- --browser all --profile behavior --run-id s286-opt-render --out ../tmp/canvas-cutover/opt-render/behavior` | exit 0; canvas-only text and line-1 readback, IME, context loss and restore, DPR and resize checks pass in Chromium and WebKit |
| `cd editor && npm run test:style` | exit 0 |
| `cd editor && npm run test:perf` (alone) | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |

Use the session-286 record format: `exitStatus: 0`, `outcome: "passed"`, `testsRun > 0` and
`failureCount: 0` for tests, details in `notes`. Run no mutation or negative-control
command.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/opt-render/intent.json`
and `receipt.json`. If a file drifted without an edit from this plan, stop editing it and
report. Edit only this plan's progress log.

## Completion Criteria

- [ ] One-texture append-only cell atlas (mask, color and run cells), with growth and reset by generation; persistent staging; dirty-rect uploads at most the new cells; no `JSON.stringify`, no per-upload `getError`
- [ ] Instanced 32-byte instances in 64-instance blocks; slot table; changed-line `bufferSubData` only; compaction bounded
- [ ] At most 4 draw calls per frame; gutter clip by flag; caret, selection and handles in overlay layers (`textBuilds` 0 on a caret move)
- [ ] Single-char edit counters: `getError`/`getParameter` 0, uploads at most new cells, geometry bytes at most the changed segment, `measureText` 0 for ASCII
- [ ] Animation-only frame: 0 uploads, 0 layout builds, 0 staging canvases, at most 4 draws
- [ ] Ported `gpu.test.ts` rows keep their intent; the resource and layout describes are unchanged
- [ ] Behavior e2e (both browsers), `test:style`, default vitest, `npm run check`, `test:perf`, clippy, nextest, wasm32 build and src-tauri check pass
- [ ] Harness sharedPaths: unedited, or frame-alignment-only edits recorded with sha256 values and no changed assertion, threshold or check comparison (`git diff <START> -- editor/test/e2e editor/test/style`)
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-06 (session 286 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 4.
**Notes**: Wave 13, after BACKDROP. The non-additive test stub keeps the existing run-tile
rows meaningful as run-cell rows. New rows use an additive stub for the cluster path.

### Session: 2026-10-06 (session 288 plan amendment)
**Tasks Completed**: Ownership amendment only (design 15.3.8.14 section 8, "Harness
sharedPaths").
**Notes**:
- Added the 8 harness and style files as concrete sharedPaths, here and in the manifest.
  `behavior.mjs` was already a sharedPath.
- Removed-stat probes are handled by keeping the stat, not by editing the probe.
- Scope, contracts, tasks and criteria are otherwise unchanged.

### Session: 2026-10-06 (session 290 plan amendment)
**Tasks Completed**: Ownership decision 2 and INT-S289-BD-PLAN-GETPARAM (design 15.3.8.14
sections 4 and 8, session-290 amendment).
**Notes**:
- Added 12 concrete sharedPaths, here and in the manifest (the 15-file list minus
  `atlas.ts`, `gpu.test.ts` and `mount.test.ts`, which are already writePaths).
- Aligned the "GL calls" wording with the amended getParameter rule.
- Scope, contracts, tasks and criteria are otherwise unchanged. This plan starts only after
  CANVAS-OPT-BACKDROP is accepted.

### Session: 2026-10-06 (session 293 implementation attempt)
**Tasks Completed**: Partial R1 atlas and UV sampling migration; initial 32-byte geometry
packing helper.
**Notes**:
- Preserved the accepted uncommitted BACKDROP palette and DOM-layer changes.
- `atlas.ts` now has a single texture, append-only cell allocation, persistent staging
  canvas ownership, per-frame byte budget counters and generation/reset bookkeeping. The
  renderer samples atlas cell UVs. `geometry.ts` currently only defines the instance format
  and encoder; renderer instancing, slot tables, segment caching and draw batching are not
  implemented.
- Fresh TypeScript check passed: `tmp/canvas-cutover/opt-render/check-current.log`.
- Focused GPU tests are not green: `focused-atlas-2.log` reports 16 failed / 33 passed.
  Failures expose unported old atlas semantics (canvas-size/resource expectations, upload
  rollback and cumulative counters) and renderer assumptions that still require migration.
- No completion criteria are checked. The implementation and required behavior/e2e/gate
  verification remain incomplete.
