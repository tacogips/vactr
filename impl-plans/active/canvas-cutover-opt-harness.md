# Canvas Cutover OPT-HARNESS: Sync Sample De-duplication, Dropped Frames and the 8 ms textWork Gate Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-OPT-HARNESS (session 286, wave 8; first of the serial OPT chain)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 5 (harness correctness) and section 6 (budgets); 15.3.8.8 table rows "Frame work" and "A/V sync, model"; 15.3.8.12 "Sync attribution"
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-HARNESS`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

The user asked to "optimize performance" and to make the editor fast outside Chrome. Before
product code changes, the measurement harness must report correct numbers.

The operator diagnosis (`tmp/canvas-cutover/diag-shape/REPORT.md` section 2, gitignored)
found that `attributeSync` in `editor/test/e2e/stats.mjs` (lines 104-137) emits one sample
per onset row. The workload has 64 voices sharing one onset time, so each beat yields 64
identical samples. With only about 24 distinct onsets, p95 and p99 both land on one late
beat group. This is a harness correctness defect, not a threshold change.

Design 15.3.8.14 also tightens the text-dirty work gate from 16.7 ms to 8 ms, and records
two non-gating targets: textWork p95 4 ms and animation-work p50 1 ms.

## Non-goals

- No change to any other `THRESHOLDS` value: input 50/100, animation 4/8/16.7, frame
  interval 20/50, sync 33.4/50, early flash 2, ledger 96 MiB, heap 8 MiB, late-beat drift 1.
- No change to the sync sample formula
  `next.frameMs - (row.targetMs + (onset.time - row.audibleTime) * 1000)`.
- The sync gate is not narrowed to dropped-frame-free samples. `syncAbsMs` stays over all
  de-duplicated samples.
- No change to the 250 ms injected stall, the workload (`fixtures/large-doc.mjs`), the
  silent sink (`silent-sink.mjs`), `run.mjs` exit codes or any product file under
  `editor/src`.
- No mutation or negative-control command. Show sensitivity with in-test control rows.

## Ownership

writePaths:

- `editor/test/e2e/stats.mjs`
- `editor/test/e2e/stats.test.ts`
- `editor/test/e2e/measure.mjs`
- `impl-plans/active/canvas-cutover-opt-harness.md` (checkboxes and progress log only)
- `tmp/canvas-cutover/opt-harness` (artifact root: intent, receipt and logs)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`

sharedPaths:

- `editor/test/e2e/run.mjs`: no edit expected. Edit it only if `measure.mjs` cannot pass the
  new metric fields through, and record the reason.

## Contracts and Key Points

### 1. `attributeSync(onsets, presented, options)` de-duplication (`stats.mjs`)

Keep the signature. Add `options.nominalMs?: number` (a test override).

- Today the second loop (lines 124-134) pushes one sample per onset in `windowOnsets`. Keep
  that search, which finds the first presented frame index `index` showing the onset's
  range. Then de-duplicate by the key `${onset.time}|${onset.epoch ?? ''}|${index}`. Push a
  sample only for the first onset with each key.
- Return the existing fields (`sync`, `earlyFlashCount`, `replayedFlashCount`, `framePairs`,
  `windowOnsets`, `excludedFrames`, `windowStart`, `windowEnd`). `sync` now holds the
  de-duplicated values. Add these fields:
  - `samples`: `Array<{ time, epoch, frameIndex, value, droppedFrames }>`, the same order as
    `sync`;
  - `duplicateSamples`: the number of onset matches folded into an existing key;
  - `droppedFrameSamples`: the samples with `droppedFrames >= 1`;
  - `droppedFrames`: the sum of `droppedFrames`.
- `droppedFrames = Math.max(0, Math.round((next.frameMs - row.frameMs) / nominal) - 1)`.
  - `nominal` is `options.nominalMs` when it is given.
  - Otherwise it is the median of the positive `frameMs` deltas between consecutive
    `presented` rows, over the whole `presented` array.
  - If there are no deltas, `nominal` is 1000/60.
- The early, replayed and frame-pair counting loop (lines 115-122) does not change.

