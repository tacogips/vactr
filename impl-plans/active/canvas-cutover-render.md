# Canvas Cutover: Renderer Layers and Frame Scheduler Implementation Plan

**Status**: In Progress
**Plan ID**: CANVAS-RENDER (wave 1, no dependencies)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.3 (frame scheduler), 15.3.8.7 (GPU per-frame bounds), 15.3.8.2 (`call-head` kind)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

The design requires that unchanged text is not rebuilt on animation-only frames, that playing,
eval and caret animation update independently, that atlas uploads per frame are bounded, and
that a single scheduler owns rAF, visibility, DPR, resize and perf instrumentation.

This plan builds those internals as standalone, jsdom-tested units, so that CANVAS-MOUNT (wave 2)
only composes them. It must not touch `code/mount.ts` or any consumer.

**Decomposition note.** Design 15.3.8.9 lists `code/frame.ts` and the renderer changes under
MOUNT. They are split out into this wave-1 plan for parallelism, because they do not change any
consumer. This is a refinement of plan boundaries only; the architecture is unchanged.

Repository facts at c9e5a05:

- `editor/src/code/renderer.ts:149` `render(feedback)` redraws everything every call. It draws
  one static unit quad per rectangle through `u_rect` uniforms (`renderer.ts:245`), and the only
  vertex buffer upload happens once in `initialize()` (`stats.bufferUploads`).
- `stats.textBuilds` counts `layout.stats.builds` deltas.
- `editor/src/code/atlas.ts:31` `GlyphAtlas.tile(request)` uploads on a miss with no per-frame
  budget, and evicts by LRU (`stats.evictions`).
- `CodeAnnotation.kind` (`editor/src/app/apis.ts:39`) has no `'call-head'`.

