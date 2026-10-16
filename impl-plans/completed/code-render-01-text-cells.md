# Code Render 01: Run-Relative Text Cells (RC2)

**Status**: Completed
**Plan ID**: CR-01
**Wave**: 1 (no dependencies)
**Design Reference**: design-docs/specs/design-code-pane-text-rendering.md sections 3 (RC2), 4.1 (R1, R2), 5.1 (U1)
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

The canvas code pane drops the first N characters of every line whose run
starts at document offset N, and draws nothing when N is at least the line
length. Line 1 is unaffected. The cause is in
`editor/src/code/layout.ts:TextLayout.textCells`. The ASCII branch starts
at `Math.max(0, this.offsetInRun(run, visibleFrom, -1))`, but
`offsetInRun` returns an ABSOLUTE document position (`run.from +
boundary`; it returns `run.from` when `x <= 0`). That value is then used
as an index into `run.text`. Chunk runs from `geometrySegments` also
carry absolute `from` (`layout.ts:393`). Repro evidence:
`tmp/code-render/t-scroll-redraw.png` shows line 2 starting at character
4 and line 3 at character 16.

## Non-goals

- Do not change `offsetInRun`. Hit testing (`advance`, `posAtCoords`,
  `boundary`) depends on its absolute contract.
- Do not touch `editor/src/code/renderer.ts`, `atlas.ts`, `geometry.ts`,
  `input.ts` or `keyboard.ts`, or any test outside this plan's writePaths.
- Do not edit `editor/test/canvas/gpu.test.ts` even if it fails. CR-05
  owns its reconciliation.

## Files

| Path | Change |
| --- | --- |
| `editor/src/code/layout.ts` | Keep the run-relative ASCII start. Propagate parent cluster metadata and additive width behavior to derived runs from `geometrySegments` so their prefix widths remain accurate without per-edit `measureText` calls. |
| `editor/test/code/text-cells.test.ts` | U1 cases plus derived-run coverage for cell geometry and zero measured text calls. |
| `impl-plans/active/code-render-01-text-cells.md` | Scope amendment and progress log. |

Imitate `editor/test/canvas/gpu.test.ts:layout` (lines 10-18) for the
fake metrics: a `TextLayout` built with `{ font: '', measureText: t => ({
width: ... }) }`, and a `LayoutFont` such as `{ font: '13px test',
lineHeight: 18, baseline: 14 }`.

## Key Points and Pitfalls

- The run-relative start remains `offsetInRun(run, visibleFrom, -1) - run.from`, clamped at 0. Preserve the non-ASCII `segmenter` behavior and the `pieces.map` geometry contract.
- INT-377-W4-01 amendment: derived runs inherit the parent run's cluster metadata and additive flag. Additive derived chunks use `AdvanceTable` widths; non-additive lines retain exact measured widths. Do not broaden the repair beyond this metadata/width propagation seam.
- CR01-ADV-01: when deriving a run after `setText` shifted a cached line, rebase copied cluster metadata to the current `line.from`; the cluster offsets remain line-relative while the parent run's absolute `from` has moved.
- `visibleFrom > 0` must still skip leading clusters. With the fix,
  `offsetInRun(run, visibleFrom, -1) - run.from` is the run-relative
  boundary at or before `visibleFrom`.
- The returned `from`/`to` stay run-relative. `renderer.ts:568` adds
  `run.from` to them; do not change that.

## Test Cases (`editor/test/code/text-cells.test.ts`)

Use a width function of 8 px per ASCII character.

- Document `# c\nuse-bpm 108\ns :kalimba > note :c5`, lines 0..2 -> for
  every chunk run of `geometrySegments(shape(n))`, `textCells(run)` texts
  concatenated equal the chunk run text, and the concatenation over all
  chunk runs of the line equals the line text.
- Same document -> for every returned cell, `cell.x ===
  layout.advance(line, run.from + cell.from) - run.x` (within 1e-9).
