# vactr Vim on emaki/vim: Engine Removal and Glue Rewire

**Status:** Design (2026-11-11, revised after design review DR-001). Not
implemented. The accepted requirement is full behavior parity. Completion is
blocked until the emaki APIs in section 7 exist, or until the user explicitly
amends the scope (EQ7).

**Issue reference:** No GitHub issue. The request comes from the workflowInput
passed by riela-manager (`comm-019329`), with intake `comm-019330`. The workflow
execution is `opus-terra-design-and-implement-review-loop-session-839`. Title:
"editor: migrate vactr off its private vim engine (delete src/code/vim/**) onto
emaki/vim @ 9dba0a3".

**Branch:** `main`, base `a998d1e`. Commit locally only, do not push.

**Related design.** `design-emaki.md` 4.2 (EM-D17 kept Vim in vactr, and EQ6
deferred the move) is superseded for Vim by this document. Vim behavior is in
`design-vim-mode.md` and `design-vim-keymap-hints-hop.md`. The emaki Vim API is
specified in emaki `design-docs/specs/design-vim.md` 12 (D16 to D24). It is
installed at `editor/node_modules/emaki/src/vim` (emaki `9dba0a3`).

---

## 1. Scope

**In scope**

1. Delete `editor/src/code/vim/**` (25 modules plus `presets/`) and the engine
   test suites under `editor/test/code/vim/` (section 5).
2. Rewire the vactr glue to `emaki/vim` through these public exports:
   `createVimController`, `VimHost`, `VimEngineOptions`, `VimHooks`,
   `VimSnapshot`, `compileKeymap`, `KEYMAP_PRESETS`, `KEYMAP_MAX_BYTES`,
   `KeymapStore`, `LocalKeymapStore` and the related types.
3. Adapt the vactr integration tests to the public API.
4. Record each missing emaki API as a blocker (section 7).

**Non-goals**

- No change to `/Users/taco/gits/tacogips/emaki`. No change to the emaki pin
  in `editor/package.json` or `package-lock.json`. Do not run `npm install`
  or `npm ci`.
- No Rust change. The Tauri commands `vim_keymap_get`, `vim_keymap_set` and
  `vim_keymap_reset` are unchanged.
- No re-implementation of engine logic in vactr (key normalization, ex
  parsing, undo ledger, dot-repeat).
- No adoption of emaki's `vim()` convenience wrapper. It owns its own prompt
  line and cursor marks, which conflicts with vactr's Solid status view and
  renderer cursor.

## 2. Baseline check (repository state at `a998d1e`)

