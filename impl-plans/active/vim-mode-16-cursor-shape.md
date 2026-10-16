# Vim Mode: Cursor-Shape Renderer Hook Implementation Plan

**Status**: Completed
**Plan ID**: VIM-16 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md section 3.6, rollout constraint in section 9
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00 (`CursorShape` in `vim/types.ts`)
- **Parallel (wave 1)**: VIM-10..15, VIM-17, VIM-18
- **Consumer**: VIM-30, which passes `cursorShape`/`cursorEnd` from `mount.ts`

## Intent and Context

`editor/src/code/renderer.ts` `CanvasRenderer.render(feedback)` draws the
caret as a 1px quad (lines 259-261):

```ts
const r = this.layout.coordsAtPos(feedback.cursor, this.view); if (r) this.quad(this.local(r), this.palette.cursor);
```

`layout.coordsAtPos` returns `{ left, right: left + 1, top, bottom }`.
`RenderFeedback` is declared at `renderer.ts:15-22`, and the overlay cache
key at line 222 decides whether the overlay layer is rebuilt.

A sibling branch (`wf/code-render`) is editing `renderer.ts` and
`atlas.ts` right now. This plan's diff to `renderer.ts` must therefore be
about 10 lines, touching only the `RenderFeedback` interface, the
`overlayKey` string and the cursor draw. The geometry lives in a new pure
file. The common execution rules in `vim-mode-00-contract.md` apply.

## Non-Goals

- Do not edit `atlas.ts`, `layout.ts`, `palette.ts` or `renderer-types.ts`.
  `CodeRenderer.render` takes `RenderFeedback`, so adding optional fields
  there is enough.
- No new annotation kinds (no search highlight, VQ3).
- Do not reformat or reorder any existing line in `renderer.ts`.

## File-Level Changes

### `editor/src/code/cursor-shape.ts` (new, about 30 lines)

```ts
export function cursorRect(shape: CursorShape, caret: CodeRect, next: CodeRect | null, fallbackWidth = 8): CodeRect
```

- `bar` returns `caret` unchanged (1px wide).
- `block`: `left = caret.left`. `right = next.left` if `next` is non-null,
  `next.top === caret.top` and `next.left > caret.left`; otherwise
  `caret.left + fallbackWidth`. Top and bottom are the caret's.
- `underline`: the same left and right as block, with
  `top = caret.bottom - 2`.
- Also export `cursorColor(shape: CursorShape, base: Rgba): Rgba`. It
  returns `base` for bar and underline, and
  `[r * 0.45, g * 0.45, b * 0.45, a * 0.45]` for block. Block colors are
  premultiplied to match the renderer's `ONE, ONE_MINUS_SRC_ALPHA` blend.
- Use type imports only: `CodeRect` from `../app/apis`, `CursorShape` from
  `./vim/types`, and `Rgba` from `./palette`.

### `editor/src/code/renderer.ts` (minimal diff)

- `RenderFeedback` gains `cursorShape?: CursorShape;` and
  `cursorEnd?: number | null;`.
- In `overlayKey`, append `|${feedback.cursorShape ?? 'bar'}|${feedback.cursorEnd ?? ''}`.
- The cursor draw becomes the following. When the shape is `bar` or
  absent, it calls `quad` with the same rect and color as today, so it is
  pixel-identical.
  - `shape = feedback.cursorShape ?? 'bar'`;
  - `r` as today;
  - `next` is `shape !== 'bar' && feedback.cursorEnd != null ? this.layout.coordsAtPos(feedback.cursorEnd, this.view) : null`;
  - `this.quad(this.local(cursorRect(shape, r, next)), cursorColor(shape, this.palette.cursor))`.
- Import `cursorRect`/`cursorColor` and `type CursorShape`. Nothing else
  changes.

### Tests

**`editor/test/canvas/cursor-shape.test.ts`** (new, at least 8 cases):

- `bar` -> identical object values.
- `block` mid-line with `next` on the same line -> `right === next.left`;
  `next` on a different line (`top` differs) -> width 8; `next` null ->
  width 8; `next.left <= caret.left` -> width 8.
- `underline` -> height 2, at the bottom.
- `cursorColor` returns all four block channels scaled by 0.45 (premultiplied)
  and returns bar/underline colors unchanged.
- Wide grapheme: `next.left - caret.left === 16` -> width 16.

**`editor/test/canvas/gpu.test.ts`**: append one `it` inside the existing
renderer `describe`, using the file's `fixture()` and the `f.r.draws`
records (`rect`, `color`):

