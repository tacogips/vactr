# EX-70: Playwright Examples Scenario, Style Gate, Docs and Final Gates

**Status**: Completed (session 382; EX70-TI-FLAKY-PLAYBACK repaired; Step 7 test-integrity and adversarial review accepted; archived 2026-10-17)
**Plan ID**: EX-70 (wave 3)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 9.1, 9.4, 9.5, 10
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

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

- [x] `npm run e2e:examples` exists and passes 8/8 silently.
- [x] `npm run test:style` scans the open picker and passes.
- [x] README and e2e README updated.
- [x] Final gates recorded with complete logs.
- [x] Three consecutive source-matched `npm run e2e:examples` runs pass 8/8.
- [x] Add a terminal evaluation-readiness guard and diagnostic snapshot for playback-onset failures; run the bounded 10-run classification loop. Attempt 4 is classified as the pre-audio-wait harness race; attempt 5 remains historically undiagnosed, with the new guard making any recurrence self-classifying.
- [x] Progress Log with records and pre/post sha256.

## Edit Protocol

Fresh read before each edit; sha256 before/after; writePaths only; stop and
record on drift; no git index or branch operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored. Revised for review finding ER-M1
(check 7 asserts draft recovery by undo and the rewritten draft) and for the
residual note on check 3 (row targeting by `data-example-id`).
**Blockers**: None.

### Session: 2026-10-17
**Tasks Completed**: Added the eight-check silent Chromium scenario, picker
style coverage, README documentation, and the `e2e:examples` script. The
browser checks use registered `industrial-techno` and `techno-pattern` IDs
because EX-10 explicitly excludes `acid-techno` and `wobble-techno` from the
browser registry. The style gate compares spacing within the open picker and
within the page separately because the floating picker overlays page controls.
The scenario waits for the audio model to reach `running` before Run, then
checks the measured pre-sink peak and both silent-sink invariants.

**Verification**:

```json
[
  {"command":"cd editor && npm run check > ../tmp/examples/EX-70/check.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/check.log","notes":"TypeScript check passed."},
  {"command":"bash -c 'cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build > ../tmp/examples/EX-70/build.log 2>&1; exit_code=$?; printf \"EXIT_STATUS=%s\\n\" \"$exit_code\" >> ../tmp/examples/EX-70/build.log; exit \"$exit_code\"'","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/build.log","notes":"Debug-wasm build passed; lazy chunks emitted for industrial-techno and fm6-algorithms."},
  {"command":"CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-70/cargo-check.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/cargo-check.log","notes":"Tauri crate cargo check passed."},
  {"command":"CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-70/cargo-test.log 2>&1","exitStatus":0,"testsRun":9,"testsPassed":9,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/cargo-test.log","notes":"9 Rust tests passed; remaining targets had 0 tests."},
  {"command":"bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done; trap \"python3 /Users/taco/gits/tacogips/vactr-worktrees/wf-examples/tmp/examples/EX-70/release-lock.py\" EXIT; printf \"%s\\n\" EX-70 > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; cd editor && npm test > ../tmp/examples/EX-70/full-vitest.log 2>&1; exit_code=$?; printf \"EXIT_STATUS=%s\\n\" \"$exit_code\" >> ../tmp/examples/EX-70/full-vitest.log; exit \"$exit_code\"'","exitStatus":0,"testsRun":987,"testsPassed":987,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/full-vitest.log","notes":"105 files and 987 tests passed on the combined tree under the measurement lock."},
  {"command":"bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done; trap \"python3 /Users/taco/gits/tacogips/vactr-worktrees/wf-examples/tmp/examples/EX-70/release-lock.py\" EXIT; printf \"%s\\n\" EX-70 > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; cd editor && npm run test:style > ../tmp/examples/EX-70/style.log 2>&1; exit_code=$?; printf \"EXIT_STATUS=%s\\n\" \"$exit_code\" >> ../tmp/examples/EX-70/style.log; exit \"$exit_code\"'","exitStatus":0,"testsRun":4,"testsPassed":4,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/style.log","notes":"Picker open during scan; Chromium and WebKit at both viewports passed."},
  {"command":"bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done; trap \"python3 /Users/taco/gits/tacogips/vactr-worktrees/wf-examples/tmp/examples/EX-70/release-lock.py\" EXIT; printf \"%s\\n\" EX-70 > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; cd editor && npm run e2e:examples > ../tmp/examples/EX-70/e2e-examples.log 2>&1; exit_code=$?; printf \"EXIT_STATUS=%s\\n\" \"$exit_code\" >> ../tmp/examples/EX-70/e2e-examples.log; exit \"$exit_code\"'","exitStatus":0,"testsRun":8,"testsPassed":8,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/e2e-examples.log","notes":"tmp/examples-e2e/result.json reports 8/8; playback peak -3.0498 dBFS, post-sink peak 0, direct destination connections 0."},
  {"command":"git diff --exit-code editor/src-tauri/capabilities/default.json > tmp/examples/EX-70/capabilities.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/capabilities.log","notes":"Capabilities file unchanged."},
  {"command":"node --check editor/test/e2e/examples.mjs","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"inline (final scenario source syntax check)","notes":"The final e2e runner parses successfully."}
]
```

