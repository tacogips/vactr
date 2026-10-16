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

- [ ] `vim-behavior.mjs` implements the six checks in their own context,
      and `behavior.mjs` merges them.
- [ ] Gates 1-3, 5 and 7 pass in this plan, with the records in the
      Progress Log.
- [ ] Gates 4 and 6 pass, or carry a recorded verificationGap for the Opus
      verification step with the exact sandbox error.
- [ ] No tracked evidence file changed.

## Progress Log

### Session: (not started)
