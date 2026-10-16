# Vim Mode: Text Objects Implementation Plan

**Status**: In Progress (implementation criteria met; independent review pending)
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

- [x] All 9 `TextObjectKind` values are handled for both `i` and `a`.
- [x] At least 30 test cases pass.
- [x] The file is under 300 lines, and the purity test passes.

## Verification

1. A scoped typecheck per common rule 5 (`src/code/vim/textobjects.ts`,
   `test/code/vim/textobjects.test.ts`):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-11/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/textobjects.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 30 tests and 0 failed.

## Progress Log

### Session: 2026-10-16 VIM-11 implementation

**Tasks Completed**: Added the DOM-free text-object selector and 35 focused behavior cases. All normalized kinds (`w`, `W`, quotes, and four bracket kinds) have inner and around coverage. Word runs use grapheme boundaries; quotes ignore escaped delimiters and stay on one line; nested bracket searches are bounded; multiline inner selections trim boundary line breaks and indentation; repeated visual bracket selection expands outward.

**Source Hashes**:
- `editor/src/code/vim/textobjects.ts`: `0dec6c731ccf14bbf917143518dcb0a0d317e668`
- `editor/test/code/vim/textobjects.test.ts`: `1cb95cc9f4af8a7923f9b99593685f929f123e90`
- This plan before progress update: `5136a8398646d6cb26194cba1d6529d416946c93`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-11/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/typecheck-final.log", notes: "Scoped typecheck of the implementation and tests; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/textobjects.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 36, testsPassed: 36, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/focused-vitest-03.log", notes: "2 files passed, including 35 text-object cases and the Vim-core purity test."}`
- `{command: "cd editor && wc -l src/code/vim/textobjects.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/line-count-final.log", notes: "126 lines, below the 300-line limit."}`

**Prior Attempts**: `typecheck-01.log` and `typecheck-02.log` exited 0. `focused-vitest-01.log` exited 1 (14 fixture/implementation failures); `focused-vitest-02.log` exited 1 (9 failures). The issues were corrected. `debug.log` contains a temporary matching trace used to isolate the range clamp; the diagnostic was removed before final tests.

**Drift**: Preserved concurrent VIM-00 and VIM-10 files and plan edits. No edits outside VIM-11 write paths.

**Blockers**: None. Independent integrity, adversarial and combined-tree review remain downstream workflow steps.

### Session: 2026-10-16 VIM11-TI-1 repair

**Finding Addressed**: `VIM11-TI-1` (mid). For `aw`/`aW` counts starting on a nonblank run, each additional count now skips one following blank run before taking the next nonblank class run. The `iw` count loop, start-on-blank `aw` behavior, and trailing/leading blank expansion remain unchanged. Added the two requested `2aw`/`2aW` cases. Compared with the pre-repair test snapshot at `tmp/riela-fanout/84534676-9233-44E0-89AE-A5B524DD28AF/BB0C238A-B1C7-49C8-AB4B-8F559BEE9016.json`; after removing only the two added rows, the test file matches the baseline exactly.

**Final Source Hashes**:
- `editor/src/code/vim/textobjects.ts`: `42f51b42095594efa607dac90b03b5a2002e835f`
- `editor/test/code/vim/textobjects.test.ts`: `5a53c81734e6073d103cdb229d3f87f304a40e68`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-11/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/typecheck-final-2.log", notes: "Scoped typecheck on repaired source; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/textobjects.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 38, testsPassed: 38, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/focused-vitest-04.log", notes: "2 files passed; all 35 prior text-object cases plus the two new count cases and purity test passed."}`
- `{command: "cd editor && wc -l src/code/vim/textobjects.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/line-count-final-2.log", notes: "138 lines, below 300."}`

**Review State**: The targeted correction and regression evidence are ready for independent re-review; no review acceptance is claimed here.

### Session: 2026-10-17 Step 6 verification rerun

**Tasks Completed**: Re-read the accepted VIM-11 contract and current sources. No implementation source or test edit was needed: the previous `aw`/`aW` count repair is present, the two regression cases remain, all 9 normalized kinds have inner and around coverage, and the purity test remains in scope. Refreshed the scoped verification on the current tree.

