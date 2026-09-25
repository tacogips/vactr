# Vactrol Editor: WebGL2 RenderHost Panes and Analyzer Displays (ED-VISUAL) Implementation Plan

**planId**: ED-VISUAL (issue #5, TASK-010, wave 3; the WebGL2 host consuming `0x72` render records with ping-pong
feedback, `TextAsset` rasterization, panes o0..o3, the frame loop, meters and scopes from `levels`)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 15.1.8, 15.1.2 G4/G5, 9.2-9.4, 12.5, 15.1.12
(criteria 3 meters, 11); design-docs/specs/command.md `0x72` and `levels`; design-docs/specs/design-visual.md;
design-docs/user-qa/pending-editor-questions.md E2
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-SCAFFOLD
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

On the browser tier (and in the Tauri webview), the session half emits `0x72` records: `program` (shader source,
uniform names, text assets) at the cycle boundary, and `uniforms` per `session_frame`. This plan renders them in four
output panes with ping-pong framebuffers (design 9.3). It also renders the analyzer displays from `levels`: master
`rms` and `bands`, plus one display per entry of `analyzers`. It provides `VisualApi.mountSpectrum` for the ED-PARAMS
EQ editor.

## Non-Goals

- No native-tier visuals (the native session uses NoopRender). The panes show "visuals: browser tier only".
- No `use-fps`/`use-canvas` handling, no audio input and no `getUserMedia` (E2).
- No shader generation; shader text comes from the core.

## writePaths

- `editor/src/visual/mount.ts` (fills the ED-SCAFFOLD stub), `editor/src/visual/render-host.ts`,
  `editor/src/visual/text-asset.ts`, `editor/src/visual/panes.ts`, `editor/src/visual/frame.ts`,
  `editor/src/visual/meters.ts`, `editor/src/visual/spectrum.ts`, `editor/src/visual/scopes.ts`,
  `editor/src/visual/visual.css`
- `editor/test/support/gl.ts`, `editor/test/support/canvas.ts`
- `editor/test/visual/render-host.test.ts`, `editor/test/visual/text-asset.test.ts`,
  `editor/test/visual/panes.test.ts`, `editor/test/visual/frame.test.ts`, `editor/test/visual/meters.test.ts`,
  `editor/test/visual/scopes.test.ts`
- `impl-plans/active/vactrol-editor-visual.md`

## sharedPaths

None.

## File-Level Changes (behavior and signatures; no code)

1. **`render-host.ts`.** `GlRenderHost(gl: WebGL2RenderingContext, size)`:
   - Per output 0..3: two framebuffer + texture pairs (ping-pong) and the current program.
   - `onRecord(rec)`:
     - `program`: compile the vertex shader (a full-screen triangle) and the fragment `source`, then link. On
       success, swap in the new program and bind its uniforms by name. On compile or link FAILURE, keep the previous
       program and emit a host diagnostic `{code: "shader-compile", out, message: infoLog}` to a listener. An empty
       `source` (the hush/stop empty program) clears the output to black.
     - `uniforms`: store `values` by the program's `uniform_names`.
   - `draw(timeSec)`: for each output with a program:
     - set `time` and `resolution` and the stored uniforms;
     - bind texture units for the samplers the core declares in `source` (`src/tex/shader.rs`). `u_out<k>`
       (k = 0..3) is the PREVIOUS frame texture of output k, which is the feedback for `src ok`. `u_text<id>` is the
       rasterized text asset `id`. Bind a sampler only when `getUniformLocation` returns non-null;
     - render into the other buffer, and swap.
2. **`text-asset.ts`.** Rasterizes `{id, text}` with canvas 2D (fixed font and size, white on transparent) into a
   texture bound at `u_text<id>`. A missing 2D context gives a host diagnostic.
3. **`panes.ts`.**
   - Four canvases o0..o3 blit the current output textures.
   - `render oN` or `render` (tiling) is reflected from the session's display choice when available. Otherwise a pane
     selector shows o0..o3 or the 2x2 tiling.
   - The pane shows a diagnostic banner for `shader-compile` events.