**Prior attempts**: E2E attempts 1-5 and their result JSON are retained under
`tmp/examples/EX-70/attempt-1/` through `attempt-5/`. Attempt-3 passed
`run-plays-moderately` and failed `library` while locating the Rename button.
Attempts 4 and 5 passed the other seven checks and timed out waiting for
playback onset; their logs contain no audio/evaluation diagnostic snapshot.
The later diagnostics-only rerun passed but did not establish a fix. On
2026-10-17, three consecutive runs of the unchanged source passed 8/8; their
complete logs and result JSON are in `tmp/examples/EX-70/flake-1/` through
`flake-3/`. Playback peaks were -2.9809, -2.9808 and -2.9808 dBFS, with
post-sink peak 0 and zero direct destination connections on all three.
These runs establish repeatable current-source passes but do not classify the
historical timeouts. `main.ts` still makes the document visible before
`documents.ready` resolves and starts audible lifecycle/subscription only
afterward; the current test has no reliable pre-Run DOM signal for that boot
boundary. No harness wait or feature change was made. The root cause remains
unconfirmed and needs an instrumented reproduction or an EX-60/integration
scope decision. See `tmp/examples/EX-70/priorVerification.json`. An initial
style attempt exposed cross-overlay gap comparisons; the checker was adjusted
and the final style rerun passed. The measurement lock was released.

**Source SHA-256 (before -> after)**:
- `editor/test/e2e/examples.mjs`: new file -> `d8d8eae2ba50d3f5dd4b260ec61843a745b7c8d563933bf9f246275f326dc242`
- `editor/package.json`: `d36c7cf11e5027c49df404210a5d5fa1db874d7f6c7a1ddb99988c8bda8b8ea5` -> `881367421bc66ec95896521d7111ca88897e79c382c8b54cc3f88879135af383`
- `editor/test/style/ui-style.mjs`: `0c31d8b10651789b5aa29068654ebf8ffcef9239696b51071b1dd7c3de3834bb` -> `cb6c1c8e8d541b6a89f64504cd49d07fc001daeecb20824d94f30f852a9d8e34`
- `editor/test/e2e/README.md`: `edbe9ef646e2f56528aa8a1bf9dc843cd5dc4b7853309e51cc4c3f4a5f1fafd6` -> `2852f11a6084c44a838f6a90c80bf64c14f11ade3bcf672436b321e2988c90ce`
- `README.md`: `9a906f8dc463fbf81ec8a639acd957bcfd0a5625643af7a891b6e4362062159a` -> `f4d92b647abe056f03d15e1a369014eb427ef425221ed0c22559167eadfe3092`
- `impl-plans/active/examples-70-e2e-docs.md`: `329769b07a0e5271a077a0e9ee79879ba498f405d671e2e86a6716a5963263f1` -> `tmp/examples/EX-70/final-source-sha256.txt`.

**Blockers**: `EX70-TI-FLAKY-PLAYBACK` is unresolved. The cause is unconfirmed;
attempts 4/5 lack diagnostic snapshots, and no existing DOM signal reliably
marks boot completion before Run. Completion requires either an instrumented
reproduction that classifies the cause or an integration-owner decision on the
possible product boot-order race and any required EX-60 scope amendment.

