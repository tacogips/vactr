# EX-40: Document Session, Draft Autosave and Startup Resolution

**Status**: Ready
**Plan ID**: EX-40 (wave 1, parallel)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 4, 5.3, 7
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

Opening an example or saved document must replace the buffer as ONE
undoable step, protect unsaved changes, and (web) keep unsaved edits as a
draft across reloads. On startup the editor picks the first available
source: `?example=<id>` > draft (web) > chosen default template > the
built-in showcase. This plan writes three logic modules against the EX-00
interfaces, with no UI and no storage implementation.

Context:
- `editor/src/code/surface.ts:CodeSurface` is the single
  `@codemirror/state` authority with `history()`; the `CodeSurface`
  interface in `editor/src/app/apis.ts` exposes `state`, `dispatch`,
  `subscribe`, `focus`. Headless `CodeSurface` also has `undo()`/`redo()`.
- Paste/IME already isolate history with `isolateHistory.of('full')`
  (`editor/src/code/input.ts` ~140, ~205).
- `editor/src/protocol/document.ts:Timers`/`defaultTimers` is the
  injectable timer pattern.

## Non-goals

- No picker UI, no storage implementation, no Tauri code, no wiring into
  `main.ts` (EX-60).
- Replacement never evaluates, hushes or stops audio.
- Do not edit any `editor/src/code/*` file.

## Dependencies

- **dependsOn**: EX-00 (`DocSource`, `Draft`, `DraftStore`,
  `ExampleSource`, `TemplateStore`, `Shell`, `DOC_MAX_CHARS`)
- **Blocks**: EX-60

## writePaths

