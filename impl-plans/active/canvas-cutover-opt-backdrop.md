# Canvas Cutover OPT-BACKDROP: Zero-Copy DOM Backdrop, Cached GL Limits and Token Palette (F1) Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-OPT-BACKDROP (session 286, wave 12; runs alone after CANVAS-OPT-TEXT is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 4 ("Zero-copy backdrop", "GL hygiene", "Palette"); 15.3.5 and 15.3.8.6 "Backdrop" (as amended); 15.3.8.3 (animation-active, as amended); design-docs/specs/design-ui-style.md section 7 (F1 mapping table)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-BACKDROP`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

Every animation frame the code renderer copies the visual canvas:

- `CanvasRenderer.uploadBackground` (`editor/src/code/renderer.ts:343-378`) creates a
  canvas, `drawImage`s the WebGL visual canvas into it, uploads it with `texSubImage2D`, and
  calls `getError`.
- The trigger is `visual/mount.ts:104`, which calls every background listener after each
  visual frame. `code/mount.ts:209-211` bumps `backgroundRevision` and requests a code
  frame each time.

In WebKit this is a GPU-process readback, a second copy and a synchronous round trip, every
frame. Also, `resize()` calls `gl.getParameter(MAX_TEXTURE_SIZE)` on every `setViewport`
(`renderer.ts:156,163`), which happens on every frame.

The design replaces the copy with DOM stacking. The visual canvas element is placed under
the transparent code canvas, so the browser compositor shows it with no copy. Follow-up F1
maps the renderer's hard-coded palette to the `--vt-*` tokens.

This plan is a refinement of the design's single OPT-RENDER plan into two serial plans:
BACKDROP (this plan; small and independently verifiable), then RENDER (atlas and
geometry).

## Non-goals

- No atlas, geometry, draw-batching or layout change (CANVAS-OPT-RENDER).
- No change to the visual pane display: `VisualPanes.present` (`visual/panes.ts:107-116`)
  still copies into its 2D pane canvases. No change to `GlRenderHost`, Hydra, the video
  composition or the visual frame loop timing.
- No change to `VisualApi.onBackgroundCanvas`'s signature.
- No new token unless a needed value has none. The scrim color stays
  `[0.04, 0.05, 0.07, 0.85]`. The playing, eval and selection colors stay as they are; they
  are not in the section-7 table.
- No Rust, no dependency, no threshold change.

## Ownership

writePaths:

- `editor/src/code/renderer.ts`
- `editor/src/code/palette.ts` (new)
- `editor/src/code/mount.ts`
- `editor/src/code/code.css`
- `editor/src/code/code-view.tsx`
- `editor/src/visual/mount.ts`
- `editor/src/app/apis.ts` (the `VisualApi.onBackgroundCanvas` doc comment only)
- tests:
  - `editor/test/canvas/gpu.test.ts` (backdrop rows and new palette and parameter rows)
  - `editor/test/canvas/mount.test.ts`
  - `editor/test/visual/panes.test.ts`
