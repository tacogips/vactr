# Canvas Cutover DIAG-OFFPATH: Diagnostics Check After a Presented Frame on an Input-Free Pause Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-DIAG-OFFPATH (session 303, wave 18; runs alone after CANVAS-SYNTAX-WORKER is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.16 part B (and part A for `afterPresent`); 15.3.8.14 section 3 "Check" (as amended); 15.3.8.7 allowed whole-text transfers (as amended); 15.3.8.13 C (unchanged revision skip)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-DIAG-OFFPATH`)
**Created**: 2026-10-07
**Last Updated**: 2026-10-07

---

## Intent and Context

`DiagnosticsController.runCheck` calls `WasmCore.check` (`session_check`) synchronously. It
takes about 32 ms and runs about 71 times per canonical run
(`tmp/canvas-cutover/diag-webkit-input/REPORT.md`). Today it fires from a 300 ms debounce timer
(`editor/src/code/diagnostics.ts:179-186`), armed only by document changes
(`sync.onChange`, `diagnostics.ts:96`). So it can start between a keystroke and that
keystroke's frame, for example when a caret key arrives without a document change, or right
after a pause.

The design decision (15.3.8.16 part B) keeps the check on the main-thread wasm and changes when
it runs:

- input events also re-arm the debounce;
- after the debounce, the controller waits for a presented frame (`afterPresent`, added by
  CANVAS-SYNTAX-WORKER in `editor/src/code/frame.ts`);
- the check runs only if no input or document change happened since that frame.

Results are capped at 1,024 diagnostics.

## Non-goals

- No worker-hosted wasm. That option was rejected in design 15.3.8.16 part B.
- No change to `CHECK_DEBOUNCE_MS` (300), to the merge or mapping of static, check and runtime
  batches, to pins (`diag:check`), or to the browser-only enablement.
- No Rust, no `editor/src/protocol/wasm.ts` change, no Session Protocol change.
- No syntax or telemetry change. Do not edit `frame.ts`, which CANVAS-SYNTAX-WORKER already
  provides.

## Ownership

writePaths:

- `editor/src/code/diagnostics.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/input.ts` (no edit expected; listed by design 15.3.8.16 part F. Edit only if
  the bridge textarea cannot be reached from `mount.ts` through `input.accessibility.textarea`.)
- `editor/test/code/diagnostics.test.ts`
- `editor/test/canvas/edit-cost.test.ts`
- `editor/test/canvas/input.test.ts` (no edit expected)
- `editor/test/canvas/mount.test.ts`
- `impl-plans/active/canvas-cutover-diag-offpath.md` (this plan; progress log only)
- `tmp/canvas-cutover/diag-offpath` (artifact root: logs, `intent.json`, `receipt.json`)
- `editor/node_modules/.vite` (artifact root)
- `editor/dist` (artifact root)
- `target` (artifact root)

sharedPaths: none.

## Contracts and Key Points

### 1. `DiagnosticsOptions` additions (`diagnostics.ts`)

- `afterPresent?: (cb: () => void) => () => void`. The default is
  `requestAnimationFrame(() => setTimeout(cb, 0))` when `globalThis.requestAnimationFrame`
  exists, and `setTimeout(cb, 0)` otherwise. This is the same default as `SyntaxSpans`; import
  the helper if CANVAS-SYNTAX-WORKER exported one, and do not duplicate it.
- `isComposing?: () => boolean`, which defaults to `() => false`.
- `export const MAX_CHECK_DIAGNOSTICS = 1024`.
- `readonly stats = { checks: 0, checkDeferrals: 0, checkDropped: 0 }`. `checks` counts actual
  `core.check` calls.

### 2. New method `noteInput(): void`

- Cost O(1): record an input sequence number (`inputSeq++`) and re-arm the debounce exactly as
  `scheduleCheck` does.
- It is a no-op when `!checksEnabled`.

### 3. Schedule (replaces the direct `runCheck` from the timer)

1. A debounce expiry no longer calls `runCheck`. It records
   `snapshot = { inputSeq, revision: sync.revision }` and calls
   `this.pendingPresent = afterPresent(() => this.afterFrame(snapshot))`.
   - The snapshot is taken at expiry, which is before the frame. Requiring both values to be
     unchanged at task time therefore also covers "no input since the frame", and is stricter.
   - Do not take the snapshot inside the task.
2. The task (`afterFrame`): if `inputSeq` or `sync.revision` changed since the snapshot, or
   `isComposing()` is true, then `stats.checkDeferrals++`, re-arm the debounce, and return.
   Otherwise call `runCheck()`.
3. `runCheck()` keeps the unchanged-revision skip and latest-wins. After `core.check` it keeps the
   first 1,024 diagnostics, adds the surplus to `stats.checkDropped`, and increments
   `stats.checks`.
4. `scheduleCheck()` (document change) and `noteInput()` also cancel a pending `afterPresent`
   callback.
5. `dispose()` cancels the debounce and any pending `afterPresent`.
6. `runCheck()` stays public for direct test use. It must not be called from any keystroke
   handler.

### 4. Mount wiring (`mount.ts`)

- Pass these to the `DiagnosticsController`:
  - `afterPresent: (cb) => scheduler.afterPresent(cb)`;
  - `isComposing: () => input?.isComposing ?? false`.

  The scheduler is created after the controller today, so pass a closure that reads the
  `scheduler` variable lazily.
- Add one listener function `noteDiagInput = () => diagnostics.noteInput()` on
  `input.accessibility.textarea` for `keydown`, `beforeinput`, `compositionstart`,
  `compositionupdate` and `compositionend`, next to the existing `keyRecord` listener
  (`mount.ts:264-265`). Remove all of them in `dispose` (`mount.ts:338`).
