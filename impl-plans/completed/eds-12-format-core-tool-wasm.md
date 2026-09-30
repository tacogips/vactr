# EDS-12: CodeMirror-free Format Core and Shared `ToolWasm`

**Status**: Completed
**Plan ID**: EDS-12 (wave 1; parallel with CMP-10, CMP-15, FST-50, EDS-10, EDS-11)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 5.5 (format part); `design-docs/specs/design-completion.md` 6.4 (`ToolWasm`)
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

`editor/src/code/format.ts` mixes the wasm formatter with the EditorView
command. The canvas editor needs the formatter without `@codemirror/view`.
The completion engine (CMP-30) also needs the same dedicated,
import-free `vactr.wasm` instance, so that the page does not instantiate
the module twice. This plan:

- extracts `format-core.ts`, with `WasmFormatter` and a pure
  `minimalChange`;
- adds `tool-wasm.ts`, with a lazily instantiated shared instance;
- leaves `format.ts` as the EditorView command adapter.

## Non-goals

- No behavior change to `formatDocument`, `formatKeymap`, `FORMAT_KEY`
  or the dispatched change (`userEvent: 'format'`).
- No edits to `main.ts`, `deps.ts` or `mount.ts`. CMP-32 switches
  `main.ts` to a shared `ToolWasm` later.
- No edit to `editor/test/wasm/format.test.ts` (owned by EDS-11).

## Dependencies

- **dependsOn**: none
- **Blocks**: CMP-30, CMP-32, CMP-40

## writePaths

- `editor/src/code/tool-wasm.ts` (new)
- `editor/src/code/format-core.ts` (new)
- `editor/src/code/format.ts`
- `editor/test/code/format.test.ts` (append cases only)
- `editor/test/code/tool-wasm.test.ts` (new)
- `impl-plans/active/eds-12-format-core-tool-wasm.md` (Progress Log only)

## sharedPaths (read only)

`editor/src/app/main.ts:100-101` (the current
`new WasmFormatter(formatterUrl)` call must keep compiling),
`editor/test/wasm/format.test.ts` (uses `new WasmFormatter(url, fetchFn)`
through `../../src/code/format`).

## Pinned Contract (CMP-30 and CMP-32 depend on it)

```ts
// tool-wasm.ts
export interface ToolExports { memory: WebAssembly.Memory; alloc(len: number): number; free(ptr: number, len: number): void; [name: string]: unknown }
export class ToolWasm {
  constructor(wasmUrl: string, fetchFn?: (url: string) => Promise<Response>);
  exports(): Promise<ToolExports>; // instantiates once (no imports); a rejected load is forgotten so the next call retries
}
// format-core.ts
export interface FormatResult { status: number; text: string }
export interface Formatter { format(text: string): Promise<FormatResult> }
export class WasmFormatter implements Formatter { constructor(source: string | ToolWasm, fetchFn?: (url: string) => Promise<Response>) }
export function minimalChange(before: string, after: string): { from: number; to: number; insert: string } | null;
```

## File-level Changes

1. `tool-wasm.ts`:
   - Move the lazy-load logic of today's `WasmFormatter.load()`
     (`editor/src/code/format.ts:52-64`) into `ToolWasm.exports()`:
     - fetch, then check `response.ok`, then `arrayBuffer`, then
       `WebAssembly.instantiate(bytes, {})`;
     - cache the promise;
     - reset the cache on rejection, keeping the
       `if (this.instance === pending)` guard.
   - The error message for a non-ok response is
     `Unable to load vactr wasm: <status>`.
2. `format-core.ts`:
   - Move `FormatResult`, `Formatter` and `WasmFormatter`.
   - `WasmFormatter` holds a `ToolWasm`. A string source constructs
     `new ToolWasm(url, fetchFn)`.
   - `format()` keeps today's alloc, set, `fmt_source`, fresh
     `memory.buffer` view, `slice` and free sequence exactly, including
     `free` in `finally`.
   - Add `minimalChange`, which is the common-prefix and common-suffix
     loop that `formatDocument` inlines today. It returns `null` when
     `before === after`.
3. `format.ts`:
   - Keep `FORMAT_KEY`, `formatDocument` and `formatKeymap`.
     `formatDocument` calls `minimalChange`, and its dispatch is
     otherwise identical.
   - Re-export `export { WasmFormatter, minimalChange } from './format-core'`
     and `export type { Formatter, FormatResult } from './format-core'`.
   - `format-core.ts` and `tool-wasm.ts` must not import `@codemirror/*`.

## Pitfalls

- Do not create a new `Uint8Array(memory.buffer)` view before calling
  `fmt_source`. Memory may grow during the call, which detaches earlier
  views.
- `minimalChange` works on UTF-16 code units, as today. Do not change it
  to code points.
- The `WasmFormatter(url)` form must still fetch lazily. Construction
  never fetches.

## Tests

