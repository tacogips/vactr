# FST-30: Editor Format Command (wasm formatter, Shift-Alt-f)

**Status**: Completed
**Plan ID**: FST-30 (wave 3)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 3.7.4, 5.4; `design-docs/user-qa/pending-formatter-syntax-questions.md` F3
**Dispatch**: `impl-plans/active/fst-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The web editor should format the document with the same Rust formatter
the CLI and LSP use. FST-22 exports `fmt_source`, `fmt_out_ptr` and
`fmt_out_len` from `vactr.wasm`. This plan adds a `Formatter` that runs a
dedicated, lazily created instance of that module. It also adds a
`formatDocument` command bound to `Shift-Alt-f` in the code pane. This
works in both tiers:
- In the native (Tauri) tier there is no core instance.
- In the browser tier, a separate instance keeps the session core's
  memory and outbox untouched.

## Non-goals

- No change to the core `WasmCore`, the worklet, `language.ts`, or
  FST-23's `syntax.ts`.
- No format-on-save and no UI button. A button is only allowed if it is
  trivial, and even then it is not required.
- Do not surface reader diagnostics. The diagnostics controller already
  shows them.

## Dependencies

- **dependsOn**: FST-22 (the exports) and FST-23 (which touches
  `mount.ts`, `main.ts` and `deps.ts` first)
- **Blocks**: FST-40

## writePaths

- `editor/src/code/format.ts` (new)
- `editor/src/code/mount.ts`: adds the format keymap only
- `editor/src/app/main.ts`: `boot()` sets `deps.formatter` in both tiers
- `editor/src/app/deps.ts`: adds `formatter?: Formatter` to `EditorDeps`
- `editor/test/code/format.test.ts` (new; jsdom)
- `editor/test/wasm/format.test.ts` (new; node environment, real `vactr.wasm`)
- this plan's Progress Log

## sharedPaths (read-only)

- `editor/test/support/wasm.ts` (`loadVactrWasm`, `callStr`, `withBytes`,
  and the rule to throw on a missing artifact)
- `editor/test/wasm/first-track.test.ts` (a node real-wasm test to imitate)
- `editor/src/code/syntax.ts` (FST-23: the pattern for optional deps)
- `src/fmt/tests/fixtures/continuation.in` and `.out` (FST-10), used as
  the node test input

## Contract

```ts
// editor/src/code/format.ts
export const FORMAT_KEY = 'Shift-Alt-f';
export interface FormatResult { status: number; text: string } // 0 formatted, 1 refused, 2 not UTF-8
export interface Formatter { format(text: string): Promise<FormatResult> }
export class WasmFormatter implements Formatter {
  constructor(wasmUrl: string, fetchFn?: (url: string) => Promise<Response>);
}
export function formatDocument(view: EditorView, formatter: Formatter): Promise<boolean>; // true when a change was dispatched
export function formatKeymap(formatter: () => Formatter | undefined): Extension;
// editor/src/app/deps.ts
formatter?: Formatter;
```

## Implementation Key Points

1. **`WasmFormatter`**:
   - Instantiate lazily. On the first `format` call, fetch `wasmUrl`,
     read the `arrayBuffer`, and call
     `WebAssembly.instantiate(bytes, {})`. Cache the promise. If loading
     fails, clear the cache so a later call retries, and reject.
   - For each call, the order is:
     1. Encode the text with `TextEncoder`.
     2. `alloc(len)` and copy the bytes into `new Uint8Array(memory.buffer)`.
     3. `status = fmt_source(ptr, len)`.
     4. Read the output from a NEW `Uint8Array(memory.buffer)` view,
        because memory may have grown. Use `fmt_out_ptr()` and
        `fmt_out_len()`.
     5. Decode with `TextDecoder`, then `free(ptr, len)`.
   - Use a `try`/`finally` so that `free` always runs.
   - `len === 0`: still call `alloc(0)`. The Rust `alloc` handles it.
2. **`formatDocument(view, f)`**:
   1. Take `before = view.state.doc.toString()` and await
      `f.format(before)`.
   2. If `status !== 0`, or the text is equal to `before`, or the
      document changed while awaiting (`view.state.doc.toString() !== before`),
      return `false`.
   3. Otherwise compute the common prefix and suffix, and dispatch ONE
      change `{ from, to, insert }` with `userEvent: 'format'`.
   4. Return `true`.
   - A formatter rejection is caught, and the function returns `false`.
3. **`formatKeymap(get)`**: this is
   `keymap.of([{ key: FORMAT_KEY, run: (view) => { const f = get(); if (!f) return false; void formatDocument(view, f); return true; } }])`.
   Add it in `mount.ts` before the `defaultKeymap` entry, using
   `() => deps.formatter`.
4. **`main.ts` `boot()`**: in both tiers set
   `deps.formatter = new WasmFormatter(new URL('vactr.wasm', win.document.baseURI).href)`.
   Loading is lazy, so boot does not change and makes no extra fetch
   until the first use.
5. **`Shift-Alt-f` conflicts.** Check that `defaultKeymap` and
   `historyKeymap` do not bind `Shift-Alt-f` (on macOS, CodeMirror maps
   Alt to Option). Record the result of the check.
6. **The sync extension** sends the change like any other edit, so do not
   call the sync API directly.

## Tests

**`editor/test/code/format.test.ts`** (jsdom, fake `Formatter`):
- Doc `a\n\t\t\tb\n` with a fake returning `{0, 'a\n\tb\n'}`: exactly
  one transaction, whose change covers only the indent region, with
  `userEvent` `format`. The final doc is `a\n\tb\n`.
- Fake returning status 1: the doc is unchanged, no transaction, and
  the result is `false`.
- Fake returning the same text: no transaction.
- Fake that resolves after the doc was edited in the meantime: the
  result is not applied, and the function returns `false`.
- Fake that rejects: `false`, and no error escapes.
- The keymap binds `FORMAT_KEY`, and running the binding with no
  formatter returns `false`.
- Mounting the code area with `deps.formatter` set to a fake, then
  dispatching the key through `runScopeHandlers` or the keymap facet,
  formats the doc.

**`editor/test/wasm/format.test.ts`** (`// @vitest-environment node`,
real `vactr.wasm` via `loadVactrWasm()`):
- Read `continuation.in` and `.out` from `../src/fmt/tests/fixtures/`.
  `fmt_source` returns 0, and the output equals `.out`.