- `impl-plans/active/canvas-cutover-opt-backdrop.md` (checkboxes and progress log only)
- `tmp/canvas-cutover/opt-backdrop` (artifact root; also receives the behavior e2e output)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules/.vite`, `editor/dist`, `tmp/ui-style`

sharedPaths:

- `editor/src/code/resources.ts`: no edit expected. The `backdrop` ledger kind stays
  defined, even though the renderer no longer reserves it. Record any edit.
- `editor/test/support/gl.ts`: additive fake members only, if needed.

## Contracts and Key Points

### 1. Visual mount (`visual/mount.ts`)

- `onBackgroundCanvas(cb)` keeps its signature. It calls `cb(glCanvas)` once when the canvas
  exists, immediately if it already does (line 63), and `cb(null)` on dispose (line 125).
- Remove the per-frame `for (const cb of [...backgroundListeners]) cb(glCanvas);` at
  line 104. `onFrame` keeps `analyzers.present(t)` and `panes.present(host)`.
- Update the `apis.ts` doc comment: "called once with the render canvas when it exists and
  with null on disposal; the caller may place the element in its DOM; the visual loop keeps
  drawing into it".

### 2. Code mount (`code/mount.ts`, `code.css`, `code-view.tsx`)

- *Inserting the element.* When the lazy subscription (lines 209-211) receives a canvas,
  insert it as the first child of `hostEl`, before `canvas`. Add the class
  `vact-code-backdrop` and `aria-hidden="true"`. On `null`, or on a different canvas,
  remove the previous element from `hostEl`. Never call
  `renderer.setBackground`, and do not bump `backgroundRevision` or call
  `scheduler.request()` per frame.
- *Disposal.* `dispose()` removes the backdrop element if it is still in `hostEl`. The
  element belongs to the visual mount, so remove it but do not zero its size.
- *CSS.*
  - `.vact-code-backdrop` is
    `position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; z-index: 0;`.
  - `.vact-code-canvas` gains `position: relative; z-index: 1;` so it paints above the
    absolutely positioned backdrop.
  - The host must be a positioning context: check the class rendered by `code-view.tsx`
    (`vact-code`) and the `.vact-code-surface` rule at `code.css:47`, and make the actual
    host `position: relative` if it is not.
  - No color literal (`style-tokens.test.ts`).

### 3. Renderer (`renderer.ts`)

- Delete `setBackground`, `uploadBackground`, the `backdrop` and `background` fields, the
  backdrop quad (line 186) and `stats.backgroundUploads`. Keep the scrim quad (line 187) as
  the first quad.
  - `releaseGpu` and `dispose` lose the backdrop branches.
  - Keep `stats` shape-compatible: either remove `backgroundUploads` and update every
    reader (grep `backgroundUploads` in `editor/src` and `editor/test`), or keep it fixed at
    0. Prefer removal and record it.
- `getParameter(MAX_TEXTURE_SIZE)` is read once in `initialize()` and stored in
  `private maxTextureSize`. `resize()` uses the stored value. `restore` re-runs `initialize`,
  so the value is re-read there.
- `getError` stays only in `initialize` (line 149), `resize` allocation (line 168) and the
  atlas upload path. The atlas path is replaced by CANVAS-OPT-RENDER; leave `atlas.ts`
  untouched here.

### 4. Palette (`palette.ts`, renderer)

- *`palette.ts` exports:*
  - `interface Palette { token: Record<string, [number, number, number, number]>; text; gutter; diagnostic; composition; callHead; cursor; labelFill; labelText; handle }`,
    every value RGBA in 0..1;
  - `readPalette(root: Element | null): Palette`, which reads
    `getComputedStyle(root).getPropertyValue('--vt-…')` once per call and parses hex,
    `rgb()` and `rgba()`;
  - `FALLBACK_PALETTE`, built from today's constants.
- *Mapping* (design-ui-style.md section 7):
  - the `vact-tok-*` classes map to `--vt-syn-comment`, `-directive`, `-keyword`,
    `-number`, `-string` (string and path), `-head` and `-bracket`;
  - default glyph -> `--vt-text`;
  - gutter -> `--vt-syn-comment`;
  - diagnostic underline -> `--vt-danger` (this is the one visible change);
  - composition underline and handles -> `--vt-data-1`;
  - call-head -> `--vt-text-muted` at alpha 0.6;
  - cursor -> `--vt-text`;
  - label fill -> `--vt-raised`;
  - label text -> `--vt-syn-head`.

  A missing or unparsable token falls back to the `FALLBACK_PALETTE` entry, which is what
  jsdom gives.
- *Renderer use.* `RendererOptions.palette?: Palette`. `CanvasRenderer` uses `this.palette`
  where it uses literals today: `GPU_TOKEN_COLORS` lookups (lines 310 and 321), the gutter
  `'#7a7f87'` (line 221), the underline colors (line 224), call-head (225), cursor (227),
  the label fill and text (233-234) and the handles (240). `GPU_TOKEN_COLORS` stays exported
  with its current values, as the fallback source.
- *Mount.* `code/mount.ts` calls `readPalette(doc.documentElement)` once at mount and passes
  it in. It re-reads on a `matchMedia('(prefers-color-scheme: dark)')` change event, calls
  `renderer.setPalette(p)` (which invalidates the cached draw lists) and invalidates text
  for reason `gpu`. Nothing reads styles per frame.

### Patterns to imitate

- `visual/mount.ts` `onBackgroundCanvas` and `backgroundListeners`.
- `code/mount.ts` lazy subscription at lines 209-211.
- `gpu.test.ts` `fixture()` and `recordingGL()` for counting `getParameter`, `getError`,
  `texSubImage2D` and `drawImage`.

## Tasks

### TASK-B1: Visual mount, called once and with null
### TASK-B2: Code mount DOM layering and CSS
### TASK-B3: Renderer backdrop removal and cached `MAX_TEXTURE_SIZE`
### TASK-B4: Palette (F1)
### TASK-B5: Ported tests, behavior e2e, style test, verification and progress log

Each task's completion criterion is its contract plus its test rows, passing.

## Test Cases

**`editor/test/visual/panes.test.ts`** (`:188-224`):

- `expect(background).toEqual([canvas, canvas])` after `raf.step()` (line 210) becomes
  `[canvas]`: no per-frame call.
- Line 223 becomes `[canvas, null]`.
- The `stopped` assertions are unchanged.
- Record the port: the design 15.3.8.6 contract changed.

**`editor/test/canvas/gpu.test.ts`**, backdrop rows:

- "copies changed background once per frame into a reusable texture" (`:381`),
  "large backgrounds downsample..." (`:418`), "reuses a near-staging-cap backdrop..."
  (`:427`), "retains every unchanged text tile with animated near-cap backgrounds"
  (`:437`), "shrinks animated backdrop staging..." (`:477`). Each is rewritten to the
  layering contract, keeping its intent:
  - 100 animation frames -> 0 `texImage2D`/`texSubImage2D` calls whose source is a canvas,
    0 `createCanvas` calls and 0 `backdrop` ledger bytes;
  - text tiles stay resident across those frames (the existing "retains every unchanged
    text tile" assertion, without the background);
  - `dispose()` -> `usedBytes === 0`.

  Every assertion that does not mention the background is kept verbatim.
- New: `setViewport` called on 100 frames -> `getParameter` called exactly once since
  init. Control: `restore` (a `webglcontextrestored` event) -> one more call.
- New: a palette with `diagnostic: [1, 0, 0, 1]` -> the diagnostic underline draw uses that
  color. `FALLBACK_PALETTE` -> today's colors (the token map equals `GPU_TOKEN_COLORS`
  parsed).

**`editor/test/canvas/mount.test.ts`:**

- With a fake `deps.visual.onBackgroundCanvas` that calls back once with a canvas -> the
  canvas is the first child of the code host with class `vact-code-backdrop`, and the code
  canvas follows it.
- `cb(null)` -> removed.
- 60 frames with playing highlights -> 0 `drawImage` calls on any 2D context and 0
  background texture uploads.
- `dispose` -> removed.

## Pitfalls

- *Paint order.* An absolutely positioned backdrop paints above a non-positioned canvas.
  The code canvas must be `position: relative` with a higher `z-index`.
- *Visual mount order.* `deps.visual` mounts after `code`. Keep the lazy subscription in
  `onFrame`, or an equivalent retry, so the canvas is picked up when it appears.
- *Clear color.* Do not make the code canvas opaque. `clearColor(0, 0, 0, 0)` and the
  premultiplied scrim must stay, so the backdrop shows through.
- *Element ownership.* Do not move the element anywhere else, and do not resize it.
  `GlRenderHost` owns its 640x360 drawing buffer. CSS only stretches it.
- *`backgroundUploads` readers.* `perf-hook.ts` and the e2e `measure.mjs` may read
  `renderer.stats`. Grep, and if one reads `backgroundUploads`, either keep the field at 0 or
  update the reader; prefer keeping the perf record shape. Record the decision.

## Verification

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`;
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
Record `BASE` and the fresh-read sha256 values in `tmp/canvas-cutover/opt-backdrop/intent.json`.

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts test/canvas/mount.test.ts test/visual` | exit 0; ported and new rows listed |
| `cd editor && ./node_modules/.bin/vitest run test/ui` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `grep -n "uploadBackground\|setBackground" editor/src` | no match (notes only) |

Outside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `mise run build-wasm-release` then `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 |
| `cd editor && npm run e2e -- --browser all --profile behavior --run-id s286-opt-backdrop --out ../tmp/canvas-cutover/opt-backdrop/behavior` | exit 0; canvas-only text, readback, context loss, DPR and backgrounding checks pass in both browsers |
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

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/opt-backdrop/intent.json`
and `receipt.json`. If a file drifted without an edit from this plan, stop editing it and
report. Edit only this plan's progress log.

## Completion Criteria

- [ ] The visual mount calls back once with the canvas and once with null; no per-frame calls
- [ ] The code mount stacks the visual canvas under the code canvas; no copy, no upload and no per-frame wake-up
- [ ] `uploadBackground` and `setBackground` removed; 0 backdrop ledger bytes; `getParameter` read once per init or restore
- [ ] `palette.ts` maps the section-7 tokens; the diagnostic underline uses `--vt-danger`; the fallback equals today's colors
- [ ] Ported backdrop rows keep every non-background assertion; `panes.test.ts` updated to the once/null contract
- [ ] Behavior e2e, `test:style`, default vitest, `npm run check`, `test:perf`, clippy, nextest, wasm32 build and src-tauri check pass
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-06 (session 286 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 4 (backdrop, GL limits,
palette).
**Notes**: Wave 12, after TEXT. Split from OPT-RENDER so the zero-copy change is verified
alone.
