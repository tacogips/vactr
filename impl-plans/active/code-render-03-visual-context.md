# Code Render 03: Visuals Context Without MSAA (RC6)

**Status**: Ready
**Plan ID**: CR-03
**Wave**: 1 (no dependencies)
**Design Reference**: design-docs/specs/design-code-pane-text-rendering.md sections 3 (RC6), 4.6 (R12), 5.1 (U6)
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

The console repeats `GL_INVALID_OPERATION: glBlitFramebuffer: Invalid
operation on multisampled framebuffer` from
`editor/src/visual/render-host.ts:226` (`GlRenderHost.present`).
`editor/src/visual/mount.ts:71` creates the visuals context with
`glCanvas.getContext('webgl2')`, which defaults to `antialias: true`, so
the default framebuffer is multisampled. WebGL2/GLES 3.0 forbids blitting
into a draw framebuffer with `SAMPLE_BUFFERS > 0`. Source and destination
rectangles already match (`RENDER_SIZE`).

## Non-goals

- Do not change `render-host.ts` (the blit stays), `main.ts` (its
  `getContext('webgl2')` probe is a separate canvas), the panes, or
  `RENDER_SIZE`.
- No change to any code-pane file.

## Files

| Path | Change |
| --- | --- |
| `editor/src/visual/mount.ts` | Line 71: `glCanvas.getContext('webgl2', { antialias: false })`. Optionally update the header comment's mention of `getContext('webgl2')`. Nothing else. |
| `editor/test/visual/panes.test.ts` | Add one test in `describe('visual mount')` (U6). |
| `impl-plans/active/code-render-03-visual-context.md` | Progress log only. |

Imitate `panes.test.ts` 'reports WebGL2 not available' (lines 178-186):
`installCanvasFakes()` from `test/support/canvas.ts` spies on
`HTMLCanvasElement.prototype.getContext` (`vi.spyOn(...).mockImplementation`).

## Key Points and Pitfalls

- Pass the options object to the visuals context only. Do not add
  `preserveDrawingBuffer` or other options.
- The TypeScript overload for `'webgl2'` accepts
  `WebGLContextAttributes`; the return type stays
  `WebGL2RenderingContext | null`.
- The test must read the arguments of the spied `getContext` calls whose
  first argument is `'webgl2'`, without loosening `installCanvasFakes`.

## Test Cases

- Browser tier with a fake core (`deps('browser', core.as())`), mount ->
  the spied `getContext` calls include `['webgl2', expect.objectContaining({
  antialias: false })]`, and no `'webgl2'` call from the visual mount
  lacks the options object.
- The existing 'reports WebGL2 not available' and routing tests still pass.

The new test fails on ea51a07 (called with one argument). CR-07 records
that as mutationEvidence.

## Invariants

- `mount.ts` stays at about 131 lines; `panes.test.ts` stays under 260.

## Write Scope

Mirrors `impl-plans/active/code-render-dispatch.json` plans[CR-03].

- writePaths: `editor/src/visual/mount.ts`, `editor/test/visual/panes.test.ts`, `impl-plans/active/code-render-03-visual-context.md`, `tmp/code-render/CR-03`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`
- sharedPaths: none
- artifactRoots: `tmp/code-render/CR-03`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules` (the last three are prebuilt read-only inputs: never rebuild, reinstall or delete them)
- Toolchain audit: before the first edit, from the repository root run
  `shasum -a 256 target/wasm32-unknown-unknown/debug/vactr.wasm tree-sitter-vact/tree-sitter-vact.wasm editor/package-lock.json > tmp/code-render/CR-03/toolchain-sha256.txt`
  and copy the three digests into the Progress Log.

## Working Rules

Same as CR-01 (fresh read plus sha256 per edit, no git writes, foreground
only, cross-plan check errors recorded). Logs go under
`tmp/code-render/CR-03/`.

## Verification

Run from `editor/`.

1. `npx vitest run test/visual/panes.test.ts test/visual/render-host.test.ts > ../tmp/code-render/CR-03/focused.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-03/focused.log` -> exit 0.
2. `npm run check > ../tmp/code-render/CR-03/check.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-03/check.log` -> exit 0 (or only cross-plan errors recorded).

The browser-level proof (no blit error in the console) is the CR-06
pixel test check P8, run in CR-07.

## Completion Criteria

- [ ] Visuals context requested with `{ antialias: false }`
- [ ] U6 test added and passing
- [ ] Progress log updated with verification records

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Ready for wave 1.