| # | Finding | Evidence |
|---|---|---|
| VB1 | 21 of the 25 vactr engine modules are byte-identical to emaki `src/vim`, and so are `presets/default.json` and `presets/index.ts`. Only `actions.ts`, `controller.ts`, `keymap-schema.ts`, `types.ts` and `presets/mise-darwin.json` differ. emaki adds `adapter.ts`, `cursor.ts`, `host.ts`, `index.ts`, `keymap-store.ts` and `prompt.ts`. | `diff -rq editor/src/code/vim editor/node_modules/emaki/src/vim` |
| VB2 | emaki renames the built-in action IDs from `vactr.*` to `emaki.*` (`evaluate`, `evaluateAll`, `hush`, `format`, `diagnostic.next`, `diagnostic.prev`, `completion`). `ActionId` becomes `string`, and any dotted ID that is not built in and does not start with `hop.` is accepted as a host action in modes n, x and i. | `diff src/code/vim/actions.ts node_modules/emaki/src/vim/actions.ts`, `keymap-schema.ts:171-177` |
| VB3 | Apart from the action branch, the keymap schema (version, sections, size limit) is identical. A stored vactr keymap that uses `vactr.*` IDs still compiles, but those IDs become host actions with no handler, so they do nothing. The engine also no longer shows "no diagnostics" for them, because that message is tied to `emaki.diagnostic.*`. | emaki `controller.ts` `runHostAction` |
| VB4 | `mise-darwin.json` differs only in the five `vactr.*` to `emaki.*` action IDs and five `reason` strings ("vactr" changed to "emaki"). | preset diff |
| VB5 | emaki's controller drops vactr's file-boundary reset. vactr's `onUpdate` clears the undo ledger, insert session, dot-repeat recorder, visual selection and parser, and forces normal mode, when `userEvent` is `input.replace.document` or `input.load.document`. emaki has no equivalent and no public reset. | `diff src/code/vim/controller.ts node_modules/emaki/src/vim/controller.ts` (hunk at 790) |
| VB6 | For `:w`, `:q`, `:wq`, `:x` and their bang forms, emaki calls `options.ex` instead of showing vactr's info message "vactr syncs the document live; use :eval, :evalall or :hush". `ex` returns only a boolean, so it cannot set the status message. If it returns false, the engine shows `E492: Not an editor command: <name>`. | controller diff 654-658, `host.ts` `VimEngineOptions.ex` |
| VB7 | emaki `handleKey` returns `false` for every `metaKey` input. vactr's engine routed Cmd keys through `isPassThrough` and `matchesDefaultKeymap`. | controller diff line 78 |
| VB8 | `normalizeKey`, `VimKey` and `VimKeyInput` are not exported from `emaki/vim`. The package `exports` map has only `./vim` and no deep paths. `vim-mount.ts` and `vim-status-view.tsx` need `normalizeKey` to resolve keymap `cmdline` remaps in the Solid ex/search prompt input. | `node_modules/emaki/src/vim/index.ts:11-18`, `node_modules/emaki/package.json` exports |
| VB9 | The public `VimSnapshot.prompt` is `{ kind, text }`, where vactr's internal snapshot has `{ kind, initial }`. All other fields vactr reads (`label`, `mode`, `pending`, `recording`, `search`, `message`, `messageKind`, `cursorShape`, `displayCursor`, `displayCursorEnd`, `hints`, `hop` including `revision`) are present. `HintView` and `HopView` are identical. | `host.ts`, `types.ts` |
| VB10 | emaki's internal host shim derives `{ topLine, rows }` from `host.viewport.fromLine/toLine`. No hook overrides it. `scrollLines`, `scrollCaret`, `isVisible`, `typeText`, `defaultKey`, `historyReduced` and `compositionActive` are hooks. | emaki `controller.ts` `createVimController` |
| VB11 | emaki draws its own search, hint and hop marks unless `render.search`, `render.hints` and `render.hop` are `false`. vactr shows hints in `vim-hints-view.tsx` and hop labels through renderer `hopLabels`/`hopRevision`, and has no search highlight today. | `vim-mount.ts`, `decorations.ts`, `vim-status-view.tsx` |
| VB12 | vactr's `KeymapStore` is `get/set/reset`, and its result includes `reason: 'error'` with a `message`. emaki's is `load/save/clear`, where `save` returns `too-large \| unavailable` and `clear` returns `void`. emaki's `LocalKeymapStore` accepts `{ key, storage }`. vactr's key is `vactr.vimKeymap`. | `src/code/vim-keymap-store.ts`, emaki `keymap-store.ts` |
| VB13 | `test/code/vim/host.ts` (`createTestHost`) is imported outside the deleted suites by `test/documents/session.test.ts`, which checks that Vim redo cannot cross a file load. | `grep -n vim test/documents/session.test.ts` |
| VB14 | Outside `src/code/vim`, these files import local engine modules: `src/code/{vim-mount.ts, vim-clipboard.ts, vim-hints-view.tsx, vim-keymap-view.tsx, vim-status-view.tsx, decorations.ts}` and `test/{ui/vim-status.test.tsx, canvas/vim-input, vim-hints-view, input-keymap-hooks, vim-status-keymap, vim-keymap-input, input-vim-hook, vim-keymap-mount}.test.ts, documents/session.test.ts}`. `src/app/main.ts` imports the vactr stores. | `grep -rln "code/vim/\|'\./vim/" src test` |
| VB15 | `mise.toml` has no editor tasks. The editor CI surface is the `editor/package.json` scripts: `check`, `test`, `build`, `test:style`, `e2e:examples`, plus `test/e2e/vim-behavior.mjs` and `vim-keymap.mjs`. | `package.json` scripts |

