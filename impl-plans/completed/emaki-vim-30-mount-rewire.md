# EVM-30: vim-mount rewired onto createVimController, and mount-level tests adapted

**Status**: Ready
**planId**: EVM-30. **Wave**: 3. **dependsOn**: EVM-20 and EVM-21.
**Design reference**: `design-docs/specs/design-emaki-vim-migration.md`
VM-D1, D2, D3, D4, D5, D6, D7 and D9, section 4 (data flow) and section 7.
**Rules**: `impl-plans/active/emaki-vim-00-rules.md`.

## Intent and context

`editor/src/code/vim-mount.ts` builds the local `VimController` from
`createSurfaceHost(deps)`. This plan replaces that with emaki's
`createVimController(host, options)` and keeps every vactr behavior that has
a public emaki API.

The local engine directory still exists during this plan, and EVM-40
deletes it. After this plan, no `src/` file may import `./vim/*`.

## Non-goals

- Do not delete `src/code/vim/**` or `test/code/vim/**`, and do not touch
  `test/documents/session.test.ts`. Those belong to EVM-40.
- Do not edit `test/canvas/vim-parity.test.ts`.
- Do not work around VMB1 to VMB4. In particular:
  - no local `normalizeKey`;
  - no key filter for meta keys;
  - no forcing of normal mode with synthetic Escape keys;
  - no recreation of the controller on a file boundary.

## Contracts pinned for EVM-40 (exports of `vim-mount.ts`)

- `export function createVimHost(deps: Pick<VimDeps, 'surface' | 'viewHost' | 'layout'>): VimHost`
- `export function createVimOptions(deps: VimDeps): VimEngineOptions`
- `export function forwardSurfaceUpdates(surface: CodeSurface, vim: VimController): () => void`

  It subscribes with `surface.subscribe`. For each update it calls
  `vim.noteUpdate(update)` (with the `userEvent` field passed through). Then,
  if `isFileBoundary(update.userEvent)` (from `./file-boundary`), it calls
  `vim.blur()`. It returns the unsubscribe function.

`createSurfaceHost` is removed. `VimDeps`, `VimKeymapDeps`, `VimMode`,
`VimCursor` and `createVimMode` keep their names. `VimKeymapDeps.store` is
emaki `KeymapStore`. `VimMode.controller()` returns emaki
`VimController | null`.

## File-level changes

### `editor/src/code/vim-mount.ts`

Imports from `'emaki/vim'`: `createVimController`, `compileKeymap`,
`KEYMAP_PRESETS`, `LocalKeymapStore`, and the types `KeymapStore`,
`VimController`, `VimHost`, `VimEngineOptions`, `VimSnapshot`, `HopLabel`.
Import `migrateLegacyActionIds` and `TauriKeymapStore` (if needed) from
`./vim-keymap-store`. Import `isFileBoundary` from `./file-boundary`.

**`createVimHost`** (VM-D1 table). Move today's `createSurfaceHost`
`viewport()` arithmetic into the getter:
- `topLine = clamp(ceil(scrollTop / lineHeight) + 1)`
- `rows = max(1, floor(height / lineHeight))`
- return `{ fromLine: topLine, toLine: topLine + rows - 1, top: scrollTop, height }`.

This makes emaki's shim derive exactly the same `{topLine, rows}`. Then:
- `state`, `dispatch`, `undo`, `redo` delegate to `surface`.
- `scrollTop` is the getter `viewHost.viewport.scrollTop`.
- `scrollTo({top})` calls `viewHost.scrollBy(0, top - scrollTop)`.
  `scrollTo({pos})` scrolls to that position's line top, using uniform
  line-height arithmetic.
- `lineTop` and `lineAtHeight` use the same arithmetic.
- `coordsAtPos(pos)` returns `layout.coordsAtPos(pos, viewHost.viewport)`.
- `marks()` returns `{ set() {}, clear() {} }`.

**`createVimOptions`**:
- `hooks`: copy today's `createSurfaceHost` members unchanged —
  `announce`, `closePopupWithEscape`, `platform`, `matchesDefaultKeymap`,
  `compositionActive`, `isVisible`, `historyReduced`, `scrollLines`,
  `scrollCaret`, `typeText`, `defaultKey`. The only rename is
  `historyReduced: () => surface.retentionStatus.reducedDepth`.
- `actions` (VM-D2):
  - `'emaki.evaluate'`, `'emaki.evaluateAll'`, `'emaki.hush'`,
    `'emaki.format'` and `'emaki.completion'` use today's bodies and return
    `undefined`. Use a block body; `void evalCtl.evalAtCursor()` must not
    leak a Promise as a return value.
  - `'emaki.diagnostic.next'` and `'emaki.diagnostic.prev'` are
    `ctx => diagnosticPosition(ctx.cursor, ±1)`, returning `number | null`.
- `ex` (VM-D5): for `cmd.name` in `{w, q, wq, x}`, call
  `deps.input.accessibility.announce('vactr syncs the document live; use :eval, :evalall or :hush')`
  and return `true`. Return `false` for everything else.
- `clipboard: browserClipboard()`.
- `timer: (ms, cb) => { id = win.setTimeout(cb, ms); return () => win.clearTimeout(id) }`.
- `render: { search: false, hints: false, hop: false }`.
- Do not pass `keymap` or `preset`.

**`createVimMode`**:
- `enable()` calls
  `vim = createVimController(createVimHost(deps), createVimOptions(deps))`,
  then `vim.setKeymap(compiledSignal)`, then `deps.input.setVim(vim)`, then
  `stopUpdates = forwardSurfaceUpdates(deps.surface, vim)`.
