# Vim Keymap: Mount Wiring, Stores and Integration Tests Implementation Plan

**Status**: Implementation complete; prescribed focused suite has a VK-12-owned timeout, serial integration pending
**Plan ID**: VK-30 (wave 3)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 4.3 (adapter actions), 4.4, 6.4, 7.2, 7.4, 8.1-8.5, 11.2
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-20 (controller), VK-02 (Tauri commands), VK-03 (stores, JSON files), VK-04 (renderer fields), VK-14 (presets), VK-15 (input hooks), VK-16 (views)
- **Next**: VK-40 e2e and gates, VK-41 docs
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full; full `npm run check`)

## Intent and Context

Connect the core to the editor: real `VimHost` members (replacing the
VK-00 stubs), keymap loading/saving/applying, the hint panel, the
keymap settings panel with presets and import/export, `c`-mode
resolution in the ex row, hop labels to the renderer, blur handling, and
the store choice in `app/main.ts`. Imitate the existing structure of
`editor/src/code/vim-mount.ts:createVimMode` and `createSurfaceHost`.

## Non-Goals

- No changes to `vim/` core files, `renderer.ts`, `input.ts`, the views
  or the stores beyond calling them.
- No Tauri capability change; no new dependency.

## File-Level Changes

### editor/src/code/vim-mount.ts (shared; replaces the VK-00 stubs)

- `VimDeps` gains optional `diagnostics?: { current(): readonly { from: number }[] }`
  (optional so `test/canvas/vim-input.test.ts` compiles unchanged).
- `createSurfaceHost`: `typeText` -> `input.typeText(text)`;
  `defaultKey` -> `input.defaultKey(key)`; `isVisible(pos)` ->
  `layout.coordsAtPos(pos, view)` non-null and inside the view host
  viewport rectangle (use the same `viewHost.viewport` data the
  `viewport()` member uses); `timer(ms, cb)` -> the window's
  `setTimeout`/`clearTimeout`; `actions.diagnosticNext/Prev(from)` ->
  sorted unique `from` values of `deps.diagnostics?.current()`, first
  greater (next) or smaller (prev) than `from` with wrap, null when none;
  `actions.openCompletion` -> `surface.runKeymaps(new KeyboardEvent('keydown', { key: ' ', code: 'Space', ctrlKey: true, cancelable: true }), 'highest')`
  (verify against `completion-popup.ts` `Ctrl-Space` matching; adjust the
  event fields so the existing matcher accepts it).
- `createVimMode(storage, hostEl, keymap?: { store: KeymapStore; files: JsonFiles })`:
  defaults to `new LocalKeymapStore(storage)` and `new BrowserJsonFiles(win)`.
  On `connect`, load `store.get()`; compile with `compileKeymap`; keep
  signals `keymapText`, `issues`, `compiled`, `panelOpen`, `activePreset`,
  `panelStatus`. When the controller exists (enable) call
  `controller.setKeymap(compiled)`; when compiled changes later, call it
  again (live apply). A null compile result means K0 plus the issues.
  Load-time issues post one status message `keymap: N entries skipped`
  (via the controller message path or `announce`).
- Mount `mountVimHints(hostEl, () => snapshot().hints ?? null)` and
  `mountVimKeymapPanel(hostEl, props)` on enable, dispose on disable.
  Pass `openKeymap` and `resolveCmdline` to `mountVimStatus`;
  `resolveCmdline(input)` -> `normalizeKey(input, true)` looked up in
  `compiled()?.cmdline` (null when absent).
- Panel props: presets from `KEYMAP_PRESETS`; `onPreset` replaces the text
  with the preset text and sets `activePreset`; `onApply` compiles: issues
  shown; when `keymap` non-null, `store.set(text)` then apply live, status
  `saved` or the store failure (`keymap not saved: storage unavailable`,
  `too-large`, error message); `onReset` -> `store.reset()`, text = Default
  preset text, apply `null` (K0); `onImport` -> `files.importJson()` into
  the text (not applied; `FileCancelled` ignored; `JsonFileTooLarge` ->
  status); `onExport` -> `files.exportJson('vactr-vim-keymap.json', text)`;
  `onClose` -> close and `input.focus()`.
- `VimMode` gains `overlay(): { hopLabels?: readonly HopLabel[]; hopRevision?: number }`
  returning the snapshot's hop view or `{}`.
- Blur: listen to `focusout` on the input bridge container while enabled
  and call `controller.blur()` (remove the listener on disable).

### editor/src/code/mount.ts (shared)

- Pass `deps.vimKeymap` (when present) to `createVimMode`.
- Pass `diagnostics` to `vim.connect({...})`.
- In both `renderer.render({...})` calls add `...vim.overlay()`.

### editor/src/app/deps.ts (shared)

`EditorDeps` gains optional `vimKeymap?: { store: KeymapStore; files: JsonFiles }`.

### editor/src/app/main.ts (shared)

