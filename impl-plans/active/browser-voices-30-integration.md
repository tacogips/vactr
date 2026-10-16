# BV-30: Browser-Tier Evaluation of the Nine Examples and Final Gates

**Status**: Ready
**Plan ID**: BV-30 (wave 2, after BV-10 and BV-20)
**Design Reference**: `design-docs/specs/design-browser-tier-voices.md`
("Editor registry" handling rule, "Validation" gating commands, "Risks")
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Intent and Context

BV-10 makes the checker accept realized templates. BV-20 registers the nine
examples. This plan proves the result in the real wasm browser-tier
session, repairs any hidden second failure in an example (design
"Editor registry" handling rule), and runs every gating suite once on the
joined tree.

## Non-goals

- No change to `src/`, to `editor/src/`, or to any test file. A defect
  there goes back to BV-10 or BV-20 as a Progress Log entry for serial
  repair.
- Never restore a removed `EXCLUDED` row. Never edit the six remaining
  exclusions or the song-mode examples.
- In an example fix: no new samples, MIDI or host input, no gain increase
  on any slot, and no change of the voice the example demonstrates.
- No golden re-bless and no system volume change.

## Dependencies

- **dependsOn**: BV-10, BV-20
- **Blocks**: workflow finalization (commit, archive)

## writePaths

- `examples/acid-techno.vact`
- `examples/offbeat-bass-techno.vact`
- `examples/rumble-techno.vact`
- `examples/wobble-techno.vact`
- `examples/bass-presets.vact`
- `examples/experimental-oscillators.vact`
- `examples/hurdy-gurdy.vact`
- `examples/kalimba.vact`
- `examples/tonewheel-organ.vact`
- `impl-plans/active/browser-voices-30-integration.md` (this plan's Progress Log only)
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `editor/dist` (artifact root)
- `tmp/browser-voices/BV-30` (artifact root; evidence logs)
- `tmp/examples-e2e` (artifact root; Playwright scenario output)
- `tmp/ui-style` (artifact root; style gate output)

## sharedPaths

- none

## Steps

1. Pre-check:
   - confirm BV-10 and BV-20 Progress Logs report their verification
     passing;
   - record `git status --porcelain=v1 -uall` and `shasum -a 256` of every
     BV-10 and BV-20 writePath file;
   - do not run git write commands.
2. Rebuild the debug cdylib:
   `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
3. Focused browser evaluation, under the measurement lock:
   `cd editor && npx vitest run test/wasm/examples.test.ts`.
   - Every registered example must pass, the nine new ones included.
   - If a new example fails for a reason other than `unknown-keyword`
     (cap, voice count, runtime fault), diagnose it from the reported
     diagnostic or failure.
   - Make the smallest edit to that example file only: lower a parameter
     within browser caps, or shorten `gate-length`. Every `gain` must stay
     equal to or lower than its current value.
   - Record the before and after hashes and the reason in the Progress
     Log, then re-run step 3.
   - If it still fails after a reasonable fix, or the failure is in
     `src/`, stop and record it for serial repair. Do not exclude the
     example.
   - If an example file changes, also re-run
     `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(bass_examples) | test(template_sounds) | test(corpus)'`,
     because native tests `include_str!` these files.
4. Full gates, in this order. Each heavy suite runs under the lock.

## Measurement lock

Run `mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`
before each heavy suite (full nextest, any vitest that loads wasm, full
vitest, `test:style`, `e2e:examples`) and `rmdir` the lock right after.
If `mkdir` fails, wait (foreground `sleep 30` in a bounded retry loop)
and retry. Remove only a lock this command created. Never kill processes
this run did not start; a sibling run is active in wf-vim-mode.

## Verification (logs in `tmp/browser-voices/BV-30/<name>.log`, each ending with its exit status; gating records use {command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes})

- `wasm-debug.log`: the step 2 build -> exit 0.
- `examples-wasm.log`: `cd editor && npx vitest run test/wasm/examples.test.ts`
  -> exit 0. The run lists `acid-techno`, `bass-presets`,
  `experimental-oscillators`, `hurdy-gurdy`, `kalimba`,
  `offbeat-bass-techno`, `rumble-techno`, `tonewheel-organ` and
  `wobble-techno` as passing "evaluates without errors".
- `fmt.log`: `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0.
- `clippy.log`: `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` -> exit 0.
- `nextest-sound.log`:
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(sound)'`
  -> exit 0.
- `nextest-full.log` (lock): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run`
  -> exit 0, 0 failures (record run, passed and skipped counts).
- `wasm-release.log`: `mise run build-wasm-release` -> exit 0.
- `npm-check.log`: `cd editor && npm run check` -> exit 0.
- `vitest-full.log` (lock): `cd editor && npm run test` -> exit 0, 0 failures (record counts).
- `build.log`: `cd editor && npm run build` -> exit 0.
- `style.log` (lock): `cd editor && npm run test:style` -> exit 0 (4/4).
- `e2e-examples.log` (lock): `cd editor && npm run e2e:examples` -> exit 0.
  `editor/test/e2e/serve.mjs` serves `dist` on an OS-assigned free port
  (`listen(0)`), so 5173 and 5174 are never used. Do not start a Vite dev
  server on 5173 or 5174.
- `lines.log`: `wc -l` of every file touched by BV-10, BV-20 and BV-30 ->
  each source file is under 1000 lines.
- `git-status.log`: `git status --porcelain=v1 -uall` -> only the
  declared writePaths of BV-10, BV-20 and BV-30 and the design and plan
  documents are changed. `tmp/` is untracked or ignored.

## Completion Criteria

- [ ] The wasm example-evaluation vitest passes, including all nine new entries
- [ ] Any example fix is recorded with reason and hashes, with no gain increase; or none was needed
- [ ] fmt, clippy, focused and full nextest, host-wasm debug build, build-wasm-release, npm check, full vitest, build, test:style and e2e:examples all exit 0, with logs
- [ ] The song-mode exclusions are unchanged (`git diff editor/src/examples/manifest.ts` shows only the nine removals and nine additions)
- [ ] Progress Log updated; plan archiving to `impl-plans/completed/` is left to workflow finalization

## Progress Log

### Session: 2026-10-17 (plan authored, session 386)
**Tasks Completed**: none
**Tasks In Progress**: none
**Blockers**: waits for BV-10 and BV-20
**Notes**: Plan created from the accepted design.
