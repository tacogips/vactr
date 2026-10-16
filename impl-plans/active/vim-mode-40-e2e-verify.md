# Vim Mode: Playwright e2e and Full Verification Gates Implementation Plan

**Status**: Ready
**Plan ID**: VIM-40 (wave 4)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 8.3, 8.4, 9
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-30
- **Parallel (wave 4)**: VIM-41 (docs only; no shared files)

## Intent and Context

This plan adds the Vim e2e checks to the existing Playwright behavior
harness (`editor/test/e2e/run.mjs` -> `behavior.mjs` `runBehavior`), and
runs every gate of design 8.4 on the integrated branch.

- `run.mjs` requires a built `editor/dist`.
- `--profile behavior` skips measurement.
- `--out` must point under `tmp/` so the tracked canvas-cutover evidence
  is not touched.
- The page exposes `window.__vactrPerf` with `?perf=1`, including `doc()`
  and `selection()` (`src/code/perf-hook.ts:52-53`).

The common execution rules in `vim-mode-00-contract.md` apply; this wave
runs the full suites.

## Non-Goals

- No source changes under `editor/src/`. If a gate fails because of a
  defect, record it with the failing log and the exact file/line, then fix
  it only if the fix is inside this plan's `sharedPaths` and is a
  mechanical correction. Otherwise report it as a blocker with the
  evidence.
- Do not pass `--write-evidence`, and do not edit
  `design-docs/specs/design-canvas-editor-evidence.md`.
- Do not change any existing behavior check in `behavior.mjs`.

## File-Level Changes

### `editor/test/e2e/vim-behavior.mjs` (new, at most 250 lines)

`export async function runVimChecks(browser, origin, name)` returns
`{ checks, limitations }`, in the same check record shape as
`runBehavior`. It uses its **own** browser context, so the Vim setting
never leaks into the existing checks. The context uses
`installSilentSink` from `./silent-sink.mjs` and the same viewport, and
the page loads `${origin}/?perf=1`. Wait for `window.__vactrPerf` and
`.vact-code-canvas`, as `behavior.mjs` does.

The checks, in order:

1. **`vim-toggle`.** Click `.vact-vim-toggle`, then expect
   `.vact-vim-status[data-mode="normal"]`. Reload; the row is still
   present (localStorage). Click again; the row is gone. Click once more
   to leave it on for the next checks.
2. **`vim-dcyp`.** Seed the doc through the textarea in insert mode (`i`,
   `fill`/`type`, `Escape`), then run `gg0`, `dw`, `cwfoo<Esc>`, `yyp`
   and `P`. Check `window.__vactrPerf.doc()` against the expected string
   after each step.
3. **`vim-visual`.** `vjd` on a 3-line doc gives the expected text; then
   `Vjy`, `G` and `p` give the expected text.
4. **`vim-dot`.** `ciwbar<Esc>`, `w`, `.` -> both words are `bar`.
5. **`vim-ime-normal`.** In normal mode:
   - Chromium: open a CDP session and send `Input.imeSetComposition`
     (`text: 'にほん'`), then `Input.insertText` (`text: '日本'`).
   - WebKit: dispatch synthetic `compositionstart`/`compositionupdate`/`compositionend`
     plus `keydown` with `keyCode: 229` and `beforeinput insertText`
     events on the textarea.
   - Expect: `doc()` unchanged, `data-mode` still `normal`, and the status
     message contains `IME input ignored`. If the CDP method is
     unavailable, return `{ status: 'limitation', detail }` (never a
     pass).
6. **`vim-shortcut`.** In normal mode, press `Meta+Enter` on macOS hosts
   or `Control+Enter` elsewhere (`process.platform === 'darwin'`). The
   eval status (`.vact-eval-status`) leaves `idle`, or the transport
   records an eval. Never click audio controls; the silent sink is
   installed.

At the end, turn the toggle off and close the context.

### `editor/test/e2e/behavior.mjs` (minimal)

Import `runVimChecks`. Before the final `return`, after the context is
closed in `finally`, add
`const vim = await runVimChecks(browser, origin, name); results.push(...vim.checks); limitations.push(...vim.limitations);`.
Wrap the call in try/catch; a throw becomes
`results.push({ id: 'vim-setup', pass: false, detail })`.

### `editor/test/e2e/README.md`

Add a "Vim mode checks" paragraph. It should list the six check IDs and
say they run in their own context, and give the non-gating command from
step 6 below.

## Gates (serial, foreground, logs under `tmp/vim-mode/VIM-40/`)

Run from `editor/`. "Locked" means:
`mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`. If it
already exists, wait 60 s and retry (at most 60 tries). Run the command,
then `rmdir` the lock even on failure (use `trap`). Never remove a lock
you did not create.

1. `npm run check`.
2. Locked: `./node_modules/.bin/vitest run`. This is the full suite. It
   must exit 0 with 0 failed. Record `testsRun` and `testsPassed` from the
   summary line.
3. `VACTR_REQUIRE_SESSION_ABI=1 npm run build`. If
   `../target/wasm32-unknown-unknown/release/vactr.wasm` is absent,
   prefix the command with
   `VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm` and say
   so in `notes`. Do not build the release wasm.
4. Locked: `npm run test:style`. It must print
   `Style assertions passed for 8 engine/viewport combinations.`. That is
   4 off plus 4 vim passes after VIM-30's extension. Record the exact
   count.
