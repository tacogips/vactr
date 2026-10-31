# EMK-LANG: incremental highlighter, JSON language and deterministic fixture

**Status**: Completed (accepted in session 607 wave 1: test-integrity, adversarial review and branch evidence passed; wave-2 join green in tmp/emaki/wave-2-join/summary.json)
**Plan ID**: EMK-LANG (wave 2; parallel with EMK-DECOR, EMK-WIDGETS, EMK-DIFF)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 9.1-9.3, 11.2; decision EM-D9
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants a 10,000-line JSON file with syntax highlighting to open and
edit fast. Only visible lines may be tokenized, and tokenization must be
incremental. EMK-CORE pinned `LineLanguage`, `SpanLanguage` and `TokenSpan`
in `emaki/src/types.ts`. This plan delivers:

- the `IncrementalHighlighter` for line-kind languages;
- the built-in JSON language;
- the deterministic fixture generator shared by the demo, the perf script
  and the tests.

## Non-goals

- No wiring into `createEditor` (EMK-API).
- No vact provider (EMK-VAC-SWITCH).
- No rendering change.

## Dependencies

- **dependsOn**: EMK-CORE.

## writePaths

Source:
- `emaki/src/language/index.ts`, `emaki/src/language/incremental.ts`,
  `emaki/src/language/json.ts`, `emaki/src/language/json-tokenizer.ts`,
  `emaki/src/language/fixture.ts`

Tests:
- `emaki/test/language/incremental.test.ts`,
  `emaki/test/language/json.test.ts`,
  `emaki/test/language/fixture.test.ts`

Generated, plus this plan:
- `emaki/node_modules`, `emaki/dist` (artifactRoots)
- `impl-plans/active/emaki-22-lang.md`

## sharedPaths

None.

## Tasks

### L1: `language/incremental.ts`, class `IncrementalHighlighter<S>`

**Constructor.**
`new IncrementalHighlighter(lang: LineLanguage<S>, opts?: { checkpointEvery?: 32; maxLinesPerFrame?: 2000; maxMsPerFrame?: 2; cacheLines?: 4096; now?: () => number })`.

**Methods**
- `setDoc(doc: Text)`: resets everything.
- `applyChanges(changes: ChangeSet, newDoc: Text)`:
  - find the first changed line (0-based);
  - invalidate checkpoints at and after it;
  - when the line count changed, splice the checkpoint array by
    `delta / 32`, keeping checkpoints before the edit;
  - evict LRU token entries for changed lines.

  Do not retokenize here.
- `tokens(fromLine, toLine): TokenSpan[]` (absolute positions):
  1. Ensure the start state of `fromLine`: walk from the nearest valid
     checkpoint at or below `fromLine`. Spend at most `maxLinesPerFrame`
     lines or `maxMsPerFrame` ms on lines above the viewport.
  2. If the budget runs out, tokenize the visible lines with the last known
     state, and mark `approximate = true` for this call.
  3. Tokenize the visible lines through the LRU cache, keyed by
     `lineText + '\u0000' + stateKey`. `stateKey` is `String(state)` when
     `S` is a string, else a per-instance WeakMap id.
  4. While walking, store checkpoint states every 32 lines.
  5. When a recomputed checkpoint state `sameState` equals the previously
     stored (pre-invalidation) state at that checkpoint, stop invalidating:
     restore the remaining stale checkpoints as valid (convergence).
- `catchUp(): boolean`: does one bounded slice of background work. Returns
  `true` while dirty checkpoints remain above the furthest requested line.
- `readonly approximate`.
- `stats`: `{ linesTokenized, checkpointsValid, cacheHits }`.

**Pitfalls**
- Keep the pre-invalidation states, to detect convergence.
- Never iterate the whole document on `applyChanges`. Use
  `changes.iterChangedRanges` and `doc.lineAt`.
- `maxMsPerFrame` uses the injected `now` (default `performance.now`).

