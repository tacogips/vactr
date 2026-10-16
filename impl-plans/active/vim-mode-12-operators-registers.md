# Vim Mode: Operators and Registers Implementation Plan

**Status**: Completed
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

- [x] Every export pinned in VIM-00 for these two modules exists with that
      signature.
- [x] At least 55 tests pass across both files.
- [x] Each file is under its line budget, and the purity test passes.
- [x] Uppercase register append preserves and concatenates macro tokens, deriving tokens from text-only sides when needed; four register-combination regressions and the qA macro replay regression pass.

## Verification

1. A scoped typecheck per common rule 5 (the 4 files):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 55 tests and 0 failed.

## Progress Log

### Session: 2026-10-16 22:00 JST
**Tasks Completed**: All VIM-12 implementation criteria.
**Changes**: Added `editor/src/code/vim/operators.ts` with all seven pinned pure edit builders and `editor/src/code/vim/registers.ts` with the pinned in-memory register store. Added `editor/test/code/vim/operators.test.ts` (44 cases) and `editor/test/code/vim/registers.test.ts` (31 cases); the purity test ran as the third file. Scoped verification scaffold: `tmp/vim-mode/VIM-12/tsconfig.json`.
**Verification**: `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` from `editor/` exited 0 (`tmp/vim-mode/VIM-12/typecheck-final-2.log`). `./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts` from `editor/` exited 0; 76/76 passed in 3 files (`tmp/vim-mode/VIM-12/focused-vitest-final-2.log`). Source line counts: operators.ts 163/400; registers.ts 54/160. All four changed source/test files are under 1000 lines (`tmp/vim-mode/VIM-12/line-count.log`).
**Repair Notes**: The first focused run exposed 13 failures (63 passed); repaired case-map operators, linewise change, grapheme-relative replacement offsets and aligned cursor/range expectations. Initial run retained at `tmp/vim-mode/VIM-12/focused-vitest-final.log`; final-source run is the `final-2` log.
**Handoff**: Implementation contract complete. VIM-20 consumes these primitives; formal integration and independent review are downstream workflow steps.


### Session: 2026-10-16 22:06 JST — adversarial repair
**Findings Repaired**: `VIM12-ADV-01` resolves linewise endpoints through `lastLine(doc, range)` at `lineAt(range.to)` (bounded to `doc.length`); charwise ranges retain the exclusive-end `to - 1` behavior. Deletion, indentation and linewise change use the helper. Added regressions for deletion/change through empty selected lines, empty middle/final lines, and a linewise motion ending on an empty line. `VIM12-ADV-02` preserves empty linewise parts in uppercase register append and verifies both append orders plus the unnamed register. Existing test rows were preserved.
**Verification**: `cd editor && ./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts` exited 0 with 84/84 passing (`tmp/vim-mode/VIM-12/focused-vitest-final-4.log`). `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` exited 0 (`tmp/vim-mode/VIM-12/typecheck-final-4.log`). Final line counts are 168/400, 54/160, 134, and 114 (`tmp/vim-mode/VIM-12/line-count-final.log`).
**Final Source Hashes**: `tmp/vim-mode/VIM-12/review-repair-hashes.txt` records operators.ts `adc028f261b36db86abcad7626ced139cc85ad29be445acc1ede73896e668331`, registers.ts `04fbb7a0b9e369790518fc0c46f100d95129baa57d984c501d5c45a8876f6d7d`, operators.test.ts `26588d2f66cc8016e4b20fd893b8e089ae1947b98ff0d55cb697bb7bdeac99ff`, and registers.test.ts `9519f90049c61cf9c345b28fc638ac5fcc2acdf7e2d3feb5a4d8d79aa791da7e`.
**Completion Evidence**: Both new findings’ required regressions pass; existing rows remain in place; `range.to - 1` occurs only in `lastLine`’s charwise branch. Independent test-integrity and adversarial re-review are pending downstream.

### Session: 2026-10-17 Step 6 rerun
**Tasks Completed**: Revalidated all VIM-12 implementation criteria on the current shared tree; no source or test edits were required.
**Current-Source Identity**: `tmp/vim-mode/VIM-12/source-hashes-step6.txt` matches the implementation and test hashes recorded by the prior repair session.
**Verification**: `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` exited 0 (`tmp/vim-mode/VIM-12/typecheck-step6-final.log`). `cd editor && ./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts` exited 0; 84/84 passed across 3 files (`tmp/vim-mode/VIM-12/focused-vitest-step6-final.log`). Current line counts: operators.ts 168/400, registers.ts 54/160, operators.test.ts 134, registers.test.ts 114, purity.test.ts 37 (`tmp/vim-mode/VIM-12/line-count-step6-final.log`).
**Handoff**: VIM-12 source, tests, plan progress and required behavioral verification are complete. Formal test-integrity and adversarial review remain downstream workflow steps.

### Session: 2026-10-17 adversarial repair — VIM12-ADV-03
**Finding Repaired**: `RegisterStore.write` now treats explicit `"` as the unnamed register, including register `0` on yank, and explicit `0` writes update both register `0` and unnamed. Added three regression cases covering explicit unnamed yank, explicit unnamed delete preserving the previous yank, and explicit `0` yank. Existing `_`, named-register append, clipboard mirror, and implicit-name cases remain unchanged; digits 1-9 remain out of scope.
**Verification**: `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` exited 0 (`tmp/vim-mode/VIM-12/typecheck-adv03-final.log`). `cd editor && ./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts` exited 0 with 87/87 passing across 3 files (`tmp/vim-mode/VIM-12/focused-vitest-adv03-final.log`). `cd editor && wc -l src/code/vim/operators.ts src/code/vim/registers.ts` reports 168 and 59 lines; both remain under budget (`tmp/vim-mode/VIM-12/line-count-adv03-final.log`).
**Final Source Hashes**: `tmp/vim-mode/VIM-12/source-hashes-adv03-final.txt` records operators.ts `adc028f261b36db86abcad7626ced139cc85ad29be445acc1ede73896e668331`, registers.ts `6b73c07268c5a73b9822aa73e328c842c2d6ffc2cec531fadbc38a27328a7916`, operators.test.ts `26588d2f66cc8016e4b20fd893b8e089ae1947b98ff0d55cb697bb7bdeac99ff`, and registers.test.ts `2a2e39841368c86a4d7eb5618751633d1a6a0595ed9b809cad7a29a803ac32cf`.
**Handoff**: The assigned repair and current-source behavioral verification are complete. Independent test-integrity and adversarial re-review remain pending.

