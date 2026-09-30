# FST-23: Editor Highlighting via web-tree-sitter with StreamLanguage Fallback

**Status**: Completed
**Plan ID**: FST-23 (wave 2; parallel with FST-20, FST-21, FST-22)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 4.5, 5.1-5.4, 6
**Dispatch**: `impl-plans/active/fst-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The user wants the `.vact` syntax to run as WASM on the web. FST-11
produces `tree-sitter-vact.wasm` and `queries/highlights.scm`. This plan
loads both with `web-tree-sitter@0.27.0` in the CodeMirror 6 editor and
highlights through them. The existing `StreamLanguage` mode
(`editor/src/code/language.ts`, `vactLanguage()`) is kept unchanged as
the initial language, and it stays in use whenever loading fails. Every
existing jsdom test mounts without a loader, so those tests see no
behavior change.

## Non-goals

- No change to `language.ts`, `code.css`, `highlight.ts` (playing-step
  highlighting) or any protocol code.
- No incremental `tree.edit` path, no folding UI and no Lezer grammar.
- No format command (that is FST-30).
- No commit of wasm files into `editor/public`. The assets are emitted by
  the vite plugin.

## Dependencies

- **dependsOn**: FST-11 (the grammar wasm, `highlights.scm` and the node
  types)
- **Blocks**: FST-30 (which shares `mount.ts`, `main.ts` and `deps.ts`),
  FST-40

## writePaths

- `editor/src/code/syntax.ts` (new)
- `editor/src/code/mount.ts`: only the language `Compartment`, the
  `data-syntax` attribute and the loader wiring
- `editor/src/app/main.ts`: `boot()` sets `deps.syntax` in both tiers
- `editor/src/app/deps.ts`: adds `syntax?: SyntaxLoader` to `EditorDeps`
- `editor/vite.config.ts`: optional syntax assets in `vactr-assets`
- `editor/package.json`: adds the dependency
  `"web-tree-sitter": "0.27.0"` (exact version, no caret)
- `editor/package-lock.json`: updated only by `npm install`
- `editor/test/code/syntax.test.ts` (new; node environment, real wasm)
- `editor/test/code/syntax-fallback.test.ts` (new; jsdom)
- this plan's Progress Log

## sharedPaths (read-only)

- `tree-sitter-vact/queries/highlights.scm`,
  `tree-sitter-vact/src/node-types.json` and
  `tree-sitter-vact/tree-sitter-vact.wasm` (the last one is built by
  `mise run ts-build-wasm`; it is ignored and not committed)
- `editor/src/code/language.ts`: `HEADS` and the `vact-tok-*` class
  names at 137-146
- `editor/test/support/wasm.ts`: the node fs pattern (`nodeFs` with a
  non-literal dynamic import) and the rule to throw on a missing file
- `editor/test/code/transport.test.ts` and `editor/test/app/main.test.ts`:
  how jsdom tests build `EditorDeps` and mount the code area
- `examples/*.vact`, `src/prelude/templates.vact`

## Contract (pinned; FST-30 and the tests rely on it)

```ts
// editor/src/code/syntax.ts
export interface SyntaxCapture { name: string; from: number; to: number } // UTF-16 offsets
export interface ParsedVact { captures(from: number, to: number): SyntaxCapture[]; delete(): void }
export interface VactSyntax { parse(text: string): ParsedVact }
export type SyntaxLoader = () => Promise<VactSyntax>;
export const CAPTURE_CLASSES: Readonly<Record<string, string>>; // capture name -> vact-tok-* class (design 4.5 table)
export function loadVactSyntax(base: string): Promise<VactSyntax>;
export function treeSitterHighlighting(syntax: VactSyntax): Extension;
// editor/src/app/deps.ts
syntax?: SyntaxLoader;   // new optional EditorDeps field
```

`CAPTURE_CLASSES` has exactly these entries:
- `comment` -> `vact-tok-comment`
- `comment.directive` -> `vact-tok-directive`
- `string` -> `vact-tok-string`
- `number` -> `vact-tok-number`
- `string.special.symbol` -> `vact-tok-keyword`
- `string.special.path` -> `vact-tok-path`
- `keyword` -> `vact-tok-head`
- `punctuation.bracket` -> `vact-tok-bracket`

## Implementation Key Points

1. **`loadVactSyntax(base)`**
   - Load web-tree-sitter with `await import('web-tree-sitter')`, so it
     gets its own chunk.
   - Call `Parser.init({ locateFile: (name) => new URL(name, base).href })`.
   - Call `Language.load(new URL('tree-sitter-vact.wasm', base).href)`.
   - Fetch `highlights.scm` from the same base and build the query with
     `new Query(language, text)`.
   - Wrap all of this in the `VactSyntax` interface. Any rejection
     propagates to the caller, which then falls back.
   - Check the installed 0.27.0 type definitions in
     `node_modules/web-tree-sitter/*.d.ts` for the exact API names. The
     current releases export `Parser`, `Language` and `Query` by name.
2. **`captures(from, to)`**
   - Call `query.captures(tree.rootNode, { startIndex: from, endIndex: to })`,
     or the 0.27 equivalent.
   - Map each result to `{ name, from: node.startIndex, to: node.endIndex }`.
   - Offsets must be UTF-16 code units. web-tree-sitter parses JS
     strings as UTF-16. The node test checks this with a line that
     contains non-ASCII text before a token.
3. **`treeSitterHighlighting(syntax)`**
   - A `ViewPlugin` that keeps one `ParsedVact`. It reparses the full
     text on `docChanged` and calls `delete()` on the old parse.
   - It builds `Decoration.mark({ class })` over `view.visibleRanges`.
   - It skips captures that have no class mapping.
   - When captures cover the same range, keep the first one in query
     order.
   - Sort the ranges before `RangeSetBuilder.add`. CodeMirror requires
     `from` in ascending order, and among ranges with the same `from`,
     `startSide` ascending. This is a common crash source.
   - Call `delete()` on the parse in `destroy()`.
   - Return
     `[plugin, EditorState.languageData.of(() => [{ commentTokens: { line: '#' } }])]`.
4. **`mount.ts`**
   - Wrap `vactLanguage()` in `const language = new Compartment()`.
   - Set `codePane.dataset.syntax = 'fallback'` at mount.
   - If `deps.syntax` exists, call it. When the promise resolves and the
     view has not been disposed, dispatch
     `language.reconfigure(treeSitterHighlighting(syntax))` and set
     `dataset.syntax = 'tree-sitter'`.
   - On rejection, do nothing: the view stays on `fallback`. Do not
     throw, and do not add an unhandled rejection.
   - Change nothing else in `mount.ts`.
5. **`main.ts`**: in `boot()`, after `deps` is built, in both tiers, set
   `deps.syntax = () => loadVactSyntax(win.document.baseURI)`.
6. **`vite.config.ts`**: extend `vactr-assets` with OPTIONAL assets.
   - The assets:
     - the grammar, from `$VACTR_TS_WASM`, else
       `../tree-sitter-vact/tree-sitter-vact.wasm`, emitted as
       `tree-sitter-vact.wasm`;
     - `../tree-sitter-vact/queries/highlights.scm`, emitted as
       `highlights.scm`;
     - the runtime wasm from `node_modules/web-tree-sitter/`, emitted
       under its own file name. Confirm that name in the installed
       package; it is expected to be `web-tree-sitter.wasm`.
   - A missing optional asset: call `this.warn(...)` and skip it.
     `vactr.wasm` stays mandatory.
   - The dev middleware serves the same three names, and calls `next()`
     when a file is missing.
   - If `vite build` fails on node built-ins that web-tree-sitter
     references, the minimal fix is to add `web-tree-sitter` to
     `optimizeDeps.exclude`, or to mark those built-ins external. Record
     which fix was needed.
7. **`package.json` and lockfile**
   - Run `npm install --save-exact web-tree-sitter@0.27.0` in `editor/`.
     Do not hand-edit the lockfile.
   - If the sandbox has no network, record the command as BLOCKED with
     its stderr. The orchestrator runs it serially, and this plan's
     other work continues against the type interfaces.
8. **Pitfalls**
   - `language.test.ts` and every other existing test must pass
     unchanged. No existing test may need a loader.
   - Do not import `web-tree-sitter` statically from `mount.ts` or
     `main.ts`. That would pull it into jsdom tests and the main chunk.

## Tests

**`editor/test/code/syntax.test.ts`**
- It starts with `// @vitest-environment node`.
- It reads the runtime wasm from `node_modules/web-tree-sitter/`, and
  the grammar from `$VACTR_TS_WASM`, else
  `../tree-sitter-vact/tree-sitter-vact.wasm`. A missing file THROWS, as
  `test/support/wasm.ts` does.
- It initializes with `locateFile` pointing to the local runtime file,
  and passes bytes to `Language.load`.
- Cases, each `input -> expected`:
  - A sample with a statement, a deep continuation, a block, a pair and
    an interpolated string gives the expected node types in
    `rootNode.toString()`: `statement`, `continuation`, `block`, `pair`,
    `interpolation`.
  - Every committed `.vact` file (scan `../examples` and
    `../src/prelude`; at least 10 files) gives `rootNode.hasError === false`.
  - `let x 1 # c` gives the capture names `keyword` for `let`, `number`
    for `1`, and `comment` for `# c`.
  - `#@ gain 0.5` gives `comment.directive`.
  - `:bd` gives `string.special.symbol`.
  - `./a.wav` gives `string.special.path`.
  - In `# é\nlet x 1`, the capture for `let` starts at UTF-16 offset 4.
  - Every capture name in `highlights.scm` (matched with the regex
    `@[\w.]+` outside `#any-of?` arguments) has a `CAPTURE_CLASSES`
    entry.
  - The `#any-of? @keyword` string list equals `HEADS`, in order.

**`editor/test/code/syntax-fallback.test.ts`** (jsdom)
- Mount with `deps.syntax = () => Promise.reject(new Error('x'))`, then
  flush microtasks. The code pane has `data-syntax="fallback"`, and
  typing `let x 1` produces the `vact-tok-head` and `vact-tok-number`
  classes from the stream mode.
- Mount with a resolving fake `VactSyntax`, whose `parse` returns fixed
  captures such as `{ name: 'keyword', from: 0, to: 3 }`. Then
  `data-syntax="tree-sitter"`, and the `let` range carries
  `vact-tok-head`.
- Mount without `deps.syntax`: `data-syntax="fallback"`, and nothing
  else changes.
- Dispose before the loader resolves: there is no error and no dispatch
  on the destroyed view.

## Execution Protocol

- Before each edit to `mount.ts`, `main.ts`, `deps.ts`, `vite.config.ts`
  or `package.json`, re-read the file. Record its sha256 and your intent
  in the Progress Log, and the post-edit sha256 after the edit. Stop if
  the file drifted.
- Make no git state changes.
- The wasm32 build compiles `src/host/wasm`, which FST-22 edits
  concurrently. A compile error in an FST-22 file is transient: never
  edit it. Wait and retry as the `fst-dispatch.json` `retryPolicy`
  describes.
- Write logs to `tmp/fst/FST-23/<n>-<name>.log`, each ending with
  `exit=<status>`.

## Verification (exit 0, complete logs)

1. `mise run ts-build-wasm`. If it is BLOCKED in the sandbox, use the
   artifact FST-11 built and record that.
2. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
   Existing real-wasm editor suites need `vactr.wasm`.
3. `cd editor && npm install --save-exact web-tree-sitter@0.27.0` (once),
   then `grep '"web-tree-sitter": "0.27.0"' package.json`.
4. `cd editor && npm run check`: tsc reports no errors.
5. `cd editor && npm test`: every test passes, including the two new
   files, and no existing test changed.
6. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`, then
   `ls dist/tree-sitter-vact.wasm dist/highlights.scm dist/web-tree-sitter.wasm`
   (or the confirmed runtime name). All three files exist.
7. `wc -l editor/src/code/syntax.ts editor/src/code/mount.ts` shows each
   file under 400 lines.

## Completion Criteria

- [x] The contract is implemented, and `CAPTURE_CLASSES` covers every capture in `highlights.scm`.
- [x] The fallback is the default. The tree-sitter path is used only after the loader resolves.
- [x] `web-tree-sitter` is pinned at exactly `0.27.0`, and the lockfile was updated by npm.
- [x] Verification 1-7 pass, or a BLOCKED network or cache step is recorded for serial rerun.
- [x] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 222 step 4).
**Blockers**: Waits for FST-11.

### Session: 2026-09-30 (FST-23 implementation)
**Tasks Completed**: Added the `web-tree-sitter@0.27.0` loader and capture mapping in `editor/src/code/syntax.ts`; retained StreamLanguage as the initial mode and fallback in `editor/src/code/mount.ts`; wired the optional loader in both `boot()` tiers; emitted and served optional grammar/query/runtime WASM assets through Vite; added real-WASM parse/highlight tests and jsdom fallback/lifecycle tests.
**Verification**: `mise run ts-build-wasm` (exit 0, `tmp/fst/FST-23/09-ts-build-wasm.log`); `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` (exit 0, `10-wasm-build.log`); `cd editor && npm install --save-exact web-tree-sitter@0.27.0` (exit 0, `03-npm-install.log`); package pin grep (exit 0, `13-package-pin.log`); `npm run check` (exit 0, `12-editor-check-final.log`); `npm test` (59 files, 364 tests passed, exit 0, `08-editor-test-rerun.log`); production build (exit 0, `11-editor-build.log`) and required dist asset listing (exit 0, `14-dist-assets.log`); line-count check (121 and 237 lines, exit 0, `15-line-count.log`).
**Resolved During Implementation**: An initial strict typecheck found the 0.27.0 `locateFile` callback needed an explicit parameter type. The first editor test run also exposed a combined fixture that produced a grammar ERROR and a check that concatenated two identical keyword predicate lists. Both test/setup issues were corrected; the final check and full editor suite pass.
**Downstream**: FST-40 owns combined-tree review and final integration gate; no review approval is claimed here.
**Post-edit SHA256** (`tmp/fst/FST-23/16-post-edit-sha256.txt`): `editor/src/code/mount.ts` 881a60989555df5eb284cd33b9cdad2b1c93ee669f1a5250004015678cac2fa1; `editor/src/app/deps.ts` c0a5f51680991daabc738c9bbb1fe5b8e820305393c8cefff28bf7029ba57f35; `editor/src/app/main.ts` 6b59baf13eeba4404e84698a8f28abe2b8cc36ed3f40315efcee49f50c405f24; `editor/vite.config.ts` 12301700ff31ec9df1bebdd05eb90f4c99ac681f5665a7c58fe53eb1e18a784c; `editor/package.json` ee53ada5f6b09e61f23e3a956506d909c506e886f707541a1fb9f53e5a8a020e; `editor/package-lock.json` 18b27dd32efd167069ab7320d7ae8b59e36adcd7f036f1b465d86c1b4c5c40f4; `editor/src/code/syntax.ts` 60a8d3fa95ef7d13ced1c68b3e1e3248c7ee06973a42e13ec32cc0804d360829; `editor/test/code/syntax.test.ts` afbb9757257604128b8b312dae508c1b8e03b01af491c7e56cd4a57801b50789; `editor/test/code/syntax-fallback.test.ts` dd547b55923183259ae17c6ad5d7c9fb0109f8eb8f5a54a8318abb5d71fb43ef.

## Related Plans

- **Depends On**: FST-11
- **Next**: FST-30, FST-40
