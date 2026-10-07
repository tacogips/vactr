# Canvas Cutover: First-Viewport Delivery Fix (Chromium 1x1 Canvas Backing) Implementation Plan

**Status**: Completed (2026-10-07)
**Plan ID**: CANVAS-EVIDENCE-VIEWPORT (wave 3, session 267; parallel with CANVAS-EVIDENCE-SILENT, -EDITCOST, -RUNSTART)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.3 (DPR and resize), #15.3.8.9 (owner-file defect-fix rule)
**Manifest**: impl-plans/completed/canvas-cutover-dispatch.json (entry `CANVAS-EVIDENCE-VIEWPORT`)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

In headless Chromium the code canvas has a 1x1 backing store although its CSS box is 767x858
(`tmp/canvas-cutover/evidence/pixel-diagnostic.log`: `"width":1,"height":1`). Nothing is
visible, and `canvas-only-text` fails. Design 15.3.8.9 requires this product defect to be fixed in
its owning file. The owner is CANVAS-MOUNT, which is accepted; this plan is the authorized
owner-file fix.

Root cause (diagnosed at plan time; the implementer must confirm it with the failing test
first):

- `editor/src/code/frame.ts` `FrameScheduler` constructor sets
  `this.viewportValue = this.readViewport()` (line 71).
- The first `tick` (lines 51-55) then calls `onViewport` only if the new reading differs:
  `if (!this.sameViewport(next, this.viewportValue))`.
- When the element already has its final size at construction (real browsers), the first
  reading equals the stored one, so `onViewport` never fires.
- `editor/src/code/mount.ts:137-159` therefore never calls `viewHost.setViewport`. `CodeViewHost`
  keeps `width = height = 1` (`editor/src/code/view-host.ts:9-10`), and every
  `renderer.setViewport(viewHost.viewport, dpr)` sizes the backing to 1x1.
- In jsdom the rect is 0x0 at both times, which hides the bug.

## Non-goals

- No change to `mount.ts`, `view-host.ts` or `renderer.ts` (owned by CANVAS-EVIDENCE-EDITCOST in
  this wave). The fix lives in `frame.ts` only.
- No change to the visibility, DPR media-query or ResizeObserver logic beyond first delivery.

## Ownership

writePaths:

- `editor/src/code/frame.ts`
- `editor/test/canvas/frame.test.ts`
- `editor/test/canvas/first-viewport.test.ts` (new)
- this plan file
- `tmp/canvas-cutover/evidence-viewport/intent.json` and `tmp/canvas-cutover/evidence-viewport/receipt.json`
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm` (setup builds only), `editor/node_modules/.vite`, `tmp/canvas-cutover/evidence-viewport/logs`

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`, then
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.

sharedPaths: none.

## Change

- `FrameScheduler`: the first tick that runs while visible delivers `onViewport` with the current
  reading unconditionally (for example a private `viewportDelivered = false` flag). After that,
  delivery stays change-only. `resume()` already marks the viewport dirty; keep it change-only
  after the first delivery.
- Do not call `onViewport` from the constructor. Mount's `onViewport` uses `viewHost` and
  `renderer`, which must be constructed first (`mount.ts:138-159` order), so delivery must
  happen in a tick.
- Keep the `ViewportInfo` shape and every public member unchanged.

## Tests (situation -> expected)

`editor/test/canvas/frame.test.ts` (imitate its existing fake `FrameHost` and element):

- The element rect is 767x858 at construction, dpr 1, and the first frame runs -> `onViewport`
  is called exactly once, with `{ width: 767, height: 858, dpr: 1, keyboardInset: 0 }`, before
  `onFrame`.
- A second frame with an unchanged rect -> no further `onViewport`.
- The rect changes to 800x600 and a ResizeObserver callback fires -> exactly one more
  `onViewport` with the new size.
- The page is hidden at construction, becomes visible, and a frame runs -> one `onViewport`.
- Every existing `frame.test.ts` assertion still passes unchanged.

`editor/test/canvas/first-viewport.test.ts` (new; mount-level; imitate the setup of
`editor/test/canvas/mount.test.ts`, including its fake GL/canvas and deps fixtures, by copying
the minimal helpers rather than importing from a test file):

- Mount with the host element rect stubbed to 767x858 (`getBoundingClientRect` on the code host
  element) and dpr 2, then run one frame -> the code canvas backing is `width === 1534 &&
  height === 1716` (or the ledger-capped `effectiveSize` result if a cap applies; assert
  `> 1x1` and equal to `effectiveSize(767, 858, 2, ...)` computed through the exported helper in
  `editor/src/code/resources.ts`).
- Before the fix this test fails with backing 1x1. Record that pre-fix failure under
  `mutationEvidence` (Rule D), not in gating.

## Pitfalls

- Firing `onViewport` synchronously in the constructor, which makes mount call `viewHost` before
  it exists.
- Delivering on every frame, which regresses the redraw-only-when-dirty rule (frame tests count
  `onViewport` calls).
- Editing `mount.ts` to work around the bug. That file belongs to another wave-3 plan.

## Verification (inside the sandbox; logs under `tmp/canvas-cutover/evidence-viewport/logs/`)

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/frame.test.ts test/canvas/first-viewport.test.ts` | all pass |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | all pass; no existing assertion removed |

Mutation evidence (separate list, not gating): the new tests run against the pre-fix `frame.ts`
must fail (expected nonzero exit), recorded with the log path.

Outside the sandbox (wave 4): the Chromium `canvas-only-text` detail reports a canvas backing
larger than 1x1.

## Overwrite and Drift Protocol

Record the sha256 before and after each edit in `intent.json` and `receipt.json`. If a file
drifted before your first edit, stop and report it. Edit only this plan's progress log.

## Completion Criteria

- [x] `frame.ts` delivers the first viewport on the first visible tick and coalesces viewport-triggered text invalidation into that frame
- [x] New `frame.test.ts` rows and `first-viewport.test.ts` pass; pre-fix failure recorded as mutation evidence
- [x] `npm run check` and full vitest pass

## Progress Log

### Session: 2026-10-05 (session 267 plan)
**Tasks Completed**: Plan authored; root cause diagnosed from `frame.ts:51-55,71` and `pixel-diagnostic.log`.

### Session: 2026-10-05 (Step 6 implementation)
**Tasks Completed**: First-visible-tick viewport delivery; change-only subsequent delivery; mount-level high-DPI backing regression; viewport invalidation coalescing.
**Files**: `editor/src/code/frame.ts`, `editor/test/canvas/frame.test.ts`, `editor/test/canvas/first-viewport.test.ts`.
**Mutation Evidence**: `tmp/canvas-cutover/evidence-viewport/logs/mutation-final-tests.log` (pre-fix simulation, exit 1; 5 failed and 8 passed, including unchanged viewport delivery and 2x2 backing failures).
**Final Verification**: Focused Vitest 13/13 passed; `npm run check` exit 0; full Vitest 678/678 passed. Logs are in `tmp/canvas-cutover/evidence-viewport/logs/`.
**Notes**: An initial full Vitest run exposed two redundant-frame assertions after first viewport delivery; fixed by consuming viewport callback text invalidation in that same frame and covered with a scheduler regression row. Formal review, integration review and workflow closeout remain downstream.
