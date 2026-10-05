# Canvas Cutover: Real-Browser Evidence, Measurements and Closeout Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-EVIDENCE (wave 3; depends on CANVAS-CLOCK, NATIVE, RENDER, MOUNT, VISUAL and SHELL)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.8 (measurement protocol and thresholds), 15.3.8.9 (gates and closeout)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

The user wants evidence that editing, audio sync, existing features and resource limits work
together, using real browsers and numbers measured against the design thresholds. The platform
limitations and the physical-iPad checks that cannot be done here must be recorded honestly.

This plan builds a Playwright-library harness and produces the evidence document. It then
performs the closeout archival.

**Sandbox boundary.** The Codex sandbox cannot bind localhost, so the implementer authors and
unit-tests the harness only. The outside-sandbox verification step runs the harness. With
`--write-evidence`, the harness itself writes the raw data files and the result section of the
evidence document, so no human-typed numbers are ever committed.

Facts:

- `playwright` 1.62.1 is a devDependency (library only; there is no `@playwright/test`).
- The browsers are cached: chromium-1243 and webkit-2359.
- The existing pattern to imitate is `editor/dev-harness/run-headless.mjs`: Node built-ins, a
  127.0.0.1 ephemeral-port server, and exit codes 0 for pass, 1 for fail and 2 for blocked.
  Also `editor/test/canvas/package-browser-smoke.mjs` (Playwright launch plus a WebGL2 probe).
- The visual language form is `osc 20 > rotate 0.5 > out o0`
  (`editor/test/wasm/criteria.test.ts:462`).
- `RENDER_SIZE` is 640x360 (`editor/src/visual/mount.ts:21`). Four outputs `o0..o3` are used at
  that shipping size. This is recorded as a deviation from the 15.3.7 "1024x1024" product
  workload.
- Perf hook from CANVAS-MOUNT (pinned in `impl-plans/active/canvas-cutover-mount.md`, section
  "`window.__vactrPerf` contract"). Under `?perf=1`, the page exposes
  `window.__vactrPerf { perf, revision(), ledger, doc(), selection(), presented(), onsets(), transportSample(), counters(), disposeCode() }`.
  The harness reads only these members, through `page.evaluate`, and never touches other
  internals.
- `editor/tsconfig.json` has `"types": []` and `allowJs: false`, and there is no `@types/node`.
  The vitest files in this plan therefore load the `.mjs` modules and `node:*` APIs only through
  the non-literal dynamic-import pattern of `editor/test/support/wasm.ts:14-24,67`
  (`(await import(/* @vite-ignore */ spec)) as LocalInterface`, with `spec` held in a variable),
  each cast to a locally declared interface. No `.d.mts` files are added. The `.mjs` scripts
  themselves are not type-checked, because `tsconfig` includes only `.ts`/`.tsx` sources plus
  `worklet/host.d.ts`.

## Non-goals

- No new npm or Cargo dependency, and no lockfile change. Only a `"e2e"` script line is added to
  `editor/package.json` `scripts`.
- No product-code change. If the harness finds a defect, record it as a failed check. Do not
  patch `editor/src` from this plan.
- No physical-device claims.

## Ownership

writePaths: see the manifest entry `CANVAS-EVIDENCE`. It covers the harness files, the evidence
document, the raw data files for run id `run-001`, `impl-plans/README.md`, and every closeout
move source and destination as concrete paths.

## Contracts

- `node editor/test/e2e/run.mjs [--browser chromium|webkit|all] [--profile behavior|measure|all] [--out <dir>] [--write-evidence] [--headed-webkit]`
- Exit codes:
  - `0`: every gated check passed;
  - `1`: a check failed or a threshold was missed;
  - `2`: blocked (browser missing, `dist` missing, or AudioContext never running for a profile
    that needs it). Blocked is never a pass.
- `--write-evidence` writes these files under
  `design-docs/specs/evidence/canvas-cutover/run-001/`:
  - `environment.json`: host, OS, browser versions, WebGL vendor and renderer strings,
    headless/headed mode, and audio state;
  - `chromium-behavior.json` and `webkit-behavior.json`;
  - `chromium-measure.jsonl` and `webkit-measure.jsonl`: per-sample records, under 2 MiB total;
  - `summary.json`.

  It also replaces the text between `<!-- EVIDENCE:BEGIN -->` and `<!-- EVIDENCE:END -->` in
  `design-docs/specs/design-canvas-editor-evidence.md`.