- Do not call `noteInput` from the surface subscription. Document changes already arrive through
  `sync.onChange`.

### Patterns to imitate

- `editor/src/code/diagnostics.ts:scheduleCheck/disarm` for the timer handling.
- `editor/test/code/diagnostics.test.ts:setup` (`RecordingTransport`, fake timers) for the rows.
- `editor/src/code/mount.ts:keyRecord` for listener add and remove.

## Tasks

### TASK-DO1: Options, `noteInput`, the post-frame schedule, cap and stats in `diagnostics.ts`

### TASK-DO2: Mount wiring (`afterPresent`, `isComposing`, input listeners) in `mount.ts`

### TASK-DO3: Test rows, verification and progress log

## Test Cases

`editor/test/code/diagnostics.test.ts`. Use fake timers, a manual `afterPresent` queue that
models "callbacks queued until a frame, then run in a later task", and a counting checker that
records a `phase` label (`'keystroke' | 'frame' | 'task'`) set by the test around each step:

- A keystroke (`noteInput` plus an edit), then timers advanced by 299 ms → `checks` 0.
- Then +1 ms (300 total), with no frame presented → `checks` 0. Present one frame (the queue
  moves to a task), then run the task → `checks` 1, and the checker's phase was `'task'`.
- After the debounce, present a frame, then `noteInput()` before the task runs → the task runs
  0 checks and `checkDeferrals` is 1. After another 300 ms, a frame and the task → `checks` 1.
- 20 edits 50 ms apart, each with `noteInput` in a `'keystroke'` phase and a frame in a
  `'frame'` phase after each → the checker never ran in phase `'keystroke'` or `'frame'`. After
  the last edit, 300 ms, a frame and the task → exactly 1 check.
- *Cap* (both branches in one test): a checker returning 1,500 diagnostics keeps 1,024 in
  `current()` sources of kind `check` and `checkDropped` is 476. A checker returning 1,024 keeps
  1,024 and `checkDropped` is 0.
- Composing at task time → 0 checks and `checkDeferrals` +1.
- Unchanged revision → 0 checks (the existing skip still holds).
- `dispose()` with a pending `afterPresent` → the callback never checks.
- Every existing row keeps passing. Rows that advanced only timers to trigger a check now also
  present a frame and run its task. Their assertions stay unchanged.

`editor/test/canvas/edit-cost.test.ts`:

- The existing "checks 0 before 300 ms" assertion still holds.
- New row: a mounted single-character edit on the 20,000-line fixture runs 0 checks in the
  keystroke task and in the next frame callback.

`editor/test/canvas/mount.test.ts`: a `keydown` on the bridge textarea calls `noteInput` (spy or
`checkDeferrals` observable), and dispose removes the listener (a `keydown` after dispose has no
effect).

## Pitfalls

- Running `runCheck` directly from the debounce timer keeps the old bug.
- Taking the snapshot when the task runs, instead of at debounce expiry, makes the guard
  meaningless.
- `queueMicrotask` or a `Promise.then` from a keystroke handler is still in the keystroke task.
  Never use them for the check.
- Forgetting to cancel the pending `afterPresent` on a new input leaves two checks in flight.
- Do not weaken any existing `diagnostics.test.ts` assertion. Only add frame and task steps.

## Verification

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run check > ../tmp/canvas-cutover/diag-offpath/npm-check.log 2>&1` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/code/diagnostics.test.ts test/canvas/edit-cost.test.ts test/canvas/mount.test.ts test/canvas/input.test.ts > ../tmp/canvas-cutover/diag-offpath/focused.log 2>&1` | exit 0; the rows above pass |
| `cd editor && ./node_modules/.bin/vitest run > ../tmp/canvas-cutover/diag-offpath/vitest-full.log 2>&1` | exit 0, failureCount 0 |
| `git diff --check` | exit 0 |

Outside the sandbox (verification step; serial; the measurement lock as in design 15.3.8.16 part
D for full nextest and any browser run, with `CANVAS-DIAG-OFFPATH` written to
`.measure-lock/owner` and released by a trap):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run test:perf > ../tmp/canvas-cutover/diag-offpath/test-perf.log 2>&1` (alone) | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Under the lock: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |

Records use the mandatory format (numeric `exitStatus: 0`, `outcome: "passed"`, `testsRun > 0`,
`failureCount: 0`, `log`; details in `notes`). No mutation or negative-control commands.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/diag-offpath/intent.json`
and `receipt.json`. `mount.ts` was last written by CANVAS-SYNTAX-WORKER, so read it fresh before
editing. Drift in a not-yet-edited file stops edits to that file, and repair is serial.

## Completion Criteria

- [ ] `DiagnosticsController` has `noteInput`, `afterPresent`/`isComposing` options,
  `MAX_CHECK_DIAGNOSTICS = 1024` and `stats { checks, checkDeferrals, checkDropped }`.
- [ ] A check runs only in an `afterPresent` task with no input and no document change since the
  frame. The keystroke-phase and frame-phase counters are 0 in the 20-edit row.
- [ ] Input listeners are attached in `mount.ts` and removed on dispose.
- [ ] The cap row (both branches), the deferral, composing, unchanged-revision and dispose rows
  pass. Existing rows pass with no weakened assertion.
- [ ] `npm run check`, full vitest, `test:perf`, strict clippy, full nextest, the wasm32 build and
  the Tauri check all exit 0.
- [ ] No touched file reaches 1,000 lines. `git diff --name-only` stays within the writePaths.
- [ ] The progress log records the hashes and logs.

## Progress Log

### Session: 2026-10-07 (session 303 plan authoring)

**Tasks Completed**: Plan authored from design 15.3.8.16 part B (accepted by step 3,
comm-004816).
