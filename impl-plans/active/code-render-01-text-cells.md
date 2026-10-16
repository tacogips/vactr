# Code Render 01: Run-Relative Text Cells (RC2)

**Status**: Ready
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
| `editor/src/code/layout.ts` | In `textCells`, ASCII branch only: convert the `offsetInRun` result to a run offset by subtracting `run.from` before clamping to 0. Nothing else in this file changes. |
| `editor/test/code/text-cells.test.ts` | New unit test file (U1). |
| `impl-plans/active/code-render-01-text-cells.md` | Progress log only. |

Imitate `editor/test/canvas/gpu.test.ts:layout` (lines 10-18) for the
fake metrics: a `TextLayout` built with `{ font: '', measureText: t => ({
width: ... }) }`, and a `LayoutFont` such as `{ font: '13px test',
lineHeight: 18, baseline: 14 }`.

## Key Points and Pitfalls

- The fix is one expression. Do not rewrite the loop, the non-ASCII
  `segmenter` branch (already run-relative via `part.index`), or the
  `pieces.map` that computes `x` and `width`.
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

- [ ] `textCells` ASCII branch uses a run-relative start; no other source change
- [ ] `editor/test/code/text-cells.test.ts` has the 6 cases and passes
- [ ] `npm run check` passes (or only cross-plan errors are recorded)
- [ ] gpu/edit-cost handoff notes recorded (or "none")
- [ ] Progress log updated with verification records

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan created
**Tasks In Progress**: None
**Blockers**: None
**Notes**: Ready for wave 1.