- `node editor/test/e2e/ios-sim.mjs --app <path.app> --device "<name>"` boots the simulator,
  installs, launches with `SIMCTL_CHILD_VACTR_SELF_CHECK=1`, captures the `VACTR_SELF_CHECK`
  line and a screenshot, and writes `run-001/ios-sim.json`. It exits 2 when there is no
  simulator.

## Tasks

### TASK-001: Harness core (serve.mjs, run.mjs, stats.mjs, stats.test.ts)

- `serve.mjs` serves `editor/dist` (built with `VACTR_REQUIRE_SESSION_ABI=1 npm run build`) on
  `127.0.0.1:0`, with correct MIME types for `.wasm` and `.js`.
- `stats.mjs` provides:
  - `percentile(values, p)` using the nearest-rank method;
  - `evaluate(summary, thresholds)`, where the thresholds are exactly the design 15.3.8.8 table;
  - `renderEvidence(summary)`, which returns the markdown for the marker section.

  It is pure, with no I/O.
- `stats.test.ts` (vitest, `// @vitest-environment node`) covers these cases:

| Situation | Expected outcome |
|-----------|------------------|
| `percentile([1..100], 95)` | 95 |
| Input p95 of 51 ms | Input check fails |
| Sync with provenance `estimate` | Reported, not gated |
| Sync with provenance `unavailable` | Listed under limitations |
| `renderEvidence` | Output contains every metric row and the run id; contains no non-ASCII characters |

### TASK-002: Workload generator (fixtures/large-doc.mjs)

- The generator uses a seeded PRNG (mulberry32, seed 265). It produces 20,000 lines of
  1.0 MiB plus or minus 5% of UTF-8, and asserts both values.
- The head is an evaluable program: a pattern layered to reach 64 concurrently sounding voices
  (record the actual peak from `playing` telemetry), plus four visual chains ending in
  `out o0`, `out o1`, `out o2` and `out o3`. Validate the program in
  `editor/test/e2e/large-doc.test.ts` (`// @vitest-environment node`) through the real Wasm
  `session_check` (imitate `editor/test/wasm/criteria.test.ts` loading), so it produces zero
  error diagnostics. The same test asserts the line count, byte size and character mix.
- The body is `#` comment lines plus inert `let` definitions. It includes Japanese, emoji, and
  at least 50 lines over 400 columns.

### TASK-003: Behavioral checks (behavior.mjs), per browser

Each check is pass, fail or limitation, with the reason. These are the design 15.3.8.8
behavioral checks:

1. **Canvas-only text.** No visible text node in `.vact-code` contains the document text. The
   bridge textarea computed color is transparent or opacity is 0. Canvas `readPixels` shows
   non-background pixels at the line-1 rectangle. Hiding the canvas leaves no visible source.
2. **Typing.** Typing, undo and redo, and word, line and document navigation change the
   document and selection as expected. Read them with `__vactrPerf.doc()` and
   `__vactrPerf.selection()`.
3. **Clipboard.** Chromium: grant permissions and use the real clipboard round trip. WebKit:
   synthetic `ClipboardEvent`, recorded as a limitation.
4. **IME.** Chromium uses CDP `Input.imeSetComposition` with the preedit
   U+306B U+307B U+3093 U+3054 ("nihongo" in hiragana), then `Input.insertText` U+65E5 U+672C U+8A9E
   ("nihongo" in kanji). Write these as JS escapes in the harness. Assert one transaction and no shortcut during composition.
   WebKit: synthetic composition events, recorded as a limitation.
5. **Touch.** `hasTouch` context: tap places the caret, and a long press selects a word
   (Chromium uses CDP `Input.dispatchTouchEvent`; WebKit uses synthetic pointer events, a
   limitation). Assert that `selection()` is the word and that the last `presented()` record has
   `handles === 2`.
6. **Context loss.** `WEBGL_lose_context`: lose, then restore. `doc()` is unchanged, and
   `counters().gpuStatus.kind` goes `context-lost` then `ready` (or `degraded`).
7. **DPR.** deviceScaleFactor 1 and 2. On Chromium, also apply a CDP
   `Emulation.setDeviceMetricsOverride` in the middle of the run.
   `counters().gpuStatus.effectiveDpr` updates and `selection()` is kept.
8. **Resize and keyboard inset.** Viewport resize, plus a simulated visual-viewport shrink: the
   caret rectangle (`selection().head` mapped through the bridge textarea position) stays inside
   the visible viewport.