- Formatting `.out` again returns status 0 and the same text
  (idempotence).
- `s "abc\n` returns status 1, and the output equals the input.
- A non-UTF-8 byte sequence `[0xff]` (written with `withBytes`) returns
  status 2.
- `WasmFormatter` given a `fetchFn` that returns
  `new Response(bytes of the artifact)` formats `continuation.in` to
  `.out`.
- After `fmt_source`, the session outbox is still empty
  (`drainRecords()` returns `[]`).

## Execution Protocol

- Before each edit to `mount.ts`, `main.ts` or `deps.ts`, re-read the
  file. FST-23 changed these files in wave 2.
- Record the sha256 and your intent before each edit, and the post-edit
  sha256 after it.
- Stop if the file differs from the state FST-23 recorded in its
  Progress Log.
- Do not change git state.
- Write logs to `tmp/fst/FST-30/<n>-<name>.log`, each ending with
  `exit=<status>`.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   (the artifact with the `fmt_*` exports).
2. `mise run ts-build-wasm`, so the build step below can include the syntax assets.
3. `cd editor && npm run check`
4. `cd editor && npm test`: all tests pass, including both new format
   files and FST-23's files.
5. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` must succeed.
6. `wc -l editor/src/code/format.ts editor/src/code/mount.ts`: each file
   is under 400 lines.

## Completion Criteria

- [x] `Formatter`, `WasmFormatter`, `formatDocument` and `formatKeymap` match the contract.
- [x] `Shift-Alt-f` formats the document in both tiers, verified by a mounted jsdom fake-formatter test and real wasm ABI/API tests.
- [x] Verification steps 1-6 pass, each with a log.
- [x] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 222 step 4).
**Blockers**: Waits for FST-22 and FST-23.

### Session: 2026-09-30 (Step 6 implementation)
**Tasks Completed**: Added `editor/src/code/format.ts` with lazy dedicated wasm loading, memory-safe formatter ABI access, stale-result protection, minimal single-change dispatch and the `Shift-Alt-f` keymap. Added `EditorDeps.formatter`, configured the formatter URL in both tiers, and mounted the keymap before default/history bindings while preserving FST-23 syntax integration. Added jsdom fake-formatter tests and real-wasm tests, including ABI statuses, idempotence, outbox isolation and loader caching. Confirmed `defaultKeymap` and `historyKeymap` do not bind `Shift-Alt-f`.
**Final-Source Verification**: `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` exit 0 (`tmp/fst/FST-30/25-wasm-build-final.log`); `mise run ts-build-wasm` exit 0 (`26-ts-build-wasm-final.log`); `cd editor && npm run check` exit 0 (`27-editor-check-final-source.log`); `cd editor && npm test` exit 0, 61 files / 374 tests passed (`28-editor-test-final-source.log`); `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` exit 0 (`29-editor-build-final-source.log`); `wc -l editor/src/code/format.ts editor/src/code/mount.ts` exit 0, 108 and 239 lines (`30-line-count-final-source.log`).
**Prior Attempts**: Initial typecheck caught and resolved the `Extension` import location (`12-editor-check.log`, exit 1; final-source rerun passed). Initial full suite caught a missing `buildLayout` in the new mount test (`15-editor-test.log`, 373/374 passed; final-source rerun passed).
**Blockers**: None for implementation. FST-40 combined-tree review and full project gate remain downstream.

### Session: 2026-09-30 (FST-40 combined-tree closeout)
**Tasks Completed**: The serial combined-tree gate passed after rebuilding the current formatter WASM. The full editor suite passed 374/374 tests, typecheck and production build passed, and `dist/` contained the formatter, grammar, query, and web-tree-sitter WASM assets. The FST-40 archive move remains orchestrator-owned.
**Verification**: `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` plus export assertion (exit 0, `tmp/fst/FST-40/07-wasm-build-and-exports.log`); `cd editor && npm run check` (exit 0, `12-editor-check.log`); `cd editor && npm test` (61 files, 374/374 tests, exit 0, `13-editor-test.log`); `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` (exit 0, `14-editor-build.log`); dist asset listing (exit 0, `15-editor-dist-assets.log`).

## Related Plans

- **Depends On**: FST-22, FST-23
- **Next**: FST-40
