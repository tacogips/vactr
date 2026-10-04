# Canvas Cutover: Production Mount and Consumer Migration Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-MOUNT (wave 2; depends on CANVAS-CLOCK and CANVAS-RENDER)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.2 (frozen contract and every consumer rule), 15.3.8.3 (mount composition), 15.3.8.7 (whole-text rules), 15.3.8.9
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

Production editing must switch from CodeMirror `EditorView` to the headless `CodeSurface`
authority, rendered on the WebGL2 canvas.

- All source text, line numbers, cursor, selection, syntax, diagnostics, playing and eval
  feedback, binding labels and call-head marks are drawn by `CanvasRenderer`.
- The DOM keeps only the textarea input and accessibility bridge, panels, the completion popup,
  the diagnostic tooltip and status.
- Removing `CodeApi.view` breaks every consumer at once. That is why the code cutover, bind,
  params and song ports, and the import guard are all in one plan (design 15.3.8.9). The suite
  must be green when this plan ends.

Inputs from wave 1:

- CANVAS-CLOCK: `deps.audible`, and `HighlightScheduler`/`TransportBar` audible options.
- CANVAS-RENDER: `FrameScheduler`, `PerfRecorder`, `RenderFeedback.textRevision/animated/cursorVisible`,
  `renderer.textPending`, and `CodeAnnotation.kind 'call-head'`.

Read those plans' pinned contracts first. Do not redesign them.

Facts at c9e5a05:

- 15 production files import `@codemirror/view` (the list is in design 15.3.8.1).
- `CodeApi.view` users: `app/song.ts:78,127`, `bind/mount.ts:79-337`,
  `params/mount.ts:186,259,305,314` and `params/roll.ts:93`.
- `code/mount.ts` builds an `EditorView` with `sync.extension()`, `vactLanguage()`,
  `lineNumbers`, `lintGutter`, `highlightExtension`, `evalCtl.extension()` and `formatKeymap`.

## Non-goals

- No change to `app/main.ts` or `test/app/main.test.ts`. CANVAS-SHELL owns them in this wave.
- No `visual/*` change (CANVAS-VISUAL). No Rust change.
- No `package.json` or lockfile change. Do not remove the unused `@codemirror/view`, `lint` or
  `language` declarations.
- No `@codemirror/autocomplete`, no hidden `EditorView`, and no visible DOM source fallback.
- No change to the completion, format or tree-sitter cores beyond what is listed below
  (`completion.ts`, `completion-popup.ts`, `completion-types.ts` and `format-core.ts` stay
  unedited).

## Ownership

writePaths: see the manifest entry `CANVAS-MOUNT`, which lists every source and test file.

New files:

- `editor/src/code/view-host.ts`: scroll, viewport, bridge and render feeding, so that
  `mount.ts` stays small.
- `editor/src/code/perf-hook.ts`: the `?perf=1` `window.__vactrPerf` installer and its rings.
- `editor/test/canvas/no-editor-view.test.ts`.
- `editor/test/canvas/mount.test.ts`.

sharedPaths (conditional, each with a recorded reason):

- `editor/src/code/layout.ts`, `editor/src/code/renderer.ts` and `editor/src/code/frame.ts`:
  edit only for integration defects found while composing. Keep the CANVAS-RENDER contracts.
- `editor/test/bind/drag.test.ts`, `editor/test/bind/routing.test.ts`,
  `editor/test/params/kinds.test.ts`, `editor/test/params/sampler.test.ts`,
  `editor/test/params/euclid.test.ts`, `editor/test/canvas/state.test.ts` and
  `editor/test/canvas/input.test.ts`: edit only to adapt fixtures. Every assertion is kept.

## Frozen contract (apply exactly; design 15.3.8.2)

`app/apis.ts`:

- Delete the `@codemirror/view` import and `CodeApi.view`. `CodeApi.surface: CodeSurface`
  becomes required.