- `editor/test/code/tool-wasm.test.ts` (node env; it needs no vactr
  artifact). Use a minimal valid wasm module `Uint8Array([0,97,115,109,1,0,0,0])`
  served by a fake `fetchFn`.
  - Two `exports()` calls give 1 fetch and the same object.
  - A fake that first returns `new Response('', {status: 404})` and then
    the module: the first call rejects with the message containing `404`,
    the second call resolves, and there are 2 fetches in total.
  - Construction alone -> 0 fetches.
- `editor/test/code/format.test.ts` (append):
  - `minimalChange('abc', 'abc') === null`.
  - `minimalChange('a  b', 'a b')` -> `{from: 1, to: 2, insert: ''}`, or
    the equivalent minimal range your loop yields; assert that applying
    it gives `'a b'`.
  - Japanese text before and after the change round-trips when applied.
  - `format-core.ts` and `tool-wasm.ts` source texts contain no
    `@codemirror/` (read with the `node:fs` dynamic-import idiom).
  - The existing `formatDocument`, `Shift-Alt-f` and refusal tests pass
    unchanged.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`. Write logs to
`tmp/cmp/EDS-12/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `cd editor && npx tsc --noEmit`
2. `cd editor && npx vitest run test/code/format.test.ts test/code/tool-wasm.test.ts`
   (a positive count, all passing)
3. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm && cd editor && npx vitest run test/wasm/format.test.ts`:
   the real-wasm formatter still passes through the re-export. If EDS-11
   is mid-edit, a transient failure there follows the `retryPolicy`.

## Completion Criteria

- [x] `ToolWasm`, `WasmFormatter(string | ToolWasm)` and `minimalChange`
      match the pinned contract.
- [x] `format.ts` keeps its public API through re-exports, and
      `main.ts` compiles unchanged.
- [x] The new and appended tests pass, and the existing format tests pass
      unchanged.
- [x] The two new source files contain no `@codemirror` import.
- [x] Logs are recorded, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

### Session: 2026-09-30 (EDS-12 implementation)
**Tasks Completed**: Extracted `ToolWasm` and `WasmFormatter` into
`tool-wasm.ts` and `format-core.ts`; preserved the lazy URL constructor,
shared-instance constructor, rejection retry, memory-growth-safe output
read and `finally` free. `format.ts` retains its command/key API and
re-exports the moved contract. Added minimal splice, Japanese text,
CodeMirror import-boundary and ToolWasm lazy/cache/retry tests.
**Changed file hashes** (pre -> post):
- `editor/src/code/format.ts`: `c789f38da72669f447b3d1a05010a1cf69708460b391be519158460177e253b5` -> `d6c3f61ba0eda4f64db39227836b9f4ec0f2e0e2b2f0660706baf35f8cb11f7b`
- `editor/src/code/format-core.ts`: absent -> `37197ced64557c179a6a6734ca7d86ba41b4fe5e97c0187163ee6951f1997ad8`
- `editor/src/code/tool-wasm.ts`: absent -> `bb940db5848d4626311817125b86d9a9576c1d9c779c887c296f3350b065206d`
- `editor/test/code/format.test.ts`: `b6eeb4b1146ad41d8867f3eb0fcce11d5c8cfa5e46c707cf004a87425329d89f` -> `eb9d5a6caeafb0d9eb809cc9ceacd58ea9d6db5fd6a5c173af414e9fe828d1c1`
- `editor/test/code/tool-wasm.test.ts`: absent -> `3ca8f64d8d26bfe6acec920f00795b5d9c63920d397b46ffb1906874f939bffe`
**Verification** (complete logs under `tmp/cmp/EDS-12/`):
- `cd editor && npx tsc --noEmit`: exit 0, `attempt-1/01-tsc.log` (initial run; repeated on final shared source below).
- `cd editor && npx vitest run test/code/format.test.ts test/code/tool-wasm.test.ts`: exit 0, 11 tests passed, `attempt-1/02-vitest.log` (initial run; repeated on final shared source below).
- `cd editor && npx tsc --noEmit`: exit 0, `attempt-2/01-tsc.log`.
- `cd editor && npx vitest run test/code/format.test.ts test/code/tool-wasm.test.ts`: exit 0, 11 tests passed, `attempt-2/02-vitest.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm && cd editor && npx vitest run test/wasm/format.test.ts`: exit 0, 3 tests passed, `attempt-2/03-wasm-build-format.log`.
- `git diff --check -- editor/src/code/format.ts editor/test/code/format.test.ts impl-plans/active/eds-12-format-core-tool-wasm.md`: exit 0, `attempt-2/04-diff-check.log`.
- The first wasm gate attempt exited 101 before compiling EDS-12 because concurrent CMP-10 edits had declared but not yet created `src/complete/rank.rs`, `scope.rs` and `sources.rs`. The complete captured failure is in `attempt-1/03-wasm-build-format-console.log`; the same gate passed after those files appeared. This was a resolved shared-tree transient, not an EDS-12 source failure.
- Final EDS-12 source identities are recorded in `attempt-2/source-sha256.txt`.
**Blockers**: None for this plan. Independent review, reconciliation and commit/push remain downstream workflow steps.

## Related Plans

- **Next**: CMP-30, CMP-32
