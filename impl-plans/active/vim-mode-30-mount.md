# Vim Mode: Mount Wiring, Integration Tests and Style Probe Implementation Plan

**Status**: In Progress
**Plan ID**: VIM-30 (wave 3)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 3.1, 3.4, 3.6, 3.7, 6, 8.2, 8.4
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

## Related Plans

- **Depends On**: VIM-15 (input routing), VIM-16 (cursor hook), VIM-17 (UI pieces), VIM-20 (controller)
- **Next (wave 4)**: VIM-40 (e2e and full gates), VIM-41 (docs)

## Intent and Context

This plan connects the accepted pieces inside the canvas mount
(`editor/src/code/mount.ts`, 382 lines):

- a `VimHost` over `CodeSurface`, `CodeViewHost`, `EvalController` and
  the formatter;
- the `VimController` installed through `InputController.setVim`;
- the status row (`mountVimStatus`);
- the transport toggle (the `vim` option of `TransportBar`);
- the cursor shape passed to `renderer.render`.

It also adds the design 8.2 integration tests, through the real
`InputController` and `mount()`, and extends the style probe. With the
setting off (the default), `mount()` must behave exactly as today. The
common execution rules in `vim-mode-00-contract.md` apply; this wave runs
alone, so use the full `npm run check`.

## Non-Goals

- No changes to `vim/` core files, `input.ts`, `surface.ts`,
  `renderer.ts`, `cursor-shape.ts`, `transport*.ts` or
  `vim-status-view.tsx`. If one is needed, record it as a cross-plan repair
  with a regression test; those files are `sharedPaths`. The regression
  test goes in a declared shared test location:
  - `editor/test/code/vim/` for `vim/` repairs;
  - `editor/test/canvas/input-vim-hook.test.ts` (or the owned
    `vim-input.test.ts`) for `input.ts`/`surface.ts` repairs;
  - `editor/test/canvas/cursor-shape.test.ts` for `cursor-shape.ts`
    repairs.

  Append cases only; never edit or weaken existing ones.
- No e2e and no README (wave 4).
- Do not change existing assertions in `test/canvas/mount.test.ts` or any
  other existing test.

## File-Level Changes

### `editor/src/code/vim-mount.ts` (new, at most 250 lines)

```ts
export interface VimDeps { surface: CodeSurface; input: InputController; viewHost: CodeViewHost; layout: TextLayout; evalCtl: EvalController; formatter: () => Formatter | undefined; onChange: () => void }
export function createSurfaceHost(deps: VimDeps): VimHost
export interface VimCursor { cursor: number; cursorShape?: CursorShape; cursorEnd?: number | null }
export interface VimMode { enabled: Accessor<boolean>; toggle(): void; connect(deps: VimDeps): void; cursor(defaultCursor: number): VimCursor; controller(): VimController | null; dispose(): void }
export function createVimMode(storage: StorageLike | null, hostEl: HTMLElement): VimMode
```

**`createSurfaceHost` fields:**

| Field | Implementation |
|---|---|
| `state` | `() => surface.state` |
| `dispatch` | `(s) => surface.dispatch(s)` |
| `subscribe` | `(cb) => surface.subscribe(cb)` (same update shape) |
| `undo` / `redo` | `surface.undo()` / `surface.redo()` |
| `historyReduced` | `() => surface.retentionStatus.reducedDepth` |
| `viewport` | `lh = layout.font.lineHeight`, `v = viewHost.viewport`; `topLine = Math.ceil(v.scrollTop / lh) + 1`, clamped to `surface.state.doc.lines`; `rows = max(1, floor(v.height / lh))` |
| `scrollLines(n)` | `viewHost.scrollBy(0, n * lh)` |
| `scrollCaret` | `viewHost.scrollCaret()` |
| `clipboard` | `browserClipboard()` |
| `actions` | `evalAtCursor: () => void deps.evalCtl.evalAtCursor()`, `evalAll: () => void deps.evalCtl.evalAll()`, `hush: () => deps.evalCtl.hush()`, `format: () => { const f = deps.formatter(); if (f) void formatDocument(surface, f); }` |
| `closePopupWithEscape` | `() => surface.runKeymaps(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }), 'highest')` |
| `announce` | `(m) => input.accessibility.announce(m)`. The status row's `role=alert` carries assertive errors (VIM-17) |
| `platform` | `/Mac\|iP/.test(navigator.platform) ? 'apple' : 'other'`, the same test as `surface.ts` `matchesKey` |
| `matchesDefaultKeymap` | `(i) => surface.matchesKeymap(i, 'default')` |
| `compositionActive` | `() => surface.compositionRange !== null` |

