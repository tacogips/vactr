# Code Render 04: Glyph Snap Flag, Shader Snap and Premultiply, Placement Helpers (R6-R8, R10)

**Status**: Completed
**Plan ID**: CR-04
**Wave**: 1 (no dependencies)
**Design Reference**: design-docs/specs/design-code-pane-text-rendering.md sections 3 (RC4, RC5), 4.4 (R6, R7, R8), 4.5 (R10), 5.1 (U4, U5 shader part)
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

Text looks heavy because the fragment shader emits straight alpha under a
premultiplied blend (`ONE, ONE_MINUS_SRC_ALPHA`, premultiplied canvas). It
looks soft because glyph quads sit at fractional device pixels and are
rescaled against their cells. This plan adds the shader-side rules and
pure placement helpers in `editor/src/code/geometry.ts`. CR-05 wires them
into the renderer.

Contracts pinned for CR-05 (all exported from `geometry.ts`):

- `export const SNAP_GLYPH = 16;` (instance flag bit; next free bit after
  `NO_HSCROLL = 8`).
- Vertex shader uniform `uniform vec2 u_device;`: device pixels per CSS
  pixel, `(canvas.width / view.width, canvas.height / view.height)`.
  CR-05 sets it every frame.
- `export function glyphPhase(cssX: number, scale: number): { devicePixel: number; phase: number }`
- `export function glyphQuad(devicePixel: number, scale: number, cell: { width: number; height: number; pad: number }): { left: number; width: number; height: number }`
  (CSS px, relative to the same origin as `cssX`).

## Non-goals

- No change to `InstanceWriter`, `LayerBuffer`, the 32-byte instance
  format, `CLIP_GUTTER`/`UNTINTED` semantics, or blend state.
- Do not touch `renderer.ts`, `atlas.ts`, `layout.ts`, or
  `editor/test/canvas/gpu.test.ts`.
- No gamma or contrast terms in the shader.

## Files

| Path | Change |
| --- | --- |
| `editor/src/code/geometry.ts` | Add `SNAP_GLYPH`, `glyphPhase`, `glyphQuad`; edit `VERTEX_SOURCE` and `FRAGMENT_SOURCE` as below. |
| `editor/test/code/glyph-placement.test.ts` | New unit test file (U4 and the shader part of U5). |
| `impl-plans/active/code-render-04-shader-placement.md` | Progress log only. |

### Helper semantics

- `glyphPhase`: `d = cssX * scale`; `base = Math.floor(d)`; `q =
  Math.round((d - base) * 4) / 4`. If `q === 1`, return `{ devicePixel:
  base + 1, phase: 0 }`, else `{ devicePixel: base, phase: q }`. Throw
  `RangeError` when `scale` is not finite or not positive, or `cssX` is
  not finite.
- `glyphQuad`: `left = (devicePixel - cell.pad) / scale`, `width =
  cell.width / scale`, `height = cell.height / scale`. Same `RangeError`
  rule for `scale`.

### Shader changes

- Vertex: declare `uniform vec2 u_device;`. Compute `origin = a_rect.xy +
  slot`. For flagged instances, use the one-line idiom
  `if ((a_meta.y & 16u) != 0u) origin = floor(origin * u_device + 0.5) / u_device;`.
  Then `p = origin + a_corner * a_rect.zw`. `v_screen_x`, `v_uv`,
  `v_color`, `v_flags` and the clip-space mapping stay as they are.
- Fragment: replace the final assignment with premultiplied output:
  `c = untinted ? texel : vec4(v_color.rgb, v_color.a * texel.a)`;
  `out_color = vec4(c.rgb * c.a, c.a)`. Keep the `CLIP_GUTTER` discard
  first.

## Key Points and Pitfalls

- Only flagged instances snap. Backgrounds, caret, underlines and handles
  stay unflagged and unsnapped (design R8).
- The quad size is NOT snapped; it is already device-integral
  (`cell.width / scale`). Rounding the size would rescale the texture.
- Mask cells: use only `texel.a` (coverage). Their rgb is white straight
  alpha and must not multiply the tint.
- `u_device` defaults to `(0, 0)` until CR-05 sets it. Flagged instances
  would then divide by zero. That is acceptable only because no instance
  carries `SNAP_GLYPH` before CR-05; do not set the flag anywhere in this
  plan.
- Interim effect: R10 changes how translucent solid quads composite until
  CR-05 converts the renderer's literal colors (R11). Do not touch
  `renderer.ts` to compensate.
- Floating-point carry: `glyphPhase(3.9999999, 1)` must return
  `{ devicePixel: 4, phase: 0 }`.

