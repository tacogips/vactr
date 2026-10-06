# Canvas Cutover OPT-RENDER-C: Overlay-Only Frames, Animation Budgets and GPU Status Badge Implementation Plan

**Status**: Ready (dispatch only after CANVAS-OPT-RENDER-B is accepted)
**Plan ID**: CANVAS-OPT-RENDER-C (session 291, wave 15; depends on CANVAS-OPT-RENDER-B)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 3 ("Dirty reasons"), section 4 ("Layers and draw calls", "Proof"), section 6 (caret and animation-only rows) and section 8 ("OPT-RENDER split (session-291 amendment)", block "CANVAS-OPT-RENDER-C")
**Parent plan**: impl-plans/active/canvas-cutover-opt-render.md (status `Split`; contract section 5 "mount.ts" and the remaining completion criteria)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-RENDER-C`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

After A and B, the renderer already keeps the caret, handles and selection out of the text
layer. The mount still defeats that:

- `editor/src/code/mount.ts:221` bumps `staticRevision` on every text frame, and `:224` and
  `:232` pass it as `annotationsRevision`.
- Selection-only frames still go through the full text-frame path.

C makes caret, selection, handle and blink frames end to end overlay-only:

- `textBuilds` 0 and syntax `captures` 0 on a caret move;
- for an animation-only frame: 0 atlas uploads, 0 layout builds, 0 staging canvases, 0
  backdrop copies, 0 `getParameter`, `getError` and `measureText`, 0 `mapWireSpan` with the
  revision unchanged, and at most 4 draws.

C also fixes the low follow-up at `editor/src/code/code.css:69-77`. The `.vact-code-gpu-status`
badge has no `z-index`, so the code canvas (`code.css:50`, `z-index: 1`, which draws the
0.85 scrim) paints over it.

## Non-goals

- No change to A's instance format, program or layers, or to B's segment cache and slot
  table.
- No change to the `FrameScheduler` reasons (`frame.ts` is a sharedPath; no edit expected).
- No change to syntax deferral, completion or check scheduling (OPT-TEXT owns them).
- No other CSS change: tokens, square styles and `theme.css` values stay the same.
- No threshold, Rust or dependency change.

## Ownership

The same writePaths, sharedPaths and rules as `canvas-cutover-opt-render-a.md`, with:

- artifact root `tmp/canvas-cutover/opt-render-c`;
- plan file `impl-plans/active/canvas-cutover-opt-render-c.md`;
- **expected edits:** `editor/src/code/mount.ts`, `editor/src/code/code.css`,
  `editor/src/code/renderer.ts` (only for overlay-input change detection, if needed),
  `editor/test/canvas/mount.test.ts`, `editor/test/canvas/gpu.test.ts` and
  `editor/test/canvas/edit-cost.test.ts`.

writePaths:

- `editor/src/code/renderer.ts`
- `editor/src/code/geometry.ts`
- `editor/src/code/atlas.ts`
- `editor/src/code/layout.ts`
- `editor/src/code/advances.ts`
- `editor/src/code/resources.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/perf-hook.ts`
- `editor/src/code/code.css`
- `editor/src/code/segments.ts`
- `editor/test/canvas/gpu.test.ts`
- `editor/test/canvas/mount.test.ts`
- `editor/test/canvas/edit-cost.test.ts`
- `editor/test/canvas/first-viewport.test.ts`
- `editor/test/canvas/contracts.test.ts`
- `editor/test/support/gl.ts`
- `editor/test/support/canvas.ts`
- `impl-plans/active/canvas-cutover-opt-render-c.md`
- artifact roots:
  - `tmp/canvas-cutover/opt-render-c`
  - `target`
  - `tree-sitter-vact/tree-sitter-vact.wasm`
  - `editor/node_modules/.vite`
  - `editor/dist`
  - `tmp/ui-style`

sharedPaths: the same 21 files as plan A, with the same rules. `editor/test/style/ui-style.mjs`
gets no edit: the badge test lives in `mount.test.ts`.

## Invariants (must hold after C)

- Every invariant of plans A and B.
- **`annotationsRevision`** changes only when one of these changes:
  - the surface annotations;
  - the selection;
  - the presentation annotations: composition, diagnostics, bindings, call-heads.

  It never changes only because a frame ran.
- **Selection-only frames.** A frame whose only dirty reason is `selection` (caret move,
  selection change, handles) calls the syntax provider's `spans()` 0 times. It also leaves
  `layout.stats.builds` and the renderer's `textBuilds` unchanged, and rewrites only the
  background and overlay layers.
- **Blink.** Caret blink toggles `cursorVisible` and rewrites only the overlay layer.
- **Animation-only frames** call no text, layout, atlas or syntax work.

## Contracts and Key Points

### 1. `mount.ts`

- **Drop the per-frame counter.** Remove `staticRevision++` per text frame (`mount.ts:221`).
  Instead keep `annotationsRevision` as a counter bumped only when the static annotation
  inputs change: the surface annotation revision, the selection, or the presentation
  composition, diagnostics or bindings. Compare by identity or revision; do not use
  `JSON.stringify`.
- **Syntax rows.** Keep them in `annotations`, reused from the last result when `spans()` is
  skipped (since OPT-TEXT), so `mount.test.ts:264-291` passes unchanged.
- **Selection-only frames.** When `ctx.reasons` contains only `selection` (or only
  `selection` and `view` with an unchanged capture window), skip the syntax provider call and
  `renderer.setText`, and call `renderer.render` with the same `textRevision`, the cached
  syntax rows, and the new cursor and handles.

### 2. Renderer (only if needed)

If A's renderer rewrites the background or overlay layer on every frame, add a cheap
overlay-input key so an unchanged overlay is not rewritten:

- the cursor;
- `cursorVisible`;
- the handles;
- `annotationsRevision`;
- the scroll;
- the size.

An animation-only frame rewrites only the dynamic playing and eval part of the background
layer, which is bounded by the ranges times the visible lines they span, using the existing
binary search. Do not touch the text layer, the slot table or the atlas.

### 3. `code.css`

Add `z-index: 2;` to `.vact-code-gpu-status` (`code.css:69-77`), matching
`.vact-code-diag-tooltip` (`code.css:78-80`). Change nothing else in the rule.

### Patterns to imitate

- `mount.ts:207` `syntaxDirty` (the existing reason test), when extending the
  selection-only gate.
- `mount.test.ts`, for mounted counting fakes and the 20,000-line fixture.

## Test cases (add; no existing assertion deleted)

**`gpu.test.ts`** (renderer level):

- *Caret move.* Changing only `cursor` and `handles` with the same `textRevision` and
  `annotationsRevision` gives `textBuilds` delta 0, 0 text-layer writes, an overlay layer
  rewritten (the decoded caret rect moved), and `lastFrameDraws <= 4`. The in-test control is
  a `textRevision` bump in the same test, which must rebuild at least 1 segment.
- *Animation-only frames.* 100 frames with playing ranges give:
  - 0 cell uploads;
  - 0 `textBuilds`;
  - 0 `createCanvas`;
  - 0 `getError` and 0 `getParameter`;
  - 0 `measureText` on any 2D context;
  - `lastFrameDraws <= 4` each frame.

  Keep A's row if it already covers this and add only the missing assertions.

**`mount.test.ts`** (mounted, fake GL, 20,000-line fixture where the file already has it):

- *Caret move* (ArrowRight, or a click-equivalent selection change) through the keystroke
  task plus the next frame: renderer `textBuilds` 0, syntax `captures` 0, `spans()` calls 0,
  `layout.stats.builds` delta 0. The in-test control is a typed character, which must give
  `textBuilds >= 1`.
- *100 animation-only frames* (playing highlights active, no edits).
  - **mapWireSpan seam.** No existing canvas test spies on `mapWireSpan`, so add one with
    `vi.spyOn(DocumentSync.prototype, 'mapWireSpan')`.
    - `DocumentSync.mapWireSpan` is at `editor/src/code/sync.ts:86-88`.
    - The mounted `HighlightScheduler` reaches it through the `map` closure at
      `editor/src/code/mount.ts:113`.
    - `HighlightScheduler.tick` (`editor/src/code/highlight.ts:168-172`) caches the mapping
      per (event, revision). It still calls `map` once for each event the first time that
      event becomes active, and for each event after a revision change.
  - **Order** (all in one test):
    1. *Inject.* Inject the playing events through the mounted test's fake client and
       clock. Every event must already be active (`start <= now`) and must stay active for
       the whole counted window (`end` later than the last counted frame time). There must
       be no event whose `start` falls inside the counted window, because a new onset
       legitimately maps once.
    2. *Warm up.* Run at least one frame, so each event is mapped once at the current
       revision.
    3. *Reset the counters:*
       - the `mapWireSpan` spy;
       - `getError` and `getParameter` on the fake GL;
       - `measureText` on every 2D context (the layout or advance measurement context and
         the atlas staging context);
       - atlas uploads, `layout.stats.builds`, `createCanvas` and `drawImage`;
       - the per-frame draw records.
    4. *Count.* Run 100 frames with no edit and no new onset.
  - **Assertions after step 4:**
    - 0 atlas uploads, 0 layout builds and 0 staging canvases (`createCanvas` 0);
    - 0 backdrop copies (`drawImage` 0, backdrop ledger bytes 0);
    - 0 `getParameter` and 0 `getError` on the fake GL;
    - 0 `measureText` on every 2D context (layout and atlas);
    - 0 `mapWireSpan` calls;
    - at most 4 draws in each frame.
  - **In-test control.** In the same test, one document edit (a revision bump) followed by
    one frame must call `mapWireSpan` at least once.
  - Never weaken a counter to "small" or skip it. If a counter is non-zero, fix the product
    path in this plan's writePaths.
- *Badge stacking.* The test reads `editor/src/code/code.css` through `node:fs` (resolved
  from the test file URL). It asserts that the `.vact-code-gpu-status` rule declares a
  numeric `z-index` greater than the `.vact-code-canvas` rule's `z-index` (1) and greater
  than the `.vact-code-backdrop` rule's `z-index` (0).

## Pitfalls

- Bumping `annotationsRevision` on a caret move brings back the per-caret static rebuild.
  The caret is not an annotation-revision input.
- Skipping `spans()` must not drop the syntax rows. Reuse the cached rows, or the text
  flickers to unstyled.
- A selection-only frame must still draw the selection rects (background layer) and the caret
  (overlay layer).
- Do not edit `ui-style.mjs` to test the badge. The style checks never change.
- Do not reorder layers or change A's instance format.

## Verification

Setup and records are the same as in plan A. Record `BASE=4262255`, `START` and the sha256
values in `tmp/canvas-cutover/opt-render-c/intent.json` and `receipt.json`.

**Inside the sandbox:**

| Command | Required evidence |
|---|---|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts test/canvas/mount.test.ts test/canvas/edit-cost.test.ts` | exit 0; `gpu.test.ts` passes every row (at least 49 plus the A, B and C rows); the C rows are listed |
| `cd editor && ./node_modules/.bin/vitest run test/canvas` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0; every file passes |
| `cd editor && npm run check` | exit 0 |
| `grep -n "staticRevision++" editor/src/code/mount.ts` | no match (notes only) |
| `grep -n "z-index" editor/src/code/code.css` | `.vact-code-gpu-status` declares `z-index: 2` (notes only) |