- `render({ cursor: 1, textRevision: 1 })` -> the cursor-colored draw
  (`color[0]` about 0.9) has width 1.
- `render({ cursor: 1, cursorShape: 'block', cursorEnd: 2, textRevision: 1 })`
  -> the cursor draw width is greater than 1, with red about 0.405 and alpha
  about 0.45 for the default palette cursor color.
- Changing only `cursorShape` re-renders: the second call returns true
  and the draw differs. This proves the overlay key includes the shape.

Do not modify any existing `it` in `gpu.test.ts`.

## Invariants

- With `cursorShape` absent, every existing `gpu.test.ts` expectation (and,
  in VIM-30, every `mount.test.ts` expectation) passes unchanged, and the cursor quad keeps today's rect and
  color.
- The `renderer.ts` diff is at most 12 changed lines (check with
  `git diff --numstat -- editor/src/code/renderer.ts`).

## Acceptance Criteria

- [x] `cursor-shape.ts` exports `cursorRect` and `cursorColor`.
- [x] The `renderer.ts` hook is in place, with at most 12 changed lines.
- [x] The new tests pass, and the full `gpu.test.ts` passes.

## Verification

1. A scoped typecheck per common rule 5 (`src/code/cursor-shape.ts`,
   `src/code/renderer.ts`, `test/canvas/cursor-shape.test.ts`,
   `test/canvas/gpu.test.ts`):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-16/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/canvas/cursor-shape.test.ts test/canvas/gpu.test.ts`
   must exit 0 with 0 failed. Record the counts. `mount.test.ts` is left
   out on purpose: it imports `input.ts`/`surface.ts`, which VIM-15 edits
   concurrently in this wave. VIM-30 runs it.
3. `git diff --numstat -- src/code/renderer.ts` must show added plus
   removed of at most 12, saved to `tmp/vim-mode/VIM-16/renderer-numstat.log`.

## Progress Log

### Session: 2026-10-16 22:40 Asia/Tokyo

**Tasks Completed**: Cursor geometry/color helpers, minimal renderer feedback hook, focused geometry and GPU renderer tests.

**Implementation Notes**:
- Added pure `cursorRect` and `cursorColor` in `editor/src/code/cursor-shape.ts`.
- Added `cursorShape`/`cursorEnd` feedback and overlay cache-key participation; missing shape remains the existing 1px bar.
- Added 8 geometry/color cases and one GPU test that varies only cursor shape and verifies the overlay draw changes.
- The renderer diff is 7 additions and 2 deletions. All touched code files are under 1000 lines.

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-16/tsconfig.json` — exit 0; `tmp/vim-mode/VIM-16/typecheck-final-2.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/cursor-shape.test.ts test/canvas/gpu.test.ts` — exit 0; 2 files, 71/71 tests passed; `tmp/vim-mode/VIM-16/focused-vitest-final-2.log`.
- `git diff --numstat -- editor/src/code/renderer.ts` — exit 0; 7 additions, 2 deletions; `tmp/vim-mode/VIM-16/renderer-numstat-final.log`.
- `wc -l editor/src/code/cursor-shape.ts editor/src/code/renderer.ts editor/test/canvas/cursor-shape.test.ts editor/test/canvas/gpu.test.ts` — exit 0; longest touched file is 999 lines; `tmp/vim-mode/VIM-16/line-count-final.log`.

### Session: 2026-10-17 01:33 Asia/Tokyo

**Tasks Completed**: Re-ran VIM-16 scoped verification on the current source and refreshed the implementation evidence.