### 2. Thresholds and targets (`stats.mjs`)

- `THRESHOLDS.textWorkP95Ms` becomes `8`. Nothing else in `THRESHOLDS` changes.
- Export `TARGETS = Object.freeze({ textWorkP95Ms: 4, animationWorkP50Ms: 1 })`.
- `evaluate()` does not gate on `TARGETS`. It adds
  `targets: { textWorkP95Met: boolean | null, animationWorkP50Met: boolean | null }` to its
  result. Each value is `null` when the metric is not finite. `pass` and `failures` stay
  computed exactly as today.
- `renderEvidence()` makes these row changes:
  - text-dirty row label `'<= 8 (target 4, recorded)'`;
  - animation p50 row label `'<= 4 (target 1, recorded)'`;
  - two new rows, "A/V sync dropped-frame samples" and "A/V sync duplicates folded",
    from `metrics.syncDroppedFrameSamples` and `metrics.syncDuplicateSamples`;
  - an informational row "A/V model absolute error p95/p99 without dropped frames
    (informational)" from `metrics.syncAbsMsNoDrop`.

  Keep every existing row.

### 3. `measure.mjs` metrics

Line 75 already calls `attributeSync(onsets, presented)`. Add these fields to `metrics`:

- `syncDroppedFrameSamples`;
- `syncDroppedFrames`;
- `syncDuplicateSamples`;
- `syncAbsMsNoDrop: { p95, p99 }`, over `samples.filter(s => s.droppedFrames === 0)`, using
  the existing `max(values, p)` helper pattern;
- `targets` (copied from `TARGETS`).

`syncAbsMs` keeps using `sync` (now de-duplicated). Do not add any other gate.

### Patterns to imitate

- `stats.mjs` `syncWindow` and `attributeSync`: pure functions, no globals.
- `stats.test.ts` rows 79-113: these attributeSync rows build `onsets` and `presented` arrays
  inline and assert the returned fields.

## Tasks

### TASK-H1: De-duplication and dropped frames
**Deliverables**: `stats.mjs` `attributeSync`; `measure.mjs` metric fields.
**Completion criteria**: contract 1 and 3; the new rows pass; the existing rows 79-113 still
pass. Update an existing row only if it encoded duplicate samples, and record that in the log.

### TASK-H2: Threshold 8 ms and targets
**Deliverables**: `THRESHOLDS`, `TARGETS`, `evaluate().targets`, `renderEvidence` rows.
**Completion criteria**: contract 2.

### TASK-H3: Verification and progress log

## Test Cases (`editor/test/e2e/stats.test.ts`)

Extend the `StatsModule` interface at the top of the file with the new fields and
`TARGETS`.

- **Fixture.** `passingMetrics.textWorkMs.p95` is 12 today, which fails the 8 ms gate. Change
  it to `6`. Record this in the progress log as "fixture follows the tightened gate; not a
  weakening".
- 64 onsets with the same `time` and epoch and 64 distinct ranges, all first shown on the
  same presented frame -> `sync.length === 1`, `duplicateSamples === 63`.
- The same 64 onsets, 32 first shown on frame k and 32 on frame k+1 -> `sync.length === 2`.
- Two distinct onset times on the same frame -> 2 samples (de-duplication keys on time).
- With `nominalMs: 16.7`, `next.frameMs - row.frameMs = 50` -> `droppedFrames === 2`, and
  `droppedFrameSamples === 1`. A delta of 16.7 -> `droppedFrames === 0`.
- No `nominalMs`, with presented deltas `[16, 17, 16, 50]` -> nominal is the median (16.5),
  so a 50 ms delta gives `droppedFrames === 2`.
- `evaluate` with `textWorkMs.p95 = 8.1` -> `pass === false`, with a failure naming
  `textWorkMs`. With `8.0` -> `pass === true` (in-test control pair).
- `evaluate` with `textWorkMs.p95 = 5` -> `targets.textWorkP95Met === false`, and `pass` is
  still `true` (targets never gate).
