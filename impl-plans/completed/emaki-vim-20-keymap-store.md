# EVM-20: Keymap store on emaki KeymapStore, plus legacy action-ID migration

**Status**: Ready
**planId**: EVM-20. **Wave**: 2, in parallel with EVM-21. **dependsOn**:
EVM-10.
**Design reference**: `design-docs/specs/design-emaki-vim-migration.md`
VM-D3 and VM-D7, baseline rows VB2, VB3 and VB12, and section 6 validation
rules.
**Rules**: `impl-plans/active/emaki-vim-00-rules.md`.

## Intent and context

vactr's store module defines its own `get/set/reset` interface. emaki
defines `KeymapStore` as `load/save/clear`, with
`KeymapStoreSaveResult = {ok:true} | {ok:false; reason:'too-large'|'unavailable'}`,
and ships `LocalKeymapStore({ key?, storage? })`. See
`editor/node_modules/emaki/src/vim/keymap-store.ts`.

Saved user keymaps may contain the seven legacy `vactr.*` action IDs. emaki
renamed them to `emaki.*`. A pure text migration keeps those keymaps
working.

## Non-goals

- Do not edit `vim-mount.ts`, `main.ts` or any canvas test. Those switch to
  the new store API in EVM-30.
- No change to the Tauri command names or to the Rust side.

## File-level changes

### `editor/src/code/vim-keymap-store.ts`

- **Delete:** the local `KeymapStore` interface, `KeymapStoreResult`, the
  `LocalKeymapStore` class, the `VIM_KEYMAP_KEY` export and the `MAX_BYTES`
  constant.
- **Keep:** `export class TauriKeymapStore implements KeymapStore`, where
  `KeymapStore` is the type imported from `'emaki/vim'`.
  - `load(): Promise<string | null>` invokes `'vim_keymap_get'`. It returns
    the value if it is a string, otherwise `null`. Any invoke error resolves
    to `null`, as today's `get` does.
  - `save(text): Promise<KeymapStoreSaveResult>`. If the UTF-8 byte length
    is above `KEYMAP_MAX_BYTES` (imported from `'emaki/vim'`), it returns
    `{ ok: false, reason: 'too-large' }` without invoking. Otherwise it
    invokes `'vim_keymap_set', { text }` and returns `{ ok: true }`. On an
    invoke error it **rejects** with the original error. This keeps the
    panel's `String(error)` text.
  - `clear(): Promise<void>` invokes `'vim_keymap_reset'` and rejects with
    the original error on failure.
- **Add:** `export function migrateLegacyActionIds(text: string): string`.
  It is pure. It replaces every JSON string token that equals exactly
  `"vactr.X"`, for X in `evaluate`, `evaluateAll`, `hush`, `format`,
  `diagnostic.next`, `diagnostic.prev` and `completion`, with `"emaki.X"`.
  The match includes the double quotes, so that `"vactr.evaluateAllX"` and
  `"vactr.evaluate"` inside longer strings are untouched. Whitespace and
  everything else are preserved byte for byte. It is idempotent. It does not
  parse JSON, so invalid JSON passes through and compile reports it.
- Keep the `InvokeFn` import from `../documents/tauri-templates`.

### `editor/test/code/vim-keymap-store.test.ts`

Rewrite it against the new API. Keep every existing behavioral scenario,
renaming `get` to `load`, `set` to `save` and `reset` to `clear`. Cases:

- Tauri `load` with an invoke returning a string -> that string.
- Tauri `load` with a non-string or a rejection -> `null`.
- Tauri `save` with oversize text -> `{ok:false, reason:'too-large'}`, and
  invoke is not called.
- Tauri `save` success -> `{ok:true}` and invoke is called with
  `('vim_keymap_set', {text})`.
- Tauri `save` with an invoke error `'disk full'` -> the promise rejects,
  and `String(error)` contains `disk full`.
- Tauri `clear` success and rejection, mirroring `save`.
- Migration: a text containing all seven `"vactr.*"` IDs as `action` values
  -> all become `"emaki.*"`, and the rest of the text is identical.
- Migration: `"vactr.evaluateAllX"`, `"myvactr.hush"` and a desc
  `"use vactr.hush"` -> unchanged.
- Migration: applying it twice equals applying it once.
- Migration: invalid JSON text -> returned unchanged except for exact tokens.
- If the old file had browser `LocalKeymapStore` scenarios, port them to
  emaki `new LocalKeymapStore({ key: 'vactr.vimKeymap', storage })`:
  - set/load round-trip under key `vactr.vimKeymap`;
  - `storage` whose `setItem` throws -> `save` gives `{ok:false, reason:'unavailable'}`.

  Keep each scenario's expectation where emaki behavior matches. Where it
  differs (for example, `clear` swallowing errors), keep the vactr
  expectation and report the case as a divergence blocker instead of
  rewriting it.

## Invariants

- The localStorage key stays `vactr.vimKeymap`.
- The Tauri command names do not change.
- There is no `vactr.*` to `emaki.*` rewrite anywhere except in
  `migrateLegacyActionIds`.

## Expected transient breakage

After this plan, `npx tsc` reports errors in `src/code/vim-mount.ts`,
`src/app/main.ts` and `test/canvas/vim-keymap-mount.test.ts`, because they
still use the removed exports. EVM-30 owns them. No other file may show
new errors.

## Acceptance criteria

- [ ] `vim-keymap-store.ts` exports exactly `TauriKeymapStore` and
      `migrateLegacyActionIds`.
- [ ] `grep -n "MAX_BYTES\b\|VIM_KEYMAP_KEY\|KeymapStoreResult" editor/src/code/vim-keymap-store.ts`
      returns nothing (other than `KEYMAP_MAX_BYTES`).
- [ ] `npx vitest run test/code/vim-keymap-store.test.ts` exits 0.
- [ ] Filtered tsc shows no error lines for `src/code/vim-keymap-store.ts`
      or `test/code/vim-keymap-store.test.ts`.
- [ ] The progress log is updated.

## Verification commands (run in `editor/`)

| Command | Evidence |
|---|---|
| `npx vitest run test/code/vim-keymap-store.test.ts` | exit 0, with all migration and store cases listed |
| `npx tsc --noEmit -p . > tmp/emaki-vim/EVM-20/tsc.log 2>&1; grep -E "vim-keymap-store" tmp/emaki-vim/EVM-20/tsc.log` | no matches |

## Progress Log