### L2: JSON (`language/json-tokenizer.ts`, `language/json.ts`)

`jsonLanguage: LineLanguage<string>`.

**State.** A compact string: the container stack characters (`{` or `[`),
then `|`, then a flag `k` (expect key) or `v`. The start state is `|v`.

**Tags**

| Tag | Token |
|-----|-------|
| `json-key` | a string in key position (followed, after spaces, by `:`) |
| `json-string` | other strings |
| `json-number` | numbers |
| `json-bool` | `true`, `false` |
| `json-null` | `null` |
| `json-punct` | `{}[]:,` |
| `json-invalid` | anything else |

Unterminated strings run to the end of the line, tagged `json-string`, and
the state is unchanged. JSON strings cannot span lines.

**Export.** `json.ts` exports `jsonLanguage`, plus
`export { generateJsonFixture } from './fixture'`. It is the `emaki/json`
package entry.

### L3: `language/fixture.ts`

`generateJsonFixture({ lines: number; seed?: number = 1 }): string`.

- Use a mulberry32 PRNG, with no `Math.random`.
- Output pretty JSON with 2-space indentation. It is an array of objects,
  each with fields: `id` (number), `name` (string with occasional `\"` and
  `\\` escapes), `tags` (array of 1-4 strings), `active` (bool), `score`
  (float), `parent` (null or number), and a nested `meta` object.
- The output has exactly `lines` lines, without a trailing newline, and
  parses with `JSON.parse`.
- Exact count: emit whole records while the remaining budget allows, then
  close the array. Pad by adjusting the last record's `tags` length or
  adding single-line fields so the total equals `lines` exactly. If
  `lines < 3`, throw `RangeError`.

### L4: Tests

**`json.test.ts`**
- `{"a": [1, true, null, "x\"y"]}` -> tags in order:
  `punct, key, punct, punct, number, punct, bool, punct, null, punct, string, punct, punct`;
- a key vs value string on separate lines with state carried ->
  `json-key`, then `json-string`;
- an unterminated string -> `json-string` to the end of the line.

**`incremental.test.ts`** (with a counting wrapper language)
- `tokens(0, 50)` on a 10k-line fixture tokenizes at most `50 + overscan`
  lines (no full-document walk);
- edit one value inside line 5000 after warming lines 0-6000 -> the next
  `tokens(4990, 5010)` retokenizes from checkpoint 4992 and stops at the
  next checkpoint because the state converged (count <= 64);
- an edit that changes the state, such as inserting `"` that opens a key
  structure change like `{` -> downstream checkpoints recompute lazily only
  when requested;
- the budget: `tokens(9000, 9050)` cold, with `maxLinesPerFrame` 2000 ->
  `approximate === true`, and repeated `catchUp()` calls eventually give
  exact tokens equal to a full sequential tokenization;
- the per-edit cost at 1k vs 100k lines: `applyChanges` plus
  `tokens(visible)` line counts are equal within 5%.

**`fixture.test.ts`**
- `lines: 10000, seed: 1` -> exactly 10000 lines, `JSON.parse` OK, UTF-8
  byte length between 300,000 and 600,000;
- a stable SHA-256: compute it once, then pin the literal in the test, and
  record the value in the Progress Log. Use `crypto.subtle.digest` (jsdom
  provides `crypto`; otherwise mark the file `@vitest-environment node`);
- `lines: 1000` and `100000` -> exact counts, and parse OK;
- `lines: 2` -> `RangeError`.

## Invariants

