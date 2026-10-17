# Vim Mode: Motions Implementation Plan

**Status**: Completed (accepted in workflow session 391; archived 2026-10-17)
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
- `%` on `'(a [b] c)'` from `a` selects the first following bracket (`[`)
  and its match (`]`); from `(` it selects the matching `)`. An unbalanced
  bracket or a match beyond `SCAN_LIMIT` returns null.
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

- [x] `runMotion` handles all 34 `MotionKind` values with the results in
      the table above.
- [x] `motions.test.ts` has at least 61 cases, all passing.
- [x] `motions.ts` is under 450 lines, and the purity test still passes.

## Verification

1. Write `tmp/vim-mode/VIM-10/tsconfig.json` per common rule 5, including
   `src/code/vim/motions.ts` and `test/code/vim/motions.test.ts`. Then run
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json`;
   it must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 60 tests and 0 failed.

## Progress Log

### Session: 2026-10-16

**Tasks Completed**: Implemented the pure motion dispatcher for all 34 `MotionKind` values and added 61 focused cases covering grapheme movement, counts, word classes, viewport/page movement, find/till repetition, paragraphs and bounded bracket matching.

**Implementation Notes**: Reused `words.ts` grapheme and column helpers. Bracket matching scans the ASCII bracket characters within `SCAN_LIMIT`, including nested pairs. Its original enclosing-opener scan was superseded by the test-integrity repair below: `%` uses the first bracket at or after the cursor on the current line, as specified by design section 4.5.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/typecheck-final.log", notes: "Final scoped typecheck on current source."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 61, testsPassed: 61, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/focused-vitest-final.log", notes: "Both files passed; 61/61 cases."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/line-count.log", notes: "266 and 100 lines; motion implementation is below 450 lines."}`

**Prior Attempts**: Earlier scoped runs exposed fixture expectation errors and motion defects; those changes were corrected and the final current-source commands above pass. Complete attempt logs are preserved as `focused-vitest-01.log` through `focused-vitest-07.log`, `typecheck-01.log` and `typecheck-02.log`; `focused-vitest-05.log` records the interrupted run during long-line scan diagnosis.

**Review State**: Implementation criteria are met. Independent integrity, adversarial and combined-tree review remain downstream workflow steps.

### Session: 2026-10-17 Step 6 rerun

**Tasks Completed**: Rechecked the assigned VIM-10 implementation on the current shared branch. No motion source or test edits were needed; the committed code already satisfies the assigned contract, including grapheme-boundary stepping and all 34 motion kinds.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `35c791aa9c6c829929729b11454f37f1064e6c536a7ebfb7bdcb17f13dda09fb`; `editor/test/code/vim/motions.test.ts` SHA-256 `71e229734bb691f89adee913a2f772da3edac3cc6e523b3f86a621cebf5f5bb6`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/typecheck-step6-rerun.log", notes: "Current-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 61, testsPassed: 61, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/focused-vitest-step6-rerun.log", notes: "Both test files passed, 61/61."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/line-count-step6-rerun.log", notes: "266 and 100 lines respectively; motions.ts remains below the 450-line limit."}`

**Review State**: VIM-10 implementation and assigned verification are complete. Independent integrity and adversarial review and shared-branch integration remain downstream workflow steps.

### Session: 2026-10-17 test-integrity repairs

**Tasks Completed**: Repaired both VIM-10 findings from `comm-006684`. `stepClass` now treats a non-empty line's end as blank for small-word movement, while `e` and `ge` skip empty lines and `w`/`b` retain their empty-line behavior. Rewrote `ge` to stop on the final grapheme of the previous word class. Added cross-line coverage for `w`, `e`, `b`, `ge`, punctuation, and counted `ge`; corrected only the flagged `ge` expectation. `words.ts` remains unchanged from checkpoint `6349b0f`.

**Expectation Note**: The review requested `e` from index 2 of `foo\n\nbar` to return index 6. Index 6 is `a`, not the end of the next word. Design section 4.4 says empty lines count as words for `w`/`b` only; section 4.5 defines `e` as inclusive. The regression asserts index 7, the `r` at the end of `bar`.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `56e66075e4be5e973f8ed85a097c2c2a926af1e0553c404a7fc1485260ea0c3a`; `editor/test/code/vim/motions.test.ts` SHA-256 `c6eca1fb539e9cb8c3eb504c6617ea466dca853ef1122cc094e38795deeb329d`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/typecheck-ti-repair-01.log", notes: "Scoped typecheck on repaired source; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 71, testsPassed: 71, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/focused-vitest-ti-repair-01.log", notes: "Both files passed, 71/71, including added cross-line word-motion cases."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/line-count-ti-repair-01.log", notes: "272 and 105 lines; motions.ts remains below 450 lines."}`

