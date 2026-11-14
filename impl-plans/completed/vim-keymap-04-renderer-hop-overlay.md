# Vim Keymap: Renderer Hop Label Overlay Hook Implementation Plan

**Status**: Completed
**Plan ID**: VK-04 (wave 0)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md section 7.4, 12
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Parallel (wave 0)**: VK-00, VK-01, VK-02, VK-03
- **Consumer**: VK-30 passes `hopLabels`/`hopRevision` from the Vim snapshot in `mount.ts`
- Common Execution Rules: `impl-plans/completed/vim-keymap-00-contract.md` (apply in full)

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

- [x] RenderFeedback has the two optional fields; overlay key includes `hopRevision`
- [x] At least 4 new passing cases; existing canvas tests pass
- [x] Diff within the narrow-hook budget
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-04 implementation

**Tasks Completed**: Renderer hop overlay hook, four dedicated renderer tests,
scoped typecheck and focused canvas verification.

**Files and identities**:
- `editor/src/code/renderer.ts`: `38162d3b855a2d2d09de8d1a4d926b106639d74c`; 685 lines.
- `editor/test/canvas/renderer-hop.test.ts`: `a1985cc5ba1dff68b0445e1ad4bb5063c0cf48aa`.
- `impl-plans/active/vim-keymap-04-renderer-hop-overlay.md` before this entry:
  `347e74bf695d6e2c0f26540b716f1a1b373013df`.
- `git diff --stat -- src/code/renderer.ts`: 8 insertions, 1 deletion (9 changed lines).

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-04/tsconfig.json", exitStatus: 0, outcome: "passed", log: "tmp/vim-keymap/VK-04/attempt-3-typecheck.log"}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/renderer-hop.test.ts test/canvas/gpu.test.ts test/canvas/cursor-shape.test.ts test/canvas/frame.test.ts", exitStatus: 0, testsRun: 91, testsPassed: 91, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-04/attempt-3-canvas-tests.log"}`
- The scoped typecheck first reported TS7053 in the fake GL proxy; the test target was typed as `Record<string, unknown>` and both required gates passed on the final source. Initial wrapper logs were incomplete due to zsh's read-only `status` name and are not verification evidence.

**Notes**: `hopRevision` invalidates the overlay, labels render through the existing fill/text overlay path, absent and empty labels preserve draw counts, unchanged revisions reuse the overlay, and null positions are skipped. Review and serial finalization remain downstream workflow steps.

### Session: 2026-10-17 VK-04 author self-check

The tests were strengthened to observe uploads to the overlay and overlayText
buffers directly. The following source identity and verification supersede the
attempt-3 evidence above; the earlier logs remain preserved.

- Final renderer identity: `38162d3b855a2d2d09de8d1a4d926b106639d74c` (685 lines).
- Final test identity: `bfefe97af10fcc73ad3ab7bc30f73aa6242ca01b`.
- Final plan identity before this follow-up: `67e54a08721d755d5d96cc9e2ed50fcac3aab24f`.
- `git diff --stat -- src/code/renderer.ts`: 8 insertions, 1 deletion; `git diff --check -- src/code/renderer.ts test/canvas/renderer-hop.test.ts` exited 0.
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-04/tsconfig.json", exitStatus: 0, outcome: "passed", log: "tmp/vim-keymap/VK-04/attempt-4-typecheck.log"}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/renderer-hop.test.ts test/canvas/gpu.test.ts test/canvas/cursor-shape.test.ts test/canvas/frame.test.ts", exitStatus: 0, testsRun: 91, testsPassed: 91, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-04/attempt-4-canvas-tests.log"}`

### Session: 2026-10-17 VK-04 independent review repair

Read-only reviewer `/root/vk04_review` found that the previous fixture
compared instance counts rather than command geometry. The fixture now records
each emitted layer and each instance rectangle/color, compares full draw
commands for absent versus empty labels, and asserts that a visible label
emits both fill (`overlay`) and text (`overlayText`) commands. This is within
VK-04's test ownership. The prior mapper finding remains outside this renderer
plan; the accepted design assigns its routing behavior to the mapper/controller
implementation plans.

- Final renderer identity remains `38162d3b855a2d2d09de8d1a4d926b106639d74c`.
- Final test identity: `51979ff3a0293a0a9bcca6bb481813f34226ebd7`.
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-04/tsconfig.json", exitStatus: 0, outcome: "passed", log: "tmp/vim-keymap/VK-04/attempt-5-typecheck.log"}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/renderer-hop.test.ts test/canvas/gpu.test.ts test/canvas/cursor-shape.test.ts test/canvas/frame.test.ts", exitStatus: 0, testsRun: 91, testsPassed: 91, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-04/attempt-5-canvas-tests.log"}`
