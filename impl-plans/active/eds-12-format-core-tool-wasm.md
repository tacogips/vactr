# EDS-12: CodeMirror-free Format Core and Shared `ToolWasm`

**Status**: Ready
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

- [ ] `ToolWasm`, `WasmFormatter(string | ToolWasm)` and `minimalChange`
      match the pinned contract.
- [ ] `format.ts` keeps its public API through re-exports, and
      `main.ts` compiles unchanged.
- [ ] The new and appended tests pass, and the existing format tests pass
      unchanged.
- [ ] The two new source files contain no `@codemirror` import.
- [ ] Logs are recorded, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

## Related Plans

- **Next**: CMP-30, CMP-32