9. **Backgrounding.** Synthetic `visibilitychange` to hidden for 2 s:
   - the `perf.snapshot().frames` count does not grow while hidden;
   - on return, the first `presented()` record's `activeKey` equals the analytic set computed
     from `onsets()` at that record's `audibleTime`;
   - no record shows a range whose onset `end <= audibleTime`.
10. **Dispose.** Keep `const ledger = __vactrPerf.ledger`, call `__vactrPerf.disposeCode()`,
    then assert `ledger.usedBytes === 0` and that `window.__vactrPerf` is undefined.

### TASK-004: Measurement profile H (measure.mjs)

- Phases: 10 s warmup, a 60 s editing run (at least 500 keystrokes at about 10/s, plus scrolling
  and selection), and a 120 s cycle run (edit, font, DPR and resize every 5 s, with
  `page.evaluate` busy-waiting 250 ms every 10 s as the injected stall).
- Video: `MediaRecorder` from a canvas, about 3 s at 1280x720, loaded through the visual pane
  file input path (`setInputFiles` with the Blob saved to a temp file) or through `VideoBackground.load`.
- Metric definitions. Every metric uses only `__vactrPerf` members, CDP, or page-level
  Playwright APIs:

| Metric | Source and computation |
|--------|------------------------|
| Input latency | `perf.snapshot().keys` `[timeStamp, revision]` and `frames` `[frameMs, workMs, text, revision]`. For each key, find the first frame with `revision` greater than the key's revision; latency is the next frame's `frameMs` (the presentation proxy) minus `timeStamp`. IME latency is reported separately (Chromium CDP composition keys). |
| Frame work and interval | `frames[i].workMs`, split by `text`; the interval is the delta between consecutive `frameMs`, excluding the frames immediately after an injected stall |
| A/V sync (model) | For each `onsets()` record `o`, take the first `presented()` record `p` whose `activeKey` contains `o.from-o.to` and whose `p.audibleTime >= o.time`. Then `onsetPageMs = p.targetMs + (o.time - p.audibleTime) * 1000`, and `error = next record's frameMs - onsetPageMs`. Gated only when `p.provenance === 'measured'`. An early flash is any record containing the range with `audibleTime < o.time - 0.002`. |
| Late frames | A stall is a frame interval over 200 ms. For the first `presented()` record after each stall, compare `activeKey` with the analytic set (`onsets()` entries with `time <= audibleTime < end`, keyed `from-to`). Replayed flashes count records showing a range whose `end <= audibleTime`. Beat drift is `beatCycle` minus the analytic cycle from `transportSample()` (`cycle + (audibleTime - sample_time) * bpm / 60 / beats_per_cycle`) at the end of the run. |
| Memory | `counters().ledger` maximum per cap and `counters().usedBytes`; Chromium heap through CDP `HeapProfiler.collectGarbage` then `Runtime.getHeapUsage`; WebKit is ledger-only (a limitation) |
| Audio | Worklet underrun and drop counters if exposed by the page, otherwise "not exposed" (a limitation). Also copy `counters().highlight`, `counters().client` and `counters().probe` into `summary.json`. |
- Fallbacks:
  - if WebKit headless lacks WebGL2, rerun WebKit with `--headed-webkit` and record the mode;
  - if the AudioContext is not running after the gesture, mark sync and audio as unavailable,
    and give the profile exit 2 only when Chromium is affected.

### TASK-005: Evidence document (design-docs/specs/design-canvas-editor-evidence.md)

Follow the `design-doc` skill format. The document has these sections:

- purpose and design link (15.3.8.8);
- an environment section, generated from `environment.json`;
- the generated results section between the markers. Before the run it reads
  "PENDING RUN: no results recorded".
- raw data paths;
- platform limitations, including:
  - native-tier Hydra unavailable;
  - native-tier and iPad video background composition unavailable, because `GlRenderHost`
    exists only on the browser tier (`editor/src/visual/mount.ts:59-66`);
  - headless audio, IME, clipboard and touch emulation limits;
  - simulator performance not representative;
  - the 640x360 visual output size deviation;
  - cpal host-reported latency semantics;
- the pending physical-iPad checks, each with an exact procedure, equipment and pass criterion:
  - Japanese hardware and software keyboard IME;
  - touch handles;
  - VoiceOver;
  - audio unlock;
  - interruption and resume (phone call or Siri);
  - route change (headphones and Bluetooth) with latency re-provenance;
  - orientation;
  - 120 Hz ProMotion frame time;
  - 10-minute sustained thermal animation;
  - camera and microphone A/V sync capture with the 15.3.7 thresholds.