**Implementation Notes**:
- No source changes were needed; the cursor geometry/color helpers, renderer feedback hook, and focused tests already match the plan. `atlas.ts` is unchanged from checkpoint `6349b0ff833c99756874b2a3c9decc44773bdd09`.
- The renderer has no additional diff against that checkpoint; the existing implementation remains within the 12-line plan limit. Current touched source files are all under 1000 lines (largest: `gpu.test.ts`, 999).

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-16/tsconfig.json` — exit 0; `tmp/vim-mode/VIM-16/typecheck-step6-rerun.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/cursor-shape.test.ts test/canvas/gpu.test.ts` — exit 0; 2 files, 71/71 tests passed; `tmp/vim-mode/VIM-16/focused-vitest-step6-rerun.log`.
- `git diff --numstat 6349b0ff833c99756874b2a3c9decc44773bdd09 -- editor/src/code/renderer.ts` — exit 0; no additional checkpoint-relative diff; `tmp/vim-mode/VIM-16/renderer-numstat-step6-rerun.log`.
- `git diff --quiet 6349b0ff833c99756874b2a3c9decc44773bdd09 -- editor/src/code/atlas.ts` — exit 0; atlas unchanged.
- `wc -l editor/src/code/cursor-shape.ts editor/src/code/renderer.ts editor/test/canvas/cursor-shape.test.ts editor/test/canvas/gpu.test.ts` — exit 0; `tmp/vim-mode/VIM-16/line-count-step6-rerun.log`.
- `shasum -a 256` over the four implementation/test files and this plan — exit 0; identities recorded in `tmp/vim-mode/VIM-16/source-sha-step6-rerun.log`.

Formal branch/integration review remains downstream.

### Session: 2026-10-17 03:47 Asia/Tokyo

**Tasks Completed**: Re-ran VIM-16 verification against the shared source; no implementation changes were required.

**Implementation Notes**:
- Current helpers, renderer hook, and focused tests match the assigned contract. No VIM-16 source files are modified in the working tree.
- The VIM-16 renderer change is 7 additions and 2 deletions in commit `84234f8` (9 changed lines total). `atlas.ts` is unchanged in that commit. Touched source files remain below 1000 lines; `gpu.test.ts` is 999 lines.
- A concurrent VIM-15 edit is present in `editor/src/code/input.ts` and `editor/test/canvas/input-vim-hook.test.ts`; it is outside VIM-16 ownership and was preserved.

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-16/tsconfig.json` — exit 0; `tmp/vim-mode/VIM-16/attempt-03/typecheck.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/cursor-shape.test.ts test/canvas/gpu.test.ts` — exit 0; 2 files, 71/71 tests passed; `tmp/vim-mode/VIM-16/attempt-03/focused-vitest.log`.
- `git show --numstat --format= 84234f8 -- editor/src/code/renderer.ts` — exit 0; 7 additions, 2 deletions; total 9 changed lines, within the 12-line limit; `tmp/vim-mode/VIM-16/attempt-03/renderer-numstat.log`.
- `git diff --quiet 84234f8^ 84234f8 -- editor/src/code/atlas.ts` — exit 0; atlas unchanged in the VIM-16 implementation commit.
- `wc -l editor/src/code/cursor-shape.ts editor/src/code/renderer.ts editor/test/canvas/cursor-shape.test.ts editor/test/canvas/gpu.test.ts` — exit 0; `tmp/vim-mode/VIM-16/attempt-03/line-count.log`.
- `shasum -a 256` over the four implementation/test files and this plan — exit 0; identities recorded in `tmp/vim-mode/VIM-16/attempt-03/source-sha.log`.

Formal branch/integration review remains downstream.

### Session: 2026-10-17 Step 6 current-source rerun

**Tasks Completed**: Re-ran the assigned VIM-16 verification on the shared worktree; no implementation source changes were needed.

