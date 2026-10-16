# Vim Mode: Search and Ex Parsing Implementation Plan

**Status**: Ready
**Plan ID**: VIM-13 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 4.12, 4.13; user-qa VQ2, VQ3, VQ6
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00
- **Parallel (wave 1)**: VIM-10, VIM-11, VIM-12, VIM-14, VIM-15, VIM-16, VIM-17, VIM-18
- **Consumer**: VIM-20, which executes the commands and owns the last pattern, the direction and the messages

## Intent and Context

This plan adds the pure search helpers (`editor/src/code/vim/search.ts`)
and the ex command-line parser (`editor/src/code/vim/ex.ts`). The exports
are pinned in `vim-mode-00-contract.md`. The common execution rules there
apply.

## Non-Goals

- No match highlighting (`hlsearch`, VQ3). No incremental search, no ex
  history, no `:g`, and no `c` flag (VQ4).
- No execution. `parseEx` returns data, and VIM-20 runs it.
- Do not translate Vim "magic" syntax. The dialect is JS `RegExp` (VQ6).

## File-Level Changes

### `editor/src/code/vim/search.ts` (new, at most 250 lines)

- **`compilePattern(source)`**:
  - A trailing `\c` is stripped and adds the `i` flag. The base flags are
    `'u'`; callers add `'g'` where they need it.
  - If `new RegExp` throws, escape the source and compile it as a literal,
    returning `literal: true` and `note: 'pattern compiled as literal text'`.
  - An empty source is the caller's job (reuse the last pattern); here it
    returns `{ regex: /(?:)/u, literal: false, note: null }`.
- **`findMatch(doc, from, regex, direction)`**:
  - Search the whole-document string. Cache the string in a module-level
    `WeakMap<Text, string>` keyed by `Text` identity.
  - Forward: the first match whose start is `> from`; if none, wrap to the
    first match from 0, with `wrapped: true`.
  - Backward: the last match whose start is `< from`; if none, wrap to the
    last match in the doc.
  - Zero-length matches are allowed. Advance `lastIndex` by 1 so the loop
    terminates. No match returns null.
  - Always clone the regex with the `g` flag added, and never mutate the
    caller's regex.
- **`wordUnderCursor(doc, pos)`**: the keyword run (class 1 via `classAt`)
  at the cursor, or the first one after it on the line. With none, null.
- **`wordPattern(word)`**: the escaped word inside
  `(?<![\p{L}\p{N}_\-])...(?![\p{L}\p{N}_\-])`, with flags `'u'`. A known
  limitation is that a word followed by a trailing punctuation `-` is not
  matched there; document it in a code comment.
- **`substitute(doc, fromLine, toLine, regex, replacement, global)`**:
  - Process each line in `fromLine..toLine` (1-based, inclusive). Without
    `global`, replace the first match per line; with it, replace every
    match.
  - Translate the replacement with a replacer function, never with
    `String.replace` `$` semantics: `&` and `\0` are the whole match,
    `\1`-`\9` are groups (empty if unmatched), `\&` is a literal `&`, `\\`
    is a backslash, and any other `\x` is `x`. A JS `$` stays literal.
  - Return start-document `ChangeSpec[]` (one per replaced match), the
    match `count`, and `lines` (how many lines changed). Matches never span
    lines.

### `editor/src/code/vim/ex.ts` (new, at most 250 lines)

Pin this union and export it:

```ts
export type ExCommand =
  | { kind: 'empty' }
  | { kind: 'goto'; line: number }
  | { kind: 'substitute'; fromLine: number; toLine: number; pattern: string; replacement: string; global: boolean; ignoreCase: boolean }
  | { kind: 'noh' }
  | { kind: 'noop-write'; name: string }
  | { kind: 'action'; action: 'evalAtCursor' | 'evalAll' | 'hush' | 'format' };
export interface ExContext { currentLine: number; lastLine: number; visual: { fromLine: number; toLine: number } | null }
```

- **Ranges.** The forms are: none (the current line), `%`, `N`, `N,M`,
  `.`, `$`, `.+N`, `.-N`, and `'<,'>` (which needs `ctx.visual`;
  otherwise the error is `E20 Mark not set`). A line number beyond
  `lastLine` is clamped to it for `goto`, but is
  `E16 Invalid range` for `s`. A reversed `N,M` is swapped. Leading
  spaces and a leading `:` are ignored.
- **Commands.**
  - A range alone gives `goto` with the last line of the range.
  - `s` / `substitute`: any non-alphanumeric delimiter except `\` and `"`.
    The fields are `pattern`, `replacement` and the flags `g`/`i`, and
    each part may omit its trailing delimiter. A `\{delim}` inside a part
    is a literal delimiter: unescape it while splitting, and keep every
    other escape for `substitute()`. An unknown flag gives
    `E488 Trailing characters`.
  - `noh` / `nohlsearch` (any prefix of at least `noh`) give `noh`.
  - `w`, `q`, `wq`, `x`, `q!`, `w!` and `wq!` give `noop-write` with the
    name.
  - `eval` -> `evalAtCursor`; `evalall` and `run` -> `evalAll`;
    `hush` -> `hush`; `format` -> `format`.
  - Empty text gives `empty`.
  - Anything else is `E492 Not an editor command: <text>`.

### Tests: `editor/test/code/vim/search-ex.test.ts` (new, at least 40 cases)

- `compilePattern`: a valid regex, `\c`, invalid `'(('` -> literal with a
  note, the `u` flag is present.
- `findMatch`: forward or backward with and without wrap (check the
  `wrapped` flag), a zero-length `^` pattern terminates, the caller's regex
  `lastIndex` is unchanged, and a cache hit happens for the same `Text`
  (spy-free: call twice and compare the results).
- `wordUnderCursor` on a blank -> the next word; `wordPattern('kit')` does
  not match inside `sound-kit` but matches `kit x`.
- `substitute`: the first match per line versus `g`; `&`, `\1` and `\&`;
  `$1` stays literal; multi-line ranges; count and lines; no match ->
  empty changes.
- `parseEx`: `12` -> goto 12; `99` on a 10-line doc -> goto 10; `%s/a/b/g`;
  `'<,'>s/x/y/` with and without a visual range; `s#a/b#c#`; `.,$s/a//`;
  `3,1s/a/b/` swapped; `s/a/b/z` -> E488; `noh`; `nohlsearch`; `w`; `q!`;
  `eval`; `run`; `hush`; `format`; `foo` -> E492; `''` -> empty;
  `20s/a/b/` on a 10-line doc -> E16.

## Pitfalls

- Never pass user replacement text as a JS replace string, because `$`
  patterns would be interpreted.
- RegExp state: global regexes keep `lastIndex`, so clone them per call.
- Lines are 1-based, matching `Text.line`.

## Acceptance Criteria

- [ ] The pinned exports and the `ExCommand`/`ExContext` types exist.
- [ ] At least 40 tests pass.
- [ ] Both files are under 250 lines, and the purity test passes.

## Verification

1. A scoped typecheck per common rule 5 (the 3 files):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-13/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/search-ex.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 40 tests and 0 failed.

## Progress Log

### Session: (not started)