### TASK-006: Closeout (after the verification run and final gates)

- Move each superseded file from `impl-plans/active/` to `impl-plans/completed/` as concrete
  paths (git mv semantics: create the destination with identical content plus the first line
  `> Superseded 2026-10 by the canvas-cutover plans (design 15.3.8); historical only.`, then
  delete the source). The files:
  - `canvas-editor-224-clock.md`, `-consumers.md`, `-contracts.md`,
    `-dependency-evidence.md`, `-editor-join.md`, `-execution.md`, `-gpu.md`, `-input.md`,
    `-native-clock.md`, `-native-shell.md`, `-package-preparation.md`, `-state.md`,
    `-telemetry.md`, `-verification.md`, `-visual.md`;
  - `canvas-editor-224-dispatch.json`. JSON cannot hold a comment line, so add a top-level
    `"supersededNote"` string instead.
- Move the seven `canvas-cutover-*.md` plans to `impl-plans/completed/` with Status
  `Completed`, after their criteria are checked. `canvas-cutover-dispatch.json` stays in
  `active/` for the workflow record.
- `impl-plans/README.md`: add the completed canvas-cutover entries and remove the active
  canvas-editor-224 rows, if any. Change nothing else.
- Design erratum (one paragraph, appended at the end of
  `design-docs/specs/design-implementation.md` 15.3.8.4, nothing else edited). It states that
  the implemented `clock-probe` wire keeps the pre-existing field names: request
  `{ page_send }` (ms); reply `clock-probe { page_send, engine_receive, engine_send, epoch, correlation?, latency_seconds, latency_kind, uncertainty_seconds }`,
  with `processing_time = (engine_receive + engine_send) / 2`. This is equivalent to the
  amendment's `page_time_ms`/`processing_time`/`clock-probe-reply` naming.

## Pitfalls

- Committing numbers typed by hand. Only harness output goes between the markers.
- Treating a blocked run (exit 2) or a timeout as a pass.
- Claiming physical-device, real IME hardware, VoiceOver or acoustic results.
- Exceeding 2 MiB of committed raw data. Downsample per-sample records if needed and say so in
  `summary.json`.
- Listing directories instead of files for the moves.
- Reaching into product internals other than the pinned `__vactrPerf` members, or editing
  `editor/src` to add a hook. A missing datum is recorded as a limitation.
- A literal `import 'node:fs'` or `import './stats.mjs'` in the `.ts` tests, adding
  `@types/node`, editing `tsconfig.json`, or using `@ts-nocheck`.

## Session 266 Amendment (operator decisions A and D)

- **Artifact roots** (manifest `CANVAS-EVIDENCE.artifactRoots`, each also a writePath; all
  gitignored and never committed):
  - `target`, `tree-sitter-vact/tree-sitter-vact.wasm`;
  - `tmp/canvas-cutover/evidence` and `tmp/canvas/evidence` (full logs, per design 15.3.8.8);
  - `editor/node_modules/.vite`, `editor/dist`;
  - `editor/src-tauri/target`, `editor/src-tauri/gen/schemas`;
  - `editor/src-tauri/gen/apple/build` and `editor/src-tauri/gen/apple/Externals` (only read or
    refreshed when the iPad simulator `.app` is reused or rebuilt).
- **No `editor/test-results` or `editor/playwright-report`.** The harness uses the `playwright`
  library from plain Node scripts. `@playwright/test` is not installed, and these directories are
  not produced. If either appears, that is a harness defect: record it and do not commit it.
- **Committed evidence stays writePaths.** That covers `design-docs/specs/evidence/canvas-cutover/run-001/*`
  and the evidence document. Raw committed data stays at or under 2 MiB.
- **Setup (not gating)**: `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`,
  then the host-wasm library build.
- **Rule D**: the gating list contains only final-source commands that are expected to pass.
  Harness self-tests that intentionally fail go under `mutationEvidence`. A measurement threshold
  miss is a recorded failure in the evidence document and is never relabeled as a pass. Exit 2
  (blocked) is never a pass.