**Review State**: Code and assigned gates are complete. The empty-line `e` example's requested index 6 conflicts with the accepted inclusive-end semantics; implementation follows design and tests index 7. Independent re-review is pending.

### Session: 2026-10-17 adversarial repairs

**Tasks Completed**: Repaired `VIM10-ADV-1` and `VIM10-ADV-2` from `comm-006703`. `%` now selects only the first bracket at or after the cursor on the current line. Matching uses one depth-counted scan, forward for openers and backward for closers, bounded by `SCAN_LIMIT`. Removed the enclosing-opener scan and aligned the test bullet above with design section 4.5. The only prior expectation changed is `(a [b] c)` at index 1, from 8 to 5; added the requested opener/closer, nested, multi-line-context, mismatched and no-match regressions. The earlier implementation note describing an enclosing-opener scan is superseded.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `f35f29774ec45d49944fca3c3f1807f6a490cddcc919fd3b40fc5860878ba7aa`; `editor/test/code/vim/motions.test.ts` SHA-256 `8ffa170fab0ff2158ed5eb6d6e1084be2243714666966e0af54101dd910d58f0`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/typecheck-adv-repair-02.log", notes: "Scoped typecheck on final adversarial-repair source; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 83, testsPassed: 83, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/focused-vitest-adv-repair-02.log", notes: "Both files passed; the 12 % probes are independent Vitest cases, and the unmatched/SCAN_LIMIT assertions pass."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/line-count-adv-repair-02.log", notes: "249 and 111 lines; motions.ts remains below 450 lines."}`
- `{command: "git diff --check -- editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts impl-plans/active/vim-mode-10-motions.md", exitStatus: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/diff-check-adv-repair-02.log", notes: "No whitespace errors."}`
- `{command: "git diff --quiet 6349b0ff833c99756874b2a3c9decc44773bdd09 -- editor/src/code/vim/words.ts editor/src/code/vim/types.ts", exitStatus: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/vim00-identity-adv-repair-01.log", notes: "VIM-00 words.ts and types.ts are unchanged from checkpoint."}`

**Review State**: Repairs are implemented; independent test-integrity and adversarial re-review remain pending.

### Session: 2026-10-17 Step 6 current-source verification

**Tasks Completed**: Rechecked VIM-10 on the current shared branch. No motion source or test edits were needed; the existing implementation and 83 focused cases satisfy the assigned contract.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/typecheck-step6-current-rerun.log", notes: "Current-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 83, testsPassed: 83, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/focused-vitest-step6-current-rerun.log", notes: "Both files passed, 83/83."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/line-count-step6-current.log", notes: "249 and 111 lines respectively; motions.ts remains below the 450-line limit."}`

**Review State**: Assigned implementation and verification are complete. Independent integrity/adversarial re-review and combined-tree integration remain downstream workflow steps.

### Session: 2026-10-17 adversarial repair VIM10-ADV-3

**Tasks Completed**: Replaced the `e`/`E` `alreadyAtEnd` heuristic in `wordMove` with the specified grapheme-forward algorithm: step once, skip blank classes, then stop on the last grapheme of the next class run. Added seven regression assertions for single-grapheme words, punctuation, empty lines, counts, and grapheme offsets. Existing expectations and other motion branches are unchanged.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `b77d062068426ca21c1e0bfdf6784c708e1e1d0047894280c39a271a91a66289`; `editor/test/code/vim/motions.test.ts` SHA-256 `13af5d2d0720b93a1f0257f1e163766b739f8ae0171a6f3ac35e3f73094d2ced`.

**VIM-20 Handoff**: `editor/src/code/vim/controller.ts:246-251` implements count-1 `cw` using `e`. With `e` repaired, `cw` on a one-grapheme word such as `a bc` needs VIM-20 to preserve Vim's single-character change behavior. VIM-20 should also handle the existing case where `cw` at the final grapheme of a multi-grapheme word (`ab cd` at `b`) changes too much. This implementation did not edit `controller.ts`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-01/typecheck-final.log", notes: "Scoped typecheck; log ends with exit=0."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 90, testsPassed: 90, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-01/focused-vitest-final.log", notes: "Both files passed; log ends with exit=0."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-01/line-count.log", notes: "248 and 127 lines; motions.ts is below 450 lines."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/controller.test.ts test/code/vim/undo-repeat.test.ts", exitStatus: 0, testsRun: 143, testsPassed: 143, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-01/cw-handoff-vitest.log", notes: "Informational VIM-20 coupling check; both files passed."}`
- `{command: "git diff --check -- editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts impl-plans/active/vim-mode-10-motions.md", exitStatus: 0, failureCount: 0, outcome: "passed", notes: "No whitespace errors."}`

**Prior Attempt**: `tmp/vim-mode/VIM-10/attempt-01/focused-vitest.log` records the initial new grapheme assertion using index 4 for `c` in `é bc`. Vitest observed index 3, the correct UTF-16 offset; only that new expected value was corrected, and the final run passes 90/90.

