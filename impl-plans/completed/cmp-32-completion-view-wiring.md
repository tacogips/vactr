# CMP-32: EditorView Completion Adapter and Wiring

**Status**: Completed
**Plan ID**: CMP-32 (wave 3)
**Design Reference**: `design-docs/specs/design-completion.md` 6.4, 6.5, 6.6, 7.4 (the view bullets)
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan connects the UI-agnostic completion (CMP-30 service, CMP-31
popup) to today's CodeMirror EditorView, through ONE adapter file. It
also adds the minimal wiring lines, so that candidates appear in the
running editor.

The canvas plans on `origin/main` (J1 and C1) own `mount.ts`, `main.ts`
and `deps.ts`. So these edits stay at a few lines each, and all logic
lives in `completion-view.ts`.

## Non-goals

- No `@codemirror/autocomplete`, and no `package.json` or lockfile
  change.
- No edits to `apis.ts`, `language.ts`, `format*.ts`, `syntax*.ts`,
  `completion.ts`, `completion-popup.ts` or `completion-types.ts`.
- No change to the existing keymaps or extension order in `mount.ts`.

## Dependencies

- **dependsOn**: CMP-21 (the real export for the engine test), CMP-30,
  CMP-31, EDS-12
- **Blocks**: CMP-40

## writePaths

- `editor/src/code/completion-view.ts` (new)
- `editor/src/code/mount.ts`: wiring lines only.
- `editor/src/app/main.ts`: wiring lines only.
- `editor/src/app/deps.ts`: wiring lines only.
- `editor/test/code/completion-view.test.ts` (new)
- `editor/test/wasm/completion-engine.test.ts` (new)
- `impl-plans/active/cmp-32-completion-view-wiring.md` (Progress Log only)

## sharedPaths (read only)

`editor/src/code/completion-types.ts`, `editor/src/code/completion.ts`,
`editor/src/code/completion-popup.ts`, `editor/src/code/tool-wasm.ts`,
`editor/src/code/format-core.ts`, `editor/src/code/format.ts`
(`formatKeymap`, for the Shift-Alt-f assertion),
`editor/test/code/format.test.ts` (the `viewWith` and mount-deps
pattern), `editor/test/support/wasm.ts`.

## Pinned Export

```ts
export function attachCompletion(view: EditorView, engine: CompletionEngine): { dispose(): void };
```

## File-level Changes

1. `completion-view.ts`:
   - Build a `CompletionSurface` over `view`:
     - `text()` is `view.state.doc.toString()`;
     - `selection()` is `view.state.selection.main`;
     - `replace` dispatches
       `{changes: {from, to, insert}, selection: {anchor: from + insert.length}, userEvent: COMPLETION_USER_EVENT, scrollIntoView: true}`;
     - `caretRect(pos)` is `view.coordsAtPos(pos)`, mapped to
       `{left, top, bottom}`, or `null`;
     - `isComposing()` is `view.composing`;
     - `popupHost()` is `view.dom.ownerDocument.body`.
   - Attach listeners with ONE
     `view.dispatch({effects: StateEffect.appendConfig.of([...])})`:
     - `Prec.highest(keymap.of(COMPLETION_KEYS.map(key => ({key, run: () => keyHandler?.(key) ?? false}))))`;
     - `EditorView.updateListener.of(update => ...)`, which emits
       `SurfaceChange`. Set `userEvent` from
       `update.transactions[i].annotation(Transaction.userEvent) ?? null`,
       using the last transaction that has one. Set `inserted` by
       concatenating the inserted texts from `changes.iterChanges`;
     - `EditorView.domEventHandlers({blur, compositionstart})`, each
       returning `false`.
   - Keep the listener sets in the adapter closure.
   - Create `new CompletionService(engine)` and
     `new CompletionPopup(surface, service)`.
   - `dispose()`: dispose the popup, clear the handler and listener sets
     (the appended extensions become inert; do not reconfigure the
     view), and make it idempotent.
2. `deps.ts`: add `import type { CompletionEngine } from '../code/completion-types';`
   and an optional field with a doc comment,
   `completion?: CompletionEngine;`.
3. `main.ts`:
   - `const tool = new ToolWasm(formatterUrl);`
   - `deps.formatter = new WasmFormatter(tool);`
   - `deps.completion = new WasmCompletionEngine(tool);`
   - add the imports.
4. `mount.ts`:
   - `import { attachCompletion } from './completion-view';`
   - right after `diagnostics.attach(editorView);`:
     `const completion = deps.completion ? attachCompletion(editorView, deps.completion) : null;`
   - in `dispose()`, `completion?.dispose();` before
     `editorView.destroy();`.

## Pitfalls

- Bind only the eight `COMPLETION_KEYS`. Never bind `Mod-Enter`,
  `Mod-Shift-Enter`, `Mod-.` or `Shift-Alt-f`.
- The `run` callback returns the popup's answer, so a closed popup lets
  Enter fall through to `defaultKeymap`'s `insertNewlineAndIndent`.
- Do not create a new `EditorView` or a new state. Use `appendConfig` on
  the live view.
- Import only from `@codemirror/view` and `@codemirror/state`, which are
  existing dependencies.

## Tests

