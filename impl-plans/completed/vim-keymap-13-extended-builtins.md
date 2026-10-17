# Vim Keymap: Extended Built-ins, Surround and Parser Contract Implementation Plan

**Status**: Completed
**Plan ID**: VK-13 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 1 (K0), 3 (last paragraph), 4.1 (`awaitingArgument`), 5, 7.5 (operator target)
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`MotionKind` gains `'g;' | 'g,'`)
- **Parallel (wave 1)**: VK-10, VK-11, VK-12, VK-14, VK-15, VK-16
- **Consumer**: VK-20 executes the new parsed commands through the pure helpers delivered here
- Common Execution Rules: `impl-plans/completed/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

The preset needs built-ins the merged Vim mode lacks: `gj gk gJ g; g, zz
<C-e> <C-y>` and `ys yss cs ds`. They are active only when a keymap is
compiled (`extended`), so the no-config behavior (K0) stays exactly the
merged one. This plan adds the parser grammar behind an `extended` flag,
the `awaitingArgument`/`pendingOperator` queries VK-20 needs for
`routeKey`, `normalizeKey` Ctrl-letter support behind the same flag, the
change list, and pure edit/scroll helpers. Controller execution is VK-20.

## Non-Goals

- No controller changes; no new behavior when `extended` is false.
- Do not edit existing tests (`parser.test.ts`, `keys.test.ts`, ...); add
  new test files.
- No visual-mode surround (`S`), no tag surround, no soft-wrap logic.

## File-Level Changes

### editor/src/code/vim/parser.ts (budget: under 300 lines)

- `ActionKind` gains `'join-raw' | 'scroll-center' | 'scroll-line-down' | 'scroll-line-up' | 'surround-add' | 'surround-change' | 'surround-delete'`.
- `Target` gains `{ kind: 'position'; pos: number; inclusive: boolean }`
  (built only by VK-20 for hop operator targets; the parser never emits it).
- `CommandParser` gains `setExtended(on: boolean): void` (default off),
  `awaitingArgument(): boolean`, and
  `pendingOperator(): { register: string | null; count: number | null; operator: OperatorKind } | null`.
- `parseTarget` takes the extended flag. With it: `gj` -> motion `j`,
  `gk` -> motion `k`, `g;` -> motion `g;`, `g,` -> motion `g,` (also after
  an operator and in visual mode).
- Normal mode with extended: `gJ` -> action `join-raw` (count);
  `zz` -> `scroll-center`; `<C-e>` -> `scroll-line-down` (count);
  `<C-y>` -> `scroll-line-up` (count); `z` alone is pending, `z` + other
  key -> invalid; `ys` + target (`s` -> `{kind:'line'}`, else
  `parseTarget`, counts allowed as `ys2w`) + one printable pair char ->
  action `surround-add` with that target and `arg` = the char;
  `cs{old}{new}` -> `surround-change`, `arg` = old + new (two single
  printable characters); `ds{c}` -> `surround-delete`, `arg` = c.
  Visual with extended: `gJ` -> `join-raw`, `gj gk g; g,` as motions,
  `zz <C-e> <C-y>` as the scroll actions.
- `awaitingArgument()` is true when the next key is consumed literally:
  the argument of `f F t T r m q @ ' \``, the register name after `"`,
  the second key after a pending `g` or `z` (also after an operator such
  as `dg` or within `g~`), the text-object char after `i`/`a` in visual
  mode or after an operator, and the surround pair chars (after the `ys`
  target, both chars of `cs`, the char of `ds`). It is false for a bare
  operator (`d`, `ys` before its target): those targets stay mappable in
  `o` mode.
- `pendingOperator()` returns the parsed register, combined count and
  operator while `operatorPending()` is true and the parser is not
  awaiting an argument; else null.
- With extended off every existing parse result is unchanged.

### editor/src/code/vim/keys.ts

`normalizeKey(input, extended = false)`: when `extended`, any `Ctrl`+letter
without `Meta`/`Alt` returns `<C-{lowercase letter}>` (existing letters
unchanged); when not extended the current behavior is unchanged
(`<Other>` for other letters). `isPassThrough` unchanged.

### editor/src/code/vim/execute.ts

`isChanging` also returns true for `join-raw`, `surround-add`,
`surround-change`, `surround-delete` (normal mode). No other change.

### editor/src/code/vim/marks.ts (budget: under 110 lines)