**Current Source Hashes**:
- `editor/src/code/vim/textobjects.ts`: `42f51b42095594efa607dac90b03b5a2002e835f`
- `editor/test/code/vim/textobjects.test.ts`: `5a53c81734e6073d103cdb229d3f87f304a40e68`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-11/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/typecheck-step6-rerun.log", notes: "Scoped typecheck on current source; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/textobjects.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 38, testsPassed: 38, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/focused-vitest-step6-rerun.log", notes: "2 files passed; 37 text-object cases and the purity test passed, including 2aw/2aW."}`
- `{command: "cd editor && wc -l src/code/vim/textobjects.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/line-count-step6-rerun.log", notes: "138 lines, below the 300-line limit."}`

**Drift**: The VIM-11 source and test hashes match the prior repair record. The current shared worktree also has concurrent plan progress modifications in VIM-00, VIM-12, VIM-13 and VIM-14; these files were preserved and not edited by VIM-11. No VIM-11 source drift or blockers.

**Review State**: Ready for downstream integrity and adversarial review; no review acceptance is claimed here.

### Session: 2026-10-17 Step 6 final verification

**Tasks Completed**: Confirmed the accepted VIM-11 implementation remains intact on the current shared tree. No source or test edits were needed. All 9 text-object kinds retain inner/around handling, the two `2aw`/`2aW` regressions remain, and the purity test is included.

**Current Source Hashes**:
- `editor/src/code/vim/textobjects.ts`: `42f51b42095594efa607dac90b03b5a2002e835f`
- `editor/test/code/vim/textobjects.test.ts`: `5a53c81734e6073d103cdb229d3f87f304a40e68`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-11/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/typecheck-step6-final-2026-10-17-rerun.log", notes: "Scoped typecheck of implementation and tests; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/textobjects.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 38, testsPassed: 38, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/focused-vitest-step6-final-2026-10-17.log", notes: "2 files passed; 37 text-object cases and the Vim-core purity test passed."}`
- `{command: "wc -l editor/src/code/vim/textobjects.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/line-count-step6-final-2026-10-17.log", notes: "138 lines, below the 300-line limit."}`

**Review State**: Ready for downstream integrity and adversarial review; no review acceptance is claimed here.

### Session: 2026-10-17 Step 6 implementation rerun

**Tasks Completed**: Rechecked the committed VIM-11 contract and accepted design reference, then verified the current implementation without source or test edits. All 9 normalized text-object kinds retain inner and around handling; counted `aw`/`aW` regressions and purity coverage remain present. The independent scoped review found no concrete correctness gap.

**Current Source Hashes** (Git blob IDs):
- `editor/src/code/vim/textobjects.ts`: `42f51b42095594efa607dac90b03b5a2002e835f`
- `editor/test/code/vim/textobjects.test.ts`: `5a53c81734e6073d103cdb229d3f87f304a40e68`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-11/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/attempt-step6-rerun-20261017-02/typecheck.log", notes: "Scoped check completed with no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/textobjects.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 38, testsPassed: 38, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/attempt-step6-rerun-20261017-02/focused-vitest.log", notes: "2 files passed, including 37 text-object cases and the purity test."}`
- `{command: "wc -l editor/src/code/vim/textobjects.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/attempt-step6-rerun-20261017-02/line-count-final.log", notes: "138 lines, below the 300-line budget."}`
- `{command: "git diff --check -- editor/src/code/vim/textobjects.ts editor/test/code/vim/textobjects.test.ts impl-plans/active/vim-mode-11-textobjects.md", exitStatus: 0, outcome: "passed", log: "tmp/vim-mode/VIM-11/attempt-step6-rerun-20261017-02/diff-check-final.log"}`

**Scope/Handoff**: Preserved concurrent changes outside VIM-11 ownership. The Chromium IME finding at `editor/src/code/input.ts:44` remains assigned to VIM-15 and VIM-40; this VIM-11 rerun does not assess or clear it. Independent formal review remains downstream.