- `CodeSurfaceUpdate.userEvent: string | null`.
- Add to `CodeSurface`:
  - `addKeymap(bindings: readonly { key: string; run(): boolean }[], precedence?: 'highest' | 'default'): () => void`
  - `onBlur(cb: () => void): () => void`
  - `onCompositionStart(cb: () => void): () => void`
  - `registerNumericDrag(provider: (event: PointerEvent, pos: number) => NumericGesture | null): () => void`
  (`NumericGesture` is re-exported as a type from `code/pointer.ts`).
- `annotationRanges()`, `undo()` and `redo()` stay class methods on `code/surface.ts`.

Key matching lives in `code/keyboard.ts`:

- Notation: `Mod-`, `Shift-`, `Alt-`, `Ctrl-` and `Meta-` plus `event.key`.
- If Alt or Shift changes the produced character, fall back to `event.code`
  (`KeyF` matches `f`, `Digit1` matches `1`). `Mod` is Meta on Apple (`navigator.platform`
  matches `/Mac|iP/`) and Ctrl elsewhere.
- Dispatch order: `highest` bindings, then `default` bindings, then the built-in
  `KeyboardController.handle`.
- No keymap runs during composition. A true return calls `preventDefault`.

## Tasks

### TASK-001: Surface and input contract (surface.ts, input.ts, keyboard.ts, pointer.ts, accessibility.ts, apis.ts)

- `CodeSurface`:
  - `dispatch` publishes `userEvent` from `tr.annotation(Transaction.userEvent) ?? null`;
  - keep the keymap registries, plus blur and composition-start listener sets;
  - `registerNumericDrag` providers are consulted in registration order, and the first
    non-null provider wins.
- `InputController` forwards `blur` and `compositionstart` to the surface listeners, and routes
  `keydown` through the surface keymap before `keyboard.handle`.
- `PointerController` `numericDrag` consults the surface providers.
- Delete `DocumentSync.extension()` from `sync.ts`, along with its `@codemirror/view` import.

### TASK-002: Mount composition (mount.ts, view-host.ts, code-view.tsx, code.css)

- `mount(root, deps, opts?: { gl?: WebGL2RenderingContext; frameHost?: FrameHost; createCanvas?: CanvasFactory })`.
  The extra parameter is a test seam; production passes nothing.
- Inside the `.vact-code` host (`code-view.tsx`, with its comment updated), create:
  - the canvas (`aria-hidden="true"`);
  - the `InputController` bridge container;
  - a `role="status"` GPU status element.
- Build `DocumentSync(client.document(DOC_FILE), Text.of(['']))`, then `CodeSurface({ sync })`,
  `InputController`, `PointerController`, `TextLayout` (metrics from an offscreen 2D context),
  and `CanvasRenderer(canvas, layout, { ledger, onStatus, gl })`. Attach the
  `surface.attachBridge` mapping through `view-host.ts` viewport and scroll state.
- `view-host.ts` owns scroll (wheel and `scrollBy`), keeps the caret visible after
  selection-setting transactions and keyboard-inset changes, and computes `LayoutViewport`.
- `FrameScheduler` `onFrame(ctx)` performs, in order:
  1. `highlight.tick(ctx.frameMs)` produces the animated playing ranges;
  2. `transport.tick(ctx.frameMs)`;
  3. eval flash ranges;
  4. if `ctx.textDirty`: `renderer.setDocument` (only when the revision changed) and the syntax
     provider `spans(visible range plus one viewport of overscan)`, then increment `textRevision`;
  5. `renderer.render({ annotations: [...syntaxSpans, ...surface.annotationRanges(), selection, composition], textRevision, animated, cursor, cursorVisible, handles })`.

  Syntax spans go directly into `RenderFeedback`. Never send them through `surface.annotate`,
  because that would publish, and the mount subscriber would invalidate text, causing a loop.
- Surface updates:
  - a doc or selection change calls `scheduler.invalidateText()`;
  - an annotation-only update also calls `invalidateText()` (that is how diagnostics, bind and
    call-head changes redraw);
  - playing, eval and caret blink call `setActive`.