4. **`frame.ts`.** The frame loop per `requestAnimationFrame`:
   - `core.frame(clock.now())` (`session_frame`), then `host.draw(clock.now())`;
   - it stops when the pane is hidden or on dispose;
   - browser tier only.
5. **`meters.ts`, `spectrum.ts`, `scopes.ts`.** Canvas 2D displays from store `levels`:
   - a master level meter (`rms`, dBFS scale);
   - an 8-band spectrum (`bands`);
   - per analyzer entry by `kind`: `level` (meter), `spectrum` (bars), `spectrogram` (rolling image from the ring
     cells), `oscilloscope` (a polyline over the ring cells in ring order, using the next-write-index cell), and
     `pitch-meter` / `stereo-meter` (readouts).
   - Unknown kinds show numeric cells.
   - `mountSpectrum(el, {bus?})` draws a spectrum from the `spectrum` analyzer of that bus when one is published,
     else from the master `bands`.
6. **`mount.ts`.**
   - Builds the panes and the analyzer area, and wires `core` `0x72` records to `GlRenderHost` (browser tier; a
     `getContext('webgl2')` null gives "WebGL2 not available").
   - Subscribes to `levels`.
   - Sets `deps.visual`, which implements `VisualApi`.
7. **`test/support/`.**
   - `gl.ts`: `RecordingGL`, a WebGL2 fake recording `createShader`, `compileShader`, `linkProgram`, `uniform*`,
     `bindFramebuffer`, `bindTexture`, `drawArrays` and `texImage2D`, with a switch to make the next compile fail with
     an info log.
   - `canvas.ts`: a fake 2D context recording `fillText`/`fillRect`/`lineTo`.

## Required Tests

- `render-host.test.ts` (criterion 11):
  - a `program` record for `osc 20 > rotate 0.5 > out o0` (use the golden source shape from `src/tex/tests`, or any
    record with `uniform_names`) compiles, links and draws into o0;
  - successive draws alternate framebuffers (the ping-pong swap is asserted by recorded `bindFramebuffer` targets);
  - a following failing `program` keeps the previous program drawing and emits `shader-compile`;
  - the empty `source` clears to black;
  - `uniforms` values are set by name on the next draw.
- `text-asset.test.ts`: `text "hello"` asset rasterized (`fillText('hello')` recorded) and uploaded (`texImage2D`),
  then bound to the declared sampler.
- `panes.test.ts`: four panes; a selection change; the diagnostic banner; the native tier shows the browser-only
  notice.
- `frame.test.ts`: fake rAF; `core.frame` is called before `draw` each frame; the loop stops on dispose.
- `meters.test.ts` (criterion 3, meters): scripted `levels` with `rms`, `bands` and `analyzers` (`level`, `spectrum`)
  render; a second `levels` repaints only the analyzer displays whose entries changed.
- `scopes.test.ts`: `spectrogram` and `oscilloscope` ring decoding honors the next-write-index cell.
  `mountSpectrum` prefers the bus analyzer over the master bands.

## Invariants

- Visual code never evaluates language code and receives only render-safe data (17, invariant 2).
- No `getUserMedia`, and no audio input.
- Every TS file stays under 800 lines.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-VISUAL`.

## Verification (`<wave>` = `visual`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5, plus:

| # | Command | Evidence |
|---|---------|----------|
| G1 | LOG(`own`): `cd editor && npx vitest run test/visual` | `exit=0`, 6 files passed |

## Completion Criteria

- [ ] Items 1-7 implemented
- [ ] Required tests pass (criterion 11 automated proxy; criterion 3 meters)
- [ ] Common rows and G1 pass with logs cited; `final-hashes.txt` written
- [ ] The manual real-browser pane check is recorded as PENDING USER CONFIRMATION (ED-FINAL records it in the core
      plan)

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-VISUAL implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-wasm.md, vactrol-editor-bind.md
- **Next**: vactrol-editor-params.md
