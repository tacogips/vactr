# EDS-10: CodeMirror-free Tree-sitter Span Core

**Status**: Completed
**Plan ID**: EDS-10 (wave 1; parallel with CMP-10, CMP-15, FST-50, EDS-11, EDS-12)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 5.5 (syntax part), 5.3, 4.5
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The GPU canvas editor (design-implementation 15.3 on `origin/main`)
draws syntax itself and cannot use CodeMirror `Decoration`s. Today,
`editor/src/code/syntax.ts` mixes the tree-sitter loader and capture
logic with a CodeMirror `ViewPlugin`. This plan moves every
CodeMirror-free part into `syntax-core.ts`, and adds `styleSpans`, so
that any renderer can get `{from, to, cls}` spans. `syntax.ts` becomes a
thin decoration adapter with IDENTICAL highlighting output.

## Non-goals

- No change to highlighting behavior, `CAPTURE_CLASSES`, the loader
  URLs, `language.ts`, `mount.ts`, `main.ts`, `deps.ts` or the
  `tree-sitter-vact/` sources.
- No incremental `tree.edit`.

## Dependencies

- **dependsOn**: none
- **Blocks**: CMP-40

## writePaths

- `editor/src/code/syntax-core.ts` (new)
- `editor/src/code/syntax.ts`
- `editor/test/code/syntax-core.test.ts` (new)
- `impl-plans/active/eds-10-syntax-span-core.md` (Progress Log only)

## sharedPaths (read only)

`editor/test/code/syntax.test.ts` (the pattern for loading the real
runtime and grammar wasm), `editor/test/code/syntax-fallback.test.ts`,
`editor/src/code/language.ts` (the `vact-tok-*` class names and `HEADS`;
do NOT edit it), `tree-sitter-vact/queries/highlights.scm`.

## File-level Changes

1. `syntax-core.ts`: move these unchanged from `syntax.ts`:
   `SyntaxCapture`, `ParsedVact`, `VactSyntax`, `SyntaxLoader`,
   `CAPTURE_CLASSES`, `TreeSitterModule` (internal), `parserInit`,
   `loadVactSyntax` and `createVactSyntax`. Imports: `web-tree-sitter`
   types only.

   Add:

   ```ts
   export interface StyleSpan { from: number; to: number; cls: string }
   export function styleSpans(parsed: ParsedVact, from: number, to: number): StyleSpan[];
   ```

   Its semantics equal today's `decorations()` for one range, without
   `Decoration`:
   - map each capture through `CAPTURE_CLASSES`;
   - skip unmapped captures and captures with `from >= to`;
   - deduplicate by `${from}:${to}`, where the first capture in query
     order wins;
   - sort by `(from, to)`.
2. `syntax.ts`:
   - Import from `./syntax-core`.
   - `decorations(view, parsed)` loops over `view.visibleRanges`, calls
     `styleSpans` for each range, and keeps ONE `seen` set across
     ranges, exactly as today. So a span in two visible ranges is emitted
     once. It maps each span to `Decoration.mark({ class: span.cls })`.
   - `treeSitterHighlighting` is unchanged.
   - Add
     `export { CAPTURE_CLASSES, createVactSyntax, loadVactSyntax } from './syntax-core'`
     and
     `export type { ParsedVact, SyntaxCapture, SyntaxLoader, VactSyntax, StyleSpan } from './syntax-core'`,
     so existing imports in `main.ts`, `deps.ts`, `mount.ts` and tests
     keep working.

## Key Points and Pitfalls

- `syntax-core.ts` must not import `@codemirror/state` or
  `@codemirror/view`, not even as a type-only import.
- Keep the comment about web-tree-sitter byte versus code-unit indexes,
  and the filtering approach, exactly as they are. Do not switch to
  `QueryOptions` `startIndex`.
- Do not change the order of `languageData` or the plugin in
  `treeSitterHighlighting`.

## Tests (`editor/test/code/syntax-core.test.ts`, `// @vitest-environment node`)

- At the top, before the imports that load code:
  `vi.mock('@codemirror/view', () => { throw new Error('syntax-core must not load @codemirror/view'); })`,
  plus the same for `@codemirror/state`. Then import only from
  `../../src/code/syntax-core`.
- Load the runtime and grammar exactly as `syntax.test.ts` does. A
  missing file THROWS. Build `createVactSyntax(parser, query)`.
- The sample is `let a 1\n# ` + U+65E5 U+672C + `\ns :bd > d1 "x" [1]\n`
  (write the non-ASCII part with `\u` escapes). Assert that
  `styleSpans(parsed, 0, text.length)`:
  - contains `vact-tok-head` at the UTF-16 range of `let`;
  - contains `vact-tok-comment` covering the comment line;
  - contains `vact-tok-keyword` at `:bd`, `vact-tok-string` at `"x"`,
    `vact-tok-bracket` at `[` and `]`, and `vact-tok-number` at `1`;
  - is sorted and has no duplicate `(from, to)`.
