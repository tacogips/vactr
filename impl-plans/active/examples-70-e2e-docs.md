# EX-70: Playwright Examples Scenario, Style Gate, Docs and Final Gates

**Status**: Ready
**Plan ID**: EX-70 (wave 3)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 9.1, 9.4, 9.5, 10
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

Prove the whole feature in a real browser (silent), gate the new picker
with the built-page UI style check, document the feature, and run the
final verification set for the run.

Context:
- `editor/test/e2e/serve.mjs:startServer` serves `editor/dist` on
  `127.0.0.1` port 0 (never 5173/5174).
- `editor/test/e2e/silent-sink.mjs`: `installSilentSink(context)` and
  `readSinkReport(page)` -> `{directDestinationConnections, pre: {peak, peakDbfs, rmsDbfs, onsetCount}, post: {peak}}`.
- `window.__vactrPerf.doc()` returns the buffer when the page URL has
  `?perf=1` (`editor/src/code/perf-hook.ts`).
- Transport controls: `.vact-audio` (resume audio), `.vact-run`, `.vact-hush`;
  code input bridge `.vact-code-input-bridge textarea`.
- `editor/test/style/ui-style.mjs` loads `/`, waits for `.vact-transport`
  and `.pane-side`, opens closed `<details>`, then scans the page.
- The release wasm is absent: builds use
  `VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm`.

## Non-goals

- Do not modify `editor/test/e2e/run.mjs`, `behavior.mjs`, `measure.mjs`,
  `serve.mjs`, `silent-sink.mjs` or the canvas evidence files.
- No feature code changes. A defect found here is recorded and handed
  back to the owning plan through serial repair; fix only test or doc
  files in this plan's writePaths.
- Never produce audible output; never change the system volume.

## Dependencies

- **dependsOn**: EX-60 (and transitively EX-00..EX-50)
- **Blocks**: none (final)

## writePaths

