# EX-60: Documents Area Controller and Boot Wiring

**Status**: Completed (session 382; Step 7 test-integrity and adversarial review accepted; archived 2026-10-17)
**Plan ID**: EX-60 (wave 2)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 4-8
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

## Intent and Context

This plan connects the wave-1 parts into the running editor: on boot the
editor shows the startup document (showcase on first open), a "Documents"
toolbar picker opens examples, saved documents and files as one undoable
replacement with unsaved-change confirmation, the web keeps a draft and a
named library, and both shells can set and reset a default template.

Context:
- `editor/src/app/main.ts`: `createEditor(root, deps)` mounts the shell,
  `MOUNT_ORDER` areas and song controls; `boot` builds deps, picks the
  tier, calls `createEditor`. `isTauri(win)` detects the app.
- `editor/src/app/song.ts:mount` is the pattern for an extra area that
  appends a section to the transport pane.
- `editor/test/app/main.test.ts` mocks `../../src/code/mount` (so
  `deps.code` is absent in boot tests) and `@tauri-apps/api/core`
  (`invokeMock` resolves `false` for every command). Fake windows there
  have no `localStorage` and no `addEventListener`.

## Non-goals

- Do not change `MOUNT_ORDER`, `createEditor`'s existing behavior without
  options, `editor/src/code/*`, `ui/shell.tsx`, `transport-view.tsx`,
  `app.css`, or the bind Save button.
- No evaluation, hush or audio resume on any document load.
- No e2e, style-gate or README work (EX-70).

## Dependencies

- **dependsOn**: EX-10 (`exampleSource`), EX-20 (`DocumentStorage`,
  `LocalTemplateStore`, `pageBackend`), EX-30 (`TauriTemplateStore`),
  EX-40 (`DocumentSession`, `DraftAutosave`, `resolveStartup`), EX-50
  (`PickerView`, `ensurePickerStylesheet`)
- **Blocks**: EX-70

## writePaths

