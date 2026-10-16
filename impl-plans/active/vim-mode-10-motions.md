# Vim Mode: Motions Implementation Plan

**Status**: Ready
**Plan ID**: VIM-10 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 4.3, 4.4, 4.5, 7
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00 (`vim/types.ts`, `vim/words.ts`)
- **Parallel (wave 1)**: VIM-11, VIM-12, VIM-13, VIM-14, VIM-15, VIM-16, VIM-17, VIM-18
- **Consumer**: VIM-20 controller

## Intent and Context

This plan implements the pure motion engine:
`runMotion(doc: Text, pos: number, motion: MotionSpec, ctx: MotionContext): MotionResult | null`.
All types come from `editor/src/code/vim/types.ts`. Grapheme stepping,
columns, classes and `clampNormal` come from `vim/words.ts`; reuse them
and do not reimplement them. The function does not dispatch anything, and
it has no state beyond its arguments. Read the "Common Execution Rules"
in `impl-plans/active/vim-mode-00-contract.md`; they apply here.

## Non-Goals

- Search motions (`/ ? n N * #`) are VIM-13. Mark motions are VIM-14.
  `:{n}` is VIM-13.
- No operator range adjustment: the exclusive-at-column-0 rule and
  inclusive expansion are VIM-12 `motionRange`.
- Do not handle `cw` -> `ce` here; that is a VIM-20 rule.
- No scrolling side effects. Return `scrollLines`, and the controller
  calls `host.scrollLines`.

## File-Level Changes

### `editor/src/code/vim/motions.ts` (new, at most 450 lines)

A `switch (motion.kind)` dispatches to small helpers. Repeat each motion
`ctx.count` times, but stop early when the position stops changing.
`count` is already clamped to 10,000 by the parser.

| Kind | Result |
|---|---|
| `h`, `BS` | charwise exclusive. Move by graphemes but never past the line start. `BS` may wrap to the previous line's last grapheme, as Vim does with `backspace` across lines |
| `l` | charwise exclusive. Stops at the last grapheme of the line (normal mode) |
| `0`, `^`, `$` | charwise. `0` and `^` are exclusive, `$` is inclusive. `$` with a count moves down `count - 1` lines and sets `desiredColumn = Infinity`. `^` uses `firstNonBlank` |
| `j`, `k`, `+` | linewise. Keep `ctx.desiredColumn`, or the current grapheme column if it is null. Clamp the line to `1..doc.lines`. The result pos uses `posAtColumn` (for `+` it is `firstNonBlank`). At the first or last line, `j`/`k` returns null (a failed motion) when no line moves at all |
| `w`, `b`, `e`, `ge` | charwise. `e` and `ge` are inclusive. Word classes come from `classAt`, and an empty line counts as a word for `w` and `b`. `w` at the last word of the doc goes to the doc end |
| `W`, `B`, `E` | the same as above, with blank-delimited classes |
| `gg`, `G` | linewise, `jump: true`. Without an explicit count, `gg` is line 1 and `G` is the last line; with one, both go to that line. The pos is `firstNonBlank` |
| `f`, `F`, `t`, `T` | charwise. `f` and `t` are inclusive; `F` and `T` are exclusive. Same line only; no match returns null. `lastFind` is set in the result |
| `;`, `,` | repeat `ctx.lastFind` in the same or the reversed direction. `;` after `t` skips an adjacent match, as Vim does. If there is no `lastFind`, return null |
| `%` | inclusive, `jump: true`. The first `()[]{}` at or after pos on the line, then its match with nesting. The scan is bounded to `SCAN_LIMIT` characters in each direction; past the bound, return null |
| `{`, `}` | charwise exclusive, `jump: true`. Move to the previous or next blank line (empty, or blanks only), or to the doc start or end |
| `H`, `M`, `L` | linewise, `jump: true`. These use `ctx.viewport`. `H` is `topLine + (count - 1)`, `L` is `topLine + rows - 1 - (count - 1)`, `M` is the middle. All are clamped to the doc and use `firstNonBlank` |
| `C-d`, `C-u` | linewise. `n = explicitCount ? count : floor(rows / 2)` (at least 1). The cursor moves by n lines with `desiredColumn` kept, and `scrollLines` is `+/-n` |
| `C-f`, `C-b` | linewise. `n = max(1, rows - 2) * count`. The cursor moves by n lines, and `scrollLines` is `+/-n` |