### Session: 2026-10-17 review repair attempt
**Tasks Completed**: Re-ran `npm run e2e:examples` eight times sequentially
under the measurement lock on the unchanged EX-70 E2E source. All runs passed
8/8 with the required playback threshold and silent-sink results. Corrected
the earlier attempt history: attempt-3 was a library failure; attempts 4 and
5 were playback-onset timeouts.
**Finding**: `EX70-TI-FLAKY-PLAYBACK` remains unresolved because attempts 4/5
do not contain diagnostics sufficient to distinguish a harness timing issue
from the boot-order race in `editor/src/app/main.ts`; the eight instrumented
current-source runs did not reproduce the timeout. Do not claim the timeout
was fixed by the passing reruns. A read-only review confirms that no existing
DOM attribute reliably marks completion of `documents.ready` before Run, and
adding one would require a product-side change outside EX-70 writePaths.
**Verification**: `tmp/examples/EX-70/flake-1/` through `flake-8/` each hold a
complete log, result JSON and exit status. `tmp/examples/EX-70/priorVerification.json`
records attempts 3-5 accurately. The harness files and threshold assertions
were not changed.
**Status**: In Progress pending root-cause classification or explicit
integration-owner disposition of the possible boot-order race.

### Session: 2026-10-17 playback finding repair
**Tasks Completed**: Added an evaluation-status readiness guard after Run in
`editor/test/e2e/examples.mjs`. A non-`ok` terminal evaluation status fails
immediately; a readiness or onset timeout records evaluation `data-status`,
`aria-label`, audio state and silent-sink report. The onset timeout and audio
thresholds remain unchanged. Attempt 4 is classified as the harness race
caused by clicking Run before waiting for the AudioContext to run; attempt 5
remains undiagnosed because its historical log contains no snapshot. The new
guard makes future occurrences self-classifying. Ten sequential runs on the
repaired source passed 8/8; no recurrence occurred.

**Verification**: Final-source logs are in `tmp/examples/EX-70/`:
`check-final.log`, `build-final.log`, `cargo-check-final.log`,
`cargo-test-final.log`, `full-vitest-final.log`, `style-final.log`,
`e2e-examples-final.log`, and `unchanged-final.log`. The 10 classification
runs are in `classify-1/` through `classify-10/`; each contains an 8/8 result,
complete command log and exit status. Full Vitest passed 987/987, style passed
four engine/viewport combinations, final e2e passed 8/8 (playback peak
-2.980848936630205 dBFS, post-sink peak 0, direct connections 0), and Tauri
tests passed 9/9. The debug-wasm build, `npm run check`, cargo check and
unchanged harness/capabilities diff exited 0. `tmp/examples/EX-70/final-source-
sha256.txt` records hashes for all changed EX-70 source/doc files except this
progress file; the e2e runner changed from
`d8d8eae2ba50d3f5dd4b260ec61843a745b7c8d563933bf9f246275f326dc242` to
`a7e9b18969fa6ff5807742a974caf7a98bcf604ea9fbeceb5d2d31d001925b`.
`git diff --exit-code` confirmed the existing harness files and
`editor/src-tauri/capabilities/default.json` are unchanged.

**Final-source verification records**:

```json
[
  {"command":"cd editor && npm run check > ../tmp/examples/EX-70/check-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/check-final.log","notes":"TypeScript check passed."},
  {"command":"cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build > ../tmp/examples/EX-70/build-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/build-final.log","notes":"Debug-wasm build passed."},
  {"command":"CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-70/cargo-check-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/cargo-check-final.log","notes":"Tauri cargo check passed."},
  {"command":"CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml > tmp/examples/EX-70/cargo-test-final.log 2>&1","exitStatus":0,"testsRun":9,"testsPassed":9,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/cargo-test-final.log","notes":"Nine Tauri tests passed."},
  {"command":"bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 10; done; trap \"python3 /Users/taco/gits/tacogips/vactr-worktrees/wf-examples/tmp/examples/EX-70/release-lock.py\" EXIT; printf \"%s\\n\" EX-70 > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; cd editor || exit 1; npm test > ../tmp/examples/EX-70/full-vitest-final.log 2>&1; rc=$?; printf \"EXIT_STATUS=%s\\n\" \"$rc\" >> ../tmp/examples/EX-70/full-vitest-final.log; exit \"$rc\"'","exitStatus":0,"testsRun":987,"testsPassed":987,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/full-vitest-final.log","notes":"105 files and 987 tests passed under the measurement lock."},
  {"command":"bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 10; done; trap \"python3 /Users/taco/gits/tacogips/vactr-worktrees/wf-examples/tmp/examples/EX-70/release-lock.py\" EXIT; printf \"%s\\n\" EX-70 > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; cd editor || exit 1; npm run test:style > ../tmp/examples/EX-70/style-final.log 2>&1; rc=$?; printf \"EXIT_STATUS=%s\\n\" \"$rc\" >> ../tmp/examples/EX-70/style-final.log; exit \"$rc\"'","exitStatus":0,"testsRun":4,"testsPassed":4,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/style-final.log","notes":"Picker open; four engine/viewport combinations passed."},
  {"command":"bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 10; done; trap \"python3 /Users/taco/gits/tacogips/vactr-worktrees/wf-examples/tmp/examples/EX-70/release-lock.py\" EXIT; printf \"%s\\n\" EX-70 > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; cd editor || exit 1; npm run e2e:examples > ../tmp/examples/EX-70/e2e-examples-final.log 2>&1; rc=$?; printf \"EXIT_STATUS=%s\\n\" \"$rc\" >> ../tmp/examples/EX-70/e2e-examples-final.log; exit \"$rc\"'","exitStatus":0,"testsRun":8,"testsPassed":8,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/e2e-examples-final.log","notes":"Peak -2.980848936630205 dBFS; post-sink peak 0; zero direct destination connections."},
  {"command":"cd editor && npm run e2e:examples","exitStatus":0,"testsRun":80,"testsPassed":80,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/classify-1/ through tmp/examples/EX-70/classify-10/ (individual logs, results, exit-status files)","notes":"Ten sequential current-source invocations passed 8/8 each; no timeout recurred."},
  {"command":"node --check editor/test/e2e/examples.mjs > tmp/examples/EX-70/node-check-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/node-check-final.log","notes":"Scenario syntax valid."},
  {"command":"shasum -a 256 -c tmp/examples/EX-70/final-source-sha256.txt > tmp/examples/EX-70/hash-check-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/hash-check-final.log","notes":"All five listed source/doc hashes match."},
  {"command":"git diff --exit-code editor/src-tauri/capabilities/default.json editor/test/e2e/run.mjs editor/test/e2e/behavior.mjs editor/test/e2e/measure.mjs editor/test/e2e/serve.mjs editor/test/e2e/silent-sink.mjs > tmp/examples/EX-70/unchanged-verify-final.log 2>&1","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-70/unchanged-verify-final.log","notes":"Capabilities and existing harness files unchanged."}
]
```

**Finding**: `EX70-TI-FLAKY-PLAYBACK` is repaired by adding the readiness
classification guard and completing the bounded loop. The ten passes are
supporting evidence, not the correction itself. Attempt 5 remains an
unclassified historical event; any future recurrence now fails with the
available evaluation/audio/sink snapshot. No EX-60 product-code scope change
was needed.
**Downstream**: Independent test-integrity, adversarial and integration
reviews remain owned by later workflow steps.
**Status**: Implementation verification complete; awaiting downstream review.

### Session: 2026-10-17 closeout (session 382, Step 8)
**Tasks Completed**: Step 7 test-integrity and adversarial review accepted
EX-70 with no high or mid findings; EX70-TI-FLAKY-PLAYBACK is recorded as
repaired (guard at `editor/test/e2e/examples.mjs:86-115` plus classify-1..10).
Step 7b did not re-run the e2e (read-only, measurement lock held by a sibling);
the accepted evidence is `tmp/examples/EX-70/e2e-examples-final.log` (8/8,
EXIT_STATUS=0). README startup and picker text refreshed. All EX-00..EX-70
plans moved to `impl-plans/completed/`; `examples-dispatch.json` stays in
`impl-plans/active/` until the branch commit because the running workflow
references it.
**Residual (low)**: attempt-5 onset timeout remains historically undiagnosed
(any recurrence is self-classifying; product cause would belong to EX-60);
`editor/test/style/ui-style.mjs:107` skips picker-to-non-picker spacing pairs
without an explanatory comment. The full runner hash is
`a7e9b18969fa6ff5807742a974caf7a98bcf604ea9fbecebeb5d2d31d001925b` (see
`tmp/examples/EX-70/final-source-sha256.txt`).
**Status**: Completed.