## 3. Decisions

### VM-D1. The VimHost is built from CodeSurface, not from the emaki editor adapter

vactr has no emaki `EditorHandle` or `PluginHost`. `vim-mount.ts` replaces
`createSurfaceHost` with `createVimHost(deps): VimHost` from emaki `host.ts`:

| VimHost member | Source |
|---|---|
| `state`, `dispatch`, `undo`, `redo` | `surface.state`, `surface.dispatch`, `surface.undo()`, `surface.redo()` |
| `viewport` | `{ fromLine: topLine, toLine: topLine + rows - 1, top: viewHost.viewport.scrollTop, height: viewHost.viewport.height }`, with `topLine` and `rows` computed by today's formulas in `createSurfaceHost`. emaki's shim then derives the same `{ topLine, rows }` (VB10). |
| `scrollTop` | `viewHost.viewport.scrollTop` |
| `scrollTo`, `lineTop`, `lineAtHeight`, `coordsAtPos` | Uniform line-height arithmetic and `layout.coordsAtPos(pos, viewHost.viewport)`. These are reached only through emaki's default hooks, which vactr overrides. |
| `marks(owner)` | A no-op `{ set, clear }`. All render switches are off (VM-D4), so the engine never calls it. |

The `VimEngineOptions.hooks` move over from `createSurfaceHost` unchanged:
`announce`, `closePopupWithEscape`, `platform`, `matchesDefaultKeymap`,
`compositionActive`, `isVisible`, `historyReduced`, `scrollLines`,
`scrollCaret`, `typeText` and `defaultKey`.

The other options are:
- `clipboard: browserClipboard()`;
- `timer`: the current `win.setTimeout`/`clearTimeout` pair;
- `actions` from VM-D2;
- `ex` from VM-D5;
- `render` from VM-D4.

`keymap` and `preset` are not passed. The mount calls `setKeymap(compiled)`
after construction, as it does today.

### VM-D2. App actions are registered under the emaki built-in IDs

The `actions` registry maps:

| Action ID | Handler | Return value |
|---|---|---|
| `emaki.evaluate` | `void evalCtl.evalAtCursor()` | `undefined` |
| `emaki.evaluateAll` | `void evalCtl.evalAll()` | `undefined` |
| `emaki.hush` | `evalCtl.hush()` | `undefined` |
| `emaki.format` | `formatDocument` if a formatter exists | `undefined` |
| `emaki.diagnostic.next` | `diagnosticPosition(ctx.cursor, 1)` | `number \| null` |
| `emaki.diagnostic.prev` | `diagnosticPosition(ctx.cursor, -1)` | `number \| null` |
| `emaki.completion` | the existing synthetic Ctrl-Space `runKeymaps` | `undefined` |

Only the two diagnostic handlers may return a number. A number makes the
engine move the cursor, and `null` shows "no diagnostics". emaki's unchanged
`ex.ts` maps `:eval`, `:evalall`, `:run`, `:hush` and `:format` to these IDs.

### VM-D3. Legacy `vactr.*` action IDs are rewritten when keymap text is compiled

Stored keymaps (localStorage `vactr.vimKeymap`, Tauri `vim-keymap.json`) and
imported files can contain the seven `vactr.*` IDs (VB3).

vactr adds one pure function, `migrateLegacyActionIds(text): string`, in
`vim-keymap-store.ts`. It replaces each JSON string token that equals exactly
`"vactr.<id>"`, for the seven IDs, with `"emaki.<id>"`. Formatting and all
other text are kept.

It runs only inside vactr's `compile(text)` wrapper, before
`compileKeymap`. That wrapper is the single path for load, apply and import.
The panel and the store keep the user's text as written, so nothing is
silently rewritten on disk and the panel text does not change.