**`createVimMode`:**

- It reads `readVimEnabled(storage)` into a Solid `createSignal`.
- `toggle()` flips the signal and calls `writeVimEnabled`. If connected,
  it enables or disables, then calls `deps.input.focus()`.
- `connect(deps)` stores `deps` and enables if the signal is true.
- **Enable:**
  1. `controller = new VimController(createSurfaceHost(deps))`;
  2. `deps.input.setVim(controller)`;
  3. `hostEl.dataset.vim = 'on'`;
  4. mount the status row with a snapshot signal fed by
     `controller.subscribe`. Every snapshot change also calls
     `deps.onChange()`. The status handlers are:
     - `submit`: `controller.submitPrompt(t)`, then `deps.input.focus()`;
     - `cancel`: `controller.cancelPrompt()`, then `deps.input.focus()`;
     - `tapMode`: `controller.tapMode()`, then `deps.input.focus()`.
- **Disable:**
  1. `deps.input.setVim(null)`;
  2. `controller.dispose()`;
  3. dispose the status row;
  4. `delete hostEl.dataset.vim`;
  5. `deps.onChange()`.

  The document and selection are not touched.
- `cursor(defaultCursor)`:
  - with no controller, or while `deps.input.isComposing`, it returns
    `{ cursor: defaultCursor }` only, without the shape keys, so the
    renderer feedback is unchanged when off;
  - otherwise it returns
    `{ cursor: snap.displayCursor, cursorShape: snap.cursorShape, cursorEnd: snap.displayCursorEnd === snap.displayCursor ? null : snap.displayCursorEnd }`.
- `dispose()` disables and stops the subscriptions.

### `editor/src/code/mount.ts` (at most about 15 changed lines)

1. After `hostEl` is created:
   `const vim = createVimMode(localStore(win), hostEl);`.
2. In the `TransportBar` options, add
   `vim: { enabled: vim.enabled, toggle: () => vim.toggle() }`.
3. After `pointer` is created (the input and view host exist by then):
   `vim.connect({ surface, input, viewHost, layout, evalCtl, formatter: () => deps.formatter, onChange: () => scheduler?.invalidateText('selection') })`.
4. In both `renderer.render({...})` calls, replace the `cursor:` entry
   with `...vim.cursor(<the existing cursor expression>)`. The existing
   cursor expressions are `annotations.cursor` and
   `input.presentation.cursor`.
5. In `mounted.dispose()`, call `vim.dispose()` before `input.dispose()`.

### Tests

**`editor/test/canvas/vim-input.test.ts`** (new, at least 25 cases). This
is the design 8.2 list through the real `InputController` plus a real
`VimController`. Build them the way `setup()` in
`test/canvas/input.test.ts` does (copy the helpers), plus
`createSurfaceHost` with a stub `CodeViewHost`-like object (only
`viewport`, `scrollBy` and `scrollCaret` are used; cast through a minimal
interface). Register real eval-like and format bindings with
`surface.addKeymap([...], 'default')` using spies. The cases are:

- **Default off** (`setVim` never called): typing, composition commit and
  `Ctrl-Enter` give the same results as `input.test.ts`.
- **Normal mode:**
  - printable keys, `Tab` and `beforeinput insertText` -> doc unchanged;
  - `keyCode 229` keydown plus a full composition -> doc and revision
    unchanged, no preedit;
  - a composition started in normal mode that ends after `i` is still
    discarded.
- **Insert mode:** the Japanese composition commits once, as one undo
  group.
- **Escape precedence:** with a highest `Escape` binding returning true
  once (the popup), Escape stays in insert; the next Escape gives normal.
  `Ctrl-[` does the same.
- **Platform-parameterized shortcuts** (`Linux x86_64` and `MacIntel`, in
  every mode): `Ctrl`/`Meta` `Enter`, `Shift-Enter` and `.` reach the
  default bindings or the `KeyboardController` fallbacks
  (`evalSelection`/`evalAll`/`hush` spies). `Ctrl-a` selects all (normal
  -> visual), `Ctrl-z` undoes, `Ctrl-y` redoes, and `Shift-Alt-f` reaches
  the format spy.
- **No-edit chords in non-insert modes, both platforms:**
  `Alt-Backspace`, `Meta-Backspace`, `Ctrl-Backspace`, `Alt-Delete`,
  `Meta-Delete`, `Tab`, `Shift-Tab` -> doc unchanged. In insert mode,
  `Alt-Backspace` deletes a word, as with Vim off.
