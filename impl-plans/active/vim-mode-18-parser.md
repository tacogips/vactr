# Vim Mode: Command Parser Implementation Plan

**Status**: Completed
**Plan ID**: VIM-18 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 2.1, 4.1, 4.2, 4.6, 4.7
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00
- **Parallel (wave 1)**: VIM-10..17
- **Consumer**: VIM-20, which executes `ParsedCommand`s

## Intent and Context

This plan turns a stream of normalized `VimKey`s (from `vim/keys.ts`)
into complete commands. It covers normal, visual, visual-line and the
operator-pending state. Insert and replace modes never call the parser;
the controller handles `<Esc>` there itself.
`editor/src/code/vim/parser.ts` exports `CommandParser` (signature pinned
in `vim-mode-00-contract.md`) and the types below. The common execution
rules in `vim-mode-00-contract.md` apply.

## Non-Goals

- No execution and no knowledge of the document.
- No visual block (`<C-v>` is never produced; VQ1).
- No `gv`, `gi`, `it`/`at`, or an operator followed by a `/`/`?` prompt
  (VQ4). These return `invalid` with a message.
- No macro stop logic. The controller intercepts `q` while recording,
  before calling the parser.

## Pinned Types (export from `parser.ts`)

```ts
export type ActionKind = 'insert' | 'append' | 'insert-line-start' | 'append-line-end' | 'open-below' | 'open-above'
  | 'replace-mode' | 'visual' | 'visual-line' | 'visual-swap' | 'put-after' | 'put-before' | 'replace-char'
  | 'toggle-case' | 'join' | 'undo' | 'redo' | 'repeat' | 'record' | 'play' | 'set-mark' | 'prompt' | 'escape';
export type Target =
  | { kind: 'motion'; motion: MotionSpec }
  | { kind: 'textobject'; spec: TextObjectSpec }
  | { kind: 'search'; which: 'n' | 'N' | '*' | '#' }
  | { kind: 'mark'; name: string; exact: boolean }
  | { kind: 'line' };
export interface ParsedCommand { register: string | null; count: number | null; operator: OperatorKind | null; target: Target | null; action: ActionKind | null; arg: string | null }
export type ParseResult = { kind: 'pending' } | { kind: 'invalid'; message: string | null } | { kind: 'command'; command: ParsedCommand };
```

`count` is the product of all typed counts, clamped to `COUNT_LIMIT`, or
`null` when no count was typed.

## Grammar (normal and operator-pending)

```
[count] ["{reg}] [count] (operator [count] (target | same-operator-again) | target | action)
```

- **Count.** `[1-9][0-9]*`. A `0` with no count digits typed is the `0`
  motion.
- **Register.** `"` followed by a key; `isRegisterName` from
  `vim/registers.ts` is not available in wave 1, so use the regex
  `/^[a-zA-Z0-9_+*"]$/` locally. An invalid name gives
  `invalid: 'E354 Invalid register name'`.
- **Operators.** `d c y > <`, plus `g~ gu gU` through the `g` prefix.
  Doubling (`dd`, `cc`, `yy`, `>>`, `<<`, `g~~` or `g~g~`, `guu` or
  `gugu`, `gUU` or `gUgU`) gives `target: { kind: 'line' }`.
- **Motion keys to `MotionSpec`.**
  - `h l j k 0 ^ $ w b e W B E G { } % H M L ; ,` map directly;
    `gg` -> `gg` and `ge` -> `ge`.
  - `f F t T` followed by one printable key (`<Esc>` cancels with
    `invalid: null`).
  - `<Left>` -> `h`, `<Right>` -> `l`, `<Up>` -> `k`, `<Down>` -> `j`,
    `<Home>` -> `0`, `<End>` -> `$`, `<BS>` -> `BS`, `<CR>` -> `+`,
    `<C-d>` -> `C-d`, and likewise for `<C-u>`, `<C-f>`, `<C-b>`.
- **Search targets.** `n N * #` give `{ kind: 'search' }`.
- **Mark targets.** `'` or `` ` `` followed by a key in `a-z`, `'` or
  `` ` `` gives `{ kind: 'mark', name, exact: key === '`' }`. The names
  `'` and `` ` `` mean the context mark.
