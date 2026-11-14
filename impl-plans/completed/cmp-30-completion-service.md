# CMP-30: Editor Completion Service (`completion.ts`)

**Status**: Completed
**Plan ID**: CMP-30 (wave 2; parallel with CMP-20, CMP-21, CMP-31)
**Design Reference**: `design-docs/specs/design-completion.md` 6.1, 6.6, 7.4 (the service bullets)
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan adds the UI-agnostic service that the popup, and later the
canvas editor, use. It is plain TypeScript with no DOM and no CodeMirror.

- It turns `(text, UTF-16 cursor)` into a UTF-8 byte cursor, calls a
  `CompletionEngine`, and maps the byte ranges back to UTF-16.
- It disables itself permanently after the first engine failure, so the
  editor keeps working without wasm.
- `WasmCompletionEngine` is the real engine. It sits on the shared
  `ToolWasm` from EDS-12 and calls the CMP-21 exports.

## Non-goals

- No DOM, no timers, no `@codemirror/*` import.
- No popup logic (CMP-31) and no wiring (CMP-32).
- No retry after the engine fails.

## Dependencies

- **dependsOn**: CMP-15 (the types), EDS-12 (`ToolWasm`)
- **Blocks**: CMP-32, CMP-40

## writePaths

- `editor/src/code/completion.ts` (new)
- `editor/test/code/completion.test.ts` (new)
- `impl-plans/active/cmp-30-completion-service.md` (Progress Log only)

## sharedPaths (read only)

`editor/src/code/completion-types.ts` (CMP-15), `editor/src/code/tool-wasm.ts`
(EDS-12), `editor/src/code/format-core.ts` (the exports-call pattern:
alloc, set, call, fresh view, slice, free in `finally`).

## Pinned Exports

```ts
export function utf16ToUtf8(text: string, cursor16: number): number;
export function utf8ToUtf16(text: string, byteOffset: number): number;
export class WasmCompletionEngine implements CompletionEngine { constructor(tool: ToolWasm) }
export class CompletionService implements CompletionSource { constructor(engine: CompletionEngine, limit?: number) } // limit default 100
```

## Key Points

- `utf16ToUtf8`:
  - Clamp `cursor16` to `[0, text.length]`.
  - If `cursor16` sits between the two halves of a surrogate pair, step
    back one unit.
  - Count bytes by code point: under U+0080 is 1, under U+0800 is 2, a
    BMP code point otherwise is 3, and a surrogate pair is 4.
- `utf8ToUtf16`: walk `text` accumulating the same byte widths, and
  return the UTF-16 index where the byte count first reaches or exceeds
  `byteOffset`. Clamp to `text.length`.
- `WasmCompletionEngine.complete(text, byteCursor, limit)`:
  1. `await tool.exports()`, which rejects when unavailable. Also reject
     if `complete_source`, `complete_out_ptr` or `complete_out_len` is
     not a function.
  2. Encode the text with `TextEncoder`, `alloc`, copy, and call
     `complete_source(ptr, len, byteCursor, limit)`.
  3. After the call, create a NEW `Uint8Array(memory.buffer)`, `slice`
     the output, and `free(ptr, len)` in `finally`.
  4. Status other than 0 -> `null`. Otherwise `JSON.parse`, returning a
     `RawCompletion`.
- `CompletionService.complete(text, cursor16)`:
  - If `!available`, return `null`.
  - Call the engine with `utf16ToUtf8(text, cursor16)`.
  - On rejection or a thrown error, set `available = false` and return
    `null`. It must NEVER throw or reject.
  - `null`, or `context === 'none'`, -> `null`.
  - Otherwise return a `CompletionResult` with `from` and `to` mapped by
    `utf8ToUtf16` and the items passed through unchanged.
- Do not cache results across different texts.

## Tests (`editor/test/code/completion.test.ts`, node env; fake `CompletionEngine`)

- ASCII `abc` -> `utf16ToUtf8('abc', 2) === 2`, and the reverse is 2.
- The text `# ` + U+65E5 U+672C + `\nsi`, with the cursor at the end
  (UTF-16 length 7) -> byte cursor 11. A fake returning
  `from: 9, to: 11` maps to UTF-16 `from: 5, to: 7`.