**Outside the sandbox:** the same table as plan A, with run id `s291-opt-render-c` and out dir
`../tmp/canvas-cutover/opt-render-c/behavior`:

- the behavior e2e in both browsers (canvas-only text, readback, IME, context loss, DPR);
- `test:style`;
- `test:perf` (alone);
- clippy;
- full nextest (alone, timeout 2400);
- the host-wasm wasm32 build;
- the src-tauri cargo check.

The record format, the no-negative-control rule and the reporting rule are the same as in
plan A.

## Overwrite and Drift Protocol

The same as in plan A, under `tmp/canvas-cutover/opt-render-c/`. Edit only this plan's
progress log. Plan-status edits to the parent plan are not this plan's job: the closeout and
the plan author handle them.

## Completion Criteria

- [ ] `mount.ts`:
  - [ ] no per-frame `staticRevision++`;
  - [ ] `annotationsRevision` bumped only on annotation, selection or presentation changes;
  - [ ] selection-only frames skip `spans()` and the text rebuild.
- [ ] Renderer-level and mount-level caret-move rows pass: `textBuilds` 0, `captures` 0,
  `spans()` 0, layout builds 0, with in-test controls.
- [ ] Mounted 100 animation-only frames pass, counted after the inject, warm-up and reset
  order:
  - [ ] 0 uploads, 0 layout builds, 0 staging canvases and 0 backdrop copies;
  - [ ] 0 `getParameter` and 0 `getError`;
  - [ ] 0 `measureText` on every 2D context;
  - [ ] 0 `mapWireSpan`, through `vi.spyOn(DocumentSync.prototype, 'mapWireSpan')`;
  - [ ] at most 4 draws per frame;
  - [ ] in-test control: an edit plus a frame gives at least 1 `mapWireSpan` call.