**Review State**: VIM10-ADV-3 has been repaired and is pending independent Opus re-review. The separate VIM-20 `cw` handoff remains owned by VIM-20.

### Session: 2026-10-17 Step 6 current-source runtime dispatch

**Tasks Completed**: Revalidated the assigned VIM-10 behavior against design sections 4.3–4.5 and the accepted plan. The existing implementation covers all 34 `MotionKind` variants, includes 127 test lines with 90 passing Vitest cases, and remains under the 450-line motion limit. No motion source or test changes were needed. A bounded independent read-only scope check found no concrete missing requirement or correctness defect.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `b77d062068426ca21c1e0bfdf6784c708e1e1d0047894280c39a271a91a66289`; `editor/test/code/vim/motions.test.ts` SHA-256 `13af5d2d0720b93a1f0257f1e163766b739f8ae0171a6f3ac35e3f73094d2ced`.

**Verification**:

- `{command: "./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/typecheck-step6-runtime.log", notes: "Scoped typecheck of motions.ts and motions.test.ts; no diagnostics."}`
- `{command: "./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 90, testsPassed: 90, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/focused-vitest-step6-runtime.log", notes: "Both focused files passed; 90/90 cases."}`
- `{command: "wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/line-count-step6-runtime.log", notes: "248 and 127 lines; motions.ts is below 450 lines."}`

**Shared-Tree Observation**: Other active plan files VIM-00, VIM-11, VIM-14 and VIM-18 appeared modified in `git status --short`; they are outside this plan's write paths and were not changed. VIM-10 source and test identity remained stable through verification.

**Review State**: Assigned implementation and behavioral verification are complete. Independent test-integrity/adversarial and combined-tree integration review remain downstream workflow steps.

**Final Self-Check**: `git diff --check -- impl-plans/active/vim-mode-10-motions.md editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts` exited 0; complete output is in `tmp/vim-mode/VIM-10/diff-check-step6-runtime.log`. The latest shared-tree status also showed edits under VIM-12, VIM-13 (`editor/src/code/vim/ex.ts`, `editor/test/code/vim/search-ex.test.ts`), VIM-15 and VIM-17; these remain outside VIM-10 write paths and were not touched.

### Session: 2026-10-17 Step 6 dispatch rerun

**Tasks Completed**: Revalidated the VIM-10 implementation on the current shared tree. No motion source or test changes were needed. The implementation still covers all 34 `MotionKind` values; 90 focused tests pass, and `motions.ts` remains under the 450-line limit. The assigned dependency VIM-00 is accepted. The supplied `IR-388-VIM30-EVIDENCE` finding is owned by downstream VIM-30 and does not alter VIM-10's contract or write paths.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `b77d062068426ca21c1e0bfdf6784c708e1e1d0047894280c39a271a91a66289`; `editor/test/code/vim/motions.test.ts` SHA-256 `13af5d2d0720b93a1f0257f1e163766b739f8ae0171a6f3ac35e3f73094d2ced`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/step6-dispatch-20261017-tsc-attempt2.log", notes: "Current-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 90, testsPassed: 90, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/step6-dispatch-20261017-vitest-attempt2.log", notes: "Both files passed; 90/90 tests."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/step6-dispatch-20261017-line-count.log", notes: "248 and 127 lines respectively; implementation is below the 450-line limit."}`

**Verification Harness Note**: The first logging attempt used a path relative to `editor/` and zsh's readonly `status` variable; both wrappers failed after invocation, and the initial Vitest output showed 90/90 passing. The exact verification commands were rerun with complete logs at the paths above and exited 0.

**Review State**: VIM-10 implementation and required behavioral verification are complete. Independent review and combined-tree integration remain downstream workflow steps.

### Session: 2026-10-17 Step 6 bounded-scan repair

**Tasks Completed**: The bounded contract audit identified two concrete `SCAN_LIMIT` gaps. `bracketMatch` now stops first-bracket discovery at the limit and returns null if no bracket was inspected within budget. Paragraph `{`/`}` scans now reject a candidate line before trimming it when that complete line would exceed the remaining budget, and propagate null as a failed motion. Added regression assertions for an over-limit first bracket and oversized paragraph lines in both directions. No downstream VIM-30 code or review finding was changed.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `31dbc4a839d12e7f70d74b27029a953f8d490f2f8b63326b553968cea839e205`; `editor/test/code/vim/motions.test.ts` SHA-256 `607361217e4b2fd38a2f667eada7eb56b7b2bf0ca52c965ffef4fd4ebdc08cc0`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/step6-dispatch-20261017-bound-repair-tsc.log", notes: "Scoped typecheck on the repaired source; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 90, testsPassed: 90, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/step6-dispatch-20261017-bound-repair-vitest.log", notes: "Both files passed; 90/90 tests, including the new scan-bound regressions."}`
- `{command: "cd editor && wc -l src/code/vim/motions.ts test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/step6-dispatch-20261017-bound-repair-line-count.log", notes: "256 and 132 lines; motions.ts remains below 450 lines."}`
- `{command: "git diff --check -- editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts impl-plans/active/vim-mode-10-motions.md", exitStatus: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/step6-dispatch-20261017-diff-check.log", notes: "No whitespace errors."}`