**Implementation Notes**:
- `cursor-shape.ts`, the renderer hook, and both cursor/GPU test files remain as previously implemented. Concurrent edits in VIM-15 and other plans were preserved and are outside VIM-16 ownership.
- Renderer hook remains 7 additions and 2 deletions in commit `84234f8` (9 changed lines, within the 12-line limit); `atlas.ts` is unchanged in that commit.
- The known Chromium normal-mode IME finding at `editor/src/code/input.ts:44` remains assigned to VIM-15 and VIM-40; this plan does not claim it resolved.

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-16/tsconfig.json` — exit 0; `tmp/vim-mode/VIM-16/attempt-04/typecheck.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/cursor-shape.test.ts test/canvas/gpu.test.ts` — exit 0; 2 files, 71/71 tests passed; `tmp/vim-mode/VIM-16/attempt-04/focused-vitest.log`.
- `git show --numstat --format= 84234f8 -- editor/src/code/renderer.ts` — exit 0; 7 additions and 2 deletions; `tmp/vim-mode/VIM-16/attempt-04/renderer-numstat.log`.
- `git diff --quiet 84234f8^ 84234f8 -- editor/src/code/atlas.ts` — exit 0; unchanged; `tmp/vim-mode/VIM-16/attempt-04/atlas-unchanged.log`.
- `wc -l editor/src/code/cursor-shape.ts editor/src/code/renderer.ts editor/test/canvas/cursor-shape.test.ts editor/test/canvas/gpu.test.ts` — exit 0; 13/673/42/999 lines; `tmp/vim-mode/VIM-16/attempt-04/line-count.log`.
- `shasum -a 256` over the four implementation/test files and this plan — exit 0; identities recorded in `tmp/vim-mode/VIM-16/attempt-04/source-sha.log`.

### Session: 2026-10-17 Step 6 dispatch current-source verification

**Tasks Completed**: Revalidated the assigned VIM-16 helpers, renderer seam, and focused tests against the current shared worktree. No source changes were needed; all three completion criteria remain met.

**Implementation Notes**:
- `cursor-shape.ts`, `renderer.ts`, `cursor-shape.test.ts`, and `gpu.test.ts` match the assigned contract. The renderer commit contains 7 additions and 2 deletions (9 changed lines); `atlas.ts` is unchanged in that commit.
- Existing concurrent plan edits outside VIM-16 were preserved. No VIM-16 code or test files changed in this dispatch.

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-16/tsconfig.json` — exit 0; `tmp/vim-mode/VIM-16/attempt-05/typecheck.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/cursor-shape.test.ts test/canvas/gpu.test.ts` — exit 0; 2 files, 71/71 tests passed; `tmp/vim-mode/VIM-16/attempt-05/focused-vitest.log`.
- `git diff --numstat 84234f8 -- editor/src/code/renderer.ts` — exit 0; no uncommitted renderer drift; `tmp/vim-mode/VIM-16/attempt-05/renderer-numstat.log`.
- `git show --numstat --format= 84234f8 -- editor/src/code/renderer.ts` — exit 0; 7 additions, 2 deletions; `tmp/vim-mode/VIM-16/attempt-05/renderer-commit-numstat.log`.
- `git diff --quiet 84234f8^ 84234f8 -- editor/src/code/atlas.ts` — exit 0; atlas unchanged; `tmp/vim-mode/VIM-16/attempt-05/atlas-unchanged.log`.
- `wc -l editor/src/code/cursor-shape.ts editor/src/code/renderer.ts editor/test/canvas/cursor-shape.test.ts editor/test/canvas/gpu.test.ts` — exit 0; 13/673/42/999 lines; all touched files remain below 1000; `tmp/vim-mode/VIM-16/attempt-05/line-count.log`.
- SHA-256 identities for the four implementation/test files and this plan are recorded in `tmp/vim-mode/VIM-16/attempt-05/source-sha.log`.

Formal branch/integration review remains downstream.

### Session: 2026-10-17 Step 6 repair for VIM-16-F1-block-cursor-premultiply

**Tasks Completed**: Addressed the mid finding from Step 7. `cursorColor('block')` now scales all four channels by 0.45 for the premultiplied renderer blend. Updated the owned unit and GPU assertions; bar and underline remain unchanged. All VIM-16 behavioral criteria pass on this source.

**Implementation Notes**:
- Changed `editor/src/code/cursor-shape.ts`, `editor/test/canvas/cursor-shape.test.ts`, and only the VIM-16 appended case in `editor/test/canvas/gpu.test.ts`; updated this plan's formula and test contract.
- The GPU case selects block red≈0.405 and alpha≈0.45 while preserving width, rect/color difference, and overlay draw-count assertions. The file remains 999 lines. `renderer.ts` and `atlas.ts` have no diff against HEAD.
- Per-edit intentions are recorded as `tmp/vim-mode/VIM-16/attempt-06/edit-intention-01.json` through `edit-intention-07.json`.

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-16/tsconfig.json` — exit 0; `tmp/vim-mode/VIM-16/attempt-06/typecheck.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/cursor-shape.test.ts test/canvas/gpu.test.ts` — exit 0; 2 files, 71/71 tests passed; `tmp/vim-mode/VIM-16/attempt-06/focused-vitest.log`.
- `cd editor && git diff HEAD --numstat -- src/code/renderer.ts src/code/atlas.ts` — exit 0; empty diff; `tmp/vim-mode/VIM-16/attempt-06/renderer-atlas-unchanged.log`.
- `wc -l editor/src/code/cursor-shape.ts editor/test/canvas/cursor-shape.test.ts editor/test/canvas/gpu.test.ts` — exit 0; 13/42/999 lines; `tmp/vim-mode/VIM-16/attempt-06/line-count.log`.
- SHA-256 identities for the four owned implementation/test files are recorded in `tmp/vim-mode/VIM-16/attempt-06/source-sha.log`.

Independent Step 7 re-review is required for acceptance.