5. `node --check test/e2e/vim-behavior.mjs`.
6. Locked: `npm run e2e -- --browser all --profile behavior --run-id vim-001 --out ../tmp/vim-mode/e2e/vim-001`.
   It must exit 0, and the JSON result must show `pass: true`,
   `failureCount: 0`, and all six `vim-*` checks passed or recorded as
   limitations. A `vim-ime-normal` limitation is allowed only on WebKit.
7. `git diff --stat -- design-docs/specs/evidence` must be empty.

**Sandbox note.** Steps 4 and 6 start a localhost server and browsers. If
the implementation sandbox denies the localhost bind or the browser launch
(for example `EPERM` on `listen`), do not record a failing gate. Record a
`verificationGap` with the exact error line and log path, and leave steps
4 and 6 to the serial Opus verification step, which runs outside the
sandbox. Every other gate must pass inside the sandbox.

## Acceptance Criteria

- [x] `vim-behavior.mjs` implements the six checks in their own context,
      and `behavior.mjs` merges them.
- [x] Gates 1-3, 5 and 7 pass in this plan, with the records in the
      Progress Log.
- [ ] Gates 4 and 6 pass, or carry a recorded verificationGap for the Opus
      verification step with the exact sandbox error.
- [x] No tracked evidence file changed.

## Progress Log

### Session: 2026-10-17 VIM-40 implementation

**Status**: In Progress
**Tasks Completed**: E2E checks and runner merge; README update; Gates 1-5 and 7.
**Notes**:
- Added `editor/test/e2e/vim-behavior.mjs` with `vim-toggle`, `vim-dcyp`, `vim-visual`, `vim-dot`, `vim-ime-normal`, and `vim-shortcut`, isolated in a separate Playwright context. `behavior.mjs` merges check results and limitations after closing its original context.
- `npm run check`: exit 0 (`tmp/vim-mode/VIM-40/gate-1-check-rerun.log`).
- Locked full Vitest: exit 0, 119 files / 1,682 tests passed (`tmp/vim-mode/VIM-40/gate-2-vitest.log`).
- Debug-Wasm build with `VACTR_REQUIRE_SESSION_ABI=1`: exit 0 (`tmp/vim-mode/VIM-40/gate-3-build.log`); release Wasm was absent, so `VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm` was used.
- Locked `npm run test:style`: exit 0, 8 engine/viewport combinations (`tmp/vim-mode/VIM-40/gate-4-style-rerun.log`). The Vim style pass now reports the actual toggle presence and resets focus to the probe input before testing Tab navigation.
- `node --check test/e2e/vim-behavior.mjs`: exit 0 (`tmp/vim-mode/VIM-40/gate-5-node-check-final.log`).
- Locked Playwright: exit 1, 32 checks / 31 passed (`tmp/vim-mode/VIM-40/gate-6-e2e-final.log`). Every Vim check passed in WebKit and five of six passed in Chromium. Chromium `vim-ime-normal` observed `stable` become `stabl日本e` (revision 2 -> 3) after CDP `Input.imeSetComposition` then `Input.insertText`; this violates the accepted normal-mode IME invariant. The relevant seam is `editor/src/code/input.ts:175-195` (beforeinput/composition routing) and `:214-220` (native input gate). VIM-40 allows no source change there; the failure is not a sandbox error and cannot be recorded as a verificationGap.
- `git diff --stat -- design-docs/specs/evidence`: empty, exit 0 (`tmp/vim-mode/VIM-40/gate-7-evidence-diff.log`).
- First style and e2e failures and their complete logs are retained as `gate-4-style.log`, `gate-6-e2e.log`, and `gate-6-e2e-rerun.log`; corrected harness/style assertions pass in later runs.

**Outstanding**: Chromium normal-mode IME commit regression requires a source-owner repair and rerun of the exact VIM-40 e2e gate. The accepted VIM-40 write paths do not include `editor/src/code/input.ts`.

### Session: 2026-10-17 VIM-40 current-tree recheck

**Status**: Blocked on source ownership
**Tasks Completed**: Reproduced Gate 6 against the current tree; recorded current source identity and lock contention.
**Notes**:
- Current-tree Playwright run (`npm run e2e -- --browser all --profile behavior --run-id vim-002 --out ../tmp/vim-mode/e2e/vim-002`) reported `pass: false`, 32 checks, 31 passed, and the same Chromium `vim-ime-normal` failure. Complete runner output: `tmp/vim-mode/VIM-40/gate-6-e2e-current-01.log`. The shell wrapper did not append its exit line because zsh rejected its use of the reserved variable `status`; the complete previous gate log `gate-6-e2e-final.log` records the same failure with exit status 1.
- Current hashes: `editor/src/code/input.ts` 68e94bb21ab210a2d995ad52ccf77d7bc86e353baac303e269360dafbb74a069; `editor/test/e2e/vim-behavior.mjs` 27f5e6eecedba230bf1f65ae3a5273caa7e0a8f90bf8434dc59e29ec20bde9be; `editor/test/e2e/behavior.mjs` 9a858bac4980886758324540a1bbe362156c896c63ee69cda3a5bf3f83e263bc.
- A retry could not acquire `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock` (exit 98). Foreground polling exceeded one minute; the lock remained present and was not touched. No further suite ran.
- The regression is not a sandbox verification gap. The fix requires `editor/src/code/input.ts`, which is outside VIM-40 `writePaths` and excluded by this plan's non-goals. Resume after the input-routing owner repairs the normal-mode IME commit path; then rerun Gate 6 under the lock and record its exact exit and complete log.
