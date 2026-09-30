# CMP-15: Editor Completion Contract Types (`completion-types.ts`)

**Status**: Completed
**Plan ID**: CMP-15 (wave 1; parallel with CMP-10, FST-50, EDS-10, EDS-11, EDS-12)
**Design Reference**: `design-docs/specs/design-completion.md` 5.2, 6.1, 6.2, 6.3 (the auto-trigger rule)
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

Three editor files are written in parallel or in sequence:

- the service, `completion.ts` (CMP-30);
- the popup, `completion-popup.ts` (CMP-31);
- the EditorView adapter, `completion-view.ts` (CMP-32).

They must agree on one set of types. This plan pins those types in one
small file, so that CMP-30 and CMP-31 can start at the same time. The
file also holds the one pure rule that both the popup and a future
canvas input bridge need: which typed character triggers completion.

## Non-goals

- No logic beyond `isTriggerChar` and the two constant arrays.
- No DOM access, and no `@codemirror/*` import (not even a type import).
- No edits to any other file.

## Dependencies

- **dependsOn**: none
- **Blocks**: CMP-30, CMP-31, CMP-32

## writePaths

- `editor/src/code/completion-types.ts` (new)
- `editor/test/code/completion-types.test.ts` (new)
- `impl-plans/active/cmp-15-completion-types.md` (Progress Log only)

## sharedPaths (read only)

`editor/src/code/format.ts` (style: exported interfaces, no default
exports) and `editor/tsconfig.json` (strictness).

## Pinned contract (write exactly these exported names)

```ts
export type CompletionContextKind = 'none' | 'keyword' | 'qualified' | 'pipe' | 'head' | 'pair-key' | 'argument';
export type CandidateKind = 'local' | 'function' | 'value' | 'variable' | 'type' | 'keyword' | 'control' | 'key' | 'module' | 'qualified';
export interface CompletionItem { label: string; kind: CandidateKind; detail: string; insert: string }
export interface RawCompletion { v: 1; context: CompletionContextKind; from: number; to: number; incomplete: boolean; items: CompletionItem[] } // UTF-8 byte offsets
export interface CompletionResult { context: CompletionContextKind; from: number; to: number; incomplete: boolean; items: CompletionItem[] } // UTF-16 offsets
export interface CompletionEngine { complete(text: string, byteCursor: number, limit: number): Promise<RawCompletion | null> } // null: status != 0
export interface CompletionSource { readonly available: boolean; complete(text: string, cursor16: number): Promise<CompletionResult | null> }
export type CompletionKey = 'ArrowUp' | 'ArrowDown' | 'PageUp' | 'PageDown' | 'Enter' | 'Tab' | 'Escape' | 'Ctrl-Space';
export interface SurfaceChange { docChanged: boolean; selectionChanged: boolean; userEvent: string | null; inserted: string }
export interface CaretRect { left: number; top: number; bottom: number }
export interface CompletionSurface { /* the ten members of design 6.2, exactly */ }
export const COMPLETION_KEYS: readonly CompletionKey[];  // the eight keys, in the order above
export const COMPLETION_USER_EVENT = 'input.complete';
export function isTriggerChar(ch: string): boolean;
```

`CompletionSurface` members, copied from design 6.2:

- `text()`, `selection()`, `replace(from, to, insert)`, `caretRect(pos)`;
- `isComposing()`;
- `onChange(listener)`, `onKey(handler)`, `onBlur(listener)` and
  `onCompositionStart(listener)`, each returning an unsubscribe function;
- `popupHost()`.

## Key Points

- `isTriggerChar(ch)` is true only when `ch.length === 1` and `ch`
  matches `/[A-Za-z0-9\-:.]/`. It is false for whitespace, brackets,
  quotes, any non-ASCII character (Japanese input is IME text, never an
  identifier) and multi-character strings such as a paste.
- Use `export type` or `export interface` for every type. The only
  runtime exports are `COMPLETION_KEYS`, `COMPLETION_USER_EVENT` and
  `isTriggerChar`.
- Keep the field names exactly as the design 5.2 JSON has them (`insert`,
  not `insertText`), so the service can pass items through without
  renaming them.

## Tests (`editor/test/code/completion-types.test.ts`, default jsdom env)

- `a`, `Z`, `7`, `-`, `:` and `.` -> true.
- A space, `\t`, `{`, `"`, the empty string, `ab`, U+3042 and U+65E5 ->
  false.
- `COMPLETION_KEYS` equals the eight keys in order and has no `Mod-*`
  entry.
- `COMPLETION_USER_EVENT === 'input.complete'`.
- A source-text check (read the file with `node:fs` through the
  `/* @vite-ignore */` dynamic-import idiom of
  `editor/test/support/wasm.ts`): the file contains no `@codemirror`.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`: fresh read, sha256 before and
after each edit, intent line, stop on drift, no git state changes. Write
logs to `tmp/cmp/CMP-15/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `cd editor && npx tsc --noEmit`
2. `cd editor && npx vitest run test/code/completion-types.test.ts`
   (a positive test count, all passing)

## Completion Criteria

- [x] Every pinned export exists with exactly the pinned names and shapes.
- [x] The tests pass, and `tsc --noEmit` passes.
- [x] No `@codemirror` string appears in `completion-types.ts`.
- [x] The Progress Log records the commands, exit codes and log paths.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

### Session: 2026-09-30 (CMP-15 implementation)
**Tasks Completed**: Added the pinned completion contract types, `CompletionSurface` members, ordered key constants, completion user-event constant and trigger-character predicate. Added focused Vitest coverage for trigger and non-trigger inputs, key ordering, user-event value and absence of CodeMirror references.
**Verification**:
- `cd editor && npx tsc --noEmit` — exit 0; complete log: `tmp/cmp/CMP-15/attempt-3/tsc.log`.
- `cd editor && npx vitest run test/code/completion-types.test.ts` — exit 0; 17 passed, 0 failed; complete log: `tmp/cmp/CMP-15/attempt-3/vitest.log`.
**Prior attempts**:
- Initial tsc log wrapper exited 1 before recording the child status because zsh reserves the variable name `status`; retained at `tmp/cmp/CMP-15/attempt-1/tsc.log`. The corrected wrapper rerun passed (exit 0), retained at `tmp/cmp/CMP-15/attempt-2/tsc.log`.
- `cd editor && npx vitest run test/code/completion-types.test.ts` initially exited 1 (16 passed, 1 failed) because Vitest supplied a non-file `import.meta.url`; retained at `tmp/cmp/CMP-15/attempt-2/vitest.log`. Source lookup now uses the editor working directory; final rerun passed as recorded above.
**Blockers**: None.

## Related Plans

- **Next**: CMP-30, CMP-31, CMP-32
