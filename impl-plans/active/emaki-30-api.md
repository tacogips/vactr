# EMK-API: plugin runtime, createEditor, public entry and README

**Status**: Ready (after wave 2)
**Plan ID**: EMK-API (wave 3; parallel with EMK-VAC-SWITCH)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 3, 5.1-5.3, 7.1-7.2, 11.3 (README content); decision EM-D2
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants vactr's editor usable as an independent library. Generic
consumers and the demos need one call, `createEditor(host, options)`, that
returns a handle. Plugins (video, animation, the diff demo, vactr's
sequencer and scope) need a runtime that gives each plugin the narrow
`PluginHost` defined in `emaki/src/types.ts` and drives the lifecycle hooks.

This plan adds:
- `PluginRuntime`;
- `createEditor`, composed from the engine blocks the same way vactr's
  `editor/src/code/mount.ts` does;
- `emaki/src/index.ts`;
- `emaki/README.md`.

## Non-goals

- No change to engine blocks (render, layout, input, view, decor, plugin
  widgets, language). If a block has a defect, stop and report a scope
  amendment.
- No demo pages (wave 4).
- No vactr edits.

## Dependencies

- **dependsOn**: EMK-CORE, EMK-DECOR, EMK-WIDGETS, EMK-LANG.
- Runs in parallel with EMK-VAC-SWITCH. Their writePaths are disjoint.
  EMK-VAC-SWITCH does not import `PluginRuntime`.

## writePaths

Source:
- `emaki/src/plugin/index.ts`, `emaki/src/plugin/runtime.ts`
- `emaki/src/editor.ts`, `emaki/src/editor-frame.ts`
- `emaki/src/index.ts`
- `emaki/README.md`

Tests:
- `emaki/test/plugin/runtime.test.ts`, `emaki/test/editor.test.ts`

Generated, plus this plan:
- `emaki/node_modules`, `emaki/dist` (artifactRoots)
- `impl-plans/active/emaki-30-api.md`

## sharedPaths

None. Read-only: everything else under `emaki/src`;
`editor/src/code/mount.ts` (the pattern to imitate).

## Tasks

### A1: `plugin/runtime.ts`, class `PluginRuntime`

**Constructor.**
`new PluginRuntime({ store: DecorationStore; widgets: WidgetSet; getState(); requestFrame(); coordsAtPos(pos); lineTop(line); viewport(): PluginViewport; theme(): PluginHost['theme']; onError?(plugin: string, error: unknown): void })`.

**`add(plugin): () => void`**
- Build a `PluginHost` for the plugin.
- Owner names are namespaced as `plugin.name + ':' + owner`, so two plugins
  never collide.
- Call `attach(host)` inside try/catch.
- The remover calls `destroy`, then clears every owner the plugin created
  (marks, line formats, widgets) and its animated providers.

**Fan-out methods.** These call hooks in add-order:
- `docChanged(update)`: first `store.map(changes)` and `widgets.map(changes)`,
  then the plugins;
- `viewportChanged(vp)`: only when `fromLine`, `toLine`, `top` or `height`
  changed;
- `frame(frameMs): boolean`: true if any plugin returned true or called
  `setActive(true)`.

**Error handling (design 7.1).** A hook that throws -> report once through
`onError` and `host.reportError`, then detach the plugin with the same
cleanup as remove. Other plugins keep running.

`destroy()` removes all plugins.

### A2: `editor.ts` and `editor-frame.ts`, `createEditor(host, options)` (design 5.1)

**Options.**
`EditorOptions = { doc?, language?, plugins?, theme?, font?, readOnly?, lineNumbers? = true, extensions?, modalKeys?, gl?, frameHost?, createCanvas? }`.
- Default font: `{ family: 'ui-monospace, SFMono-Regular, Menlo, monospace', size: 13, lineHeight: 18, baseline: 14 }`
  (the same as `mount.ts:91`).