- `editor/test/e2e/examples.mjs`
- `editor/package.json`
- `editor/test/style/ui-style.mjs`
- `editor/test/e2e/README.md`
- `README.md`
- `impl-plans/active/examples-70-e2e-docs.md`
- `editor/dist` (artifact root)
- `editor/src-tauri/target` (artifact root)
- `editor/src-tauri/gen/schemas` (artifact root)
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-70` (artifact root; evidence logs)
- `tmp/examples-e2e` (artifact root; Playwright scenario output)
- `tmp/ui-style` (artifact root; style gate output)

## sharedPaths

- `editor/package.json` (add one script: `"e2e:examples": "node test/e2e/examples.mjs"`)
- `editor/test/style/ui-style.mjs` (open the picker before the scan)
- `README.md` (Editor section only)

## Patterns to Imitate

- Harness structure, browser launch flags, JSON result line and exit
  codes: `editor/test/e2e/run.mjs` (`--mute-audio`, exit 0/1/2, blocked
  when dist or browser is missing).
- Page waits and textarea usage: `editor/test/e2e/behavior.mjs`.

## File-level Changes

### `editor/test/e2e/examples.mjs`

Chromium only, headless, `--mute-audio`. Each check is
`{id, pass, detail}`; continue after a failed check where possible. Every
context gets `installSilentSink`. Read expected texts from
`examples/showcase.vact`, `examples/acid-techno.vact`,
`examples/fm6-algorithms.vact`, `examples/wobble-techno.vact`.
Checks:
1. `first-open-showcase`: fresh context, `/?perf=1`, wait for
   `window.__vactrPerf` and for the doc to be non-empty -> doc equals
   showcase; after 1.5 s `pre.onsetCount === 0` (not auto-played).
2. `run-plays-moderately`: click `.vact-audio`, then `.vact-run` -> within
   10 s `pre.onsetCount >= 1`; `pre.peakDbfs <= -1`; `post.peak === 0`;
   `directDestinationConnections === 0`; click `.vact-hush`.
3. `picker-keyboard-open-undo`: focus `.vact-doc-picker-toggle`, Enter ->
   filter focused; type "Acid techno"; press ArrowDown until
   `document.activeElement` has `data-example-id="acid-techno"` (at most
   10 presses, else fail; the filter may match several rows); Enter -> doc equals
   acid-techno; `ControlOrMeta+z` -> showcase; `ControlOrMeta+Shift+z` ->
   acid-techno.
4. `draft-restore`: type a marker into the textarea; wait 1.5 s; reload ->
   doc equals the edited text.
5. `library`: Save as "e2e-one" -> listed; open `acid-techno` (no
   confirmation expected); type a char; open "e2e-one"
   -> `.vact-doc-picker-confirm` appears; Replace -> doc equals the saved
   text; rename to "e2e-two" -> listed under the new name; delete with
   confirm -> not listed.
6. `default-template`: open `fm6-algorithms` (clean); Use as default
   template; open `acid-techno`; New -> doc equals fm6-algorithms; reload
   -> doc equals fm6-algorithms; Reset default template; New -> showcase.
7. `explicit-example-url`: type a marker, wait 1.5 s, go to
   `/?perf=1&example=wobble-techno` -> doc equals wobble-techno and
   `.vact-doc-picker-status` contains "restores your unsaved draft";
   focus the code input textarea and press `ControlOrMeta+z` -> doc
   equals the marker text (draft recovered, review finding ER-M1); wait
   1.5 s and reload `/?perf=1` -> doc equals the marker text (draft
   rewritten).
8. `storage-unavailable`: new context with an init script making
   `window.localStorage` throw `SecurityError` on access -> doc equals
   showcase; opening the picker shows "Browser storage is unavailable".
Output: one JSON line `{pass, blocked, testsRun, testsPassed, failureCount, checks}`
to stdout and `tmp/examples-e2e/result.json`; exit 0 all pass, 1 any
failure, 2 blocked (dist or browser missing).

### `editor/package.json`

Add `"e2e:examples": "node test/e2e/examples.mjs"`. No dependency changes.

### `editor/test/style/ui-style.mjs`

After the existing details-opening step, click `.vact-doc-picker-toggle`
(if present) so the open picker panel is part of the scan; close it in
the same cleanup that closes the details. Fail the run if the toggle is
missing (the picker must be gated).

### `editor/test/e2e/README.md`

Add a short "Examples scenario" section: command
`cd editor && npm run e2e:examples`, what it checks, the silent-sink gate,
exit codes, log location, and the debug-wasm build prerequisite.

### `README.md` (Editor section)

Document: the Documents picker (examples, saved documents, Open file,
default template actions); startup precedence table (URL
`?example=<id>` > draft (web) > default template > showcase); web
storage keys `vactr.doc.v1.*`, the 262144-char limit and failure
behavior; app default template file
`<app config dir>/default-template.json`; `npm run e2e:examples`.

## Invariants

- The scenario is silent: post-sink peak exactly 0 and no direct
  destination connections, or the check fails.
- Existing e2e harness files unchanged.

## Pitfalls

- Wait for startup to finish (doc non-empty) before reading the doc;
  `boot` awaits `ready`, but the page may still be loading.
- Draft writes are debounced 1000 ms: wait 1.5 s before reload.
- Keyboard shortcuts in Playwright use `ControlOrMeta`.
- Use fresh browser contexts where storage must be empty.
- Do not leave the measurement lock behind on failure (use `trap`).

## Verification

Evidence in `tmp/examples/EX-70/`, each log ending with its exit status.
Acquire `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`
(mkdir loop; owner file `EX-70`; `trap` removes only your own lock) for
steps 3-6.

1. `cd editor && npm run check > ../tmp/examples/EX-70/check.log 2>&1` -> exit 0.
2. `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-70/cargo-check.log 2>&1` and
   `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-70/cargo-test.log 2>&1` -> exit 0 (run after step 4 if `editor/dist` is missing).
3. `cd editor && npm test > ../tmp/examples/EX-70/full-vitest.log 2>&1` -> exit 0, failureCount 0.
4. `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build > ../tmp/examples/EX-70/build.log 2>&1` -> exit 0.
5. `cd editor && npm run test:style > ../tmp/examples/EX-70/style.log 2>&1` -> exit 0, with the picker panel scanned.
6. `cd editor && npm run e2e:examples > ../tmp/examples/EX-70/e2e-examples.log 2>&1` -> exit 0, `failureCount: 0`, `testsRun: 8`.
7. `git diff --exit-code editor/src-tauri/capabilities/default.json` -> exit 0.

Records use `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`. Negative controls (for
example a temporary showcase gain change that makes check 2 fail) go
only in `mutationEvidence` and are reverted.

## Completion Criteria

- [ ] `npm run e2e:examples` exists and passes 8/8 silently.
- [ ] `npm run test:style` scans the open picker and passes.
- [ ] README and e2e README updated.
- [ ] Steps 1-7 recorded as passed with complete logs.
- [ ] Progress Log with records and pre/post sha256.

## Edit Protocol

Fresh read before each edit; sha256 before/after; writePaths only; stop and
record on drift; no git index or branch operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored. Revised for review finding ER-M1
(check 7 asserts draft recovery by undo and the rewritten draft) and for the
residual note on check 3 (row targeting by `data-example-id`).
**Blockers**: None.