This restores the old behavior: actions fire, the "no diagnostics" message
returns, and `vactr.diagnostic.*` in insert mode is still rejected by the
mode check. The transform is idempotent.

### VM-D4. Render switches are all off

The controller is created with
`render: { search: false, hints: false, hop: false }`. vactr keeps its own
views:
- block/underline cursor through `snapshot.cursorShape`, `displayCursor` and
  `displayCursorEnd` (`vim-mount.ts` `cursor()`);
- hop labels through `snapshot.hop` passed to renderer `hopLabels` and
  `hopRevision`;
- which-key hints through `vim-hints-view.tsx`.

There is still no search highlight, which matches today.

### VM-D5. The `ex` handler for no-op write commands

**Required behavior (unchanged from today).** For `:w`, `:q`, `:wq`, `:x` and
their bang forms, the status line shows the visible info message "vactr syncs
the document live; use :eval, :evalall or :hush" (`messageKind: 'info'`).

**Wiring.** `ex: (cmd) => boolean` returns `true` for `name` in
`{w, q, wq, x}`, with or without bang. It also announces the same text through
`input.accessibility.announce`, as the old message path did. All other names
return `false`, so the engine shows E492.

**Gap.** With emaki `9dba0a3` this wiring cannot make the status line show the
message, because `ex` cannot return one. That is a parity loss and a
completion blocker (VMB2). The wiring is not a substitute, and parity check P3
(section 9) fails until VMB2 is fixed.

### VM-D6. External transactions reach the engine through `noteUpdate`

The mount subscribes to `surface.subscribe(update => vim.noteUpdate(update))`
and passes `userEvent` (D24). It unsubscribes in `disable()`.

The emaki facade ignores `noteUpdate` while it is dispatching its own
transaction. This needs `CodeSurface` to publish synchronously inside
`dispatch`. The implementer confirms this in `surface.ts`. If publication is
deferred, double notification is a defect to report, not to work around.

**Required behavior (unchanged from today).** When an update's `userEvent` is
a file boundary (`isFileBoundary`: `input.replace.document` or
`input.load.document`), the engine:
- enters normal mode, leaving insert, replace or visual mode;
- clears the visual selection, the parser, the insert session, the dot-repeat
  recorder and the undo ledger.

So `.`, `u` and `<C-r>` never act on the previous file.

**Wiring.** The mount forwards the boundary `userEvent` and calls `vim.blur()`.
`blur()` flushes pending input, ends hop, resets the parser and leaves
operator-pending, but it does not leave insert or visual mode and does not
clear repeat or ledger state (emaki `controller.ts:117-124`).

**Gap.** That is a parity loss and an unconditional completion blocker
(VMB1). It blocks whether or not any existing test fails. Parity check P1
(section 9) fails until VMB1 is fixed.

### VM-D7. Keymap stores implement emaki `KeymapStore`

- **Browser.** `src/app/main.ts` and the `createVimMode` fallback use emaki's
  `new LocalKeymapStore({ key: 'vactr.vimKeymap', storage })`. The key is
  kept, so saved keymaps survive. vactr's `LocalKeymapStore` class is deleted.
- **Tauri.** vactr's `TauriKeymapStore` implements emaki `KeymapStore`:
  - `load()` invokes `vim_keymap_get`. It returns a string or `null`, and
    `null` on error.
  - `save(text)` returns `{ ok: false, reason: 'too-large' }` above
    `KEYMAP_MAX_BYTES` (imported from emaki, which replaces the local
    `MAX_BYTES`). It returns `{ ok: true }` on success. An invoke error
    rejects the promise with that error.
  - `clear()` invokes `vim_keymap_reset` and rejects on error.
- **Panel.** The `VimKeymapDeps.store` type becomes emaki `KeymapStore`. The
  panel status strings are unchanged:
  - "saved", "reset" and "keymap not saved: too-large";
  - "keymap not saved: storage unavailable" for a `save` result of
    `unavailable`;
  - for a rejected `save` or `clear`, the rejection text (`String(error)`),
    as `message` showed it before.

  The `load` failure path ("keymap not loaded: storage unavailable") is kept
  for rejections.
