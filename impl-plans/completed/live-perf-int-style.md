# LPI-STYLE: right-pane style regression UF-1 (glide row and gap-check clipping)

**Status**: Completed (2026-10-18; accepted by the final integration review, workflow session 441)
**Plan ID**: LPI-STYLE (wave 3; dependsOn edge on LPI-PLAYBACK for editor/dist and lock serialization)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 2 (I8), 8 (plan graph, artifact roots), 9 (risks) and 10 (S0, S1, S2, forbidden fixes, acceptance)
**Manifest**: `impl-plans/active/live-perf-int-dispatch.json`
**Branch**: `merge/live-perf` (shared working directory; no private branch)
**Created**: 2026-10-18 (workflow session 438)
**Last Updated**: 2026-10-18

## Intent and Context

`npm run test:style` passes on main (8866a43). It fails on `wf/live-perf`
(4ca8edc) and on the merged tree. The report is
`tmp/ui-style/after/report.json`, with baselines
`tmp/live-perf-int/reconcile/7-main-style.log` (exit 0) and
`9-wf-live-perf-style.log` (exit 1). There are eight `gap` findings:

- `button -> input`, gap -563.6, at 1440x900 and 1440x900-vim;
- `button -> button`, gap -459.6, at 1180x820-coarse and
  1180x820-coarse-vim;
- both in Chromium and in WebKit.

What is already established (design I8):

- The reconcile probe (`tmp/live-perf-int/reconcile/3-gap-probe.log`,
  Chromium 1440x900) pairs the visuals `.vact-section-toggle` (rect y
  385-413, x 864-1440, full width; parent `.pane-section-header`) with the
  `.pkg-proxy` URL input (y 382-410).
- `.pane-right` is `overflow: auto` (`editor/src/app/app.css:352-355`).
  The package area is appended inside it (`editor/src/pkg/mount.ts:72`).
- LP-EDITOR-MOMENTARY prepends the `.vact-glide-setting` row to
  `.pane-right` (`editor/src/bind/mount.ts:168`,
  `editor/src/bind/glide-setting.ts:20`). That pushes the proxy input
  below the pane's visible bottom, where it is scrolled out of view.
- The gap check's `isVisible` (`editor/test/style/ui-style.mjs:42-50`)
  ignores scroll clipping.
- Separate defect: the glide row has no horizontal inset
  (`editor/src/bind/bind.css:154`, `padding: var(--vt-space-2) 0`). Its
  sibling rows use `var(--vt-space-3)` (`bind.css:7`), and
  `tmp/ui-style/after/chromium-1440x900@2x.png` shows the row flush with
  the pane edges.

Not yet established: whether the 1180x820-coarse pair, and the WebKit
geometry, have the same mechanism. S0 decides.

The glide setting itself is accepted behavior: design-live-performance.md
D6 and line 401, localStorage key `vactr.momentary.glideMs`. So is its
position as the first right-pane row.

## Non-goals

- Do not move, hide or reorder the glide row, and do not change its DOM
  contract (title span, number input, range input, the
  `.vact-glide-setting` class, the clamp/round/persist behavior tested in
  `editor/test/bind/glide-setting.test.ts`).
- **Forbidden fixes** (design section 10):
  - resizing or reshaping the glide row only so the clipped proxy input
    leaves the header band (a pixel-coincidence fix);
  - changing viewports, engines, gap minimums (4 and 8) or any other
    threshold;
  - removing or skipping the gap check, the proxy checks or any
    per-control check;
  - changing `isVisible` itself.
