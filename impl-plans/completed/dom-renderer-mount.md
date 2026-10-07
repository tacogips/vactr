# DOM-RENDERER-MOUNT: Renderer Selection Seam in mount.ts and Perf Hook Implementation Plan

**Status**: Completed (accepted in session 296; archived 2026-10-07)
**Plan ID**: DOM-RENDERER-MOUNT (wave 2 of 4, serial chain)
**Design Reference**: `design-docs/specs/design-dom-renderer.md` DR-1 (seam and perf hook), DR-2 (boundary), DR-8 (mount tests)
**Depends On**: DOM-RENDERER-CORE (`impl-plans/completed/dom-renderer-core.md`)
**Next**: `impl-plans/completed/dom-renderer-harness.md`
**Created**: 2026-10-07
**Last Updated**: 2026-10-07

---

## Intent and Context

`?renderer=dom` (or the mount option `renderer: 'dom'`) must render the editor through
`DomRenderer`. Everything else stays shared: document, input and IME, pointer, view host, frame
scheduler, highlights, syntax, diagnostics, completion and backdrop. Canvas stays the default
and its behavior does not change.

This branch is later merged with `wf/canvas`, which may also touch `mount.ts`. The edit must
therefore be a minimal, additive seam.

Repository facts:

- `editor/src/code/mount.ts:72` creates the canvas. `:76` appends `canvas, inputContainer,
  gpuStatus, diagTip`. `:84-88` constructs `CanvasRenderer` with an `onStatus` closure.
  `:107-114` `placeBackground` inserts the backdrop before `canvas`. `:177-183` passes the canvas
  to `PointerController` and pointer forwarding. `:299-300` and `:339` attach and detach the
  tooltip listeners. `:185` reads `?perf=1` from `win.location.search`.
- `editor/src/code/perf-hook.ts:6,27-29,35,56-59` types the renderer as `CanvasRenderer` and
  exposes `counters()`.
- From DOM-RENDERER-CORE: `selectRendererKind`, `CodeRenderer` and `RendererKind` in
  `editor/src/code/renderer-types.ts`, and `DomRenderer` in `editor/src/code/dom-renderer.ts`
  (root class `vact-code-dom`; `stats.liveLines` and the other fields).

## Non-goals

- No edit to `renderer.ts`, `dom-renderer.ts`, `dom-overlay.ts`, `layout.ts`, `frame.ts`,
  `view-host.ts`, `input.ts`, `pointer.ts`, `app/main.ts` or any canvas test.
- No change to frame callback logic, syntax windows, annotation assembly, highlight ticking,
  eval, diagnostics or transport code in `mount.ts`.
- No UI toggle, no new URL flag other than `renderer`, no new dependency.

## Ownership

writePaths:

- `editor/test/code/dom-mount.test.ts` (new)
- `impl-plans/active/dom-renderer-mount.md` (checkboxes and progress log only)
- `tmp/dom-renderer/mount` (artifact root: logs and receipts)
- `editor/node_modules/.vite` (artifact root: vitest cache)
- `target` (artifact root: the wasm cdylib needed by the full vitest suite)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root: rebuilt only if missing, with
  `mise run ts-build-wasm`)

sharedPaths (shared with the concurrent `wf/canvas` branch; keep the edits minimal):

- `editor/src/code/mount.ts`: the selection seam only (see Contract).
- `editor/src/code/perf-hook.ts`: type widening plus two counters.

## Contract

### `mount.ts` seam (exact scope)

1. Import `DomRenderer` from `./dom-renderer`, and `selectRendererKind`, `type CodeRenderer` and
   `type RendererKind` from `./renderer-types`.
2. `MountOptions` gains `renderer?: RendererKind`.
3. After the `win` check: `const rendererKind = selectRendererKind(win.location.search,
   opts.renderer);` and `codePane.dataset.renderer = rendererKind;`.
4. The presentation element is `surfaceEl: HTMLElement`.
   - Canvas: exactly today's `canvas` element (same class and `aria-hidden`).
   - DOM: `doc.createElement('div')` with class `vact-code-dom` and `aria-hidden="true"`
     (`DomRenderer` also sets these; setting them twice is harmless).
   - Append order stays `surfaceEl, inputContainer, gpuStatus, diagTip`.
5. Extract the existing `onStatus` closure into a `const` and reuse it unchanged.
   - Canvas: `new CanvasRenderer(canvas, layout, { ledger, palette, gl?, createCanvas?,
     onStatus })`, the same arguments in the same order as today.
   - DOM: `new DomRenderer(surfaceEl, layout, { onStatus })`.
   - `renderer` is typed `CodeRenderer`.
   - In DOM mode, `opts.gl` and `opts.createCanvas` are ignored.
