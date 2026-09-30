# EDS-10: CodeMirror-free Tree-sitter Span Core

**Status**: Ready
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

- [ ] `syntax-core.ts` exports the moved symbols plus `StyleSpan` and
      `styleSpans`, and it has no `@codemirror` import.
- [ ] `syntax.ts` is a thin adapter that re-exports the moved symbols,
      and no other file changed.
- [ ] The new test passes with the mocks that throw, and the existing
      syntax tests pass.
- [ ] Logs are recorded, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

## Related Plans

- **Next**: CMP-40