- `editor/src/documents/session.ts`
- `editor/src/documents/autosave.ts`
- `editor/src/documents/startup.ts`
- `editor/test/documents/session.test.ts`
- `editor/test/documents/autosave.test.ts`
- `editor/test/documents/startup.test.ts`
- `impl-plans/active/examples-40-session-startup.md`
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-40` (artifact root; evidence logs)

## sharedPaths

None.

## Patterns to Imitate

- Test surface construction: `editor/test/canvas/state.test.ts:setup`
  (RecordingTransport + Client + DocumentSync + headless CodeSurface).
- Timers: `editor/src/protocol/document.ts:Timers`.

## File-level Changes

### `editor/src/documents/session.ts`

`export class DocumentSession` with
`constructor(surface: CodeSurface /* app/apis interface */)`:
- `get source(): DocSource` (initial `{kind:'new'}`).
- `dirty(): boolean` -> `baseline === null ? !doc.eq(Text.empty) : !doc.eq(baseline)`.
- `replace(text: string, source: DocSource): void` -> ONE dispatch:
  `changes {from:0, to:doc.length, insert:text}`, `selection {anchor:0}`,
  `annotations: isolateHistory.of('full')`, `userEvent: 'input.replace.document'`,
  `scrollIntoView: true`; then `source = source`, `baseline = new doc`,
  then notify `onBaseline` listeners.
- `loadInitial(text, source, opts: { restored: boolean }): boolean`: only
  when the doc is empty AND no doc-changing update was seen since
  construction; dispatch the same change with
  `annotations: Transaction.addToHistory.of(false)` (no isolateHistory);
  `baseline = restored ? null : new doc`; notify; return true. Otherwise
  return false and change nothing.
- `markSaved(source: DocSource): void` -> `source = source`,
  `baseline = current doc`, notify.
- `setSource(source: DocSource): void` -> changes `source` only (used
  after a library rename or delete); baseline unchanged; no notify.
- `onBaseline(cb: () => void): () => void`.
- `dispose()` unsubscribes.

### `editor/src/documents/autosave.ts`

`export class DraftAutosave` with
`constructor(opts: { surface; session: DocumentSession; drafts: DraftStore; win: Window | null; timers?: Timers; now?: () => number; delayMs?: number /* 1000 */; notice(message: string): void })`:
- On every surface update with `docChanged`: (re)arm a trailing timer.
- When the timer fires (or on flush): read `session.dirty()` NOW. Dirty ->
  `writeDraft({text, source: session.source, savedAt: now()})`; clean ->
  `clearDraft()`.
- On `session.onBaseline`: cancel the timer; if clean, `clearDraft()`.
- `win` `pagehide` and `visibilitychange` (document hidden) -> flush if a
  timer is pending.
- Write failures: `too-large` -> notice "Document too large for the browser
  draft; use Save file"; `quota` -> notice "Browser storage is full; draft
  not saved"; each notice is shown once until the next successful write.
  `unavailable` -> no notice (EX-60 shows one storage notice).
- `flush()` and `dispose()` (removes listeners, cancels timer, no write).

### `editor/src/documents/startup.ts`

```ts
export interface StartupInput { shell: Shell; search: string; examples: ExampleSource; drafts: DraftStore | null; templates: TemplateStore; notice(message: string): void }
export interface StartupDocument { text: string; source: DocSource; restored: boolean; underlying?: Draft }
export function resolveStartup(input: StartupInput): Promise<StartupDocument | null>
export function applyStartup(session: DocumentSession, doc: StartupDocument, notice: (message: string) => void): boolean
```
Order (first success wins; never rejects):
1. `new URLSearchParams(search).get('example')`: registered id ->
   `{text: await load(id), source:{kind:'example', id}, restored:false}`,
   plus `underlying: draft` when `shell === 'web'` and
   `drafts?.readDraft()` returns a valid draft (review finding ER-M1:
   the draft must stay recoverable). Unknown id -> notice "Unknown example
   <id>" and continue; load rejection -> notice and continue.
2. `shell === 'web'` and `drafts?.readDraft()` -> `{text, source, restored:true}`.
   Skipped entirely for `'app'`.
3. `await templates.get()`: example -> load (unregistered id -> notice
   "Default template example <id> is no longer available" and continue);
   snapshot -> `{text, source:{kind:'template'}, restored:false}`.
4. `examples.defaultId` -> load, source example.
5. All failed -> notice and `null`.

`applyStartup(session, doc, notice)` (design section 7):
- No `underlying`: `session.loadInitial(doc.text, doc.source, {restored: doc.restored})`;
  return its result.
- With `underlying`: `session.loadInitial(underlying.text, underlying.source, {restored: true})`;
  if that returns false (the user already edited) return false and do
  nothing else. Otherwise `session.replace(doc.text, doc.source)` (one
  undoable step) and `notice('Opened <id>; Undo (Mod-Z) restores your unsaved draft')`;
  return true.

## Invariants

- `replace` = exactly one undo step and one redo step.
- Startup adds no undo step, except row 1 over a web draft, which adds
  exactly one (the example replace whose undo target is the draft text).
  Startup never overwrites a user edit.
- No startup path discards a draft that is not in the undo history.
- Autosave writes only dirty buffers; clean loads/saves clear the draft
  (design 5.3, review finding DR-L1).
- Nothing here evaluates or touches audio.

## Pitfalls

- Surface subscribers run INSIDE `dispatch`, before `replace` updates the
  baseline. Never decide dirty in the update callback; decide when the
  timer fires and on `onBaseline`.
- Without `isolateHistory.of('full')` a replace within 500 ms of typing
  merges into the typing undo group.
- `addToHistory.of(false)` is `Transaction.addToHistory` from
  `@codemirror/state`, not from `@codemirror/commands`.
- `CodeSurface.dispatch` throws on reentrant dispatch: do not dispatch
  from a subscriber.
- `visibilitychange` must check `document.visibilityState === 'hidden'`.
- `resolveStartup` must catch every await (template store, loaders).

## Tests (input -> expected)

`session.test.ts` (headless surface, fake timers):
- type "abc" (dispatch inserts), then immediately `replace('X', example)`
  -> doc "X"; one `undo()` -> "abc"; `redo()` -> "X".
- `replace` -> `dirty()` false; then insert a char -> true.
- empty surface, `loadInitial('S', example, {restored:false})` -> true,
  doc "S", `undoDepth` 0, `dirty()` false.
- `loadInitial(..., {restored:true})` -> `dirty()` true for non-empty text.
- surface with a prior user insert -> `loadInitial` returns false, doc unchanged.
- `markSaved(library)` -> `dirty()` false and `source` updated.
- `setSource({kind:'new'})` on a dirty buffer -> `source` changed, still dirty.

`autosave.test.ts` (fake `DraftStore` recording calls, fake timers):
- user insert after a clean load -> after 999 ms no write; at 1000 ms one
  `writeDraft` with current text and source.
- inserts every 300 ms for 3 s -> no write until 1000 ms after the last.
- `replace` (clean) while a timer is pending -> timer cancelled,
  `clearDraft` called, no write.
- `loadInitial` clean -> `clearDraft`, never `writeDraft`.
- `pagehide` event with a pending timer -> immediate write.
- `writeDraft` returns `too-large` twice -> one notice.
- `dispose()` -> later events cause no calls.

`startup.test.ts` (fake ExampleSource, DraftStore, TemplateStore):
- web, `?example=acid-techno`, draft present -> acid-techno, restored
  false, `underlying` equals the draft (not discarded).
- app, `?example=acid-techno`, draft store returning a draft -> no
  `underlying`.
- `applyStartup` with `underlying` on an empty headless surface (draft
  "D", example "A") -> doc "A", notice contains "restores your unsaved
  draft", `undoDepth` 1; `undo()` -> doc "D" and `session.dirty()` true.
- the same with a `DraftAutosave` attached -> after the replace,
  `clearDraft` is called; after `undo()` and 1000 ms, `writeDraft` is
  called with "D" (draft rewritten).
- `applyStartup` with `underlying` on a surface the user already edited ->
  false, doc unchanged, no notice.
- web, `?example=nope`, draft present -> notice, draft returned, restored true.
- web, no draft, template example `fm6-algorithms` -> that example.
- web, no draft, template snapshot -> its text, source template.
- web, no draft, template example unregistered -> notice, showcase.
- app, draft store returning a draft -> draft ignored, template used.
- app, template `get` resolves null -> showcase.
- showcase loader rejects and nothing else -> null plus notice, no throw.
- template store whose `get` rejects (contract violation) -> treated as
  null, showcase.

## Verification

Evidence in `tmp/examples/EX-40/`, each log ending with its exit status.

1. `cd editor && npx vitest run test/documents/session.test.ts test/documents/autosave.test.ts test/documents/startup.test.ts > ../tmp/examples/EX-40/vitest.log 2>&1` -> all pass.
2. `cd editor && npm run check > ../tmp/examples/EX-40/check.log 2>&1` -> exit 0 for this plan's files.

Records use `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`.

## Completion Criteria

- [ ] `DocumentSession`, `DraftAutosave`, `resolveStartup` exported with the
      signatures above.
- [ ] All listed tests pass; check clean for this plan's files.
- [ ] Each file under 1000 lines.
- [ ] Progress Log with records and pre/post sha256.

## Edit Protocol

Fresh read before each edit; sha256 before/after; writePaths only; stop and
record on drift; no git index or branch operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored. Revised for review finding ER-M1
(`StartupDocument.underlying`, `applyStartup` two-step load, recovery tests).
**Blockers**: None.