- **Removed types.** The vactr `KeymapStoreResult` and `VIM_KEYMAP_KEY`
  exports are removed. The key literal moves to the `LocalKeymapStore`
  construction sites.

### VM-D8. Types come from emaki/vim

| Old local type | Replacement |
|---|---|
| `VimController` (class) | emaki `VimController` (interface). It satisfies `ModalKeyHook` for `input.setVim`. |
| `VimSnapshot` | emaki `VimSnapshot`. The status view reads `prompt.text` instead of `prompt.initial` (VB9). |
| `CursorShape` | `VimSnapshot['cursorShape']` |
| `HintView`, `HopLabel` | emaki exports |
| `KeymapIssue`, `KeymapPreset` | emaki exports |
| `ClipboardAdapter` | `NonNullable<VimEngineOptions['clipboard']>` |
| `VimKey` | `string` (`VimController['cmdline']` is keyed by string) |
| `VimKeyInput` | emaki `ModalKeyInput` |

### VM-D9. `normalizeKey` is not re-implemented

The status-prompt `cmdline` remap (`resolveCmdline` in `vim-status-view.tsx`)
needs `normalizeKey`, which emaki does not export (VMB4). vactr does not copy
it, because it is engine key-normalization logic.

**Required behavior (unchanged from today).** A `cmdline` remap in the active
compiled keymap, whether user-authored or persisted, applies to keys typed in
the mounted status prompt (`.vact-vim-ex`). For example, a stored
`<C-g>` to `<Esc>` mapping cancels the prompt.

**Wiring.** The `resolveCmdline` parameter and `applyCmdlineKey` stay in
`vim-status-view.tsx` unchanged. The production resolver in `vim-mount.ts`
must be `key => compiled.cmdline.get(normalizeKey(input, true))`, with
`normalizeKey` imported from `emaki/vim`.

**Gap.** emaki `9dba0a3` does not export `normalizeKey`, so this resolver
cannot be built. That is a parity loss and an unconditional completion
blocker (VMB4).

The stub-resolver unit test in `test/ui/vim-status.test.tsx` and
`test/canvas/vim-status-keymap.test.ts` stays, but it is not parity evidence.
Parity check P2 (section 9) exercises the production wiring.

### VM-D10. Test helper replacement

`test/code/vim/host.ts` is deleted. `test/documents/session.test.ts` builds
its controller with `createVimController(host, { timer })`. The `host` is a
`VimHost` over the test `CodeSurface`, built with the same mapping as VM-D1,
and the test forwards `surface.subscribe` to `noteUpdate`.

To keep that mapping in one place, `vim-mount.ts` exports
`createVimHost(deps: Pick<VimDeps, 'surface' | 'viewHost' | 'layout'>)`.
The test calls it with minimal stubs, or builds an equivalent inline object
if `viewHost` and `layout` are impractical to stub.

The two "prevents Vim redo across a file load" cases keep every existing
assertion. A failure is VMB1 evidence. The test is never weakened, skipped or
marked as an expected failure.

## 4. Data flow after the change

```
keydown -> emaki InputController -> ModalKeyHook = emaki VimController (facade)
   -> engine -> VimHost.dispatch -> CodeSurface.dispatch
   -> CodeSurface publish -> vim-mount noteUpdate (ignored while the engine is dispatching)
   -> facade.subscribe(snapshot) -> Solid signal -> status/hints views, onChange -> renderer
      (cursor() and overlay() read vim.snapshot)
keymap: store.load() -> compile = compileKeymap(migrateLegacyActionIds(text)) -> vim.setKeymap
actions: engine -> options.actions['emaki.*'](ctx) -> evalCtl / formatter / diagnostics / completion
```

## 5. File disposition and parity

### 5.1 Source

