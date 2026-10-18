# LPI-VIM-STOP: stop and cut shortcuts with Vim on, and the removed hush option

**Status**: Implemented (b327141); awaiting independent correctness, test-integrity and adversarial review
**Plan ID**: LPI-VIM-STOP (wave 1)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 2 (I2, I3, I9), 4 (V1-V3, Tests) and 11 (R1-R6)
**Manifest**: `impl-plans/active/live-perf-int-dispatch.json`
**Branch**: `merge/live-perf` (shared working directory; no private branch)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Intent and Context

Live-perf (design-live-performance.md D2, D10, section 6) set up the stop
shortcuts like this:

- `Mod-.` sends gentle `stop-all`.
- `Mod-Shift-.` sends `hush` (cut).
- One platform-strict window keydown handler in the capture phase owns
  them (`editor/src/code/transport.ts:198-215`). It calls
  `preventDefault` and `stopPropagation`.
- It replaced `KeyboardOptions.hush` with `stopAll`/`cut`
  (`editor/src/code/keyboard.ts:76-77`).

Main's Vim mode (design-vim-mode.md 3.3.1 P2, 4.14) relies on a
KeyboardController fallback that accepts `metaKey || ctrlKey`, so
`Ctrl-.` on Apple and `Meta-.` on Linux reach the stop action while Vim is
on. The merge broke this in three ways:

1. `editor/src/code/mount.ts:179` still passes `hush`, so `npm run check`
   reports 3 errors, together with `test/canvas/input-vim-hook.test.ts:20`
   and `test/canvas/vim-input.test.ts:23`.
2. `KeyboardController.handle` now uses the platform-strict
   `stopShortcut(event, isApplePlatform())`. The alternate modifier
   therefore matches nothing, and the two `vim-mount.test.ts:111` cases
   get 0 messages.
3. Vim's P2 pass-through (`editor/src/code/vim/keys.ts:31`) lists key
   `'.'` but not `'>'`. With the alternate modifier, `Mod-Shift-.` (US
   layout key `'>'`) never reaches the fallback.

## Non-goals

- No change to the app-level handler in `transport.ts` or to
  `stopShortcut` semantics. They stay platform-strict, and
  `test/ui/stop-keys.test.ts`'s existing cases stay unchanged.
- No change to Vim behavior other than the one pass-through condition. The
  following still call `EvalController.hush()` (cut), unchanged:
  - `:hush` (`editor/src/code/vim/ex.ts`);
  - `vactr.hush` (`editor/src/code/vim/controller.ts:271`);
  - `editor/src/code/vim-mount.ts:78`.
- With Vim off, the alternate modifier must still do nothing, as on main.
- Do not re-add a `Mod-.` keymap binding to `EvalController`
  (`editor/src/code/eval.ts`).
- Do not touch `editor/test/e2e/*`, `editor/src/code/perf-hook.ts` or any
  Rust file.

## Dependencies

- **dependsOn**: none (wave 1). Runs alongside LPI-WIRE (Rust) and
  LPI-NS-INDEX (Rust).
- Do not run the full vitest suite, `npm run build` or e2e in this plan.
  LPI-WIRE may be rebuilding `target/` wasm at the same time, and the full
  suite runs in LPI-CLOSEOUT. The focused vitest files below do not load
  wasm.

## writePaths

- `editor/src/ui/stop-keys.ts`
- `editor/src/code/keyboard.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/vim/keys.ts`
- `editor/test/ui/stop-keys.test.ts`
- `editor/test/code/vim/keys.test.ts`
- `editor/test/canvas/vim-mount.test.ts`
- `editor/test/canvas/vim-input.test.ts`
- `editor/test/canvas/input-vim-hook.test.ts`
- `impl-plans/active/live-perf-int-vim-stop.md` (this plan; Progress Log
  only)
- Artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules`, `tmp/live-perf-int/vim-stop`

## sharedPaths

None. Read-only: `editor/src/code/transport.ts`, `editor/src/code/input.ts`,
`editor/src/code/eval.ts`, `editor/src/code/vim-mount.ts`,
`editor/test/canvas/input.test.ts`.

## Tasks

### TASK-V1: Modifier-tolerant fallback matcher (`editor/src/ui/stop-keys.ts`, `editor/src/code/keyboard.ts`)

- Add an exported function to `stop-keys.ts`:
  `export function fallbackStopShortcut(event: StopKeyEvent): StopShortcut | null`.
  - It returns `'cut'` or `'stop-all'` when `(metaKey || ctrlKey) &&
    !altKey`, and the key is a period. The period rule is exactly the one
    `stopShortcut` already uses: `code === 'Period'`, or `code === ''`
    with `key === '.'`, or `key === '>'` with Shift.
  - Shift selects `'cut'`; no Shift selects `'stop-all'`.
  - Factor the period test into one private helper shared by both
    functions, so the two cannot drift apart. `stopShortcut`'s behavior
    must not change.
- In `KeyboardController.handle` (`keyboard.ts:88`), replace
  `stopShortcut(event, isApplePlatform())` with
  `fallbackStopShortcut(event)`. Drop the `isApplePlatform` import if it
  becomes unused.

**Pitfalls:**

- Keep the dispatch order in `handle`: the `Mod-Enter` check stays first.
- `event.code` can be `undefined` in some synthetic events, so treat a
  missing code like `''`.
- Do not make `stopShortcut` tolerant. The window handler must stay strict.
  Otherwise a platform-Mod press and an alternate press would both be
  consumed globally.

### TASK-V2: Mount wiring (`editor/src/code/mount.ts`)

- In the `new InputController(surface, inputContainer, {...})` options
  (line about 179), replace
  `hush: () => { if (vim.enabled()) evalCtl.hush(); },` with:
  - `stopAll: () => { if (vim.enabled()) transport.stopAll(); },`
  - `cut: () => { if (vim.enabled()) transport.hush(); },`
- `transport` is the `TransportBar` constructed at line about 141. Both
  methods send exactly one message and call `onHush` (`highlight.clear()`).

**Pitfalls:**

- Keep the `vim.enabled()` gate. Without it, an alternate-modifier press
  with Vim off would start sending messages, a behavior change from main.
- Do not call `deps.client` directly. That would skip the highlight clear.

### TASK-V3: Vim pass-through for the Shift form (`editor/src/code/vim/keys.ts`)

- In `isPassThrough`, also return true when `mod && input.code ===
  'Period'`. `mod` is the existing `input.metaKey || input.ctrlKey`.
- Nothing else in the pass-through set changes.

### TASK-V4: Tests

**Mechanical updates:**

- `test/canvas/input-vim-hook.test.ts:19-20, 26` and
  `test/canvas/vim-input.test.ts:22-23, 42`: in the `InputController`
  options, replace the `hush` spy with `stopAll` and `cut` spies, and
  rename the returned fields to match. No existing assertion uses the
  removed option. Record the change in the Progress Log as mechanical
  (caused by the live-perf option rename).

**Intentional change** in `test/canvas/vim-mount.test.ts:111-124`. Reason:
accepted design D2/D10 maps the unshifted period to gentle stop; see
design-live-perf-integration.md section 4. Keep the two platform cases
(`['Linux x86_64', 'metaKey']`, `['MacIntel', 'ctrlKey']`) and the two
Enter presses, then:

- `key(rig, '.', { [modKey]: true })` -> exactly 1 `stop-all` and 0
  `hush`;
- then
  `key(rig, '>', { [modKey]: true, shiftKey: true, code: 'Period' })` ->
  exactly 1 `hush`, and still exactly 1 `stop-all`;
- `eval` stays at 2.

Record the reason in the Progress Log.

**New test cases (situation -> expected outcome):**

- `test/ui/stop-keys.test.ts`, describe `fallbackStopShortcut`:
  - `{code:'Period', key:'.', ctrlKey:true}` -> `'stop-all'`;
  - `{code:'Period', key:'.', metaKey:true}` -> `'stop-all'`;
  - `{code:'Period', key:'>', metaKey:true, shiftKey:true}` -> `'cut'`;
  - `{code:'', key:'>', ctrlKey:true, shiftKey:true}` -> `'cut'`;
  - `{code:'Period', key:'.', ctrlKey:true, altKey:true}` -> `null`;
  - `{code:'Period', key:'.'}` with no modifier -> `null`;
  - `{code:'KeyA', key:'.', metaKey:true}` -> `null`.

  The existing `stopShortcut` cases stay unchanged.
- `test/code/vim/keys.test.ts`: in the `isPassThrough` passthrough list,
  add `input('>', { ctrlKey: true, shiftKey: true, code: 'Period' })` and
  `input('>', { metaKey: true, shiftKey: true, code: 'Period' })`. Add one
  negative case to the non-pass list, `input('>', { shiftKey: true, code:
  'Period' })` (no modifier), which stays not passed.
- `test/canvas/vim-mount.test.ts`, one new case: with Vim **off**, on
  `Linux x86_64`, `key(rig, '.', { metaKey: true })` -> 0 `stop-all` and
  0 `hush` (parity with main's gate).

`test/canvas/input.test.ts:262-272` (platform Meta on MacIntel reaches
`stopAll`/`cut` once each) must keep passing unchanged.

## Invariants

- A platform-Mod press sends exactly one message whether Vim is on or off,
  because the capture-phase window handler consumes it.
- An alternate-modifier press sends exactly one message only with Vim on.
- `stopShortcut` results are unchanged for every existing test input.
- Vim `:hush` and `vactr.hush` still send `hush`.

## Verification (run in `editor/`; every command exits 0; logs under `tmp/live-perf-int/vim-stop/`, written as `../tmp/live-perf-int/vim-stop/<n>.log`)

1. `npm run check` -> exit 0 with zero errors (`1-check.log`).
2. `./node_modules/.bin/vitest run test/ui/stop-keys.test.ts test/code/vim/keys.test.ts test/canvas/vim-mount.test.ts test/canvas/vim-input.test.ts test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/code/transport.test.ts test/code/eval.test.ts`
   -> exit 0, testsRun > 0, failures 0 (`2-vitest-focused.log`). Both
   `alternate shortcut fallback while Vim is on` cases pass.
3. `./node_modules/.bin/vitest run test/code/vim test/canvas` -> exit 0,
   failures 0 (`3-vitest-vim-canvas.log`). This is the Vim and canvas
   regression net. These directories must not need wasm; if a file there
   fails because wasm is missing, record it and rerun it alone after
   LPI-WIRE's wasm build finishes.
4. `wc -l` on every touched file -> all under 1000 (`4-line-counts.log`).
5. **Mutation evidence** (report only as `mutationEvidence`). Temporarily
   remove the `vim.enabled()` gate from the `stopAll` wiring. Show that the
   new Vim-off test fails. Then restore it.

## Completion Criteria

- [x] `fallbackStopShortcut` is exported, and `KeyboardController` uses it.
- [x] `mount.ts` passes `stopAll`/`cut`, gated on `vim.enabled()`. No
  `hush` option remains anywhere in `InputController` options
  (`grep -rn "hush:" editor/src/code/mount.ts` shows no InputController
  use).
- [x] `isPassThrough` passes `mod && code === 'Period'`.
- [x] Tests are updated as listed, and the intentional change is recorded
  with its reason.
- [x] Verification 1-4 exit 0. Mutation evidence is recorded separately.
- [x] The Progress Log records pre/post sha256 for every edited file.

## Rerun (workflow session 438): verify and review the committed work

TASK-V1 to TASK-V4 are implemented in commit b327141. This plan has never
had a test-integrity or adversarial review. Design section 11 requires
both before acceptance. This rerun does **not** re-implement anything. All
logs go to `tmp/live-perf-int/vim-stop/rerun-438/`, written from `editor/`
as `../tmp/live-perf-int/vim-stop/rerun-438/<name>.log`.

### TASK-VR1: Deliverable check (no edit; `rerun-438/r1-deliverables.log`, from the repository root)

- `grep -n 'export function fallbackStopShortcut' editor/src/ui/stop-keys.ts`
  [1 match]
- `grep -n 'fallbackStopShortcut(' editor/src/code/keyboard.ts` [1 match,
  after the `Mod-Enter` check in `handle`]
- `grep -n 'stopAll: () => { if (vim.enabled()) transport.stopAll(); }' editor/src/code/mount.ts`
  and `grep -n 'cut: () => { if (vim.enabled()) transport.hush(); }' editor/src/code/mount.ts`
  [1 match each]
- `grep -n 'hush:' editor/src/code/mount.ts` [no InputController option]
- `grep -n "input.code === 'Period'" editor/src/code/vim/keys.ts` [1 match
  inside `isPassThrough`]
- `git diff 9474491 b327141 --stat -- editor/test editor/src` [only this
  plan's writePaths are listed]
- `git diff 9474491 HEAD -- editor/src/code/transport.ts editor/src/code/eval.ts editor/src/code/vim/ex.ts editor/src/code/vim/controller.ts editor/src/code/vim-mount.ts`
  [empty: the app-level handler and the Vim hush paths are unchanged]

If any result differs, record it and stop. The review step decides.

### TASK-VR2: Verification rerun

Run verification steps 1-4 of this plan again, unchanged, with logs
`rerun-438/1-check.log` to `rerun-438/4-line-counts.log`. Report each as
a verification record. Step 2 must show both
`alternate shortcut fallback while Vim is on` cases and the Vim-off case
passing.

### TASK-VR3: Mutation evidence (only in `mutationEvidence`)

Take the sha256 before each mutation. Restore byte-for-byte afterwards
and record the matching hash. Run only the named focused vitest file for
each mutation.

- M1: remove the `vim.enabled()` gate from `stopAll` in mount.ts -> the
  Vim-off case in `test/canvas/vim-mount.test.ts` fails.
- M2: make `KeyboardController.handle` call
  `stopShortcut(event, isApplePlatform())` instead of
  `fallbackStopShortcut(event)` -> both
  `alternate shortcut fallback while Vim is on` cases fail.
- M3: drop `|| input.code === 'Period'` from `isPassThrough` -> the new
  `test/code/vim/keys.test.ts` `'>'` pass-through cases fail.
- M4: drop the `!altKey` condition from `fallbackStopShortcut` -> the Alt
  rejection case in `test/ui/stop-keys.test.ts` fails.
- Afterwards, rerun VR2 step 2 green (`rerun-438/m-restore-2.log`).

When VR1-VR3 are done, set Status to
`Implemented (b327141); awaiting independent review`.

### TASK-VR4: Independent review (performed by the workflow review step; read-only; reviewer did not author b327141)

**Correctness** against design section 4: V1 matcher, V2 wiring and the
`vim.enabled()` gate, V3 pass-through. `stopShortcut` stays
platform-strict, and the period rule is shared through one helper.

**Test-integrity review** of `git diff 9474491 b327141 -- editor/test`:

- `vim-mount.test.ts` is the only intentional assertion change. Its reason
  (D2/D10: the unshifted alternate period means gentle stop-all) is
  recorded. It still asserts 2 `eval`, exactly 1 `stop-all` and exactly 1
  `hush` per platform. No assertion is removed, and no count is loosened
  (for example to `>= 1`).
- `vim-input.test.ts` and `input-vim-hook.test.ts` contain only the
  mechanical `hush` -> `stopAll`/`cut` option rename.
- `stop-keys.test.ts` keeps every existing `stopShortcut` case and adds
  the seven fallback cases: both modifiers, Shift `'>'`, empty `code`,
  Alt rejection, no modifier, non-period code.
- `test/code/vim/keys.test.ts` adds both `'>'` pass-through cases and the
  no-modifier negative case.
- No test was skipped, deleted or made weaker. `test/canvas/input.test.ts`
  is unchanged.

**Adversarial review**, each case backed by a test or an argument with a
file:line citation:

- A platform-Mod press with Vim on sends exactly one message (the
  capture-phase handler at transport.ts consumes it, so there is no
  fallback double send).
- An alternate-modifier press with Vim off sends nothing.
- Alt-period is ignored.
- `Mod-Shift-.` with key `'>'` passes through in Vim normal, insert,
  visual and ex/command-line modes.
- `:hush`, `vactr.hush` and keymap presets bound to `vactr.hush` still
  send `hush`.
- A missing `event.code` (synthetic event) is treated as `''`.
- Key repeat on a held alternate-modifier period routes once per keydown,
  as on main; no new debounce is expected.

### TASK-VR5: Repair only findings

Fix each high or mid finding inside writePaths. A behavior defect gets a
failing-first test. List each fix in `addressedFeedback` with status
`repaired`, then rerun VR2. If a finding needs a file outside writePaths,
stop and report a scope amendment (mechanical test updates excepted).

### TASK-VR6: Acceptance

After the review step accepts, add a Progress Log entry with the reviewer
step id and three verdicts: correctness, test integrity and adversarial.
Then set Status to `Completed`.

### Rerun Completion Criteria

- [x] VR1 log shows every expected result.
- [x] VR2 steps 1-4 exit 0 with positive test counts; focused step 2 also passed after mutation restoration.
- [x] VR3 M1-M4 fail as expected; restore hashes match.
- [ ] Correctness, test-integrity and adversarial verdicts are recorded;
  every high or mid finding is repaired.
- [ ] Status is Completed only after the recorded acceptance.

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
**Notes**: Ready for wave 1.

### Session: 2026-10-18 (implementation, workflow step6-implement)
**Tasks Completed**: TASK-V1 through TASK-V4.
**Notes**:
- Exported `fallbackStopShortcut` with a shared period matcher; `stopShortcut` remains platform-strict. `KeyboardController` uses the fallback after the existing Mod-Enter precedence.
- Replaced the obsolete InputController `hush` option in `mount.ts` with Vim-gated `transport.stopAll()` and `transport.hush()` callbacks. Vim Shift-period passes through by `code === 'Period'`.
- Added fallback matcher and Vim pass-through cases. Updated Vim mount alternate-modifier tests intentionally: unshifted alternate period sends gentle stop-all; shifted alternate period sends cut. Added Vim-off no-op coverage. Updated the two InputController fixture option objects mechanically; Vim hush commands/bindings remain unchanged.
- Pre-edit SHA-256 values: `tmp/live-perf-int/vim-stop/pre-edit-sha256.txt`; post-edit values: `tmp/live-perf-int/vim-stop/post-edit-sha256.txt`. The plan's pre-update hash is `tmp/live-perf-int/vim-stop/plan-pre-update.sha256`; its final hash is captured in `tmp/live-perf-int/vim-stop/plan-post-update.sha256` after this update.
- Verification: `cd editor && npm run check` exit 0; focused Vitest exit 0 (8 files, 188 tests passed); Vim/canvas Vitest exit 0 (57 files, 1528 tests passed); line-count gate exit 0 (all touched source/test files under 1000 lines). Logs: `tmp/live-perf-int/vim-stop/1-check.log`, `2-vitest-focused.log`, `3-vitest-vim-canvas.log`, `4-line-counts.log`.
- Mutation evidence: removing the `vim.enabled()` gate caused the new Vim-off test to fail (1 failed, 13 skipped); the gate was restored and `mount.ts` SHA-256 matches the pre-mutation value. Evidence: `tmp/live-perf-int/vim-stop/mutation-vim-off-gate.log`, `mutation-before-sha256.txt`, `mutation-restored-sha256.txt`.
- Non-gating style context: `cd editor && npm run test:style` exited 1 on eight `button -> input` / `button -> button` spacing measurements across Chromium and WebKit (`tmp/live-perf-int/vim-stop/5-style.log`). This command is not in LPI-VIM-STOP verification steps 1-4. The eight findings are JSON-identical with and without this plan's editor changes (`tmp/live-perf-int/reconcile/5-baseline-style.log` and `2-style.log`) and the check passes on main (`tmp/live-perf-int/reconcile/7-main-style.log`). Track the baseline findings as UF-1 under its separate scope; do not count this command as a passing verification.
- Implementation is complete for this plan. Formal review and shared closeout remain downstream workflow work.

### Session: 2026-10-18 (UF-2 redispatch, workflow step6-implement)
**Tasks Completed**: Reconciled the style-gate disposition and reran LPI-VIM-STOP verification steps 1-4 on the current source.
**Notes**:
- Confirmed `editor/src/ui/stop-keys.ts`, `editor/src/code/keyboard.ts`, `editor/src/code/mount.ts`, and `editor/src/code/vim/keys.ts` SHA-256 values exactly match `tmp/live-perf-int/vim-stop/post-edit-sha256.txt`; recorded the pre-edit plan hash at `tmp/live-perf-int/vim-stop/plan-pre-redispatch.sha256`. No editor source or test file changed in this redispatch.
- Recorded the test:style result as non-gating baseline context only. Its eight findings match the baseline with and without the LPI-VIM-STOP edits, while main passes; UF-1 owns that separate scope.
- Current-source verification: `npm run check` passed (exit 0; `tmp/live-perf-int/vim-stop/6-redispatch-check.log`); focused Vitest passed 188/188 (exit 0; `7-redispatch-vitest-focused.log`); Vim/canvas Vitest passed 1528/1528 (exit 0; `8-redispatch-vitest-vim-canvas.log`); line-count gate passed (exit 0; `9-redispatch-line-counts.log`). These are the plan's verification steps 1-4 only.
- Existing mutation evidence remains `tmp/live-perf-int/vim-stop/mutation-vim-off-gate.log`: removing the `vim.enabled()` gate made the Vim-off regression test fail, and the gate was restored. Mutation evidence is not a gating verification record.
- Status remains Completed for implementation; typed test-integrity and adversarial review decisions are downstream review steps.

### Session: 2026-10-18 (plan revised, workflow session 438 step 4)
**Tasks Completed**: none (plan revision only).
**Notes**: Status reset to In Progress. Converted to a review-and-repair rerun (design section 11). Added TASK-VR1 to TASK-VR6, including the test-integrity and adversarial review checklists, and the Rerun Completion Criteria. Completed only after correctness, test-integrity and adversarial verdicts are recorded.


### Session: 2026-10-18 (rerun, workflow session 438 step6-implement)
**Tasks Completed**: TASK-VR1, TASK-VR2, TASK-VR3.
**Notes**:
- VR1 deliverables match the accepted design and assigned writePaths; `rerun-438/r1-deliverables.log` records the expected matcher, Vim-gated callbacks, code-period passthrough, scoped commit diff, and empty diffs for transport/eval/Vim hush files.
- VR2 current-source checks passed: `npm run check` (exit 0); focused Vitest (8 files, 188/188 tests); Vim/canvas Vitest (57 files, 1528/1528 tests); line counts (all listed files under 1000). Logs are `rerun-438/1-check.log` through `rerun-438/4-line-counts.log`. After mutation restoration, focused Vitest passed again (8 files, 188/188; `rerun-438/m-restore-2.log`).
- VR3 mutation controls M1-M4 each failed the intended focused assertion and each restore hash matched its immediately pre-mutation file hash. Logs: `rerun-438/mutation-m1.log`, `mutation-m2-retry.log`, `mutation-m3.log`, `mutation-m4.log`; intents and SHA-256 pairs are in the adjacent `m*-intent.txt`, `m*-before.sha256`, and `m*-restored.sha256` files. The first M2 setup attempt imported a nonexistent `../ui/platform` path and failed before tests loaded (`mutation-m2.log`); it was corrected to use the existing `../ui/stop-keys` export, and the valid retry produced the expected two failing alternate-modifier cases.
- No implementation source changed in this rerun. Correctness, test-integrity, and adversarial verdicts and any resulting repair/acceptance remain assigned to the downstream review step; do not mark Completed before those verdicts.
- Plan-file SHA-256 before this Progress Log update: `9bd84945b1e1f647d0b0056a2e0286c6215a0428c68dcc69cbe24eab5937c52d`; the final plan hash is recorded at `rerun-438/plan-after-hash-note-retry.sha256`.
