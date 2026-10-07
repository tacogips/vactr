# Canvas Cutover OPT-RENDER-B: Per-Line Geometry Cache and Incremental Slot Table Implementation Plan

**Status**: Completed (2026-10-07)
**Plan ID**: CANVAS-OPT-RENDER-B (session 291, wave 14; depends on CANVAS-OPT-RENDER-A)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 4 ("Per-line geometry cache", "Context loss, DPR and disposal", "Proof"), section 6 (single-char edit budget row) and section 8 ("OPT-RENDER split (session-291 amendment)", block "CANVAS-OPT-RENDER-B")
**Parent plan**: impl-plans/completed/canvas-cutover-opt-render.md (status `Split`; contract section 2 "Text buffer", "Segment key", "Gutter segments" and "Slot table")
**Manifest**: impl-plans/completed/canvas-cutover-dispatch.json (entry `CANVAS-OPT-RENDER-B`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-07

---

## Intent and Context

After A, each text-dirty frame rebuilds every visible line's instances and rewrites the
whole slot table. B changes this so that an edit costs only the lines it changes. The goal is
the WebKit edit-path budget of design section 6 for a single ASCII edit on the
20,000-line fixture:

- atlas uploads at most the new cells;
- geometry bytes at most the changed line's segment bytes;
- `getError` 0, `getParameter` 0 and `measureText` 0;
- at most 4 draws.

Enter rebuilds at most 2 text segments, plus the slot table and the renumbered gutter
segments.

B builds on A's output, which it must not change:

- the 32-byte instance format and the flag bits (`editor/src/code/geometry.ts`);
- the single program, including the `texelFetch` slot path;
- the four `LayerBuffer`s and the draw order.

Each integration review checks this.

Two facts in the current code matter here:

- `TextLayout.setText(doc, changes)` (`layout.ts:81-118`) keeps a shifted, unchanged line's
  `runs` array object. It mutates the run `from`/`to` in place and replaces only the
  `ShapedLine` wrapper. So `line.runs` identity is a stable line identity across
  renumbering.
- `mount.ts:161` calls `layout.setText(presentation.doc, changes)`. `mount.ts:196` then
  calls `renderer.setText(doc)`, and the inner `layout.setText(doc)` returns early because the
  doc is identical. In tests, call `layout.setText(doc, changes)` before
  `renderer.setText(doc)` to get the same path.

## Non-goals

- No change to the instance format, the shaders, the layer set, the draw order or the atlas
  contract.
- No `mount.ts` feedback or overlay-only work (that is C), and no `code.css` edit.
- No new compaction heuristics beyond the parent contract.
- No hit-testing change, cap change, threshold change, Rust or dependency change.

## Ownership

The same writePaths, sharedPaths and rules as `canvas-cutover-opt-render-a.md`, with this
plan's own artifact root and plan file:

- `tmp/canvas-cutover/opt-render-b`
- `impl-plans/completed/canvas-cutover-opt-render-b.md`

writePaths:

- `editor/src/code/renderer.ts`
- `editor/src/code/geometry.ts`
- `editor/src/code/atlas.ts`
- `editor/src/code/layout.ts` (additive only)
- `editor/src/code/advances.ts` (additive only)
- `editor/src/code/resources.ts`
- `editor/src/code/mount.ts` (no edit expected; compile fixes only)
- `editor/src/code/perf-hook.ts` (additive stats only)
- `editor/src/code/code.css` (no edit)
- `editor/src/code/segments.ts` (create it if `renderer.ts` would reach 1,000 lines; this is
  the expected home of the segment cache)
- tests:
  - `editor/test/canvas/gpu.test.ts`
  - `editor/test/canvas/mount.test.ts`
  - `editor/test/canvas/edit-cost.test.ts`
  - `editor/test/canvas/first-viewport.test.ts`
  - `editor/test/canvas/contracts.test.ts`
  - `editor/test/support/gl.ts`
  - `editor/test/support/canvas.ts`
- `impl-plans/completed/canvas-cutover-opt-render-b.md`
- artifact roots:
  - `tmp/canvas-cutover/opt-render-b`
  - `target`
  - `tree-sitter-vact/tree-sitter-vact.wasm`
  - `editor/node_modules/.vite`
  - `editor/dist`
  - `tmp/ui-style`

sharedPaths: the same 21 files as plan A (`editor/src/code/frame.ts`, the 8 harness files,
the 12 session-290 files), with the same edit rules.

## Invariants (must hold after B)

- Every invariant of plan A.
- **Changed lines only.** A text-dirty frame rebuilds only the segments whose key changed or
  that newly entered the window. `stats.textBuilds` counts **segment** builds from now on.
- **Uploads.** Text geometry uploads are `bufferSubData` of changed blocks only.
  `bufferData` on the text buffer happens only on capacity growth, after an atlas reset, on
  restore or on compaction, and at most once per frame.
- **Slot-table writes.** The slot table is written only for slots whose y changed or that
  were newly assigned. Count the bytes in `stats.slotTableBytes`.

## Contracts and Key Points

### 1. Segment (`segments.ts` or `geometry.ts`)

- **Shape.** A segment is one (line identity, 256-cluster chunk index) whose x-range
  intersects `[scrollLeft - width, scrollLeft + 2 * width]`.
- **Key.** `{ runs: readonly ShapedRun[] (identity), chunk: number, fontGeneration, dpr, styleHash, atlasGeneration }`.
  - `styleHash` is computed from the line's syntax rows **relative to `line.from`**, as
    (offset, length, className) triples. Otherwise a shifted line looks changed.
  - The text identity is the `runs` array identity. A line whose text changed gets a new
    `runs` array from the layout.
- **Storage.** A segment owns a list of 64-instance blocks in the text `LayerBuffer`.
  Instance y is 0, line-local, and its slot index is in `a_meta`.
- **Window membership.** Shape rows in `[visibleFirst - overscan, visibleLast + overscan]`,
  with overscan equal to one viewport of lines. Keep already-built segments while their line
  remains in that window; materialize unseen offscreen geometry only when its line enters the
  viewport, so an edit does not synchronously rasterize unrelated offscreen text. Segments
  outside the window are freed: their blocks go to the free list and are zero-written with
  `bufferSubData`.

### 2. Text buffer blocks

- **Capacity.** The text `LayerBuffer` holds blocks of 64 instances (2 KiB). Capacity grows
  by doubling, up to `MAX_TEXT_BLOCKS = 1536` (3 MiB), reserved in the ledger `geometry` kind
  before `bufferData`.
- **At the cap.** When the blocks needed exceed the cap, render what fits, set
  `text-pending`, and never omit a line silently.
- **Allocation.** The free list is reused first, then the high-water mark grows.
- **Compaction.** When `highWater > 2 * usedBlocks`, rewrite the live segments contiguously
  with one `bufferData` (`stats.compactions += 1`, at most once per frame).
- **Draw.** The draw uses `count = highWater * 64`. Zero-size instances (freed or unused
  slots) are skipped by the shader through degenerate quads.

### 3. Slot table (incremental)

- **Assignment.** A slot is assigned to a line identity (the `runs` array) while its segment
  lives, from a free-slot pool of `MAX_SLOTS = 1024`. Gutter instances use the same slot.
- **Writes.** Each frame, for each live slot, compute `y = line.number * lineHeight`, and
  write only the slots whose y changed, coalesced into contiguous `texSubImage2D` runs.
  Enter on line 30 therefore changes the y of the following visible lines' slots (slot-table
  bytes) and rebuilds none of their text segments.