**Interpretation of design 15.3.8.3** ("rebuild and upload only the dirty geometry ranges into
the reused buffer"). This renderer has no per-glyph vertex buffer: geometry is the static unit
quad plus a per-quad uniform. Therefore:

- "text geometry" means a cached CPU draw list of `{rect, texture, color}` built only in the
  text phase;
- the GPU vertex buffer stays a single upload for the renderer's lifetime
  (`stats.bufferUploads` unchanged across frames);
- instanced batching is out of scope (an optimization, not required).

## Non-goals

- No consumer migration, no `code/mount.ts` edit, no EditorView removal (CANVAS-MOUNT).
- No change to the `layout.ts` shaping algorithm, to resource caps, or to `resources.ts` limits.
- No instancing or WebGL program redesign.

## Ownership

writePaths: see the manifest entry `CANVAS-RENDER`.

| File | Change |
|------|--------|
| editor/src/code/frame.ts (new) | `FrameScheduler` and `PerfRecorder` |
| editor/src/code/renderer.ts | Static/animated layer split, cached draw list, `call-head` underline, `textPending` |
| editor/src/code/atlas.ts | Per-frame upload budget |
| editor/src/app/apis.ts | Add `'call-head'` to the `CodeAnnotation.kind` union. This is the only edit to this file. |
| editor/test/canvas/frame.test.ts (new) | Scheduler tests |
| editor/test/canvas/gpu.test.ts | New renderer and atlas tests; every existing assertion is kept |

sharedPaths (conditional):

- `editor/src/code/layout.ts`: edit only if a zero-width range needs a `coordsAtPos` fix.
- `editor/test/support/gl.ts`: add fake GL methods only if needed.

## Contracts (pinned; CANVAS-MOUNT composes these)

```ts
// renderer.ts (additive; existing callers keep working)
export interface RenderFeedback {
  annotations?: readonly CodeAnnotation[]; cursor?: number | null; handles?: readonly { pos: number; end?: boolean }[];
  textRevision?: number;                 // static-layer cache key; undefined = rebuild every call (legacy behavior)
  animated?: readonly CodeAnnotation[];  // 'playing' | 'eval' ranges, drawn every frame, never cached
  cursorVisible?: boolean;               // caret blink phase; default true
}
class CanvasRenderer { readonly textPending: boolean; /* stats.textBuilds counts draw-list rebuilds */ }

// frame.ts
export interface FrameHost {
  requestAnimationFrame(cb: FrameRequestCallback): number; cancelAnimationFrame(id: number): void;
  readonly devicePixelRatio: number; readonly innerHeight: number;
  matchMedia?(query: string): MediaQueryList; readonly visualViewport?: VisualViewport | null;
  ResizeObserver?: typeof ResizeObserver;
}
export interface ViewportInfo { width: number; height: number; dpr: number; keyboardInset: number }
export interface FrameContext { frameMs: number; textDirty: boolean }
export class FrameScheduler {
  constructor(host: FrameHost, doc: Document, element: HTMLElement,
              opts: { onFrame(ctx: FrameContext): void; onViewport(v: ViewportInfo): void; perf?: boolean });
  invalidateText(): void;                    // schedules one frame with textDirty = true
  setActive(owner: string, active: boolean): void; // animation owners keep frames coming
  request(): void;                           // one frame, textDirty = false unless invalidated
  readonly hidden: boolean;
  readonly perf: PerfRecorder | null;
  readonly stats: { frames: number; textFrames: number };
  dispose(): void;
}
export class PerfRecorder {                  // fixed rings, capacity 4096 each, no per-frame allocation
  recordFrame(frameMs: number, workMs: number, text: boolean, revision: number): void;
  recordKey(timeStampMs: number, revision: number): void;
  snapshot(): { frames: number[][]; keys: number[][] }; // copies out on demand only
}
```

## Tasks

### TASK-001: Renderer layer split (renderer.ts)

- Keep the existing draw order exactly:
  1. backdrop and scrim;
  2. selection;
  3. animated (playing, eval);
  4. source text tiles;
  5. gutter numbers;
  6. diagnostic, composition and call-head underlines;
  7. cursor;
  8. binding labels;
  9. handles.
- The static layers are 2, 4, 5, 6, 8 and 9. Cache them as two lists: before the animated
  layer, and after it. Rebuild a list only when any of these changed since the last build:
  `textRevision`, viewport, effective scale, `layout.font.generation`, atlas evictions or
  invalidation, or context restore. `textRevision === undefined` means always rebuild (legacy
  tests).
- Animation-only frames:
  - replay the cached lists, drawing animated ranges through `layout.rangeRects` and the cursor
    when `cursorVisible !== false`;
  - make no `atlas.tile` call and no `layout.shape` call;
  - use exactly one `uploadBackground` check (already revision-gated).
- `call-head` draws a 1 px underline in color `[0.55, 0.6, 0.66, 0.6]` at the bottom of each
  rect. Its label is never drawn.
- Context loss, restore, `fail()` and `dispose()` clear both cached lists. A cached list never
  survives a texture deletion.

### TASK-002: Atlas per-frame budget (atlas.ts)

- Add `beginFrame(budgetBytes = 1 << 20)` and change the signature to
  `tile(request): AtlasTile | null`. A miss whose upload would push this frame's uploaded bytes
  over the budget returns `null`. Hits always return.
- The renderer calls `beginFrame` once per `render`. A `null` tile sets `textPending = true`,
  skips that quad, and forces a rebuild on the next call.
- `textPending` clears once a build completes with no `null`.
- Keep the existing eviction and rollback behavior. The existing atlas tests keep passing,
  with call sites updated for the nullable return.

### TASK-003: FrameScheduler and PerfRecorder (frame.ts)

**Frame requests.**

- At most one pending rAF.
- `onFrame` is called with the rAF timestamp, never `performance.now()`.
- After `onFrame`, another frame is requested only while any `setActive` owner is active, or
  when an invalidation arrived during the frame.

**Visibility.**

- When `doc.visibilityState === 'hidden'`, or on the `freeze` event, cancel the pending rAF and
  set `hidden`.
- On `visibilitychange` to visible, or on `resume`: re-read the size and DPR (calling
  `onViewport` if they changed), call `invalidateText()`, and request one frame.
- No rAF is scheduled while hidden, even if `request()` or `invalidateText()` is called. Record
  the pending dirtiness and apply it on resume.

**DPR.**

- Use `host.matchMedia(`(resolution: ${dpr}dppx)`)` with a `change` listener that re-registers
  itself at the new DPR, then calls `onViewport` and `invalidateText`.

**Size.**

- A `ResizeObserver` on `element`, plus `visualViewport` `resize` and `scroll`, are coalesced:
  `onViewport` is called at most once per frame, from inside the next frame before `onFrame`.
- `keyboardInset = max(0, innerHeight - (visualViewport.height + visualViewport.offsetTop))`,
  or 0 without `visualViewport`.

**Perf and disposal.**

- `perf` is non-null only when `opts.perf` is set. `onFrame` work time is measured around the
  call with `performance.now()` (allowed only for that measurement).
- `dispose()` removes every listener and observer, cancels the rAF, and is idempotent.

**Imitate**: the listener bookkeeping in `editor/src/code/pointer.ts:41-43` (`listen` plus a
cleanup array), and the injected rAF in `editor/src/visual/frame.ts` (`scheduler` seam).

### TASK-004: Tests

| File | Situation | Expected outcome |
|------|-----------|------------------|
| frame.test.ts | Idle with no owners active | Zero rAF requests after the first frame |
| same | `setActive('playing', true)` for 5 frames, then false | Exactly 5 or 6 frames, then none |
| same | `invalidateText()` twice before a frame | One frame with `textDirty: true` |
| same | Hidden, then `invalidateText()` | No rAF while hidden; on visible, one frame with `textDirty: true` and `onViewport` called once |
| same | DPR media change 1 to 2 | `onViewport` with dpr 2; the listener is re-registered for 2dppx |
| same | 10 resize callbacks within one frame | One `onViewport` |
| same | visualViewport `height = 500`, `offsetTop = 0`, `innerHeight = 800` | `keyboardInset = 300` |
| same | `dispose()` | Every `removeEventListener` and `disconnect` called; later events do nothing |
| same | `PerfRecorder` 5000 frames | Keeps the newest 4096; `snapshot()` lengths are 4096 |
| gpu.test.ts | Two `render` calls with the same `textRevision` and different `animated` ranges | Second call: `stats.textBuilds` unchanged, `atlasStats.uploads` unchanged, `stats.bufferUploads` unchanged, `stats.frames` + 1 |
| same | `textRevision` changed | Draw list rebuilt (`textBuilds` + 1) |
| same | Atlas budget of 1 MiB with a first frame needing 3 MiB of tiles | `textPending === true` after frame 1; after at most 3 more `render` calls `textPending === false` and every visible tile drawn |
| same | `call-head` annotation | Underline quad present; no label run uploaded for it |
| same | Context loss, then restore, then render with the same `textRevision` | Rebuild happens; no deleted texture is bound (fake GL asserts) |

## Pitfalls

- Caching tile textures across an atlas eviction, which binds a deleted texture. Key the cache
  on `atlas.stats.evictions` plus an invalidation counter.
- Calling `performance.now()` for frame time. Frame time must be the rAF argument.
- Scheduling rAF while hidden, or polling visibility with timers.
- Changing behavior when `textRevision` is undefined. That breaks the existing gpu tests,
  which must pass unchanged except for the nullable `tile()` call-site updates.
- Editing `editor/src/app/apis.ts` beyond the one union member, or touching
  `editor/test/canvas/contracts.test.ts`. CANVAS-CLOCK owns that file in this wave.

## Session 266 Resume Amendment (operator decisions A, B and D)

The source for this plan is already implemented at `40467f3` (pushed WIP checkpoint). It is not
accepted yet. This session re-verifies that source; it does not re-implement it.

- **Artifact roots** (manifest `CANVAS-RENDER.artifactRoots`, each also a writePath): `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm`, `tmp/canvas-cutover/render`, `editor/node_modules/.vite`.
  These are gitignored outputs and are never committed. If a command writes any other gitignored
  in-repo path, stop, record the path in this progress log, and ask for a serial manifest
  amendment. Never `git add -f`.
- **Setup (not gating)**: `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`,
  then the host-wasm library build.
- **Decision B**: run the Verification table below on the unchanged `40467f3` bytes and write new
  evidence to `tmp/canvas-cutover/render/checks.log`: command, exit code, test count, complete log
  path, and the sha256 of every writePath source file. Before running, confirm that each source
  hash equals `git show 40467f3:<path> | shasum -a 256`. Do not edit source unless a review
  finding requires it. If a review fix changes source, re-run the full table on the fixed bytes.
- **Rule D**: the gating list contains only final-source commands that are expected to pass. A
  failed run that was then fixed is replaced by its passing re-run, and the failure goes only into
  a history note. Mutation and negative-control runs go under `mutationEvidence`. The session-265
  ENOENT full-Vitest run is history, not gating evidence.
- **Do not**: touch `editor/test/canvas/contracts.test.ts` (CANVAS-CLOCK owns it in this wave),
  delete or skip assertions, or lower the 612 floor.

## Verification (record in tmp/canvas-cutover/render/checks.log)

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/canvas/frame.test.ts test/canvas/gpu.test.ts test/canvas/state.test.ts test/canvas/input.test.ts test/canvas/contracts.test.ts` | all pass |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | all pass (full suite; at least 612 tests, none removed; 0 failed files) |
| `wc -l editor/src/code/renderer.ts editor/src/code/frame.ts editor/src/code/atlas.ts` | each < 1000 |

## Overwrite and Drift Protocol

Record fresh-read sha256 values before each edit (`tmp/canvas-cutover/render/intent.json`) and
after it (`receipt.json`). On drift, stop editing that file and repair serially after the join.
Edit only this plan's progress log.

## Completion Criteria

- [x] Contracts exactly as pinned
- [x] Animation-only frames proven to perform zero shaping, zero atlas uploads and zero buffer uploads
- [x] `textPending` budget behavior tested
- [x] Scheduler hidden, DPR, resize, inset and dispose tests pass
- [ ] `npm run check` exit 0; full vitest passes with no assertion deleted (type check passes; full suite awaits approved tree-sitter WASM artifact path)
- [ ] Session 266: new final-source gating evidence on the `40467f3` bytes (or on the reviewed fix): `npm run check` exit 0, focused Vitest pass, host-wasm build exit 0, full Vitest pass (at least 612 tests, 0 failed files), line counts < 1000; source sha256 values recorded in `checks.log`
- [ ] Session 266: test-integrity, adversarial and integration reviews accepted

## Progress Log

### Session: 2026-10-05
**Tasks Completed**: Plan authored

### Session: 2026-10-05 CANVAS-RENDER implementation
**Tasks Completed**: TASK-001, TASK-002, TASK-003, TASK-004 implementation and focused verification
**Notes**: Added cached renderer draw layers and `call-head`, 1 MiB atlas upload budget with `textPending`, and `FrameScheduler`/`PerfRecorder`. Focused Vitest passed 132/132 and `npm run check` passed. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` passed. Full Vitest ran 80 files: 606 passed, 6 skipped, 2 suites failed because `tree-sitter-vact/tree-sitter-vact.wasm` is absent. Building it with the repository task writes outside this plan's empty `artifactRoots`; resume after the dispatch artifact root is amended or the test asset is otherwise made available within an authorized artifact path. `renderer.ts`, `frame.ts`, and `atlas.ts` are 359, 130, and 105 lines.

### Session: 2026-10-05 (session 266 plan amendment)
**Tasks Completed**: Plan amended per operator decisions A, B and D. Artifact roots declared, setup separated from gating, the blocked full-Vitest row replaced by a gating row with a 612-test floor, re-verification criteria added. Source was not changed.
**Notes**: The operator generated `tree-sitter-vact/tree-sitter-vact.wasm` with `mise run ts-build-wasm` (gitignored). The operator's 612/612 run on `40467f3` is historical only; this session needs new gating evidence.