- Do not touch `editor/src/bind/mount.ts` (LPI-BEHAVIOR's writePath),
  `editor/src/app/app.css`, `editor/src/pkg/*`, `editor/src/ui/shell.tsx`
  or any e2e file.
- No Rust change.

## Dependencies

- **dependsOn**: LPI-WIRE, LPI-VIM-STOP, LPI-NS-INDEX, all accepted after
  their session-438 review-and-repair reruns. The editor build must
  contain the reviewed wave-1 editor code.
- **dependsOn LPI-PLAYBACK** as a serialization edge only. Source
  writePaths are disjoint, and this plan uses none of LPI-PLAYBACK's
  content. But both plans rebuild `editor/dist`, write the e2e output
  roots and take the measurement lock, and native fanout starts every
  ready plan at once. Do not read LPI-PLAYBACK's results as input, and do
  not edit its files.
- LPI-BEHAVIOR and LPI-CLOSEOUT depend on this plan.

## writePaths

Source:

- `editor/src/bind/bind.css` (S1)
- `editor/src/bind/glide-setting.ts` (only if S1 needs a class or
  structure change; expected unchanged)
- `editor/test/bind/glide-setting.test.ts` (only if `glide-setting.ts`
  changes; expected unchanged)
- `editor/test/style/ui-style.mjs` (S2 only, and only on S0
  classification (a))
- `impl-plans/active/live-perf-int-style.md` (this plan; Progress Log and
  status only)

Artifact roots (identical in the manifest):

- `target`
- `tree-sitter-vact/tree-sitter-vact.wasm`
- `editor/node_modules`
- `editor/dist`
- `tmp/live-perf-int/style` (probe script, probe output and logs)
- `tmp/canvas-cutover`
- `tmp/code-render`
- `tmp/canvas`
- `tmp/examples-e2e`
- `tmp/ui-style` (test:style writes `tmp/ui-style/after/`)

## sharedPaths

None. Read-only: `editor/src/app/app.css`, `editor/src/app/theme.css`
(`--vt-space-2` is 8px, `--vt-space-3` is 12px), `editor/src/bind/mount.ts`,
`editor/src/pkg/mount.ts`, `editor/src/ui/shell.tsx`,
`tmp/live-perf-int/reconcile/probe/gap-probe.mjs` (the probe to imitate).

## Tasks

### TASK-S0a: Baseline diagnosis probe (no source change)

1. Fresh build, from the repository root:
   - `mise run build-wasm-release` (`tmp/live-perf-int/style/0-wasm-release.log`)
   - `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`
     (`0-build.log`)
2. Write `tmp/live-perf-int/style/probe/style-probe.mjs`. It is an
   artifact, not committed. Imitate
   `tmp/live-perf-int/reconcile/probe/gap-probe.mjs` (repository root as
   `argv[2]`, Playwright imported from `editor/node_modules`,
   `startServer` from `editor/test/e2e/serve.mjs`). Reproduce the
   `ui-style.mjs` page setup exactly:
   - viewports `1440x900` and `1180x820-coarse` (`hasTouch`, `isMobile`,
     with the same fallback as ui-style.mjs);
   - `deviceScaleFactor: 2`;
   - wait for `.vact-transport` and `.pane-side`, `document.fonts.ready`,
     then 1000 ms;
   - force the coarse CSS where ui-style.mjs forces it;
   - Chromium with `--mute-audio`, and WebKit.
3. For every control pair that the ui-style gap loop flags
   (`ui-style.mjs:111-122` logic, gap < minimum), record as JSON:
   - both controls' tag, class, text and parent class, and rect;
   - for each control, every ancestor whose computed `overflow-x` or
     `overflow-y` is not `visible`, with that ancestor's rect,
     `clientHeight`, `scrollHeight` and `scrollTop`;
   - whether the control or any ancestor has computed `position` `fixed`
     or `absolute`;
   - the control's painted fraction: its rect intersected with every
     clipping ancestor's rect, as a fraction of its own height and of its
     own width.
4. Classify each pair:
   - **(a)** One control's painted fraction is below 0.5 in height or
     width (scrolled out of view).
   - **(b)** Both controls are painted (at least 0.5 in both
     dimensions), which is a real overlap.
5. Run it under the measurement lock (LOCK, defined below) for both
   engines. Write `tmp/live-perf-int/style/probe/s0a-<engine>.json` and
   the log `s0a-probe.log`.
6. Record in the Progress Log the classification for every pair, engine
   and viewport. Label anything not measured as unconfirmed.

### TASK-S1: Glide row inset (`editor/src/bind/bind.css`)

- Change `.vact-glide-setting` `padding` from `var(--vt-space-2) 0` to
  `var(--vt-space-2) var(--vt-space-3)`. This matches
  `.bind-panel, .bind-controls` (`bind.css:4-9`).
- Keep `display: grid`, the column template, `gap: var(--vt-space-2)`
  (8px, which equals the test:style minimum), the square, flat input
  rules (`border-radius: 0`, solid border, `--vt-surface` fill) and the
  coarse-pointer `min-height: 36px`.
- `box-sizing`: the label is a grid block in the pane. Check in the S0b
  screenshot that the range does not overflow the pane horizontally.
- Do not change vertical padding, font or row height. That would be the
  forbidden coincidence lever.

**Pitfall.** Do not check whether S1 alone clears the gap finding and
then tune it until it does. S1 is a spacing fix, judged only by the inset
rule above.

### TASK-S0b: Decisive probe after S1

Rebuild (`mise run build-wasm-release`, then
`VACTR_REQUIRE_SESSION_ABI=1 npm run build`, logs `1-wasm-release.log`
and `1-build.log`). Rerun the probe under LOCK, writing
`probe/s0b-<engine>.json` and `s0b-probe.log`. Then run `npm run test:style`
under LOCK (`2-style-after-s1.log`); it may still fail.

- If test:style already passes: S2 is not applied. Record that and go to
  TASK-S3.
- If every remaining failing pair is classification (a): do TASK-S2.
- If any remaining pair is classification (b): do not apply S2 to that
  pair. Fix the product layout inside `bind.css` or `glide-setting.ts`,
  without the forbidden levers. If the fix needs any other file, stop,
  set Status `Blocked (scope amendment)`, and report the pair, its
  evidence and the file needed.

### TASK-S2: Painted-visibility rule for gap pairing (`editor/test/style/ui-style.mjs`; only on S0b classification (a))

Change only the generic gap-pairing loop (`ui-style.mjs:111-122`):

- Add one in-page helper, for example `paintedForGap(element)`. It
  returns false only when **all** of these hold:
  - neither the element nor any ancestor has computed `position`
    `fixed` or `absolute`. If one does, the helper always returns true,
    so the element stays checked. This conservative guard keeps the doc
    picker and other positioned popovers checked, because their painting
    escapes ancestor clipping;
  - the element's rect, intersected with the rect of every ancestor whose
    computed `overflow-x` or `overflow-y` is not `visible`, keeps less
    than half of the element's height or less than half of its width.
- Apply it as an extra filter on the gap candidates: keep a control when
  `inProbe || paintedForGap(element)`. Apply it both to the loop's
  `first` and to its `second` candidates (filter `gapControls` once,
  after the existing overlay filtering).
- `[data-style-probe]` controls (`inProbe === true`) are never excluded.
- Do not change: `isVisible`, `controls` (the per-control checks still
  see every control), the explicit `gapCheck(...)` calls (song-apply,
  sample map, bank), the `firstInProbe !== secondInProbe`, overlay and
  doc-picker pairing rules, the minimums 4/8, viewports, engines,
  screenshots or `report.json` format.
- Add a short comment that names the reason: a control scrolled out of
  its overflow container is not painted where it is measured (UF-1).
- Record the intentional test change in the Progress Log. Reason: the
  accepted glide row (D6) makes the right pane overflow at the tested
  viewports, which main's content did not.

**Pitfalls:**

- Do not compare containers. The design forbids "same clipping container"
  pairing (review DR-1). Painted pairs in different containers must stay
  checked.
- Do not use `elementFromPoint` or hit-testing. With two overlapping
  painted controls, it would hide the lower one and lose exactly the
  overlap the check exists to catch.
- Keep the file under 1000 lines (it is 340 now).

### TASK-S3: Final verification and mutation evidence

See Verification. Mutation cases M1-M3 apply only if S2 was applied.

## Test cases (situation -> expected outcome)

- Merged tree after S1 (+S2 if applied), `npm run test:style`, Chromium
  and WebKit, 1440x900 and 1180x820-coarse, with and without Vim, keymap
  and hints -> exit 0, `Style assertions passed for 16 engine/viewport combinations.`
- The `.pkg-proxy` URL input is still checked for presence, visibility,
  radius, fill, border and coarse height (`ui-style.mjs:140-154`)
  -> no `proxy-*` failure.
- M1: S2 reverted (only if S2 was applied) -> test:style exits 1 with the
  UF-1 `gap` findings.
- M2: S2 applied, with a temporary negative margin on a
  `[data-style-probe]` button so it overlaps its neighbor -> a `gap`
  failure for that probe pair.
- M3: S2 applied, with a temporary style injected after page load (for
  example `page.addStyleTag` with
  `.pane-section-header[data-section="visual"] { margin-top: -40px; position: relative; z-index: 1 }`;
  adjust the offset as needed) so the visuals toggle is painted over a
  control that is painted at least half inside `.pane-right` -> a `gap`
  failure for that cross-container pair. Record the pair and both painted
  fractions.
- `editor/test/bind/glide-setting.test.ts` -> unchanged and passing.

## Invariants

- The glide row stays first in `.pane-right` with an unchanged DOM
  contract and unchanged storage behavior.
- The UI stays solid, flat, square and spaced: no radius, no native look.
- Every control is still inspected by every per-control style check.
- Painted controls are gap-paired whatever their clipping container.
- Touched files stay under 1000 lines.

## Verification (every gating command exits 0; logs under `tmp/live-perf-int/style/`)

**LOCK** means: `until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done`,
then run, then `rmdir` the lock. Always release it (use a `trap`).

1. `cd editor && npm run check` -> zero errors (`v1-check.log`).
2. `cd editor && ./node_modules/.bin/vitest run test/bind` -> exit 0,
   testsRun > 0, failures 0 (`v2-vitest-bind.log`).
3. If S2 was applied: `node --check editor/test/style/ui-style.mjs`
   (`v3-node-check.log`).
4. `mise run build-wasm-release` (`v4-wasm-release.log`).
5. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` (`v5-build.log`).
6. LOCK: `cd editor && npm run test:style` -> exit 0, stdout
   `Style assertions passed for 16 engine/viewport combinations.`
   (`v6-style.log`). Record testsRun 16, testsPassed 16. Keep
   `tmp/ui-style/after/report.json` with `failures: []`, and keep the
   1440x900 screenshot that shows the inset glide row.
7. `wc -l editor/src/bind/bind.css editor/src/bind/glide-setting.ts editor/test/style/ui-style.mjs editor/test/bind/glide-setting.test.ts`
   -> all under 1000 (`v7-line-counts.log`).
8. `git diff --stat` -> only this plan's source writePaths changed
   (besides other plans' already-committed work) (`v8-diff-stat.log`).

Mutation evidence (only in `mutationEvidence`, each under LOCK, each
restored byte-for-byte with matching sha256): M1, M2 and M3 from the test
cases, logs `m1-style.log`, `m2-style.log`, `m3-style.log`. Rerun
verification step 6 green afterwards (`m-restore-style.log`).

## Completion Criteria

- [x] S0a and S0b probe JSON files and per-pair classification are
  recorded in the Progress Log.
- [x] S1 is applied: `.vact-glide-setting` padding is
  `var(--vt-space-2) var(--vt-space-3)`.
- [x] S2 is applied only if S0b found classification (a), exactly as
  specified, with the reason recorded; or it is recorded as not applied.
- [x] Verification 1-8 exit 0. test:style passes all 16 combinations.
- [x] M1-M3 fail as expected (when S2 is applied); restore hashes match.
- [x] No forbidden fix was used; touched files are under 1000 lines.
- [x] The Progress Log records pre/post sha256 for every edited file.

## Execution Rules (all plans in this manifest)

- Read each file fresh immediately before editing it. Record
  `shasum -a 256 <file>` before and after the edit in the Progress Log.
- Edit only writePaths. Report drift in any other file; never repair it.
- Run commands in the foreground only. Write each to its log with
  `cmd > log 2>&1; echo "exit=$?" >> log`.
- Do not commit, stash, branch or run any mutating git command. Read-only
  `git diff`, `git status` and `git show` are allowed.
- Use English only, with no emojis. Keep every touched source file under
  1000 lines.
- Never change the Mac system volume.
- Verification record shape is exactly `{command, exitStatus: 0, testsRun,
  testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
  - Negative-control and mutation runs go only in `mutationEvidence`.
  - Repaired review findings go only in `addressedFeedback`, with status
    `repaired`.
- Edit only this plan's own Progress Log.

## Progress Log

### Session: 2026-10-18 (plan authored, workflow session 438 step 4)
**Tasks Completed**: none. Plan authored from the accepted design (section 10, with the DR-1 repair).
**Notes**: Ready for wave 3. It runs after LPI-PLAYBACK (serialization edge), once all wave-1 plans are accepted.

### Session: 2026-10-18 (Step 6, LPI-STYLE implementation rerun)
**Tasks Completed**: TASK-S0a, TASK-S1, TASK-S0b, TASK-S2, TASK-S3.
**S0a classification (before S1)**: Probe JSON is in `tmp/live-perf-int/style/probe/s0a-chromium.json` and `s0a-webkit.json`; the full console log is `s0a-probe.log`. For Chromium and WebKit, at 1440x900 with Vim off/on, `.vact-section-toggle` -> `.pkg-proxy input` has gap -563.59 (minimum 8), toggle painted fraction 1.00 x 1.00, input 0.106 x 1.00: classification (a). At 1180x820-coarse with Vim off/on, `.vact-section-toggle` -> `.params-tab` has gap -459.59 (minimum 8), toggle 1.00 x 1.00, params-tab 0.00 x 1.00: classification (a). No failing pair was classification (b). Other raw `.params-tabs` pairs have gap 4 and their unchanged minimum is 4, so they are not findings.
**S1**: Changed only `editor/src/bind/bind.css` padding from `var(--vt-space-2) 0` to `var(--vt-space-2) var(--vt-space-3)`. The S1 screenshot shows the label and range inset; the range stays within the pane. No vertical padding, grid, gap, control shape, coarse height, class or DOM contract changed.
**S0b classification (after S1)**: `tmp/live-perf-int/style/probe/s0b-chromium.json`, `s0b-webkit.json`, and `s0b-probe.log` record the same per-engine, per-viewport, per-Vim classifications above: 1440x900 toggle -> proxy input (a), 1180x820-coarse toggle -> params-tab (a), with the same painted fractions. `2-style-after-s1.log` records the eight UF-1 failures, all these pairs classified (a), so S2 applied. S1 alone did not remove the findings.
**S2**: Added `paintedForGap` only to generic gap candidate selection in `editor/test/style/ui-style.mjs`. It intersects each candidate with every overflow ancestor, retains fixed/absolute positioned elements and ancestors, and filters as `inProbe || paintedForGap(element)`. No container identity, `isVisible`, explicit `gapCheck`, threshold, overlay/picker rule, or per-control check changed.
**Mutation evidence (all temporary edits restored)**:
- M1: `m1-style.log` exits 1 and restores the eight UF-1 findings after the S2 candidate filter is removed.
- M2: `m2-style.log` exits 1 with probe `gap: -12` findings at eight engine/viewport states when a temporary -20px margin is applied to the enabled `[data-style-probe]` button.
- M3: `m3-style-cross-container-visible.log` exits 1 on `button -> input` at 1180x820-coarse in Chromium and WebKit, gap -336.52. `m3-cross-container-fractions.json` records the `visuals` `.vact-section-toggle` (painted fraction 1.00 x 1.00, no clipping ancestor) overlapping a painted `.pane-right` input (1.00 x 1.00, clipped by `.pane-right`): these cross-container painted controls remain checked. Earlier M3 attempts did not meet this exact cross-container condition; they are not counted as M3 evidence. The final mutation edit was restored.
- S2 source hash after M1/M2/M3 restoration is `5775d7577ea88060649414d0f5fcb60da953de2ca0027bb200d650821314eec0`, matching the pre-mutation S2 hash; S2 file was restored after each control.
**Verification (final source)**: `tmp/live-perf-int/style/final/v1-check.log` (`cd editor && npm run check`, exit 0, zero TypeScript errors); `v2-vitest-bind.log` (`cd editor && ./node_modules/.bin/vitest run test/bind`, exit 0, 90/90); `v3-node-check.log` (`node --check editor/test/style/ui-style.mjs`, exit 0); `v4-wasm-release.log` (`mise run build-wasm-release`, exit 0); `v5-build.log` (`cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`, exit 0); `v6-style.log` (`cd editor && npm run test:style`, exit 0, `Style assertions passed for 16 engine/viewport combinations.`); `tmp/ui-style/after/report.json` has `failures: []`; `v7-line-counts.log` records 208/22/363/26 lines; `v8-diff-stat.log` shows only the two intended source files before this plan update. The measurement lock was released.
**Pre/post SHA-256**: `editor/src/bind/bind.css` pre `deda81e59dad03c9db3e855a2a81e499a0bd02f73647627da91b8971b8651748`; post `5ce07bbb63a7ec8b00951d570e954c5cb9df77419f4ee7ef848a15e188eb7718`. `editor/test/style/ui-style.mjs` pre-S2 `7d083cad6adfcf6abd466bcdecd09f850a3e4669267384fbddec6758f2a9a1fb`; post `5775d7577ea88060649414d0f5fcb60da953de2ca0027bb200d650821314eec0`. `editor/src/bind/glide-setting.ts` and `editor/test/bind/glide-setting.test.ts` were not edited; their hashes remain `af00ecb1470b5aefc5d3190115f09fe10dd17449978b97a3b5220d335d0a02fb` and `4c3a6662a85ef54398b47bac5b29981a424ab7962592c34c282d5983bbae0f06`. This plan file pre-update hash was `5c07f7f57ea22ece3b1d4d9a2fca97ed4ebfe7a4424aeeb861f233c1cd5fd301`.
**Review handoff**: Implementation-phase acceptance is complete. Independent test-integrity/adversarial and combined-tree review remain downstream; no commit or staging performed.

### Session: 2026-10-18 (final integration review and post-review archive, workflow session 441 step 8)
**Tasks Completed**: Post-review archive steps 1-4 of `live-perf-int-closeout.md` for this plan.
**Final review decision**: Accepted with no findings. The final serial integration review of the combined tree (`integration-review-attempt-1-exec-6264`, comm-007718) accepted all seven LPI plans after each fanout branch passed test-integrity and adversarial review (fanout group run `shared-branch-implementation:dispatch-plans-attempt-7-exec-6256`). Closeout gates 1-13 passed on the combined tree (`tmp/live-perf-int/closeout/`: behavior e2e 54/54, e2e:examples 8/8, nextest 3093/3093, vitest 2384/2384, test:style 16/16, Tauri cargo test 17/17).
**Archive**: Status set to Completed; this plan was moved from `impl-plans/active/` to `impl-plans/completed/` together with the other LPI plans and `live-perf-int-dispatch.json`. Physical iPad two-finger validation remains an accepted out-of-scope limitation.