- **Release.** A freed segment releases its slot.

### 4. Gutter segments

One segment per visible line, keyed by (line-number string, slot, fontGeneration, dpr,
atlasGeneration), using digit mask cells with `TEXT_SPACE | NO_HSCROLL`. It is rewritten only
when the line-number string changes, so Enter rewrites the gutter segments of renumbered
lines only.

### 5. Renderer orchestration (`renderer.ts`)

- **Text pass.** Replace A's whole-window rebuild with a diff against the current window:
  1. build the segments whose key changed or that are new;
  2. free the segments that left the window;
  3. update the slot table;
  4. write the changed blocks.

  `stats.geometryBytes` adds only the `bufferSubData` bytes of the text layer; the
  slot-table bytes are counted separately.
- **Atlas generation change** (a reset or growth): mark every segment dirty and rebuild the
  visible ones under the per-frame budget with `text-pending`. One `bufferData` is allowed
  that frame.
- **Restore.** Rebuild the buffer and slot table from the CPU caches and mark every segment
  dirty. **DPR change:** the atlas resets and the segments reset.
- **`setDocument` (string mode)** keeps its current semantics. The layout drops the shifted
  lines, so string-mode tests rebuild them. Gate the Enter row on `setText(doc, changes)`.

### Patterns to imitate

