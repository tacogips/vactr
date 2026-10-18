# LPI-BEHAVIOR: diagnose and fix the four LP-EVIDENCE behavior checks

**Status**: Completed (2026-10-18; accepted by the final integration review, workflow session 441)
**Plan ID**: LPI-BEHAVIOR (wave 4)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 2 (I4), 5.1 (B1-B3, decision table), 6 (perf-hook and check part of I6), plus rules D-a/D-b/D-c; section 8 (depends on LPI-STYLE, revised session 438)
**Manifest**: `impl-plans/active/live-perf-int-dispatch.json`
**Branch**: `merge/live-perf` (shared working directory; no private branch)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Intent and Context

LP-EVIDENCE (session 349, commit 4ca8edc) added four checks to
`editor/test/e2e/behavior.mjs`:

- `momentary-right-drag`
- `momentary-two-finger`
- `stop-gentle-shortcut`
- `cut-shortcut`

It also added perf-hook accessors in `editor/src/code/perf-hook.ts` and
`editor/src/code/mount.ts`. The checks had never run. On the merged tree
they fail in Chromium and WebKit (44/52). The cause is not diagnosed.

- All four call `loadLiveFixture`, which fills the textarea with
  `d1 s :analog > note [:a4 :c5] > lpf 800 > room 0.6 > gain 0.1`, clicks
  `.vact-run` and waits for `momentaryHit` at the `800` literal. A shared
  failure point is therefore likely.
- Run normally starts audio. `editor/src/code/mount.ts:146` `onEval`
  calls `transport.audio.start()`, and the click is a user gesture.
  "Audio never started" is a candidate, not an established cause. Do not
  record it as the cause unless the diagnosis shows it.
- One check defect is established: the stop and cut checks hard-code
  `Meta+.` / `Meta+Shift+.`. That is correct only where `navigator.platform`
  matches `/Mac|iP/` (`editor/src/ui/stop-keys.ts` `isApplePlatform`).

The behavior required by design-live-performance.md 8.3:

- A right-drag on a bound site changes the overlay label, and the document
  text is unchanged.
- No context menu appears on the site.
- `Mod-.` leads to `TransportSample.output` reaching `draining` and then
  `idle`.
- `Mod-Shift-.` leads to `cutting` and then `idle`.
- LP-EVIDENCE TASK-V2 lists the exact steps.

## Non-goals

- Do not delete or weaken any of the four checks or their assertions
  (rule D-c). Do not change any existing behavior, Vim or keymap check.
- No threshold or workload change, no `--write-evidence` run, and no edit
  to the canvas `run-001` evidence.
- Do not touch `editor/test/e2e/examples.mjs`, `silent-sink.mjs`,
  `run.mjs`, `vim-behavior.mjs` or `vim-keymap.mjs`.
- No redesign of live-perf features. A product fix must restore the
  design-live-performance.md 4-6 behavior.

## Dependencies

- **dependsOn**: LPI-PLAYBACK and LPI-STYLE (and through them all wave-1
  plans). The e2e run needs the fixed product and the fixed right pane.
  This plan shares `editor/dist` and the measurement lock with both.
- Runs alone in wave 4.
- Do not touch `editor/src/bind/glide-setting.ts`, `editor/src/bind/bind.css`,
  `editor/test/bind/glide-setting.test.ts` or `editor/test/style/ui-style.mjs`;
  LPI-STYLE owns them. If the glide row's position or styling must change,
  stop and report a scope amendment.

## writePaths

Checks and hooks:

- `editor/test/e2e/behavior.mjs`
- `editor/src/code/perf-hook.ts`
- `editor/test/canvas/mount.test.ts` (only the perf-hook row added in
  4ca8edc, if an accessor changes)

Product candidates, per the decision table. Edit only what the diagnosis
implicates:

- `editor/src/bind/momentary.ts`
- `editor/src/bind/mount.ts`
- `editor/src/code/momentary-pointer.ts`
- `editor/src/code/surface.ts`
- `editor/src/code/transport.ts`
- `editor/src/code/mount.ts` (perf-hook wiring lines only; LPI-VIM-STOP's
  `stopAll`/`cut` wiring must stay as is)
- `src/session/momentary.rs`
- `src/sched/runtime/live.rs`

Their tests:

- `editor/test/bind/momentary.test.ts`
- `editor/test/canvas/momentary-pointer.test.ts`
- `editor/test/code/transport.test.ts`
- `src/session/tests/momentary.rs`
- `src/session/tests/live_stop.rs`