- [ ] `.vact-code-gpu-status` has `z-index: 2`, and the `mount.test.ts` badge-stacking row
  passes.
- [ ] `gpu.test.ts` passes every row (at least 49) with no assertion deleted.
- [ ] Full default vitest and `npm run check` pass. Outside the sandbox, the behavior e2e,
  `test:style`, `test:perf`, clippy, nextest, the wasm32 build and the src-tauri check pass.
- [ ] Harness and session-290 sharedPaths are unedited, or their edits are recorded.
- [ ] Progress log updated.

## Session 292 Dispatch Notes

- `START` is HEAD when C is dispatched, after B is accepted. The structure A froze and B's
  segment cache are read from `START`.
- Row floor: `gpu.test.ts` keeps at least as many passing rows as at `START` (53 after A,
  plus B's rows). The design's "at least 49" is the minimum; no row present at `START` is
  removed.
- Rust carry-forward and the deadline rule: the same as plan B, section "Session 292
  Dispatch Notes".

## Progress Log

### Session: 2026-10-06 (session 291 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 8 ("OPT-RENDER split
(session-291 amendment)").
**Notes**:
- C closes OPT-RENDER. CANVAS-EVIDENCE (wave 16) depends on C.
- Step-5 repair F-S291-C-ANIM-ROW:
  - the mounted animation row now lists 0 `getError` and 0 `measureText`;
  - it names the `DocumentSync.prototype.mapWireSpan` seam;
  - it fixes the inject, warm-up, reset and count order, with an edit control.