- U+20BB7 (a surrogate pair) + `a`:
  - `utf16ToUtf8(text, 1)` (inside the pair) equals
    `utf16ToUtf8(text, 0)`, which is 0;
  - `utf16ToUtf8(text, 2) === 4`;
  - `utf8ToUtf16(text, 4) === 2`.
- A string holding U+97F3 before the cursor maps correctly both ways.
- Mapping: a fake returning 2 items -> the same labels, kinds, details
  and inserts, in the same order, and `incomplete` is passed through.
- A fake returning `context: 'none'` -> `null`. A fake returning `null`
  -> `null`.
- No-wasm fallback: a fake that rejects -> the first call resolves to
  `null`, `available` becomes `false`, and a second call returns `null`
  without calling the engine (the call count stays 1). Nothing throws.
- `WasmCompletionEngine` over a `ToolWasm` whose `exports()` rejects:
  the service resolves to `null` and becomes unavailable.
- A source-text check: `completion.ts` contains no `@codemirror/`.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`. Write logs to
`tmp/cmp/CMP-30/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `cd editor && npx tsc --noEmit`
2. `cd editor && npx vitest run test/code/completion.test.ts` (a positive
   count, all passing)

## Completion Criteria

- [x] The pinned exports exist, and the service never throws.
- [x] The UTF-16 and UTF-8 helpers are correct for Japanese text and
      surrogate pairs.
- [x] The fallback disables the engine after one failure.
- [x] `completion.ts` has no `@codemirror` import.
- [x] Logs are recorded, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: Waits for CMP-15 and EDS-12.

### Session: 2026-09-30 (session 226, step 6 implementation)
**Tasks Completed**: Added `editor/src/code/completion.ts` with the pinned UTF-16/UTF-8 helpers, shared-ToolWasm completion engine and permanent first-failure fallback. Added `editor/test/code/completion.test.ts` covering ASCII, Japanese text, surrogate pairs, result mapping, status/export handling, fallback, and the CodeMirror import boundary. Current source hashes: `completion.ts` 7038cf116edd9bcb063db3a3fb7033efa98cf502e7b113241b7f396a4950553c; `completion.test.ts` 7ab26635d5d0bf954ee63d7c6b085b78c0db423fe777c61eee7becd56bad667c.
**Verification**: `cd editor && npx vitest run test/code/completion.test.ts` exit=0, 12/12 tests passed; complete log `tmp/cmp/CMP-30/attempt-1/vitest.log`. `cd editor && npx tsc --noEmit` exit=1; complete log `tmp/cmp/CMP-30/attempt-1/tsc.log`; blocked by sibling-owned `editor/src/code/completion-popup.ts:114` (TS2678: `Ctrl-Space` is excluded by the narrowed switch union). CMP-30 does not own that path; retry after sibling changes stabilize.
**Blockers**: Required TypeScript gate has no passing result yet because of the sibling-owned popup type error. Resume criterion: after CMP-31 repairs or completes `editor/src/code/completion-popup.ts`, rerun `cd editor && npx tsc --noEmit` and record exit=0.

### Session: 2026-09-30 (session 226, step 6 stable-tree verification)
**Tasks Completed**: CMP-15 and EDS-12 dependencies were accepted by dispatch. All five CMP-30 completion criteria are met. The initial TypeScript failure was in sibling-owned `editor/src/code/completion-popup.ts:114`; after that shared source stabilized, the required typecheck passed. The plan remains In Progress pending downstream review and CMP-40 closeout.
**Verification**: `cd editor && npx tsc --noEmit` exit=0; complete log `tmp/cmp/CMP-30/attempt-2/tsc.log`. `cd editor && npx vitest run test/code/completion.test.ts` exit=0, 12/12 passed; complete log `tmp/cmp/CMP-30/attempt-2/vitest.log`. The earlier sibling-caused tsc exit=1 remains recorded at `tmp/cmp/CMP-30/attempt-1/tsc.log`; the retry is on the stable shared tree. Current source hashes: `completion.ts` 7038cf116edd9bcb063db3a3fb7033efa98cf502e7b113241b7f396a4950553c; `completion.test.ts` 7ab26635d5d0bf954ee63d7c6b085b78c0db423fe777c61eee7becd56bad667c.
**Blockers**: None for CMP-30 implementation or its required gates. Formal review, CMP-32 wiring, and CMP-40 integration/closeout remain downstream.

## Related Plans

- **Depends On**: CMP-15, EDS-12
- **Next**: CMP-32