- A's `LayerBuffer.write`, for reservation-before-allocation.
- `layout.ts:81-118` `setText`, for change-set-aware identity reuse.
- `edit-cost.test.ts` and `mount.test.ts`, for the 20,000-line fixture and counting fakes
  (reuse their fixture helpers; do not create a new fixture generator).

## Test cases (add; no existing assertion deleted)

**`gpu.test.ts`** (additive stub font, Text-backed layout through `setText(doc, changes)`):

- *One-line edit.* An edit inside one line of a 60-line viewport gives:
  - `geometryBytes` delta <= `ceil(clusters(line) / 64) * 2048`;
  - `textBuilds` delta exactly 1;
  - atlas `uploads` delta <= the new clusters;
  - 0 `bufferData` calls on the text buffer.

  The in-test control is a font-generation bump, which must rebuild every visible segment.
- *Enter on line 30.* Gives `textBuilds` delta <= 2, `slotTableBytes` delta > 0, gutter
  rewrites only for lines >= 30, and line 31+ text segments not rebuilt (their block indices
  are unchanged).
- *Unchanged shifted line.* A line shifted by an insert above gives 0 new cells, 0 segment
  builds for that line and a slot-table write only.
- *Scroll by one line.* Builds only the newly visible line's segments; the leaving line's
  existing segments remain cached while inside the one-viewport overscan window, then free
  after scrolling beyond that window (free-list reuse is visible on a later build).
- *Compaction.* After freeing more than half the blocks, compaction runs once (`compactions`
  1) and the decoded text instances are unchanged.
- *Restore.* A context loss and restore rebuilds every visible segment once and drops no
  line.
- *DPR change.* Rebuilds every visible segment, with the atlas generation bumped.

**`edit-cost.test.ts`** (or `mount.test.ts`, whichever already mounts the 20,000-line fixture
with an injectable `gl`) uses a mounted document with a counting fake GL and a counting 2D
context. A single ASCII character typed through the keystroke task plus the next frame gives:

- `getError` 0 and `getParameter` 0;
- atlas uploads <= the new cells;
- text geometry bytes <= the changed line's segment block bytes;
- `measureText` 0 on every 2D context (layout and atlas);
- `lastFrameDraws <= 4`.

The in-test control is a non-ASCII (Japanese) character in the same test, which must call
`measureText` or upload at least one new cell.

## Pitfalls

- A style hash computed with absolute offsets makes every shifted line look changed. Use
  offsets relative to `line.from`.
- Matching segments by line number instead of `runs` identity rebuilds every line after an
  Enter.
- Zero-write freed blocks; otherwise stale glyphs draw.
- Do not reallocate the buffer on every growth inside one frame. Grow once to the needed
  capacity.
- Do not change the instance format, the shader or the layer order (A owns them).
- The slot table must never exceed `MAX_SLOTS`. Bound the window and use `text-pending`.

## Verification

Setup and records are the same as in plan A. Record `BASE=4262255`, `START` and the sha256
values in `tmp/canvas-cutover/opt-render-b/intent.json` and `receipt.json`.

**Inside the sandbox:**