| Removed `src/code/vim/` module | emaki counterpart (`emaki/vim`, `src/vim/`) | Notes |
|---|---|---|
| `ex.ts`, `execute.ts`, `extended.ts`, `hints.ts`, `hop.ts`, `keymap-runtime.ts`, `keys.ts`, `mapper.ts`, `marks.ts`, `motions.ts`, `notation.ts`, `operators.ts`, `parser.ts`, `record.ts`, `registers.ts`, `search.ts`, `session.ts`, `surround.ts`, `textobjects.ts`, `undo.ts`, `words.ts` | same file name | byte-identical (VB1) |
| `presets/default.json`, `presets/index.ts` | `presets/*` -> `KEYMAP_PRESETS` | identical |
| `presets/mise-darwin.json` | `presets/mise-darwin.json` | IDs and reason text renamed (VB4, VM-D3) |
| `actions.ts` | `actions.ts` | `emaki.*` IDs plus host IDs (VB2) |
| `keymap-schema.ts` | `keymap-schema.ts` -> `compileKeymap`, `KEYMAP_MAX_BYTES` | host action branch (VB2) |
| `types.ts` | `types.ts` plus public `host.ts` | `VimActions` becomes the registry (VM-D2) |
| `controller.ts` | `controller.ts` -> `createVimController` | divergences in VB5 to VB7 and section 8 |

Adapted glue:
- `vim-mount.ts`: VM-D1, D2, D4, D5, D6, D9 and D10.
- `vim-keymap-store.ts`: VM-D3 and D7.
- `vim-clipboard.ts`, `vim-hints-view.tsx`, `vim-keymap-view.tsx`,
  `vim-status-view.tsx`, `decorations.ts`: type imports (VM-D8), plus
  `prompt.text` in the status view.
- `src/app/main.ts`: stores (VM-D7).
- `src/app/deps.ts`: the `VimKeymapDeps` type comes through.

The following need no change unless the compiler requires it:
`vim-settings.ts`, `mount.ts`, `src/ui/transport-view.tsx`.
`src/code/file-boundary.ts` stays, and `surface.ts` and `vim-mount.ts` use it.

### 5.2 Tests

| Deleted `test/code/vim/` suite | Coverage after the change |
|---|---|
| `motions`, `words`, `textobjects`, `operators`, `parser`, `parser-extended`, `registers`, `marks`, `macro-marks`, `record`, `surround`, `search-ex`, `undo`, `undo-repeat`, `keys`, `keys-extended`, `mapper`, `notation`, `extended`, `extended-controller`, `hints`, `hop` | emaki engine tests for the byte-identical modules (emaki repo, not visible from this sandbox), plus vactr `test/e2e/vim-behavior.mjs` (end-to-end motions, operators, macros, surround, hop, hints) |
| `controller`, `controller-io` | emaki controller tests, plus vactr `test/canvas/vim-mount.test.ts`, `vim-input.test.ts`, `input-vim-hook.test.ts`, `test/documents/session.test.ts` (file boundary, VM-D10) |
| `actions`, `keymap-schema`, `keymap-controller`, `host-keymap`, `presets`, `keymap-parity` | emaki keymap tests, plus vactr `test/canvas/vim-keymap-{input,mount,view}.test.ts`, `vim-status-keymap.test.ts`, `input-keymap-hooks.test.ts`, `test/code/vim-keymap-store.test.ts` (adds VM-D3 migration cases: the seven IDs rewritten, other text unchanged, idempotent), and `test/e2e/vim-keymap.mjs` |
| `purity.test.ts` | removed. The directory it scans no longer exists, and emaki owns engine purity. |
| `host.ts` (helper) | replaced by `createVimHost` usage (VM-D10) |

These tests are adapted (imports and API only, same assertions):
`test/canvas/{input-keymap-hooks, input-vim-hook, vim-hints-view, vim-input,
vim-keymap-input, vim-keymap-mount, vim-keymap-view, vim-status-keymap,
vim-mount}.test.ts`, `test/code/{vim-clipboard, vim-keymap-store,
vim-settings}.test.ts`, `test/documents/session.test.ts` and
`test/ui/vim-status.test.tsx`.