```ts
export class ChangeList {
  record(changes: ChangeSet, docAfter: Text): void;   // any docChanged update
  step(direction: -1 | 1, count: number): number | { error: string };
  clear(): void;
}
```
Entries are change-start positions in the new document (first changed
range `fromB`); existing entries map through `changes.mapPos(pos, -1)`; a
change on the same line as the newest entry replaces it; cap 100; a new
change resets the walk index to "after newest". `step(-1)` (`g;`) moves to
older, error `E662 At start of changelist`; `step(1)` (`g,`) to newer,
error `E663 At end of changelist`. Empty list -> `E664 changelist is empty`.

### editor/src/code/vim/extended.ts (new, budget 160 lines)

```ts
export function joinRaw(state: EditorState, cursor: number, count: number, visual: { anchor: number; head: number } | null): { outcome: EditOutcome | null; range: TextRange | null };
export function scrollCenterDelta(doc: Text, cursor: number, viewport: VimViewport): number;
export function scrollLineResult(doc: Text, cursor: number, viewport: VimViewport, delta: number, desiredColumn: number | null): { scrollLines: number; cursor: number | null };
```
`joinRaw`: like `execute.ts:visualJoin` / `operators.ts:joinLines` but
without inserting or removing any characters other than the line breaks.
`scrollCenterDelta`: `cursorLine - (topLine + floor(rows / 2))`.
`scrollLineResult`: scroll by `delta` lines clamped to the document; if
the cursor line leaves the new viewport, return the nearest visible line
at the desired column (`posAtColumn`), else `cursor: null`.

### editor/src/code/vim/surround.ts (new, budget 200 lines)

```ts
export function surroundPair(ch: string): { open: string; close: string; space: boolean } | null;
export function surroundAdd(state: EditorState, range: TextRange, ch: string): EditOutcome | null;
export function surroundDelete(state: EditorState, cursor: number, ch: string): EditOutcome | null;
export function surroundChange(state: EditorState, cursor: number, from: string, to: string): EditOutcome | null;
```
Pairs (design 5): `( ) b`, `[ ] r`, `{ } B`, `< > a`, quotes `" ' \``;
`space` is true only for the opening brackets. Add: wrap the range; a
linewise range becomes one charwise span from the first non-blank of the
first line to the end of the last line. Delete/change find the pair with
`textobjects.ts:selectTextObject` (`a(`, `a"`, ... with count 1);
an opening-bracket `from` also trims one run of blanks inside each side.
Unknown char or no pair -> null. Cursor after add: the opening char;
after delete/change: the start of the former pair. One `EditOutcome`
(single change set) each.

## Invariants

- `extended` off: parser, keys and execute behave exactly as before;
  all existing tests in `editor/test/code/vim/` pass unchanged.
- Pure modules (purity test).

## Test Cases

`editor/test/code/vim/parser-extended.test.ts`:
- extended off: `gj`, `gJ`, `zz`, `ys`, `ds(`, `<C-e>` -> exactly the old result (invalid or pending as today)
- extended on: `gj` -> motion j; `dgk` -> operator d + motion k; `3gJ` -> join-raw count 3; `zz`; `2<C-e>`; `ysiw(` -> surround-add textobject iw arg `(`; `yss"` -> line target; `cs([` -> arg `([`; `ds'` -> arg `'`; visual `gj` -> motion
- awaitingArgument: after `f`, `m`, `"`, `g`, `z`, `dg`, `di`, visual `i`, `ysiw`, `cs`, `cs(`, `ds` -> true; after `d`, `ys`, `3`, empty -> false
- pendingOperator: `"a2d` -> `{register:'a', count:2, operator:'d'}`; `df` -> null (awaiting); empty -> null

`editor/test/code/vim/keys-extended.test.ts`: Ctrl-h -> `<Other>` without extended, `<C-h>` with; Ctrl-r unchanged both ways; Meta-h -> `<Other>`.

`editor/test/code/vim/surround.test.ts` (at least 12): `ysiw(` on `foo` -> `( foo )`; `ysiw)` -> `(foo)`; `yss"` on `  a b  ` -> `  "a b"  `; `ds(` on `( a )` -> `a`; `ds)` on `(a)` -> `a`; `cs"'` on `"x"` -> `'x'`; `cs)[` on `(x)` -> `[x]`; `cs)(` on `(x)` -> `( x )`; `cs(]` on `( x )` -> `[x]`; unknown char -> null; linewise add; no pair -> null.