- `renderer.textPending` true calls `scheduler.request()`.
- Background: on each frame, if not yet subscribed and `deps.visual?.onBackgroundCanvas` exists,
  subscribe once. The callback calls `renderer.setBackground(canvas, ++bgRevision)` and
  `scheduler.request()`.
- GPU status: show `status.message` in the status element. For `unavailable`, show
  "GPU unavailable; editing and save remain available". Never render source into the DOM.
- Perf: when `win.location.search` has `perf=1`, construct the scheduler with `perf: true` and
  install `win.__vactrPerf` from the new file `editor/src/code/perf-hook.ts` (contract below).
  Record `recordKey(event.timeStamp, sync.revision)` on bridge `keydown`.
- Status counters: when any of `highlight.stats.overflow`, `horizonDrops` or `epochDrops`, or
  the client `queueStats.dropped`, is non-zero, the GPU status element carries
  `data-telemetry-dropped="<sum>"` (design 15.3.8.7). Update it at most once per text phase.

#### `window.__vactrPerf` contract (pinned; CANVAS-EVIDENCE reads only these members)

Installed only under `?perf=1`. Without that flag, `perf-hook.ts` is never instantiated, and no
ring or record is allocated.

```ts
export interface PresentedRecord { frameMs: number; targetMs: number; audibleTime: number; provenance: Provenance; valid: boolean;
  activeKey: string; beatCycle: number | null; beatFlash: boolean; revision: number; handles: number }
export interface OnsetRecord { time: number; end: number; from: number; to: number; epoch: string | null; receivedMs: number }
export interface VactrPerf {
  perf: PerfRecorder;                      // CANVAS-RENDER: frames [frameMs, workMs, text, revision], keys [timeStamp, revision]
  revision(): number;                      // sync.revision
  ledger: ResourceLedger;                  // the code pane ledger; the harness keeps this reference across disposeCode()
  doc(): string;                           // surface.state.doc.toString()
  selection(): { anchor: number; head: number };
  presented(): PresentedRecord[];          // copy of a fixed 4096-entry ring, oldest first
  onsets(): OnsetRecord[];                 // copy of a fixed 4096-entry ring, fed by HighlightScheduler.onAccept
  transportSample(): TransportSample | null; // store.transportSample
  counters(): { highlight: HighlightScheduler['stats']; probe: ProbeCorrelation['stats'] | null; client: Client['queueStats'];
    syntaxTruncated: number; textPending: boolean; renderer: CanvasRenderer['stats']; gpuStatus: { kind: string; effectiveDpr: number };
    ledger: ResourceLedger['counters']; usedBytes: number };
  disposeCode(): void;                     // runs the normal code-area dispose path, then deletes window.__vactrPerf
}
```

Write a presented record exactly once per frame, after `renderer.render`. Its fields are:

- `targetMs` and `audibleTime`: from the frame's `AudibleSample`;
- `activeKey`: the animated playing ranges sorted by `(from, to)` and joined as
  `from-to,from-to` (an empty string when none);
- `beatCycle` and `beatFlash`: from `TransportBar.state`;
- `handles`: the count passed to `render`.

`ProbeCorrelation` is reached through `deps.audible`. `perf-hook.ts` takes it as an optional
constructor argument, and `counters().probe` is `null` when it is absent.
- Dispose, in reverse order: scheduler, controllers, renderer (ledger `usedBytes` returns to 0),
  bridge, listeners, then delete `deps.code` and `__vactrPerf`.
- `CodeApi`: `{ surface, mapWireSpan, currentRevision, selectedSiteId (reads surface.state.selection.main.head), samples }`.
- Pointer events: forward the canvas `pointerdown`, `pointermove`, `pointerup` and
  `pointercancel` events to `surface.notifyPointer` before the `PointerController` handles them.

### TASK-003: Eval, diagnostics, format, syntax, completion

- **eval.ts**:
  - `EvalController.attach(surface)` registers default keymap bindings for `Mod-Enter`,
    `Mod-Shift-Enter` and `Mod-.`;
  - remove the StateField and decoration, and expose `onFlash(cb: (range, kind) => void)`. The
    mount shows the flash for 200 ms through `setActive('eval')`;
  - keep flush-before-eval and stale-generation behavior exactly.