- `disable()` calls `stopUpdates()` **before** `vim.dispose()`.
- The snapshot subscription is unchanged: `vim.subscribe(next => ...)`.
- `compile = text => compileKeymap(migrateLegacyActionIds(text))` (VM-D3).
  This is the only compile path. The panel text and stored text stay
  as written.
- Store calls:
  - `load()` replaces `get()`. Keep the catch: "keymap not loaded: storage
    unavailable".
  - `save(text)` replaces `set(text)`. Status strings:
    - `ok` -> `'saved'`
    - `too-large` -> `'keymap not saved: too-large'`
    - `unavailable` -> `'keymap not saved: storage unavailable'`
    - a rejection -> `String(error)`
  - `clear()` replaces `reset()`. On resolve **and** on reject, run today's
    UI reset: default preset text, empty issues, active preset `default`,
    `apply(null)`. The status is `'reset'` on resolve and `String(error)` on
    reject.
- The fallback store is
  `new LocalKeymapStore({ key: 'vactr.vimKeymap', storage: localStorageFor(win) ?? undefined })`.
- `mountVimStatus(hostEl, snapshot, handlers)` is called **without** the
  `resolveCmdline` argument (VM-D9 and VMB4). Remove the `normalizeKey`
  import and the `compiledSignal?.cmdline` resolver. Do not substitute
  anything.
- `VimCursor.cursorShape` is typed `VimSnapshot['cursorShape']`.
  `cursor()` and `overlay()` logic is unchanged.

### `editor/src/app/main.ts`

`LocalKeymapStore` comes from `'emaki/vim'` and is constructed as
`new LocalKeymapStore({ key: 'vactr.vimKeymap', storage: vimStorage ?? undefined })`.
`TauriKeymapStore` still comes from `../code/vim-keymap-store`.

### Tests (adapt imports and API only; keep every assertion)

- `editor/test/canvas/vim-input.test.ts` and
  `editor/test/canvas/vim-keymap-input.test.ts`:
  - `new VimController(createSurfaceHost(deps))` becomes
    `createVimController(createVimHost(deps), createVimOptions(deps))`,
    plus `forwardSurfaceUpdates(surface, vim)`, disposed in cleanup.
  - `compileKeymap` and `KEYMAP_PRESETS` come from `'emaki/vim'`.
  - Assertions on compiled action IDs change `vactr.*` to `emaki.*`.
  - Assertions on stored or panel text keep `vactr.*`.
- `editor/test/canvas/input-vim-hook.test.ts` and
  `editor/test/canvas/input-keymap-hooks.test.ts`: the `VimInputHook` type
  becomes `ModalKeyHook` from `'emaki/vim'`.
- `editor/test/canvas/vim-keymap-mount.test.ts`:
  - `KEYMAP_MAX_BYTES`, `KEYMAP_PRESETS` and `LocalKeymapStore` come from
    `'emaki/vim'`.
  - `VIM_KEYMAP_KEY` becomes the literal `'vactr.vimKeymap'`.
  - Fake stores implement `load/save/clear`.
  - A fake that used to return `{ok:false, reason:'error', message}` now
    rejects with `new Error(message)`. Expected status texts stay identical
    whenever the old text was `message`. If an expected text equalled
    `String(error)` of a non-Error value, keep the text by rejecting with
    that value.
- `editor/test/canvas/vim-mount.test.ts`: edit only if an import breaks.

Pitfalls:

- emaki's facade ignores `noteUpdate` while the engine itself dispatches.
  Confirm in `editor/src/code/surface.ts` that `publish` runs synchronously
  inside `dispatch`, and write one sentence of evidence in the log. If it is
  deferred, report `BLOCKER-SYNC-PUBLISH`; do not add dedup logic.
- `createVimController` without `options.timer` creates its own scheduler.
  Always pass the window timer so that fake timers in tests still work.
- `mise-darwin` preset text now shows `emaki.*` IDs. That is accepted
  (design section 8).

## Expected results after this plan

- `npm run check` exits 0. The local engine still exists, so `test/code/vim`
  and `session.test.ts` still compile.
- `npx vitest run` failures are exactly:
  - the parity cases blocked by VMB1 (P1a, P1b), VMB4 (P2a, P2b), VMB2 (P3),
    and P4 if VMB3 is real;
  - plus any `BASE-FAIL` entries recorded by EVM-10.

  Any other failure is a defect in this plan and must be fixed within the
  owned paths, or reported.

## Acceptance criteria

- [ ] `grep -nE "'\./vim/|code/vim/" editor/src -r` returns nothing.
- [ ] `vim-mount.ts` exports `createVimHost`, `createVimOptions` and
      `forwardSurfaceUpdates` with the pinned signatures.
- [ ] `npm run check` exits 0.
- [ ] `npx vitest run test/canvas` has failures only in
      `vim-parity.test.ts`, each mapped to VMB1 to VMB4 in the log, plus
      any BASE-FAIL entries.
- [ ] No assertion is removed or relaxed, as shown by the `git diff` check
      below.
- [ ] The progress log is updated.

## Verification commands (run in `editor/`)

| Command | Evidence |
|---|---|
| `npm run check` | exit 0 |
| `npx vitest run test/canvas test/code/vim-keymap-store.test.ts test/ui` | the failure list equals the parity-blocker list plus BASE-FAIL; each failing test name is listed with its VMB ID |
| `npx vitest run test/canvas/vim-parity.test.ts` | log kept as blocker evidence (expected non-zero exit) |
| `git diff -U0 -- test/canvas \| grep -E "^-.*expect\("` | every removed line has a re-added counterpart that differs only in import, API name or compiled-ID literal |

## Progress Log