- `editor/src/documents/mount.ts`
- `editor/src/app/main.ts`
- `editor/test/documents/mount.test.ts`
- `editor/test/app/main.test.ts`
- `impl-plans/active/examples-60-integration.md`
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-60` (artifact root; evidence logs)

## sharedPaths

- `editor/src/app/main.ts` (boot wiring and an optional `createEditor`
  parameter only)
- `editor/test/app/main.test.ts` (mechanical updates plus two new tests)

## Patterns to Imitate

- Area mount returning a handle and appending to the transport pane:
  `editor/src/app/song.ts:mount`.
- Solid render into a host element with dispose: `editor/src/ui/shell.tsx:mountShell`.
- Real code area in jsdom tests: `editor/test/wasm/first-track.test.ts`
  (buildLayout + `mount(root, deps)` from `src/code/mount` with
  `RecordingTransport`, `MemoryFiles`, `MockClock`).

## File-level Changes

### `editor/src/documents/mount.ts`

- `export interface DocumentsArea extends Mounted { ready: Promise<void> }`
- `export function mount(root: HTMLElement, deps: EditorDeps, opts: DocumentsOptions): DocumentsArea`
- No `deps.code` -> return `{ ready: Promise.resolve(), dispose() {} }`
  and render nothing.
- Creates `DocumentSession(deps.code.surface)`; for `shell === 'web'` and
  `opts.store.available` a `DraftAutosave` (guard `win.addEventListener`
  existence). Web with `!store.available` -> status
  "Browser storage is unavailable; saving to this browser is disabled".
- Renders `PickerView` into a new `section[data-doc-picker]` appended to
  `pane(root, 'transport')`; calls `ensurePickerStylesheet(doc)`.
- Signals: `open`, `library` (refreshed from `store.list()` on open and
  after each library action; app shell -> `{available:false, entries:[], unreadable:0}`),
  `template` (cached `DefaultTemplate | null`), `status`, `pendingReplace`
  plus the pending action.
- Replacement flow `replaceWith(load: () => Promise<{text, source}>)`: if
  `session.dirty()` set `pendingReplace` to "Replace the current document?
  Undo (Mod-Z) restores it." and keep `load`; `confirmReplace` runs it,
  `cancelReplace` drops it. Running: await `load` (rejection -> status,
  nothing replaced; `FileCancelled` -> silent), `session.replace(text, source)`,
  close the picker, `deps.code.surface.focus()`.
- Actions (`PickerActions`):
  - `openExample(id)` -> `opts.examples.load(id)`, source example.
  - `openLibrary(id)` -> `store.read(id)` (null -> status "Saved document
    not found"), source `{kind:'library', id, name}`.
  - `openFile()` -> `deps.files.open()` AFTER confirmation, source
    `{kind:'file', name}`.
  - `newDocument()` -> template example (load), template snapshot (source
    template), else `opts.examples.defaultId`. A template example id that
    `opts.examples.find` does not know falls back to `defaultId` with the
    status "Default template example <id> is no longer available"
    (design section 6).
  - `save()` -> target = current library source still readable ->
    `store.update`, `session.markSaved(source)`, status "Saved <name>".
  - `saveAs(name)` -> `store.create(name, text)`; success ->
    `markSaved({kind:'library', id, name})`, refresh, return null; failure
    -> message per reason: `name-taken` "A saved document with this name
    already exists", `invalid-name` "Use 1-80 characters without control
    characters", `too-large` "Document too large for browser storage; use
    Save file", `quota` "Browser storage is full", `unavailable` "Browser
    storage is unavailable", else the message.
  - `rename(id, name)` -> `store.rename`; if the session source is that id,
    `session.setSource({kind:'library', id, name: newName})`.
  - `remove(id)` -> `store.remove`; if the session source is that id,
    `session.setSource({kind:'new'})`.
  - `useAsTemplate()` -> clean example source -> `{kind:'example', id}`;
    otherwise snapshot `{name, text}` with name from the source (library
    name, file base name, example title, else "Untitled"); `templates.set`;
    status "Default template set" or the failure message.
  - `resetTemplate()` -> `templates.reset()`, status "Default template reset".
  - `toggle`, `close`.
- Derived props: `canSave = shell === 'web' && store.available`;
  `saveTarget` = library source name when that entry still exists, else
  null; `templateExampleId` and `hasTemplate` from the `template` signal.
- `ready`: `resolveStartup({shell, search, examples, drafts: web && available ? store : null, templates, notice})`,
  then `applyStartup(session, doc, notice)` (EX-40; it does the draft-first
  two-step load when `doc.underlying` is set, so a `?example=` link never
  discards a web draft: review finding ER-M1), then
  `template = await templates.get()`; wrapped so it NEVER rejects. The
  DraftAutosave must exist BEFORE `applyStartup` runs, so its
  `onBaseline` handling sees both startup steps.
- `dispose`: autosave, session, Solid root, section.

### `editor/src/app/main.ts`

- `createEditor(root, deps, opts: { documents?: DocumentsOptions } = {})`:
  when `opts.documents` is set, push `mountDocuments(root, deps, opts.documents)`
  after the song handle and expose it as `Editor.documents?: DocumentsArea`.
  Without options: identical to today.
- `boot`: before `createEditor`, build
  `shell = isTauri(win) ? 'app' : 'web'`,
  `store = new DocumentStorage(pageBackend(win))`,
  `templates = shell === 'app' ? new TauriTemplateStore(invoke) : new LocalTemplateStore(store)`,
  `examples = exampleSource`, `search = win.location.search`; pass
  `{ documents }`; after the tauri-fallback notice, `await editor.documents?.ready`
  before returning. Keep the returned `dispose` wrapper.

### `editor/test/documents/mount.test.ts` (jsdom, real code area)

Setup: `buildLayout`, `Store`, `Client(new RecordingTransport())`,
`MemoryFiles`, `MockClock`, real `src/code/mount`; fake `ExampleSource`
(showcase "S", acid "A"); `DocumentStorage` over an in-memory backend;
a fake `TemplateStore`; fake timers for autosave.

Tests (input -> expected):
- empty storage, web -> after `ready`, doc "S"; `section[data-doc-picker]`
  exists; no `eval` in `transport.kinds()`.
- open acid with a clean buffer -> doc "A", no confirmation; undo via
  `undo({state, dispatch})` from `@codemirror/commands` -> "S".
- type into the buffer, open acid -> `pendingReplace` shown, doc
  unchanged; confirm -> "A"; cancel path leaves the doc unchanged.
- saveAs "one" -> listed; saveAs "one" again -> returns the name-taken
  message; Save with target -> `store.read(id).text` updated.
- rename then remove the current entry -> later Save opens the Save as
  path (`saveTarget` null).
- useAsTemplate on clean example -> template `{kind:'example', id:'acid'}`;
  after typing -> snapshot; resetTemplate -> `get()` null.
- newDocument with template example acid -> "A".
- web: type, advance 1000 ms -> `readDraft()` has the text; open an
  example cleanly -> draft cleared.
- app shell -> no library section; typing + 2 s -> no draft written.
- store over a throwing backend -> startup "S" and the unavailable status.
- `MemoryFiles.queueOpen('x.vact')` + openFile -> doc from the file;
  `openFile` with nothing queued (FileCancelled) -> no error status.
- `deps.code` absent -> `ready` resolves; no section rendered.
- `?example=acid` search with a stored draft "D" (web) -> after `ready`
  doc "A" and the status contains "restores your unsaved draft"; undo via
  `undo({state, dispatch})` -> doc "D"; advance 1000 ms -> `readDraft().text`
  is "D" (draft recovered and rewritten).
- newDocument with a template example id unknown to the ExampleSource ->
  doc "S" and the "no longer available" status.

### `editor/test/app/main.test.ts`

- Existing tests keep passing; fix only assertions broken mechanically by
  the new boot wiring and list each in the Progress Log.
- Add: `createEditor` without options renders no `[data-doc-picker]`.
- Add: `boot` with a fake web window -> `editor.documents` defined and its
  `ready` resolved (code is mocked, so it is the no-op area).

## Invariants

- Startup precedence exactly as design section 7; nothing auto-plays.
- Every replacement is one undo step; clean loads clear the draft.
- `createEditor` without options is unchanged.
- No new localStorage keys beyond EX-20's.

## Pitfalls

- `deps.code` is set by the code area mount; read it at use time.
- `boot` must not fail when the template store or storage fails; `ready`
  never rejects.
- Do not call `surface.focus()` inside a dispatch callback.
- Confirm BEFORE opening the file dialog, not after.
- Keep `main.ts` and `mount.ts` under 1000 lines (`mount.ts` target < 400).
- Do not import `PickerView` into `main.ts`; only `documents/mount.ts`.

## Verification

Evidence in `tmp/examples/EX-60/`, each log ending with its exit status.

1. `cd editor && npx vitest run test/documents test/app/main.test.ts test/examples test/ui/picker-view.test.tsx > ../tmp/examples/EX-60/focused.log 2>&1` -> all pass.
2. `cd editor && npm run check > ../tmp/examples/EX-60/check.log 2>&1` -> exit 0 (whole project).
3. Under the measurement lock (mkdir loop on
   `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`, owner file
   `EX-60`, remove only your own lock): `cd editor && npm test > ../tmp/examples/EX-60/full-vitest.log 2>&1`
   -> exit 0, failureCount 0.

Records use `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`.

## Completion Criteria

- [x] `documents/mount.ts` implements every action and startup as above.
- [x] `main.ts` wiring done; `createEditor` without options unchanged.
- [x] Focused tests, `npm run check` and full `npm test` pass.
- [x] Mechanical `main.test.ts` changes listed.
- [x] Progress Log with records and pre/post sha256.

## Edit Protocol

Fresh read before each edit; sha256 before/after; writePaths only; stop and
record on drift; no git index or branch operations. If a wave-1 contract
mismatch is found, record it and fix only inside this plan's writePaths;
request contract edits through serial reconciliation.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored. Revised for ER-M1 (`ready` uses
`applyStartup`, autosave created first, recovery test) and ER-L2
(`newDocument` fallback for an unregistered template id).
**Blockers**: None.

### Session: 2026-10-17 — EX60-TI-DROPPED-CASES repair
**Finding**: `EX60-TI-DROPPED-CASES` (mid; test-integrity review).
**Changes**:
- Added controller coverage in `editor/test/documents/mount.test.ts` for
  queued file opening after dirty confirmation, file-source identity (verified
  through the resulting `x.vact` template snapshot), one-step undo, silent
  `FileCancelled`, confirmation-row cleanup, and New fallback from an
  unregistered template example to the showcase with its status notice.
- The first test-first run reproduced the stale confirmation row after
  `FileCancelled`. Updated only the `FileCancelled` branch in
  `editor/src/documents/mount.ts::runReplacement` to clear `pendingReplace`
  and `pendingLoad` without setting a status. The captured first-run output is
  `tmp/examples/EX-60/mount-ti-repair-first-attempt-captured.log`; the final
  prescribed log is `tmp/examples/EX-60/mount-ti-repair.log`.
- Cross-layer coverage retained without duplicating controller tests:
  `editor/test/documents/startup.test.ts` covers the `?example=` draft recovery
  ordering (ER-M1); EX-70 e2e checks 6-8 cover clean-example template use and
  New, storage-unavailable startup, and reload recovery at boot level. Those
  e2e checks remain assigned to the downstream EX-70 integration gate.

**Verification Records** (final repair source; logs include exit status):

```json
{"command":"cd editor && npx vitest run test/documents/mount.test.ts > ../tmp/examples/EX-60/mount-ti-repair.log 2>&1","exitStatus":0,"testsRun":9,"testsPassed":9,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-60/mount-ti-repair.log","notes":"All nine controller tests pass, including the three new review cases."}
{"command":"cd editor && npx vitest run test/documents test/app/main.test.ts test/examples test/ui/picker-view.test.tsx > ../tmp/examples/EX-60/focused.log 2>&1","exitStatus":0,"testsRun":82,"testsPassed":82,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-60/focused.log","notes":"10 test files passed on the repaired source."}
{"command":"cd editor && npm run check > ../tmp/examples/EX-60/check.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-60/check.log","notes":"Whole-project TypeScript check."}
{"command":"bash -c 'lock=/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock; until mkdir \"$lock\" 2>/dev/null; do sleep 30; done; cleanup() { unlink \"$lock/owner\" 2>/dev/null || true; rmdir \"$lock\" 2>/dev/null || true; }; trap cleanup EXIT; printf \"%s\\n\" EX-60 > \"$lock/owner\"; cd editor && npm test > ../tmp/examples/EX-60/full-vitest.log 2>&1; rc=$?; printf \"EXIT_STATUS=%s\\n\" \"$rc\" >> ../tmp/examples/EX-60/full-vitest.log; tail -n 35 ../tmp/examples/EX-60/full-vitest.log; exit \"$rc\"'","exitStatus":0,"testsRun":987,"testsPassed":987,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-60/full-vitest.log","notes":"105 test files passed under the measurement lock; lock owner EX-60."}
```

**Repair Hashes**: `tmp/examples/EX-60/ti-pre-edit-sha256.txt` records
pre-edit hashes. `tmp/examples/EX-60/ti-repair-source-sha256.txt` records
pre/post hashes for the changed test, controller, and this plan. Controller
remains under 1000 lines. No `main.ts` or `main.test.ts` changes were needed.
**Blockers**: None; the finding is repaired on the current source. Independent
re-review remains pending.

### Session: 2026-10-17 — Step 6 implementation
**Tasks Completed**:
- Added `editor/src/documents/mount.ts` (199 lines): controller wiring for
  `DocumentSession`, web-only `DraftAutosave`, startup resolution/application,
  picker rendering, dirty replacement confirmation, example/library/file
  opening, New, library create/update/rename/delete, template set/reset,
  status reporting, and disposal. `ready` absorbs startup/store failures.
- Updated `editor/src/app/main.ts` with optional `createEditor` documents
  options and boot store/template/example wiring. The two-argument
  `createEditor(root, deps)` path still omits the documents area. Boot awaits
  its optional `ready` after adding a Tauri fallback notice.
- Added six documents-area integration tests using a real `CodeSurface` and
  `DocumentSync` with a recording transport. They cover silent showcase
  startup, undoable replacement, dirty confirmation, library save/rename/
  delete, template set/use/reset, web draft autosave/clear, app no-draft
  behavior, and the missing-code no-op.
- Mechanical `main.test.ts` changes: added an assertion that the default
  `createEditor` call renders no picker, and a boot test proving the optional
  documents area exists and its no-op `ready` resolves when the mocked code
  mount leaves `deps.code` unset. No existing assertions required repair.
- No Rust files changed; Cargo checks were not applicable to EX-60.

**Verification Records** (final source; logs include `EXIT_STATUS`):

```json
{"command":"cd editor && npx vitest run test/documents test/app/main.test.ts test/examples test/ui/picker-view.test.tsx > ../tmp/examples/EX-60/focused.log 2>&1","exitStatus":0,"testsRun":79,"testsPassed":79,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-60/focused.log","notes":"10 test files passed; focused tests include the six EX-60 controller cases."}
{"command":"cd editor && npm run check > ../tmp/examples/EX-60/check.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-60/check.log","notes":"Whole-project TypeScript check; tsc --noEmit."}
{"command":"cd editor && npm test > ../tmp/examples/EX-60/full-vitest.log 2>&1","exitStatus":0,"testsRun":984,"testsPassed":984,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-60/full-vitest.log","notes":"Run in the foreground under /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock with owner EX-60; 105 test files passed."}
```

**Prior Attempts**: Retained intermediate logs include
`tmp/examples/EX-60/focused-retry.log` (unsupported Vitest `waitFor` import),
`mount-tests.log` and `mount-tests-retry.log` (replacement timing and picker
reopen corrections), `mount-tests-final.log` (6/6 passed before the final
selector typing-only edit), and `focused-final-attempt.log` (77/77 passed
before the two added integration cases). All required checks were rerun on
the final source and passed.

**Source Hashes**: `tmp/examples/EX-60/pre-plan-final-sha256.txt` records the
pre-plan-update hashes. `tmp/examples/EX-60/source-sha256.txt` records initial
and final hashes, including the post-update plan hash. The newly created `mount.ts` and
`mount.test.ts` were absent at the pre-node snapshot. Final `mount.ts` and
`main.ts` are 199 and 238 lines, respectively. No shared-path drift was
observed during the implementation edits.

**Downstream**: Formal independent test-integrity/adversarial review and the
EX-70 integration gates remain assigned to later workflow steps; no review
acceptance is claimed here.
**Blockers**: None.