- **diagnostics.ts**:
  - `attach(surface)` uses `surface.annotate('diagnostics', ranges)` with
    `{ kind: 'diagnostic', className: 'vact-diag-<severity>', label: message }`;
  - `text` comes from `surface.state.doc.toString()` (debounced local check, unchanged);
  - add `diagnosticAt(pos)` for the mount hover tooltip (a DOM element positioned at
    `coordsAtPos`, hidden on `pointerleave` and scroll);
  - announce count changes with `accessibility.announce` when the count changes.
- **format.ts**:
  - `formatDocument(surface, formatter)` returns false while composing, aborts if the text
    changed during the await, and otherwise dispatches one `minimalChange` with
    `userEvent: 'format'`;
  - `formatKeymap(surface, getFormatter)` returns the `addKeymap` disposer for `FORMAT_KEY`.
- **syntax.ts and syntax-core.ts**:
  - `class SyntaxSpans { constructor(syntax: VactSyntax); noteChanges(changes: ChangeSet, newState: EditorState): void; spans(state, from, to, cap = 16384): { spans: CodeAnnotation[]; truncated: boolean }; dispose(): void }`;
  - reparse lazily at most once per `spans` call after changes, via `tree.edit` for each
    coalesced change plus `parser.parse(text, oldTree)`;
  - add `reparse(parsed, text, edits)` to `syntax-core.ts`;
  - any reparse exception falls back to a full parse;
  - spans beyond the cap count in `syntaxTruncated` and are still drawn in the default color.
- **language.ts**:
  - remove the imports from `@codemirror/language` and `@lezer/highlight`, and remove
    `vactLanguage` and `vactHighlightStyle`;
  - add a local `LineStream` class implementing the `StringStream` methods `vactParser` uses
    (`match`, `eat`, `eatWhile`, `next`, `peek`, `skipToEnd`, `sol`, `eol`, `current`, `pos`,
    `start`);
  - add `class FallbackSpans` with a per-line start-state cache up to the viewport end,
    truncated at the first changed line;
  - `tokenize` and `tokenizerSpans` keep their outputs.
- **completion-view.ts**:
  - `attachCompletion(surface, engine)` builds `CompletionSurface` from the contract (design
    15.3.8.2 rule b);
  - the completion keys use `addKeymap(..., 'highest')`;
  - `caretRect` uses `surface.coordsAtPos`;
  - the mount calls `popup.reanchor()` on viewport change. If `completion-popup.ts` lacks a
    method for this, close the popup instead, by dispatching blur listeners.
- `codePane.dataset.syntax` stays `fallback` until tree-sitter loads, then becomes
  `tree-sitter`.

### TASK-004: Song, bind and params (app/song.ts, bind/*, params/*)

Follow design 15.3.8.2 rule (a) and the "Bind and params" rule exactly:

- **song.ts**: only `surface.subscribe`; delete the `Compartment`/`EditorView` branch.
- **bind/mount.ts**: surface reads (`sliceDoc`, `doc.lineAt`, `doc.toString()`) and
  `surface.dispatch`. Overlay values become zero-width
  `annotate('bind-overlays', [{ from: pos, to: pos, kind: 'binding', label }])`, cleared on
  dispose.
- **bind/drag.ts**: `registerNumericDrag`; delete `OverlayWidget`, `overlayField` and
  `setOverlays`.
- **bind/write.ts and bind/routing.ts**: the `view` dependency type becomes `CodeSurface`, and
  writes overlapping composition use `deferSourceWrite`.
- **params/mount.ts**:
  - `markHeads` uses `annotate('params-heads', ...)` with `kind: 'call-head'`,
    `className: 'params-call-head'` and `label: groupId`;
  - click-to-open is an `onPointer` handler: a primary `pointerup` within 5 px of its
    `pointerdown`, with no numeric gesture and no composition, on `posAtCoords` inside a
    call-head range, runs `showTab('editors')` and `open(id)`;
  - the handler never calls `preventDefault`;
  - `formText` uses `sliceDoc`;
  - dispose clears the owner.