- A range query `styleSpans(parsed, fromOfLine3, toOfLine3)` returns no
  span that ends before line 3 starts.
- A source-text check: `syntax-core.ts` contains no `@codemirror/`.
- The existing `syntax.test.ts` and `syntax-fallback.test.ts` stay green
  unchanged.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`: fresh read, sha256 before and
after each edit, intent line, stop on drift, no git state changes. Write
logs to `tmp/cmp/EDS-10/attempt-<n>/`.

If the grammar wasm is missing, run `mise run ts-build-wasm` first. If
that is blocked by the sandbox (network or `~/.cache/tree-sitter`),
record it as BLOCKED with the command, exit code and stderr.

## Verification (exit 0, complete logs)

1. `cd editor && npx tsc --noEmit`
2. `cd editor && npx vitest run test/code/syntax-core.test.ts test/code/syntax.test.ts test/code/syntax-fallback.test.ts`
   (a positive count, all passing)

## Completion Criteria

- [x] `syntax-core.ts` exports the moved symbols plus `StyleSpan` and
      `styleSpans`, and it has no `@codemirror` import.
- [x] `syntax.ts` is a thin adapter that re-exports the moved symbols,
      and no other file changed.
- [x] The new test passes with the mocks that throw, and the existing
      syntax tests pass.
- [x] Logs are recorded, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

### Session: 2026-09-30 (implementation)
**Tasks Completed**: Moved the tree-sitter capture types, classes, loader and parser to `editor/src/code/syntax-core.ts`; added sorted, first-capture-wins `styleSpans`; kept `syntax.ts` as the CodeMirror adapter with shared visible-range deduplication; added the throwing-mock core test. No EDS-10 files outside the plan's writePaths were changed.
**Pre-edit hashes / intent**:
- `editor/src/code/syntax.ts` `716c5b09bb38b37737837efb99c1457afe0e8f8c9244da2e59bc4b03f37a5bcb`: keep the current CodeMirror plugin and visible-range behavior; replace capture mapping with `styleSpans`, and re-export the moved API.
- `editor/src/code/syntax-core.ts`: absent in the authoritative pre-node snapshot; move capture types, class map, loader and parser implementation here, adding sorted deduplicated style spans.
- `editor/test/code/syntax-core.test.ts`: absent in the authoritative pre-node snapshot; add the node test with throwing CodeMirror mocks and assertions from the plan.
- `impl-plans/active/eds-10-syntax-span-core.md` `1526d633a83f68d018d901ca6f4d7b21cf52b852bbaa23b0f0446d93dedba079`: append implementation intent and source hashes before editing the adapter.
- Adapter result: `editor/src/code/syntax.ts` is now `d83962dcf1a1a8b6a44fc8a565d4e05f1ebf3835c6c70d813f87655f101e539d`; `editor/src/code/syntax-core.ts` is `8aab1b65c59afc8ee4f1b421c0842ee566b97432dac282222c6a27b4a565e52a`.
- Next edit intent: add `editor/test/code/syntax-core.test.ts` with throwing CodeMirror mocks, real runtime/grammar WASM setup, requested style/range assertions and a source check; test file was absent in the fanout snapshot.
**Post-edit hashes**:
- `editor/src/code/syntax-core.ts`: `8aab1b65c59afc8ee4f1b421c0842ee566b97432dac282222c6a27b4a565e52a`
- `editor/src/code/syntax.ts`: `d83962dcf1a1a8b6a44fc8a565d4e05f1ebf3835c6c70d813f87655f101e539d`
- `editor/test/code/syntax-core.test.ts`: `3ab4d61db67de712c010394e3147b72452020e849d3c1f520ac1a0572d16c6fd`
**Verification**:
- `cd editor && npx tsc --noEmit` — exit 0; complete log: `tmp/cmp/EDS-10/attempt-1/tsc-rerun.log`.
- `cd editor && npx vitest run test/code/syntax-core.test.ts test/code/syntax.test.ts test/code/syntax-fallback.test.ts` — exit 0; 3 files and 10 tests passed; complete log: `tmp/cmp/EDS-10/attempt-1/vitest.log`.
- Initial typecheck wrapper attempt exited 1 because zsh rejected an assignment to its read-only `status` variable after the check ran; preserved in `tmp/cmp/EDS-10/attempt-1/tsc.log`. The rerun above is complete and passed.
**Blockers**: None.

## Related Plans

- **Next**: CMP-40