**Composition.** Imitate `editor/src/code/mount.ts:67-430`, without vactr
parts:
- `EditorCore`, then `TextLayout(createMeasureContext(doc) ?? fallback)`;
- `CanvasRenderer` (canvas class `emaki-canvas`, `aria-hidden`);
- `ViewHost`;
- `InputController` (no `fallbackShortcut`, no eval callbacks; `modalKeys`
  via `setModalHook`);
- `PointerController`;
- `FrameScheduler`;
- `DecorationStore`, `WidgetSet`, `WidgetLayer`, `PluginRuntime`;
- for a line-kind language, `IncrementalHighlighter`.

**Read-only.** `readOnly` installs an `EditorState.readOnly.of(true)`
extension plus a transaction filter that rejects user-event document changes
(`userEvent` starting `input` or `delete`). Selection still works.
`dispatch` from the handle still applies.

**Per frame** (in `editor-frame.ts`, imitating `mount.ts:237-298`):
1. On edit, `runtime.docChanged`, `layout.setText(doc, changes)`, and
   `highlighter.applyChanges`.
2. Viewport lines come from `layout.heightmap.lineAtHeight` at the
   overscanned top and bottom.
3. `runtime.viewportChanged`.
4. `runtime.frame(frameMs)`.
5. `store.collect(frameMs, doc)`.
6. When `widgets.revision` changed: `layout.heightmap.setBlocks(widgets.entries(doc))`.
7. Tokens: line kind uses `highlighter.tokens(from, to)`; span kind uses
   `provider.spans(state, from, to, 16384)`.
8. `renderer.render({ textRevision, decorationsKey: store key + selection + revision, tokens, selection, composition, marks, animatedMarks, lineFormats, cursor, handles })`.
9. `widgetLayer.update(...)`.
10. Keep frames active while the runtime, the widget layer or
    `highlighter.catchUp()` want more work.

**Handle.**
- `state`, `dispatch`, `undo`, `redo`
- `setDoc(text)`: one transaction replacing the whole doc with
  `addToHistory.of(false)`, then `layout.resetWidestShaped()`
- `onUpdate`, `onScroll`
- `scrollTop`, `scrollTo({top}|{pos})`
- `lineTop`, `lineAtHeight`
- `addPlugin`, `focus`, `requestFrame`
- `revision`: a counter incremented on every doc change
- `stats`
- `destroy()`: disposes everything and removes the created DOM. A double
  destroy is a no-op.

**Perf hooks (contract for EMK-JSON-DEMO, EMK-DIFF-DEMO and EMK-PERF).**
`handle.perf` has two subscriptions.

- `onPresent(cb: (s: { frameStart: number; frameEnd: number; workMs: number; revision: number; visibleComplete: boolean }) => void): () => void`:
  - fires at the end of every frame callback that rendered;
  - `frameStart` and `frameEnd` are `performance.now()` at the callback's
    start and end;
  - `visibleComplete` is false while the highlighter result is
    `approximate` or a span provider reported `truncated`.
- `onInput(cb: (s: { eventTimeStamp: number; start: number; end: number; workMs: number; revisionAfter: number }) => void): () => void`:
  - fires for each `keydown` and `beforeinput` on the input bridge;
  - measured with a capture-phase listener on the editor host element
    (start) and a bubble-phase listener on the same element (end);
  - no change to `InputController`.

Pitfall: register the bubble listener after `InputController` is constructed.
It runs after target listeners anyway, because the bridge textarea is a
descendant.

**File size.** Keep `editor.ts` and `editor-frame.ts` each under 600 lines.

### A3: `src/index.ts`

- Export `createEditor`, `EditorOptions`, `EditorHandle`.
- Re-export the design 5.1 types from `./types`: `Mark`, `MarkStyle`,
  `AnimatedMarkStyle`, `LineFormat`, `LineFormatStyle`, `BlockWidgetSpec`,
  `EolWidgetSpec`, `WidgetContent`, `EmakiPlugin`, `PluginHost`,
  `PluginInstance`, `LanguageProvider`, `LineLanguage`, `SpanLanguage`,
  `Rect`, `Range`, `Rgba`.