- **params/roll.ts**: `sourceText` uses `code.surface.state.sliceDoc`.

### TASK-005: Guard and test ports

- `editor/test/canvas/no-editor-view.test.ts`: begin with `// @vitest-environment node`.
  `editor/tsconfig.json` has `"types": []` and `allowJs: false`, and there is no `@types/node`,
  so a literal `import 'node:fs'` fails `npm run check`. Follow
  `editor/test/support/wasm.ts:14-24,67`: declare a local `NodeFs` interface
  (`readdirSync(path, { withFileTypes: true })`, `readFileSync(path, 'utf8')`) and a
  `NodeProcess` (`cwd()`), then load with
  `(await import(/* @vite-ignore */ spec)) as NodeFs`, where `const spec = 'node:fs'` is
  non-literal at the call. Recurse over `<cwd>/src`, reading every `.ts` and `.tsx` file. Fail on `from '@codemirror/view'`, `'@codemirror/lint'`, `'@codemirror/language'`
  or `'@codemirror/autocomplete'`, and on the identifier `EditorView`. Print the offending
  `file:line`.
- Port each listed test from EditorView to the surface, keeping every assertion's intent:
  - `view.dispatch` becomes `surface.dispatch`;
  - `playingRanges(view)` becomes the `HighlightScheduler.active()` or `apply` output;
  - lint `diagnosticCount` becomes the `annotationRanges()` `diagnostic` entries;
  - DOM `.params-call-head` queries become the `call-head` annotations, plus a synthetic
    `pointerdown`/`pointerup` with a stubbed `posAtCoords` (design 15.3.8.2 test-port rule).
- `test/params/open.test.ts` keeps the `['peq','env-adsr','euclid','odd','wobble']` and
  "click opens envelope-shape" assertions.
- `test/canvas/contracts.test.ts:26` asserts `CodeApi['surface']` equals `CodeSurface`.
- `test/code/language.test.ts` drops `syntaxTree` and `vactLanguage` cases in favor of the
  equivalent `tokenize` and `tokenizerSpans` assertions on the same inputs.

New tests in `editor/test/canvas/mount.test.ts` (jsdom, fake GL from `test/support/gl.ts`, fake
2D from `test/support/canvas.ts`):

| Situation | Expected outcome |
|-----------|------------------|
| Type `let x 1` through the bridge | `surface.state.doc` updated; `doc-changed` sent; no text node under the code pane contains `let x 1` |
| No WebGL2 (no `gl` injected in jsdom) | Status says GPU unavailable; editing and `deps.code.surface` still work |
| Idle | After the first frame, no further rAF |
| Playing events active | Frames continue; `renderer.stats.textBuilds` does not grow across animation frames |
| `Mod-Enter` | Eval request sent; eval flash active for about 200 ms of frame time, then cleared without replay |
| `Shift-Alt-f` with `event.key` = U+00CF (the macOS Shift-Alt-f character), `code 'KeyF'` | Format runs |
| Composition active | Keymaps do not run; format refused |
| dispose | ledger `usedBytes === 0`; no listeners left (spy counts); `deps.code` deleted |
| `?perf=1` | `window.__vactrPerf.perf` records frames and keys |
| `?perf=1`, a playing event active at the frame's audible time | The last `presented()` record has `activeKey` equal to the mapped `from-to`, `audibleTime === sample.time`, `targetMs === sample.targetMs`, and `beatCycle` from `TransportBar.state` |
| `?perf=1`, a playing batch received | `onsets()` gains one record per accepted event, with mapped `from`/`to` and `receivedMs` |
| `?perf=1`, an event 3 s in the future | `counters().highlight.horizonDrops === 1`; the status element has `data-telemetry-dropped="1"` |
| `?perf=1`, type `ab` and then select all | `doc()` ends with `ab`; `selection()` equals `surface.state.selection.main` |
| `?perf=1`, `disposeCode()` | The retained `ledger.usedBytes === 0`; `window.__vactrPerf` is undefined |
| No `perf` flag | `window.__vactrPerf` is undefined, and `perf-hook.ts` is not constructed (spy) |
| One-character edit in a 1 MiB document | `doc-changed` payload < 256 bytes; no whole-text message (design 15.3.8.7) |
| Incremental syntax property test: 200 random edits on a 2,000-line doc | `SyntaxSpans` output equals a fresh full-parse `styleSpans` after each edit (in `test/code/syntax-core.test.ts`) |

