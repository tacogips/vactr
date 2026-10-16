# Code Render 02: Width- and Phase-Keyed Atlas Cells, NEAREST Sampling (RC1, R6 raster, R9)

**Status**: Ready
**Plan ID**: CR-02
**Wave**: 1 (no dependencies)
**Design Reference**: design-docs/specs/design-code-pane-text-rendering.md sections 3 (RC1, RC5), 4.2 (R3, R4), 4.4 (R6 raster part, R9), 5.1 (U2, U5 texture part)
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

`GlyphAtlas.cell` (`editor/src/code/atlas.ts:92`) caches cells by a key
that leaves out the rasterized cell width. The gutter label run for line 1
is `{ text: '1', width: gutter - 8 }`, so the first `1` cell is 40 CSS px
wide. The document `1` (about 7.8 px) gets the same key and reuses it,
squeezed into a narrow quad (`tmp/code-render/t-typed-code.png`). This
plan also adds the subpixel phase input that CR-05 uses, and switches
the atlas texture to `NEAREST`.

Contract pinned for CR-05: `CellRequest` gains an optional field
`phase?: number`, whose value is one of `0`, `0.25`, `0.5`, `0.75`
(device pixels; default 0). The glyph is rasterized shifted right by
`phase` device pixels inside the existing pad.

## Non-goals

- No change to shelf packing, growth, reset, budgets, ledger accounting
  or `stats` semantics.
- No `UNPACK_PREMULTIPLY_ALPHA_WEBGL` change. Uploads stay straight alpha
  (design R10 premultiplies in the shader, CR-04).
- Do not touch `renderer.ts`, `geometry.ts`, `layout.ts`, or
  `editor/test/canvas/gpu.test.ts` (CR-05 reconciles it).

## Files

| Path | Change |
| --- | --- |
| `editor/src/code/atlas.ts` | (a) `CellRequest.phase?: number`. (b) In `cell()`: validate `phase` (undefined -> 0; anything other than 0, 0.25, 0.5, 0.75 -> `RangeError('Invalid atlas cell')`, matching the existing invalid-input throw). (c) Append the texel width (the already computed `width`) and the phase to the cache key, e.g. `...|${request.chunkX ?? 0}|w${width}|p${phase}`. (d) Raster offset: mask/color path `fillText(text, (pad + phase) / dpr - pieceOffset, font.baseline)`; run path: shift both the clip rect origin and the `fillText` x by `phase / dpr` (`ctx.rect((pad + phase) / dpr, 0, request.width, font.lineHeight)` and `fillText(text, (pad + phase) / dpr - (request.chunkX ?? 0), ...)`). (e) `configure()`: `TEXTURE_MIN_FILTER` and `TEXTURE_MAG_FILTER` = `NEAREST`. |
| `editor/test/code/recording-gl.ts` | New shared test helper (not a test file): export `width`, `rasterizer` and `recordingGL`, copied verbatim from `editor/test/canvas/gpu.test.ts` lines 10-14 (`width`), 37-54 (`rasterizer`) and 59-120 (`recordingGL`), with `export` added and the needed imports (`vitest` `expect`, `CanvasFactory` type). Do not modify gpu.test.ts; CR-05 imports this helper. |
| `editor/test/code/atlas-cells.test.ts` | New unit test file (U2 and the atlas part of U5). |
| `impl-plans/active/code-render-02-atlas-cells.md` | Progress log only. |

Imitate the existing `cell()` validation style (`atlas.ts:87`) and the
gpu.test.ts atlas cases that construct `new GlyphAtlas(gl, new
ResourceLedger(), raster.createCanvas)`.

## Key Points and Pitfalls

- Key on the TEXEL width (`Math.ceil(request.width * dpr) + pad * 2`),
  not the raw float `request.width`. Two requests that rasterize identical
  pixels must still share a cell (a float-noise width such as 7.8000001
  versus 7.8 with the same ceil must hit).
- `phase` must be in the key, and must also change the raster x. A key
  change without the raster change produces wrong pixels.
- The cell width formula stays `ceil(width * dpr) + 2 * pad`. The phase
  (at most 0.75 px) fits inside the right pad (`pad >= 2`). Do not widen
  cells.
