# Vim Mode: Text Objects Implementation Plan

**Status**: Ready
**Plan ID**: VIM-11 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 4.4, 4.7
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00
- **Parallel (wave 1)**: VIM-10, VIM-12, VIM-13, VIM-14, VIM-15, VIM-16, VIM-17, VIM-18
- **Consumer**: VIM-20

## Intent and Context

This plan implements
`selectTextObject(doc: Text, cursor: number, spec: TextObjectSpec, count: number, visual: { anchor: number; head: number } | null): TextRange | null`
in `editor/src/code/vim/textobjects.ts`. `TextObjectSpec.kind` is already
normalized by the parser (VIM-18): `b`/`)` arrive as `(`, `B`/`}` as `{`,
`]` as `[` and `>` as `<`. Use `classAt`, `nextGrapheme`/`prevGrapheme`
from `vim/words.ts`, and `SCAN_LIMIT` from `vim/types.ts`. The common
execution rules in `vim-mode-00-contract.md` apply.

## Non-Goals

- No `it`/`at` tag objects, no sentence/paragraph objects, and no
  string- or comment-aware bracket matching (VQ4).
- No dispatch and no state.

## File-Level Changes

### `editor/src/code/vim/textobjects.ts` (new, at most 300 lines)

- **Word objects (`w`, `W`).** `iw` selects the run of one class (keyword,
  punctuation or blank) around the cursor. `aw` adds trailing blanks; if
  there are none, it adds leading blanks instead. A count n extends over n
  runs, where a blank run counts for `iw` as in Vim. Word objects stay on
  the cursor line. The result is charwise.
- **Quote objects (`"`, `'`, `` ` ``).** These work on the cursor line
  only, and a quote preceded by a backslash does not count. The pair
  chosen is the one containing the cursor (counting quotes from the line
  start); if the cursor is not inside a pair, it is the first pair after
  the cursor. `i` is the inside; `a` includes both quotes plus trailing
  blanks (or leading blanks if there are none). With no pair, the result is
  null.
- **Bracket objects (`(`, `[`, `{`, `<`).** Find the innermost enclosing
  pair with nesting, scanning backward and forward at most `SCAN_LIMIT`
  characters each way. A count n selects the n-th enclosing pair. If the
  cursor is on an open bracket, that pair is the innermost.
  - `a` covers both brackets.
  - `i` covers the inside. If the inside starts with a line break right
    after the opening bracket, exclude it. If the closing bracket is
    preceded only by indentation on its line, exclude that line's
    indentation and the line break before it. A multi-line `i` is still
    charwise.
  - An empty inside (`()`) gives an empty range at the position between
    the brackets.
- **Visual extension.** If `visual` is given with anchor !== head and the
  current selection already equals the object, return the next enclosing
  pair (Vim's repeated `a(`). Only the bracket objects need this; word and
  quote objects ignore `visual`.

### `editor/test/code/vim/textobjects.test.ts` (new, at least 30 cases)

Format: `{ doc: 'foo b|ar baz', spec: { around: false, kind: 'w' }, count: 1, expect: [4, 7] }`.

- `iw` on keyword, punctuation and blank runs; `aw` with trailing blanks;
  `aw` at the line end takes leading blanks; `2iw`; `iW` over `a.b-c`;
  `iw` on `sound-kit`.
- `i"`/`a"` inside and before a pair; an escaped `\"` is skipped; no pair
  -> null; `i'` and `` i` ``.
- `i(` and `a(`, nested `2i(`, the cursor on `(`, unbalanced -> null;
  `i{` on a multi-line block (`'f {\n  x\n}'` -> exactly `'  x'`);
  `i[`, `i<`, empty `()`.
- Bound: an enclosing pair farther than `SCAN_LIMIT` -> null.
- Visual repeat: the selection equals `i(` of the inner pair, and
  `a(` -> the outer pair.

## Pitfalls

- Ranges are half-open `[from, to)`. Do not return inclusive ends.
- Use grapheme stepping. Brackets and quotes are single-unit ASCII, but
  the content around them may not be.
- Do not let quote objects cross lines.

## Acceptance Criteria

- [ ] All 9 `TextObjectKind` values are handled for both `i` and `a`.
- [ ] At least 30 test cases pass.
- [ ] The file is under 300 lines, and the purity test passes.

## Verification

1. A scoped typecheck per common rule 5 (`src/code/vim/textobjects.ts`,
   `test/code/vim/textobjects.test.ts`):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-11/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/textobjects.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 30 tests and 0 failed.

## Progress Log

### Session: (not started)