A new mounted parity suite, `test/canvas/vim-parity.test.ts`, holds checks P1
to P4 (section 9). It uses the same mount rig as `vim-mount.test.ts`. It
drives keys through the mounted input and reads the DOM status, the surface
document and `vim.controller().snapshot`.

Rules for assertions:
- An assertion that checks a `vactr.*` ID in compiled output changes to
  `emaki.*`.
- An assertion that checks persisted text keeps `vactr.*`.
- No behavioral assertion is removed to get a test to pass. A failure caused
  by VB5 to VB8 is reported against the matching blocker.

## 6. Validation rules

- `grep -rE "code/vim/|['\"]\./vim/" editor/src editor/test` returns nothing.
- Engine symbols are imported only from `'emaki/vim'`. There are no deep
  `emaki/src/...` imports.
- `migrateLegacyActionIds` touches only the seven exact quoted tokens.
- The `vactr.vimKeymap` storage key and the Tauri commands are unchanged.

## 7. Blockers (missing emaki API, emaki read-only in this run)

| ID | Missing API | vactr impact | Status | Parity check |
|---|---|---|---|---|
| VMB1 | A document-boundary reset on `VimController`. Proposed: `resetDocument(): void`, or `VimEngineOptions.isDocumentBoundary?(userEvent: string \| null): boolean` checked in `noteUpdate`. Either clears the undo ledger, insert session, dot-repeat recorder, visual selection and parser, and enters normal mode, as vactr's old `onUpdate` did. | After a file load or replace, insert or visual mode, `.`, and Vim undo/redo ledger state carry over from the previous file. | Confirmed from source. Unconditional completion blocker. | P1 and `session.test.ts` |
| VMB2 | A way for the `ex` handler to set the status message. Proposed: `ex?: (cmd) => boolean \| { message: string; kind: 'info' \| 'error' }`. | `:w`, `:q`, `:wq` and `:x` no longer show the visible "vactr syncs the document live ..." info. | Confirmed from source. Unconditional completion blocker. | P3 |
| VMB3 | Host control over `metaKey` routing. emaki returns `false` for every meta input (VB7). | Cmd-modified keys in normal mode may reach the default keymap and edit text. | Unconfirmed. It becomes a completion blocker if P4 or a vim e2e fails because of a meta key. Do not add a vactr key filter. | P4 |
| VMB4 | Export `normalizeKey` (and `VimKey`/`VimKeyInput` types) from `emaki/vim`. | Persisted or user-authored `cmdline` remaps do not apply in the status prompt input. | Confirmed from source. Unconditional completion blocker. | P2 |

**Completion rule.** The migration is complete only when every parity check
(P1 to P4), every existing assertion and every gate in section 9 passes on
public `emaki/vim` APIs. An explicit user scope amendment recorded in EQ7 is
the only other way to accept a listed loss.

The VM-D5, VM-D6 and VM-D9 wiring is partial work, not completion. Holding
the push, test status, and TODO entries do not authorize a parity loss.

While VMB1, VMB2 or VMB4 is unresolved (or VMB3 is confirmed), the
implementation reports `blocked`. For each blocker it names the exact
missing API and the failing parity check, with its exit status and log path.

The blockers are recorded in
`design-docs/user-qa/pending-emaki-questions.md` EQ7.

## 8. Intentional divergences (emaki behavior adopted)

- `E486: Pattern not found` gains a colon, and the E492 text becomes
  `E492: Not an editor command: <cmd>`. Both now match Vim.
- In normal mode, the status message clears on the next key when no prompt or
  pending count is active.
- The `mise-darwin` preset text shown in the keymap panel shows `emaki.*` IDs
  and "emaki" in five `reason` strings.
- `vactr.*` IDs in user text compile through VM-D3. In the rare case that a
  `desc` string is exactly one of the seven IDs, it is also rewritten.

VMB1 to VMB4 are **not** intentional divergences. They are parity losses that
block completion (section 7).

