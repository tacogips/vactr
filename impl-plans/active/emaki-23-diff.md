# EMK-DIFF: reusable line/word diff and split alignment (`emaki/diff`)

**Status**: In Progress (implementation complete; review pending)
**Plan ID**: EMK-DIFF (wave 2; parallel with EMK-DECOR, EMK-WIDGETS, EMK-LANG)
**Design Reference**: `design-docs/specs/design-emaki.md` section 10
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants a GitHub-style split diff demo. The design puts the diff
engine in emaki as a reusable, DOM-free module behind the `emaki/diff`
entry. This plan delivers:

- **`diffLines`**: prefix/suffix trimming, then Myers O(ND) with a cost cap
  and a single replace hunk as the fallback.
- **`diffWords`**: a word-level diff within a line pair.
- **`alignSplit`**: rows and filler requirements for a two-pane view.

## Non-goals

- No UI and no editor wiring. EMK-DIFF-DEMO does that.
- No patience or histogram algorithm.

## Dependencies

- **dependsOn**: EMK-CORE (package and toolchain only).

## writePaths

Source:
- `emaki/src/diff/index.ts`, `emaki/src/diff/myers.ts`,
  `emaki/src/diff/lines.ts`, `emaki/src/diff/words.ts`,
  `emaki/src/diff/split.ts`

Tests:
- `emaki/test/diff/lines.test.ts`, `emaki/test/diff/words.test.ts`,
  `emaki/test/diff/split.test.ts`

Generated, plus this plan:
- `emaki/node_modules`, `emaki/dist` (artifactRoots)
- `impl-plans/active/emaki-23-diff.md`

## sharedPaths

None.

## Tasks

### F1: Contract types (exported from `diff/index.ts`)

- `interface Hunk { aFrom: number; aTo: number; bFrom: number; bTo: number }`:
  0-based, half-open line ranges. Equal regions are not emitted.
- `diffLines(a: readonly string[], b: readonly string[], opts?: { maxCost?: number }): Hunk[]`.
  The default `maxCost` is 2000.
- `diffWords(a: string, b: string): { a: Range[]; b: Range[] }`: changed
  character ranges in each string.
- `alignSplit(hunks: readonly Hunk[], aLength: number, bLength: number): SplitLayout`, where
  `SplitLayout = { removed: number[]; added: number[]; modified: { a: number; b: number }[]; fillers: { side: 'a' | 'b'; afterLine: number; count: number }[] }`.
  - `afterLine = -1` means above the first line.
  - In a hunk with `d` deletions and `i` insertions, the first `min(d, i)`
    lines pair as `modified`.
  - The extra `|d - i|` lines are `removed` (side a) or `added` (side b),
    with one filler of `|d - i|` rows on the other side, placed after the
    other side's last line of the hunk (or `bFrom - 1`).

### F2: Algorithms

**`lines.ts`**
1. Trim the common prefix and suffix with `===` on line strings.
2. Intern the middle lines to integer ids with a `Map<string, number>`.
3. Run `myers.ts` on the id arrays.
4. Translate the result back with offsets, and merge adjacent edits into
   hunks.

**`myers.ts`**
- Classic forward Myers with `V` arrays (an `Int32Array`) and a trace for
  backtracking.
- If `D` exceeds `maxCost`, return `null`. `lines.ts` then emits one hunk
  covering the whole trimmed middle.

**`words.ts`**
- Tokenize with `/\w+|\s+|[^\w\s]/gu`, then run Myers on the token string
  arrays (reuse `myers.ts`).
- Map changed tokens to character ranges, merging adjacent ones.
- If either string is longer than 2000 characters, return the whole of both
  as changed.

**Pitfalls**
- Do not split on `\n` inside `diffLines`. Callers pass line arrays.
- Hunks must be sorted and non-overlapping.
- Empty input arrays are valid.

### F3: Tests

**`lines.test.ts`**
- identical -> `[]`;
- one line changed in the middle of 10k -> one hunk
  `{5000,5001,5000,5001}`;
- insert 3 lines -> `{k,k,k,k+3}`;
- delete at start, insert at end -> two hunks;
- an empty `a` -> one hunk `{0,0,0,b.length}`;
- `maxCost: 2` with 5 scattered changes -> exactly one replace hunk spanning
  the trimmed middle;
- a randomized property test, 200 seeded cases with a mulberry32 inline:
  applying the hunks (take `b[bFrom..bTo]` for each hunk, the rest from `a`)
  reconstructs `b` exactly, and the edit count is at most `|a| + |b|`.

**`words.test.ts`**
- `'let x = 1'` vs `'let y = 1'` -> `a: [{4,5}]`, `b: [{4,5}]`;
- identical -> empty;
- a 3000-character line -> whole ranges.

**`split.test.ts`**
- hunk `{2,4,2,3}` -> `modified [{a:2,b:2}]`, `removed [3]`,
  `fillers [{side:'b', afterLine:2, count:1}]`;
- a pure insertion of 2 at the top -> `added [0,1]`,
  `fillers [{side:'a', afterLine:-1, count:2}]`;
- the total rows per side are equal: `aLength + fillers(a) === bLength + fillers(b)`.

**Performance check** (recorded, not gated here). A 10k-line `a`, with `b` =
`a` with one changed line -> `diffLines` takes under 5 ms in vitest. Record
the measured time in the Progress Log.

## Invariants

- No DOM.
- No import from outside `src/diff` except `../types` for `Range`.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-DIFF/`.

1. `npm run check`: passes per rule 12b.
2. `./node_modules/.bin/vitest run test/diff`: exit 0, testsRun > 0.
   **This is the pass gate.**
3. `./node_modules/.bin/vitest run`: informational per rule 12b. The
   wave-2 join gate is authoritative.
4. `wc -l` on every written file: each under 1000.

**mutationEvidence.** Remove suffix trimming -> the 10k single-change hunk
test still passes, but add and run a timing assertion only in the mutation
run, and record the slower time. Separately, break backtracking -> the
property test fails. Restore afterwards.

## Completion Criteria

- [x] F1-F3 delivered, and `diff/index.ts` exports exactly the F1 symbols.
- [x] Verification steps pass. Mutation evidence is recorded. Hashes are
  logged.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.

### Session: 2026-11-01 (EMK-DIFF implementation)
**Tasks Completed**: F1-F3; bounded Myers, trimmed line hunks, character ranges and split alignment. Scoped tests and TypeScript check passed; full suite informational: 73 tests passed.
**Evidence**: `tmp/emaki/EMK-DIFF/`. The 10k change took 1.320 ms; without suffix trimming 2.948 ms, and the fallback-hunk test detected the mutation. A separate backtracking mutation failed the property test. Both controls were restored byte-identically. The mutation-only <5 ms assertion passed on this machine; timing is informational. Formal reviews and the serial wave join remain downstream.

### Session: 2026-11-01 (test-integrity repair EMK-DIFF-TI-01)
**Tasks Completed**: Strengthened the cost-cap test with five scattered changes, asserting both the capped single hunk and five default-cost hunks. Ignoring the cap caused the focused test to fail; `lines.ts` was restored byte-identically. Evidence and refreshed hashes are under `tmp/emaki/EMK-DIFF/ti01-*`. Independent re-review remains downstream.