In `boot`, set `deps.vimKeymap = isTauri(win) ? { store: new TauriKeymapStore(invoke), files: new TauriJsonFiles() } : { store: new LocalKeymapStore(<page storage>), files: new BrowserJsonFiles(win) }`
before `createEditor`. Use the same `invoke` already imported for
`TauriTemplateStore` and the same page storage helper used elsewhere in
`main.ts` (`pageStorage`).

## Invariants

- With no stored keymap, the editor behaves exactly as the merged Vim
  mode: no hint panel, no hop labels, `setKeymap(null)`.
- Vim off: no keymap UI in the DOM; `vim-mount.test.ts` passes unchanged.
- Existing mount and canvas tests pass unchanged.

## Test Cases

`editor/test/canvas/vim-keymap-input.test.ts` (through `InputController` with jsdom, imitate `vim-input.test.ts` setup; compile the mise-darwin preset and call `setKeymap`):
- insert `kj` -> leaves insert, no text
- insert `k`, advance timers past timeoutlen -> `k` inserted with userEvent `input.type`
- insert `k` then a Japanese composition (`compositionstart`/`update`/`end` with `日本`) -> doc `k日本`, composition committed once as one undo group
- normal mode composition with a keymap -> discarded (doc unchanged)
- ex row `c` maps: Ctrl-g cancels, Ctrl-a moves the caret to 0 (through `resolveCmdline`)
- a keymap trying `<C-a>` in n -> compile error, and `Ctrl-a` still selects all

`editor/test/canvas/vim-keymap-mount.test.ts`:
- status row has the `keymap` button; click opens the panel
- preset button `mise-darwin` fills the textarea; Apply with a broken entry shows the issue path and saves nothing; valid Apply writes `vactr.vimKeymap` and applies (insert `kj` escapes)
- Reset removes the key and restores K0 (`gj` no longer starts hop)
- throwing storage -> status `keymap not saved: storage unavailable`, still applied for the session
- hint panel appears after 200 ms of `<Space>` with the summary text, closes on `Escape`
- hop: `gj` + char -> `vim.overlay().hopLabels` non-empty and `render()` receives them
- `LocalKeymapStore` byte limit equals `KEYMAP_MAX_BYTES` (import both constants and compare)
- `focusout` closes the hint panel

## Verification (from editor/)

1. `npm run check` -> exit 0
2. `./node_modules/.bin/vitest run test/canvas test/code/vim test/code/vim-keymap-store.test.ts test/platform` -> pending: three attempts report 1536/1537 passing; `test/code/vim/hop.test.ts:45` times out at 5000 ms under aggregate load. The isolated file passes 23/23 and the VK-30 integration files pass 18/18. Final aggregate evidence is `tmp/vim-keymap/VK-30/attempt-1/focused-vitest-serialized.log`.

## Completion Criteria

- [x] No VK-00 stub remains in `vim-mount.ts` (`rg -n 'VK-30' src/code/vim-mount.ts` returns nothing)
- [x] Store choice in `main.ts`; `EditorDeps.vimKeymap` optional
- [x] VK-30 integration tests pass (18/18); existing canvas tests pass in the prescribed focused run
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-30 implementation

**Tasks completed**: Mount adapter and store wiring; settings and hint views; c-mode and hop overlay integration; 18 VK-30 integration cases.

**Notes**: `vim-mount.ts` now uses the real `InputController` insert/default-key paths, viewport-clipped visibility and window timers; diagnostics navigate sorted unique positions with wrap; completion invokes the existing Ctrl-Space keymap. Keymap load, live apply, reset, import/export, status and panel callbacks use the accepted stores and views. `mount.ts` passes the optional keymap adapters and diagnostics, then forwards hop overlay fields to both render paths. `main.ts` selects Tauri adapters or web localStorage/file adapters through `pageStorage`. No VK-00 marker remains in `vim-mount.ts`.

**Verification**:
- `{command: "cd editor && npm run check", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-30/attempt-1/npm-check-final.log"}`.
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/vim-keymap-input.test.ts test/canvas/vim-keymap-mount.test.ts", exitStatus: 0, testsRun: 18, testsPassed: 18, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-30/attempt-1/new-integration-final.log"}`.
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/hop.test.ts", exitStatus: 0, testsRun: 23, testsPassed: 23, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-30/attempt-1/hop-isolation.log"}`.
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas test/code/vim test/code/vim-keymap-store.test.ts test/platform", exitStatus: 1, testsRun: 1537, testsPassed: 1536, failureCount: 1, outcome: "blocked by VK-12 hop target cap timeout under aggregate load", log: "tmp/vim-keymap/VK-30/attempt-1/focused-vitest-serialized.log"}`. Three attempts, including under the measurement lock; the sole failure is `test/code/vim/hop.test.ts:45` timing out at 5000 ms, while that file passes alone. This test is outside VK-30 write paths and is not changed here.

**Next**: Serial integration must clear the exact prescribed focused suite on the combined tree; VK-40 owns e2e/style/build gates and VK-41 owns docs. Formal test-integrity and adversarial review, documentation index updates, commit and push remain downstream.