- No module here imports DOM APIs.
- All functions are deterministic given their inputs.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-LANG/`.

1. `npm run check`: passes per rule 12b.
2. `./node_modules/.bin/vitest run test/language`: exit 0, testsRun > 0.
   **This is the pass gate.**
3. `./node_modules/.bin/vitest run`: informational per rule 12b. The
   wave-2 join gate is authoritative.
4. `wc -l` on every written file: each under 1000.

**mutationEvidence.** Disable the convergence stop (always invalidate to the
end) -> the convergence test fails. Restore afterwards.

## Completion Criteria

- [x] L1-L4 delivered. `language/index.ts` exports `IncrementalHighlighter`,
  `jsonLanguage` and `generateJsonFixture`.
- [x] Verification steps pass. The fixture SHA-256 is recorded. Mutation
  evidence is recorded. Hashes are logged.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.

### Session: 2026-11-01 (implementation)
**Tasks Completed**: L1-L4. Implemented checkpointed incremental tokenization, bounded catch-up, LRU, JSON tokens, and deterministic fixture. Multi-range edits cannot converge before the last changed line.
**Evidence**: scoped vitest 15/15 (`tmp/emaki/EMK-LANG/vitest-final-5.log`); `npm run check` exit 0 (`tmp/emaki/EMK-LANG/check-final-3.log`). Full vitest informational 93/93 (`tmp/emaki/EMK-LANG/full-final.log`); the serial wave join is authoritative. Convergence negative control failed as intended and was restored byte-identically (`tmp/emaki/EMK-LANG/mutation-3.log`, `mutation-3-before.sha256`, `mutation-3-after.sha256`). Fixture SHA-256: `78d56a6b3cd73f3c437f83203525c73eafbbc6c579f4fd7609cf06ea4fd8d46c`. Source hashes and line counts: `tmp/emaki/EMK-LANG/final-hashes.txt`, `tmp/emaki/EMK-LANG/final-lines.log`. Formal reviews and wave join remain downstream.

### Session: 2026-11-01 (Step 6 re-verification)
**Tasks Completed**: Rechecked L1-L4 against the accepted design and dispatched scope; no source changes were necessary.
**Evidence**: scoped vitest 15/15 (`tmp/emaki/EMK-LANG/step6-scoped-20261101.log`); `npm run check` exit 0 (`tmp/emaki/EMK-LANG/step6-check-20261101.log`); full vitest informational 93/93 (`tmp/emaki/EMK-LANG/step6-full-20261101.log`); all assigned source files under 1000 lines (`tmp/emaki/EMK-LANG/step6-lines-20261101.log`). The recorded convergence mutation restores the current `incremental.ts` hash byte-identically. Formal reviews and wave join remain downstream.

### Session: 2026-11-01 (test-integrity revision EMK-LANG-TI-01)
**Tasks Completed**: Added stateful JSON fixture coverage for catch-up and nonzero checkpoint resumption, including visible-created checkpoints. Existing tests and fixture pin remain unchanged.
**Evidence**: scoped vitest 16/16 (`tmp/emaki/EMK-LANG/ti01-final-vitest.log`); `npm run check` exit 0 (`tmp/emaki/EMK-LANG/ti01-final-check.log`). Both checkpoint boundary and resume-state mutations fail the new test (`tmp/emaki/EMK-LANG/ti01-mutation-a3.log`, `ti01-mutation-b.log`); before/after hashes in `ti01-a3-{before,after}.sha256` and `ti01-b-{before,after}.sha256` match the original source. Independent re-review and wave join remain downstream.

### Session: 2026-11-01 (adversarial revision EMK-LANG-AR-01)
**Tasks Completed**: Bounded the catch-up target after document shrink and added a deletion regression without changing existing tests or the fixture.
**Evidence**: scoped vitest 17/17 (`tmp/emaki/EMK-LANG/ar01-final-vitest.log`); `npm run check` exit 0 (`tmp/emaki/EMK-LANG/ar01-final-check.log`). Removing both bounds reproduced the RangeError (`tmp/emaki/EMK-LANG/ar01-mutation.log`); restored source hashes match (`tmp/emaki/EMK-LANG/ar01-before-mutation.sha256`, `tmp/emaki/EMK-LANG/ar01-after-mutation.sha256`). Independent re-review and wave join remain downstream.