## Verification

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/e2e` | stats and generator tests pass |
| `node --check` on each `editor/test/e2e/*.mjs` | exit 0 |

Outside the sandbox (verification and review step; logs in `tmp/canvas-cutover/evidence/`):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 |
| `cd editor && npm run e2e -- --browser all --profile all --write-evidence` | exit 0, or exit 1 with the failures written to the evidence file (a missed threshold is reported, never hidden); exit 2 is blocked, never a pass |
| `node editor/test/e2e/ios-sim.mjs --app <SHELL .app> --device "<iPad simulator>"` | `ios-sim.json` with the self-check line |

Final gates on the closeout commit (design 15.3.8.9):

| Command | Required evidence |
|---------|-------------------|
| `cargo build` | exit 0 |
| `cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Full nextest (timeout >= 1500 s) | all pass |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | all pass |
| `cd editor && ./node_modules/.bin/vitest run test/canvas/no-editor-view.test.ts` | pass (zero `@codemirror/view` or `EditorView` in production `editor/src`) |
| e2e | as above |
| `cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |
| `rustfmt --check` on touched Rust files | exit 0 |

## Overwrite and Drift Protocol

Record fresh-read sha256 values before each edit or move (`tmp/canvas-cutover/evidence/intent.json`)
and after it (`receipt.json`). Closeout runs serially, after all other plans are accepted. Edit
only this plan's progress log and the plan files being archived.

## Completion Criteria

- [x] Harness and unit tests in place; `run.mjs` preserves exit 0/1/2 semantics and fails on any measurement failure.
- [x] Full 20,000-line fixture is checked through host-WASM `session_check`, with exactly one check record and zero error diagnostics.
- [x] Evidence document has the harness-generated results, raw paths, explicit browser limitations, paired-edit counts, and pending physical-iPad procedures.
- [x] Active-workload attribution is control-backed: the built-in pad control emits an onset in both browsers; the same toolbar click times out on the 20,000-line document, which records zero workload onset and a product/full-document startup failure. Editing keystrokes and paired samples are reported separately, with the 500-key gate applied to editing keys.
- [x] Simulator evidence recorded: iPad Pro 11-inch (M5) launched and emitted the native-tier self-check
- [ ] Closeout moves done file by file; README updated (serial closeout after review)
- [ ] Final gates recorded with exit codes and log paths (closeout gate)

## Progress Log

### Session: 2026-10-05
**Tasks Completed**: Plan authored

### Session: 2026-10-05 (session 266 plan amendment)
**Tasks Completed**: Plan amended per operator decisions A and D. Artifact roots declared; the final-gate wasm row aligned to the host-wasm build; the explicit no-editor-view guard row added; setup separated from gating. Tasks, contracts and closeout list are unchanged.

### Session: 2026-10-05 (CANVAS-EVIDENCE implementation)
**Tasks Completed**: Playwright-library harness, workload and unit tests; generated Chromium/WebKit behavior and profile-H evidence; iPad simulator launch evidence. `npm run e2e -- --browser all --profile all --write-evidence` returned 1 as designed because required checks and thresholds exposed current product failures. Final browser run observed 583 Chromium and 550 WebKit key events, canvas backing 1x1 in Chromium, no canvas glyph readback in WebKit, missing long-press selection handles in both, Chromium DPR remaining 1, 5.77s/5.64s input p95, 114.2ms/96ms text-work p95, no measured sync samples and no playing onset telemetry. Raw files and the generated results section are at `design-docs/specs/evidence/canvas-cutover/run-001/`. The iPad Pro 11-inch (M5) simulator self-check succeeded. No product code was changed. Remaining archive moves, README update, clock-probe erratum, formal reviews and closeout gates belong to serial downstream closeout.

### Session: 2026-10-05 (test-integrity repair, comm-003981)
**Tasks Completed**: Corrected the workload to valid vact definitions and comments; the full 20,000-line/1 MiB document now has one parsed `session_check` record and zero error diagnostics, asserted by `large-doc.test.ts`. Added tested bounded key/frame pairing with editing snapshots, a paired-sample >=500 gate, and onset/failure-aware exit status. Added behavioral assertions for undo/redo, navigation, context loss/restore, single-transaction Chromium IME and a synthetic visual-viewport inset. WebKit synthetic clipboard and IME are explicit limitation statuses excluded from pass/fail counts and from generated failed-check text. `stats.mjs` renders the paired-edit count.

**Final-source verification**: `node --check ...` exit 0 (`tmp/canvas-cutover/evidence/ti-node-check-final.log`); `cd editor && ./node_modules/.bin/vitest run test/e2e` 13/13 exit 0 (`ti-vitest-e2e-final.log`); `cd editor && npm run check` exit 0 (`ti-npm-check-final.log`); `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` exit 0 (`ti-abi-build-final.log`). Full browser command `cd editor && npm run e2e -- --browser all --profile all --write-evidence` exited 1, not blocked (`ti-e2e-final.log`): Chromium 8/10 behavior checks and 40 paired editing keys; WebKit 6/8 with two synthetic limitations excluded and 22 paired editing keys. Both reported audio running with zero onsets/playing ranges; Chromium's run shortcut timed out at 120 seconds. Current performance thresholds also fail, as captured in `run-001/summary.json` and generated results. The full run result section was regenerated through `stats.mjs` (`ti-render-evidence-final.log`).

**Correction to earlier evidence**: The previous session's fixture-based diagnosis and profile numbers were invalid because the generated document contained parse errors and only its head had been checked. Do not reuse those numbers; current files under `design-docs/specs/evidence/canvas-cutover/run-001/` are the regenerated source-matched evidence. No product code was changed; browser and playback defects remain recorded findings for downstream disposition.

**Downstream pending**: Independent Opus re-review of EVID-TI-001 through EVID-TI-005, serial closeout archival/README/clock-probe erratum, combined-tree final gates, and commit/push remain owned by later workflow steps.

### Session: 2026-10-05 (test-integrity repair, comm-003984)
**Tasks Completed**: Replaced the sample-bank `bd` workload with the built-in `pad` synth (64 voices) and added a same-page head-only control using the same `.vact-run` toolbar click. The control produced one onset in Chromium and WebKit; the full-document click timed out in both, so control records are excluded from workload onset/playing-range counts and the evidence records a diagnosed large-document startup failure. The results include the control count, start method/status and attribution. Kept the Chromium CDP session attached through `dpr-and-resize` assertions and through the cycle loop; Chromium cycle samples alternate DPR 1/2. Changed the 500 floor to `editKeyCount`, retained `editPairedKeyCount`, and gated paired samples >0 plus unpaired <=10%; stats tests cover 499 fail, 500/100 pass, zero pairs fail and >10% unpaired fail. EVID-TI-002 through EVID-TI-005 were retained.

**Final-source verification**: `node --check editor/test/e2e/run.mjs editor/test/e2e/serve.mjs editor/test/e2e/behavior.mjs editor/test/e2e/measure.mjs editor/test/e2e/stats.mjs editor/test/e2e/ios-sim.mjs editor/test/e2e/fixtures/large-doc.mjs` exit 0 (`tmp/canvas-cutover/evidence/ti-source-check.log`); `cd editor && ./node_modules/.bin/vitest run test/e2e` 15/15 exit 0 (`ti-source-vitest.log`); `cd editor && npm run check` exit 0 (`ti-source-npm-check.log`). `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` exit 0 (`ti-rerun-editor-build.log`). The head-only Chromium control command logged `controlOnsetCount=1`, `audioState=audio running`, `controlStartMethod=toolbar-click` (`ti-audio-control-chromium.log`). Final `cd editor && npm run e2e -- --browser all --profile all --write-evidence` exited 1, not blocked (`ti-final-e2e-browser-v2.log`): 18 behavior checks, 15 passed and 3 failed; both controls emitted one onset, both large-document toolbar clicks timed out, and both workload onset counts/playing ranges are zero. Chromium DPR behavior observed actual/effective 2; its cycle samples alternate 1/2. Editing counts are Chromium 253 with 37 paired and 0 unpaired, WebKit 187 with 32 paired and 0 unpaired; the 500-key floor and product latency/frame/sync thresholds correctly remain failed. The full failure list and raw source-matched samples are in `run-001/summary.json`, `chromium-measure.jsonl`, `webkit-measure.jsonl`, and generated marker section.

**Prior attempt disposition**: `ti-rerun-e2e-browser.log` exited 1 before measurement because the toolbar was absent when audio state was queried after the full-document load. The corrected run captures audio state immediately after the user gesture; the final run completed measurement and produced current run-001 artifacts. `ti-final-e2e-browser.log` (before isolating control onset records) also exited 1 and was superseded by the final artifact-producing run; both logs are retained.

**Remaining product evidence**: The active control confirms the voice and start method work. The large-document toolbar click still times out after Playwright dispatches the action; classify as full-document startup failure. Threshold failures, including fewer than 500 editing keys in the fixed 60-second window, remain recorded failures and were not relabeled as passes. No editor product source was changed.

**Downstream pending**: Independent Opus review of the current repairs, serial closeout archival/README/clock-probe erratum, combined-tree final gates and commit/push remain owned by later workflow steps.