## Test Cases (`editor/test/code/glyph-placement.test.ts`)

- `glyphPhase(10.3, 1)` -> `{ devicePixel: 10, phase: 0.25 }`
- `glyphPhase(10.9, 1)` -> `{ devicePixel: 11, phase: 0 }`
- `glyphPhase(7.8, 2)` -> `{ devicePixel: 15, phase: 0.5 }`
- `glyphPhase(0, 1)` -> `{ devicePixel: 0, phase: 0 }`
- `glyphPhase(3.9999999, 1)` -> `{ devicePixel: 4, phase: 0 }`
- `glyphPhase(5.2, 1.5)` -> `devicePixel` is an integer and `phase` is one
  of 0, 0.25, 0.5, 0.75, with `devicePixel + phase` within 0.125 of 7.8
- `glyphPhase(1, 0)`, `glyphPhase(1, NaN)`, `glyphPhase(NaN, 1)` -> throw
  `RangeError`
- `glyphQuad(15, 2, { width: 21, height: 36, pad: 4 })` -> `{ left: 5.5,
  width: 10.5, height: 18 }`; `left * 2` and `width * 2` are integers
- `glyphQuad` at scale 1, 1.5 and 2 -> `width * scale` equals
  `cell.width` and `(left * scale) + cell.pad` equals `devicePixel`, each
  within 1e-9
- `SNAP_GLYPH === 16`, and it does not overlap `CLIP_GUTTER`, `UNTINTED`,
  `TEXT_SPACE` or `NO_HSCROLL`
- `VERTEX_SOURCE` matches `/uniform vec2 u_device;/` and `/& 16u/` and
  contains `floor(`; `FRAGMENT_SOURCE` matches `/\.rgb \* \w+\.a/` and no
  longer contains `texel * v_color`

These fail on ea51a07 (missing exports and the old shader). CR-07
records that as mutationEvidence.

## Invariants

- The shaders still compile under GLSL ES 3.00. `a_meta` is `uvec2`, so
  bitwise `&` with `16u` is valid. `precision mediump float` in the
  fragment shader stays.
- `geometry.ts` stays under 200 lines.

## Write Scope

Mirrors `impl-plans/active/code-render-dispatch.json` plans[CR-04].

- writePaths: `editor/src/code/geometry.ts`, `editor/test/code/glyph-placement.test.ts`, `impl-plans/active/code-render-04-shader-placement.md`, `tmp/code-render/CR-04`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`
- sharedPaths: none
- artifactRoots: `tmp/code-render/CR-04`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules` (the last three are prebuilt read-only inputs: never rebuild, reinstall or delete them)
- Toolchain audit: before the first edit, from the repository root run
  `shasum -a 256 target/wasm32-unknown-unknown/debug/vactr.wasm tree-sitter-vact/tree-sitter-vact.wasm editor/package-lock.json > tmp/code-render/CR-04/toolchain-sha256.txt`
  and copy the three digests into the Progress Log.

## Working Rules

Same as CR-01 (fresh read plus sha256 per edit, no git writes, foreground
only, cross-plan check errors recorded). Logs go under
`tmp/code-render/CR-04/`.

## Verification

Run from `editor/`.

1. `npx vitest run test/code/glyph-placement.test.ts > ../tmp/code-render/CR-04/focused.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-04/focused.log` -> exit 0.
2. `npm run check > ../tmp/code-render/CR-04/check.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-04/check.log` -> exit 0.

The optional `npx vitest run test/canvas/gpu.test.ts` run is informational and
CR-05-owned; it is not a CR-04 gating verification. Record any such result only
as a CR-05 handoff note, not as a CR-04 completion criterion.

Real-GPU shader compilation is proven by the CR-06 pixel test in CR-07.

## Completion Criteria

- [x] `SNAP_GLYPH`, `glyphPhase`, `glyphQuad` exported with the pinned signatures
- [x] Vertex shader snaps flagged instance origins using `u_device`
- [x] Fragment shader emits premultiplied color
- [x] `glyph-placement.test.ts` passes
- [x] Progress log updated

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Ready for wave 1.