- The contents of `examples/kalimba.vact` (read with `node:fs` from the
  repository root; declare `// @vitest-environment node` only if jsdom
  blocks fs, otherwise read via `fs.readFileSync(new URL(...))`) -> every
  line's cells concatenate to the line text.
- A line at document offset 8 with text `abc def` (document `# hello\nabc
  def`) -> 7 cells, the first `a` at `x = 0`.
- `textCells(run, visibleFrom = 2 * 8 + 1)` on line 1 of the 3-line
  document -> the first returned cell has `from === 2` (it starts at the
  boundary at or before `visibleFrom`, not at 0 and not past the line).
- A tab-split line `a\tbc` on line 1 (document `x\na\tbc`) -> each run's
  cells concatenate to that run's text.
- For an additive line, `geometrySegments`-derived run cells match the parent run's cell x positions, and calling `textCells` on the derived runs does not increase `stats.measuredTextCalls`.
- For `aaaa\nx👍🏽y👍🏽z`, shift the cached second line by inserting `bbb` at document offset 0 through `EditorState.update` and `setText(doc, changes)`. With cluster-aware metrics (16 px per multi-code-unit grapheme, 8 px otherwise), assert the moved line keeps its runs identity and derived cells still match pre-shift and parent cells, including trailing `z`.

Each case fails on ea51a07 (cells missing, empty, or starting at the
absolute offset). CR-07 records that as mutationEvidence.

## Invariants

- `offsetInRun`, `advance`, `posAtCoords`, `boundary` and `rangeRects`
  outputs are unchanged.
- `layout.ts` stays at about 499 lines (well under 1000).

## Write Scope

Mirrors `impl-plans/active/code-render-dispatch.json` plans[CR-01].

- writePaths: `editor/src/code/layout.ts`, `editor/test/code/text-cells.test.ts`, `impl-plans/active/code-render-01-text-cells.md`, `tmp/code-render/CR-01`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules`
- sharedPaths: none
- artifactRoots: `tmp/code-render/CR-01`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules` (the last three are prebuilt read-only inputs: never rebuild, reinstall or delete them)
- Toolchain audit: before the first edit, from the repository root run
  `shasum -a 256 target/wasm32-unknown-unknown/debug/vactr.wasm tree-sitter-vact/tree-sitter-vact.wasm editor/package-lock.json > tmp/code-render/CR-01/toolchain-sha256.txt`
  and copy the three digests into the Progress Log.

## Working Rules

- Before each edit, re-read the file and record its sha256
  (`shasum -a 256 <file>`). After the edit, record the new sha256. If the
  pre-edit hash differs from the hash you last saw, re-read and re-plan
  the edit; never overwrite another plan's change.
- Wave-1 peers (CR-02, CR-03, CR-04, CR-06) edit other files at the same
  time. If `npm run check` fails only in files outside this plan's
  writePaths, wait briefly and re-run. If it still fails, record the
  file and error in the progress log as a cross-plan note; do not fix it.
- No git writes (no add, commit, stash, checkout). Foreground commands
  only. Logs go under `tmp/code-render/CR-01/`.

## Verification

Run from `editor/`. Each log must end with its exit status.

1. `npx vitest run test/code/text-cells.test.ts > ../tmp/code-render/CR-01/focused.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-01/focused.log`
   -> exit 0, all cases pass.
2. `npm run check > ../tmp/code-render/CR-01/check.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-01/check.log`
   -> exit 0, or only cross-plan errors as described above.
3. `npx vitest run test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts > ../tmp/code-render/CR-01/canvas.log 2>&1; echo "exit=$?" >> ../tmp/code-render/CR-01/canvas.log`
   -> informational. Record any failure caused by this change as a
   handoff note for CR-05 (test name and the expectation that changed).
   Do not edit those files.

Record gating results as `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`.

## Completion Criteria