### Session: 2026-10-17 Step 6 implementation rerun — VIM-12
**Tasks Completed**: Revalidated assigned VIM-12 contract and current-source evidence; no implementation or test edits were needed.
**Verification**: `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` exited 0 (`tmp/vim-mode/VIM-12/typecheck-step6-current.log`). `cd editor && ./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts` exited 0 with 87/87 passing across 3 files (`tmp/vim-mode/VIM-12/focused-vitest-step6-current.log`).
**Completion Evidence**: Exports remain pinned, operators/register tests pass above the 55-test threshold, purity passes, and line budgets remain satisfied. No high/mid VIM-12 feedback is pending. Formal review remains downstream.

### Session: 2026-10-17 adversarial repair — VIM12-ADV-04
**Finding Repaired**: `RegisterStore.write` now concatenates existing and appended macro tokens in order. When exactly one side has tokens, the text-only side is converted with `textToTokens`; text-only appends remain text-only. Added four register combination regressions and corrected `editor/test/code/vim/macro-marks.test.ts` so the qA replay case expects the result `''`.
**Scoped Typecheck**: Added `editor/src/code/vim/record.ts` to `tmp/vim-mode/VIM-12/tsconfig.json` because `registers.ts` now uses its pinned `textToTokens` export.
**Verification**: `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` exited 0 (`tmp/vim-mode/VIM-12/typecheck-adv04.log`). `cd editor && ./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts test/code/vim/macro-marks.test.ts test/code/vim/controller.test.ts` exited 0; 221/221 passed across 5 files (`tmp/vim-mode/VIM-12/focused-vitest-adv04.log`). `cd editor && wc -l src/code/vim/registers.ts` reports 63/160 (`tmp/vim-mode/VIM-12/line-count-adv04.log`).
**Final Source Hashes**: `tmp/vim-mode/VIM-12/source-hashes-adv04.txt` records the current operator, register, operator-test, register-test, and macro-marks-test hashes.
**Handoff**: The VIM12-ADV-04 fix and required current-source regressions are complete. Independent adversarial re-review remains pending. The separate Chromium normal-mode IME high finding remains owned by VIM-15 and VIM-40.

### Session: 2026-10-17 Step 6 redispatch — VIM-12
**Tasks Completed**: Revalidated the assigned operators/registers contract and ADV-04 macro-token append behavior on the current shared tree. No source or test edits were required.
**Current-Source Identity**: All five hashes in `tmp/vim-mode/VIM-12/source-hashes-adv04.txt` match current files, including the `record.ts`-backed register token append logic and qA macro regression.
**Verification**: `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` exited 0 (`tmp/vim-mode/VIM-12/attempt-redispatch-01/typecheck.log`). The assigned focused suite exited 0 with 91/91 tests across 3 files (`tmp/vim-mode/VIM-12/attempt-redispatch-01/focused-vitest.log`). ADV-04 integration coverage (operators, registers, purity, macro-marks and controller) exited 0 with 221/221 tests across 5 files (`tmp/vim-mode/VIM-12/attempt-redispatch-01/integration-vitest.log`). Source/test line counts and `git diff --check` exited 0 (`line-count.log`, `diff-check.log`).
**Handoff**: VIM-12 implementation and its required behavioral verification are complete. The Chromium IME finding remains assigned to VIM-15 and VIM-40; independent review and final integration checks remain downstream.

### Session: 2026-10-17 Step 6 implementation dispatch — VIM-12
**Tasks Completed**: Confirmed dependency VIM-00 is accepted by the runtime dispatch. Rechecked all pinned exports, operator/register scope, and current source identity; no source or test edits were needed.
**Current-Source Identity**: `tmp/vim-mode/VIM-12/attempt-redispatch-02/source-hashes.txt` records current hashes for operators.ts, registers.ts and their two assigned test files.
**Verification**: `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-12/tsconfig.json` exited 0 (`tmp/vim-mode/VIM-12/attempt-redispatch-02/typecheck.log`). `cd editor && ./node_modules/.bin/vitest run test/code/vim/operators.test.ts test/code/vim/registers.test.ts test/code/vim/purity.test.ts` exited 0 with 91/91 tests passing across 3 files (`tmp/vim-mode/VIM-12/attempt-redispatch-02/focused-vitest.log`). `wc -l editor/src/code/vim/operators.ts editor/src/code/vim/registers.ts editor/test/code/vim/operators.test.ts editor/test/code/vim/registers.test.ts editor/test/code/vim/purity.test.ts` reports 168/63/134/164/37 lines (`tmp/vim-mode/VIM-12/attempt-redispatch-02/line-count.log`). `git diff --check` exited 0 (`tmp/vim-mode/VIM-12/attempt-redispatch-02/diff-check.log`).
**Handoff**: Assigned implementation and current-source behavioral verification are complete. Independent review and combined-tree integration checks remain downstream.