- `editor/test/code/completion-view.test.ts` (jsdom). Mount a real
  `EditorView` on `document.body` with the `defaultKeymap`, as
  `format.test.ts` does, and a fake `CompletionEngine` that returns a
  `RawCompletion` for `alpha` whose byte offsets cover the current word.
  - Dispatch
    `{changes: {from: end, insert: 'a'}, selection: {anchor: end + 1}, userEvent: 'input.type'}`,
    then flush the promises. `.vact-completion` exists in
    `document.body`.
  - While open: `runScopeHandlers(view, new KeyboardEvent('keydown', {key: 'Enter'}), 'editor')`
    returns `true`, the document now contains `alpha`, it contains no
    newline, and the popup is closed.
  - While closed: the same Enter event goes through `defaultKeymap`
    (`insertNewlineAndIndent`). The document gains exactly one `\n` and
    no candidate text.
  - The facet keymaps contain no binding for `Mod-Enter`,
    `Mod-Shift-Enter`, `Mod-.` or `Shift-Alt-f` that the adapter added.
    Read `view.state.facet(keymap)` before and after attaching, and
    compare the added key names against `COMPLETION_KEYS`.
  - `dispose()` removes the panel, and later typing opens nothing.
  - A rejecting engine: typing opens nothing, and the editor still
    accepts input.
  - Mount through `mount()` with `deps.completion` set (as in the
    `format.test.ts` mount test): the pane mounts, and `dispose` does not
    throw. Without `deps.completion`, the behavior is unchanged.
  - A source-text check: `completion.ts`, `completion-popup.ts` and
    `completion-types.ts` contain no `@codemirror/view`.
- `editor/test/wasm/completion-engine.test.ts` (`// @vitest-environment node`).
  Build `new ToolWasm('test://vactr.wasm', fetchFn)`, where `fetchFn`
  returns `new Response(bytes)` read from `(await loadVactrWasm()).path`.
  Then `new CompletionService(new WasmCompletionEngine(tool))`.
  - `fn f alpha:\n\tal` at the end -> the first label is `alpha`, with
    UTF-16 `from`/`to` at `al`.
  - The same text with a Japanese comment line before it -> `from` and
    `to` are UTF-16 correct.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`. Before editing `mount.ts`,
`main.ts` and `deps.ts`, record their sha256. The diff of each must be
wiring lines only: `git diff --stat` shows a few added lines per file,
with no removals other than the replaced `WasmFormatter(formatterUrl)`
line in `main.ts`. Write logs to `tmp/cmp/CMP-32/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `cd editor && npx tsc --noEmit`
2. `cd editor && npx vitest run test/code/completion-view.test.ts test/wasm/completion-engine.test.ts`
   (a positive count; needs the wasm build from CMP-21)
3. `cd editor && npx vitest run test/code`: the existing eval, format,
   history, syntax, highlight and diagnostics tests stay green.
4. `cd editor && npm run build`
5. `git diff --numstat -- editor/src/code/mount.ts editor/src/app/main.ts editor/src/app/deps.ts`:
   each file has 5 or fewer added lines.

## Completion Criteria

- [x] `attachCompletion` implements the surface in the only completion
      file that imports `@codemirror/view`.
- [x] Enter accepts while the popup is open and inserts a newline while
      it is closed.
- [x] Eval and format keys are untouched, and the wiring is minimal.
- [x] The real-wasm engine test passes.
- [x] Steps 1-5 are logged, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None. The initial dependency wait was resolved by the runtime accepted dependency set for CMP-21, CMP-30, CMP-31 and EDS-12.

### Session: 2026-09-30 (CMP-32 implementation)
**Tasks Completed**: Added the EditorView CompletionSurface adapter with one appendConfig dispatch, completion-only key bindings, update/blur/composition forwarding, guarded replacement and idempotent disposal. Added optional mount wiring and shared ToolWasm boot wiring. Added EditorView behavior tests and real-wasm completion service tests. CMP-21, CMP-30, CMP-31 and EDS-12 are accepted dependencies per the dispatch runtime.
**Verification**:
- `cd editor && npx tsc --noEmit` — exit 0; `tmp/cmp/CMP-32/attempt-1/07-tsc-final.log`.
- `cd editor && npx vitest run test/code/completion-view.test.ts test/wasm/completion-engine.test.ts` — exit 0, 9/9; `tmp/cmp/CMP-32/attempt-1/04-focused-vitest.log`.
- `cd editor && npx vitest run test/code` — exit 0, 124/124; `tmp/cmp/CMP-32/attempt-1/05-code-vitest.log`.
- `cd editor && npm run build` — exit 0; `tmp/cmp/CMP-32/attempt-1/06-build.log`.
- `git diff --numstat -- editor/src/code/mount.ts editor/src/app/main.ts editor/src/app/deps.ts` — exit 0; added lines are 3, 5 and 3 respectively; `tmp/cmp/CMP-32/attempt-1/08-wiring-numstat.log`.
- `rg -l '@codemirror/view' editor/src/code/completion*.ts` — exit 0; only `completion-view.ts`; `tmp/cmp/CMP-32/attempt-1/09-codemirror-import-boundary.log`.
**Blockers**: None for CMP-32 implementation. CMP-40 owns combined-tree review and plan closeout.

## Related Plans

- **Depends On**: CMP-21, CMP-30, CMP-31, EDS-12
- **Next**: CMP-40
