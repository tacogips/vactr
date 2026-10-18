# LPI-VIM-STOP: stop and cut shortcuts with Vim on, and the removed hush option

**Status**: Ready
**Plan ID**: LPI-VIM-STOP (wave 1)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 2 (I2, I3) and 4 (V1-V3, Tests)
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

- [ ] `fallbackStopShortcut` is exported, and `KeyboardController` uses it.
- [ ] `mount.ts` passes `stopAll`/`cut`, gated on `vim.enabled()`. No
  `hush` option remains anywhere in `InputController` options
  (`grep -rn "hush:" editor/src/code/mount.ts` shows no InputController
  use).
- [ ] `isPassThrough` passes `mod && code === 'Period'`.
- [ ] Tests are updated as listed, and the intentional change is recorded
  with its reason.
- [ ] Verification 1-4 exit 0. Mutation evidence is recorded separately.
- [ ] The Progress Log records pre/post sha256 for every edited file.

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