- Re-export `Palette` as a type.

`plugin/index.ts` adds `PluginRuntime` to EMK-WIDGETS' `WidgetSet` export.

### A4: `emaki/README.md`

Sections:
1. What emaki is.
2. Install:
   - `file:` and `github:` installs;
   - the source-exports note;
   - required consumer config: Vite `resolve.dedupe` for
     `@codemirror/state` and `@codemirror/commands`, Vitest
     `test.server.deps.inline: ['emaki']`, TypeScript `preserveSymlinks`;
   - the peers.
3. Quick start.
4. `createEditor` options and handle.
5. Document, transactions and history (`state`, `dispatch`, `undo`/`redo`).
6. Language providers (line kind, span kind, `IncrementalHighlighter`,
   `emaki/json`).
7. Marks, including glow, with the animated-mark restriction.
8. Line formats.
9. Precedence (copy design 6.3).
10. Plugins (lifecycle, `PluginHost`, error isolation).
11. Block and EOL widgets (DOM and canvas content, virtualization, hit
    testing).
12. `emaki/diff`.
13. `emaki/engine` (the composition-level API).
14. Demos (`npm run dev`).
15. Scripts (`check`, `test`, `build`, `perf`, `e2e`), with the metric
    definitions from design 12.2.
16. License MIT.

No vactr mention: the boundary test scans only `.ts`, `.mjs` and `.html`,
but keep the README product-neutral anyway.

### A5: Tests

**`plugin/runtime.test.ts`**
- two plugins setting owner `x` -> no collision;
- a plugin whose `frame` throws -> `onError` is called once, the plugin is
  detached and its marks cleared, and the other plugin keeps receiving
  `frame`;
- the remover -> `destroy` is called, and widgets and animated providers
  are gone;
- `viewportChanged` is not called again for an identical viewport;
- `docChanged` maps the store before calling the plugin, so the plugin sees
  mapped marks.

**`editor.test.ts`** (jsdom; recording GL from
`emaki/test/support/recording-gl.ts` via `options.gl`; a manual frame host
imitating `editor/test/canvas/mount.test.ts`'s frame host)
- `createEditor` with `doc 'a\nb'` -> one canvas, and the textarea bridge
  exists;
- after a frame, `onPresent` fired with `revision 0`;
- type via a `beforeinput` insertText `'x'` -> `revision 1`, `onInput`
  fired with `workMs >= 0`, and the next frame's `onPresent.revision === 1`;
- `readOnly` -> `beforeinput` does not change the doc, but
  `handle.dispatch` does;
- `addPlugin` with a block widget below line 0, 40 px -> `lineTop(1) === 18 + 40`;
- `jsonLanguage` -> the render input `tokens` contain `json-key`;
- `destroy` twice -> no throw, and the host is empty.

## Invariants

- `createEditor` uses only engine blocks. No engine logic is duplicated into
  `editor.ts`.
- The boundary test stays green.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-API/`.

0. **Precondition.** The wave-2 join gate (rule 12a) is green: cite
   `tmp/emaki/wave-2-join/` in the Progress Log. Do not start otherwise.
1. `npm run check`: passes per rule 12b. EMK-VAC-SWITCH may be editing
   emaki parity sharedPaths, so errors there are drift.
2. `./node_modules/.bin/vitest run test/plugin test/editor.test.ts`:
   exit 0, testsRun > 0. **This is the pass gate.**
3. `./node_modules/.bin/vitest run` and `npm run build` (lock):
   informational per rule 12b. The wave-3 join gate is authoritative.
4. `wc -l` on every written file: each under 1000.
5. `node -e` that reads `src/index.ts` and checks each A3 name appears in an
   export statement.

**mutationEvidence.** Remove the try/catch in `PluginRuntime.frame` -> the
error-isolation test fails. Restore afterwards.

## Completion Criteria

- [ ] A1-A5 delivered.
- [ ] Verification steps 0-5 pass per rule 12b scoping. Mutation evidence
  is recorded. Hashes are
  logged.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
