# EX-20: Web Document Storage (localStorage schema v1) and LocalTemplateStore

**Status**: Ready
**Plan ID**: EX-20 (wave 1, parallel)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 5.1, 5.2, 5.4, 6
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

The web editor must keep an unsaved draft and a named document library in
`localStorage`, and (XQ1) the web default-template choice. Storage can be
missing (private mode), throw `SecurityError`, be full (`QuotaExceededError`)
or hold corrupt or foreign-version JSON. The editor must still open. This
plan writes the storage module that never throws.

Context: today `localStorage` is used only for small preferences, always
guarded (`editor/src/ui/shell.tsx:pageStorage`, `read`, `write`).

## Non-goals

- No autosave timing, startup logic, picker UI or Tauri code.
- No migration code (v1 is the first schema).
- Do not touch `ui/shell.tsx` or other existing modules.

## Dependencies

- **dependsOn**: EX-00 (`DocumentStore`, `TemplateStore`, `Draft`,
  `LibraryEntry`, `DefaultTemplate`, `StorageResult`, `DOC_MAX_CHARS`,
  `isDocSource`, `isDefaultTemplate`)
- **Blocks**: EX-60

## writePaths

- `editor/src/documents/storage.ts`
- `editor/test/documents/storage.test.ts`
- `impl-plans/active/examples-20-storage.md`
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-20` (artifact root; evidence logs)

## sharedPaths

None.

## Patterns to Imitate

- Guarded access: `editor/src/ui/shell.tsx:read`, `write`, `pageStorage`.
- Plain class with injected dependency: `editor/src/platform/files.ts:MemoryFiles`.

## File-level Changes

### `editor/src/documents/storage.ts`

Exports:
- `export type StorageBackend = Pick<Storage, 'getItem' | 'setItem' | 'removeItem' | 'key' | 'length'>;`
- `export const DRAFT_KEY = 'vactr.doc.v1.draft'`, `ITEM_PREFIX = 'vactr.doc.v1.item.'`,
  `TEMPLATE_KEY = 'vactr.doc.v1.default-template'`, `PROBE_KEY = 'vactr.doc.v1.probe'`.
- `export function pageBackend(win: Window | null): StorageBackend | null`
  (try/catch around `win?.localStorage`, like `pageStorage`).
- `export function normalizeName(name: string): string | null` (trim;
  1-80 chars; reject control characters).
- `export class DocumentStorage implements DocumentStore` with
  `constructor(backend: StorageBackend | null, now: () => number = Date.now, newId?: () => string)`.
- `export class LocalTemplateStore implements TemplateStore` with
  `constructor(storage: DocumentStorage)`; it reads/writes `TEMPLATE_KEY`
  through package-private helpers on `DocumentStorage` (or a small exported
  helper pair) so all guarding stays in one place.

Record shapes (JSON):
- draft: `{v:1, text, source, savedAt}`
- item: `{v:1, id, name, text, createdAt, updatedAt}`, key `ITEM_PREFIX + id`
- template: `{v:1, template}`

Behavior:
- `available`: false when backend is null or the probe
  `setItem(PROBE_KEY,'1')`/`removeItem(PROBE_KEY)` throws anything other
  than a quota error. A quota error during the probe still means available
  (reads work; writes will report `quota`).
- Every backend call is inside try/catch. No public method throws.
- Reads: invalid JSON, `v !== 1`, or failed shape check -> treated as
  absent. Reads NEVER delete or rewrite such values.
- `list()`: collect all keys first (`for i < length: key(i)`), then read
  those with `ITEM_PREFIX`; valid ones are returned sorted by `updatedAt`
  descending; invalid ones increase `unreadable`.
- `create(name, text)`: `normalizeName` -> `invalid-name`; name equal to an
  existing valid entry's name -> `name-taken`; `text.length > DOC_MAX_CHARS`
  -> `too-large`; id from `newId` or `crypto.randomUUID()` when present,
  else `Date.now().toString(36) + random`.
- `update(id, text)`: missing or invalid item -> `missing`; size cap;
  keeps `name`/`createdAt`, sets `updatedAt`.
- `rename(id, name)`: same name rules; renaming to its own current name is
  ok.
- `remove(id)`: missing -> `missing`; else `removeItem`.
- `writeDraft(draft)`: size cap -> `too-large`; `clearDraft()` removes the
  key and swallows errors. `readDraft()` validates with `isDocSource`.
- Error mapping: `DOMException` with name `QuotaExceededError` or
  `NS_ERROR_DOM_QUOTA_REACHED`, or code 22 / 1014 -> `quota`; backend null
  or `available === false` -> `unavailable`; anything else -> `error` with
  `message`.
- `LocalTemplateStore.get()` resolves a value passing `isDefaultTemplate`,
  else `null`; `set()` rejects invalid templates as `{ok:false, reason:'invalid-name'}`
  for a bad snapshot name and `too-large` for oversize text; `reset()`
  removes the key. All return resolved promises.

## Invariants

- No method throws or rejects.
- Foreign or corrupt values are never deleted by reads.
- No other keys are touched (only the four constants and item prefix).

## Pitfalls

- Do not iterate `key(i)` while removing items.
- jsdom provides `localStorage`; for failure cases use hand-written fake
  backends (throwing getters, a `setItem` throwing
  `new DOMException('full', 'QuotaExceededError')`), not spies on the
  global.
- `structuredClone`/`JSON.stringify` of `text` is fine; do not store
  `Text` objects.
- Do not write `savedAt`/`updatedAt` from `Date.now()` directly in methods;
  use the injected `now` so tests are deterministic.

## Tests (input -> expected)

`editor/test/documents/storage.test.ts`:
- create "a" -> ok; list -> one entry; read(id).text equals input.
- create "a" twice -> second `name-taken`; create "  " -> `invalid-name`;
  create with 81-char name -> `invalid-name`.
- update then list -> entry first by `updatedAt`; update unknown id -> `missing`.
- rename to an existing other name -> `name-taken`; rename to own name -> ok.
- remove -> no longer listed; remove again -> `missing`.
- text of `DOC_MAX_CHARS + 1` chars in create/update/writeDraft -> `too-large`.
- item key holding `"{"` and one holding `{"v":2,...}` -> skipped,
  `unreadable === 2`, and both raw values still present in the backend.
- draft with invalid `source` -> `readDraft()` null, raw value kept.
- backend `setItem` throws QuotaExceededError -> create returns `quota`;
  `available` stays true.
- backend whose every method throws `SecurityError` -> `available` false;
  list -> `{entries: [], unreadable: 0}`; create -> `unavailable`; no throw.
- `new DocumentStorage(null)` -> `available` false; all calls safe.
- `pageBackend` with a window whose `localStorage` getter throws -> null.
- LocalTemplateStore: set example -> get returns it; set snapshot with
  oversize text -> `too-large`, get unchanged; corrupt template JSON -> get
  null; reset -> get null; with null backend all resolve without throwing.

## Verification

Evidence in `tmp/examples/EX-20/`, each log ending with its exit status.

1. `cd editor && npx vitest run test/documents/storage.test.ts > ../tmp/examples/EX-20/vitest.log 2>&1` -> all pass.
2. `cd editor && npm run check > ../tmp/examples/EX-20/check.log 2>&1` -> exit 0 (errors confined to other
   wave-1 plans' files are recorded and re-checked later; this plan's files
   have zero errors).

Records use `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`.

## Completion Criteria

- [ ] `DocumentStorage`, `LocalTemplateStore`, `pageBackend`,
      `normalizeName` and the key constants exported as specified.
- [ ] All listed tests pass; `npm run check` clean for this plan's files.
- [ ] File under 1000 lines.
- [ ] Progress Log with records and pre/post sha256.

## Edit Protocol

Fresh read before each edit; sha256 before/after in the Progress Log;
writePaths only; stop and record on drift; no git index or branch
operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored.
**Blockers**: None.