### Session: 2026-10-16
**Tasks Completed**: CR-04 placement helpers and shader contract; focused regression coverage; plan progress update.
**Tasks In Progress**: None for CR-04.
**Blockers**: None for the assigned contract.
**Notes**: `editor/src/code/geometry.ts` exports `SNAP_GLYPH`, `glyphPhase` and `glyphQuad`; vertex snapping is gated by bit 16 and `u_device`; fragment output is premultiplied. No instances receive `SNAP_GLYPH`, and `renderer.ts` remains untouched for CR-05. `geometry.ts` is 158 lines. The prebuilt input digests are recorded in `tmp/code-render/CR-04/toolchain-sha256.txt`: `target/wasm32-unknown-unknown/debug/vactr.wasm` `de9c55f37b4037d73e28f8aaa11e06d3bc87a3a41a3bc95dd8e64ebf02c93210`; `tree-sitter-vact/tree-sitter-vact.wasm` `ae3e5cdf1217e943f19c3ff15f111801e537ba9b7e463f84d6bf8cedd7fd0609`; `editor/package-lock.json` `f32e9585c409487e0d74ee9b8dc1880d4e54ff8610ab2cefa529828c53449749`.
**Verification**: Focused `npx vitest run test/code/glyph-placement.test.ts` passed 6/6 (`tmp/code-render/CR-04/focused.log`). `npm run check` exited 1 only for a TypeScript declaration error on the concurrent, out-of-scope `test/e2e/pixel-compare.mjs` import (`check.log`). Informational `npx vitest run test/canvas/gpu.test.ts` ran 62, passed 61; its existing upload-count expectation received 34 vs 15 after CR-02's atlas key now distinguishes rasterized cell widths (`canvas.log`). Hand off this changed expectation to CR-05 integration; do not change the excluded `gpu.test.ts` here. The first focused run's wrapper hit zsh's read-only `status` variable after Vitest passed; its incomplete log is preserved as `focused-attempt-1.log`, and the complete rerun is the verification record.

### Session: 2026-10-16 (INT-377-03 evidence repair)
**Tasks Completed**: Re-ran the assigned focused suite and combined TypeScript check; corrected the CR-04 verification record for review.
**Tasks In Progress**: None for the assigned CR-04 contract.
**Blockers**: None for the assigned CR-04 contract.
**Notes**: Current-source `npx vitest run test/code/glyph-placement.test.ts` passed 6/6 (`tmp/code-render/CR-04/attempt-2/focused.log`). Current-source `npm run check` exited 0 (`tmp/code-render/CR-04/attempt-2/check.log`); the concurrent CR-06 declaration issue is resolved in the combined tree. The prior `test/canvas/gpu.test.ts` result remains an informational CR-05 handoff: 61/62 passed, with the atlas upload-count assertion observing 34 instead of 15; this plan did not edit `gpu.test.ts`. Real-GPU shader compilation remains owned by CR-06/CR-07. `renderer.ts` remains unchanged. Evidence intention: `tmp/code-render/CR-04/attempt-2/edit-intent.txt`.

### Session: 2026-10-17 (INT-377-03 current-source evidence)
**Tasks Completed**: Re-ran CR-04's two assigned gating checks on current source and clarified that the CR-05 canvas suite is informational rather than a CR-04 gate.
**Tasks In Progress**: None for the CR-04 implementation contract.
**Blockers**: None for implementation; independent review remains downstream.
**Notes**: `editor/src/code/geometry.ts` still exports `SNAP_GLYPH`, `glyphPhase` and `glyphQuad`; vertex origin snapping is guarded by bit 16 and `u_device`; fragment output is premultiplied. CR-04 did not set `SNAP_GLYPH` on instances or edit `renderer.ts`; the current integrated branch includes CR-05-owned renderer wiring that sets the flag on text instances. Current-source focused verification passed 6/6 at `tmp/code-render/CR-04/attempt-3-focused.log`; `npm run check` exited 0 at `tmp/code-render/CR-04/attempt-3-check.log`. Both are CR-04 gating checks. Canvas GPU verification is CR-05-owned and is explicitly informational under the dispatch contract. Real-GPU shader compilation remains owned by CR-06/CR-07. Per-edit intents: `tmp/code-render/CR-04/attempt-3-edit-intent.txt` and `tmp/code-render/CR-04/attempt-3-edit-intent-02.txt`.

### Session: 2026-10-17 (CR-04 current-source implementation handoff)
**Tasks Completed**: Rechecked the assigned helpers and shaders; reran both CR-04 gates and refreshed toolchain digests.
**Tasks In Progress**: None for CR-04.
**Blockers**: None.
**Notes**: Current `geometry.ts` satisfies the pinned signatures, bit-16 `u_device` origin snap, and premultiplied fragment output. The focused suite passed 6/6 in `tmp/code-render/CR-04/attempt-4/focused.log`; `npm run check` exited 0 in `tmp/code-render/CR-04/attempt-4/check.log`. Toolchain digests are in `tmp/code-render/CR-04/attempt-4/toolchain-sha256.txt`. CR-04 source/test files are already present in the worktree; this session made no implementation-source changes. Renderer wiring remains owned by CR-05.