**Review State**: The identified VIM-10 scan-bound defects are repaired and verified on current source. Independent integrity, adversarial, and combined-tree integration review remain downstream workflow steps.

### Session: 2026-10-17 Step 6 current-source verification (dispatch attempt 04)

**Tasks Completed**: Revalidated the assigned VIM-10 contract on the current shared tree. The bounded `%` first-bracket scan and `{`/`}` paragraph scans remain in place with regression assertions for over-limit discovery and oversized lines in both directions. VIM-00 is accepted by the dispatch dependency record. The carried `IR-388-VIM30-EVIDENCE` finding is owned by VIM-30 and does not change VIM-10 ownership or block these gates.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `31dbc4a839d12e7f70d74b27029a953f8d490f2f8b63326b553968cea839e205`; `editor/test/code/vim/motions.test.ts` SHA-256 `607361217e4b2fd38a2f667eada7eb56b7b2bf0ca52c965ffef4fd4ebdc08cc0`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-04/typecheck.log", notes: "Current-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 90, testsPassed: 90, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-04/vitest.log", notes: "Both files passed; 90/90 tests, including bounded-scan regressions."}`
- `{command: "wc -l editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-04/lines.log", notes: "256 and 132 lines; motions.ts is below the 450-line limit."}`
- `{command: "git diff --check -- editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts impl-plans/active/vim-mode-10-motions.md", exitStatus: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-04/diff-check.log", notes: "No whitespace errors."}`

**Verification Harness Note**: The first attempt's tee path was relative to `editor/` after the command changed directories, so the wrapper produced no complete log. It is excluded; the exact scoped typecheck and Vitest commands above were rerun with complete logs and exited 0.

**Review State**: VIM-10's assigned implementation and behavioral verification are complete on this source. Independent integrity/adversarial review and combined-tree integration remain downstream; `IR-388-VIM30-EVIDENCE` remains owned by VIM-30.

### Session: 2026-10-17 Step 6 exact-limit paragraph-scan repair (dispatch attempt 05)

**Tasks Completed**: Addressed `VIM10-PARAGRAPH-EXACT-LIMIT`. A paragraph scan that exhausted exactly `SCAN_LIMIT` on a nonblank line could return that line's start without inspecting the next boundary. `{` and `}` now return null when the budget is exhausted before a blank boundary or document edge. Added exact-budget forward and backward regression assertions. The existing 90-case test total is unchanged because the assertions extend one test case. The unrelated `IR-388-VIM30-EVIDENCE` finding remains assigned to VIM-30.

**Source Identity**: `editor/src/code/vim/motions.ts` SHA-256 `9a8565cc1a02c02d1fccd92d9d075ab28fb1e3f7726a046d699148796d6b871a`; `editor/test/code/vim/motions.test.ts` SHA-256 `770116c8e59a37e625c260576e91ab6c0eef430a19b69098dd5729b52b18a7df`.

**Verification**:

- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-05/typecheck.log", notes: "Final-source scoped typecheck; no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/motions.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 90, testsPassed: 90, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-05/vitest.log", notes: "Both files passed, including exact-limit forward and backward paragraph regressions."}`
- `{command: "wc -l editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-05/lines.log", notes: "260 and 136 lines; motions.ts remains below the 450-line limit."}`
- `{command: "git diff --check -- editor/src/code/vim/motions.ts editor/test/code/vim/motions.test.ts impl-plans/active/vim-mode-10-motions.md", exitStatus: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-10/attempt-dispatch-20261017-05/diff-check.log", notes: "No whitespace errors."}`

**Review State**: The VIM-10 exact-limit paragraph scan finding is repaired and verified on current source. Independent re-review and combined-tree integration remain downstream workflow steps.

### Session: 2026-10-17 archive (session 391, Step 8)

**Tasks Completed**: Marked Completed and moved to `impl-plans/completed/`. Acceptance basis: the session-391 fanout lists VIM-10 in `acceptedPlanIds`/`completedBranchIds` (`shared-branch-implementation:dispatch-plans-attempt-8-exec-5554`), and the serial integration review ran the combined-tree gates on this source (`tmp/vim-mode/reconcile-391-wave8/`: check, full vitest, build, style, e2e 32/32, all exit 0; `tree-identity.log` records `motions.ts` `9a8565cc…`). No source change in this step.