- **Text objects** (only after an operator, or in visual mode): `i` or
  `a` followed by an object key. The key normalizes as `w` -> `w`,
  `W` -> `W`, `"` `'` `` ` `` as is, `(` `)` `b` -> `(`, `[` `]` -> `[`,
  `{` `}` `B` -> `{`, `<` `>` -> `<`. Any other key gives
  `invalid: null`.
- **Shortcuts.** These expand into operator and target:

  | Key | Expands to |
  |---|---|
  | `x` | `d` + motion `l` |
  | `<Del>` | `d` + motion `l` |
  | `X` | `d` + motion `h` |
  | `s` | `c` + motion `l` |
  | `S` | `c` + line |
  | `D` | `d` + motion `$` |
  | `C` | `c` + motion `$` |
  | `Y` | `y` + line |
- **Actions.**

  | Key | Result |
  |---|---|
  | `i` | `insert` |
  | `a` | `append` |
  | `I` | `insert-line-start` |
  | `A` | `append-line-end` |
  | `o` | `open-below` |
  | `O` | `open-above` |
  | `R` | `replace-mode` |
  | `v` | `visual` |
  | `V` | `visual-line` |
  | `p` | `put-after` |
  | `P` | `put-before` |
  | `r` + one key | `replace-char`, with `arg` the char (`<CR>` gives `'\n'`; `<Esc>` cancels) |
  | `~` | `toggle-case` |
  | `J` | `join` |
  | `u` | `undo` |
  | `<C-r>` | `redo` |
  | `.` | `repeat` |
  | `q` + `[a-zA-Z]` | `record`, `arg` = register |
  | `@` + `[a-z]` or `@` | `play`, `arg` = register or `'@'` |
  | `m` + `[a-z]` | `set-mark` |
  | `:` `/` `?` | `prompt`, `arg` = the key |
  | `<Esc>` | `escape` (always completes; resets pending state) |

  An action key after an operator is `invalid` (for example `dp`), except
  that `<Esc>` gives `escape`.
- **Errors.** `/` or `?` after an operator gives
  `invalid: 'search prompt cannot follow an operator'`. `<Tab>`,
  `<Other>` and unknown keys give `invalid: null`.

## Visual and visual-line modes (`mode` argument)

- Motions, search and mark targets give a command with `operator: null`;
  the controller moves the head.
- `i`/`a` + object gives a `textobject` target with `operator: null`.
- `d x <Del> c s y > < ~` and `g~ gu gU` complete **immediately** with
  `target: null` (the selection). `~` gives the operator `g~`.
- `D`, `X`, `C`, `S`, `R` and `Y` complete as the linewise forms
  `d`/`d`/`c`/`c`/`c`/`y`, with `target: { kind: 'line' }`.
- `r` + char gives `replace-char`; `J` gives `join`; `p`/`P` give
  `put-after`/`put-before`; `o` gives `visual-swap`; `v`/`V` give
  `visual`/`visual-line` (the controller toggles); `:` gives `prompt`.
- The register and count prefixes work as in normal mode.

## `pending()` and `operatorPending()`

- `pending()` returns the raw keys typed for the current incomplete
  command (named keys as is), and `''` after completion or reset.
- `operatorPending()` is true while an operator has been typed but no
  target yet.

## Tests: `editor/test/code/vim/parser.test.ts` (at least 50 cases)

The tests are table-driven: `{ mode: 'normal', keys: ['2','"','a','d','3','w'], expect: { register: 'a', count: 6, operator: 'd', target: motion w } }`.

- `dd` -> line; `3dd`; `d2w`; `"Ayy`; `"%` -> E354;
  `gUiw` -> textobject `w` inside; `g~~`; `gugu`.
- `0` motion versus `10j`; a count of 99999 clamped to 10000.
- `fx`, `dt)`, `f<Esc>` -> invalid null.
- `ci(`, `cib`, `da<`, `di}`, `yaW`, `diq` -> invalid.
- `x` / `D` / `Y` / `S` / `<Del>` expansions; `dp` -> invalid;
  `d<Esc>` -> escape.
- `r<CR>` -> `'\n'`; `qa`; `qA`; `q1` -> invalid; `@a`; `@@`; `ma`;
  `'a`; `` `a ``; `''`.
- `:`, `/` and `?` give prompts; `d/` -> invalid with the message.
- Visual: `iw` -> textobject; `d` -> immediate target null; `D` -> line;
  `~` -> `g~`; `o` -> visual-swap; `"ay` -> register `a`.
- `pending()` after `"`, `a` and `2` -> `'"a2'`; `operatorPending()` after
  `d`.
- Arrows map to motions, `<CR>` to `+`, `<BS>` to `BS`.

## Pitfalls

- Reset fully after `command` and `invalid`, so no stale register or
  count leaks into the next command.
- A count typed after an operator multiplies: `2d3w` is 6.
- In visual mode, `i` and `a` are text objects, not insert or append.

## Acceptance Criteria

- [x] The pinned types and `CommandParser` methods are exported.
- [x] At least 50 test cases pass.
- [x] `parser.ts` is under 350 lines, and the purity test passes.

## Verification

1. A scoped typecheck per common rule 5 (`src/code/vim/parser.ts`,
   `test/code/vim/parser.test.ts`):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-18/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/parser.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 50 tests and 0 failed.

## Progress Log

### Session: 2026-10-16 VIM-18 implementation

**Tasks Completed**: Implemented the pure command parser and table-driven tests for normal, operator-pending, visual and visual-line commands. Added count/register parsing, motion aliases, text objects, operator expansions, actions, marks, macros, prompts, pending state, cancellation and reset behavior.

**Write-path hashes before edits**:
- `editor/src/code/vim/parser.ts`: missing; final `760ba7bd648527fac2e1c95210d2734c92215352`
- `editor/test/code/vim/parser.test.ts`: missing; final `601afeb5a27e880028fef060fdd1862de3540b72`
- This plan before update: `7c993b7e8324fb8eab79f9f53db751f4ba83ded3`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-18/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/typecheck-final.log", notes: "Scoped parser and parser-test typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/parser.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 118, testsPassed: 118, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/focused-vitest-final.log", notes: "2 files passed; 118 tests passed, exceeding the 50-case threshold, and the purity test passed."}`
- `{command: "cd editor && wc -l src/code/vim/parser.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/line-count.log", notes: "193 lines, under the 350-line plan limit."}`

**Drift**: Existing sibling-plan changes were present in the shared tree. No files outside this plan's `writePaths` were edited; no existing shared source was overwritten.

**Blockers**: None. Formal review and combined-tree verification remain downstream workflow steps.

### Session: 2026-10-17 Step 6 rerun

**Tasks Completed**: Revalidated the assigned VIM-18 contract on the current shared tree. The parser and table-driven tests were already committed and matched the assigned scope, so no source/test edits were needed. The scoped typecheck, parser plus purity suite, and parser line-count gate passed. VIM-00 is accepted in the runtime-owned `acceptedPlanIds`; no assigned review repair was supplied in `reviewFeedback`.

**Source identity**:
- `editor/src/code/vim/parser.ts`: `git hash-object` `760ba7bd648527fac2e1c95210d2734c92215352`.
- `editor/test/code/vim/parser.test.ts`: `git hash-object` `601afeb5a27e880028fef060fdd1862de3540b72`.
- Existing shared-tree change `impl-plans/active/vim-mode-00-contract.md` was preserved; no VIM-18 source/test drift was found.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-18/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/typecheck-step6-final.log", notes: "Current-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/parser.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 118, testsPassed: 118, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/focused-vitest-step6-final.log", notes: "2 files passed; 118 tests passed, including the purity test."}`
- `{command: "cd editor && wc -l src/code/vim/parser.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/line-count-step6-final.log", notes: "193 lines, below the 350-line plan limit."}`

**Blockers**: None. Formal review and combined-tree verification remain downstream workflow steps.

### Session: 2026-10-17 Step 6 review repair

**Finding addressed**: `VIM-18-step7-finding-1-printable-lt` (mid). Updated `printable()` to accept any one-code-point `VimKey`, including literal `<`; named keys remain multi-character tokens and are rejected as `f`/`F`/`t`/`T` arguments. Added exact-result cases for `f<`, `F<`, `t<`, `T<`, `dt<`, `dt)`, `r<` in normal and visual modes, and invalid `f<Tab>` / `f<Other>`. Existing test cases were retained.

**Source identity after repair**:
- `editor/src/code/vim/parser.ts`: `git hash-object` `35455a302fed580ddd01cc8f55314e16f44d2be6`.
- `editor/test/code/vim/parser.test.ts`: `git hash-object` `bfeefa8f98de0ec6efbf63b4a79018ac2590822f`.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-18/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/typecheck-step6-repair-01.log", notes: "Scoped typecheck on repaired sources; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/parser.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 128, testsPassed: 128, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/focused-vitest-step6-repair-01.log", notes: "2 files passed; all 118 prior cases remain and 10 regression rows were added; purity passed."}`
- `{command: "cd editor && wc -l src/code/vim/parser.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/line-count-step6-repair-01.log", notes: "193 lines, below the 350-line plan limit."}`

**Blockers**: None. Independent adversarial re-review remains downstream.

### Session: 2026-10-17 Step 6 current-source verification

**Tasks Completed**: Confirmed VIM-00 admission through the runtime-owned `acceptedPlanIds`. Re-ran the assigned scoped typecheck, parser and purity tests, and parser line-count gate against the current shared source. No VIM-18 source changes were needed. The latest current-source verification passes 129 cases (including the purity test); `parser.ts` is 193 lines.

**Source identity**:
- `editor/src/code/vim/parser.ts`: `git hash-object` `0b4e15d3da15c458cd0897e797223f2d10606a7a`.
- `editor/test/code/vim/parser.test.ts`: `git hash-object` `e5f2a41a04c25baf66544dc8dc42d99b1a27af7c`.
- The current identities differ from the earlier repair session's logged hashes; verification below was freshly run on the current files. The pre-existing VIM-00 worktree change and unrelated concurrent VIM-15/source and sibling-plan changes were preserved.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-18/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/typecheck-step6-current-02.log", notes: "Current-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/parser.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 129, testsPassed: 129, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/focused-vitest-step6-current-02.log", notes: "2 test files passed; parser suite and purity test passed."}`
- `{command: "wc -l editor/src/code/vim/parser.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/line-count-step6-current-01.log", notes: "193 lines, below the 350-line plan limit."}`

**Ownership handoff**: Prior Chromium normal-mode IME finding `opus-luna-design-and-implement-review-loop-session-381-integration-review-attempt-1-finding-1` is assigned to VIM-15 (`editor/src/code/input.ts`) and re-verified by VIM-40. It is outside VIM-18 `writePaths`; this plan does not claim it as repaired.

**Blockers**: None for VIM-18. Independent review and the VIM-15/VIM-40 IME repair remain downstream.

### Session: 2026-10-17 Step 6 current-source verification (dispatch)

**Tasks Completed**: Reconfirmed VIM-00 admission from the assigned runtime `acceptedPlanIds`. The parser and test deliverables required no edits in this dispatch. Re-ran the scoped typecheck, parser plus purity suite, and parser line-count gate on the current shared-tree source; all passed with 129 tests and `parser.ts` at 193 lines.

**Source identity**:
- `editor/src/code/vim/parser.ts`: SHA-256 `b485e422022100a52c2cc579bd0d62379826ccd7fb41c62c494af155ea53adda`.
- `editor/test/code/vim/parser.test.ts`: SHA-256 `8972e0204a6e22eb86e46ad35d91e9624812afe2af9b46926a24a190e3071bbd`.
- Preserved the pre-existing `impl-plans/active/vim-mode-00-contract.md` worktree change; no VIM-18 source or test changes were made.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-18/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/typecheck-step6-current-03.log", notes: "Current-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/parser.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 129, testsPassed: 129, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/focused-vitest-step6-current-03.log", notes: "2 files passed; parser and purity tests passed."}`
- `{command: "wc -l editor/src/code/vim/parser.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-18/line-count-step6-current-02.log", notes: "193 lines, below the 350-line plan limit."}`

**Blockers**: None for VIM-18. Independent review and combined-tree verification remain downstream.
