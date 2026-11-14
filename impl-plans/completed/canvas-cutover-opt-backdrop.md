# Canvas Cutover OPT-BACKDROP: Zero-Copy DOM Backdrop, Cached GL Limits and Token Palette (F1) Implementation Plan

**Status**: Completed (2026-10-07)
**Plan ID**: CANVAS-OPT-BACKDROP (session 286, wave 12; runs alone after CANVAS-OPT-TEXT is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 4 ("Zero-copy backdrop", "GL hygiene", "Palette"); 15.3.5 and 15.3.8.6 "Backdrop" (as amended); 15.3.8.3 (animation-active, as amended); design-docs/specs/design-ui-style.md section 7 (F1 mapping table)
**Manifest**: impl-plans/completed/canvas-cutover-dispatch.json (entry `CANVAS-OPT-BACKDROP`)
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
- `impl-plans/completed/canvas-cutover-opt-backdrop.md` (checkboxes and progress log only)
- `tmp/canvas-cutover/opt-backdrop` (artifact root; also receives the behavior e2e output)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules/.vite`, `editor/dist`, `tmp/ui-style`

sharedPaths:

- `editor/src/code/resources.ts`: no edit expected. The `backdrop` ledger kind stays
  defined, even though the renderer no longer reserves it. Record any edit.
- `editor/test/support/gl.ts`: additive fake members only, if needed.
- Harness sharedPaths (session-288 ownership amendment; design 15.3.8.14 section 8). These
  are edited only for frame alignment:
  - `editor/test/e2e/behavior.mjs`
  - `editor/test/e2e/measure.mjs`
  - `editor/test/e2e/run.mjs`
  - `editor/test/e2e/stats.mjs`
  - `editor/test/e2e/stats.test.ts`
  - `editor/test/e2e/ios-sim.mjs`
  - `editor/test/e2e/README.md`
  - `editor/test/style/ui-style.mjs`
- Backdrop seam sharedPaths (session-290 ownership amendment; design 15.3.8.14 section 8,
  "Backdrop and render sharedPaths"). These are the 15-file list minus the three that are
  already writePaths here (`gpu.test.ts`, `mount.test.ts`, `panes.test.ts`). Edit them only
  to finish this plan's contract, and record each edit with sha256 values:
  - `editor/src/code/atlas.ts`: no edit expected. Its init-time `getParameter` (line 36) is
    allowed and recorded. If the palette CSS string changes a tile identity, fix that in
    `renderer.ts`, not here. The atlas rewrite belongs to CANVAS-OPT-RENDER.
  - `editor/src/visual/panes.ts`, `editor/src/visual/render-host.ts`,
    `editor/src/visual/frame.ts`: only a removed `onBackgroundCanvas` per-frame dependency
    or a doc comment. Pane presentation, `GlRenderHost` and frame timing do not change.
  - `editor/src/app/theme.css`: only if a section-7 token the palette reads is missing. Do
    not change any existing token value.
  - `editor/test/canvas/frame.test.ts`, `editor/test/visual/frame.test.ts`,
    `editor/test/visual/meters.test.ts`, `editor/test/visual/render-host.test.ts`,
    `editor/test/visual/scopes.test.ts`, `editor/test/visual/text-asset.test.ts`,
    `editor/test/visual/video.test.ts`: port a row only if it depends on the removed
    per-frame background callback or on `setBackground`/`backgroundUploads`. Keep its
    intent, keep every other assertion byte-identical, and record the port.

### Harness sharedPaths (session 288)

The rules are identical to the "Harness sharedPaths (session 288)" section of
`impl-plans/completed/canvas-cutover-opt-dom.md`. They are restated here so this plan stands
alone.

**Allowed edit.** Only frame alignment of harness sampling (wait for a presented frame,
then a bounded poll; reference 842cf6e), or documentation of that alignment.

**Never change:**
- a check name, assertion, expected value or comparison;
- `THRESHOLDS` or `TARGETS`;
- the workload, the fixtures or the silent sink;
- `run.mjs` exit codes or its gating refusal;
- the `ios-sim.mjs` predicate;
- the `ui-style.mjs` square and token checks.

A non-alignment harness change is a blocker for a serial plan-author amendment.

**Recording.** Record the path, reason and sha256 values in the progress log and in
`tmp/canvas-cutover/opt-backdrop/intent.json` and `receipt.json`.

**Check.** `git diff <START> -- <path>` shows no removed or changed line containing
`expect(`, `assert`, `THRESHOLDS`, `TARGETS` or a check comparison.

**Likely need.** The DOM-stacked backdrop changes compositing. If the line-1 readback
(`behavior.mjs`) or a `ui-style.mjs` screenshot samples before the first frame after the
backdrop is inserted, add a frame wait. If a check still fails after alignment, treat it as
a product defect in this plan's writePaths, never as a harness issue.

## Session 290 resume (brief INT-S289-BD-IMPL-INCOMPLETE)

This section governs the session-290 dispatch. Where it is more specific than the sections
below, it wins. The contracts below are otherwise unchanged.

**Bases.**
- `BASE=212bb54` is the operator WIP checkpoint that holds the partial implementation.
- `START` is HEAD at dispatch: the session-290 plan checkpoint commit, whose source tree
  equals 212bb54.
- Record both, with fresh-read sha256 values, in `tmp/canvas-cutover/opt-backdrop/intent.json`.
  Continue from the working tree. Do not revert, and do not reimplement what is already done.

**Already done at 212bb54.** Verify each item; do not redo it.
- B1: `visual/mount.ts` no longer calls background listeners per frame. The `apis.ts` doc
  comment is updated.
- B2: `code/mount.ts` `placeBackground` inserts the visual canvas before the code canvas
  with `vact-code-backdrop` and `aria-hidden`, removes it on null and on dispose, and
  `backgroundStop` subscribes lazily. `code.css` sets `.vact-code` to
  `position: relative`, `.vact-code-backdrop` to absolute with z-index 0, and
  `.vact-code-canvas` to relative with z-index 1.
- B3: `renderer.ts` has no `setBackground`, `uploadBackground`, backdrop fields, backdrop
  quad or `backgroundUploads`. `maxTextureSize` is read in `initialize()`, and `resize()`
  uses the cached value.
- B4: `palette.ts` exists (`Rgba`, `Palette`, `FALLBACK_PALETTE`, `readPalette`,
  `rgbaCss`). The renderer has `setPalette`. The mount reads the palette once and on a
  `prefers-color-scheme` change.
- Accepted OPT-TEXT hunk: the `onPresentation` callback in `code/mount.ts` (Text-identity
  change detection, `presentation.changesBase === previousDoc`). It must stay byte-identical
  to 212bb54. Check with `git diff 212bb54 -- editor/src/code/mount.ts`: no line inside that
  callback may change.

**Red baseline** (logs `tmp/canvas-cutover/opt-text/reconcile-s289b-combined-{check,vitest}.log`):
- `npm run check`: 20 errors. They are `renderer.ts(220,202)` and `(233,71)` (an `Rgba`
  passed where `drawRun` takes a string) and 18 errors in `gpu.test.ts` that use the
  removed `setBackground`/`backgroundUploads`.
- Vitest: 7 of 778 rows fail. Six are in `gpu.test.ts` (`:296`, `:402`, `:439`, `:448`,
  `:458`, `:498`) and one is in `visual/panes.test.ts` (`:210`).

**R1. Color format (fixes the TS errors).**
- Decision: `rgbaCss(c: Rgba): string` returns lowercase `#rrggbb` (each channel
  `Math.round(v * 255)`, two hex digits) when alpha is exactly 1, and
  `rgba(R, G, B, A)` otherwise.
- Wrap the two call sites with it: `drawRun(..., rgbaCss(this.palette.gutter), true)` at
  line 220, and `drawRun(..., rgbaCss(this.palette.labelText))` at line 233.
- Why: `FALLBACK_PALETTE` then produces exactly the legacy strings (`'#d8dee9'`,
  `'#7a7f87'`, `'#ebcb8b'` and the token hexes). Atlas tile identities and the
  `gpu.test.ts:310` assertion `t.color === '#ebcb8b'` stay byte-identical.
- Do not change the color parameter type of `drawRun`, `RunStyle.color` or
  `TileRequest.color` to `Rgba`, and do not edit `atlas.ts`: its color contract belongs to
  CANVAS-OPT-RENDER.

**R2. Port the six `gpu.test.ts` rows.** Keep the intent of each row, keep every assertion
that does not mention the background byte-identical, and record each port with its old and
new assertion in the progress log. Do not invent exact counts: run the row and pin the
observed value only where the old row pinned one.
- `:296` "draws all required feedback in order...":
  - Remove the source canvas and `setBackground`.
  - The leading backdrop quad `[1, 1, 1, 1]` disappears, so `colors.slice(0, 5)` becomes
    `colors.slice(0, 4)`, starting with the scrim `[0.04, 0.05, 0.07, 0.85]`.
  - `expect(diagnostic).toBeGreaterThan(4)` becomes `toBeGreaterThan(3)`, the same
    one-index shift.
  - Everything else is unchanged, including `'#ebcb8b'`, the scissor check and the
    dispose checks.
- `:402` "copies changed background once per frame..." becomes "animation frames never copy
  or upload a backdrop":
  - 1 text frame, then 100 animation-only frames
    (`render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })`).
  - Expect: 0 new `texImage2D`/`texSubImage2D` calls, 0 new `raster.createCanvas`
    calls, 0 `drawImage` calls on any fake 2D context, a stable live-texture count,
    `b.counters.byKind.backdrop === 0`, and `dispose` -> `usedBytes === 0`.
- `:439` "large backgrounds downsample..." becomes "a large viewport stays within the
  staging and pixel caps with no backdrop reservation":
  - Keep the geometry and pixel cap assertions and the `copied` undefined assertion.
  - Replace `backgroundUploads === 1` with `byKind.backdrop === 0`.
- `:448` "reuses a near-staging-cap backdrop...":
  - Two renders of the 40-line document.
  - Keep the geometry cap, dispose `usedBytes === 0` and `live.size === 0`.
  - Replace the `backgroundUploads` check with `byKind.backdrop === 0`.
- `:458` "retains every unchanged text tile with animated near-cap backgrounds":
  - Keep the 4-iteration loop as 4 animation frames (an animated playing range, with no
    `setBackground`).
  - Keep `firstUploads === 80`, the 40 rasterized `line ` texts, identical textures,
    stable `usedBytes`, 0 evictions and the ledger-cap checks in the `allocate` wrapper.
  - Replace `backgroundUploads === revision` with `byKind.backdrop === 0`.
  - The `texSubImage2D` count becomes 0, and the `texImage2D` count drops by the one
    backdrop upload: white plus 80 glyph tiles. Confirm by running.
  - `peakGeometry > 4 MiB - 20,000` measured backdrop staging, so it is removed. Record
    that as the only dropped assertion, because its subject no longer exists.
- `:498` "shrinks animated backdrop staging under new geometry pressure..." becomes
  "geometry pressure during animation frames never evicts text":
  - Keep the pressure allocation, the unchanged atlas uploads, 0 evictions and the
    dispose checks.
  - Replace the backdrop byte comparisons with `byKind.backdrop === 0`.

**R3. `visual/panes.test.ts`.** Line 210 `[canvas, canvas]` becomes `[canvas]`, and line 223
`[canvas, canvas, null]` becomes `[canvas, null]`. The `stopped` assertions are unchanged.

**R4. New rows** (from "Test Cases"; none of them exist at 212bb54):
- the `getParameter` row (amended above);
- the palette rows:
  - a `diagnostic: [1, 0, 0, 1]` palette is used for the underline draw;
  - `FALLBACK_PALETTE.token` equals `GPU_TOKEN_COLORS` parsed;
  - `rgbaCss` of every fallback field equals the legacy string;
  - `readPalette` on a root whose computed style gives `--vt-danger: #ff0000` returns
    `diagnostic [1, 0, 0, 1]`, and an unparsable value falls back;
- the `mount.test.ts` layering rows:
  - first child with class `vact-code-backdrop`, followed by the code canvas;
  - `cb(null)` removes it;
  - 60 frames with playing highlights give 0 `drawImage` and 0 background uploads;
  - `dispose` removes it.

**R5. Other sharedPaths.** The grep `grep -rln "backgroundUploads\|setBackground\|onBackgroundCanvas" editor/src editor/test`
at 212bb54 matches only `apis.ts`, `code/mount.ts`, `visual/mount.ts`, `panes.test.ts` and
`gpu.test.ts`. The other session-290 sharedPaths are expected to stay unedited.
`theme.css` already defines every section-7 token the palette reads.

**Reporting rule** (progress gate `scripts/implementation-progress-check.py`):
- Repaired findings go only into `addressedFeedback`/`resolvedFindings` with status
  `repaired` and their evidence.
- `risks`, `findings`, `authorSelfCheck.findings` and `authorSelfCheck.residualRisks` carry
  no critical, high, mid or medium item. They may hold a low or unrated note, or a genuinely
  unfixed defect.
- Pending independent review is not a risk.
- Run no mutation or negative-control command.
- Every verification record is `{command, exitStatus: 0, testsRun > 0, testsPassed,
  failureCount: 0, outcome: "passed", log}` for tests, or `{command, exitStatus: 0,
  outcome: "passed", log}` otherwise. Details go in `notes`.
- `priorVerification` is empty or uses the same format.

**Session 290 gates** (all exit 0; heavy suites one at a time):
- Inside the sandbox:
  - `cd editor && npm run check`;
  - `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts test/canvas/mount.test.ts test/canvas/frame.test.ts test/visual`;
  - `cd editor && ./node_modules/.bin/vitest run` (at least 778 tests, all passing);
  - the host-wasm wasm32 build;
  - the src-tauri cargo check.
- Outside the sandbox: `npm run test:style`, the behavior e2e, `npm run test:perf`
  (alone), strict clippy and full nextest (timeout 2400, alone).
- Line-count check: `wc -l` of every touched TS file is below 1000.

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
- New (session-290 amendment INT-S289-BD-PLAN-GETPARAM, design 15.3.8.14 section 4 "GL
  hygiene"): after init, spy on `gl.getParameter` (`vi.spyOn(f.r.gl, 'getParameter')`) and
  clear it. 100 iterations of `setViewport` plus an animation-only `render` (same and
  alternating sizes at DPR 1) -> 0 `getParameter` calls. Control branch in the same test: a
  context loss and restore (`webglcontextlost` then `webglcontextrestored`) -> at least 1
  call with `MAX_TEXTURE_SIZE`. The init-time reads (`CanvasRenderer.initialize`,
  `renderer.ts:126`; the atlas constructor, `atlas.ts:36`) are allowed. The test records
  their count in a comment and does not assert "exactly once".
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
- *`backgroundUploads` readers.* At the session-288 checkpoint (842cf6e),
  `grep -rn backgroundUploads editor/src editor/test` matches only `renderer.ts` and
  `gpu.test.ts`. No harness file and no `perf-hook.ts` reads it, so removing it needs no
  harness edit. Re-run the grep. If a new reader has appeared, keep the field at 0 instead
  of editing the reader, because a harness reader change is not a frame-alignment edit.
  Record the decision.

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

- [x] The visual mount calls back once with the canvas and once with null; no per-frame calls
- [x] The code mount stacks the visual canvas under the code canvas; no copy, no upload and no per-frame wake-up
- [x] `uploadBackground` and `setBackground` removed; 0 backdrop ledger bytes; 0 `getParameter` calls over 100 `setViewport`/animation frames, at least 1 fresh `MAX_TEXTURE_SIZE` read on context restore, and the init-time reads (`renderer.ts:126`, `atlas.ts:36`) recorded and allowed
- [x] Counter rows: 100 animation frames give 0 backdrop copies (0 `drawImage`, 0 canvas-source texture uploads, 0 `createCanvas`) and 0 `getParameter`
- [x] Session 290: `npm run check` exit 0 (the Rgba/string errors at `renderer.ts` 220 and 233 are fixed); the accepted OPT-TEXT `onPresentation` hunk in `mount.ts` is byte-identical to 212bb54
- [x] `palette.ts` maps the section-7 tokens; the diagnostic underline uses `--vt-danger`; the fallback equals today's colors
- [x] Ported backdrop rows keep every non-background assertion; `panes.test.ts` updated to the once/null contract
- [x] Behavior e2e, `test:style`, default vitest, `npm run check`, `test:perf`, clippy, nextest, wasm32 build and src-tauri check pass
- [x] Harness sharedPaths are unedited
- [x] Progress log updated

## Progress Log

### Session: 2026-10-06 (session 286 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 4 (backdrop, GL limits,
palette).
**Notes**: Wave 12, after TEXT. Split from OPT-RENDER so the zero-copy change is verified
alone.

### Session: 2026-10-06 (session 288 plan amendment)
**Tasks Completed**: Ownership amendment only (design 15.3.8.14 section 8, "Harness
sharedPaths").
**Notes**:
- Added the 8 harness and style files as concrete sharedPaths, here and in the manifest,
  with the frame-alignment-only rule.
- Resolved the `backgroundUploads` reader pitfall: no harness reader exists (step3 review
  finding, low).
- Scope, contracts, tasks and criteria are otherwise unchanged.

### Session: 2026-10-06 (session 290 plan amendment)
**Tasks Completed**: Operator decisions INT-S289-BD-PLAN-GETPARAM and ownership decision 2;
design 15.3.8.14 sections 4 and 8 (session-290 amendment).
**Notes**:
- The getParameter criterion is now: 0 over 100 `setViewport`/animation frames, at least 1
  fresh `MAX_TEXTURE_SIZE` read on restore, and the init reads at `renderer.ts:126` and
  `atlas.ts:36` allowed. This plan's Test Cases and Completion Criteria and the manifest
  `acceptanceCriteria[2]` are updated.
- Added 12 concrete sharedPaths (the 15-file design list minus the three existing
  writePaths).
- Added the "Session 290 resume" brief INT-S289-BD-IMPL-INCOMPLETE, which pins the
  `rgbaCss` hex format, the six `gpu.test.ts` ports, the `panes.test.ts` port and the new
  rows.
- Status is In Progress from the partial implementation at 212bb54.

### Session: 2026-10-06 (session 290 implementation)
**Tasks Completed**: TASK-B1 through TASK-B5; zero-copy backdrop contract, cached GL limit behavior, palette F1 and ported/new coverage.
**Notes**:
- `palette.ts` now formats opaque RGBA values as lowercase `#rrggbb`, preserving legacy atlas color identities. `renderer.ts` converts gutter and binding-label text colors at the `drawRun` boundary; `npm run check` is green.
- Ported the six backdrop-dependent GPU rows: the feedback ordering loses only the removed backdrop white-texture draw; 100 animation frames perform no texture upload, staging-canvas creation, `drawImage` or backdrop ledger allocation; large viewport caps, geometry pressure, retained text uploads, no atlas evictions and disposal remain asserted. The only retired non-background assertion is the `peakGeometry > 4 MiB - 20,000` check because its measured peak was backdrop staging, which no longer exists.
- Added `getParameter` counters: the renderer init read (`renderer.ts:126`) and atlas init read (`atlas.ts:36`) are permitted; 100 viewport/animation frames make zero queries; context restoration makes a fresh `MAX_TEXTURE_SIZE` read. Added diagnostic token, all nine fallback field legacy CSS values, token-map equality, CSS parse/fallback, mount stacking/null/dispose and 60-frame no-copy assertions. `panes.test.ts` retains the once/null callback contract.
- The accepted OPT-TEXT `onPresentation` hunk in `editor/src/code/mount.ts` remains byte-identical to `212bb54` (`git diff 212bb54 -- editor/src/code/mount.ts` is empty). Session-290 sharedPaths beyond the three test writePaths remain unchanged; harness and style paths were not edited.
- Fresh-source SHA-256 values are recorded in `tmp/canvas-cutover/opt-backdrop/receipt.json`. Per-edit intent records and immutable start snapshot reference are under `tmp/canvas-cutover/opt-backdrop/intent*.json`.
- Final verification: `npm run check` exit 0 (`check-final-final2.log`); focused GPU/mount/frame/visual 121/121 (`focused-final-final2.log`); `test/ui` 18/18 (`ui-final.log`); default vitest 781/781 (`vitest-final-final2.log`); UI style 4/4 browser/viewport combinations (`style-final.log`); behavior e2e 18/18 across Chromium and WebKit (`e2e-behavior-final.log`; WebKit synthetic clipboard/IME/touch limitations remain as documented); serial perf 1/1, median ratio 2.71 (`perf-final-source.log`); strict clippy exit 0 (`clippy-final-source.log`); full nextest 2,816/2,816 with 3 skipped, exit 0 (`nextest-final-source.log`); wasm32 build and Tauri check exit 0 (`wasm-final.log`, `tauri-check-final.log`); release wasm and editor production build exit 0 (`build-wasm-release-final.log`, `editor-build-final.log`). The last assertion-only edits are in TypeScript tests; production source, CSS, Rust and e2e harness sources were unchanged after their listed gates.
- Preliminary syntax and mount-selector errors were corrected and rerun successfully; the complete logs are retained as `check-pass-01.log`, `check-pass-02.log` and `focused-pass-01.log`. The expanded fallback check first expected `rgba(...)` for opaque colors (`focused-final-final.log`); the expected values were corrected to `#rrggbb` and the full focused/default suites passed (`focused-final-final2.log`, `vitest-final-final2.log`). These superseded attempts are not final verification records.
- No design changes, shared ownership changes, new dependencies, Rust edits or harness edits. Independent review, documentation/index archival, commit and push remain downstream workflow steps.