- `AtlasCell.width`/`height` and `u0..u1`/`v0..v1` must keep describing
  exactly the uploaded texel rectangle (R4); CR-05 sizes quads from them.
- The white texels at (0,0)-(2,2) used for solid quads are sampled at
  their texel centers with `NEAREST`; keep `writeWhiteTexels` unchanged.

## Test Cases (`editor/test/code/atlas-cells.test.ts`)

Use `recordingGL()` and `rasterizer()` from `./recording-gl`.

- `cell({kind:'mask', text:'1', width:40, dpr:1, font})` then the same
  with `width: 8` -> distinct cells; `cell.width` equals `ceil(w * 1) + 2
  * pad` for each (pad = `ceil(2 * dpr)`); the second call is not a hit
  (`stats.uploads` grows by 2).
- Same at `dpr: 2` -> widths `ceil(w * 2) + 2 * pad`.
- `width: 7.8` twice -> the second call is a hit (`stats.hits` + 1).
- `width: 8` with `phase: 0` and `phase: 0.5` -> distinct cells; the
  recorded `fillText` x differs by `0.5 / dpr`.
- `phase: 0.3` -> throws `RangeError`.
- Run cell (`kind: 'run'`, piece and chunkX as in
  `gpu.test.ts:atlasCell`) with `phase: 0.25` -> the recorded clip rect x
  (from `rasterizer().crops`) and the `fillText` x are both shifted by
  `0.25 / dpr` compared with `phase: 0`.
- After construction, the recorded `texParameteri` calls for the atlas
  texture set `TEXTURE_MIN_FILTER` and `TEXTURE_MAG_FILTER` to `NEAREST`
  (fake GL constants from `recordingGL`).

The width and filter cases fail on ea51a07. CR-07 records that as
mutationEvidence.

## Invariants

- `atlas.ts` stays under 260 lines.
- Behavior for existing callers that pass no `phase` is identical to
  `phase: 0`.

## Write Scope

Mirrors `impl-plans/active/code-render-dispatch.json` plans[CR-02].

- writePaths: `editor/src/code/atlas.ts`, `editor/test/code/recording-gl.ts`, `editor/test/code/atlas-cells.test.ts`, `impl-plans/active/code-render-02-atlas-cells.md`, `tmp/code-render/CR-02`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`
- sharedPaths: none
- artifactRoots: `tmp/code-render/CR-02`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules` (the last three are prebuilt read-only inputs: never rebuild, reinstall or delete them)
- Toolchain audit: before the first edit, from the repository root run
  `shasum -a 256 target/wasm32-unknown-unknown/debug/vactr.wasm tree-sitter-vact/tree-sitter-vact.wasm editor/package-lock.json > tmp/code-render/CR-02/toolchain-sha256.txt`
  and copy the three digests into the Progress Log.

## Working Rules

Same as CR-01: fresh read plus sha256 before and after every edit, drift
means re-read and re-plan, no git writes, foreground only, cross-plan
`npm run check` errors are recorded, not fixed. Logs go under
`tmp/code-render/CR-02/`.

## Verification

Run from `editor/`. Each log ends with its exit status.

1. `npx vitest run test/code/atlas-cells.test.ts > ../tmp/code-render/CR-02/focused.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-02/focused.log` -> exit 0.
2. `npm run check > ../tmp/code-render/CR-02/check.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-02/check.log` -> exit 0 (or only cross-plan errors recorded).
3. `npx vitest run test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts > ../tmp/code-render/CR-02/canvas.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-02/canvas.log` -> informational. Record failures caused by this change as CR-05 handoff notes.

## Completion Criteria

- [ ] `CellRequest.phase` added and validated; key includes texel width and phase
- [ ] Raster x (and the run clip) shifted by `phase / dpr`
- [ ] Atlas texture uses `NEAREST` for min and mag
- [ ] `editor/test/code/recording-gl.ts` exports `width`, `rasterizer`, `recordingGL`
- [ ] `editor/test/code/atlas-cells.test.ts` passes
- [ ] Progress log updated with verification records and handoff notes

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Ready for wave 1.