- **Replace mode:** overwrite, and `Backspace` restores.
- **Control chords:** `Ctrl-r` redoes a unit and `Ctrl-d` calls `scrollBy`.

**`editor/test/canvas/vim-mount.test.ts`** (new, at least 10 cases). Copy
the minimal `mount()` setup from `test/canvas/mount.test.ts` (canvas
fakes, rafHost, deps):

- Storage `vactr.vimMode` absent -> no `.vact-vim-status`, no
  `data-vim`, and the toggle shows `aria-pressed=false`.
- `'true'` -> row present, `data-mode='normal'`, `data-vim='on'`.
- Clicking `.vact-vim-toggle` -> writes `'true'`/`'false'` and adds or
  removes the row.
- `getItem` or `setItem` throwing -> mount still works.
- A `CanvasRenderer.prototype.render` spy receives
  `cursorShape: 'block'` in normal mode and `'bar'` in insert mode, and
  has no `cursorShape` key when off.
- `Ctrl-Enter`/`Meta-Enter` (per platform) in normal mode sends an eval
  through the `RecordingTransport`.
- Dispose with Vim on leaves no listeners or DOM behind.

**`editor/test/style/ui-style.mjs`** (extend; keep all existing checks):

For each engine and viewport, after the existing pass, run a second pass
named `<viewport>-vim`:

1. a new context with
   `addInitScript(() => localStorage.setItem('vactr.vimMode', 'true'))`;
2. `goto`, then wait for `.vact-vim-status`;
3. focus `.vact-code-input-bridge textarea`, press `:`, and wait for
   `.vact-vim-ex`;
4. add the probe and call the same `inspectPage`;
5. take a screenshot `<engine>-<viewport>-vim@2x.png`.

Results go into the same `results`/`failures`. Also record whether
`.vact-vim-toggle` was present in the first pass. The script is only
syntax-checked here; it runs in VIM-40.

## Invariants

- With the setting off, `test/canvas/mount.test.ts`,
  `test/canvas/first-viewport.test.ts`, `test/canvas/gpu.test.ts`,
  `test/canvas/input.test.ts` and `test/ui/controls.test.ts` pass
  unchanged.
- `mount.ts` stays under 420 lines and `vim-mount.ts` under 250.

## Acceptance Criteria

- [x] `vim-mount.ts` exports exactly the API above, and `mount.ts` is
      wired in the five places listed.
- [x] At least 35 new tests pass in the two new files.
- [x] `npm run check` exits 0.
- [x] `node --check test/style/ui-style.mjs` exits 0.

## Verification

1. `npm run check` must exit 0.
2. `./node_modules/.bin/vitest run test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts test/canvas/cursor-shape.test.ts test/canvas/mount.test.ts test/canvas/input.test.ts test/canvas/first-viewport.test.ts test/canvas/gpu.test.ts test/ui test/code/vim`
   must exit 0 with 0 failed. Record the counts.
3. `node --check test/style/ui-style.mjs` must exit 0.

## Progress Log

### Session: 2026-10-17 VIM-30 implementation

**Tasks Completed**: Adapter and mount wiring, real input and mount tests, style-probe second pass, assigned verification.

**Changes**:
- Added `editor/src/code/vim-mount.ts` with the planned `VimDeps`, `VimCursor`, `VimMode`, `createSurfaceHost` and `createVimMode` APIs.
- Wired persisted opt-in state, status row, transport toggle, controller lifecycle, cursor feedback and enabled-only KeyboardController eval/hush fallbacks into `editor/src/code/mount.ts`.
- Added 27 `vim-input.test.ts` integration cases and 13 `vim-mount.test.ts` cases; final run reports 40/40 across both files.
- Extended `editor/test/style/ui-style.mjs` to inspect and screenshot the Vim ex-row pass per engine and viewport, while recording first-pass toggle visibility.
- Fixed adapter keymap matching to pass DOM key/modifier fields explicitly. A spread of the keyboard event failed to preserve the shortcut fields required for the registered Shift-Alt-f binding.

