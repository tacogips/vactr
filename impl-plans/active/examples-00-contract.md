# EX-00: Shared Document and Example Contracts

**Status**: Ready
**Plan ID**: EX-00 (wave 0)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 3.2, 4, 5.1, 5.2, 6, 7, 8
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

The example picker feature is split into parallel plans (EX-10 registry,
EX-20 web storage, EX-30 Tauri template store, EX-40 session/startup/
autosave, EX-50 picker view). They must agree on data shapes before they
start. This plan pins those shapes in ONE types module plus the ambient
typing for Vite's `import.meta.glob`. It writes no behavior beyond small
pure type guards.

Repository context: `editor/` is Vite + TypeScript + Solid.js.
`editor/tsconfig.json` has `"types": []`, so Vite client types are NOT
loaded and `import.meta.glob` is untyped today. Strict mode is on.

## Non-goals

- No storage, registry, session, UI or Tauri code.
- Do not edit `editor/tsconfig.json`, `editor/vite.config.ts` or any
  existing source file.
- No CSS module declarations.

## Dependencies

- **dependsOn**: none
- **Blocks**: EX-10, EX-20, EX-30, EX-40, EX-50 (they import these types)

## writePaths

- `editor/src/documents/types.ts`
- `editor/src/examples/glob.d.ts`
- `editor/test/documents/types.test.ts`
- `impl-plans/active/examples-00-contract.md`
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-00` (artifact root; evidence logs)

## sharedPaths

None.

## Patterns to Imitate

- Header comment style: `editor/src/app/apis.ts` (a short comment naming
  the design sections, then exported interfaces).
- Union-of-literals style: `editor/src/app/apis.ts:CodeAnnotation['kind']`.

## File-level Changes

### `editor/src/documents/types.ts`

Header comment: "Shared contracts for the example registry, document
storage, default template and picker (design-example-picker-and-persistence.md)."

Exports (names and shapes are the contract; do not rename):

```ts
export type Shell = 'web' | 'app';
export type Requirement = 'audio-input' | 'midi';
export interface ExampleEntry { id: string; title: string; description: string; category: string; requires: readonly Requirement[] }
export interface ExampleSource {
  readonly defaultId: string;
  list(): readonly ExampleEntry[];          // manifest order, grouped by category
  find(id: string): ExampleEntry | undefined;
  load(id: string): Promise<string>;        // rejects for unknown id or failed chunk load
}
export type DocSource =
  | { kind: 'example'; id: string } | { kind: 'library'; id: string; name: string }
  | { kind: 'template' } | { kind: 'file'; name: string } | { kind: 'new' };