- `THRESHOLDS.syncAbsP95Ms === 33.4`, `syncAbsP99Ms === 50`, `inputP95Ms === 50` and
  `frameIntervalP95Ms === 20` (an unchanged-threshold guard).
- `renderEvidence` contains `'<= 8 (target 4, recorded)'` and the dropped-frame row.

## Pitfalls

- Do not de-duplicate by range. Distinct voices have distinct ranges and the same time;
  the key is time, epoch and frame index.
- Do not exclude dropped-frame samples from `syncAbsMs`. That would loosen the gate.
- `presented` can be empty. Return `nominal` 1000/60 and no samples, and never throw.
- Keep `stats.mjs` free of imports from `editor/src`. It runs in Node.
- Do not change `percentile` or `max` semantics.

## Verification

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`
and `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.

At start, record `BASE=$(git rev-parse HEAD)` and the fresh-read sha256 of each writePath in
`tmp/canvas-cutover/opt-harness/intent.json`.

Inside the sandbox (logs in `tmp/canvas-cutover/opt-harness/`):

| Command | Required evidence |
|---------|-------------------|
| `node --check editor/test/e2e/stats.mjs editor/test/e2e/measure.mjs` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/e2e/stats.test.ts` | exit 0; the new rows are listed by name |
| `cd editor && ./node_modules/.bin/vitest run test/e2e` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 (default config) |
| `cd editor && npm run check` | exit 0 |
| `git diff --name-only $BASE` | only this plan's writePaths |

Outside the sandbox (verification step):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run test:perf` (alone on the host) | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |

Every `verification[]` record uses `{command, exitStatus: 0, testsRun > 0, testsPassed,
failureCount: 0, outcome: "passed", log}` for test commands, and
`{command, exitStatus: 0, outcome: "passed", log}` for other commands. Details go in `notes`.
Run no mutation or negative-control command.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/opt-harness/intent.json`
and `receipt.json`. If a file changed from its fresh read without an edit from this plan,
stop editing it and report. Edit only this plan's progress log.

## Completion Criteria

- [ ] `attributeSync` emits one sample per (onset time, epoch, first presented frame); 64 voices on one frame -> 1 sample
- [ ] `droppedFrames`, `droppedFrameSamples`, `duplicateSamples` and `samples` returned; `measure.mjs` reports `syncDroppedFrameSamples`, `syncDroppedFrames`, `syncDuplicateSamples`, `syncAbsMsNoDrop` and `targets`
- [ ] `THRESHOLDS.textWorkP95Ms === 8`; every other threshold unchanged (guard row passes)
- [ ] `TARGETS` exported and recorded without gating; `renderEvidence` rows updated
- [ ] `git diff $BASE -- editor/test/e2e/stats.mjs` shows only `textWorkP95Ms` changed inside `THRESHOLDS`
- [ ] Default vitest, `npm run check`, `test:perf`, clippy, nextest, wasm32 build and src-tauri check pass
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-06 (session 286 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 sections 5 and 6.
**Notes**: Wave 8, runs alone. Product files are untouched.

### Session: 2026-10-06 (session 287 restart; plan-author note, no re-plan)
**Tasks Completed**: None accepted. Session 286 stopped for host memory pressure. Commit
28ed027 holds unreviewed partial edits to `stats.mjs`, `stats.test.ts` and `measure.mjs`.
**Notes**: Contracts, tests and criteria are unchanged. Two restart rules apply:
- **Diff base.** `$BASE` in the Verification table and the Completion Criteria means
  `1d17ced`, the plan checkpoint. Do not use the current HEAD. Record `START=$(git rev-parse HEAD)`
  separately in `intent.json`. `git diff --name-only 1d17ced` must list only this plan's
  writePaths. `git diff 1d17ced -- editor/test/e2e/stats.mjs` must show only
  `textWorkP95Ms` changed inside `THRESHOLDS`.
- **Starting state.** Treat the 28ed027 edits as a draft. Check them line by line against
  contracts 1-3 and every Test Cases bullet. Complete or fix anything missing, then run the
  full verification table. Do not revert the draft wholesale.