**Verification**:
- `cd editor && npm run check` -> exit 0 (`tmp/vim-mode/VIM-30/npm-check-step6-final-04.log`).
- `cd editor && ./node_modules/.bin/vitest run test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts test/canvas/cursor-shape.test.ts test/canvas/mount.test.ts test/canvas/input.test.ts test/canvas/first-viewport.test.ts test/canvas/gpu.test.ts test/ui test/code/vim` -> exit 0, 30 files and 983/983 tests (`tmp/vim-mode/VIM-30/vitest-step6-final-04.log`).
- `cd editor && node --check test/style/ui-style.mjs` -> exit 0 (`tmp/vim-mode/VIM-30/node-check-style-step6-final-02.log`).
- File budgets: `vim-mount.ts` 121 lines; `mount.ts` 391 lines.
- Initial scoped run failed 18 assertions across the two new files due to an incomplete mount fixture, incorrect expectations for existing Vim behavior, and the adapter's missed Shift-Alt-f binding match. Corrected the fixture, assertions and adapter matching, then added mounted alternate-modifier fallback coverage; final source matched run passes. Full initial log: `tmp/vim-mode/VIM-30/vitest-step6-01.log`.

**Downstream**: VIM-40 owns full Vitest, style execution and Playwright e2e; formal test-integrity, adversarial review, integration, docs indexing, commit and push remain downstream workflow steps.

### Session: 2026-10-17 VIM-30 current-tree verification

**Tasks Completed**: Re-ran the assigned VIM-30 checks against the current shared tree; no VIM-30 source edits were needed.

**Verification**:
- `cd editor && npm run check` -> exit 0 (`tmp/vim-mode/VIM-30/npm-check-step6-current-02.log`).
- `cd editor && ./node_modules/.bin/vitest run test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts test/canvas/cursor-shape.test.ts test/canvas/mount.test.ts test/canvas/input.test.ts test/canvas/first-viewport.test.ts test/canvas/gpu.test.ts test/ui test/code/vim` -> exit 0, 30 files and 1005/1005 tests (`tmp/vim-mode/VIM-30/vitest-step6-current-01.log`). The run held `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock` and released it on exit.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts` -> exit 0, 2 files and 40/40 new integration tests (`tmp/vim-mode/VIM-30/vitest-new-tests-step6-current-01.log`). The run held and released the measurement lock.
- `cd editor && node --check test/style/ui-style.mjs` -> exit 0 (`tmp/vim-mode/VIM-30/node-check-style-step6-current-02.log`).

### Session: 2026-10-17 VIM-30 API contract alignment

**Tasks Completed**: Matched the exported `createVimMode` storage parameter to the planned `StorageLike | null` type; runtime behavior is unchanged.

**Verification**:
- `cd editor && npm run check` -> exit 0 (`tmp/vim-mode/VIM-30/npm-check-step6-current-03.log`).
- `cd editor && node --check test/style/ui-style.mjs` -> exit 0 (`tmp/vim-mode/VIM-30/node-check-style-step6-current-03.log`).
- The required current-source Vitest rerun is pending. Lock acquisition returned exit 98 because `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock` is held by an active Rust nextest process (observed with `ps`); the lock was unchanged for over two minutes. Earlier 1005/1005 and 40/40 Vitest results apply to the immediately preceding source and are retained as prior verification, not as current-source passes.

### Session: 2026-10-17 VIM-30 final-source verification after lock release

**Tasks Completed**: Re-ran the required VIM-30 suites after the final `StorageLike | null` API alignment. Waited for the existing lock owner to finish, acquired the measurement lock, ran both suites, and released the lock.

**Verification**:
- `cd editor && ./node_modules/.bin/vitest run test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts test/canvas/cursor-shape.test.ts test/canvas/mount.test.ts test/canvas/input.test.ts test/canvas/first-viewport.test.ts test/canvas/gpu.test.ts test/ui test/code/vim` -> exit 0, 30 files and 1005/1005 tests (`tmp/vim-mode/VIM-30/vitest-final-source-02.log`). Start time 07:22:43; final source `editor/src/code/vim-mount.ts` mtime 07:02:38.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts` -> exit 0, 2 files and 40/40 tests (`tmp/vim-mode/VIM-30/vitest-new-tests-final-source-02.log`). Start time 07:22:49.
- `cd editor && npm run check` -> exit 0 (`tmp/vim-mode/VIM-30/npm-check-step6-final-source-03.log`).
- `cd editor && node --check test/style/ui-style.mjs` -> exit 0 (`tmp/vim-mode/VIM-30/node-check-style-step6-final-source-02.log`).
- SHA-256 values for all five VIM-30 owned source/test files are recorded in `tmp/vim-mode/VIM-30/source-hashes-final-source-01.json`; `vim-mount.ts` matches the reviewed `840a1bdc…` hash. No VIM-30 source change was needed. The prior lock-acquisition failure remains historical evidence and was resolved by waiting for the lock owner to finish.
