# Vim Keymap: Web/Tauri Keymap Store and JSON Import/Export Implementation Plan

**Status**: Ready
**Plan ID**: VK-03 (wave 0)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 8.3, 8.4 (TS side), 8.5
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Parallel (wave 0)**: VK-00, VK-01, VK-02 (Rust commands this store calls), VK-04
- **Consumer**: VK-30 mount wiring and `app/main.ts`
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

The keymap document text is stored per viewer: on the web in
`localStorage` key `vactr.vimKeymap` (same scope as `vactr.vimMode`,
`editor/src/code/vim-settings.ts`), in the Tauri app through the VK-02
commands. Users import and export the JSON as a file. Imitate
`editor/src/documents/tauri-templates.ts:TauriTemplateStore` (invoke
wrapper, never throws) and `editor/src/platform/files.ts`
(`BrowserFiles`, `TauriFiles`, `FileCancelled`, `loadTauriPlugins`).

## Non-Goals

- No schema validation here beyond the byte limit (VK-01 owns it).
- No UI (VK-16) and no wiring into `main.ts`/`mount.ts` (VK-30).
- No behavior change for `.vact` open/save or sidecar files.

## File-Level Changes

### editor/src/code/vim-keymap-store.ts (new, budget 120 lines)

```ts
export const VIM_KEYMAP_KEY = 'vactr.vimKeymap';
export type KeymapStoreResult = { ok: true } | { ok: false; reason: 'too-large' | 'unavailable' | 'error'; message?: string };
export interface KeymapStore { get(): Promise<string | null>; set(text: string): Promise<KeymapStoreResult>; reset(): Promise<KeymapStoreResult> }
export class LocalKeymapStore implements KeymapStore { constructor(storage: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | null) }
export class TauriKeymapStore implements KeymapStore { constructor(invoke: InvokeFn) }
```

- Import `KEYMAP_MAX_BYTES` from `./vim/keymap-schema` is NOT allowed
  (VK-01 runs in parallel); define a local `MAX_BYTES = 262_144` with a
  comment that it must equal `KEYMAP_MAX_BYTES`. VK-30 adds a test
  asserting equality.
- Local: every storage access in try/catch; null storage or a throw ->
  `get` returns null, `set`/`reset` return `{ok:false, reason:'unavailable'}`.
  Text over the byte limit (UTF-8) -> `too-large` without writing.
- Tauri: `get` invokes `vim_keymap_get` and returns the string or null
  (non-string or throw -> null); `set` checks the byte limit then invokes
  `vim_keymap_set` with `{ text }`; `reset` invokes `vim_keymap_reset`;
  errors -> `{ok:false, reason:'error', message:String(error)}`.
  Reuse the `InvokeFn` type from `documents/tauri-templates.ts`.

### editor/src/platform/files.ts (shared; export-only refactor)

- Export the existing `loadTauriPlugins`, `isAbort` and `baseName`
  (currently module-private) without changing them.
- Extract the body of `BrowserFiles.download` into an exported
  `downloadText(doc: Document, name: string, text: string, mime: string): void`
  and make `BrowserFiles.download` call it. Behavior must be identical.

### editor/src/platform/json-files.ts (new, budget 140 lines)

```ts
export const JSON_FILTER: DialogFilter = { name: 'JSON (.json)', extensions: ['json'] };
export class JsonFileTooLarge extends Error {}
export interface JsonFiles { importJson(): Promise<string>; exportJson(name: string, text: string): Promise<void> }
export class BrowserJsonFiles implements JsonFiles { constructor(win?: Window) }
export class TauriJsonFiles implements JsonFiles { constructor(load?: () => Promise<TauriPlugins>) }
```

- Both reject with `FileCancelled` when the user cancels, and
  `importJson` rejects with `JsonFileTooLarge` over 262,144 bytes.
- Browser import: `showOpenFilePicker` with `{ description: 'JSON', accept: { 'application/json': ['.json'] } }`
  when present, else an `<input type=file accept=".json">` (same shape as
  `BrowserFiles.openWithInput`). Export: `showSaveFilePicker` when
  present, else `downloadText(doc, name, text, 'application/json')`.
- Tauri: `dialog.open({ multiple: false, directory: false, filters: [JSON_FILTER] })`
  then `fs.readTextFile`; export `dialog.save({ defaultPath: name, filters: [JSON_FILTER] })`
  then `fs.writeTextFile`. This uses only the existing capabilities.

## Invariants

- `editor/test/platform/files.test.ts` passes unchanged.
- No new Tauri capability, no new dependency.

## Test Cases

`editor/test/code/vim-keymap-store.test.ts`:
- Local get on empty storage -> null; set then get -> same text; reset -> get null
- storage whose getItem/setItem throw -> get null, set `{ok:false, reason:'unavailable'}`
- set with 262,145-byte text -> `too-large`, storage untouched
- Tauri: fake invoke records calls; get maps a string and maps `42` -> null; set passes `{text}` to `vim_keymap_set`; invoke throw -> `reason:'error'`

`editor/test/platform/json-files.test.ts`:
- Tauri import with fake plugins returning a path -> text; cancelled (null) -> FileCancelled; 262,145 chars -> JsonFileTooLarge
- Tauri export -> `writeTextFile(chosenPath, text)`; save cancelled -> FileCancelled
- Browser export without pickers (jsdom) -> an anchor with `download` named as given (spy on `downloadText` behavior via created `a`)

## Verification (from editor/)

1. Scoped typecheck including the new/edited source files and the two tests -> exit 0
2. `./node_modules/.bin/vitest run test/code/vim-keymap-store.test.ts test/platform/json-files.test.ts test/platform/files.test.ts` -> exit 0, failureCount 0, at least 14 new cases

## Completion Criteria

- [ ] Pinned exports exist; files within budget
- [ ] files.test.ts unchanged and passing
- [ ] At least 14 new passing cases
- [ ] Progress Log entry added

## Progress Log

(none yet)