- [x] `textCells` ASCII branch uses a run-relative start; derived runs inherit parent cluster metadata and additive width behavior
- [x] `editor/test/code/text-cells.test.ts` has the 6 U1 cases plus derived-run regression coverage and passes
- [x] A cluster-aware cached-line-shift regression fails on layout.ts `f0a370b5…` and passes after metadata rebasing
- [x] `npm run check` passes (or only cross-plan errors are recorded)
- [x] gpu/edit-cost handoff notes recorded (or "none")
- [x] Progress log updated with verification records

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Ready for wave 1.

### Session: 2026-10-16
**Tasks Completed**: CR-01 implementation and U1 verification
**Tasks In Progress**: None
**Blockers**: None
**Notes**:
- Changed only the ASCII `textCells` start to subtract `run.from`; added the six planned U1 cases in `editor/test/code/text-cells.test.ts`.
- Toolchain SHA-256: `target/wasm32-unknown-unknown/debug/vactr.wasm` = `de9c55f37b4037d73e28f8aaa11e06d3bc87a3a41a3bc95dd8e64ebf02c93210`; `tree-sitter-vact/tree-sitter-vact.wasm` = `ae3e5cdf1217e943f19c3ff15f111801e537ba9b7e463f84d6bf8cedd7fd0609`; `editor/package-lock.json` = `f32e9585c409487e0d74ee9b8dc1880d4e54ff8610ab2cefa529828c53449749` (capture: `tmp/code-render/CR-01/toolchain-sha256.txt`).
- Focused verification: `cd editor && npx vitest run test/code/text-cells.test.ts`; exit 0, 6/6 passed; complete log `tmp/code-render/CR-01/attempt-3/focused.log`.
- Type check: `cd editor && npm run check`; retried after 5 seconds, exit 1 both times only for `test/e2e/pixel-compare.test.ts(2,88)` (CR-06 cross-plan missing declaration for `./pixel-compare.mjs`); complete final log `tmp/code-render/CR-01/attempt-4/check.log`. The initial `node:fs` type error in this plan's test was corrected using the repository's typed dynamic-import pattern.
- Informational canvas verification: `cd editor && npx vitest run test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts`; 76/77 passed, one failure in `GPU code compositor > retains every unchanged text tile across animation frames without backdrop uploads`: expected 15 atlas uploads, observed 34 (`tmp/code-render/CR-01/attempt-2/canvas.log`). `edit-cost.test.ts` passed. Handoff to CR-05: reconcile the `gpu.test.ts:789` upload-count expectation with the restored text cells/cache keys; do not alter that file in CR-01.
- Initial focused run passed 6 tests but its log lacks the required exit line due to the shell's reserved `status` variable; retained at `tmp/code-render/CR-01/focused.log`. Complete source-matched rerun is recorded above.

### Session: 2026-10-16 (evidence repair)
**Tasks Completed**: INT-377-01 evidence repair
**Tasks In Progress**: None
**Blockers**: None
**Notes**:
- Fresh SHA-256 identities match the review: `editor/src/code/layout.ts` = `7d865bef9f61c86689ced11ad553968e9fe29c30463281f85561a11f07e5f237`; `editor/test/code/text-cells.test.ts` = `a88b94e008c768a7ae2f790e3494408d0dc246b37197d949eb771e4b98505390`. No source changes were needed.
- Current-source U1 verification: `cd editor && npx vitest run test/code/text-cells.test.ts`; exit 0, 6/6 passed. Complete log: `tmp/code-render/CR-01/attempt-5/focused.log`.
- Current combined-tree type check: `cd editor && npm run check`; exit 0. Complete log: `tmp/code-render/CR-01/attempt-5/check.log`.
- Gating verification for this repair consists only of these two exit-0 records. The earlier informational CR-05 handoff remains: `gpu.test.ts:789` expected 15 atlas uploads and observed 34 after CR-02's cache-key change; see `tmp/code-render/reconcile-377/canvas-informational.log`. It is not a CR-01 gate.
- Exact per-edit intent: `tmp/code-render/CR-01/attempt-5/edit-plan-intent.txt`.

