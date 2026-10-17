# Vim Keymap: Renderer Hop Label Overlay Hook Implementation Plan

**Status**: Ready
**Plan ID**: VK-04 (wave 0)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md section 7.4, 12
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Parallel (wave 0)**: VK-00, VK-01, VK-02, VK-03
- **Consumer**: VK-30 passes `hopLabels`/`hopRevision` from the Vim snapshot in `mount.ts`
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

Hop labels must be drawn on the WebGL code pane without changing glyph
geometry. `editor/src/code/renderer.ts` already draws bounded overlay
labels for `binding` annotations in `CanvasRenderer.render` (the loop that
uses `this.palette.labelFill`, `labelRun` and `drawRun` on the
`overlayText` layer). This plan adds one optional feedback field that
reuses exactly that path. The renderer diff must stay about 10 lines.

## Non-Goals

- Do not touch `atlas.ts`, `layout.ts`, glyph shaping, the text or
  background layers, or the cursor code.
- No new palette entries.

## File-Level Changes

### editor/src/code/renderer.ts (shared, narrow)

- `RenderFeedback` gains
  `hopLabels?: readonly { pos: number; text: string }[]; hopRevision?: number;`
  (structural type; do not import from `vim/`).
- Append `|${feedback.hopRevision ?? ''}` to the `overlayKey` template.
- In the overlay pass, after the binding-label loop, when `overlayDirty`:
  for each hop label, `r = this.layout.coordsAtPos(label.pos, this.view)`;
  skip when null; `x = r.left - (this.view.left ?? 0)`,
  `y = r.top - (this.view.top ?? 0)`,
  `width = Math.max(8, label.text.length * 8 + 4)`; draw the fill quad with
  `this.palette.labelFill` over `{left:x, right:x+width, top:y, bottom:y+lineHeight}`,
  then switch to `overlayText`, `drawRun(this.labelRun(label.text, width), x + 2, y, [], rgbaCss(this.palette.labelText))`,
  and restore `overlay`. Unlike binding labels the quad starts at the
  target cell (it covers the glyph), not after it.

Pitfalls: keep the `currentLayer` restore exactly like the binding loop;
when `hopLabels` is absent nothing is drawn and the key only gains an
empty suffix, so the no-hop path draws identical commands.

## Invariants

- With `hopLabels` absent, render output (draw commands) is identical to
  before for the same feedback.
- `renderer.ts` stays under 700 lines (it is 678).

## Test Cases (`editor/test/canvas/renderer-hop.test.ts`, new)

Build the renderer with the same fake-GL harness used in
`editor/test/canvas/gpu.test.ts` (copy its setup helpers locally; do not
edit gpu.test.ts). Use the observable that test uses for command
equality (`it('keeps Text-backed and string-backed renderer commands identical')`).

- feedback without `hopLabels` vs with `hopLabels: []` -> identical commands
- `hopLabels: [{pos: 0, text: 'f'}]`, `hopRevision: 1` -> render returns true and the overlay command list is longer than without labels
- same labels, `hopRevision` 1 then 2 -> the overlay is rebuilt (observable via the overlay command list or stats); same revision twice -> not rebuilt
- label at a position outside the view (`coordsAtPos` null) -> no extra commands, no throw

## Verification (from editor/)

1. Scoped typecheck including `src/code/renderer.ts` and the new test -> exit 0
2. `./node_modules/.bin/vitest run test/canvas/renderer-hop.test.ts test/canvas/gpu.test.ts test/canvas/cursor-shape.test.ts test/canvas/frame.test.ts` -> exit 0, failureCount 0
3. `git diff --stat -- src/code/renderer.ts` -> at most about 15 changed lines (record the number)

## Completion Criteria

- [ ] RenderFeedback has the two optional fields; overlay key includes `hopRevision`
- [ ] At least 4 new passing cases; existing canvas tests pass
- [ ] Diff within the narrow-hook budget
- [ ] Progress Log entry added

## Progress Log

(none yet)