6. Replace the identifier `canvas` with `surfaceEl` in `placeBackground` (`insertBefore`),
   `PointerController`, pointer forwarding add and remove, and the two tooltip listeners and
   their removal.
7. The perf hook call passes `rendererKind` (see below).

Nothing else in `mount.ts` changes. The `readPalette` call and `updatePalette` stay; they are
harmless for DOM, because `setPalette` is a no-op store.

### `perf-hook.ts`

- `PerfInputs.renderer: CodeRenderer`. Add `PerfInputs.rendererKind: RendererKind`.
- `VactrPerf.counters()` return type changes:
  - `renderer: CodeRenderer['stats']`
  - `layout: TextLayout['stats']`
  - new `rendererKind: RendererKind`
  - new `domNodes: number`
- `domNodes` is `input.win.document.getElementsByTagName('*').length`, computed on each
  `counters()` call. It is the same definition for both kinds.
- Every existing field keeps its name and value (`gpuStatus`, `textPending`, `ledger`,
  `usedBytes`, and the rest).

## Key Points

- **Canvas byte-identical behavior.** In canvas mode, the DOM tree, class names, append order,
  listener targets and renderer arguments must be exactly as today. Do not create the canvas in
  DOM mode. Measure and behavior scripts query `.vact-code-canvas` for WebGL, so its absence in
  DOM mode is intended.
- **Read the URL once.** Read the URL in `mount()` once, not per frame.
- **Do not add a second `getBoundingClientRect`.** The only per-frame rect read stays
  `CodeViewHost.readRect`.
- **Dispose.** `mounted.dispose()` already calls `renderer.dispose()` and `hostEl.remove()`.
  With the seam, DOM mode leaves no `.vact-code-dom` and a ledger of 0. Do not add renderer-kind
  branches to dispose beyond what the seam needs.
- **Typing pitfalls.**
  - `stats` widens to `Readonly<Record<string, number>>`. Existing tests that read
    `counters().renderer.textBuilds` must still compile (index access gives `number`).
  - Do not narrow `CodeRenderer` to add canvas-only members.
- **Tests and state.** Tests that set `?renderer=dom` through `history.replaceState` must reset
  it in `afterEach` (as `test/canvas/mount.test.ts:88-92` does for `?perf=1`).

## Patterns to imitate

- `editor/test/canvas/mount.test.ts:setup` (rAF host, `installCanvasFakes`, `buildLayout`,
  `Store`, `Client`, `RecordingTransport`, `MockClock`) and its `afterEach` cleanup.
- `editor/src/code/mount.ts:185` for the URL read style.

## Tests (`editor/test/code/dom-mount.test.ts`, jsdom)

- default (no option, URL `/`) -> `.vact-code-canvas` present, `.vact-code-dom` absent,
  `code pane dataset.renderer === 'canvas'`.
- `?renderer=bogus` -> canvas.
- `?renderer=dom` -> `.vact-code-dom` present, no `canvas.vact-code-canvas`,
  `dataset.renderer === 'dom'`.
- `{ renderer: 'dom' }` with URL `/` -> DOM. `{ renderer: 'canvas' }` with `?renderer=dom` ->
  canvas.
- DOM mode, after `surface` dispatch of an insert of `let a 1` and a `host.step(16)` ->
  `.vact-dom-line` text contains `let a 1`.
- DOM mode, pointer down and up on `.vact-code-dom` at the coordinates of column 3 on line 0
  (computed from `layout` geometry, gutter 48, line height 18, 8 px advance fake) ->
  `surface.state.selection.main.head === 3`. Build and dispatch the event the way
  `test/canvas/input.test.ts` `pointerSetup`/`fire` (around line 273) does, including any
  PointerEvent shim that test relies on.
- DOM mode with `?perf=1&renderer=dom` -> `window.__vactrPerf.counters()` has
  `rendererKind === 'dom'`, `domNodes > 0`, `gpuStatus.kind === 'ready'` and
  `textPending === false`.
- Canvas mode with `?perf=1` -> `counters().rendererKind === 'canvas'` and `domNodes > 0`.
- DOM mode: spy on `Element.prototype.getBoundingClientRect`; over 5 frames, each with a typed
  character -> at most 1 call per frame (the view host).
- DOM mode, when `deps.visual.onBackgroundCanvas` is provided -> the backdrop element is
  inserted immediately before `.vact-code-dom`.
- DOM mode dispose -> ledger `usedBytes === 0`, no `.vact-code-dom` in the document, and
  `__vactrPerf` removed.