### Session: 2026-10-16 (INT-377-W4-01 repair)
**Tasks Completed**: Scope amendment recorded; implementation and verification in progress
**Tasks In Progress**: Derived-run metadata/additivity propagation and regression coverage
**Blockers**: None
**Notes**:
- Amended CR-01 because derived runs created by `geometrySegments` were not registered in the parent's cluster metadata or additive width index, causing `textCells` to call `measureText` for every prefix on later lines. Preserve the existing run-relative start at `textCells`.
- Per-edit intent and source identities are recorded under `tmp/code-render/CR-01/attempt-6/`.
- Implemented only the amended seam in `layout.ts`: additive derived chunks use the existing `AdvanceTable`, while non-additive derived runs continue through `measureDirect`. The original `textCells` run-relative expression is unchanged.
- SHA-256 before repair: `layout.ts` = `7d865bef9f61c86689ced11ad553968e9fe29c30463281f85561a11f07e5f237`; `text-cells.test.ts` = `a88b94e008c768a7ae2f790e3494408d0dc246b37197d949eb771e4b98505390`. After repair: `layout.ts` = `f0a370b5c15aeddae53f680366a2ca959780624510a9d7f1d6a033919f9e8366`; `text-cells.test.ts` = `fcccb4f96de508c0abdca4291d6ac4f9172ffab070310fc14f2a7a2038230732`.
- `cd editor && npx vitest run test/code/text-cells.test.ts`: exit 0, 7/7 passed; complete log `tmp/code-render/CR-01/attempt-6/focused.log`. This includes matching global cell x positions and unchanged `measuredTextCalls` for additive derived runs.
- `cd editor && npm run check`: exit 0; complete log `tmp/code-render/CR-01/attempt-6/check.log`.
- Under `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`, `cd editor && npx vitest run test/canvas`: exit 0, 223/223 passed; complete log `tmp/code-render/CR-01/attempt-6/canvas.log`. The unchanged `editor/test/canvas/mount.test.ts:321` zero-measureText assertion passed. This resolves the CR-05 GPU/edit-cost handoff in the combined tree.
- `cd editor && npx vitest run test/code/renderer-layers.test.ts test/code/atlas-cells.test.ts test/code/glyph-placement.test.ts`: exit 0, 14/14 passed; complete log `tmp/code-render/CR-01/attempt-6/adjacent.log`.
- `layout.ts` is 511 lines. `git diff --check` passes. `mount.test.ts` remains byte-identical to HEAD.
- Per-edit intents and initial hashes are in `tmp/code-render/CR-01/attempt-6/`; all verification logs end with the exact shell exit status.