Plan file: `impl-plans/active/live-perf-int-behavior.md` (Progress Log
only).

Artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
`editor/node_modules`, `editor/dist`, `tmp/live-perf-int/behavior`, `tmp/canvas-cutover`, `tmp/code-render`, `tmp/canvas`, `tmp/examples-e2e`, `tmp/ui-style`.

## sharedPaths

None. Read-only: `editor/test/e2e/run.mjs`, `editor/test/e2e/examples.mjs`
(audio start pattern, lines 84-85), `editor/test/e2e/silent-sink.mjs`,
`impl-plans/active/live-perf-evidence.md` (TASK-V2 check specification).

## Tasks

### TASK-B1: Review the session-349 editor work (no edit unless defective)

Check and record in the Progress Log:

- the accessors `transportOutput`, `outputHistory`, `momentaryLabels`,
  `momentaryHit` and `coordsAtPos` exist only through `installPerfHook`
  with `?perf=1`;
- momentary label sampling in `mount.ts` is guarded by `perfEnabled`;
- the `mount.test.ts` perf-hook row asserts plain data;
- `dispose-ledger` runs after the four checks.

Keep all of it. Fix only a concrete defect, and record it.

### TASK-B2: Diagnosis run (no code change)

1. `mise run build-wasm-release` (repository root), then in `editor/`
   `VACTR_REQUIRE_SESSION_ABI=1 npm run build`.
2. Under the measurement lock, in `editor/`:
   `npm run e2e -- --browser all --profile behavior --run-id live-perf-int-diag --out ../tmp/live-perf-int/behavior/diag > ../tmp/live-perf-int/behavior/2-diag.log 2>&1`.
   Release the lock afterwards, also on failure.
   - Without `--write-evidence`, the runner writes **no** per-browser files.
     The result JSON is the last stdout line, and each check's `detail` is
     in `summary.browsers[].checks[]`.
3. For each failing check and browser, record in the Progress Log the
   failing step (from the stack in `detail`) and its classification under
   the decision table below.

### TASK-B3: Required check corrections (rule D-a), in `behavior.mjs`

- **B1.** In `loadLiveFixture`, after the Run click, read
  `.vact-audio-state` `data-state`. If it is not `running`, click
  `.vact-audio` and wait up to 10 s for `running` (examples.mjs:84-85
  pattern). This is an idempotent precondition. The context already has
  the silent sink.
- **B2.** Derive the modifier in the page:
  `/Mac|iP/.test(navigator.platform) ? 'Meta' : 'Control'`. Press
  `${mod}+.` and `${mod}+Shift+.`.
- **B3.** In each of the four checks, on failure, click `.vact-hush`
  (catch and ignore its own errors) before rethrowing, so later checks
  start from silence. On success:
  - `momentary-right-drag` and `momentary-two-finger` click `.vact-hush`
    at their end;
  - `stop-gentle-shortcut` and `cut-shortcut` already end idle.

### TASK-B4: Fix remaining failures by ownership (decision table)

| Failure point (audio running, eval `ok`) | Owner and action |
|---|---|
| `momentaryHit` never true | product: site table or Direct eligibility (design-live-performance.md 5.1). Look at `editor/src/bind/momentary.ts` `hit`, `editor/src/code/surface.ts` `momentaryHit` and `src/session/momentary.rs` eligibility. Fix with a unit test in the matching test file. |
| label, document or context-menu assertion | product if the behavior contradicts 5.2, 5.3 or 5.9 (a fix plus a unit test); otherwise the check is wrong, so fix the check to follow the TASK-V2 specification, never to loosen it |
| `outputHistory` lacks `draining` then `idle` (or `idle` after a cut) | product: session `TransportSample.output` (`src/sched/runtime/live.rs` `OutputTracker`) or editor `TransportBar.tick` (`editor/src/code/transport.ts`). Fix with a unit test (`src/session/tests/live_stop.rs` or `editor/test/code/transport.test.ts`). |
| timing waits | the check may use only the design constants: Shift snap within 10 frames, glide default 1000 ms (gone by 1500 ms), drain cap 15 s (20 s wait), cut idle within 3 s |

If a failure needs a file outside writePaths, stop and report it with the
diagnosis. Do not edit that file.

### TASK-B5: Final run

Rebuild (`mise run build-wasm-release`, then
`VACTR_REQUIRE_SESSION_ABI=1 npm run build`), then run the full behavior
e2e under the lock with
`--run-id live-perf-int-behavior --out ../tmp/live-perf-int/behavior/final`.