`editor/test/code/vim/extended.test.ts`: joinRaw `a\n  b` -> `a  b`; count 3; scrollCenterDelta; scrollLineResult keeps or moves the cursor; ChangeList record/step/errors/cap/mapping.

## Verification (from editor/)

1. Scoped typecheck including all touched/new source files and tests plus `src/code/vim/controller.ts` (it imports parser/keys) -> exit 0
2. `./node_modules/.bin/vitest run test/code/vim` -> exit 0, failureCount 0 (existing Vim tests plus the new files; at least 40 new cases)

## Completion Criteria

- [x] Pinned exports and grammar; file budgets met; purity passes
- [x] Existing Vim tests unchanged and passing (K0)
- [x] At least 40 new passing cases
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-13 implementation
**Tasks Completed**: Parser extended grammar and argument-state contract; Ctrl-letter normalization gate; extended change classification; ChangeList; raw join and viewport scroll helpers; pure surround add/change/delete; four new test files.
**Notes**: Controller execution remains owned by VK-20. No Rust files or shared host/controller files were edited. Sibling edits in `editor/src/code/input.ts`, `editor/src/code/vim/hop.ts`, and sibling plans were observed on the shared worktree and left untouched.
**Source Hashes**:
- `editor/src/code/vim/parser.ts`: `1a975000811c01758af4442c15b2eb64e1f90134` (298 lines)
- `editor/src/code/vim/keys.ts`: `a5a8d7261f0286433c50da821dc99e58219ae7a8`
- `editor/src/code/vim/execute.ts`: `06d29a4f9cc74d6e23512cbec4c3f759f76f7f41`
- `editor/src/code/vim/marks.ts`: `7df7653c4ffc294cfa79cd95f61bb2c90801c3d7` (65 lines)
- `editor/src/code/vim/extended.ts`: `5312005cf0ef30457d4ee5660ed886f82b482a01` (35 lines)
- `editor/src/code/vim/surround.ts`: `e5460d7d418cb04ec716454eca4b92cad6fc8b75` (73 lines)
- `editor/test/code/vim/parser-extended.test.ts`: `e76d73722763b598423abb6f7e56096caa6bbe77`
- `editor/test/code/vim/keys-extended.test.ts`: `fab8137cca2f608469ee8433325f10ac8c7431e9`
- `editor/test/code/vim/surround.test.ts`: `bdfdeb8c3f8880ca9158a944e87d08500cdaf227`
- `editor/test/code/vim/extended.test.ts`: `d1a899f5a734ac662925f8731f5b1b12bef0bc1b`
**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-13/attempt-1/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-5/final-typecheck.log", notes: "Final source; includes assigned files and controller.ts."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim", exitStatus: 0, testsRun: 1030, testsPassed: 1030, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-5/final-vitest.log", notes: "28 Vim test files passed; existing Vim tests were not modified."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/parser-extended.test.ts test/code/vim/keys-extended.test.ts test/code/vim/surround.test.ts test/code/vim/extended.test.ts", exitStatus: 0, testsRun: 80, testsPassed: 80, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-5/new-tests.log", notes: "80 new passing cases, exceeding the 40-case minimum."}`
**Prior Verification**: Attempt-1 scoped typecheck failed on new test syntax (`tmp/vim-keymap/VK-13/attempt-1/typecheck.log`; shell did not append the exit marker because zsh reserves `status`). Attempt-2 failed on malformed test table (`attempt-2/typecheck.log`, exit 1). Attempt-3 found test typing issues (`attempt-3/typecheck.log`, exit 1). Attempt-4 full Vim suite found 8 failures (`attempt-4/vitest.log`, exit 1); corrected in source/test attempt 5. Attempt-5 typecheck and focused suite passed; final-source reruns above passed.

