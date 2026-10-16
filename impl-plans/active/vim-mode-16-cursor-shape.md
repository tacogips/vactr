# Vim Mode: Cursor-Shape Renderer Hook Implementation Plan

**Status**: Ready
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
  returns `base` for bar and underline, and `[r, g, b, a * 0.45]` for
  block.
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
- `cursorColor` alpha is 0.45 times for block and unchanged otherwise.
- Wide grapheme: `next.left - caret.left === 16` -> width 16.

**`editor/test/canvas/gpu.test.ts`**: append one `it` inside the existing
renderer `describe`, using the file's `fixture()` and the `f.r.draws`
records (`rect`, `color`):

- `render({ cursor: 1, textRevision: 1 })` -> the cursor-colored draw
  (`color[0]` about 0.9) has width 1.
- `render({ cursor: 1, cursorShape: 'block', cursorEnd: 2, textRevision: 1 })`
  -> the cursor draw width is greater than 1, and its alpha is about
  0.45.
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

- [ ] `cursor-shape.ts` exports `cursorRect` and `cursorColor`.
- [ ] The `renderer.ts` hook is in place, with at most 12 changed lines.
- [ ] The new tests pass, and the full `gpu.test.ts` passes.

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

### Session: (not started)