## Pitfalls

- Sending syntax spans through `surface.annotate`, which causes a publish loop.
- Calling `layout.setDocument` or reparsing on animation-only frames.
- Letting a call-head click call `preventDefault`, or opening a head when a numeric drag
  started.
- web-tree-sitter `tree.edit` indices: verify the units with the property test. Do not assume
  that UTF-8 bytes equal UTF-16 code units.
- Leaving any `EditorView` reference in `editor/src`. The guard must pass.
- A literal `import ... from 'node:fs'` in a test file, adding `@types/node`, editing
  `tsconfig.json`, or using `@ts-nocheck`. Use the `test/support/wasm.ts` dynamic-import
  pattern.
- Allocating `__vactrPerf` rings or building `activeKey` strings when `perf` is off.
- Editing `app/main.ts`, `test/app/main.test.ts`, `visual/*`, `package.json` or lockfiles.
- `mount.ts`, `view-host.ts` and every touched file must stay below 1000 lines (split helpers
  into `view-host.ts`).

## Session 266 Amendment (operator decisions A and D)

- **Artifact roots** (manifest `CANVAS-MOUNT.artifactRoots`, each also a writePath): `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm`, `tmp/canvas-cutover/mount`, `editor/node_modules/.vite`.
  These are gitignored outputs and are never committed. If a command writes any other gitignored
  in-repo path, stop, record the path in this progress log, and ask for a serial manifest
  amendment.
- **Setup (not gating)**: `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`,
  then `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
  The syntax tests need the grammar wasm. A missing grammar is an environment gap; never skip
  those tests.
- **Baseline**: MOUNT starts after wave-1 acceptance. Record the pre-plan full-Vitest count
  (at least 612 at `40467f3`) in `checks.log` before the first edit. The final count must not be
  lower.
- **Rule D**: the gating list contains only final-source commands that are expected to pass.
  A re-run after a fixed failure replaces the failed run, and the failure goes into history notes
  only. Negative-control runs (for example, proving the no-editor-view guard fails on an injected
  import) go under `mutationEvidence` with their log paths.

## Verification (record in tmp/canvas-cutover/mount/checks.log)

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/canvas test/code test/bind test/params test/app/song.test.ts` | all pass |
| `cd editor && ./node_modules/.bin/vitest run` | all pass; test count >= the pre-plan count; no `it(` removed without a ported replacement (list the ports in the progress log) |
| `grep -rn "@codemirror/view\|EditorView" editor/src` | no output |
| `wc -l` on every touched source file | each < 1000 |

## Overwrite and Drift Protocol

Record fresh-read sha256 values before each edit (`tmp/canvas-cutover/mount/intent.json`) and
after it (`receipt.json`). Wave-1 outputs are the base. On drift, stop editing that file and
repair serially after the join. Edit only this plan's progress log.

## Completion Criteria

- [ ] Frozen contract applied; `CodeApi.view` gone; guard test passes
- [ ] All consumer rules of 15.3.8.2 implemented; every `code.view` use migrated
- [ ] Ported tests keep their assertions; new mount and syntax tests pass
- [ ] `npm run check` exit 0; full vitest passes

## Progress Log

### Session: 2026-10-05
**Tasks Completed**: Plan authored

### Session: 2026-10-05 (session 266 plan amendment)
**Tasks Completed**: Plan amended per operator decisions A and D. Artifact roots declared, setup separated from gating, pre-plan Vitest baseline rule added. Tasks and contracts are unchanged.