### Session: 2026-10-16 (CR01-ADV-01 adversarial repair)
**Tasks Completed**: Stale derived cluster metadata corrected; line-shift regression and required verification passed
**Tasks In Progress**: Independent review of this correction
**Blockers**: None
**Notes**:
- Rebased derived run metadata at `geometrySegments` to `{ lineFrom: line.from, clusters: metadata.clusters }`; `setText` can shift cached run offsets without changing the parent cluster offsets.
- Added a cluster-aware regression using `EditorState.update` and `setText` for `aaaa\nx👍🏽y👍🏽z`. It asserts runs identity, pre/post and parent/derived cell equality, retained `z`, and no additional `measureText` calls.
- Mutation evidence: focused test failed on the exact pre-fix `layout.ts` SHA-256 `f0a370b5c15aeddae53f680366a2ca959780624510a9d7f1d6a033919f9e8366` (8 run, 7 passed, 1 failed); it observed 32 px emoji cells and missing `z`. Log and structured record: `tmp/code-render/CR-01/attempt-7/mutationEvidence/f0a370b5-focused.log` and `tmp/code-render/CR-01/attempt-7/mutationEvidence.json`.
- SHA-256 before this correction: `layout.ts` = `f0a370b5c15aeddae53f680366a2ca959780624510a9d7f1d6a033919f9e8366`; `text-cells.test.ts` = `fcccb4f96de508c0abdca4291d6ac4f9172ffab070310fc14f2a7a2038230732`. After correction and test: `layout.ts` = `bf574defa2099bd89b014497dfc5cdd67edd71169194f2ac813e6c8699dbe61c`; `text-cells.test.ts` = `db4618440b367674e7015b50e37985bc9b8fbc9b9c90bbd75bd585850cec4992`.
- `cd editor && npx vitest run test/code/text-cells.test.ts`: exit 0, 8/8 passed; complete log `tmp/code-render/CR-01/attempt-7/focused.log`.
- `cd editor && npm run check`: exit 0; complete log `tmp/code-render/CR-01/attempt-7/check.log`.
- Under `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`, `cd editor && npx vitest run test/canvas`: exit 0, 223/223 passed; complete log `tmp/code-render/CR-01/attempt-7/canvas.log`. `editor/test/canvas/mount.test.ts:321` remains byte-identical and passes.
- `cd editor && npx vitest run test/code/renderer-layers.test.ts test/code/atlas-cells.test.ts test/code/glyph-placement.test.ts`: exit 0, 14/14 passed; complete log `tmp/code-render/CR-01/attempt-7/adjacent.log`.
- `layout.ts` remains under 1000 lines. `git diff --check` and the mount-test byte-identity check are recorded in `tmp/code-render/CR-01/attempt-7/final-audit.log`.

### Session: 2026-10-17 (current-source verification)
**Tasks Completed**: CR-01 assigned implementation and regression checks reconfirmed
**Tasks In Progress**: None in CR-01
**Blockers**: None
**Notes**:
- No implementation source changes were needed; the current source retains the run-relative ASCII start, derived-run metadata/additivity propagation, and cached-line-shift cluster rebasing.
- Current-source focused verification: `cd editor && npx vitest run test/code/text-cells.test.ts`; exit 0, 8/8 passed. Complete log: `tmp/code-render/CR-01/attempt-8/focused.log`.
- Current combined-tree check: `cd editor && npm run check`; exit 0. Complete log: `tmp/code-render/CR-01/attempt-8/check.log`.
- Informational CR-05 handoff verification: under `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`, `cd editor && npx vitest run test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts`; exit 0, 77/77 passed. Complete log: `tmp/code-render/CR-01/attempt-8/canvas.log`. No remaining GPU/edit-cost handoff from this run.
- Current-source checksums and the exact plan edit intent are recorded in `tmp/code-render/CR-01/attempt-8/pre-verification-sha256.txt` and `edit-intent-plan.txt`.

### Session: 2026-10-17 (CR-01 implementation handoff)
**Tasks Completed**: Current-source CR-01 gates and CR-05 informational handoff reconfirmed
**Tasks In Progress**: None in CR-01
**Blockers**: None
**Notes**:
- No source edits were needed; `layout.ts` remains at SHA-256 `bf574defa2099bd89b014497dfc5cdd67edd71169194f2ac813e6c8699dbe61c` and `text-cells.test.ts` remains at `db4618440b367674e7015b50e37985bc9b8fbc9b9c90bbd75bd585850cec4992`.
- Current-source focused verification: `cd editor && npx vitest run test/code/text-cells.test.ts`; exit 0, 8/8 passed. Complete log: `tmp/code-render/CR-01/attempt-9/focused.log`.
- Current combined-tree check: `cd editor && npm run check`; exit 0. Complete log: `tmp/code-render/CR-01/attempt-9/check.log`.
- Informational CR-05 handoff: under `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`, `cd editor && npx vitest run test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts`; exit 0, 77/77 passed. Complete log: `tmp/code-render/CR-01/attempt-9/canvas.log`; no handoff remains.
- Pre-verification identities and this exact plan-log intent are recorded in `tmp/code-render/CR-01/attempt-9/pre-verification-sha256.txt` and `edit-intent-plan.txt`.