## Pitfalls

- Always rebuild the release wasm and the dist immediately before each e2e
  run. The runner serves a stale dist without warning.
- `page.keyboard.press` key names are `'.'`; the app matches
  `code === 'Period'`.
- Do not click `.vact-audio` in a context without the silent sink. Never
  change the system volume.
- Never run two Playwright or vitest suites at once. Hold the lock only
  while a suite runs.

## Verification (logs under `tmp/live-perf-int/behavior/`)

1. `node --check editor/test/e2e/behavior.mjs` -> exit 0.
2. In `editor/`, `npm run check` -> exit 0.
3. In `editor/`, `./node_modules/.bin/vitest run test/canvas/mount.test.ts test/bind test/canvas/momentary-pointer.test.ts test/code/transport.test.ts`
   -> exit 0, failures 0.
4. If Rust changed:
   - `CARGO_TERM_QUIET=true cargo fmt --check` and
     `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
     -> exit 0;
   - `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib session::tests sched::tests`
     -> exit 0, failures 0;
   - `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
     -> exit 0.
5. `mise run build-wasm-release` -> exit 0. In `editor/`,
   `VACTR_REQUIRE_SESSION_ABI=1 npm run build` -> exit 0.
6. Under the lock, in `editor/`:
   `npm run e2e -- --browser all --profile behavior --run-id live-perf-int-behavior --out ../tmp/live-perf-int/behavior/final`.
   - Exit 0. The stdout JSON has `pass: true` and `failureCount: 0`.
   - The four checks pass in both `chromium` and `webkit`, and all
     existing behavior, Vim and keymap checks pass.
   - Post-sink peak is 0 (the silent-sink assertions are unchanged).
   - Save the log as `6-behavior-final.log`.
7. The touched files' `wc -l` -> all under 1000.

## Completion Criteria

- [x] The B1 review is recorded.
- [x] The diagnosis log and per-check classification are in the Progress
  Log.
- [x] B1-B3 check corrections are applied.
- [x] Remaining failures are fixed by owner, each product fix with a unit
  test.
- [x] The final behavior e2e passes in Chromium and WebKit with
  `failureCount: 0`.