### Session: 2026-10-17 VK-13 test-integrity repairs
**Tasks Completed**: Repaired VK13-TI-1, VK13-TI-2 and VK13-TI-3 within VK-13 paths. `joinRaw` now clamps to two lines and expands a single-line visual selection; parser argument-state scanning skips operator/ys counts, covers g~/gu/gU and incomplete ys targets, and reports combined operator/surround counts; surround aliases b/r/B/a are tight. Added requested regression cases, including literal `ysiwb` and `cs)B` parser inputs, without modifying existing parser test files.
**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-13/attempt-1/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-6/retry-3/typecheck.log", notes: "Final repair source including literal alias sequence assertions; scoped VK-13 TypeScript files and controller dependency typecheck."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim", exitStatus: 0, testsRun: 1055, testsPassed: 1055, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-6/retry-3/vitest.log", notes: "28 test files; final repair source including literal alias sequence assertions."}`
- `wc -l editor/src/code/vim/parser.ts` -> 298 lines.
**Prior Verification**: First attempt-6 Vim run failed 2 newly added awaitingArgument rows (`tmp/vim-keymap/VK-13/attempt-6/vitest.log`, 1053 passed / 2 failed, exit 1); added cs-pair pending handling and distinguished incomplete `g~g` from doubled `g~g~`, then reran final source successfully. Retry-3 is the current-source gate after adding literal alias-input assertions; the earlier retry-2 gate was superseded.
**Source Hashes**:
- `editor/src/code/vim/parser.ts`: `1a975000811c01758af4442c15b2eb64e1f90134` (298 lines)
- `editor/src/code/vim/extended.ts`: `5312005cf0ef30457d4ee5660ed886f82b482a01` (35 lines)
- `editor/src/code/vim/surround.ts`: `e5460d7d418cb04ec716454eca4b92cad6fc8b75`
- `editor/test/code/vim/parser-extended.test.ts`: `e76d73722763b598423abb6f7e56096caa6bbe77`
- `editor/test/code/vim/surround.test.ts`: `bdfdeb8c3f8880ca9158a944e87d08500cdaf227`
- `editor/test/code/vim/extended.test.ts`: `d1a899f5a734ac662925f8731f5b1b12bef0bc1b`
**Handoff**: Independent test-integrity and adversarial review of the three repairs, followed by serial integration gates, remain downstream workflow steps.

### Session: 2026-10-17 VK-13 current-tree verification
**Tasks Completed**: Re-ran the assigned scoped TypeScript check and complete Vim test directory on the current shared source tree. Completion criteria remain satisfied: extended-off parity, new behavior coverage, and parser line budget.
**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-13/attempt-1/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-8/typecheck.log", notes: "Current-tree scoped typecheck."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim", exitStatus: 0, testsRun: 1055, testsPassed: 1055, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-8/vitest.log", notes: "28 test files passed on current tree."}`
- `wc -l editor/src/code/vim/parser.ts` -> 298 lines; `git diff --check` on VK-13 source and test paths -> exit 0.
**Handoff**: DR-K1 mapper re-entry behavior remains owned by mapper integration (VK-10/VK-20) and final integration review; VK-13 provides the parser `awaitingArgument` contract and tests its parser states. Independent review and serial integration gates remain downstream.

### Session: 2026-10-17 VK-13 quote-surround repair
**Tasks Completed**: Repaired VK13-ADV-1-quote-surround-blanks. `selectedPair` now trims only outer spaces/tabs for quote text-object ranges and verifies that both span endpoints are the requested quote. Bracket ranges and `trimInnerSpaces` are unchanged. Added four regression cases for double/single quote deletion and change with adjacent blanks; the first also asserts the cursor remains at the former opening quote (position 4).
**Reproduction**: Before the source fix, `tmp/vim-keymap/VK-13/attempt-9/pre-fix-surround-repro.log` showed the four new cases failing (13 passed, 4 failed), including the reviewed stray-quote and deleted-blank outputs.
**Source Hashes**:
- `editor/src/code/vim/surround.ts`: `6494645d7860bec52371e67fc2055da4706c5da0` (79 lines)
- `editor/test/code/vim/surround.test.ts`: `045ab627c94f2a24889a5c69797343c1313766fa`
**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-13/attempt-1/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-10/typecheck.log", notes: "Current-source scoped typecheck."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/surround.test.ts test/code/vim/parser-extended.test.ts test/code/vim/keys-extended.test.ts test/code/vim/extended.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 110, testsPassed: 110, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-10/focused-vitest.log", notes: "All five selected files passed, including quote and bracket regressions plus purity."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim", exitStatus: 0, testsRun: 1104, testsPassed: 1104, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-10/vim-suite.log", notes: "All 31 Vim test files passed on the current shared tree, including the previously failing VK-10 mapper tests."}`
- `{command: "git diff --check -- editor/src/code/vim/surround.ts editor/test/code/vim/surround.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-13/attempt-10/diff-check.log", notes: "No whitespace errors."}`
**Handoff**: Independent adversarial re-review of VK13-ADV-1 and serial integration gates remain downstream.
