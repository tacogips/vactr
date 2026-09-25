# Vactrol Editor: WebGL2 RenderHost Panes and Analyzer Displays (ED-VISUAL) Implementation Plan

**planId**: ED-VISUAL (issue #5, TASK-010, wave 3; the WebGL2 host consuming `0x72` render records with ping-pong
feedback, `TextAsset` rasterization, panes o0..o3, the frame loop, meters and scopes from `levels`)
**Status**: Completed (implemented, gate-verified, adversarial review and integration review accepted in session 186; removed from the dispatch manifest by the session-187 amendment; source rides in the single workflow commit)
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

- [x] Items 1-7 implemented
- [x] Required tests pass (criterion 11 automated proxy; criterion 3 meters)
- [x] Common rows and G1 pass with logs cited; `final-hashes.txt` written
- [x] The manual real-browser pane check is recorded as PENDING USER CONFIRMATION (ED-FINAL records it in the core
      plan)

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-VISUAL implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 186, ED-VISUAL implementer)

**Tasks completed**: items 1-7. Evidence: `tmp/ed-editor-20260926-s186/ED-VISUAL/attempt-1/` (intent.md,
pre/post-edit-hashes.txt, notes.md, final-hashes.txt).

- `render-host.ts` `GlRenderHost`: ping-pong framebuffer/texture pairs per output o0..o3; full-screen-triangle vertex
  shader; `program` compile+link with `shader-compile` diagnostics that keep the previous program; the empty source
  releases the program and clears both buffers to black; `uniforms` set by `uniform_names` on the next draw (values
  arriving after a FAILED program are not applied to the previous one, since `uNN` names are positional); samplers
  `u_out<k>` (output k's PREVIOUS frame, snapshotted before any output draws) and `u_text<id>` bound only when
  `getUniformLocation` is non-null; `present(out)` blits into the default framebuffer for the panes.
- `text-asset.ts`: canvas 2D rasterization (fixed `bold 64px monospace`, white on transparent, 512x128, flip-Y
  upload); no 2D context gives a `text-asset` host diagnostic (emitted after the program-accept event).
- `panes.ts` `VisualPanes`: four 2D canvases o0..o3 drawn from the host's canvas; selector o0..o3 / tile (the session
  publishes no display choice in v1); a diagnostic banner cleared when the output accepts a program; notice mode.
- `frame.ts`: rAF loop `core.frame(now)` then `host.draw(now)` then present; pauses on `visibilitychange` hidden;
  stops on dispose or on a thrown frame (reported in the banner as `frame-loop`).
- `meters.ts` `AnalyzerArea`, `spectrum.ts`, `scopes.ts`: master rms meter (dBFS, -60 floor) and 8 bands; per
  analyzer kind `level`, `spectrum`, `spectrogram`/`note-spectrogram` (rolling image), `oscilloscope` (polyline),
  `pitch-meter`/`stereo-meter` readouts, unknown kinds numeric; repaint only changed displays; entries no longer
  published are removed. `mountSpectrum` prefers the bus `spectrum` analyzer, else the master bands.
- `mount.ts`: analyzers on both tiers; native tier "visuals: browser tier only"; null WebGL2 "WebGL2 not available";
  wires `core.onRender` to the host; sets and clears `deps.visual`. `visual.css` loads through
  `new URL('./visual.css', import.meta.url)` plus a `<link>` (the ED-MIDI TS2882 precedent).
- Ring index semantics follow `src/dsp/effects/analyzer.rs`: the spectrogram kinds' last cell is the index of the
  frame written MOST RECENTLY (`ring_slot`), the oscilloscope's is the NEXT write position (`scope`).

**Verification** (logs under `target/fe-logs/`, all with `exit=`):
- V1 `ed-visual-build-s186-1.log` exit=0; V2 `ed-visual-clippy-s186-1.log` exit=0.
- V3 `ed-visual-nextest-s186-1.log` exit=0 (985 run, 985 passed, 1 skipped).
- V3t `ed-visual-cargotest-s186-1.log` exit=0 (985 passed, 0 failed over 7 result lines).
- V7 `ed-visual-fmt-s186-1.log` exit=0; V6a `ed-visual-wasm32-s186-1.log` exit=0; V6b
  `ed-visual-wasm32-hostwasm-s186-1.log` exit=0; V6c `target/ed-wasm/ED-VISUAL.wasm` copied.
- V4 largest `.rs` 799 lines (src/dsp/build.rs); E0 node v26.9.0, npm 11.19.1.
- E1 `ed-visual-npm-ls-s186-1.log` exit=0.
- E2 `ed-visual-npm-check-s186-3.log` exit=0. Runs -1 and -2 exited 1 ONLY on the sibling ED-CODE in-flight file
  `test/code/reconcile.test.ts(37,18)` (TS2741); a scoped check over `src/`, `test/visual`, `test/support`
  (`ed-visual-tsc-scoped-s186-1.log`) exited 0, and run -3 passed once the sibling settled.
- E3 `ed-visual-npm-test-s186-2.log` exit=0 (32 files, 182 tests passed); run -1 exit=0 (31 files, 174 tests).
- E4 `ed-visual-npm-build-s186-1.log` exit=0 into `target/ed-dist/ED-VISUAL`; E4c exit 0; `visual.css` is inlined as
  a data URL asset.
- E5 largest `.ts` 446 lines (src/protocol/types.ts); this plan's largest is render-host.ts at 341.
- G1 `ed-visual-own-s186-1.log` exit=0 (6 files, 28 tests passed).

**Pending user confirmation**: the real-browser visual pane check (a WebGL2 page showing `osc 20 > rotate 0.5 > out
o0`, feedback via `src o0`, a text source, tiling, and hush clearing to black). ED-FINAL records it in the core plan.
The automated proxies are render-host.test.ts, text-asset.test.ts and panes.test.ts.

**Notes for ED-FINAL**: the session does not publish the `render oN`/`render` display choice in v1, so the pane
selector is the only path; `analyzer.rs`'s module doc says "next write index in the last cell" for every ring kind,
but `ring_slot` stores the most recently written frame (the code is followed).

### CLOSING NOTE (ED-FINAL, session 188)

Status confirmed Completed (accepted). Final-tree evidence (ED-FINAL attempt-2, `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/`): join integrity re-checked (this plan's hashes OK or explained); every row exit=0 in `target/fe-logs/ed-final-*-s188-1.log`: build, build-lsp, clippy, clippy-lsp, fmt, nextest (1007 passed, 1 skipped), cargo test, both wasm32 builds, clippy wasm32 host-wasm, npm ci/check/test (54 files, 336 tests)/build (`VACTROL_REQUIRE_SESSION_ABI=1`), real-wasm vitest (3 files, 20 tests), Tauri fetch/check/fmt, session subset, lsp_smoke, spec fixtures. Own evidence: `test/visual/*` (6 files) in target/fe-logs/ed-final-npm-test-verbose-s188-1.log; `criteria.test.ts` criterion 11 drives `GlRenderHost` from the real artifact. The real-browser visual pane is PENDING USER CONFIRMATION.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-wasm.md, vactrol-editor-bind.md
- **Next**: vactrol-editor-params.md