- `desiredColumn` in the result follows design 4.3:
  - `ctx.desiredColumn` passed through unchanged for
    `j k C-d C-u C-f C-b H M L gg G`. `gg G H M L` still place the cursor
    on the first non-blank, but keep the value. When the value is null,
    `j k C-d C-u C-f C-b` first compute it as the current grapheme column
    and return that;
  - `Infinity` for `$`;
  - `null` for every other motion, including `+` and `BS`. The controller
    recomputes it lazily.
- Every position returned is a grapheme boundary.

### `editor/test/code/vim/motions.test.ts` (new)

The tests are table-driven:
`{ doc: 'fo|o bar', motion: { kind: 'w' }, count: 1, expect: 'foo |bar', linewise: false, inclusive: false }`.
Use `createTestHost` / `show` from `test/code/vim/host.ts`, or parse
the `|` marker inline. Required rows (at least 60):

- `h`/`l` at the line edges -> no wrap; `BS` at a line start -> wraps to
  the previous line's last grapheme; `l` over `'é'` and `'😀'` ->
  one grapheme per step.
- `w` from `'|foo.bar baz'` -> `.` (punctuation class); `W` -> `baz`;
  `w` over `sound-kit x` -> `x`; `w` across an empty line -> stops on the
  empty line; `3w`; `e`, `ge`, `b` and `B` cases; `w` at the doc end ->
  `doc.length`.
- `0`, `^` on `'  x'`, `$` sets `desiredColumn = Infinity`, `2$` goes to
  the next line's end.
- `j`/`k`: keeps the desired column across a short line
  (`'abcdef|' / 'ab' / 'abcdef'`, then `jj` -> column 5 restored); at the
  last line -> null; `5j` clamps to the last line.
- `gg`, `G` and `3G`, with the result `jump: true`.
- `f`/`t`/`F`/`T` with `;` and `,`; `t` then `;` with an adjacent match
  skips it; no match -> null.
- `%` on `'(a [b] c)'` from inside `a` -> the matching `)`; an unbalanced
  bracket -> null; a scan longer than `SCAN_LIMIT` -> null (build the doc
  with `'('.padEnd(70_000, 'x')`).
- `{`/`}` across paragraphs.
- `H`/`M`/`L` with viewport `{ topLine: 5, rows: 10 }` on a 40-line doc.
- `C-d` with rows 10 -> 5 lines and `scrollLines: 5`; `C-f` -> 8 lines;
  `C-u` at the top -> line 1.

## Pitfalls

- Never step by UTF-16 unit; always use `nextGrapheme` and
  `prevGrapheme`.
- `Text.line(n)` is 1-based, and `doc.lineAt(pos).number` is too.
- Do not read beyond the current line for `f t`; return null instead.
- Bound every multi-line scan (`%`, `{ }`, `w` across many empty lines) to
  `SCAN_LIMIT` characters.

## Acceptance Criteria

- [ ] `runMotion` handles all 34 `MotionKind` values with the results in
      the table above.
- [ ] `motions.test.ts` has at least 60 cases, all passing.
- [ ] `motions.ts` is under 450 lines, and the purity test still passes.

## Verification

1. Write `tmp/vim-mode/VIM-10/tsconfig.json` per common rule 5, including
   `src/code/vim/motions.ts` and `test/code/vim/motions.test.ts`. Then run
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json`;
   it must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 60 tests and 0 failed.

## Progress Log

### Session: (not started)