## 9. Verification gates

Run from `editor/`, in the foreground. Record the exit status and log path for
each:

1. `npm run check`
2. `npx vitest run`
3. `npm run build`
4. `npm run test:style`
5. `npm run e2e:examples`
6. `node test/e2e/vim-behavior.mjs`
7. `node test/e2e/vim-keymap.mjs`
8. `grep -rE "code/vim/|['\"]\./vim/" src test` must return nothing.
9. From the repo root,
   `git diff --stat a998d1e -- '*.rs' Cargo.toml Cargo.lock editor/package.json editor/package-lock.json`
   must be empty.

If gate 5, 6 or 7 cannot start because the environment lacks a browser
runtime, record it as a blocked check with its log. It is not a pass.

### 9.1 Mounted parity checks (`test/canvas/vim-parity.test.ts`, run by gate 2)

Every check goes through production `createVimMode` wiring. None of them uses a
stub controller or a stub resolver.

- **P1, file boundary (VMB1).** In mounted Vim, enter insert mode (`i`) and
  type text. Then dispatch a document load the way `DocumentSession` does
  (`userEvent` `input.load.document`, and `input.replace.document` as a
  second case). Then check:
  - `.vact-vim-status[data-mode]` is `normal` and `snapshot.mode` is
    `normal`;
  - `.` and `u` leave the new document unchanged.

  Repeat starting from visual mode (`v`): after the load, mode is `normal`.
- **P2, persisted cmdline remap (VMB4).** Seed the mount's keymap store with
  a keymap whose `cmdline` maps `<C-g>` to `<Esc>`, then wait for load. Open
  `:` and dispatch Ctrl-g keydown on `.vact-vim-ex`. The prompt closes and
  `snapshot.prompt` is `null`. Repeat with a `<C-j>` to `<CR>` mapping on
  `:eval`: the prompt submits and `evalAtCursor` runs.
- **P3, visible write feedback (VMB2).** In normal mode, submit `:w`. The
  mounted status shows "vactr syncs the document live; use :eval, :evalall or
  :hush" as an info message. Do the same for `:q`, `:wq`, `:x` and `:w!`.
- **P4, meta keys (VMB3).** The existing `vim-mount.test.ts` case "keeps
  modified Backspace and Delete from editing in mounted normal mode" passes
  unchanged. In addition, Cmd-Backspace and Cmd-Delete with the platform set
  to `MacIntel` leave the document unchanged in normal mode.

P1 to P4 encode the behavior of `a998d1e`. Before any engine change, they must
pass against the current local engine. The implementer runs them first at the
base commit to prove that they are valid parity oracles, and records the exit
status and log.

After the migration they keep exactly the same assertions. A failure is
reported as the matching blocker. It is never skipped, marked `it.fails`,
deleted or relaxed.

## 10. Rollout constraints

- `editor/node_modules/emaki` is the local `9dba0a3` copy. `package.json`
  still pins `6138022`, so a fresh `npm ci` would install an emaki without
  `emaki/vim` and break the build until the operator bumps the pin. This is
  expected and goes into the commit TODOs.
- **When all gates and P1 to P4 pass:** make one local commit on `main` in the
  repository style, with no AI attribution, and do not push.
- **While any parity blocker remains:** do not make a completion commit, and
  do not describe the migration as done. Leave the partial migration
  uncommitted in the working tree. Report `blocked` with the missing APIs and
  the failing gate logs.

  A local commit of partial work is allowed only after an explicit user scope
  amendment (EQ7 option b). Its message must then list the accepted losses.
  This rule is separate from push timing. `gitPush=false` holds in every case.
- **After emaki ships the APIs and the operator bumps the pin,** the vactr
  follow-ups are:
  - VMB1: call the reset in VM-D6.
  - VMB2: return the message from `ex`.
  - VMB4: build the resolver in VM-D9 on the exported `normalizeKey`.
  - VMB3: needs a follow-up only if P4 fails.

  Then rerun all gates.