| Command | Required evidence |
|---|---|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts test/canvas/mount.test.ts` | exit 0; the new B rows are listed |
| `cd editor && ./node_modules/.bin/vitest run test/canvas` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0; every file passes |
| `cd editor && npm run check` | exit 0 |
| `wc -l editor/src/code/renderer.ts editor/src/code/geometry.ts editor/src/code/segments.ts editor/src/code/atlas.ts` | each below 1000 (notes only) |
| `git diff START -- editor/src/code/geometry.ts` | no change to the instance offsets, flag values or shader sources (notes only) |

**Outside the sandbox:** the same table as plan A, with run id `s291-opt-render-b` and out dir
`../tmp/canvas-cutover/opt-render-b/behavior`:

- the behavior e2e in both browsers;
- `test:style`;
- `test:perf` (alone);
- clippy;
- full nextest (alone, timeout 2400);
- the host-wasm wasm32 build;
- the src-tauri cargo check.

The record format, the no-negative-control rule and the reporting rule are the same as in
plan A.

## Overwrite and Drift Protocol

The same as in plan A, under `tmp/canvas-cutover/opt-render-b/`. Edit only this plan's
progress log.

## Completion Criteria

- [x] Segments are keyed by `runs` identity, chunk, font generation, DPR, line-relative style
  hash and atlas generation, and only window lines get segments.
- [x] Text buffer blocks:
  - [x] free list;
  - [x] bounded compaction;
  - [x] `bufferSubData` for changed blocks only;
  - [x] `bufferData` only on growth, reset, restore or compaction, at most once per frame.
- [x] Slot table with incremental writes counted in `slotTableBytes`, and gutter segments
  rewritten only on renumbering.
- [x] One-line edit, Enter, shifted line, scroll, compaction, restore and DPR rows pass.
- [x] The 20,000-line single ASCII edit row passes: `getError`/`getParameter`/`measureText`
  0, uploads at most the new cells, geometry at most the changed segment, at most 4 draws,
  with an in-test non-ASCII control.
- [x] A's instance format, program and layers are unchanged.
- [x] Full default vitest and `npm run check` pass. Outside the sandbox, the behavior e2e,
  `test:style`, `test:perf`, clippy, nextest, the wasm32 build and the src-tauri check pass.
- [x] Harness and session-290 sharedPaths are unedited, or their edits are recorded.
- [x] Progress log updated.

## Session 292 Dispatch Notes

- `START` is HEAD when B is dispatched, after A is accepted on `35140bd`. The structure
  A froze (instance format, program, four layers) is read from `START`.
- Row floor: `gpu.test.ts` keeps all 53 rows passing at `START`, plus B's new rows. No row
  present at `START` is removed.
- Rust carry-forward: B edits no Rust. If
  `git diff --name-only 5e58d04 HEAD -- src Cargo.toml Cargo.lock build.rs editor/src-tauri`
  still prints nothing, full nextest may be carried forward exactly as in plan A, section
  "Session 292 Resume". Clippy, the wasm32 build and the src-tauri check run fresh where the
  sandbox allows. The closeout reruns every Rust gate fresh.
- Deadline: if the step deadline cuts the pass short, keep the partial work in the tree.
  Record what is done and what remains in this progress log, and report it as a
  severity-low note for an operator Luna completion pass. Do not revert it.

## Progress Log

### Session: 2026-10-06 (session 291 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 8 ("OPT-RENDER split
(session-291 amendment)").
**Notes**:
- B owns the slot table (design decision): its Enter criterion needs it.
- Dispatch only after A is accepted.

### Session: 2026-10-07 (Step 6 implementation)
**Tasks Completed**: Implemented retained 256-grapheme text segments, free-list GPU block allocation,
incremental slot writes, line-number gutter reuse, bounded compaction, and 20,000-line hot-path controls.
**Evidence**:
- Focused canvas/edit-cost/mount tests: 92/92; canvas suite: 212/212; full vitest: 792/792.
- `npm run check`, behavior E2E Chromium/WebKit 18/18, style 4/4, serial perf 1/1 passed.
- Strict clippy, full nextest 2816/2816 (3 skipped), wasm32 build and Tauri check passed.
- `tmp/canvas-cutover/opt-render-b/source-invariants.log` confirms A's instance constants, offsets,
  flags and shader sources remain unchanged; source files remain below 1000 lines.
- Final command logs and `receipt.json` are recorded under `tmp/canvas-cutover/opt-render-b/`.
**Notes**: Shared harness paths were not edited. Test-integrity, adversarial and serial integration
reviews are downstream workflow steps and remain pending.
- Intermediate chunk-overbuild and geometry-budget failures were resolved by horizontal overscan,
  chunk-local syntax filtering and packing gutter commands with the first visible text segment;
  diagnostic logs are `gpu-chunk-rerun.log`, `gpu-gutter.log` and `focused-final.log` under the
  plan evidence directory. Final-source reruns are the passing logs listed above.
- The initial eager vertical overscan experiment uploaded 10,240 bytes (five blocks) during the
  mounted edit frame and was reverted; this finding led to lazy geometry materialization below.
  Diagnostic evidence remains in `mount-overscan-diagnostic.log` and `mount-warmup.log`.

### Session: 2026-10-07 (Step 6 continuation: vertical overscan)
**Tasks Completed**: Added a one-viewport vertical shaping and retention window with lazy geometry
materialization on viewport entry. Existing offscreen segments remain cached within overscan and
are freed after leaving it; unseen offscreen geometry is deferred so unrelated long lines do not
inflate the mounted edit upload budget.
**Evidence**: GPU/mount focused run passed 79/79 at
`tmp/canvas-cutover/opt-render-b/vertical-overscan-focused-4.log`; mounted 20,000-line ASCII edit
kept geometry uploads within one changed-line block. Earlier eager-build failures remain diagnostic
evidence in `mount-overscan-diagnostic.log` and `mount-warmup.log`.

**Final-source verification**: focused 92/92, canvas 212/212, full vitest 792/792, check,
behavior E2E 18/18, style 4/4, serial perf 1/1 (ratio 2.84), strict clippy, full nextest
2816 passed / 3 skipped, wasm32 build, Tauri check and source invariants all passed. Complete logs
are listed in `tmp/canvas-cutover/opt-render-b/receipt.json`; full nextest took 891.631 seconds.
Test-integrity, adversarial and serial integration reviews remain downstream workflow steps.

### Session: 2026-10-07 (Step 6 continuation: compaction reference repair)
**Tasks Completed**: Replaced the compaction frame's same-renderer self-comparison with an
independent cold `CanvasRenderer` reference at the final viewport. The fake decodes instance
slots and resolves viewport-relative absolute positions through its slot table. The row compares
sorted visible non-padding text tuples (excluding UVs), asserts ten visible lines and compaction
count +1, and verifies a scrollTop 40 reference differs.
**Evidence**: `gpu.test.ts` 60/60 (`repair-s297-gpu.log`); focused 92/92
(`repair-s297-focused.log`); canvas 212/212 (`repair-s297-canvas.log`); full Vitest 792/792
(`repair-s297-vitest-full.log`); `npm run check` and `git diff --check` exit 0. The source-token
scan is clean. Logs are under `tmp/canvas-cutover/opt-render-b/`.
**Source identity**: GPU test SHA-256 `b2413b3fad7e5ddc1e2da0152f4cb842ce3c7066b67d2ba3187710f7948b5060`.
Production renderer, segments, layout and geometry hashes remain `7c0de895...`, `62e4beea...`,
`c033fba9...` and `0495442b...`; therefore prior behavior E2E, style and serial perf evidence
continues to match production sources. Receipt records the full hashes and gate logs.

### Session: 2026-10-07 (Step 6 continuation: adversarial slot and gutter repairs)
**Tasks Completed**: Added explicit slot ownership and release on eviction/invalidation so a returning
line cannot reuse a slot that has already returned to the free list. Added a scroll regression row
covering 400 -> 420 -> 400 -> 340 -> 160 with unique visible slots, expected row positions, and a
retained-slot/entering-segment control. Gutter segments now build at the fixed x=4 origin; replay
compensates for the current horizontal scroll only for appended gutter commands, preserving the
cache key and text geometry. Added a horizontal scroll and new-row gutter regression row with a
text-shift control.
**Evidence**: GPU 62/62, focused 94/94, canvas 214/214, full Vitest 794/794, `npm run check`,
behavior E2E 18/18 (Chromium 10/10, WebKit 8/8), style 4/4, serial perf 1/1 (ratio 2.86),
forbidden-token scan and `git diff --check` all pass. Logs are recorded in the receipt under
`tmp/canvas-cutover/opt-render-b/repair-s298-*`; Rust diff against HEAD is empty.
**Source identity**: Renderer SHA-256 `e502280d788203b849393f1ac2d20345dac491f4f11e6fb2e9015e88e5cf4656`;
GPU test SHA-256 `4ec441d5b5cd8236da3b4e7c56e08e108deb8d6c9165a7348d760b9031c28517`.
Independent test-integrity, adversarial and serial integration reviews remain downstream workflow steps.

### Session: 2026-10-07 (Step 6 continuation: far horizontal gutter fallback)
**Tasks Completed**: When a row has a cached gutter segment but no visible text chunk carries its
gutter commands, the renderer writes those commands into blocks owned by the gutter segment. The
horizontal chunk filter and segment keys remain unchanged. The gutter row now includes an empty
line, far-scroll frames at `scrollLeft: 1000` for `scrollTop: 0` and `400`, a control confirming
short-line text is filtered while its gutter remains, and duplicate-instance checks across
standalone/packed placement transitions.
**Evidence**: GPU 62/62, focused 94/94, canvas 214/214, full Vitest 794/794, `npm run check`,
behavior E2E 18/18 (Chromium 10/10, WebKit 8/8), style 4/4, serial perf 1/1 (ratio 2.72),
forbidden-token scan and `git diff --check` all pass. Logs are recorded under
`tmp/canvas-cutover/opt-render-b/repair-s299-*`; Rust diff against HEAD is empty.
**Source identity**: Renderer SHA-256 `175918ee588a0f1a7f4d254c717f840b22d983396007911e83234cfaa2f521bb`;
GPU test SHA-256 `35a45e71f727e06a017ce0dc207b3931330f2ccccb05850cd80a9b2a65478605`.