- [x] Verification 1-7 exit 0 (step 4 only if Rust changed).
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
- Verification record shape is exactly `{command, exitStatus: 0, testsRun,
  testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
  - Negative-control and mutation runs go only in `mutationEvidence`.
  - Repaired review findings go only in `addressedFeedback`, with status
    `repaired`.
- Edit only this plan's own Progress Log.

## Progress Log

### Session: 2026-10-18 (plan authored, workflow session 433 step 4)
**Tasks Completed**: none. Plan authored from the accepted design.
**Notes**: Ready for wave 3 after LPI-PLAYBACK.

### Session: 2026-10-18 (plan revised, workflow session 438 step 4)
**Tasks Completed**: none (plan revision only).
**Notes**: Now wave 4, depending on LPI-PLAYBACK and LPI-STYLE. The LPI-STYLE files (glide-setting.ts, bind.css, glide-setting.test.ts, ui-style.mjs) are off-limits. No task content changed.

### Session: 2026-10-18 (implementation, workflow Step 6)
**Tasks Completed**: TASK-B1 through TASK-B5.
**TASK-B1 review**: Kept the session-349 perf-hook work. The accessors remain installed only behind `perfEnabled && scheduler.perf` (`?perf=1`); momentary label snapshots remain guarded by `perfEnabled`; the `mount.test.ts` row verifies plain-data accessors and absence without the flag; `dispose-ledger` remains after all four live-perf checks. No concrete defect was found. Temporary diagnosis-only accessors in `perf-hook.ts` and their `mount.ts` wiring were removed; both files' final hashes match their pre-edit hashes below.
**Diagnosis and per-check classification**:
- Initial run `tmp/live-perf-int/behavior/diag/2-diag.log`: all four checks failed in both browsers at the shared `momentaryHit` wait (Chromium and WebKit, 8 failures). With the required audio-ready check and diagnostic capture, audio reached `running`, but the original fixture `d1 s :analog > note [...] ...` evaluated with `d1 takes 1 arguments, found 2`; its site was not bound. This was a fixture/check defect, not a product deviation.
- The standard equivalent sink pipeline `s :analog > note [:a4 :c5] > lpf 800 > room 0.6 > gain 0.1 > d1` made the site and momentary behaviors valid. Repeated `.fill()` calls then appended to the editor document; the detailed run recorded the concrete form failure ``d1s` is not defined` and no sink onset. This was a repeated-fixture input defect, not a product deviation. `loadLiveFixture` now focuses and selects all before filling, keeping each check on the same intended fixture.
- The final check classifies all four checks as passing in Chromium and WebKit: right-drag, synthetic two-finger, gentle `Mod-.`, and cut `Mod-Shift-.`. No product fix or product unit test was needed. Diagnosis artifacts are under `tmp/live-perf-int/behavior/diag/`.
**Corrections**: B1 verifies audio state after Run and starts audio only when it is not running. B2 derives Meta vs Control from `navigator.platform`. B3 hushes on failure, and successful momentary checks hush then await idle within the design's 3-second cut bound. Existing assertions and thresholds are retained. Added `silent-sink-post-peak` to assert peak 0 and zero violations in the behavior run.
**Final verification** (all logs are complete; exit=0):
- `node --check editor/test/e2e/behavior.mjs` — `tmp/live-perf-int/behavior/final/retry-2/1-node-check.log`.
- `cd editor && npm run check` — `tmp/live-perf-int/behavior/final/retry-2/2-check.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/mount.test.ts test/bind test/canvas/momentary-pointer.test.ts test/code/transport.test.ts` — 16 files, 133 tests passed; `tmp/live-perf-int/behavior/final/retry-2/3-vitest.log`.
- No Rust files changed; verification 4 was not applicable.
- `CARGO_TERM_QUIET=true mise run build-wasm-release` — `tmp/live-perf-int/behavior/final/retry-2/5-wasm-release.log`.
- `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` — `tmp/live-perf-int/behavior/final/retry-2/5-editor-build.log`.
- `cd editor && npm run e2e -- --browser all --profile behavior --run-id live-perf-int-behavior --out ../tmp/live-perf-int/behavior/final` — `pass=true`, 54/54 checks, failureCount 0; Chromium 29/29, WebKit 25/25; all four assigned checks and existing Vim/keymap checks passed, both post-sink checks reported peak 0 and zero violations; log `tmp/live-perf-int/behavior/final/retry-2/6-behavior-final.log`.
- Touched source line counts: behavior.mjs 346; perf-hook.ts 114; mount.ts 406; all under 1000. `perf-hook.ts` and `mount.ts` were temporarily used for diagnosis then restored exactly.
**SHA-256**:
- `editor/test/e2e/behavior.mjs`: pre `a657133d3fa1c517127a04ea5840a961ebfa411a003e248cd57f839572ee2614`; post `72f833896f090d24fc6c99c5b16ee58e35820777f614999a8c017219879d815f`.
- `editor/src/code/perf-hook.ts`: pre/post `ed4baf0195408d7f7ab1189d29b0a494012143cdaddaee41538a11ef0a6d7223`.
- `editor/src/code/mount.ts`: pre/post `008a3e32301b6ec6f110394d6d1ea2b83716eee0018b9d8bf3e6de2289afb8c9`.
- `impl-plans/active/live-perf-int-behavior.md`: pre `c6ae076beefea179fdcde02ddaff1b89dd5cf43d001b72c533a88d1f564d4f97`; post SHA-256 recorded in `tmp/live-perf-int/behavior/final/plan-post-hash.json`.
**Notes**: Independent test-integrity/adversarial and serial combined-tree reviews remain downstream workflow work. The physical iPad gesture check remains an accepted out-of-scope limitation.

### Session: 2026-10-18 (final integration review and post-review archive, workflow session 441 step 8)
**Tasks Completed**: Post-review archive steps 1-4 of `live-perf-int-closeout.md` for this plan.
**Final review decision**: Accepted with no findings. The final serial integration review of the combined tree (`integration-review-attempt-1-exec-6264`, comm-007718) accepted all seven LPI plans after each fanout branch passed test-integrity and adversarial review (fanout group run `shared-branch-implementation:dispatch-plans-attempt-7-exec-6256`). Closeout gates 1-13 passed on the combined tree (`tmp/live-perf-int/closeout/`: behavior e2e 54/54, e2e:examples 8/8, nextest 3093/3093, vitest 2384/2384, test:style 16/16, Tauri cargo test 17/17).
**Archive**: Status set to Completed; this plan was moved from `impl-plans/active/` to `impl-plans/completed/` together with the other LPI plans and `live-perf-int-dispatch.json`. Physical iPad two-finger validation remains an accepted out-of-scope limitation.