## Tasks

### TASK-M1: Seam
- [x] `mount.ts` edits limited to Contract items 1-7. `git diff editor/src/code/mount.ts` shows
      no change to frame, annotation, highlight or eval logic.

### TASK-M2: Perf hook
- [x] `perf-hook.ts` widened, with `rendererKind` and `domNodes` added.

### TASK-M3: Tests
- [x] `dom-mount.test.ts` covers every bullet above.

### TASK-M4: Verification and progress log
- [x] Commands below exit 0. Logs are under `tmp/dom-renderer/mount/`. The progress log records
      the sha256 of `mount.ts` and `perf-hook.ts` before and after the edit.

## Verification (implementer, in the sandbox; serial; never two heavy suites at once)

1. `cargo build --lib --target wasm32-unknown-unknown > tmp/dom-renderer/mount/wasm-debug.log 2>&1`
   (repo root) -> exit 0. Needed for the wasm-backed vitest files.
2. `cd editor && npm run check > ../tmp/dom-renderer/mount/check.log 2>&1` -> exit 0.
3. `cd editor && ./node_modules/.bin/vitest run test/code/dom-mount.test.ts test/code/dom-renderer.test.ts test/canvas > ../tmp/dom-renderer/mount/vitest-focused.log 2>&1`
   -> exit 0.
4. `cd editor && ./node_modules/.bin/vitest run > ../tmp/dom-renderer/mount/vitest-full.log 2>&1`
   (the default full suite, run alone) -> exit 0, failureCount 0.
5. `git diff --stat -- editor/src/code/renderer.ts editor/src/code/layout.ts editor/src/code/frame.ts editor/src/code/view-host.ts editor/src/app/main.ts`
   -> empty.

Report records only in the mandatory format, with details in `notes`. Do not run mutation or
negative-control commands. If a load-sensitive unrelated test times out, rerun the full suite
once when the machine is idle. Report only the final passing run, and note the rerun in `notes`.

## Completion Criteria

- [x] All Tests bullets are implemented and passing.
- [x] Verification 1-5 pass.
- [x] Only writePaths and the two sharedPaths changed.

## Progress Log

### Session: 2026-10-07 DOM-RENDERER-MOUNT implementation
**Tasks Completed**: TASK-M1 through TASK-M4; added the renderer selection seam, widened perf counters, and added eight jsdom mount tests.
**Verification**:
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown > tmp/dom-renderer/mount/wasm-debug.log 2>&1` -> exit 0.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/dom-renderer/mount/wasm-host-final.log 2>&1` -> exit 0; this is the required artifact for the wasm-backed Vitest tests.
- `cd editor && npm run check > ../tmp/dom-renderer/mount/check-final.log 2>&1` -> exit 0.
- `cd editor && ./node_modules/.bin/vitest run test/code/dom-mount.test.ts test/code/dom-renderer.test.ts test/canvas > ../tmp/dom-renderer/mount/vitest-focused-final.log 2>&1` -> exit 0, 232/232 tests passed.
- `cd editor && ./node_modules/.bin/vitest run > ../tmp/dom-renderer/mount/vitest-full-final.log 2>&1` -> exit 0, 827/827 tests passed.
- `git diff --stat -- editor/src/code/renderer.ts editor/src/code/layout.ts editor/src/code/frame.ts editor/src/code/view-host.ts editor/src/app/main.ts > tmp/dom-renderer/mount/protected-paths.log` -> exit 0, empty diff.
**Notes**: Source sha256 before/after: `mount.ts` `0ba81ffb2d2fc2975a99e5e21dc0efea9c41eddc771e2b380fd8942684811777` -> `c1ad2136051a0a0ed8460033d7074ff5ee48c89eec882164800366a428d6eece`; `perf-hook.ts` `2f6e38f683f0c9c9547981317ace52d75cd52aa03f4dcbbc0f02e88f519a4549` -> `dc3f4dd25b2bcfd986f878a5d3ae5fe560b1c2bbe903e94ff9aeaeb8e1d4aabe`. Initial focused run `tmp/dom-renderer/mount/vitest-focused.log` had 231/232 passing because the pointer test used an empty document; the test now seeds a line before selecting and final focused run passes. Initial full run `tmp/dom-renderer/mount/vitest-full.log` had 778/827 passing because the default-feature wasm artifact lacked `session_init`; rebuilding with `--no-default-features --features host-wasm` resolved this, and the final full suite passes. The focused final suite includes 8 new mount tests, 9 DOM renderer tests, and 215 canvas tests. No mutation or negative-control command was run. Formal review and final integration are downstream workflow steps.