export type DefaultTemplate = { kind: 'example'; id: string } | { kind: 'snapshot'; name: string; text: string };
export interface Draft { text: string; source: DocSource; savedAt: number }
export interface LibraryEntry { id: string; name: string; text: string; createdAt: number; updatedAt: number }
export type StorageFailure = 'unavailable' | 'quota' | 'too-large' | 'invalid-name' | 'name-taken' | 'missing' | 'error';
export type StorageResult<T = void> = ([T] extends [void] ? { ok: true } : { ok: true; value: T }) | { ok: false; reason: StorageFailure; message?: string };
// `{ ok: true }` typechecks for StorageResult (void); `{ ok: true, value }` for StorageResult<LibraryEntry>.
export interface DraftStore { readDraft(): Draft | null; writeDraft(draft: Draft): StorageResult; clearDraft(): void }
export interface DocumentStore extends DraftStore {
  readonly available: boolean;
  list(): { entries: LibraryEntry[]; unreadable: number };   // newest updatedAt first
  read(id: string): LibraryEntry | null;
  create(name: string, text: string): StorageResult<LibraryEntry>;
  update(id: string, text: string): StorageResult<LibraryEntry>;
  rename(id: string, name: string): StorageResult<LibraryEntry>;
  remove(id: string): StorageResult;
}
export interface TemplateStore {
  get(): Promise<DefaultTemplate | null>;            // never rejects
  set(template: DefaultTemplate): Promise<StorageResult>;  // never rejects
  reset(): Promise<StorageResult>;                   // never rejects
}
export interface DocumentsOptions { shell: Shell; store: DocumentStore; templates: TemplateStore; examples: ExampleSource; search: string }
export const DOC_MAX_CHARS = 262144;
export const EXAMPLE_ID_PATTERN = /^[a-z0-9][a-z0-9/-]{0,95}$/;
```

Picker view contract (consumed by EX-50 and EX-60; `Accessor` from
`solid-js`):

```ts
export interface LibraryView { available: boolean; entries: readonly LibraryEntry[]; unreadable: number }
export interface PickerActions {
  toggle(): void; close(): void;
  openExample(id: string): void; openLibrary(id: string): void; openFile(): void; newDocument(): void;
  save(): void;
  saveAs(name: string): string | null;            // error text, or null on success
  rename(id: string, name: string): string | null; // error text, or null on success
  remove(id: string): void;
  useAsTemplate(): void; resetTemplate(): void;
  confirmReplace(): void; cancelReplace(): void;
}
export interface PickerProps {
  shell: Shell;
  open: Accessor<boolean>;
  examples: readonly ExampleEntry[];
  library: Accessor<LibraryView>;
  templateExampleId: Accessor<string | null>;  // the example marked `default`
  hasTemplate: Accessor<boolean>;              // Reset shown only when true
  canSave: Accessor<boolean>;                  // web and library available
  saveTarget: Accessor<string | null>;         // library name Save overwrites; null -> Save opens the Save as form
  status: Accessor<string>;                    // shown next to the trigger, always visible
  pendingReplace: Accessor<string | null>;     // confirmation text when a dirty replace waits
  actions: PickerActions;
}
```

Pure guards (no storage access):

- `isDocSource(value: unknown): value is DocSource`: exact `kind` and string
  fields; `example` ids must match `EXAMPLE_ID_PATTERN`.
- `isDefaultTemplate(value: unknown): value is DefaultTemplate`: `example`
  id matches the pattern; `snapshot` name is 1-120 chars with no control
  characters (`/[\u0000-\u001f\u007f]/` rejects) and `text` is a string of
  at most `DOC_MAX_CHARS` code units.

### `editor/src/examples/glob.d.ts`

Ambient (no imports or exports, so it merges globally) declaration of the
two `ImportMeta.glob` overloads used by EX-10: with `eager: true` it
returns `Record<string, T>`; otherwise `Record<string, () => Promise<T>>`.
Options type: `{ query?: string; import?: string; eager?: boolean }`.
Pattern argument: `string | string[]`. Keep it to these members.

## Invariants

- `types.ts` imports only `type { Accessor } from 'solid-js'`.
- No runtime side effects at import.
- Shapes above are byte-for-byte the contract other plans code against.
  If a dependent plan needs a change, it records the request in its
  Progress Log; the change is applied in serial reconciliation, not by
  editing this file from a parallel plan.

## Pitfalls

- Do not add `vite/client` to `tsconfig.json` types (it can conflict with
  the DOM lib setup and touches a shared file).
- `glob.d.ts` must not contain `export {}`; a module-scoped interface does
  not merge into the global `ImportMeta`.
- `message?` on failures is optional: guards and tests must not require it.

## Tests (input -> expected)

`editor/test/documents/types.test.ts` (jsdom default env):

- `{kind:'example', id:'synth-labs/lorenz'}` -> `isDocSource` true.
- `{kind:'example', id:'../etc'}` and `{kind:'example', id:'A'}` -> false.
- `{kind:'library', id:'x', name:'n'}`, `{kind:'template'}`, `{kind:'new'}`, `{kind:'file', name:'a.vact'}` -> true.
- `{kind:'library', id:'x'}` (missing name), `null`, `'example'` -> false.
- `{kind:'snapshot', name:'My song', text:'s :fm'}` -> `isDefaultTemplate` true.
- snapshot with `name:''`, a 121-char name, a name containing `\n`, or text of `DOC_MAX_CHARS + 1` chars -> false.
- `{kind:'example', id:'showcase'}` -> true; `{kind:'other'}` -> false.

## Verification

Log every command to `tmp/examples/EX-00/<name>.log` with the exit status
appended (`; echo "exit=$?" >> <log>`).

1. `cd editor && npx vitest run test/documents/types.test.ts` -> all
   tests pass (`tmp/examples/EX-00/vitest.log`).
2. `cd editor && npm run check` -> exit 0 (`tmp/examples/EX-00/check.log`).

Record each as `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}` in the Progress Log.

## Completion Criteria

- [ ] `editor/src/documents/types.ts` exports every symbol above with the
      exact names and shapes.
- [ ] `editor/src/examples/glob.d.ts` declares both overloads.
- [ ] Guard tests pass; `npm run check` exits 0.
- [ ] Progress Log has the verification records and pre/post sha256 of
      each written file.

## Edit Protocol

Read each file fresh before editing; record `shasum -a 256` before and
after each write in the Progress Log. Edit only writePaths. If a file
changes between your read and your write, stop, record the drift and
re-read. Never run git commands that change the index or branches.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored. Revised for ER-L1 (`StorageResult`
void success is `{ ok: true }`).
**Blockers**: None.
