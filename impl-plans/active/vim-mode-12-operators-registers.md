# Vim Mode: Operators and Registers Implementation Plan

**Status**: Ready
**Plan ID**: VIM-12 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 4.5 (operator adjustment), 4.6, 4.8 (store only)
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00
- **Parallel (wave 1)**: VIM-10, VIM-11, VIM-13, VIM-14, VIM-15, VIM-16, VIM-17, VIM-18
- **Consumer**: VIM-20

## Intent and Context

This plan adds pure edit builders, `editor/src/code/vim/operators.ts`,
and the in-memory register store, `editor/src/code/vim/registers.ts`.
Exact exports are pinned in `vim-mode-00-contract.md` ("Module signatures
pinned for wave 1"). The functions return change specs and cursor
offsets; they never dispatch. The controller (VIM-20) adds the
`isolateHistory.of('full')` and `userEvent: 'vim.<cmd>'` annotations when
it dispatches. The common execution rules in `vim-mode-00-contract.md`
apply.

The controller composes the shortcut commands from these primitives, so
they need no builders here:

| Command | Composed as |
|---|---|
| `x` | `dl` |
| `X` | `dh` |
| `s` | `cl` |
| `S` | `cc` |
| `D` | `d$` |
| `C` | `c$` |
| `Y` | `yy` |

## Non-Goals

- No async clipboard I/O. `RegisterStore` keeps only the `+`/`*` mirror,
  and VIM-20 calls the `ClipboardAdapter`.
- No numbered registers `1-9` or `-` (VQ4).
- No dispatch, no annotations and no history handling.

## File-Level Changes

### `editor/src/code/vim/operators.ts` (new, at most 400 lines)

#### `motionRange(doc, from, result, op)`

This turns a cursor position plus a `MotionResult` into a `TextRange`:

- `a = min(from, result.pos)`, `b = max(...)`.
- If `result.linewise`, the range runs from `line(a).from` to `line(b).to`,
  with `linewise: true`.
- Otherwise, if `result.inclusive`, `to = nextGrapheme(doc, b)`, but never
  past `line(b).to`.
- Otherwise, the range is exclusive `[a, b)`. With `op !== null`, if
  `b > a`, `b` is at column 0, and `b`'s line is after `a`'s line, move
  `b` to the end of the previous line (design 4.5).

#### `applyOperator(state, op, range)`

| Op | Changes | Cursor (resulting document) | Register | `enterInsert` |
|---|---|---|---|---|
| `d`, charwise | delete `[from, to)` | `from` | `{ text: slice, linewise: false }` | false |
| `d`, linewise | delete the lines plus one line break. Use the trailing break when one exists; otherwise the leading break. A whole-document delete leaves `''` | `firstNonBlank` of the line now at that position | `{ text: lines joined, without the final break, linewise: true }` | false |
| `c`, charwise | delete `[from, to)` | `from` | as for `d` | true |
| `c`, linewise | replace `[line(from).from, line(to).to)` with the first line's leading whitespace | after that indent | as for `d` | true |
| `y` | none (`changes: null`) | charwise: `from`; linewise: `-1` (the controller keeps the cursor if it is on the first range line, otherwise it uses the first line at the current column) | the yanked text | false |
| `>` | insert `INDENT_UNIT` at each covered non-blank line start. A charwise range is first expanded to its lines | `firstNonBlank` of the first line | null | false |
| `<` | remove up to `INDENT_UNIT.length` leading spaces, or one leading tab, per line | `firstNonBlank` of the first line | null | false |
| `g~`, `gu`, `gU` | replace the range with case-mapped text, per grapheme, using `toUpperCase`/`toLowerCase`. The length may change (`ß` -> `SS`) | `from` | null | false |

#### The other builders

- `putRegister(state, cursor, reg, before, count)`:
  - **Charwise.** Insert `reg.text.repeat(count)` at `cursor` (`P`) or
    after the cursor grapheme (`p`; at an empty line, at the cursor). The
    resulting cursor is on the last inserted grapheme for single-line text,
    or at the insert start for multi-line text.
  - **Linewise.** Insert the lines below the current line (`p`: at
    `line.to`, prefixed with `'\n'`) or above it (`P`: at `line.from`,
    suffixed with `'\n'`), repeated count times. The cursor is the first
    non-blank of the first inserted line.
- `joinLines(state, cursor, count)`: joins `max(2, count)` lines starting
  at the cursor line (count-1 joins, at least 1). Each join removes the
  break and the next line's leading blanks, then inserts one space. The
  space is skipped if the next line becomes empty or the current line
  already ends with a blank. The cursor is at the last join point. On the
  last line, return null.
- `replaceChars(state, cursor, ch, count)`: needs `count` graphemes from
  the cursor on the same line; otherwise null. Each grapheme becomes `ch`.
  `ch === '\n'` replaces all `count` graphemes with one break. The cursor
  is on the last replaced grapheme (after a break: at the start of the new
  line).
- `toggleCaseChars(state, cursor, count)`: toggles up to `count`
  graphemes, bounded by the line end. The cursor goes just after them (the
  controller clamps it). On an empty line, return null.
- `openLine(state, cursor, above)`: the new line copies the current line's
  leading whitespace. Below: insert `'\n' + indent` at `line.to`. Above:
  insert `indent + '\n'` at `line.from`. The cursor is after the indent on
  the new line.

### `editor/src/code/vim/registers.ts` (new, at most 160 lines)

- `isRegisterName(ch)`: `/^[a-zA-Z0-9_+*"]$/`.
- `RegisterStore`:
  - `read(name)`:
    - `'"'` is the unnamed register;
    - `a-z` are the named registers, and `A-Z` read the lowercase one;
    - `'0'` is the yank register;
    - `'+'`/`'*'` return the mirror (one shared value);
    - `'_'` and unset registers return null.
  - `write(name, value, kind)`:
    - `name === null`: set unnamed, and also `0` when `kind === 'yank'`.
    - `'_'`: do nothing at all; unnamed is unchanged.
    - `a-z`: set that register and unnamed.
    - `A-Z`: append to the lowercase register, then set unnamed to the
      result. If either part is linewise, the result is linewise and the
      parts are joined with `'\n'`; otherwise the text is concatenated. A
      missing target means a plain set.
    - `+`/`*`: set the mirror and unnamed.
    - `value.tokens` is stored as given (macros).
  - `isClipboard(name)`: true for `+` and `*`.
  - `setMirror(text, linewise)`: updates the mirror only.

### Tests

`editor/test/code/vim/operators.test.ts` (at least 40 cases). It uses
`EditorState.create({ doc })` and applies `state.update({ changes, selection: { anchor: cursor } })`
to check the resulting doc and cursor:

- `motionRange`: inclusive `e`; exclusive `w` that ends at column 0 of the
  next line, which adjusts to the previous line end; linewise `j`.
- `d` charwise/linewise: the middle line, the last line (it uses the
  leading break), the whole doc -> `''`. `c` linewise keeps the indent
  (`'  foo'` -> `'  '`, with the cursor at 2).
- `y` -> no changes; linewise -> cursor `-1`.
- `>`/`<` on mixed lines (blank lines untouched; a tab removed by `<`).
- `gU` on `'straße'` -> `'STRASSE'`; `g~` on `'aB'` -> `'Ab'`.
- `putRegister`: charwise `p`/`P` with count 3, multi-line charwise, and
  linewise `p` on the last line and `P` on the first line.
- `joinLines` with leading blanks, an empty next line and a count of 3.
- `replaceChars` with too few graphemes -> null, and `r<CR>`.
- `openLine` above and below, with indent.

`editor/test/code/vim/registers.test.ts` (at least 15 cases): unnamed and
`0` on yank versus delete; `"a` then `"A` append (charwise + charwise and
charwise + linewise); `_` leaves unnamed unchanged; `+` sets the mirror and
unnamed; `isRegisterName` accepts and rejects; tokens are kept.

## Pitfalls

- `cursor` is in **resulting**-document coordinates (it goes straight into
  `selection`). Compute it from the change, not from the old document.
- A linewise delete must not leave a stray empty line or eat two breaks.
- Never produce a selection past the resulting `doc.length`.

## Acceptance Criteria

- [ ] Every export pinned in VIM-00 for these two modules exists with that
      signature.
- [ ] At least 55 tests pass across both files.
- [ ] Each file is under its line budget, and the purity test passes.

## Verification

1. A scoped typecheck per common rule 5 (the 4 files):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 55 tests and 0 failed.

## Progress Log

### Session: (not started)
