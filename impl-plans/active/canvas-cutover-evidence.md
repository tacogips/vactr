# Canvas Cutover: Real-Browser Evidence, Measurements and Closeout Implementation Plan

**Status**: In Progress (session 293: redispatched under the "Session 302 Amendment", TASK-701 to TASK-706, design 15.3.8.15; TASK-601 is superseded by TASK-703)
**Plan ID**: CANVAS-EVIDENCE (dispatch wave 16 since session 291; every dependency is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.8 (measurement protocol and thresholds, including the session-267 silent automated audio rule), 15.3.8.9 (gates and closeout), 15.3.8.11 (serial perf gate), 15.3.8.12 (session-274 evidence repair wave), 15.3.8.13 (session-277 release-wasm measurement build and repair scope; "Session 285 scope record" for the edit-path repair), 15.3.8.14 (session-286 performance wave), 15.3.8.15 (session-293 stall-window sync classification and transport-sample pairing)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-07

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

## Session 267 Amendment (operator decision S, open EVID-TI findings, owner-file defect fixes)

This section supersedes the earlier text wherever they conflict.

- **Split.** The remaining harness repairs and product-defect fixes move to four wave-3 plans
  that run in parallel:
  - `impl-plans/active/canvas-cutover-evidence-silent.md` (CANVAS-EVIDENCE-SILENT): silent
    virtual sink and `--mute-audio`; workload `amp` 0.005; head-stack control; EVID-TI-001,
    -006 and -007 alignment; in-frame readback; WebKit `isPrimary` touch; keystroke pacing;
    `ios-sim.mjs` and the `playingEvents` self-check; native silence audit.
  - `impl-plans/active/canvas-cutover-evidence-viewport.md` (CANVAS-EVIDENCE-VIEWPORT):
    `frame.ts` first-viewport delivery. This fixes the Chromium 1x1 backing.
  - `impl-plans/active/canvas-cutover-evidence-editcost.md` (CANVAS-EVIDENCE-EDITCOST): no
    whole-document string work per keystroke or per animation frame.
  - `impl-plans/active/canvas-cutover-evidence-runstart.md` (CANVAS-EVIDENCE-RUNSTART): the
    large-document Run start stall.

  This plan becomes wave 4. It owns only the outside-sandbox evidence run, the evidence
  document, the final integration review inputs, and closeout.
- **Non-goal change.** The non-goal "No product-code change" is superseded by design 15.3.8.9.
  Product defects are fixed in their owner files by the wave-3 plans. If the wave-4 run exposes
  a further product defect, it goes back to the owning wave-3 plan for repair (or to a serial
  repair recorded in the manifest). It is never recorded as acceptable.
- **Harness ownership.** The harness files are writePaths of CANVAS-EVIDENCE-SILENT in wave 3.
  Here they are sharedPaths, edited only in serial wave-4 repair after a review finding, with the
  edit recorded in the progress log.
- **Run id.** The silent run overwrites `run-001` (`--run-id run-001`). The audible session-266
  data is replaced, and the evidence document records that run-001 was regenerated silently in
  session 267.

### TASK-007 (wave 4): Silent evidence run (outside the sandbox, by the verification step)

The run is preceded by the setup builds and `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`.

- `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001`.
  The log goes to `tmp/canvas-cutover/evidence/s267-e2e.log`.
- `node editor/test/e2e/ios-sim.mjs --app <SHELL .app> --device "iPad Pro 11-inch (M5)"`. The log
  goes to `tmp/canvas-cutover/evidence/s267-ios-sim.log`. Pass requires the self-check line and
  `playingEvents === 0`.
- Required evidence in `summary.json`, per browser:
  - sink `installedBeforeFirstConnect: true`;
  - `directDestinationConnections: 0`;
  - post-sink peak exactly 0;
  - workload pre-sink peak <= -1 dBFS;
  - control >= 1 onset with peak above -60 dBFS;
  - large-document start returned and workload onsets > 0;
  - Chromium canvas backing larger than 1x1;
  - Chromium cycle samples showing DPR 1 and 2.

### TASK-008 (wave 4): Evidence document update

Edit the static sections of `design-docs/specs/design-canvas-editor-evidence.md`. The marker
section is written only by the harness.

- Add an "Automated audio silence" section. It states that every automated run was silent and
  describes the method:
  - the init-script virtual sink (pre-sink AnalyserNode, then zero gain, then post-sink
    AnalyserNode, then the real destination);
  - Chromium `--mute-audio`;
  - the numeric assertions;
  - that macOS volume was never read or changed and no audio driver was installed.

  It also states how levels were measured: fftSize 32768, 100 ms polling of new samples only, and
  the 10 ms block RMS onset rule (on at -40 dBFS after 50 ms below -50 dBFS). It cites the
  measured pre-sink values from `summary.json` by path, never as hand-typed numbers.
- Add a "Native and simulator silence" paragraph:
  - the TASK-005 audit result from CANVAS-EVIDENCE-SILENT, with command log paths;
  - the simulator `playingEvents: 0` evidence path;
  - a statement that no Tauri desktop automation exists (the desktop shell is verified by
    `cargo check` only).
- Update the limitations:
  - add the SwiftShader software-GL line if the renderer string still shows it;
  - remove limitations that the wave-3 fixes resolved (for example the WebKit synthetic touch
    failure, if it now passes).
- Add a "Remaining failures triage" table. Each gated failure still present after the wave-3
  fixes gets one row: metric or check, value, classification (`product defect` -> repair
  required before acceptance; `environment limitation` such as software GL), and the evidence
  path.

### TASK-009 (wave 4, serial): Final integration review and closeout

- The final integration review covers all seven canvas-cutover plans plus the four wave-3
  evidence plans.
- TASK-006 closeout additionally moves these four plans file by file to `impl-plans/completed/`
  with Status `Completed`:
  - `impl-plans/active/canvas-cutover-evidence-silent.md`
  - `impl-plans/active/canvas-cutover-evidence-viewport.md`
  - `impl-plans/active/canvas-cutover-evidence-editcost.md`
  - `impl-plans/active/canvas-cutover-evidence-runstart.md`
- `impl-plans/README.md` lists all eleven completed canvas-cutover plans.
- Commit and non-force push to `origin wf/canvas`.

## Session 269 Amendment (operator authorizations 3, 5 and 6; design 15.3.8.9, 15.3.8.10)

This section supersedes the earlier text wherever they conflict.

- **Order.** This plan starts only after CANVAS-EVIDENCE-EDITCOST and CANVAS-EVIDENCE-RUNSTART
  are accepted. CANVAS-EVIDENCE-SILENT and CANVAS-EVIDENCE-VIEWPORT were accepted in session 267
  (commit `6f6b807`) and are not redispatched. TASK-007 to TASK-009 are otherwise unchanged.
- **Workload and silence.** TASK-007 is unchanged. The run uses the 20,000-line, about 1 MiB
  document (at least 5,000 lines), 64 voices at amp 0.005, Hydra, scopes and the 720p video.
  Per browser, the post-sink peak is exactly 0, and the pre-sink peak dBFS, RMS and onset counts
  and times are reported. The evidence document is regenerated by the harness only
  (`--write-evidence --run-id run-001`). The iPad simulator runs where feasible. Otherwise the
  exact blocker is recorded as a platform limitation.
- **Pre-authorized product seams (operator authorization 3).** The files listed below are
  sharedPaths of this plan, in addition to the session-267 harness sharedPaths. Edit one only
  when all of these hold:
  1. The wave-4 run exposes a product defect in that file, as a gated check or threshold
     failure classified `product defect` in the TASK-008 triage table.
  2. The fix is the smallest change in that file, plus its listed test file, that removes the
     defect.
  3. Before the edit, the fresh-read sha256 and the triage row are recorded in the progress
     log. After the edit, the post-edit sha256 is recorded.
  4. The owning plan's tests and the full vitest suite stay green, and the run is repeated.

  A defect in any other file goes back to its owning accepted plan by selective redispatch, with
  a concrete writePaths amendment and a changed manifest fingerprint (design 15.3.8.10). It is
  never recorded as acceptable.
  - Editing cost and input latency (owner: EDITCOST and MOUNT):
    - `editor/src/code/mount.ts`, `editor/src/code/renderer.ts`, `editor/src/code/layout.ts`
    - `editor/src/code/input.ts`, `editor/src/code/keyboard.ts`, `editor/src/code/accessibility.ts`
    - `editor/src/code/sync.ts`, `editor/src/code/syntax.ts`, `editor/src/code/syntax-core.ts`,
      `editor/src/code/history.ts`
    - tests: `editor/test/canvas/mount.test.ts`, `editor/test/canvas/gpu.test.ts`,
      `editor/test/canvas/input.test.ts`, `editor/test/canvas/edit-cost.test.ts`,
      `editor/test/code/sync.test.ts`, `editor/test/code/syntax-fallback.test.ts`
  - Frame scheduling (owner: VIEWPORT and MOUNT): `editor/src/code/frame.ts`, with
    `editor/test/canvas/frame.test.ts`
  - Touch long-press selection and handles (owner: RENDER and MOUNT): `editor/src/code/pointer.ts`,
    with `editor/test/canvas/input.test.ts` (listed above)
  - Playing highlights, A/V sync records and the beat indicator (owner: CLOCK):
    - `editor/src/code/highlight.ts`, `editor/src/app/clock.ts`, `editor/src/code/transport.ts`,
      `editor/src/code/perf-hook.ts`
    - tests: `editor/test/code/highlight.test.ts`, `editor/test/canvas/clock.test.ts`,
      `editor/test/code/transport.test.ts`
  - Run start (owner: RUNSTART): `editor/src/code/eval.ts`, with `editor/test/code/eval.test.ts`
  - Per-change whole-text `Utf8Index` in binding code (DR-269-L2; owner: MOUNT):
    `editor/src/bind/mount.ts`, `editor/src/bind/write.ts`, with
    `editor/test/bind/write.test.ts`
  - Visual composition (owner: VISUAL):
    - `editor/src/visual/frame.ts`, `editor/src/visual/scopes.ts`, `editor/src/visual/video.ts`,
      `editor/src/visual/render-host.ts`
    - tests: `editor/test/visual/frame.test.ts`, `editor/test/visual/scopes.test.ts`,
      `editor/test/visual/video.test.ts`, `editor/test/visual/render-host.test.ts`
- **Seam limits.** A seam edit must not:
  - change a public contract pinned by an accepted plan (`CodeSurface`, `DocumentSync`,
    `__vactrPerf` members, the Session Protocol);
  - add a dependency;
  - touch Rust;
  - weaken a test assertion;
  - change a threshold.
  A defect that needs any of these goes to selective redispatch.
- **Re-measurement.** The TASK-007 run re-measures the EDITCOST targets (text-dirty work p95,
  input latency, editing-key count) and the RUNSTART target (the large-document toolbar click
  returns within 10 s, and workload onsets are above 0) in both browsers.
- **Design file.** `design-docs/specs/design-implementation.md` already contains the session-269
  15.3.8.10 amendment, committed at the plan checkpoint. The closeout edit to this file is still
  only the 15.3.8.4 clock-probe erratum paragraph.

## Session 271 Amendment (operator decision P; design 15.3.8.11)

This section supersedes the earlier text wherever they conflict.

- **Order.** Dispatch wave 5. This is the design's "wave-4 CANVAS-EVIDENCE"; the number changed
  only because RUNSTART (wave 3) and EDITCOST (wave 4) now run one after the other. This plan
  starts after both are accepted. TASK-007 to TASK-009, the seam protocol and the closeout lists
  are unchanged.
- **`editor/package.json`.** Moves from writePaths to sharedPaths, limited to `scripts.e2e`, which
  already exists, so no edit is expected. Never edit `scripts.test:perf`; RUNSTART owns it.
  `editor/vitest.config.ts` and `editor/test/e2e/large-eval*.ts` are not owned by this plan.
- **In-sandbox `vitest run test/e2e`.** The default config excludes
  `test/e2e/large-eval.perf.test.ts`. This is expected and is not a missing test.
- **Final gates.** Add `cd editor && npm run test:perf`, run alone with no other test, build or
  browser process active. It must exit 0. The full-vitest gate is the default
  `./node_modules/.bin/vitest run` (perf files excluded), which must exit 0.
- **Design file.** 15.3.8.11 is committed at the session-271 plan checkpoint. The closeout edit to
  `design-docs/specs/design-implementation.md` is still only the 15.3.8.4 clock-probe erratum
  paragraph.
- **README.** `impl-plans/README.md` lists all eleven completed canvas-cutover plans, as before.

## Session 274 Amendment (operator decisions A and B; design 15.3.8.12)

This section supersedes all earlier text wherever they conflict, including the "No product-code
change" non-goal, the session-269 selective-redispatch rule and the Completion Criteria line that
routed the simulator repair to CANVAS-SHELL.

### Intent and context

The session-271 integration review (`loopGate.decision = needs-work`) rejected this plan with
five high findings. Their full briefs are in `tmp/canvas-cutover/s271-integration-review.json`
(gitignored, read-only, present in the worktree). That file is authoritative if any wording below
differs, except where this plan states an explicit operator or review override: the
INT-S271-EV-PERF operator override in TASK-405, and the TASK-402 sync-window override
(finding S274-PLAN-SYNC-WINDOW). Where an override applies, the plan text wins. Session 271 then failed at dispatch because the brief paths `editor/src/code/pointer.ts`,
`editor/test/e2e/behavior.mjs`, `editor/test/e2e/ios-sim.mjs` and `editor/test/e2e/measure.mjs`
were outside this plan's writePaths. In session 274, the plan author amended the manifest
serially, before redispatch.

- Every `allowedWritePaths` entry of the five briefs is now a concrete writePath of this plan:
  the harness, 23 product seams, 38 existing seam tests, the evidence document, the seven
  run-001 files and this plan.
- The artifact roots `editor/dist`, `editor/src-tauri/target`, `editor/src-tauri/gen/apple/build`
  and `tmp/canvas-cutover/evidence` remain.
- sharedPaths keep only `editor/package.json`, `editor/test/e2e/silent-sink.mjs`,
  `editor/test/e2e/fixtures/large-doc.mjs` and `editor/test/e2e/large-doc.test.ts`. No edit to
  these is expected.

The user goal does not change. Real-browser evidence must pass the accepted 15.3.8.8 gates
silently, with honest limitations. Then come the final integration review, the closeout and the
push.

### Ownership

CANVAS-EVIDENCE is the single active owner of every listed harness file, product seam and seam
test for this wave. All ten other canvas-cutover plans are accepted and are not redispatched:
CLOCK, NATIVE, RENDER, MOUNT, VISUAL, SHELL, EVIDENCE-SILENT, -VIEWPORT, -EDITCOST and -RUNSTART.
The concrete file list is the manifest entry `CANVAS-EVIDENCE` in
`impl-plans/active/canvas-cutover-dispatch.json`. The manifest `seamProtocol` governs every
product-seam edit:

- a triage row in the evidence document;
- fresh-read and post-edit sha256 values in `tmp/canvas-cutover/evidence/intent.json` and
  `receipt.json`, plus the progress log;
- the smallest change;
- a new operation-counter or deterministic row in the listed test;
- no public contract change (`CodeSurface`, `DocumentSync`, `PointerController`,
  `__vactrPerf` member names and shapes, Session Protocol);
- no Rust edit and no dependency;
- no weakened assertion or threshold.

**Unlisted dominant cost.** If a profile shows the dominant cost in a TypeScript file that is
not listed, do not edit it. Record the exact path, its prior owner plan and its measured share of
main-thread self time in the progress log and the triage table, then continue with the listed
seams. The plan author adds that path serially and checkpoints. A cost that needs Rust, a public
contract change or a dependency is reported as an operator blocker with that evidence. It is
never recorded as acceptable.

### Non-goals

- No change to any `THRESHOLDS` constant in `editor/test/e2e/stats.mjs`, to the "measured only"
  sync gating rule, or to the run-level `measured sync samples unavailable` failure
  (`stats.mjs:120`).
- No workload change. These stay fixed:
  - the 20,000-line, about 1 MiB document;
  - 64 voices at amp 0.005;
  - four synth outputs at the shipping `RENDER_SIZE`;
  - the 720p video;
  - active scopes;
  - keystroke pacing of about 10/s.
- No `preserveDrawingBuffer: true` in production (`editor/src/code/renderer.ts:97`).
- No edit to SHELL-owned files (`editor/src-tauri/src/*.rs`, `editor/src/app/main.ts`), no
  signing, provisioning or upload, and no device build.
- No change to the silent sink, the post-sink-peak-0 assertion or the production master output.
  The macOS system volume is never read or changed.
- No Rust edit, no new dependency, no lockfile change and no crate-wide `cargo fmt`.

### Execution split (sandbox versus outside)

The Codex sandbox cannot bind localhost and keeps `.git` read-only. Playwright runs, simulator
build and launch, CDP profiles and full nextest therefore run in the verification and review
steps outside the sandbox. The fix order is fixed by design 15.3.8.12:

1. **Iteration 1 (implementer, in the sandbox).** Do TASK-401 to TASK-405 below: harness fixes,
   pure functions and their unit tests, the opt-in profiler and the WebKit control page. Then
   run the in-sandbox checks. Product seams are edited in this iteration only for:
   - TOUCH, when the jsdom reproduction of the recorded WebKit event sequence fails in
     `pointer.ts`;
   - the additive `PresentedRecord.epoch` field (TASK-402).
2. **Verification 1 (outside the sandbox).** Run the ABI build, the full e2e with
   `--write-evidence --run-id run-001`, the Chromium `--profile-trace` diagnostic, the simulator
   rebuild and `ios-sim.mjs`. Logs go to `tmp/canvas-cutover/evidence/s271b-*.log`.
3. **Iteration 2 and later (implementer).** Work from the recorded profile ranking and the new
   numbers. Do TASK-406 (PERF seam fixes) and any READBACK or TOUCH product fix the corrected
   probes prove necessary. Then re-measure. Repeat until the gates pass, or until a precise
   non-seam blocker is recorded.
4. Then TASK-407 (evidence document), TASK-408 (final gates) and TASK-409 (final integration
   review, closeout, commit and non-force push).

### TASK-401: INT-S271-EV-SIM (harness only)

**Brief (verbatim from `tmp/canvas-cutover/s271-integration-review.json`).**

- Reproduction: Run `node editor/test/e2e/ios-sim.mjs --app editor/src-tauri/gen/apple/build/arm64-sim/Vactr.app --device "iPad Pro 11-inch (M5)"`. Current result: `ios-sim.json` has `selfCheck=null` and a `selfCheckLine` taken from the `log` process; `tmp/canvas-cutover/evidence/s271-ios-self-check-process-filter-10m.log` shows the app line without `playingEvents`. Expected: the parsed self-check JSON with `playingEvents === 0` and `pass=true`.
- Root cause: Confirmed. (a) The predicate is not restricted to the app process, so the log tool's own argv line matches first. (b) The `.app` was built before the `playingEvents` field existed. `strings` found 0 occurrences. Frontend assets may be embedded compressed, but the mtime alone proves the binary predates 6f6b807.
- Ordered changes:
  1. In `ios-sim.mjs`, change the log predicate to also require `process == "Vactr"` (for example: `process == "Vactr" AND eventMessage CONTAINS "VACTR_SELF_CHECK {"`). In addition, select only lines that contain 'Vactr' as the process, so a 'log' process line can never be selected. The direct query already recorded in `s271-ios-self-check-process-filter-10m.log` is the proven pattern.
  2. Make the selected line the most recent matching line from this launch, not the first. Use the `--last` window after the launch timestamp, or pick the last match. This prevents a stale earlier launch's line from being parsed.
  3. Rebuild the frontend with `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`, then rebuild the simulator app from current source with the documented path `tmp/canvas/tools/cargo-tauri ios build --target aarch64-sim` (debug, unsigned, no upload). Confirm that the resulting `Vactr.app` mtime is newer than the build start.
  4. Rerun `ios-sim.mjs` and record the log under `tmp/canvas-cutover/evidence/`.
  5. Update the simulator row of the triage table in the evidence document. Remove the CANVAS-SHELL attribution.
- Forbidden changes: Do not edit `editor/src-tauri/src/*.rs`, `editor/src/app/main.ts` or any SHELL-owned file. No signing, provisioning, upload or device builds; unsigned simulator build only. Do not start playback or evaluate a document in the simulator. Do not drop the `playingEvents === 0` assertion or treat a null selfCheck as a pass.
- Completion criteria:
  - [ ] `run-001/ios-sim.json` has a non-null `selfCheck` parsed from a Vactr-process line, with `playingEvents === 0`, `silent === true` and `pass === true`.
  - [ ] The app binary mtime is later than 6f6b807.
  - [ ] The evidence triage table no longer attributes this to CANVAS-SHELL.
  - [ ] Only allowedWritePaths changed, with sha256 values recorded in the plan progress log.
- Verification (implementer, outside-sandbox items by the verification step):
  - `node --check editor/test/e2e/ios-sim.mjs` (exit 0)
  - `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` (exit 0)
  - `tmp/canvas/tools/cargo-tauri ios build --target aarch64-sim`, with the cwd and flags documented in `canvas-cutover-shell.md` (exit 0)
  - `node editor/test/e2e/ios-sim.mjs --app editor/src-tauri/gen/apple/build/arm64-sim/Vactr.app --device "iPad Pro 11-inch (M5)" > tmp/canvas-cutover/evidence/s271b-ios-sim.log 2>&1` (expect exit 0; `ios-sim.json` `selfCheck.playingEvents === 0` and `pass === true`)

**Key points.**

- The pattern to imitate is the existing `run()` / `result.commands.push` style at
  `editor/test/e2e/ios-sim.mjs:5,17`.
- Record the launch time before `simctl launch`, and use a `log show --start` value at or
  before it (or `--last` that is bounded to cover it). Then take the last line that matches
  `/^\S+\s+\S+\s+\S+\s+Vactr\[/` (compact style puts the process name before `[pid`), or
  parse the process field explicitly. A line whose process is `log` must never be selected.
- Add `result.appBinary = { path, mtimeIso }` (the `.app/Vactr` binary) and
  `result.selfCheckSource = 'Vactr-process'`. `result.silent` stays
  `selfCheck?.playingEvents === 0`. When `selfCheck` is null, `pass` is false.
- Never call `simctl` with audio playback. Never set `VACTR_SELF_CHECK` to anything except
  `1`. Exit codes 0, 1 and 2 keep their current meaning.
- The simulator build may rewrite tracked files under `editor/src-tauri/gen/apple/` outside
  `build/`. These are SHELL-owned. The verification step must run
  `git status --porcelain editor/src-tauri` after the build. Any tracked change is restored
  with `git restore <path>` and recorded. It is never committed.

### TASK-402: INT-S271-EV-SYNC-ATTRIBUTION (harness; additive perf-hook field if needed)

**Brief (verbatim).**

- Reproduction: Data: `design-docs/specs/evidence/canvas-cutover/run-001/webkit-measure.jsonl`. It has 2048 onset rows; ranges such as [981,983] occur 16 times; it has 1250 presented rows. The current algorithm counts, for each onset o, every presented frame whose activeKey contains o's range with `audibleTime < o.time - 0.002` as early. That counts every frame from all earlier occurrences of the same range. Expected: each frame's active range is attributed to the single onset occurrence (same range, same epoch) whose `[time, end)` window contains the frame's audibleTime, give or take the 2 ms tolerance.
- Root cause: Confirmed by reading `measure.mjs` (the loops at line 55+) and the onset range recurrence counts. The window mismatch between the independently capped `onsets()` and `presented()` buffers (`perf-hook.ts` presentedRows of 4096, and the onset buffer) is also confirmed in code. Its exact effect on the empty Chromium `sync[]` is unverified; verify it.
- Ordered changes:
  1. Move the attribution into a pure, exported function in `stats.mjs`, for example `attributeSync(onsets, presented, {earlyToleranceS: 0.002})`. `measure.mjs` calls it, so it can be unit-tested.
  2. Restrict the inputs to the overlapping time window: keep only onsets whose time lies within [first presented audibleTime, last presented audibleTime]. Match on epoch.
  3. For each frame and each active range in that frame, find the onset occurrence with the same range and epoch and the largest `time <= audibleTime + 0.002`. If none exists, or that occurrence's `end <= audibleTime`, count it as early or replayed respectively, once per frame-range pair.
  4. For each onset in the window, take the first frame whose activeKey contains its range with `audibleTime >= o.time - 0.002` and that is attributed to this occurrence. Compute sync from the next presented frame as today: `next.frameMs - (p.targetMs + (o.time - p.audibleTime)*1000)`.
  5. Apply the same occurrence-based attribution to `lateActiveMismatchCount` (the post-stall active set compared with the analytic set derived from onsets in the window).
  6. Add `stats.test.ts` rows: (a) a repeating range with N occurrences, correctly presented: early=0, replayed=0, sync within 1 frame; (b) a frame showing range r 10 ms before its occurrence: early=1; (c) a frame showing r after its end with no newer occurrence: replayed=1; (d) disjoint onset and frame windows: no samples, and the result is not counted as failures; (e) a Chromium-like case with measured provenance and non-overlapping buffers, which must yield samples once windows overlap.
  7. Rerun the full e2e and record the new numbers. If real failures remain, classify them in the triage table with an owner seam.
- Forbidden changes: Do not change any threshold constant in `stats.mjs` (`syncAbsP95Ms` 33.4, `syncAbsP99Ms` 50, `earlyFlashMs` 2, etc.) or the 'measured only' gating rule. Do not drop onsets or frames to make metrics pass. Do not change `editor/src/app/clock.ts` or `editor/src/code/highlight.ts` under this finding; only after the corrected metric shows a real defect may a separate seam repair follow, under the seamProtocol with its own triage row. Do not weaken the silent-sink assertions.
- Completion criteria:
  - [ ] `earlyFlashCount` and `replayedFlashCount` are computed per frame-range occurrence and are bounded by the number of frame-range pairs.
  - [ ] Unit tests cover repeated ranges and window mismatch.
  - [ ] The Chromium run produces sync samples when provenance is measured.
  - [ ] Any remaining sync failure has a triage row naming a concrete seam file and its evidence.
  - [ ] Thresholds are unchanged (git diff shows no threshold edits in `stats.mjs`).
- Verification:
  - `cd editor && ./node_modules/.bin/vitest run test/e2e` (exit 0, with the new `stats.test.ts` rows passing)
  - `node --check editor/test/e2e/measure.mjs editor/test/e2e/stats.mjs` (exit 0)
  - `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001 > ../tmp/canvas-cutover/evidence/s271b-e2e.log 2>&1` (outside the sandbox). Record the exit code. Silence assertions must still hold.

- Session-274 review override (finding S274-PLAN-SYNC-WINDOW). The pinned contract below
  refines brief ordered change 2 (inputs restricted to the presented span), brief ordered
  change 5 (late-active analytic set from in-window onsets) and test row 6(d). For TASK-402, the
  pinned contract takes precedence over the brief JSON wherever the two differ:
  - attribution candidates are all retained onsets;
  - the evaluated window is the `syncWindow` intersection with the eviction guard;
  - frames outside the window are reported as `excludedFrames`;
  - the late-active analytic set uses all retained onsets.

  Everything else in the brief stands, including its forbidden changes and completion
  criteria.

**Contract (pinned; corrected after Step 5 review finding S274-PLAN-SYNC-WINDOW).**

Three pure functions are exported from `stats.mjs` and imported by `measure.mjs:55-65`, which
replaces its inline loops. The inputs are the existing `OnsetRecord { time, end, from, to, epoch }`
rows (all retained rows from `__vactrPerf.onsets()`, after the existing
`attributeWorkloadOnsets` control exclusion) and `PresentedRecord` rows. The range key is
`${from}-${to}`, as in `activeKey`.

- `syncWindow(onsets, presented)` returns `{ windowStart, windowEnd, empty }`. The window is the
  intersection of the two ring buffers' coverage, with an eviction guard:
  - `maxRetainedDuration = max(end - time)` over the retained onsets;
  - `windowStart = max(first presented audibleTime, first retained onset time + maxRetainedDuration)`;
  - `windowEnd = min(last presented audibleTime, max(end) over retained onsets)`.

  `empty` is true when there are no retained onsets, no presented rows, or
  `windowStart > windowEnd`. The guard exists because any onset that could still be sounding
  at `windowStart` started after the first retained onset, so it is retained. Both rings are
  independently capped at 4096 rows (`editor/src/code/perf-hook.ts:38-40`), and the onset ring
  may have evicted older notes.
- `attributeSync(onsets, presented, { earlyToleranceS = 0.002 } = {})` returns
  `{ sync: number[], earlyFlashCount, replayedFlashCount, framePairs, windowOnsets, excludedFrames, windowStart, windowEnd }`.
  The function separates two roles:
  - **Attribution candidates** are all retained onsets, including those that start before
    `windowStart`.
  - **Sync samples** come only from onsets whose `time` lies inside
    `[windowStart, windowEnd]`.

  `windowOnsets` is the number of such onsets. `excludedFrames` is the number of presented rows
  whose `audibleTime` lies outside the window. Those rows are not evaluated for early or
  replayed counts. They stay in the raw JSONL, and the count is reported next to
  `windowOnsets` in the metrics. If the window is empty, the result is `sync []`, early 0,
  replayed 0, `framePairs` 0, `windowOnsets` 0 and `excludedFrames` equal to the presented row
  count.
- **Classification of an evaluated frame-range pair.** An evaluated frame-range pair is a frame
  inside the window and one range from its split `activeKey`. Its candidate is the retained
  onset with the same range and epoch and the largest `time <= audibleTime + earlyToleranceS`.
  - No candidate: count early. This includes an in-window range that has no onset at all, or
    only a later one. The guard never masks a real early flash inside the window.
  - Candidate `end <= audibleTime`: count replayed.

  Each pair is counted at most once, so `earlyFlashCount + replayedFlashCount <= framePairs`.
- **Sync sample.** For each in-window onset `o`, find the first presented row (any row, not
  only in-window rows) that satisfies all of these:
  - its split `activeKey` contains `o`'s range;
  - its epoch equals `o`'s epoch;
  - `audibleTime >= o.time - earlyToleranceS`;
  - its candidate (as defined above) is `o` itself.

  The sample is `next.frameMs - (p.targetMs + (o.time - p.audibleTime) * 1000)`, where `next`
  is the following presented row. This is the same formula as today. There is no sample if no
  such row or no `next` exists.
- `lateActiveMismatches(onsets, presented, stallFrameMs)` returns a count. For each stall, it
  takes the first presented row with `frameMs >= stall`. It evaluates the stall only if that
  row's `audibleTime` lies inside `syncWindow(onsets, presented)`. The analytic active set
  is built from all retained onsets (same epoch) with `time <= audibleTime < end`, as
  `measure.mjs:64` does today but epoch-aware. That set is compared with the row's split
  `activeKey` as a sorted set. Stalls outside the window are not evaluated.

**Key points.**

- `PresentedRecord` has no `epoch` today (`editor/src/code/perf-hook.ts:11-14`). If one is
  needed to match on epoch, add the record field `epoch: string | null` to `PresentedRecord`.
  This is a field, not a new `__vactrPerf` member. Populate it in the `recordPresented` call
  at `editor/src/code/mount.ts:212` from the same epoch source that `HighlightScheduler` uses
  (`opts.epoch`, `editor/src/code/highlight.ts:87`). Add a row to
  `editor/test/canvas/mount.test.ts` asserting the field. Record why in the progress log.
  `attributeSync` treats a `null` epoch as matching only a `null` onset epoch.
- `activeKey` is a comma-joined list. Split it, and never use `includes()` on the joined
  string. `981-98` must not match `981-983`.
- Use the intersection window with the eviction guard from `syncWindow` for both
  `attributeSync` and `lateActiveMismatches`. Never bound the window by the presented side
  alone. A correct highlight of a note that started before the first presented row, or whose
  onset was evicted from the onset ring, must not become an early flash.
- Keep the two onset roles separate. The candidate lookup uses all retained onsets. Only the
  sync-sample loop and `windowOnsets` are limited to in-window onsets. Frames outside the
  window go to `excludedFrames`, which is reported in the metrics and the evidence table.
  Rows are never deleted from the raw JSONL.
- `earlyFlashCount + replayedFlashCount` must be at most `framePairs`. Assert this in tests.
- No gate changes. `stats.mjs:123` still fails a measured run when early > 0, and the
  `stats.mjs:120` `measured sync samples unavailable` failure still applies when the window
  yields no samples under `measured`. An empty window is not a pass.
- For Chromium, check that the provenance `find((p)=>p.valid)` and the window really overlap.
  If the corrected function still yields no samples under `measured`, the existing
  `measured sync samples unavailable` gate fails the run. Never bypass it.
- After moving the loops, `measure.mjs` must stay under 1,000 lines. It is 70 lines today.

**Test cases** (`editor/test/e2e/stats.test.ts`, loaded through the existing non-literal
dynamic-import pattern):

Shared fixture F:

- epoch `e` everywhere unless stated;
- filler range `1-2` with onsets at 0.0, 0.25, ..., 3.0 s, each with `end = time + 0.2`;
- presented rows every 1/60 s from `audibleTime` 0.0 to 3.0 s, with
  `frameMs = targetMs = audibleTime * 1000`, each showing exactly the ranges of the onsets with
  `time <= audibleTime < end`.

With the 0.4 s `5-7` notes added below, `maxRetainedDuration` is 0.4, so the window is
[0.4, 3.0].

- (a) F plus `5-7` onsets at 0.5, 1.0, 1.5, 2.0 and 2.5 s, each with `end = time + 0.4`,
  presented while active -> early 0, replayed 0. Every in-window onset that has a following
  row yields exactly one sample with absolute value at most 16.7 ms. `sync.length` equals that
  count.
- (b) F plus a `5-7` onset at 1.0-1.4 s, and the row at about 0.99 s additionally showing
  `5-7` -> early 1.
- (c) F plus a `5-7` onset at 1.0-1.4 s, and the row at about 1.42 s still showing `5-7`, with
  no newer `5-7` onset -> replayed 1.
- (d) Corrected. A `5-7` onset at 0.0-1.0 s only; presented rows at 5.0-6.0 s that all show
  `5-7`. The window is [max(5.0, 0.0 + 1.0), min(6.0, 1.0)], which is empty -> `sync.length`
  0, early 0, replayed 0, `framePairs` 0, `excludedFrames` equal to the row count.
- (e) Chromium-like overlap with measured provenance: `5-7` onsets every 0.5 s from 0.0 to
  6.0 s (0.4 s each), and presented rows at 5.0-6.0 s showing active notes. The window is
  [5.0, 6.0] -> at least 1 sync sample, early 0.
- (f) Frames with ranges `981-98` and `981-983` -> no cross-match.
- (g) F plus a `5-7` onset with epoch `x` at 1.0-1.4 s, and a row (epoch `e`) at 1.1 s showing
  `5-7` -> early 1 (no candidate with a matching epoch).
- (h) `lateActiveMismatches`:
  - an in-window stall whose row's active set equals the analytic set -> 0;
  - the same stall with an extra stale range -> 1;
  - a stall whose row lies before `windowStart` -> not evaluated (0).
- (i) A pre-window onset that is still sounding: a `5-7` onset at 0.9-2.0 s, and rows at
  1.0-1.5 s showing `5-7`. The window is [max(1.0, 0.9 + 1.1), min(1.5, 2.0)], which is
  empty -> early 0 and 0 sync samples (the onset is outside the window).
- (ii) An onset ring that starts later than the presented ring:
  - presented rows at 25.0-35.0 s;
  - retained `5-7` onsets every 0.5 s from 30.0 to 35.0 s (0.4 s each);
  - rows at 25.0-30.0 s that show `5-7` (their onsets were evicted).

  The window is [30.4, 35.0] -> those earlier rows count in `excludedFrames` (> 0) and early
  is 0.
- (iii) An in-window frame showing a range with no candidate: F plus the row at 1.5 s
  additionally showing range `9-9`, which has no onset -> early 1 (the guard does not mask
  real early flashes).

### TASK-403: INT-S271-EV-READBACK (harness first)

**Brief (verbatim).**

- Reproduction: `run-001/chromium-behavior.json` and `webkit-behavior.json`: canvas-only-text `pass=false`, detail 'canvas pixel readback has no glyph variation'. The same runs pass the dpr-and-resize and context-loss checks, so a canvas exists with a non-1x1 backing.
- Root cause: Two confirmed probe faults. The sampled region is not a text line, and the readback timing is not tied to a draw while `preserveDrawingBuffer` is false (`renderer.ts:97`). Uncertain: whether the product also fails to draw glyphs. That is decided only after the probe is corrected.
- Ordered changes:
  1. Compute the line-1 text rectangle in device pixels from the canvas CSS box times devicePixelRatio. Use the gutter width and line height exposed through `__vactrPerf` or the layout, if available. Otherwise sample a band at the first line's top-left after the gutter, after typing the '# canvas-visible-evidence' marker on line 1.
  2. Read pixels in the same frame as a draw. Use a nested requestAnimationFrame, so the read runs after the app's frame callback registered for that frame; or read inside a callback that the harness proves runs after the renderer's draw. Caret blink keeps animation frames running while focused.
  3. Keep the existing DOM and bridge assertions.
  4. If the corrected probe still finds uniform pixels, treat it as a product defect: add a triage row and fix it in the renderer/mount/layout seam with a `gpu.test.ts` or `mount.test.ts` row.
- Forbidden changes: Do not set `preserveDrawingBuffer:true` in production to satisfy the probe. Do not loosen the check: at least 2 colors in a region that contains glyphs, and no document text in the DOM. Do not remove the DOM-text and bridge-visibility assertions.
- Completion criteria:
  - [ ] canvas-only-text passes in both browsers, with a readback region that sits on line-1 text.
  - [ ] No production `preserveDrawingBuffer` change.
  - [ ] Any product repair carries a triage row, sha256 values and green owner tests.
- Verification: `node --check editor/test/e2e/behavior.mjs` (exit 0); the full e2e command from TASK-402; canvas-only-text must pass in chromium and webkit.

**Key points.**

- `__vactrPerf` exposes no layout geometry, and adding a member is forbidden. Derive line 1
  from the DOM as follows:
  - The layout defaults are a 48 CSS px gutter (`editor/src/code/layout.ts:161`) and line `n`
    at `top + n * lineHeight`.
  - Take `lineHeight` from the computed font of the code pane if it is exposed there.
    Otherwise use the input bridge rect when `accessibility.position()` places the bridge at
    the caret: after `fill('# canvas-visible-evidence')` the caret is at the end of line 1.
    Read `editor/src/code/accessibility.ts` to confirm before relying on it.
  - Convert CSS px to device px with
    `(x - canvasRect.left) * canvas.width / canvasRect.width`.
  - The region spans from just after the gutter to before the caret x, over line 1's vertical
    band. Record the region in the check detail.
- Same-frame read. Install a one-shot page-side wrapper of `window.requestAnimationFrame`
  just for this check. The next app callback is wrapped so that, right after it returns, the
  harness copies the canvas with `drawImage` into a 2D probe canvas, and the wrapper
  uninstalls itself.
  - Accept the read only if that app frame recorded a new presented row (compare
    `__vactrPerf.presented().length` or the last `frameMs` before and after).
  - Otherwise retry on the next app frame, at most 30 frames, and then fail with a precise
    detail.
  - A nested rAF alone does not guarantee ordering relative to the app's own re-registration.
- Keep `canvas.width > 1` and the `colors.size >= 2` rule exactly as they are.
- If the corrected probe fails on a frame that recorded a presentation, that is a product
  defect. Repair it in `renderer.ts`, `mount.ts` or `layout.ts` with a `gpu.test.ts` or
  `mount.test.ts` row and a triage row.

### TASK-404: INT-S271-EV-TOUCH (diagnose, then fix the harness or the pointer.ts seam)

**Brief (verbatim).**

- Reproduction: `run-001/webkit-behavior.json` touch-selection: `pass=false`, 'page.waitForFunction: Timeout 2000ms exceeded' at `behavior.mjs:101`. Chromium touch-selection passed with selection {anchor:30, head:33} and handles 2.
- Root cause: Unverified. Instrument it: record the events `pointer.ts` receives in WebKit (type, pointerType, isPrimary, buttons, timing) and whether the long-press timer fires or is cancelled.
- Ordered changes:
  1. Add temporary instrumentation in the harness (page-side event listener logging) to capture the received events in WebKit. Keep the diagnosis log under `tmp/canvas-cutover/evidence/`.
  2. If the product cancels or ignores a valid primary touch long-press (for example, small-movement tolerance or a missing pointer capture path), fix it minimally in `pointer.ts` and add an `input.test.ts` row that reproduces the WebKit event sequence in jsdom.
  3. If the harness events are malformed (for example, not real PointerEvents or a missing pointerId/width/height), fix `behavior.mjs` so it dispatches proper PointerEvent instances (via `page.evaluate` with `new PointerEvent(...)`).
  4. Rerun e2e and add or update the triage row.
- Forbidden changes: Do not reclassify the failure as a limitation unless an instrumented repro proves WebKit cannot deliver the events (record that proof). Do not change the CodeSurface or PointerController public contracts. No timeout widening beyond what the gesture itself needs.
- Completion criteria:
  - [ ] WebKit touch-selection passes, or an instrumented proof that WebKit cannot deliver the events is recorded as an explicit platform limitation reviewed in integration.
  - [ ] Any `pointer.ts` change has a jsdom regression row, and the canvas tests stay green.
- Verification: `cd editor && ./node_modules/.bin/vitest run test/canvas/input.test.ts` (exit 0, if `pointer.ts` changed); the full e2e command; webkit touch-selection must pass with anchor != head and handles === 2.

**Key points.**

- The relevant code is `PointerController.down` at `editor/src/code/pointer.ts:57-89`:
  - it rejects `button !== 0` or `isPrimary === false`;
  - the 500 ms timer selects the word and calls `capture`;
  - `move` cancels the press when movement exceeds 8 px before the timer fires.

  The harness waits 650 ms before `pointermove`, so the timer should have fired.
- The waiter also needs `presented().at(-1).handles === 2`. That only updates after a
  presented frame, so check whether WebKit presents a frame after the selection (rAF cadence)
  before blaming `pointer.ts`. Log `selection()` and the last presented row in the diagnosis.
- Instrumentation is a harness-side capture listener registered with
  `addEventListener(..., { capture: true })` on the canvas. It logs `type`, `pointerType`,
  `isPrimary`, `pointerId`, `button`, `buttons`, `width`, `height` and `timeStamp`. It writes
  `tmp/canvas-cutover/evidence/s274-webkit-touch-events.json` and runs in the diagnostic path
  only, never in the gated assertions.
- The `pointer.ts` long-press path calls `wordRange(this.surface.state.doc.toString(), ...)`.
  That is acceptable here (not the edit path). Do not refactor it under this finding.
- Any `pointer.ts` fix keeps `PointerController` public members unchanged and adds a jsdom row
  in `editor/test/canvas/input.test.ts` that replays the recorded WebKit sequence (same field
  values and timing, with fake timers) and expects a word selection with two handles.

### TASK-405: INT-S271-EV-PERF harness parts (profiler, ranking and WebKit control page)

**Brief (verbatim).**

- Reproduction: `design-docs/specs/evidence/canvas-cutover/run-001/summary.json` failures 2-7 (chromium) and 11-18 (webkit). In `webkit-measure.jsonl` the 'edit' rows advance 10 keys per about 3 s. `tmp/canvas-cutover/evidence/s271-canvas-evidence-e2e-rebuilt.log`.
- Root cause: Unknown. Code-pane frame work alone (frame rows, workMs) is far below the observed frame intervals, so the dominant cost is outside the code-pane frame callback or is caused by blocking tasks between frames. This must be measured before editing product code.
- Ordered changes:
  1. First complete INT-S271-EV-SYNC-ATTRIBUTION and INT-S271-EV-READBACK, so the rerun reports are trustworthy.
  2. Add a non-gating diagnostic option to `measure.mjs`: when `--profile-trace` is passed, on Chromium only, capture a CDP Tracing (devtools.timeline) or Profiler CPU profile for 10 s of the edit run and 10 s of the cycle run. Save it under `tmp/canvas-cutover/evidence/s271b-trace-*.json`. It must be off by default and must not change gated numbers.
  3. From the profile, rank main-thread self time by function and file. Record the top 10 and their share in the progress log and the evidence triage table.
  4. Fix the dominant costs inside seam files with the smallest change. Likely shapes, to be confirmed by the profile: work done per frame that 15.3.8.3 says must not run on animation-only frames; visual loop work that is not frame-capped; whole-document operations triggered per keystroke or per telemetry message. Add a jsdom regression row in the matching seam test for each fix (an operation counter, not wall time).
  5. Rerun the full e2e in both browsers. Repeat until the gates pass or the remaining cost is proven outside the seams (then report the blocker as above). Never relabel a failed gate as a limitation; only the existing documented fallbacks (software GL and similar) may apply, and they do not apply here.
- Forbidden changes: No threshold changes in `stats.mjs`. No workload reduction: keep the 20,000-line, about 1 MiB document; 64 voices at amp 0.005; four synth outputs at the shipping `RENDER_SIZE`; 720p video; active scopes; keystroke pacing of about 10/s. No Rust edits, no public contract changes (CodeSurface, DocumentSync, `__vactrPerf` members, Session Protocol), and no new dependencies, per the seamProtocol. Audio must never wait on, allocate for, or be driven by rendering. Do not edit files outside the seam list. If the dominant cost is in a non-seam file (for example `editor/src/visual/mount.ts`, `editor/src/app/main.ts`, `editor/src/protocol/*`, or Rust session code), stop and report a blocker naming the file, the owning accepted plan, and the measured share of main-thread time, for a manifest scope amendment and selective redispatch.
- Session-274 operator override of the two brief sentences above (design 15.3.8.12):
  - For a non-seam TypeScript file, "stop and report ... selective redispatch" becomes:
    record the path, owner and share, continue with the listed seams, and let the plan author
    amend writePaths serially. The run does not stop, and no accepted plan is redispatched.
  - "Never relabel ... they do not apply here" is narrowed by operator decision B. A
    headless-WebKit limitation is allowed only with the control-page proof below. Everything
    else in the brief stands.
- Completion criteria:
  - [ ] `summary.json` `pass=true` for both browsers with no threshold edits, or a precise blocker naming a non-seam owner file with profiled evidence.
  - [ ] Every seam edit has a triage row, fresh and post-edit sha256 values, and a green owner test with a new operation-counter row.
  - [ ] The evidence document is regenerated by the harness.
- Verification: `cd editor && npm run check` (exit 0); `cd editor && ./node_modules/.bin/vitest run test/canvas test/code test/visual test/e2e` (exit 0); `cd editor && ./node_modules/.bin/vitest run` (default full suite, exit 0); `cd editor && npm run test:perf` (alone, exit 0); `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` (outside the sandbox; exit 0 required for acceptance; silence assertions hold).

**Harness contracts (pinned).**

- `run.mjs` passes `--profile-trace` through to `runMeasurement(..., { profileTrace })`. With
  it set, on Chromium only, `measure.mjs` uses `context.newCDPSession(page)` and
  `Profiler.enable`/`Profiler.start`/`Profiler.stop` (CPU profile) around a 10 s window of the
  edit run and a 10 s window of the cycle run. It writes
  `tmp/canvas-cutover/evidence/s271b-trace-<browser>-<edit|cycle>.json`.
  - A profiled run is a diagnostic. Invoke it with `--run-id s274-trace` and
    `--out ../tmp/canvas-cutover/evidence/s274-trace`, never with `--write-evidence`. Its
    numbers are never used as gated evidence.
  - Without the flag, no CDP profiler call is made and the code path is unchanged.
- `stats.mjs` exports `rankSelfTime(cpuProfile, top = 10)`. It returns
  `[{ functionName, url, line, selfMs, share }]` aggregated by function and url from the
  `nodes`, `samples` and `timeDeltas` of the CPU profile. Add a `stats.test.ts` row with a tiny
  synthetic profile: two nodes, known deltas, expected ordering and shares summing to at most 1.
- **WebKit control page (design 15.3.8.12).**
  - In WebKit headless only, `measure.mjs` opens one extra page in the same context with
    `page.setContent(...)`. The content is a minimal HTML document with a focused `<textarea>`,
    a `requestAnimationFrame` loop that records timestamps, and a `keydown` listener that
    records `event.timeStamp`. There is no app code and no audio.
  - Drive it with the same key pacing loop and the same 60 s duration as the edit run.
  - It reports `control = { frameIntervalMs: { p95, p99 }, inputLatencyMs: { p95, p99 }, editKeyCount, frames, mode: 'headless' }`.
    Input latency here is keydown `timeStamp` to the rAF timestamp of the second frame after
    the key, matching the product presentation proxy. The result goes into the browser summary
    and the measure JSONL as `phase: 'control'` rows.
- `stats.mjs` exports `classifyWithControl(metricKey, path, productValue, controlValue, threshold)`.
  It returns `'pass' | 'fail' | 'limitation'`. `evaluate()` uses it only when
  `summary.browser === 'webkit'` and `summary.control?.mode === 'headless'`, and only for
  `frameIntervalMs.p95/p99`, `inputLatencyMs.p95/p99` and `editKeyCount`. A `'limitation'`
  pushes a limitation string with both numbers and the threshold instead of a failure. Every
  other metric, and Chromium, is unchanged.
- Test cases (`stats.test.ts`):
  - product pass -> `'pass'`;
  - product fail, control pass -> `'fail'`;
  - product fail, control fail -> `'limitation'`;
  - control missing or not finite -> `'fail'`;
  - a Chromium summary with a failing control -> failure kept;
  - `animationWorkMs`/`textWorkMs`/`syncAbsMs` failures are never converted.

**Key points.**

- Do not slow the gated run. The profiler is off by default. The control page adds about 60 s
  to WebKit only.
- Keep the key pacing and durations identical between the control and the edit run. Share the
  loop; do not copy it with different constants.
- The run-level exit stays 1 for any remaining failure and 2 for blocked.

### TASK-406: INT-S271-EV-PERF product fixes (iteration 2 and later, profile-driven)

- Input: the ranked top 10 from the `s271b-trace-*` profiles, recorded in the progress log and
  the triage table.
- Fix only inside listed seams, following the seamProtocol. Each fix adds an operation-counter
  row in the matching seam test, never a wall-clock assertion:
  - `editor/test/canvas/frame.test.ts`, `mount.test.ts`, `gpu.test.ts`, `edit-cost.test.ts`;
  - `editor/test/visual/frame.test.ts`, `scopes.test.ts`, `video.test.ts`,
    `render-host.test.ts`;
  - `editor/test/code/highlight.test.ts`, `transport.test.ts`.
- Shapes to check against the profile (do not guess):
  - per-frame work that 15.3.8.3 forbids on animation-only frames;
  - an idle editor that still requests frames;
  - visual-loop work without a frame cap (Hydra, scopes, video);
  - whole-document `toString` or `Utf8Index` per keystroke or per telemetry message;
  - debounced checks firing per key.
- A cost in a non-listed file follows the "Unlisted dominant cost" rule above.
- Audio independence: no change may make worklet or audio code wait on, allocate for, or be
  driven by rendering.

### TASK-407: Evidence document and triage

Edit only the static sections of `design-docs/specs/design-canvas-editor-evidence.md`. The
marker section is regenerated by the harness only.

- Update the remaining-failure triage table. Each row gives:
  - the check or metric and its value;
  - the classification;
  - the owner seam file, or the non-seam path with profile share;
  - the evidence path.
- Remove the CANVAS-SHELL attribution from the simulator row.
- Add a "Headless WebKit control-page proof" subsection whenever a `'limitation'`
  classification is used. Cite the control and product numbers by `summary.json` path, never
  hand-typed.
- Keep the silent-sink method, the native and simulator silence audit and the pending
  physical-iPad procedures. Add the PERF profile paths and the top-10 ranking.

### TASK-408: Final gates (outside the sandbox, serial)

Run the Verification table rows in order on the final source:

- `npm run test:perf` alone;
- full nextest with a timeout of at least 1500 s (a timeout kill is neither a pass nor a
  failure).

Record each exit code and complete log path in the progress log. Logs go to
`tmp/canvas-cutover/evidence/s274-*.log`.

### TASK-409: Final integration review, closeout, push

As TASK-009 and TASK-006, unchanged. Move these files one by one with `git mv` to
`impl-plans/completed/`, each with a one-line superseded note (for canvas-editor-224) or
`Status: Completed`:

- the fifteen `impl-plans/active/canvas-editor-224-*.md` files, listed in the manifest;
- `impl-plans/active/canvas-editor-224-dispatch.json`;
- the eleven `impl-plans/active/canvas-cutover-*.md` plans: clock, native, render, mount,
  visual, shell, evidence, evidence-silent, evidence-viewport, evidence-editcost and
  evidence-runstart.

`impl-plans/active/canvas-cutover-dispatch.json` stays in `active/` until the workflow closes,
because it is the live manifest.

Then:

- update `impl-plans/README.md`;
- append the 15.3.8.4 clock-probe erratum paragraph;
- commit;
- `git push origin wf/canvas` (non-force).

### Session 274 test and invariant checklist

- The silent sink holds in every browser run: `installedBeforeFirstConnect: true`,
  `directDestinationConnections: 0`, post-sink peak exactly 0, and numeric pre-sink peak dBFS,
  RMS and onset counts and times.
- `git diff 2156699 -- editor/test/e2e/stats.mjs` shows no change inside the `THRESHOLDS`
  object.
- `git diff 2156699 -- editor/test/e2e/fixtures/large-doc.mjs editor/test/e2e/silent-sink.mjs editor/package.json editor/package-lock.json`
  is empty.
- `git diff 2156699 --stat -- src editor/src-tauri/src editor/src/app/main.ts` is empty.
- `grep -n "preserveDrawingBuffer" editor/src` finds no match (grep exit 1).
- Every touched TypeScript source stays under 1,000 lines (`wc -l`).

## Session 277 Amendment (operator decisions A-G; design 15.3.8.13)

This section supersedes all earlier text wherever they conflict. That includes the session-274
checklist line `git diff 2156699 --stat -- src ...`: CANVAS-EVIDENCE-SCOPE now changes Rust
by design. It also includes the "Unlisted dominant cost" blocker that stopped session 274.

### Intent and context

Session 274 implemented TASK-401 to TASK-405 (see its progress log). It stopped on two items:

- INT-S271-EV-PERF: `vactr.wasm` took 70.56-76.58% of main-thread self time, with no Rust
  owner in this plan.
- TASK-402: WebKit presented zero sync pairs.

The operator's read-only diagnosis (`tmp/canvas-cutover/diag-wasm/REPORT.md`) found four causes.
Operator decisions A to E fix them in three serial plans that run before this one:

| Wave | Plan | Decisions | Fixes |
|------|------|-----------|-------|
| 5 | `canvas-cutover-evidence-scope.md` (CANVAS-EVIDENCE-SCOPE) | A | O(1) `Scopes` name index and `Doc::top_of` (quadratic `session_check` and eval) |
| 6 | `canvas-cutover-evidence-sched.md` (CANVAS-EVIDENCE-SCHED) | B, C | fresh engine-time tick with backlog coalescing in `editor/worklet/host.js`; diagnostics check skipped on an unchanged revision |
| 7 | `canvas-cutover-evidence-framecost.md` (CANVAS-EVIDENCE-FRAMECOST) | E, D | no per-draw `isTexture`, no per-frame `getError`, cached grapheme clusters; release wasm with the name section as the vite default, `mise run build-wasm-release`, wasm provenance and gating refusal in `run.mjs` |
| 8 | this plan (CANVAS-EVIDENCE) | G | silent re-measure on the release wasm in Chromium and WebKit, evidence, final gates, integration review, closeout, push |

Thresholds and the workload stay unchanged (decision G). The 15.3.8.8 silent-sink rule
applies to every run. That includes the headed WebKit GL fallback, where the sink must be
installed before any audio node connects.

### Ownership changes (manifest entry `CANVAS-EVIDENCE`)

- `dependsOn` adds CANVAS-EVIDENCE-SCOPE, -SCHED and -FRAMECOST; `wave` becomes 8.
- These files move from writePaths to sharedPaths:
  - `editor/src/code/renderer.ts`
  - `editor/src/code/layout.ts`
  - `editor/test/canvas/gpu.test.ts`
  - `editor/test/canvas/mount.test.ts`
  - `editor/test/code/diagnostics.test.ts`
  - `editor/test/e2e/run.mjs`
  - `editor/test/e2e/README.md`

  They are owned by SCHED or FRAMECOST in waves 6 and 7, as design 15.3.8.13 "Ownership and
  order" requires. After those plans are accepted, this plan may edit them only under the
  seamProtocol, for a re-measure finding, with the reason recorded.
- writePaths add the closeout paths of the three new plans:
  - `impl-plans/active/canvas-cutover-evidence-{scope,sched,framecost}.md`, listed in the
    manifest as three concrete paths;
  - their `impl-plans/completed/` counterparts.
- `editor/worklet/host.js`, `editor/src/code/diagnostics.ts` and the decision-A Rust files are
  not paths of this plan. A re-measure finding there follows the "Unlisted dominant cost" rule:
  a serial plan-author amendment and checkpoint. For Rust, the design lifts the
  "no Rust edit" rule only for the SCOPE files, so the amendment names them explicitly. No
  accepted plan is redispatched.

### TASK-501: Release build setup (outside the sandbox, before every browser or simulator run)

1. `mise run build-wasm-release`, exit 0.
2. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` with no `VACTR_WASM`. This builds
   the release default.
3. Run the FRAMECOST `inspectWasm` check command. It must print `profile: "release"`,
   `nameSection: true` and `dwarf: false`. Record bytes and sha256 in the progress log.
4. The debug wasm (`cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`)
   is still built for vitest. Never point `VACTR_WASM` at it for an e2e or simulator build.

### TASK-502: Evidence rendering of the wasm build (sandbox)

- `editor/test/e2e/stats.mjs` `renderEvidence(summary)` adds one line to the generated section.
  It shows the wasm profile, bytes and sha256 from `summary.wasm`, or
  `summary.environment.wasm` when `summary.wasm` is absent.
- Add a `stats.test.ts` row: a summary with a `wasm` block renders the line, and a summary
  without one renders `wasm: not recorded`.
- No `THRESHOLDS` change.

### TASK-503: Silent re-measure (outside the sandbox)

- Run `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001`
  on the TASK-501 dist. The run must not be refused (exit 2) for its wasm.
- Then, non-gating: run the Chromium `--profile-trace` diagnostic and record the top-10
  `rankSelfTime`, including the `vactr.wasm` share, next to the session-274 numbers.
- Triage every remaining failure as in TASK-407.
  - A product defect inside this plan's writePaths or sharedPaths is fixed under the
    seamProtocol.
  - A defect elsewhere follows the "Unlisted dominant cost" rule.
  - Re-measure after each fix round.

### TASK-504: Escalation F check (decision F; this plan never implements F)

F is triggered only when both of the following hold:

- after TASK-503, the release-wasm A/V sync or input-latency gate still fails in Chromium or
  WebKit;
- the `--profile-trace` evidence shows main-thread long tasks overlapping the late onsets.

When it triggers, stop seam work on that metric and record in the progress log and the triage
table:

- the failing metric;
- the late-onset times;
- the overlapping long tasks with their durations and trace paths.

Report it as the F trigger. The plan author then appends design 15.3.8.14 and a Worker plan
serially, before any implementation. Do not create a Worker, and do not edit
`editor/worklet/host.js`. If either condition is false, F is not triggered; record that with
the evidence.

### TASK-505: Evidence document (sandbox, static sections only)

- Add a "Measurement build" paragraph:
  - gating numbers come from the release wasm with the name section kept
    (`mise run build-wasm-release`);
  - the profile, bytes and sha256 are generated into the results section;
  - the run-001 numbers from sessions 271 and 274 were measured on the debug wasm and are
    superseded by this run.
- Update the triage table and the TASK-402 sync row with the new `summary.json` paths.
- Keep the silent-sink method and the pending physical-iPad procedures.

### TASK-506: Final gates and simulator (outside the sandbox, serial)

- The Verification and final-gate tables, with TASK-501 run first.
- The iOS simulator build bundles `editor/dist`, so it runs after TASK-501, and `ios-sim.json`
  must still show `selfCheck.playingEvents === 0`, `silent === true` and `pass === true`.
- The SCOPE Rust files get `rustfmt --edition 2021 --check`.

### TASK-507: Final integration review, closeout, push

As TASK-409. The move list adds the three new plans, so it is the fourteen
`impl-plans/active/canvas-cutover-*.md` plans: clock, native, render, mount, visual, shell,
evidence, evidence-silent, evidence-viewport, evidence-editcost, evidence-runstart,
evidence-scope, evidence-sched and evidence-framecost. Each is moved by `git mv` as a concrete
path with `Status: Completed`. The fifteen canvas-editor-224 plans and
`canvas-editor-224-dispatch.json` get one-line superseded notes, as before. Then:

- update `impl-plans/README.md`;
- append the 15.3.8.4 clock-probe erratum;
- commit;
- run `git push origin wf/canvas` (non-force).

Integration reviewers return the exact runner envelope `{ "payload": {...}, "when": {...} }`.

### Session 277 checklist

- `git diff 3e69e16 -- editor/test/e2e/stats.mjs` shows no change inside `THRESHOLDS`.
- `git diff 3e69e16 -- editor/test/e2e/fixtures/large-doc.mjs editor/test/e2e/silent-sink.mjs editor/package.json editor/package-lock.json Cargo.toml Cargo.lock`
  is empty.
- `git diff 3e69e16 --name-only -- src` lists only the CANVAS-EVIDENCE-SCOPE files:
  `src/types/scope.rs`, `src/types/tests/mod.rs`, `src/types/tests/scope_cost.rs`,
  `src/directives/attach.rs`, `src/directives/tests/attach.rs` and
  `src/directives/tests/labels.rs`, plus `src/types/check.rs` or `src/directives/labels.rs` only
  if SCOPE recorded a compiler-required edit.
- `summary.json` has `wasm.profile === "release"`, `wasm.nameSection === true` and
  `wasm.dwarf === false`.
- The silent sink holds in every browser run, including the headed WebKit fallback.

## Session 285 Amendment (operator decisions 1-6; design 15.3.8.13 "Session 285 scope record")

This section supersedes all earlier text where they conflict. That includes:

- the Non-goal "No product-code change";
- the session-277 rule that `editor/worklet/host.js`, `editor/src/code/diagnostics.ts`,
  `renderer.ts` and `layout.ts` are not writable here;
- the manifest non-goal that forbids edits to `editor/src/app/main.ts`.

The session-283 hard rule and the session-284 record format still apply.

### Intent and context

Every other canvas-cutover plan is accepted. They are listed in the manifest
`acceptedDependencies`, and SCOPE, SCHED and FRAMECOST were moved there at this checkpoint
(accepted at 2e3361d). Do not redispatch or re-gate them.

The canonical release-wasm silent run
`cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001`
exits 1 with 10 measurement failures (`tmp/canvas-cutover/evidence/s285-continuation1-release-e2e-canonical.log`):

- **INT-S284-EV-WEBKIT-PERF (high).** WebKit fails these gates:
  - 162 editing keys (needs at least 500);
  - input p95/p99 360/377 ms;
  - animation p99 165 ms;
  - text p95 178 ms;
  - frame p95/p99 185/197 ms;
  - sync p95/p99 62/66 ms.

  The same-run WebKit control passes (642 keys, input 33/35 ms, frames 18/19 ms), so these are
  product defects. Never relabel them as limitations.
- **INT-S284-EV-CHROMIUM-TEXTWORK (mid).** Chromium `textWorkMs.p95` is 18.7 ms against the
  unchanged 16.7 ms threshold.

The source is confirmed by reading, but the cost is not yet measured. Per keystroke,
`CodeViewHost.keepCaretVisible` (`editor/src/code/view-host.ts:57`) runs twice:

1. from the `selectionSet` subscription (`view-host.ts:29-31`);
2. from `scrollCaret`, called by `input.ts:169`, `keyboard.ts:99` and `keyboard.ts:110` and
   wired at `mount.ts:143`.

Each run costs:

- two `getBoundingClientRect` reads: `view-host.ts:58`, plus the `viewport` getter at `:35`
  through the `coordsAtPos` bridge at `:22`;
- a `clampScroll` that shapes up to 1024 lines (`view-host.ts:69`).

`TextLayout.setText` (`layout.ts:55-66`) evicts every cached line after an edit whose
`from`/`to` shifted. An edit in the first 1024 lines therefore makes `clampScroll` reshape them
all, with `measureText` per run.

The `onFrame` path also reads `viewHost.viewport` (a forced layout read) on every frame
(`mount.ts:179`, `:200`).

### Ownership (manifest entry `CANVAS-EVIDENCE`, operator decision 1)

writePaths now include every decision-1 product file and test:

- `editor/src/app/{apis,clock,deps,layout,main,song}.ts`;
- `editor/src/code/{accessibility,atlas,completion-popup,completion-view,completion,diagnostics,eval,frame,highlight,history,input,keyboard,layout,mount,perf-hook,pointer,renderer,resources,surface,sync,syntax-core,syntax,transport,view-host}.ts`;
- `editor/worklet/host.js` and `editor/worklet/processor.js`;
- `editor/test/canvas/{view-host (new),edit-cost,frame,gpu,input,mount,clock}.test.ts`;
- `editor/test/code/{diagnostics,highlight,transport,completion-view}.test.ts`;
- `editor/test/app/main.test.ts`;
- `editor/test/protocol/{host-js,worklet-quad}.test.ts`.

The manifest lists each of these as a concrete path. `renderer.ts`, `layout.ts`,
`gpu.test.ts`, `mount.test.ts` and `diagnostics.test.ts` move from sharedPaths back to
writePaths.

The closeout adds two writePaths:

- the top-level `README.md`;
- `tmp/canvas-cutover/framecost/receipt.json`, gitignored, so it is also an artifactRoot.

Reserved for escalation F only, and never created in this run unless TASK-511 triggers and
design 15.3.8.14 exists:

- `editor/worklet/tick-worker.js`;
- `editor/test/protocol/tick-worker.test.ts`.

`editor/test/e2e/run.mjs`, `editor/test/e2e/README.md` and the four frozen harness files stay
sharedPaths. No edit to them is expected.

Do not touch:

- `editor/src/protocol/*`, `editor/src/visual/*` beyond the existing writePaths,
  `editor/test/support/*`, Rust, `Cargo.*`, `package*.json` or `mise.toml`;
- `editor/test/e2e/stats.mjs` `THRESHOLDS`;
- `editor/test/e2e/fixtures/large-doc.mjs` and `editor/test/e2e/silent-sink.mjs`.

### Non-goals

- No threshold, workload, silent-sink, dependency, protocol-field or `CodeSurface` member
  change.
- No change to the production master output stage.
- No Worker, unless TASK-511 triggers, and then only after a design-author 15.3.8.14 amendment.
- No mutation or negative-control command. Cite historical logs in notes only.
- No wall-clock assertion outside `npm run test:perf`.
- No refactoring beyond what TASK-508 and TASK-509 need. Touched TypeScript files stay under
  1000 lines.

### Log naming

Session 285 was already used as a log prefix by the previous run's continuation (`s285-*`).
This run writes every log and intent file as `tmp/canvas-cutover/evidence/s285b-<name>.log`
(or `.json`). Never overwrite an `s285-*` file.

### TASK-508: Phase attribution instrumentation (sandbox; no behavior change)

The perf hook is enabled only under `?perf=1` (`mount.ts:156`). All instrumentation must be
inert when it is off: no `performance.now()` call, no ring allocation and no global is set.

- **`editor/src/code/frame.ts`.** Export the phase type and timer. Pinned contract:

  ```ts
  export type PerfPhase = 'input' | 'caret' | 'shaping' | 'syntax' | 'upload' | 'frame' | 'tick';
  export class PhaseTimer { begin(phase: PerfPhase): void; end(phase: PerfPhase): void; rows(): number[][] }
  ```

  Mapping to design phases:
  - scroll/caret = `caret`;
  - layout/shaping = `shaping` + `syntax` (two sub-rows);
  - renderer upload/draw = `upload`;
  - scheduler = `frame` + `tick`.

  Semantics:
  - `begin` and `end` nest on a stack.
  - Each phase accumulates exclusive time: the elapsed time minus the time of nested phases.
  - The outermost `begin` opens a span, and the matching `end` closes it.
  - When a span closes, one row is written to a preallocated ring of 4096 rows, reusing rows
    like `PerfRecorder.recordFrame`. The row shape is
    `[spanKind, startMs, input, caret, shaping, syntax, upload, frame, tick]`, where
    `spanKind` is the index of the outermost phase.
  - An `end` that does not match the stack top is a programming error. In tests it throws. It
    must not corrupt later rows.
  - `PerfRecorder` gains `readonly phases: PhaseTimer`. `FrameScheduler.perf` is non-null only
    when `perf: true`, so the timer exists only then.
  - The frame callback wraps `onFrame` in `begin('frame')`/`end('frame')`.
- **`editor/src/code/perf-hook.ts`.** Add the member
  `phases(): { names: PerfPhase[]; rows: number[][] }` (additive; existing members unchanged).
  It returns ordered copies.
- **`editor/src/code/keyboard.ts` and `input.ts`.** Add `phases?: PhaseTimer | null` to
  `KeyboardOptions` (inherited by `InputOptions`). Each top-level DOM handler body wraps
  `phases?.begin('input')`/`end('input')` in `try/finally`. These are keydown, beforeinput,
  input, composition and clipboard. `mount.ts` passes `scheduler.perf?.phases ?? null`.
- **`editor/src/code/view-host.ts`.** `CodeViewHost` takes an optional
  `phases: PhaseTimer | null`, either as a trailing constructor parameter or a `setPhases`
  setter (the implementer's choice). It wraps the caret evaluation and `clampScroll` in
  `caret`.
- **`editor/src/code/layout.ts`.** Add `TextLayout.phases: PhaseTimer | null = null`. When a
  cache miss builds a line in `shape()`, wrap the build in `shaping`. A cache hit is not
  timed.
- **`editor/src/code/mount.ts`.** In `onFrame`:
  - wrap `syntaxProvider.spans` in `syntax`;
  - wrap `renderer.setText`, `renderer.setViewport` and `renderer.render` in `upload`.

  Set `layout.phases` only when perf is on. Set `globalThis.__vactrPhaseTimer = phases` only
  when perf is on, and delete it on dispose.
- **`editor/worklet/host.js`.** Around `this.x[this.fn.tick](this.now)`, read
  `globalThis.__vactrPhaseTimer` once per tick. If it is present, wrap the tick in
  `begin('tick')`/`end('tick')`. When it is absent, make no timing call. Add the global's
  optional type to `editor/worklet/host.d.ts` only if `npm run check` needs it; that file is
  not in writePaths, so prefer a local JSDoc and no type change.
- **`editor/test/e2e/measure.mjs`** (a writePath since session 274). In each browser's measure
  pass, read `__vactrPerf.phases()` at the end of the edit and cycle windows and store the raw
  rows in the `*-measure.jsonl` sample records. Keep the per-file total under 2 MiB: store
  rows only for the edit window, capped at 4096.
- **`editor/test/e2e/stats.mjs`.** Add a pure export
  `phaseSummary(rows) -> { [phase]: { spans, p50, p95, p99, totalMs } }`, computed over the
  rows where that phase is greater than 0. `summary.json` gets a per-browser `phaseMs` block.
  It is not gating: no `THRESHOLDS` entry, and it adds nothing to the failure list.
  `renderEvidence` is unchanged.
- **Tests:**
  - `editor/test/canvas/frame.test.ts`, nested exclusive accounting:
    `begin(input) 0, begin(caret) 2, end(caret) 7, end(input) 10` with a fake `now` ->
    one row with `input = 5` and `caret = 5`. A mismatched `end` throws and the next span
    records correctly. After 4097 spans the ring holds 4096 rows and drops the oldest.
  - `editor/test/canvas/mount.test.ts`, existing "installs no performance hook without the
    query flag" pattern: without `?perf=1`, `globalThis.__vactrPhaseTimer` is undefined and
    `layout.phases` is null. With `?perf=1`, one keystroke plus one frame produce rows with
    `spanKind` `input` and `frame` through `__vactrPerf.phases()`.
  - `editor/test/protocol/host-js.test.ts`: with no global, a tick makes 0
    `performance.now` calls (host.js has none today; spy it). With a fake
    `globalThis.__vactrPhaseTimer` that counts calls, each tick calls `begin('tick')` and
    `end('tick')` exactly once, and a message that does not tick calls neither. Assert both
    branches in one test, and delete the global in `afterEach`.
  - `editor/test/e2e/stats.test.ts`: `phaseSummary` on fixed rows gives the expected p95 per
    phase, and a phase that is always 0 gives `spans: 0`.

**Before measurement (outside the sandbox, non-gating, diagnostic).** Run it after TASK-508 and
before any TASK-509 change:

1. `mise run build-wasm-release`
2. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`
3. `cd editor && npm run e2e -- --browser all --profile measure --run-id s285b-before --out ../tmp/canvas-cutover/evidence/s285b-before > ../tmp/canvas-cutover/evidence/s285b-before.log 2>&1`

Exit 1 is expected, because the thresholds still fail. This run never uses `--write-evidence`
and never appears in `verification[]`; cite it in notes. Record the before p95 per phase per
browser in the progress log.

If the implementer's environment can run the harness (the session-285 implementer did, see
its progress log), run this immediately and continue to TASK-509 in the same pass. If it
cannot bind localhost, stop after TASK-508 with a handoff note naming the three commands. The
outside step runs them, and the implementer resumes from the recorded numbers.

### TASK-509: Edit-path repair (sandbox; driven by the before table)

Apply design invariants 1-3. They are expected to dominate, but confirm each one against the
before table and record the phase share it removes. Apply invariant 4, or any further fix
(invariant 5), only for a phase that is the largest in either browser or that alone exceeds a
gate threshold after 1-3.

1. **One caret evaluation per transaction** (`view-host.ts`).
   - Add `readonly stats = { caretEvaluations: 0, rectReads: 0 }` to `CodeViewHost`, imitating
     `TextLayout.stats` (`layout.ts:24`).
   - Rewrite `keepCaretVisible` as arithmetic:
     - `line = state.doc.lineAt(head).number - 1`;
     - `top = line * lineHeight`, `bottom = top + lineHeight`;
     - if `top < scrollTop`, set `scrollTop = top`;
     - otherwise, if `bottom > scrollTop + (height - inset)`, set
       `scrollTop = bottom - (height - inset)`;
     - then clamp vertically.

     This is identical to the old rule, because `layout.ts:190` gives
     `top = view.top + line * lineHeight - scrollTop`. It reads no rect and calls no
     `coordsAtPos`.
   - **Authoritative line count (Step 5 finding S5-S285-CARET-COALESCE-STALE-LINECOUNT).**
     The vertical clamp must read the authoritative line count `surface.state.doc.lines`,
     never `layout.lineCount`. This applies to both the arithmetic caret rule and the
     `clampScroll` vertical bound:
     `maxTop = max(0, surface.state.doc.lines * lineHeight - max(1, height - inset))`.
     The caret line index comes from `surface.state.doc.lineAt(head)`. The rule and the
     clamp therefore use the same fresh source, and the result does not depend on subscriber
     order. That is what makes the coalesced second `scrollCaret` truly redundant. Only
     `font.lineHeight` is read from `layout`.
   - Coalesce:
     - Remember `lastEvaluated = { state, scrollTop, height, inset }` after each evaluation.
     - `scrollCaret()` returns immediately when `surface.state === lastEvaluated.state` and
       `scrollTop`, `height` and `inset` are unchanged.
     - The `selectionSet` subscription keeps evaluating.
     - A key or navigation that changes nothing, after the user has wheel-scrolled away, must
       still scroll back. `scrollTop` differs in that case, so the evaluation runs.
   - `setViewport` still evaluates.
   - Do not move the evaluation into the frame. It stays synchronous, so the completion popup
     and the diagnostic tip see the new scroll.
2. **Horizontal watermark** (`layout.ts`, `view-host.ts`, `mount.ts`).
   - `TextLayout` gains a private `widest = 0`, `get widestShaped(): number` and
     `resetWidestShaped(): void`.
   - Every `shape()` return, hit or build, does `widest = max(widest, line.width)`.
   - `invalidate()`, `setFont` (through `invalidate`) and `setDocument` reset it.
   - `mount.ts`'s surface subscriber calls `layout.resetWidestShaped()` when a transaction
     replaces the whole previous document: exactly one change with `fromA === 0` and
     `toA === startState.doc.length`. Use `update.changes.iterChanges`, which costs O(changes),
     never a document read.
   - `clampScroll` uses `layout.widestShaped` and shapes no line. Keep the existing
     `+ 48 - width` gutter arithmetic.
3. **Rect cache** (`view-host.ts`, `mount.ts`).
   - `CodeViewHost` keeps `rect: { left: number; top: number } | null` and
     `rectStale = true`.
   - The `viewport` getter reads `element.getBoundingClientRect()` (incrementing
     `stats.rectReads`) only when `rectStale` is true. It then caches the rect and clears the
     flag.
   - Set `rectStale = true`:
     - in `setViewport`;
     - on window `scroll` (capture, passive) and window `resize`, registered through the
       existing `listen` helper and removed in `dispose`;
     - in a new `invalidateRect()` that `mount.ts` calls at the start of each text-dirty
       `onFrame`;
     - on `pointerdown` on the element, so a gesture maps against a fresh rect.
   - Animation-only frames reuse the cache. Do not read the rect anywhere else in
     `view-host.ts`.
4. **Conditional** (`layout.ts`, only if the after-1-3 measurement shows it):
   - `setText` cache maintenance iterates `update.changes` instead of every cache entry.
   - It drops entries for touched lines, and for shifted lines whose `from`/`to` no longer
     match.
   - It never compares the text of untouched cached lines.
   - It keeps the `setText(doc === this.text)` early return.
5. **Conditional, further costs.** Fix them in the owning decision-1 file with a counter test.
   Examples: renderer upload or syntax spans, or diagnostics/eval scheduling on the main
   thread. Record the phase evidence first.

Pitfalls:

- Subscriber order: layout is stale during the caret evaluation.
  - `CodeViewHost` subscribes to the surface at `mount.ts:140`, before `InputController` at
    `mount.ts:143`. `CodeSurface.publish` (`surface.ts:95`) calls listeners in insertion
    order.
  - The `selectionSet` evaluation therefore runs before `onPresentation -> layout.setText`.
    Inside it, `layout` text and `layout.lineCount` still describe the previous document.
  - Nothing in `keepCaretVisible` or the vertical path of `clampScroll` may read layout
    document state (`lineCount`, `coordsAtPos`, `lineAt`). Use `surface.state` only.
  - Today this staleness is masked by the second, uncoalesced `scrollCaret` from
    `input.ts:169`. After coalescing it would leave the caret below the viewport on Enter or a
    multi-line paste at the end of the document.
  - Do not "fix" this by reordering the `mount.ts` subscriptions; other subscribers depend on
    the current order.
  - The horizontal clamp uses the `widestShaped` watermark, which may lag by one transaction.
    That is accepted.
- IME. During composition, the presentation document differs from `surface.state`. The caret
  rule uses `surface.state.selection.main.head` as today. Do not switch to the presentation
  doc.
- `coordsAtPos` still returns `view.top`-relative values from the cached rect. The existing
  completion-popup and diagnostic-tip tests must pass unchanged.
- jsdom returns all-zero rects. Tests must not assume a nonzero `top`.
- Do not add `ResizeObserver` here. `FrameScheduler` already delivers resizes through
  `setViewport`.
- Do not remove `scrollCaret` from `KeyboardOptions`. Coalescing makes the extra call free.

Test cases. They are deterministic and use no wall-clock assertion.

The new file `editor/test/canvas/view-host.test.ts` builds a unit rig:

- `CodeSurface` with a 20,000-line document (lines as in `edit-cost.test.ts:14`);
- `TextLayout` with a fake `measureText` (`width = length * 8`);
- `CodeViewHost` on a jsdom element with a fake `FrameHost`, and `setViewport({ width: 800, height: 600, dpr: 1, keyboardInset: 0 })`;
- `InputController` wired with `scrollCaret: () => viewHost.scrollCaret()`, as in `mount.ts:143`;
- production wiring order is required:
  - call `layout.setText(surface.state.doc)` at rig start;
  - construct `CodeViewHost` first;
  - then construct `InputController` with
    `onPresentation: (p) => layout.setText(p.doc)` and
    `scrollCaret: () => viewHost.scrollCaret()`.

  This makes the layout stale inside the `selectionSet` evaluation exactly as in production.

The rows:

- one `beforeinput insertText` at line 10,000 -> `stats.caretEvaluations` grows by exactly 1;
  a spied `element.getBoundingClientRect` is called 0 times inside the handler; and
  `layout.stats.builds` grows by at most the visible lines plus 2 (no 1024-line shaping);
- Enter (`beforeinput insertLineBreak`) at line 10,000, and a
  `surface.dispatch({ selection: { anchor: doc.length } })` jump to the document end -> each
  grows `caretEvaluations` by 1, and the caret line lies within
  `[scrollTop, scrollTop + height - inset]`;
- end of the document. Use a 200-line document in a viewport of height 600 with `lineHeight`
  20, so the document is taller than the viewport. Put the caret at the end of the last line
  and scroll to the bottom (`scrollTop` 3400).
  - `beforeinput insertLineBreak` -> `stats.caretEvaluations` grows by exactly 1, and the
    caret line lies within `[scrollTop, scrollTop + height - inset]` (expected `scrollTop`
    3420).
  - Separately, from the same starting state, paste 5 lines through
    `input.replaceSelection('a\nb\nc\nd\ne')` -> `caretEvaluations` grows by exactly 1, and
    the caret line is visible (expected `scrollTop` 3480).
  - These rows fail if the vertical clamp reads the stale `layout.lineCount`;
- control branch, in the same test: with the caret on line 1, `viewHost.scrollBy(0, 5000)`,
  then `viewHost.scrollCaret()` with an unchanged state -> exactly 1 evaluation that scrolls
  back to `scrollTop === 0`. A second `scrollCaret()` -> 0 evaluations. After
  `invalidateRect()`, one `viewport` access -> exactly 1 rect read, and a second access -> 0;
- watermark: shape a 400-column line, then scroll vertically to short lines -> `scrollLeft` is
  not clamped below its previous value. `resetWidestShaped()` -> `widestShaped === 0`;
- keyboard inset 300 -> the caret stays above the inset, matching the old rule's expectation in
  the existing input/mount tests.

Additions to `editor/test/canvas/mount.test.ts`, using the existing `setup(perf, withGl)` rig:

- a 20,000-line document, one keystroke, then `rig.host.step` -> the text-dirty frame makes at
  most 1 `getBoundingClientRect` call on the code host element (spy the prototype and filter
  on `this`);
- a `TextLayout.prototype.shape` build-count spy is at most
  `3 * ceil(viewHeight / lineHeight) + 2`, the mount overscan window at `mount.ts:180-181`;
- an animation-only frame -> 0 rect reads.

`editor/test/canvas/edit-cost.test.ts` needs no change unless invariant 4 lands. In that case,
add a row: one keystroke with N cached lines does at most (changed lines + 2) cache
evictions.

Existing caret, IME, touch, keyboard-inset, completion-popup, diagnostics and accessibility
assertions are not modified. Any test that relied on two evaluations may be updated only to
the new count, with the reason recorded.

### TASK-510: Measure -> fix -> re-measure (outside the sandbox, then gating)

After each TASK-509 round:

1. `mise run build-wasm-release`
2. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`
3. a non-gating attribution run:
   `cd editor && npm run e2e -- --browser all --profile measure --run-id s285b-after-<n> --out ../tmp/canvas-cutover/evidence/s285b-after-<n>`

Repeat until every gate metric passes in both browsers. Then run the canonical gating command
`cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001 > ../tmp/canvas-cutover/evidence/s285b-e2e-run-001.log 2>&1`.
It must exit 0. In both browsers, `summary.json` must show:

- sink installed before the first connect;
- 0 direct destination connections;
- post-sink peak exactly 0;
- numeric pre-sink peak dBFS, RMS and onsets;
- `wasm.profile === "release"`, `nameSection === true` and `dwarf === false`.

The "after" phase table is that run's `phaseMs`.

### TASK-511: Escalation F check (replaces TASK-504 for this run)

F triggers only when both of these hold after the TASK-509 fixes:

- `syncAbsMs` p95 or p99 still fails in WebKit or Chromium;
- the attribution shows main-thread starvation of `session_tick`. That means `tick` spans are
  delayed behind long `input`, `frame` or `upload` spans: tick `startMs` gaps above the 120 ms
  lookahead overlap spans longer than 50 ms.

If F triggers:

- record the failing metric, the overlapping spans (start, duration, kind) and the log paths in
  the progress log and the triage table;
- create no Worker file and stop. A design author must append 15.3.8.14 and checkpoint before
  `editor/worklet/tick-worker.js` or `editor/test/protocol/tick-worker.test.ts` is created.

If either condition is false, record "F not triggered" with the evidence.

### TASK-512: Evidence document (sandbox)

In `design-docs/specs/design-canvas-editor-evidence.md`, outside the harness marker section:

- Add a static section "Edit-path phase attribution (session 285)": a table with rows
  input, caret (scroll/caret), shaping, syntax, upload, frame and tick (scheduler). It has
  before and after p95 columns for WebKit and Chromium, with the source paths
  `tmp/canvas-cutover/evidence/s285b-before/` and `run-001/summary.json`.
- Update the WebKit and Chromium triage rows. Remove the "outside this plan's writePaths"
  wording, and name the fixed cause with its file and test.
- Keep the silent-sink method and the pending physical-iPad procedures unchanged.

### TASK-513: Final gates and simulator (outside the sandbox, serial)

Run the Verification and final-gate tables below in order, release build first. Run the iOS
simulator build and `ios-sim.mjs` after the release `npm run build`. `ios-sim.json` must show
`selfCheck.playingEvents === 0`, `silent === true` and `pass === true`.

Use the session-284 record format for every `verification[]` element.

### TASK-507 additions (closeout, serial, after the final integration review accepts)

Fresh-read and record sha256 before each edit, in `tmp/canvas-cutover/evidence/s285b-closeout-intent.json`.

1. **`impl-plans/active/canvas-cutover-evidence-scope.md:483` and `:530`.** Replace
   `fea4d7b7eee01383218e29c3b250ded35` with `fea4d4b7eee01383218e29c3b250ded35`, to match line
   260. First confirm that `shasum -a 256 src/directives/tests/labels.rs` prints
   `e11a8bc9a6c8f7c17963e0dde204b46fea4d4b7eee01383218e29c3b250ded35`. If it prints something
   else, record the actual value and do not edit.
2. **`tmp/canvas-cutover/framecost/receipt.json`.** Set `"editor/vite.config.ts"` to the
   output of `shasum -a 256 editor/vite.config.ts`. It currently records `2b12445f...`.
   Change only that value.
3. **`README.md:210-214`.** State that `VACTR_WASM` overrides the artifact path and that the
   defaults differ by consumer:
   - the vite page build (`npm run dev` and `npm run build`) defaults to
     `target/wasm32-unknown-unknown/release/vactr.wasm`, built by `mise run build-wasm-release`;
   - the vitest real-wasm suites default to `target/wasm32-unknown-unknown/debug/vactr.wasm`.

   Keep the `--lib` stub warning. Add `mise run build-wasm-release` to the code block above it.
   Change nothing else in `README.md`.
4. **Archive.** As TASK-507 and the session-283 closeout scope:
   - `git mv` the 14 `impl-plans/active/canvas-cutover-*.md` plans to `impl-plans/completed/`,
     each with `Status: Completed`;
   - `git mv` the 15 `impl-plans/active/canvas-editor-224-*.md` plans and
     `impl-plans/active/canvas-editor-224-dispatch.json`, each with a one-line superseded
     note;
   - make the typo fix of item 1 before moving `canvas-cutover-evidence-scope.md`.

   `impl-plans/active/canvas-cutover-dispatch.json` stays in `active/` (session-283 decision:
   it is the run manifest, not a plan). It is not moved.
5. Update `impl-plans/README.md`. Append the 15.3.8.4 clock-probe erratum to
   `design-docs/specs/design-implementation.md`, at the end of 15.3.8.4 only.
6. Commit, then `git push origin wf/canvas` (non-force).

### Session 285 checklist (mechanical)

- `git diff 2e3361d -- editor/test/e2e/stats.mjs` shows no change inside `THRESHOLDS`.
- `git diff 2e3361d -- editor/test/e2e/fixtures/large-doc.mjs editor/test/e2e/silent-sink.mjs editor/package.json editor/package-lock.json Cargo.toml Cargo.lock mise.toml`
  is empty.
- `git diff 2e3361d --name-only -- src editor/src-tauri` is empty.
- `git status --porcelain editor/worklet/tick-worker.js editor/test/protocol/tick-worker.test.ts`
  is empty unless TASK-511 triggered and 15.3.8.14 exists.
- `wc -l` on every touched `.ts` file is under 1000.

## Session 286 Amendment (operator decisions 1-8; design 15.3.8.14)

**Issue reference:** `workflowInput:RESUME-session-285` (resumed as session 286, the
WebKit-first performance wave).

**Order.** This plan runs after these six serial plans are accepted:

- CANVAS-OPT-HARNESS (`impl-plans/active/canvas-cutover-opt-harness.md`);
- CANVAS-OPT-HISTORY (`canvas-cutover-opt-history.md`);
- CANVAS-OPT-DOM (`canvas-cutover-opt-dom.md`);
- CANVAS-OPT-TEXT (`canvas-cutover-opt-text.md`);
- CANVAS-OPT-BACKDROP (`canvas-cutover-opt-backdrop.md`);
- CANVAS-OPT-RENDER (`canvas-cutover-opt-render.md`).

The design's single OPT-RENDER plan is refined into BACKDROP and RENDER, as 15.3.8.14
section 8 allows.

**Session 291 amendment (design 15.3.8.14 section 8, "OPT-RENDER split").**

- *Superseded plan.* CANVAS-OPT-RENDER now has status `Split` and is replaced by three
  serial plans:
  - CANVAS-OPT-RENDER-A (`canvas-cutover-opt-render-a.md`, wave 13);
  - CANVAS-OPT-RENDER-B (`canvas-cutover-opt-render-b.md`, wave 14);
  - CANVAS-OPT-RENDER-C (`canvas-cutover-opt-render-c.md`, wave 15).
- *Order.* This plan runs at wave 16, after C is accepted. CANVAS-OPT-BACKDROP was accepted
  in session 290 (commit 4262255).
- *Closeout.* TASK-605 also archives `canvas-cutover-opt-render-a.md`, `-b.md` and `-c.md`
  file by file, next to the parent `canvas-cutover-opt-render.md`. These paths are concrete
  writePaths in the manifest.
- *Unchanged.* No other task, threshold or gate changes.

**Baseline.** The session-285 WIP at `cea81cd` (TASK-508 attribution, TASK-509 edit-path
invariants) is the baseline. It is not redone. Its rows must still pass.

**Numbering.** Every reference in this plan to "15.3.8.14" for escalation F now means "a
new F design amendment at the next free 15.3.8.x number". 15.3.8.14 is the session-286
performance wave (design 15.3.8.13, as edited in session 286).

### TASK-601: Release re-measure (outside the sandbox)

- **Build.** `mise run build-wasm-release`, then `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`.
- **Gating run.** `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001`
  on a quiet host (no concurrent vitest, nextest, cargo or browser run). It must exit 0
  with the session-286 thresholds:
  - in both WebKit and Chromium: input p95 at most 50 ms and p99 at most 100 ms; frame
    interval p95 at most 20 ms and p99 at most 50 ms; text-dirty work p95 at most 8 ms;
    the animation-work and A/V sync gates of 15.3.8.8, computed on de-duplicated sync
    samples;
  - at least 500 edit keys;
  - post-sink peak 0, 0 direct destination connections, and the sink installed before the
    first connect (including the headed WebKit GL fallback);
  - `wasm.profile === "release"`, `nameSection === true` and `dwarf === false`.
- **Targets.** `summary.json` records the non-gating targets (textWork p95 4 ms, animation
  p50 1 ms) and the dropped-frame counts.
- **Failure handling.** If the run fails, attribute the failure with the session-285 phase
  rows (`?perf=1`; non-gating runs `s286-after-<n>` with
  `--out ../tmp/canvas-cutover/evidence/s286-after-<n>`, cited in notes only). Repair it
  in the owning file, which is a writePath of this plan (session-286 additions below), with
  a deterministic counter row. Re-run. Never loosen a threshold or change the workload.

### TASK-602: Escalations S and F

- **S (syntax Worker, design 15.3.8.14 section 7).** It triggers only if TASK-601 fails the
  input p95, frame p95 or textWork p95 gate and the attribution shows the deferred reparse
  task as the dominant long task overlapping late frames.
  - If it triggers: record the evidence. Append the S scope record to 15.3.8.14 (a design
    edit, serial, by the plan author). Then create `editor/src/code/syntax-worker.ts` and
    `editor/test/code/syntax-worker.test.ts` per section 7, and re-run TASK-601.
  - Otherwise: record "S not triggered" with the phase evidence.
- **F (tick Worker).** Same rule as TASK-511. If it triggers, stop for a new design
  amendment. The reserved tick-worker files are not created.

### TASK-603: Evidence document

In `design-docs/specs/design-canvas-editor-evidence.md`, outside the harness markers:

- add a static section "Performance wave (session 286)". It is a table of the diagnosis
  baseline (`tmp/canvas-cutover/diag-shape/REPORT.md`: WebKit 181 keys, input p95 307 ms,
  frame p95 174 ms, text p95 154 ms; Chromium text p95 16.8 ms) against the final
  `run-001` numbers, with raw data paths;
- note the sync de-duplication and dropped-frame reporting (a correctness fix, not a
  threshold change);
- record the S and F outcomes;
- keep the platform limitations and the pending physical-iPad procedures unchanged.

### TASK-604: Final gates and simulator

Run the final-gate table and the silent iOS simulator run (`playingEvents === 0`) on the
closeout source. Use the session-286 record format.

### TASK-605: Closeout additions

These extend TASK-507 and its session-285 additions:

- **Archive.** Also `git mv` the six `canvas-cutover-opt-*.md` plans to
  `impl-plans/completed/`, each with `Status: Completed`. That makes 20 canvas-cutover plans
  plus 15 canvas-editor-224 plans plus `canvas-editor-224-dispatch.json`.
  `canvas-cutover-dispatch.json` stays in `active/`.
- **Index.** `impl-plans/README.md` lists the session-286 plans as completed.
- **Erratum and typo fixes.** The 15.3.8.4 erratum and the deferred typo fixes of the
  session-285 additions stay as specified.
- **Push.** Commit and push (non-force) to `origin wf/canvas`.

### Session-286 path additions

These are writePaths in the manifest, for re-measure repairs only, under the seam protocol
(smallest change, counter row, hashes recorded):

- the OPT-plan source files and tests: `editor/src/code/{line-bytes,advances,geometry,palette}.ts`,
  `editor/src/code/{completion-types,code-view}.ts(x)`, `editor/src/code/code.css`,
  `editor/src/bind/{panel.ts,panel-view.tsx,bind.css}`, `editor/src/protocol/{store,utf8}.ts`,
  `editor/src/params/{roll.ts,telemetry-view.tsx}`, `editor/src/visual/{mount,panes}.ts`,
  `editor/test/bind/*` (listed file by file), `editor/test/protocol/{store,utf8}.test.ts`,
  `editor/test/params/displays.test.ts` and `editor/test/support/{gl,canvas}.ts`;
- the reserved S files;
- the six OPT plan files (active and completed).

### Session 286 checklist (mechanical)

- `git diff b9093e2 -- editor/test/e2e/stats.mjs` changes only `textWorkP95Ms` (16.7 to 8)
  inside `THRESHOLDS`.
- `git diff b9093e2 -- editor/test/e2e/fixtures/large-doc.mjs editor/test/e2e/silent-sink.mjs editor/package.json editor/package-lock.json Cargo.toml Cargo.lock mise.toml`
  is empty.
- `git diff b9093e2 --name-only -- src editor/src-tauri` is empty.
- `grep -c opacity editor/src/bind/bind.css` is 0.
- `editor/worklet/tick-worker.js` does not exist. `editor/src/code/syntax-worker.ts` exists
  only if S triggered.

## Session 302 Amendment (user approval 2026-10-07, Option A; design 15.3.8.15)

**Issue reference.** `workflowInput:RESUME-session-292`, resumed as session 293
(workflow execution `opus-luna-design-and-implement-review-loop-session-293`). The finding
is INT-S302-EV-STALL-SYNC-GATE.

**User approval.** The user was asked about this on 2026-10-07 and approved Option A. Stall
windows are recorded explicitly. Sync samples are classified by those windows. Stall-window
samples are gated by stall-recovery criteria instead of `syncAbsMs`. Thresholds, the 250 ms
stall and the workload stay unchanged. No sample is ever excluded by its latency value. The
approval is also recorded in design 15.3.8.15 and in the manifest's `operatorRules`.

### Intent and context

All OPT plans and every earlier canvas-cutover plan are accepted. Only this plan remains.

The canonical silent release-wasm `run-001` (`tmp/canvas-cutover/evidence/s291-final-bundle-e2e.log`)
fails three gates:

- Chromium `syncAbsMs.p99`: 157.4 ms;
- WebKit `syncAbsMs.p99`: 155.7 ms;
- WebKit `beatDriftMs`: 1.50 ms, against the 1 ms gate.

Every other gate passes:

- input, frame and text p95;
- behavior checks, 18/18;
- post-sink peak 0;
- 0 direct destination connections.

Design 15.3.8.15 gives two fixes:

- **Part A (harness).** Classify sync samples by stall windows that `measure.mjs`
  records.
- **Part B (product).** The beat drift comes from `src/session/publish.rs`. It pairs
  `sample_time = host_now` with `cycle = rt.pos`, and `rt.pos` is floored to the 1/960-cycle
  `GRID` (`src/sched/runtime.rs:48` and `:544`). This makes each sample up to 2.083 ms
  inconsistent at 120 bpm with 4 beats per cycle. The run-001 drift values, 0.1667 ms and
  1.5000 ms, are exact multiples of 1/12 ms, and stall windows cannot produce lattice-exact
  values.

The tasks below run in order: TASK-701 (B), TASK-702 (A), TASK-703 (re-measure), then the
evidence document, final gates and closeout. TASK-701 and TASK-702 touch disjoint files, so
either may come first. Both must pass their focused checks before TASK-703.

### Non-goals

- No threshold, workload, stall duration (250 ms), stall period (every other 5 s cycle),
  silent-sink rule, `large-doc.mjs` fixture or `editor/package.json` change.
- No Session Protocol field, frontend clock formula (`editor/src/code/transport.ts`,
  `editor/src/app/clock.ts`), wasm ABI, `CodeSurface` or `__vactrPerf` shape change.
  `__vactrPerf` is not touched at all, because the stall windows come from `measure.mjs`.
- The frame-interval computation (`measure.mjs`, the `dt <= 200` filter) is not changed. It
  is only reported, split into stall and non-stall exclusions.
- No widening of the classification beyond conditions (a) and (b). A second or later late
  frame after a stall stays a non-stall sample.
- No tick Worker (escalation F) and no syntax Worker (S). If either is ever needed, it takes
  design 15.3.8.16.
- No mutation or negative-control commands. Sensitivity is shown only by in-test control
  branches.
- No edit to `.agents/settings.local.json`. No crate-wide `cargo fmt`.

### Ownership changes (manifest entry `CANVAS-EVIDENCE`)

- New writePaths:
  - `src/session/publish.rs`
  - `src/session/tests/publish.rs`
  - `editor/test/wasm/canvas-clock.test.ts`

  The wasm test asserts `sample_time` equals the tick time exactly, at `:104`, `:124` and
  `:174`. Part B changes that contract, so the test is ported, not weakened.
- `editor/test/e2e/README.md` moves from sharedPaths to writePaths. It documents the stall
  classification.
- `editor/test/e2e/measure.mjs`, `stats.mjs`, `stats.test.ts`, the evidence document, the
  `run-001` files, `design-implementation.md` (closeout erratum only) and every archive path
  are already writePaths.
- `editor/test/e2e/run.mjs` stays a sharedPath. Edit it only if the summary wiring needs the
  new metrics, and record the reason.
- artifactRoots are unchanged. They already cover `target`, `tmp/canvas-cutover/evidence`,
  `tmp/canvas/evidence`, `editor/dist` and the Apple build outputs. Logs for this amendment
  use the prefix `tmp/canvas-cutover/evidence/s293-`.

### TASK-701: Transport-sample pairing fix (part B; sandbox)

**Files:**

- `src/session/publish.rs` (590 lines);
- `src/session/tests/publish.rs` (673 lines);
- `editor/test/wasm/canvas-clock.test.ts`.

**Change.** Change the `TransportSample { .. }` construction near `publish.rs:555`. Nothing
else in `publish.rs` changes.

- **Running sample.** When `!frozen && !lost`, compute
  `candidate = self.rt.clock().to_host(self.transport_cycle)`, using `Clock::to_host` at
  `src/clock/clock.rs:179`. Use it as `sample_time` only when all three hold:
  - `candidate` is finite;
  - `candidate >= 0`;
  - `(candidate - host_now).abs() <= grid_period + 1e-9`.

  Here `grid_period = 1 / (960 * cps)` seconds, and
  `cps = tempo.bpm / 60 / tempo.beats_per_cycle`, taken from the `tempo` already read at
  `:466`. Otherwise `sample_time = host_now`.
- **Not running** (paused, frozen or lost): `sample_time = host_now`.
- **Unchanged:**
  - `last_transport = Some(host_now)`, which drives the rate ceiling;
  - the `transport_state` tuple and every epoch rule (`:470-:494`);
  - `LevelsBody.time = Some(host_now)`;
  - the `cycle` field (still `ratio_pair(self.transport_cycle)`).
- **Grid period.** Use the runtime grid constant. `GRID` is private in
  `src/sched/runtime.rs`. Do not make it public and do not edit `runtime.rs` (it is not owned).
  Use a local `960.0` constant in `publish.rs` with a comment naming `sched::runtime::GRID`.
  The test then pins the value: it asserts the bound against a grid period computed
  independently.

**Pitfalls.**

- Never move `last_transport` or the epoch state onto the derived time. The rate-ceiling test
  and the rollback epoch must keep keying on `host_now`.
- After a host-clock rollback, `rt.pos` keeps its maximum (`.max(self.pos)`). So
  `to_host(pos)` lies far from `host_now`, and the guard must fall back. The wasm test at
  `canvas-clock.test.ts:178-195` (rewind to 0.25 s, resume at 0.301 s) relies on this and
  must still see `sample_time` equal to 0.25 and 0.301 exactly.
- `to_host` returns `0.0` when there is no anchor. The guard handles this through the
  distance check.
- Do not add `#[allow]` or `expect` to silence clippy. Clippy's cast and float lints apply.
  Compare with `abs()`, never with float equality.

**Rust test changes** (`src/session/tests/publish.rs`; imitate the existing `Rig`,
`transport_samples` and `rig.clock.set` usage at `:365-:407`, and `Rig::ok` in
`src/session/tests/support.rs:103`).

- Port `periodic_transport_uses_matching_host_time_and_runtime_cycle_with_rate_ceiling`. A
  rename is allowed (for example
  `periodic_transport_pairs_cycle_with_its_exact_host_time_with_rate_ceiling`).
  - Keep `cycle == ratio_pair(runtime pos)`, `running`, `latency_kind` and the 19-21 sample
    count.
  - Replace `sample_time == time` with `0 <= time - sample_time < 1/(960*cps) + 1e-9`.
  - Measure the 0.05 s rate ceiling on the rig tick times recorded alongside each sample, not
    on `sample_time`.
  - Keep the restart assertions: a new epoch, and `sample_time == 0.0` at clock 0.
- Add a new test, for example `transport_sample_time_matches_cycle_at_quantum_ticks`.
  - Drive `rig.clock.set(n * 128 / 48000)` for `n` covering at least 20 simulated seconds.
  - Case 1 is the default 120 bpm with 4 beats per cycle.
  - Case 2 is a non-lattice tempo set with `rig.ok("use-bpm 137 ...", rev)`. Use 3 beats per
    cycle if the language exposes a beats-per-cycle directive; otherwise keep 137 bpm at
    4 beats and say so in the progress log. Only assert pairs whose samples both come after
    the tempo change.
  - Assert, for every pair of running samples in one epoch:
    `|((c_j - c_i) / cps) - (t_j - t_i)| <= 1e-9`.
  - Assert, for every running sample: `0 <= host_now - sample_time < 1/(960*cps) + 1e-9`.
  - Assert that `sample_time` is non-decreasing within an epoch.
  - *In-test control branch:* re-pair the same samples' cycles with their tick `host_now` and
    assert the maximum pair inconsistency is above 1 ms. This shows the defect class is
    real at this tick lattice.
- Keep every other test in the file (pause, lost, MIDI restart, epoch) unchanged, and keep it
  passing.

**Wasm test port** (`editor/test/wasm/canvas-clock.test.ts`).

- `:104`: replace `toBe(ms / 1000)` with `0 <= ms/1000 - sample_time < 1/(960*0.5) + 1e-9`.
  Also assert `cycle[0]/cycle[1]` equals `sample_time * 0.5` within `1e-9`, since the
  workload is 120 bpm with 4 beats per cycle, so cps is 0.5.
- `:119`: keep it.
- `:124`: measure the spacing on the recorded tick times, not on `sample_time`.
- `:174`: replace `toBe(0.05)` with `toBeCloseTo(0.05, 9)`. Position 24/960 lies exactly on
  the grid.
- `:183`, `:186` and `:193`: keep them exact. These are the fallback and zero cases.
- No other assertion changes, and no assertion is deleted.

**Done when:**

- the focused nextest filter `session::tests::publish` passes;
- `cargo clippy --locked --all-targets -- -D warnings` exits 0;
- `rustfmt --edition 2021 --check src/session/publish.rs src/session/tests/publish.rs`
  exits 0;
- the host-wasm wasm32 build exits 0, and then
  `cd editor && ./node_modules/.bin/vitest run test/wasm/canvas-clock.test.ts` passes;
- `git diff --name-only HEAD -- src` lists only the two publish files.

### TASK-702: Stall-window classification (part A; sandbox)

**Files:**

- `editor/test/e2e/measure.mjs`;
- `editor/test/e2e/stats.mjs` (468 lines);
- `editor/test/e2e/stats.test.ts`;
- `editor/test/e2e/README.md`.

**`measure.mjs`:**

- **Record the window.** The busy loop at `:56` becomes one `page.evaluate` that returns
  `{ startMs, endMs }`:
  - read `startMs = performance.now()` once;
  - loop until `performance.now() >= startMs + 250`, deriving the deadline from that same
    reading, because WebKit coarsens the clock to 1 ms;
  - read `endMs = performance.now()` after the loop.

  Push `{ index, startMs, endMs }` to a `stallWindows` array, and count the injected stalls
  in `injectedStallCount`, incremented in the same `if (cycle % 2 === 1)` branch. Push each
  window to `samples` as a `{ phase: 'stall-window', ... }` row.
- **Late-frame audit.** Replace the gap inference at `:91` (frame delta over 200 ms) with
  `stallWindows.map((w) => w.startMs)` passed to `lateActiveMismatchDetails`. That function
  already selects the first presented row with `frameMs >= stall`, which is F. Delete the gap
  loop.
- **Classification.** After `attributeSync` runs, call the new stats function (below) with
  the sync samples, `presented` and `stallWindows`. Then set:
  - `metrics.syncAbsMs`: non-stall samples, gated;
  - `metrics.syncAbsMsAll`: every sample, informational;
  - `metrics.stallWindowSync`;
  - `metrics.stallWindows`;
  - `metrics.stallWindowsInjected`.

  Keep `syncAbsMsNoDrop` as it is. Keep `sync-sample` raw rows, and add a
  `stallClass: 'stall' | 'non-stall'` field and the condition flags `{ a, b }` to each.
- **Beat residual.** Replace the single-point computation at `:87` with the new stats
  function `beatResidualMs(row, transport)`.
  - `metrics.beatDriftMs` is the residual of the last presented row that is not an F frame.
  - `metrics.beatResidual` is `{ maxAbsNonStallMs, maxAbsStallFrameMs, frames }`, informational.
  - Each window entry carries its F residual.
- **Frame intervals.** Keep the `dt <= 200` filter at `:74`. Add
  `metrics.frameIntervalExcluded = { stall, nonStall }`. An excluded interval is `stall` when
  its later frame is the F of a recorded window, and `nonStall` otherwise.

**`stats.mjs`** (new exports, pure data, no browser):

- **Classification.** The signature is
  `classifyStallSamples(samples, presented, windows) -> { stall, nonStall, windows: [...], counts: { a, b, both } }`.
  - **F.** F of a window is the first index in `presented` with `frameMs >= startMs`.
  - **Onset page time.** It is
    `sample.frame.targetMs + (sample.time - sample.frame.audibleTime) * 1000`, computed from
    those fields. Never derive it from `sample.value`.
  - **Condition (a)** holds when the onset page time is in `[startMs, endMs]`.
  - **Condition (b)** holds when `sample.frameIndex === F` or `sample.frameIndex + 1 === F`.
  - Every sample lands in exactly one of `stall` or `nonStall`.
- **Window entries.** Each entry is
  `{ index, startMs, endMs, firstFrameMs, firstFrameLagMs, samples, activeSetMatch, replayed, early, beatResidualMs, audited }`:
  - `firstFrameLagMs = firstFrameMs - endMs`;
  - `audited` uses the same sync-window and coverage test as `lateActiveMismatchDetails`.
- **Beat residual.** The signature is `beatResidualMs(row, transport)`. It returns `null`
  unless all of these hold:
  - `transport.running`;
  - `row.epoch === transport.epoch`;
  - `row.beatCycle` is finite.

  Otherwise it returns
  `(row.beatCycle - (c + (row.audibleTime - transport.sample_time) * bpm / 60 / bpc)) * 60 * bpc / bpm * 1000`,
  where `c = cycle[0] / cycle[1]`. This is the formula at `measure.mjs:87`, plus the epoch
  check.
- **Gates in `evaluate`.** When `metrics.syncProvenance === 'measured'`:
  - `syncAbsMs` keeps its existing `limit` calls; it now holds non-stall samples only.
  - Fail when `stallWindowSync.earlyCount > 0`.
  - Fail when `stallWindowsInjected !== stallWindows.length`.
  - Fail when any window has `endMs - startMs < 250`.
  - Fail when `stallWindowsInjected > 0` and zero windows are audited.
  - Fail when any audited window has `|beatResidualMs| > thresholds.lateBeatDriftMs`.
  - Fail, with the message `beat drift unavailable`, when `metrics.audioRunning === true` and
    `beatDriftMs` is not finite. This replaces the limitation in `measure.mjs:94`.

  The existing replayed-flash and active-mismatch failures stay. No `THRESHOLDS` value
  changes.
- **`renderEvidence` rows.** Relabel the sync rows "non-stall samples". Add rows for:
  - all-sample p95/p99 (informational);
  - stall-window sample count by condition, and their p50/p95/max (informational);
  - stall windows injected, recorded and audited;
  - stall-window early flashes (`0`);
  - F beat residual max (`<= 1`);
  - frame-interval exclusions, stall and non-stall (informational).

  Change the dropped-frame row text to say the gate covers non-stall samples.

**`stats.test.ts` rows** (plain data; imitate the existing `attributeSync` and
`lateActiveMismatches` rows). Each row is a situation and its expected outcome:

- An onset page time inside `[startMs, endMs]` is `stall` with `a`.
- An onset 10 ms before `startMs`, shown on the last pre-stall frame, with F as the proxy
  frame, is `stall` with `b`.
- An onset first shown at F is `stall` with `b`.
- F's timestamp in `[startMs, endMs)` (a straddling frame) is still F.
- *Latency-independence control,* in one test: a sample with value 300 ms and no window is
  `nonStall`, and `evaluate` fails `syncAbsMs.p99`. The same sample placed inside a
  recorded window is `stall`, and the non-stall p99 passes. Both branches are asserted.
- A sample on the second frame after F, with an onset after `endMs`, stays `nonStall`.
- A stall-window sample with value -3 ms fails the early rule.
- An F residual of 1.01 ms fails and 1.0 ms passes.
- `stallWindowsInjected` 11 against 10 recorded windows fails.
- A window of 249 ms fails.
- Audio running with `beatDriftMs: null` fails with `beat drift unavailable`.
- `beatResidualMs` returns `null` on an epoch mismatch and on `running: false`.
- Existing rows stay unchanged and keep passing. Any row that relied on gap-inferred stalls
  is ported to explicit windows, with the same expectations.

**`README.md`.** Add one paragraph on stall-window recording, classification and the
stall-recovery gates, citing design 15.3.8.15.

**Pitfalls.**

- Never filter, cap or sort samples by `value` to classify them. The control row exists to
  catch this.
- `presented` is the merged, `frameMs`-sorted array built at `measure.mjs:81`. The F index
  and `sample.frameIndex` must refer to that same array. `attributeSync` indexes it, so pass
  the same array.
- Window `startMs` and `endMs` are page `performance.now()` values. Never use Node
  `Date.now()` for them.
- `stats.mjs` must stay under 1,000 lines. Split helpers into a new
  `editor/test/e2e/stall.mjs` only if it would cross the limit; that file is then a recorded
  addition to writePaths.

**Done when:**

- `cd editor && ./node_modules/.bin/vitest run test/e2e` passes, with the new rows included;
- `node --check` passes on the harness files;
- `cd editor && npm run check` exits 0;
- `git diff -- editor/test/e2e/stats.mjs` shows no change inside `THRESHOLDS` or `TARGETS`.

### TASK-703: Release re-measure (replaces TASK-601; outside the sandbox, quiet host)

1. Run `mise run build-wasm-release`, then
   `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` (inspection `profile: "release"`,
   `nameSection: true`, `dwarf: false`).
2. Run `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001`.
   Nothing else may run at the same time: no vitest, nextest, cargo or other browser run.
3. It must exit 0. In Chromium and WebKit:
   - non-stall `syncAbsMs` p95 <= 33.4 ms and p99 <= 50 ms;
   - stall-window early count 0, 0 active-set mismatches, 0 replayed flashes, and F beat
     residuals <= 1 ms;
   - `beatDriftMs` finite and <= 1 ms in absolute value;
   - window integrity holds;
   - every session-286 gate still passes: input p95 <= 50 ms and p99 <= 100 ms; frame
     interval p95 <= 20 ms and p99 <= 50 ms; textWork p95 <= 8 ms; animation-work gates;
     at least 500 edit keys;
   - behavior 18/18;
   - post-sink peak 0, 0 direct destination connections, and the sink installed before the
     first connect, including the headed WebKit GL fallback.
4. **If non-stall p99 still fails.** Look at the failing samples' `stallClass`, onset page
   time and frame indices in the JSONL. A late sample outside conditions (a) and (b) is a
   product finding. Attribute it with the session-285 phase rows (non-gating runs
   `s293-after-<n>` with `--out ../tmp/canvas-cutover/evidence/s293-after-<n>`, cited in
   notes only). Repair it in a writePath with a deterministic counter row. Never widen the
   classification and never change a threshold. If the fix needs a file outside writePaths,
   stop and report the exact path for a serial plan-author amendment.
5. **Beat drift data.** Record the post-fix `beatResidual` maxima per browser, which are
   expected at f64 noise level. Next to them, record the pre-fix run-001 values
   (0.1667 ms and 1.5000 ms) and the 1/12 ms lattice explanation.

### TASK-704: Evidence document (extends TASK-603; sandbox)

Work in `design-docs/specs/design-canvas-editor-evidence.md`, outside the harness markers.
Add a static section "Stall-window sync classification (session 293, design 15.3.8.15)"
containing:

- the user approval (2026-10-07, Option A);
- the classification rule;
- a per-browser stall-window table, one row per window from `metrics.stallWindows`;
- non-stall, all-sample and stall-window sync figures side by side;
- the frame-interval exclusion split;
- the beat-drift diagnosis: the defect, the fix, the lattice values before and the
  residuals after.

Update the triage table so the three run-001 failures are resolved, citing the new run. Keep
the raw data paths, the silent virtual-sink method, the platform limitations and the pending
physical-iPad procedures. Never claim physical-iPad, real-IME, VoiceOver or physical latency
results.

### TASK-705: Final gates (replaces TASK-604; outside the sandbox, strictly serial)

Run these in order on the closeout source, one heavy command at a time. Each record uses the
session-286 format.

1. `CARGO_TERM_QUIET=true cargo build`
2. `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
3. Full nextest, alone:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
5. `cd editor && npm run check`
6. `cd editor && ./node_modules/.bin/vitest run`
7. `cd editor && npm run test:perf`, alone
8. `cd editor && npm run test:style`
9. `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml`
10. `rustfmt --edition 2021 --check src/session/publish.rs src/session/tests/publish.rs`
11. The silent iOS simulator build and `ios-sim.mjs` run (`playingEvents === 0`,
    `silent === true`, `pass === true`), after the release wasm and page build.

The TASK-703 `run-001` is the e2e record. If any source changes after it, repeat TASK-703.

### TASK-706: Closeout (TASK-507 and TASK-605 sets; serial, after the final integration review accepts)

- **Archive.** `git mv` every path below to `impl-plans/completed/` with `Status: Completed`
  or a one-line superseded note. Each is a concrete writePath. `canvas-cutover-dispatch.json`
  stays in `active/`.
  - The 23 canvas-cutover plans: `clock`, `native`, `render`, `mount`, `visual`, `shell`,
    `evidence`, `evidence-silent`, `evidence-viewport`, `evidence-editcost`,
    `evidence-runstart`, `evidence-scope`, `evidence-sched`, `evidence-framecost`,
    `opt-harness`, `opt-history`, `opt-dom`, `opt-text`, `opt-backdrop`, `opt-render`,
    `opt-render-a`, `opt-render-b` and `opt-render-c`.
  - The 15 `canvas-editor-224-*.md` plans.
  - `canvas-editor-224-dispatch.json`.
- **Index.** Update `impl-plans/README.md`.
- **Erratum and typos.** Append the 15.3.8.4 clock-probe erratum to `design-implementation.md`.
  Apply the deferred typo fixes of the TASK-507 session-285 additions (the scope-plan sha256
  typo, the framecost receipt hash, and the README wasm default text).
- **Commit and push.** Commit, then push non-force to `origin wf/canvas`.

### Invariants (must hold after every task)

- `THRESHOLDS` and `TARGETS` in `stats.mjs` are byte-identical to HEAD `c10ab72`.
- `git diff c10ab72 -- editor/test/e2e/fixtures/large-doc.mjs editor/test/e2e/silent-sink.mjs editor/package.json editor/package-lock.json Cargo.toml Cargo.lock mise.toml`
  is empty.
- `git diff c10ab72 --name-only -- src editor/src-tauri` lists only `src/session/publish.rs`
  and `src/session/tests/publish.rs`.
- `src/session/protocol.rs` and `src/session/codec.rs` are unchanged, so no protocol field
  changes.
- `editor/src/code/transport.ts`, `editor/src/app/clock.ts` and
  `editor/src/code/perf-hook.ts` are unchanged.
- No touched source file reaches 1,000 lines.
- Every automated audio run stays silent.

### Session 302 checklist (mechanical)

- `grep -n "stall-window" editor/test/e2e/measure.mjs` finds the window row.
  `grep -n "frames\[i\]\[0\]-frames\[i-1\]\[0\]>200" editor/test/e2e/measure.mjs` finds
  nothing, because the gap inference is gone.
- `grep -n "classifyStallSamples\|beatResidualMs" editor/test/e2e/stats.mjs` finds both
  exports.
- The `stats.test.ts` latency-independence control row exists and passes.
- `git diff c10ab72 -- src/session/tests/publish.rs` contains the in-test control branch
  (pair inconsistency above 1 ms when re-paired with `host_now`).

## Verification

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/e2e` | stats and generator tests pass, including the session-274 `attributeSync`, `lateActiveMismatches`, `rankSelfTime` and `classifyWithControl` rows |
| `node --check editor/test/e2e/run.mjs editor/test/e2e/serve.mjs editor/test/e2e/behavior.mjs editor/test/e2e/measure.mjs editor/test/e2e/stats.mjs editor/test/e2e/ios-sim.mjs editor/test/e2e/silent-sink.mjs editor/test/e2e/fixtures/large-doc.mjs` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/canvas test/code test/app test/bind test/visual test/e2e` | exit 0 (seam tests green after any seam edit; new operation-counter rows pass) |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 (default config; perf files excluded) |

Outside the sandbox (verification and review step; logs in `tmp/canvas-cutover/evidence/`):

| Command | Required evidence |
|---------|-------------------|
| `mise run build-wasm-release` (session 277, first) | exit 0; `target/wasm32-unknown-unknown/release/vactr.wasm` exists |
| `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 (no `VACTR_WASM`; release default since session 277); the FRAMECOST `inspectWasm` check prints `profile: "release"`, `nameSection: true`, `dwarf: false` |
| `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` | exit 0 is required for acceptance (session 267). Exit 1 is written to the evidence file and triaged under TASK-008. A product defect blocks acceptance until it is repaired; only a recorded environment limitation (for example software GL frame metrics) may remain as a documented failure, and that is a review decision. Exit 2 is blocked, never a pass. The silent-sink assertions of TASK-007 must hold in every case. |
| `tmp/canvas/tools/cargo-tauri ios build --target aarch64-sim` (debug, unsigned; cwd and flags as in `canvas-cutover-shell.md`), then `git status --porcelain editor/src-tauri` | exit 0; the `Vactr.app/Vactr` mtime is later than the build start; no tracked change is kept |
| `node editor/test/e2e/ios-sim.mjs --app editor/src-tauri/gen/apple/build/arm64-sim/Vactr.app --device "iPad Pro 11-inch (M5)" > tmp/canvas-cutover/evidence/s271b-ios-sim.log 2>&1` | exit 0; `ios-sim.json` has a non-null `selfCheck` from a Vactr-process line, `playingEvents === 0`, `silent === true`, `pass === true` and `appBinary.mtimeIso` |
| Diagnostic, not gating: `cd editor && npm run e2e -- --browser chromium --profile measure --profile-trace --run-id s274-trace --out ../tmp/canvas-cutover/evidence/s274-trace` | writes `tmp/canvas-cutover/evidence/s271b-trace-chromium-{edit,cycle}.json`; the top-10 `rankSelfTime` output is recorded in the progress log |
| Session 285, diagnostic, not gating: `cd editor && npm run e2e -- --browser all --profile measure --run-id s285b-before --out ../tmp/canvas-cutover/evidence/s285b-before` (after TASK-508, before TASK-509) and `... --run-id s285b-after-<n> --out ../tmp/canvas-cutover/evidence/s285b-after-<n>` | `phaseMs` per browser in the run summary; exit 1 is allowed; cited in notes only, never in `verification[]` |

Session 285 sandbox rows (added; the rows above still apply):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/view-host.test.ts test/canvas/frame.test.ts test/canvas/mount.test.ts test/canvas/edit-cost.test.ts test/canvas/input.test.ts test/protocol/host-js.test.ts test/e2e/stats.test.ts` | exit 0; the TASK-508 and TASK-509 rows pass |
| `cd editor && ./node_modules/.bin/vitest run test/code/completion-view.test.ts test/code/diagnostics.test.ts test/code/highlight.test.ts test/app/main.test.ts test/protocol/worklet-quad.test.ts test/canvas/gpu.test.ts test/canvas/clock.test.ts` | exit 0 (unchanged assertions) |
| `git diff --check` | exit 0 |

Final gates on the closeout commit (design 15.3.8.9):

| Command | Required evidence |
|---------|-------------------|
| `cargo build` | exit 0 |
| `cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | all pass (a timeout kill is neither a pass nor a failure) |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | all pass (default config; perf files excluded) |
| `cd editor && npm run test:perf` (alone) | exit 0 (session 271, design 15.3.8.11) |
| `cd editor && ./node_modules/.bin/vitest run test/canvas/no-editor-view.test.ts` | pass (zero `@codemirror/view` or `EditorView` in production `editor/src`) |
| e2e | as above |
| `cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |
| `rustfmt --check` on touched Rust files (session 277: `rustfmt --edition 2021 --check src/types/scope.rs src/types/tests/scope_cost.rs src/directives/attach.rs src/directives/tests/attach.rs src/directives/tests/labels.rs`) | exit 0 |
| iOS simulator build and `ios-sim.mjs`, as above, after `mise run build-wasm-release` and the release `npm run build` | exit 0; `playingEvents === 0`, `silent === true`, `pass === true` |

Session 302 rows (added; the rows above still apply). These run inside the sandbox unless
marked:

| Command | Required evidence |
|---------|-------------------|
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests::publish/)'` | exit 0; the ported periodic-transport row and the new quantum-tick pairing row pass, with their in-test control branch |
| `rustfmt --edition 2021 --check src/session/publish.rs src/session/tests/publish.rs` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`, then `cd editor && ./node_modules/.bin/vitest run test/wasm/canvas-clock.test.ts` | exit 0; every ported row passes and none is deleted |
| `cd editor && ./node_modules/.bin/vitest run test/e2e` | exit 0; the TASK-702 rows pass, including the latency-independence control |
| `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` (outside the sandbox, alone) | exit 0, as specified in TASK-703 |

## Overwrite and Drift Protocol

Record fresh-read sha256 values before each edit or move (`tmp/canvas-cutover/evidence/intent.json`)
and after it (`receipt.json`). Closeout runs serially, after all other plans are accepted. Edit
only this plan's progress log and the plan files being archived.

## Completion Criteria

- [x] Harness and unit tests in place; `run.mjs` preserves exit 0/1/2 semantics and fails on any measurement failure.
- [x] Full 20,000-line fixture is checked through host-WASM `session_check`, with exactly one check record and zero error diagnostics.
- [x] Evidence document has the harness-generated results, raw paths, explicit browser limitations, paired-edit counts, and pending physical-iPad procedures.
- [x] Active-workload attribution is control-backed: the built-in pad control emits positive onsets in both browsers; the 20,000-line toolbar click times out in Chromium and returns in WebKit. WebKit records workload onsets but no presented range pairs. Sink metrics and editing-key pairing are reported separately; all start and presentation defects remain product failures.
- [x] Simulator evidence recorded for the rebuilt iPad Pro 11-inch (M5): the Vactr-process self-check reports `playingEvents === 0`; the harness records `silent === true`, `pass === true`, and a binary mtime after 6f6b807 (session 274 TASK-401).
- [x] Session 274 TASK-401 INT-S271-EV-SIM: `ios-sim.json` has a latest Vactr-process self-check, `playingEvents === 0`, derived `silent === true` and `pass === true`, and app binary mtime later than 6f6b807.
- [x] Session 274 TASK-402 harness implementation: `syncWindow`, `attributeSync`, `lateActiveMismatches`, intersection window, eviction guard, candidate/sample split and `excludedFrames`; rows (a)-(h) and (i)-(iii) pass in `stats.test.ts`; thresholds are unchanged.
- [ ] Session 274 TASK-402 measured sync acceptance: Chromium has no measured workload window because large-document start timed out; WebKit has 4,032 in-window onsets but zero presented range pairs and no numeric sync percentiles. This remains a product/highlight failure.
- [x] Session 274 TASK-403 INT-S271-EV-READBACK: canvas-only-text passes in both browsers with line-1 same-frame readback; no production `preserveDrawingBuffer`.
- [x] Session 274 TASK-404 INT-S271-EV-TOUCH: WebKit synthetic tap and long-press pass; selected offsets 2–8 with two handles and recorded PointerEvent geometry/attributes in `s274-webkit-touch-events.json`. No `pointer.ts` edit was needed.
- [x] Session 274 TASK-405 diagnostics: `--profile-trace`, `rankSelfTime`, WebKit control page and `classifyWithControl` tests are present; top-10 profiles and control metrics are recorded.
- [ ] Session 274 TASK-406/product acceptance: browser `summary.json` is not passing; the dominant `vactr.wasm` self-time share is 70.56%–76.58% and is not resolved to an authorized Rust source file. A serial plan-author write-path amendment is required before the dominant-cost repair; product thresholds remain failing.
- [ ] Session 274: `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` exits 0 with every silent-sink assertion holding
- [x] Session 267: wave-3 plans SILENT, VIEWPORT, EDITCOST and RUNSTART accepted (the runtime fanout item lists all four in `acceptedPlanIds`)
- [x] Session 269 seam protocol for current edits: additive `PresentedRecord.epoch` seam has a deterministic `mount.test.ts` row; all edited seams have triage/evidence entries and source hashes below.
- [ ] Session 274 dominant-cost ownership: the largest profile share is a WASM module without a confirmed Rust source owner in this plan’s writePaths; obtain a serial amendment naming exact source path(s) before fixing it.
- [x] Session 267/274: silent `run-001` regenerated; both browser sinks were installed before first connect, direct destination connections were 0, post-sink peak was exactly 0, numeric workload pre-sink metrics were recorded, control/workload onsets were positive, and simulator self-check `playingEvents === 0` is evidenced.
- [x] Session 267: evidence document has the silence method, the native/simulator silence audit and the remaining-failure triage table
- [ ] Session 267: final integration review across the seven canvas-cutover plans and four wave-3 plans passes
- [ ] Closeout moves done file by file, including the four wave-3 plans; README updated (serial closeout after review)
- [ ] Final gates recorded with exit codes and log paths (closeout gate)
- [ ] Session 277: CANVAS-EVIDENCE-SCOPE, -SCHED and -FRAMECOST accepted (runtime `acceptedPlanIds`)
- [x] Session 277 TASK-501: release wasm built with `mise run build-wasm-release`; dist inspected as `profile: "release"`, `nameSection: true`, `dwarf: false`; bytes and sha256 recorded (`s285-release-wasm-build-final.log`, `s285-release-wasm-inspection.log`)
- [x] Session 277 TASK-502: `renderEvidence` shows the wasm line with summary/environment fallback and absent-data behavior; `stats.test.ts` rows pass (`s285-vitest-e2e.log`)
- [ ] Session 277 TASK-503: `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` exits 0 on the release wasm with every silent-sink assertion holding in Chromium and WebKit (including the headed WebKit fallback); TASK-402 sync has numeric percentiles within threshold
- [x] Session 277 TASK-504: F trigger evaluated and recorded; available Chromium trace shows no long-task overlap with late onsets, so no 15.3.8.14 amendment is triggered (`s285-profile-trace-chromium.log`)
- [x] Session 277 TASK-505: evidence document has the "Measurement build" paragraph, current triage table and superseded-debug note
- [x] Session 277 TASK-506 simulator portion: unsigned simulator build and silent self-check passed (`s285-ios-simulator-build-retry2.log`, `s285-ios-simulator-run.log`); final gates and TASK-507 remain downstream closeout work
- [ ] Session 285 TASK-508: `PhaseTimer`/`PerfPhase` in `frame.ts`, `__vactrPerf.phases()`, input/caret/shaping/syntax/upload/frame/tick spans, `phaseSummary` and `phaseMs` in `summary.json`. Everything is inert without `?perf=1`. The frame, mount, host-js and stats rows pass.
- [ ] Session 285 before table: the `s285b-before` p95 per phase for WebKit and Chromium is recorded in the progress log, with its log path.
- [ ] Session 285 TASK-509: one caret evaluation per transaction (arithmetic over `surface.state.doc.lines`/`lineAt`, never `layout.lineCount`; no rect; no shaping), including Enter and a multi-line paste at document end, with the caret visible in the production-order rig; the `widestShaped` watermark replaces the 1024-line clamp shaping; the rect cache gives 0 reads in keystroke handlers and at most 1 per text-dirty frame. `view-host.test.ts` (new) and the `mount.test.ts` counter rows pass, each with an in-test control branch. Invariant 4 and invariant 5 fixes are recorded with their phase evidence, or recorded as not needed.
- [ ] Session 285 TASK-510: `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` exits 0 on the release wasm. In both browsers, post-sink peak is 0 and the sink assertions hold. WebKit has at least 500 edit keys and input, animation, text, frame and sync are within threshold; Chromium `textWorkMs.p95` is at most 16.7. Thresholds and workload are unchanged.
- [ ] Session 285 TASK-511: F evaluated and recorded. If F triggers, the plan stops for design 15.3.8.14, and no Worker file exists.
- [ ] Session 285 TASK-512: the evidence document has the "Edit-path phase attribution (session 285)" before/after p95 table for both browsers, and updated triage rows.
- [ ] Session 285 TASK-513: final gates and the silent simulator run pass. Every `verification[]` record has `exitStatus: 0` and `outcome: "passed"`, and test records also have `testsRun > 0` and `failureCount: 0`, with a log path.
- [ ] Session 286: CANVAS-OPT-HARNESS, -HISTORY, -DOM, -TEXT, -BACKDROP and -RENDER accepted (runtime `acceptedPlanIds`)
- [ ] Session 286 TASK-601: release `run-001` exits 0 in WebKit and Chromium with textWork p95 at most 8 ms, input p95 at most 50 ms, frame p95 at most 20 ms, at least 500 edit keys, de-duplicated sync within threshold, post-sink peak 0
- [ ] Session 286 TASK-602: S and F evaluated and recorded (S implemented only on its trigger, with the scope record appended first)
- [ ] Session 286 TASK-603: evidence document "Performance wave (session 286)" baseline/final table with raw paths
- [ ] Session 286 TASK-604/605: final gates and the silent simulator pass; the six OPT plans archived along with the TASK-507 set; README updated; pushed non-force
- [x] Session 291 TASK-602: S not triggered. F evaluated and not triggered: 11 tick-start gaps exceeded 120 ms in each browser, but none overlapped a main-thread phase span over 50 ms. No Worker file or design amendment was created.
- [x] Session 291 TASK-603: evidence document has the session-286 baseline/final table, current failure triage, raw paths, silent-sink results, and pending physical-iPad procedures.
- [ ] Session 291 TASK-601: latest auditable canonical release `run-001` is incomplete (runner exit 1): sync p99 is 140.93 ms Chromium / 200.67 ms WebKit; Chromium has three post-stall active-set/receipt-timing discrepancies; behavior checks are 18/18 and silent-sink assertions pass. Beat drift passes in both browsers. The F condition is false, so continue product/harness attribution without a tick Worker.
- [ ] Session 291 TASK-604/605 and final integration: downstream final gates on the accepted source, review-dependent archive/index updates, commit and non-force push remain pending.
- [x] Session 302 plan amendment: design 15.3.8.15 records the 2026-10-07 Option A user approval; this plan has TASK-701 to TASK-706; the manifest `operatorRules` records the approval; the design, plan and manifest are checkpoint-committed before CANVAS-EVIDENCE is redispatched
- [x] Session 302 TASK-701: `publish.rs` pairs a running sample's `sample_time` with `Clock::to_host(cycle)` under the one-grid-period guard; the ported and new `session::tests::publish` rows pass, including the in-test control branch; `canvas-clock.test.ts` is ported with no assertion deleted and passes on the rebuilt host-wasm artifact; strict clippy and rustfmt on the two Rust files exit 0
- [x] Session 302 TASK-702: `measure.mjs` records `{index,startMs,endMs}` stall windows and no longer infers stalls from frame gaps; `classifyStallSamples` and `beatResidualMs` are exported; `evaluate` gates non-stall `syncAbsMs`, stall-window early flashes, window integrity, audited F residuals and a missing `beatDriftMs`; the `stats.test.ts` rows, including the latency-independence control, pass; `THRESHOLDS` and `TARGETS` are unchanged
- [ ] Session 302 TASK-703: `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` exits 0 on the release wasm; in both browsers non-stall sync p95 <= 33.4 ms and p99 <= 50 ms, stall-recovery criteria hold, beat drift <= 1 ms, the session-286 gates hold, behavior is 18/18, post-sink peak is 0 and direct destination connections are 0
- [ ] Session 302 TASK-704: the evidence document has the "Stall-window sync classification (session 293, design 15.3.8.15)" section with the per-window table, the side-by-side sync figures and the beat-drift diagnosis, and its triage table cites the new run
- [ ] Session 302 TASK-705: all eleven final gates pass serially, with full nextest run alone under `timeout 2400`
- [ ] Session 302 TASK-706: 23 canvas-cutover plans, 15 canvas-editor-224 plans and `canvas-editor-224-dispatch.json` are archived file by file; `impl-plans/README.md` is updated; the erratum and typo fixes are applied; the commit is pushed non-force to `origin wf/canvas`
- [ ] Session 285 TASK-507 additions: the scope-plan sha256 typo is fixed at :483 and :530; the framecost receipt `vite.config.ts` hash is corrected; `README.md` states the release page-build default and the debug vitest default; 30 files are archived (14 + 15 + 1) and `canvas-cutover-dispatch.json` stays in `active/`; `impl-plans/README.md` is updated; the erratum is appended; the commit is pushed non-force to `origin wf/canvas`.

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

### Session: 2026-10-05 (session 267 plan amendment)
**Tasks Completed**: Plan amended in place for operator decision S and design 15.3.8.8/15.3.8.9 (session-267 amendment). The remaining work is split into four parallel wave-3 plans (SILENT, VIEWPORT, EDITCOST, RUNSTART), and this plan moves to wave 4 (TASK-007 silent run, TASK-008 evidence document, TASK-009 review and closeout). Root causes recorded at plan time:

- Chromium 1x1 backing: `frame.ts` never delivers the first viewport when the size is unchanged since construction.
- WebKit readback: read outside the frame with `preserveDrawingBuffer: false`.
- WebKit touch: synthetic events lack `isPrimary`.
- Edit cost: whole-document `toString` in `input.ts`, `mount.ts`, `accessibility.ts` and `keyboard.ts`; `layout.ts` full rescans; the renderer cache key stringify.
- Run start: a main-thread stall during large-document eval, to be located by RUNSTART.

### Session: 2026-10-05 (session 269 plan amendment)
**Tasks Completed**: Applied operator authorizations 3, 5 and 6 and design 15.3.8.10. This plan now follows EDITCOST and RUNSTART acceptance. Added the pre-authorized product seam sharedPaths with an edit protocol (triage row, sha256 values, green owner tests, rerun), seam limits, the selective-redispatch rule for any other file, and re-measurement of the EDITCOST and RUNSTART targets. TASK-007 to TASK-009 and the closeout lists are unchanged.

### Session: 2026-10-05 (session 271 plan amendment)
**Tasks Completed**: Applied operator decision P and design 15.3.8.11. Changes:

- Dispatch wave 5, after RUNSTART (wave 3) and then EDITCOST (wave 4).
- `editor/package.json` moved to sharedPaths, limited to `scripts.e2e`.
- `npm run test:perf`, run alone, added to the final gates.
- The default full-vitest gate excludes perf files.

TASK-007 to TASK-009, the seams and the closeout lists are unchanged.

### Session: 2026-10-05 (session 271 implementation)
**Tasks Completed**: Regenerated silent `run-001` after the required ABI build, wrote both-browser raw data and harness-generated results, reran the iPad simulator launch, and added the required automated/native/simulator silence sections and remaining-failure triage table to the evidence document. No product source was changed. The accepted fanout dependencies CANVAS-EVIDENCE-EDITCOST and CANVAS-EVIDENCE-RUNSTART were admitted by the runtime `acceptedPlanIds` list.

**Silent run evidence**: `design-docs/specs/evidence/canvas-cutover/run-001/summary.json` reports for both browsers: sink installed before the first connection, zero direct destination connections, exact post-sink peak zero, workload pre-sink peak -13.0 dBFS, a positive control onset, positive large-document workload onsets, and `largeDocumentStartStatus: toolbar-click-returned`; the 64-voice peak was observed. Chromium's canvas backing passed the >1x1 guard (the later glyph-readback probe failed); its behavior DPR check passed at 2 and measurement-cycle samples exercised DPR 1 and 2. Raw data: `chromium-behavior.json`, `webkit-behavior.json`, `chromium-measure.jsonl`, `webkit-measure.jsonl`, `environment.json`, `summary.json`.

**Browser verification**: `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` exit 0 (`tmp/canvas-cutover/evidence/s271-canvas-evidence-abi-build.log`). `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` exit 1, not blocked (`tmp/canvas-cutover/evidence/s271-canvas-evidence-e2e-rebuilt.log`): 18 behavior checks, 15 passed and 3 failed, plus gated measurement failures. Remaining defects include the canvas glyph readback probe, WebKit touch selection, missed input/frame thresholds, unavailable Chromium sync samples, and WebKit late/replayed highlight failures; each is listed in the evidence triage table and remains repair-required. The earlier pre-build attempt is retained at `tmp/canvas-cutover/evidence/s271-canvas-evidence-e2e.log` and is not used as final-source evidence.

**Simulator**: `node editor/test/e2e/ios-sim.mjs --app editor/src-tauri/gen/apple/build/arm64-sim/Vactr.app --device "iPad Pro 11-inch (M5)" --out design-docs/specs/evidence/canvas-cutover/run-001/ios-sim.json` exit 1 (`tmp/canvas-cutover/evidence/s271-canvas-evidence-ios-sim.log`). Install, launch and screenshot commands returned 0, but the script matched its own `log show` command and left `selfCheck` null. The process-filtered diagnostic is at `tmp/canvas-cutover/evidence/s271-ios-self-check-process-filter-10m.log`; the app line omits `playingEvents`, so SHELL-owned work remains required.

**Local verification**: `cd editor && npm run check` exit 0 (`tmp/canvas-cutover/evidence/s271-canvas-evidence-npm-check.log`); `node --check editor/test/e2e/run.mjs editor/test/e2e/serve.mjs editor/test/e2e/behavior.mjs editor/test/e2e/measure.mjs editor/test/e2e/stats.mjs editor/test/e2e/ios-sim.mjs editor/test/e2e/fixtures/large-doc.mjs editor/test/e2e/silent-sink.mjs` exit 0 (`tmp/canvas-cutover/evidence/s271-canvas-evidence-node-check.log`); `cd editor && ./node_modules/.bin/vitest run test/e2e` exit 0, 23/23 across 4 files (`tmp/canvas-cutover/evidence/s271-canvas-evidence-vitest-e2e.log`). Native silence audit logs are the three paths listed in `impl-plans/active/canvas-cutover-evidence-silent.md` and repeated in the evidence document.

**Handoff**: Implementation remains incomplete until the harness/render probe is corrected, product failures are repaired by their accepted owners or selective redispatch, and the simulator self-check reports `playingEvents === 0`. Formal review, final integration review, closeout moves/index, final gates, commit and push remain downstream workflow work.

### Session: 2026-10-05 (session 274 plan amendment)
**Tasks Completed**: Applied operator decisions A and B and design 15.3.8.12.

- Manifest `impl-plans/active/canvas-cutover-dispatch.json`:
  - CANVAS-EVIDENCE-EDITCOST and -RUNSTART moved from `plans` to `acceptedDependencies`
    (commit 2156699). `plans` now lists only CANVAS-EVIDENCE.
  - CANVAS-EVIDENCE writePaths grew from 80 to 148 concrete files (no directories besides the
    unchanged artifact roots). They now include every `allowedWritePaths` entry of the five
    session-271 briefs: the harness `run`, `behavior`, `measure`, `stats` and `ios-sim` `.mjs`
    files, `stats.test.ts` and `README.md`; 23 product seams; and 38 existing seam tests under
    `editor/test/{canvas,code,app,bind,visual}`.
  - sharedPaths are reduced to `editor/package.json`, `editor/test/e2e/silent-sink.mjs`,
    `editor/test/e2e/fixtures/large-doc.mjs` and `editor/test/e2e/large-doc.test.ts`.
  - Also rewritten: `seamProtocol` (single owner; serial amendment instead of selective
    redispatch), the verification list (concrete `ios-sim` app path and the simulator rebuild),
    a new non-gating `diagnostics` list, the acceptance criteria, `resume.session274`,
    `executionModel.rules`, `reviewContext` and `designDocSections`.
  - jq cross-check: brief paths missing from writePaths 0, duplicate writePaths 0,
    writePath/sharedPath overlap 0, artifact roots not in writePaths 0, notes without a path 0,
    illegal path characters 0.
- This plan:
  - added the "Session 274 Amendment" with TASK-401 to TASK-409, the briefs carried verbatim,
    the pinned harness contracts (`attributeSync`, `lateActiveMismatches`, `rankSelfTime`,
    `classifyWithControl`, `--profile-trace`, the WebKit control page, the optional
    `PresentedRecord.epoch` field), pitfalls, test cases and the sandbox/outside split;
  - updated the Verification table and the Completion Criteria.

Pre-edit state: both files clean at HEAD 2156699 (`git status --porcelain` showed only the
step-2 design edit). The session-274 plan checkpoint commit precedes the redispatch.

**Correction after Step 5 review (finding S274-PLAN-SYNC-WINDOW, mid).** The TASK-402 contract
was revised:

- a new `syncWindow` defines the window as the intersection of both rings' coverage, with the
  eviction guard `first retained onset time + max(end - time)`;
- attribution candidates are all retained onsets, while sync samples come only from in-window
  onsets;
- frames outside the window are reported as `excludedFrames`, and an in-window frame with no
  candidate still counts as early;
- `lateActiveMismatches` evaluates only in-window stalls against all retained onsets;
- test row (d) was corrected, and rows (i), (ii) and (iii) were added.

No threshold, gate, workload, design-doc or manifest change was made.

**Precedence clarification after the second Step 5 review (finding S274-PLAN-SYNC-PRECEDENCE,
mid).**

- A "Session-274 review override (finding S274-PLAN-SYNC-WINDOW)" bullet was added under the
  TASK-402 brief. It makes the pinned contract take precedence over brief ordered changes 2
  and 5 and row 6(d).
- The Intent-and-context precedence sentence now treats the brief JSON as authoritative except
  for the explicit overrides: PERF in TASK-405 and the sync window in TASK-402.
- No other content changed.

### Session: 2026-10-05 (session 274 implementation)
**Tasks Completed**: Implemented the five assigned review repairs within the committed write paths. Dependency readiness was admitted from runtime `acceptedPlanIds` for all ten predecessors. No threshold, workload, Rust file, dependency, lockfile, public contract, or SHELL-owned file changed.

- **INT-S271-EV-SYNC-ATTRIBUTION**: Added occurrence/epoch-aware sync-window, frame-range, late-stall, profile-ranking and control-classification helpers and tests. Added the optional presented epoch to `editor/src/code/perf-hook.ts`, sourced from the active transport epoch in `editor/src/code/mount.ts`; the deterministic epoch row is in `editor/test/canvas/mount.test.ts`. `stats.test.ts` and the full Vitest suite pass. The corrected run still produces zero WebKit presented range pairs and no numeric sync values, so no clock defect is inferred from that failed attribution.
- **INT-S271-EV-READBACK / TOUCH**: `behavior.mjs` reads the line-1 glyph rectangle in the drawing frame. Both browsers pass text readback; WebKit touch passes with a tap at offset 5, long-press selection 2–8 and two handles. The event log is `tmp/canvas-cutover/evidence/s274-webkit-touch-events.json`.
- **INT-S271-EV-SIM**: `ios-sim.mjs` now filters Vactr process/thread lines, uses a local macOS `log show --start` timestamp, takes the newest line, and records derived silent/pass fields. Unsigned simulator build exit 0 (`s274-ios-build-05.log`); launch/self-check exit 0 (`s274-ios-sim-05.log`); `run-001/ios-sim.json` has `playingEvents: 0`, `silent: true`, `pass: true`, `appBinary.afterCommit: true`. Earlier failed build and parser attempts are preserved in `s274-ios-build-01..04.log` and `s274-ios-sim-01..04.log`.
- **INT-S271-EV-PERF**: Added opt-in Chromium CPU traces, self-time ranking, and a no-app WebKit control page; thresholds were not changed. Control metrics pass (input p95/p99 33/34 ms, frame interval 18/20 ms, 622 editing keys), so WebKit product latency/frame/key failures are not classified as headless limitations. Top CPU self time is `wasm://wasm/vactr.wasm` at 76.58% edit and 70.56% cycle. `isTexture` at `editor/src/code/renderer.ts:269` is 3.86% edit and 8.5% cycle; `src/types/scope.rs` lookup is only 2.36% cycle and does not identify the dominant module owner. The largest cost is outside the assigned Rust write paths; the precise Rust owner remains unresolved.
- **E2E result**: `npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` exits 1 (`s274-e2e-03.log`), not blocked. All 18 browser behavior checks pass; silent sinks pass in both browsers (direct connections 0, post peak 0; numeric workload peak dBFS -13.03 Chromium / -13.00 WebKit). Chromium large-document toolbar start times out; WebKit returns but fails editing/presentation thresholds and has zero presented-range pairs plus 7 post-stall mismatches. The document triage table records metrics, attribution and the profile ownership blocker.
- **Verification**: focused E2E/canvas tests 74/74 (`s274-focused-vitest-02.log`); default Vitest 703/703 across 90 files (`s274-vitest-full-02.log`); `npm run test:perf` 1/1 (`s274-test-perf-01.log`); `npm run check` exit 0 (`s274-npm-check-final.log`); final `node --check` exit 0 (`s274-node-check-final.log`); strict clippy exit 0 (`s274-cargo-clippy.log`); nextest 2810/2810, 3 skipped, exit 0 (`s274-nextest.log`); host-WASM build exit 0 (`s274-wasm-build.log`); Tauri cargo check exit 0 (`s274-tauri-cargo-check.log`); cargo build exit 0 (`s274-cargo-build.log`). A first full Vitest run had 3 timeouts/failures (700/703); isolated rerun passed 14/14 and the repeated full suite passed 703/703.
- **Source identity**: epoch seam baseline hashes from HEAD `20798dcb38c6413d30f6b280531c4f1e837d733c`: `editor/src/code/mount.ts` d63e7ec2, `editor/src/code/perf-hook.ts` 5ebc18d1, `editor/test/canvas/mount.test.ts` 44c1914b; post-edit hashes: mount.ts f6ca1cbc, perf-hook.ts 0008bdfb, mount.test.ts 0823cef2. Harness post-edit hashes: `run.mjs` 26525b88, `behavior.mjs` 3a03ac89, `measure.mjs` 5c4aed94, `stats.mjs` cc5d8be0, `stats.test.ts` 2f11ce71, and `ios-sim.mjs` 5cb8cc93. Full hashes are recorded by `sha256sum`; per-edit intent records are in `s274-edit-intents.jsonl`.

**Implementation blocker**: The E2E performance gates remain failing. The dominant module profile does not resolve to a specific Rust file, while this plan forbids Rust edits and its authorized writePaths contain no Rust source. Resume after a serial plan-author amendment identifies the exact profile-resolved source owner, adds its concrete write/test paths, and defines the owner regression test. Do not route the whole 70.56%–76.58% module share to `src/types/scope.rs` from its 2.36% function sample.

**Downstream pending**: Independent Opus integration review, review-dependent closeout/archive/index updates, clock-probe erratum, final closeout-commit gates, commit and non-force push remain owned by later workflow steps.

### Session: 2026-10-05 (session 277 plan amendment)
**Tasks Completed**: Applied operator decisions A-G and design 15.3.8.13.

- New serial plans:
  - `impl-plans/active/canvas-cutover-evidence-scope.md` (wave 5, decision A);
  - `impl-plans/active/canvas-cutover-evidence-sched.md` (wave 6, decisions B and C, with the
    engine-timebase tick from DR-S277-B-TIMEBASE);
  - `impl-plans/active/canvas-cutover-evidence-framecost.md` (wave 7, decisions E and D).
- This plan moves to wave 8 and gains:
  - the "Session 277 Amendment" (TASK-501 to TASK-507);
  - new Verification rows (the release build first, the SCOPE rustfmt files, and the simulator
    after the release build);
  - new Completion Criteria.
- Manifest: seven files (`renderer.ts`, `layout.ts`, `gpu.test.ts`, `mount.test.ts`,
  `diagnostics.test.ts`, `run.mjs`, `README.md`) move from this plan's writePaths to its
  sharedPaths, so exactly one plan writes each file at a time. The closeout paths of the three
  new plans are added to writePaths.

No source, threshold or workload change.

### Session: 2026-10-06 (session 283 amendment: no negative controls; closeout paths)
**Hard rule (workflowInput, session 283)**: run no mutation, negative-control or
intentionally failing harness self-test. Rule D's `mutationEvidence` list stays empty in this
run. Earlier logs may be cited in prose only. Structured `verification[]` and
`priorVerification[]` list only final-source commands that exited 0 with positive test
counts. A failed, blocked (exit 2) or timed-out e2e or gate run is fixed and rerun, and only
the final passing run is reported. Diagnostic `--profile-trace` runs (TASK-504) are not
gating and are reported in notes with their log paths, never in the structured verification
fields.

**Closeout scope (TASK-507, concrete paths only; unchanged from the manifest writePaths)**:
- Move each `impl-plans/active/canvas-editor-224-*.md` (15 files) plus
  `impl-plans/active/canvas-editor-224-dispatch.json` to the same name under
  `impl-plans/completed/`, each with a one-line superseded note.
- Move the 14 `impl-plans/active/canvas-cutover-*.md` plans to `impl-plans/completed/`, each
  with a completion note.
- `impl-plans/active/canvas-cutover-dispatch.json` stays in `active/` as the run manifest. It
  is not a plan and is not in the writePaths.
- Update `impl-plans/README.md`.

The evidence document, TASK-501 to TASK-507, thresholds and the workload are unchanged.

### Session: 2026-10-06 (session 285 implementation)
**Tasks Completed**: Built and inspected the gating release wasm, added generated wasm provenance to the evidence summary, collected the required browser/simulator reruns, assessed escalation F, and refreshed the evidence document and this plan's progress.

- `editor/dist/vactr.wasm` is release profile, retains its name section, has no DWARF, is 6,451,635 bytes, and hashes to `17c8630bb5d3f3acf90ea73b49ed475f1a497f3f1a363732e4136ccedf743a2d`. `stats.mjs` renders this from `summary.wasm`, falling back to `summary.environment.wasm`, and says `WASM: not recorded` when absent; three new `stats.test.ts` rows cover these cases.
- Fresh release-wasm `run-001` kept silent-sink checks green and all 18 browser behavior checks passed, with numeric A/V sync percentiles in both browsers. The aggregate command still exits 1: WebKit input/edit/frame thresholds fail; Chromium has one active-range mismatch whose onset revision and edit/mapping disposition are absent from raw evidence. Chromium edit/start/performance gates pass. Evidence and exact triage are in `design-docs/specs/design-canvas-editor-evidence.md` and `design-docs/specs/evidence/canvas-cutover/run-001/`.
- Chromium release trace does not show the long-task/late-onset overlap required to trigger escalation F; the trace facility only profiles Chromium. No Worker or threshold/workload change was made. WebKit's performance failure is not accepted as a platform limitation, and its dominant source is not localized by current evidence; investigate/propose an authorized seam before source repair.
- Unsigned iPad Pro 11-inch (M5) simulator build, install, launch, self-check and screenshot passed. The Vactr process line reports `playingEvents: 0`; `ios-sim.json` has `silent: true` and `pass: true`. Physical-iPad procedures remain pending.
- Current source checks: `npm run check`, focused e2e Vitest (35/35), scoped Vitest, full Vitest, serial `npm run test:perf` (1/1), Node syntax check, release build/inspection all pass. The required full browser gate remains failed as recorded; do not accept CANVAS-EVIDENCE or run closeout as green until repaired and rerun.
- Changed-file hashes, edit intents, full logs and generated-artifact provenance are in `tmp/canvas-cutover/evidence/`; shared SCHED/FRAMECOST changes were preserved. Formal integration review, archive/index updates, final closeout gates, commit and push remain downstream.

### Session: 2026-10-06 (session 285 continuation 1)
**Tasks Completed**: Added targeted Chromium highlight-map diagnostics, reran canonical release-wasm `run-001`, and refreshed triage from that run. TASK-503 remains incomplete.

- `HighlightScheduler.tick()` now increments its existing `unmapped` statistic when a previously accepted range no longer maps. `editor/test/code/highlight.test.ts` covers a range that maps on receipt and fails on a later tick. `measure.mjs` records the existing counter at periodic edit/cycle samples, without per-keystroke polling or a perf API shape change. Edit intents are in `s285-edit-intent-continuation1-highlight-map.json` and `s285-edit-intent-continuation1-rerun001.json`.
- The untraced canonical command `npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` exits 1; full log: `s285-continuation1-release-e2e-canonical.log`. All 18 browser behavior checks pass and both silent sinks pass (installed before first connect, zero direct destination connections, post-sink peak 0; workload pre-sink peak -13.00 dBFS). Chromium `lateActiveMismatchCount` is 0, so the prior active-range mismatch did not reproduce. Chromium fails text-dirty p95 (18.7 ms versus 16.7 ms). WebKit fails nine measurements: 162 editing keys; input p95/p99 360/377 ms; animation p99 165 ms; text p95 178 ms; frame p95/p99 185/197 ms; sync p95/p99 62/66 ms. Its same-run control passes at 642 keys, input 33/35 ms and frames 18/19 ms. Total: 10 measurement failures. Thresholds and workload are unchanged.
- The separate Chromium release diagnostic trace ranks `getError` at 3.18% of edit and 2.53% of cycle self time and does not establish long-task overlap with late onsets; escalation F remains untriggered. WebKit remains unprofiled by this harness and its dominant product source is not localized. A read-only investigation identified `editor/src/code/view-host.ts` as an unconfirmed candidate, but it is not an authorized path in this plan. Do not edit it speculatively. Resume after a serial plan amendment confirms the cost owner and adds its concrete source, regression-test and verification paths.
- Evidence triage now records the latest canonical Chromium, WebKit and active-range results plus the separate trace findings in `design-canvas-editor-evidence.md`. Edit intents and pre-edit hashes are in `s285-edit-intent-continuation1-canonical-results.json`, `s285-edit-intent-continuation1-canonical-rerun.json`, and `s285-edit-intent-continuation1-canonical-rerun-before.sha256`; the harness diagnostic intent is in `s285-edit-intent-continuation1-highlight-map.json`.
- Continuation checks on the final source: focused highlight/e2e Vitest 45/45 (`s285-continuation1-focused.log`); full Vitest 722/722 (`s285-continuation1-vitest-full-final.log`); serial `npm run test:perf` 1/1, ratio 2.74 (`s285-continuation1-test-perf.log`); `npm run check` exit 0 (`s285-continuation1-check.log`); Node syntax check exit 0 (`s285-continuation1-node.log`); `git diff --check HEAD` exit 0 (`s285-continuation1-diff-check-final.log`). The canonical browser gate remains failed as detailed above.

**Implementation blocker**: The required WebKit performance and A/V gates still fail against a passing same-run control, but no available WebKit trace localizes the product cost to an authorized seam. The suspected `editor/src/code/view-host.ts` path is outside this plan's writePaths and is not yet confirmed. Resume only after a serial plan-author amendment adds the confirmed owner, its regression test and the exact verification paths; then repair the product defect and rerun the canonical browser gate without changing thresholds or workload.

### Session: 2026-10-06 (session 285 implementation continuation 2)
**Tasks Completed**: Restored the seven pinned phase names, removed per-measure timing wrappers, added cached long-run width chunks and deterministic layout counters, and collected source-matched browser diagnostics. This continuation does not complete the assigned browser repair.

- `PerfPhase` and `phaseSummary` expose only input, caret, shaping, syntax, upload, frame and tick. Long-run position mapping reuses 256-character grapheme-safe measured chunks and caches queried prefixes. `edit-cost.test.ts` and `mount.test.ts` include live-counter controls, zero animation-only measurement/segmentation, and bounded edit measurement assertions. `gpu.test.ts` preserves long-document behavior.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/edit-cost.test.ts test/canvas/mount.test.ts test/canvas/gpu.test.ts test/canvas/view-host.test.ts test/canvas/frame.test.ts test/e2e/stats.test.ts` passed 86/86 (`s285b-task509-chunk-contract.log`); full Vitest passed 736/736 (`s285b-task509-full-vitest.log`); `npm run check` passed (`s285b-task509-chunk-check.log`). Earlier source iterations had one failed focused run and one GPU timeout; the chunked implementation passed the final focused suites without weakening assertions.
- Current-source WebKit diagnostic `s285c-after-1` (no `--write-evidence`; `s285b-task510-webkit-after-1.log`) still fails: 151 edit keys; input p95/p99 410/799 ms; text p95 187 ms; frame p95/p99 181/197 ms; A/V p95/p99 72/75.33 ms. Control passes at 637 keys, 33/34 ms input and 18/19 ms frame. Silent sink remains valid (installed before connect, direct destination connections 0, post peak 0; workload pre peak -13.00 dBFS). Phase p95 shows shaping 205 ms and upload p99 184 ms; `layoutStats` reports 119 builds, 1,483 measured text calls, 121,022 measured chars, maximum call length 256, and 0 segmentations. Text shaping is still a dominant measured cost; the required repair is unresolved.
- Current-source Chromium diagnostic `s285c-after-2` (no `--write-evidence`; `s285b-task510-chromium-after-2.log`) measured 556 edit keys, input p95/p99 30.6/33.4 ms, text p95 16.7 ms, frame p95/p99 17.5/17.7 ms, and sync p95 22.55 ms. It reports `pass:false` for beat drift just over 1 ms. The log is complete through the harness JSON summary; the invoking zsh wrapper's final status assignment used reserved variable `status`, so its exit 1 is a wrapper error and is not represented as a clean diagnostic exit record.
- No diagnostic overwrote the canonical marker section or raw `run-001` files. The marker section still contains the prior noncanonical diagnostic and remains invalid until the exact canonical `run-001` command passes and regenerates it. TASK-511 escalation F was not implemented: the required post-repair sync evaluation has not occurred and these traces do not establish a worker-owned tick defect. TASK-512 table, TASK-513 final gates, and TASK-507 closeout remain pending.
- Latest per-edit intent record: `tmp/canvas-cutover/evidence/s285b-task510-progress-round-intent.json` (41d698c9…); plan pre-edit SHA-256: `b5464105bfec3dc5e618a191a69ed732b1dc6a46b642868bab4ea472d9f6234c`.

**Implementation status**: Incomplete. `INT-S285-EV-EDITPATH-REGRESSION` remains unresolved because WebKit edit, text, frame, input and sync measurements exceed the unchanged thresholds. `INT-S285-EV-DOC-OVERWRITE` also remains unresolved because the canonical marker cannot be restored without a passing canonical run. The serial continuation must investigate and repair the measured WebKit shaping/upload cost, rerun both browser diagnostics to pass, then run canonical evidence and assigned remaining tasks. Do not accept the plan or begin closeout on these diagnostics.

### Session: 2026-10-06 (session 285 continuation 3)
**Tasks Completed**: Cached grapheme segmentation before long-run shaping and extended cached prefix-width indexing to short runs shared by caret and tile-position queries. The focused counter and boundary tests pass. No thresholds, workload, or silent-sink behavior changed.

- `editor/src/code/layout.ts` now reuses the line's cached grapheme clusters for measurement chunks, avoids repeat segmentation during cached shaping, and indexes prefix widths for short as well as long runs. `editor/test/canvas/edit-cost.test.ts` proves one segmentation for shaping, no repeat on cached lookups, a live invalidation control, and reuse of short-run prefixes. Edit intents: `s285c-task509-layout-preedit-intent.json`, `s285c-task509-test-preedit-intent.json`, `s285c-task509-surrogate-helper-intent.json`, and `s285c-task509-short-run-index-intent.json`.
- Final-source focused Vitest passed 114/114 across six files (`tmp/canvas-cutover/evidence/s285c-task509-short-index-focused.log`); full Vitest passed 737/737 (`s285c-task509-full-vitest.log`); `npm run check` exited 0 (`s285c-task509-short-index-check.log`); serial `npm run test:perf` passed 1/1 (`s285c-task509-short-index-test-perf.log`), with 5000-line median 803.5 ms and 20000-line median 2254.4 ms (ratio 2.81).
- WebKit diagnostic `s285c-after-4` used release wasm and no `--write-evidence` (`s285c-after-4.log`): 221 edit keys, input p95/p99 237/499 ms, text p95 112 ms, frame p95/p99 129/144 ms, sync p95/p99 34.33/57.33 ms, one post-stall mismatch, beat drift 1.75 ms. Control passes at 645 keys, 33/34 ms input and 18/19 ms frame. Phase p95: shaping 111 ms, upload 3 ms (upload p99 112 ms), syntax 13 ms; sink checks pass with pre-sink workload peak -13.00 dBFS and post-sink peak 0. `layoutStats`: 137 builds, 2465 measured calls, 174445 chars, maximum call length 256, zero segmentations.
- Chromium diagnostic `s285c-after-5` used release wasm and no `--write-evidence` (`s285c-after-5.log`): 557 edit keys, input p95/p99 31.1/32.9 ms, text p95 16.9 ms, frame p95/p99 17.5/17.7 ms, sync p95/p99 83.44/83.44 ms, beat drift 1.0 ms. It fails text, sync and beat gates. `phaseMs` p95 is input 5.9, caret 0.1, shaping 0.1, syntax 14.8, upload 1.5, frame 0.2 and tick 0.3 ms. Sink checks pass with pre-sink workload peak -13.00 dBFS and post-sink peak 0. Both diagnostics were non-gating and did not alter the marker section or canonical raw `run-001` files.
- TASK-511 F was evaluated and not triggered: sync still misses a threshold, but the captured phase summaries show low tick p95 (WebKit 1 ms; Chromium 0.3 ms) and do not provide the required >120 ms tick gaps overlapping >50 ms main-thread spans. The evidence does not establish main-thread starvation of `session_tick`; no Worker or design 15.3.8.14 was added.
- TASK-510 remains incomplete because both browsers fail diagnostics. TASK-512 cannot use canonical after-phase results until those gates pass; TASK-513 and TASK-507 remain downstream. The canonical marker remains unresolved, so `INT-S285-EV-DOC-OVERWRITE` is unresolved. `INT-S285-EV-EDITPATH-REGRESSION` remains high and unresolved. Progress intent: `s285c-task509-progress-round-intent.json` (pre-edit plan SHA-256 `1180dd6913c801486e3bfa73fdb8e67a14938e91e7c7b4f37b1da09aa3131469`).

### Session: 2026-10-06 (session 285 continuation 4)
**Tasks Completed**: Seeded run-width prefix indexes from widths already measured during shaping, added a bounded full-run raster path for styled multi-tile glyph runs, and retained tile-local style clipping for oversized runs. The full GPU fallback test exposed and verified the bounded fallback after correction.

- `editor/src/code/layout.ts` passes shaped chunk measurements into the cached run-width index, avoiding a second measure pass to initialize prefixes. `editor/test/canvas/edit-cost.test.ts` asserts the index initialization does not add measurement calls. Intent: `s285c-task509-seed-run-widths-intent.json`; test intent: `s285c-task509-seed-width-test-intent.json`.
- `editor/src/code/atlas.ts` rasterizes eligible styled multi-tile runs once into a geometry-accounted staging canvas (capped at 16,384 backing pixels wide and the geometry budget) and crops tiles from it. `editor/src/code/renderer.ts` uses whole-run styles only when that path is eligible; oversized runs retain tile-local style intersections. `editor/test/canvas/gpu.test.ts` covers raster reuse, crop uploads and release. Intents: `s285c-task509-atlas-raster-intent.json`, `s285c-task509-atlas-budget-intent.json`, `s285c-task509-atlas-test-intent.json`, `s285c-task509-fallback-raster-intent.json`.
- Current-source verification passed: GPU Vitest 45/45 (`s285c-task509-fallback-gpu.log`); assigned focused Vitest 115/115 (`s285c-task509-fallback-focused.log`); full Vitest 738/738 (`s285c-task509-fallback-full-vitest.log`); `npm run check` exit 0 (`s285c-task509-fallback-check.log`); serial `npm run test:perf` 1/1, ratio 2.78 (`s285c-task509-fallback-test-perf.log`); `git diff --check` exit 0 (`s285c-task509-fallback-diff-check.log`).
- Current-source WebKit diagnostic `s285c-after-6` used release wasm and no `--write-evidence` (`s285c-after-6.log`): 224 edit keys; input p95/p99 239/616 ms; text p95 112 ms; frame p95/p99 127/143 ms; sync p95/p99 44/489.67 ms; two post-stall mismatches; beat drift 0.67 ms. Phase p95: shaping 110 ms, syntax 13 ms, upload 3 ms (upload p99 112 ms). Large-document start 5,972 ms. Sink passed: installed before connects, direct connections 0, post-sink peak 0, workload pre-sink peak -13.00 dBFS.
- Current-source Chromium diagnostic `s285c-after-7` used release wasm and no `--write-evidence` (`s285c-after-7.log`): 557 edit keys; input p95/p99 29.1/31.3 ms; text p95 16.9 ms; frame p95/p99 17.7/18.6 ms; sync p95/p99 68.78/68.78 ms; no active-set mismatch; beat drift 1.42 ms. Phase p95: syntax 14.5 ms, upload 1.7 ms, tick 0.3 ms. Large-document start 5,996 ms. Sink passed: installed before connects, direct connections 0, post-sink peak 0, workload pre-sink peak -13.00 dBFS.
- TASK-511 F remains not triggered: the summaries show tick p95 1 ms in WebKit and 0.3 ms in Chromium, without the required span-overlap proof of tick starvation. No Worker or 15.3.8.14 design amendment was added. Both browser diagnostics still fail unchanged gates. Canonical `run-001` was not run, its marker/raw files were not edited, TASK-512 after-phase table, TASK-513, and TASK-507 remain pending. `INT-S285-EV-EDITPATH-REGRESSION` and `INT-S285-EV-DOC-OVERWRITE` remain unresolved. Progress intent: `s285c-task509-progress-continuation4-intent.json` (pre-edit plan SHA-256 `b27dfe88bc9bdb9f78462c42d887169a2a4052d4ee0a78d52a2f5810edc7f821`).

### Session: 2026-10-06 (session 285 plan amendment, resume of 284)
**Tasks Completed**: Applied operator decisions 1-6 and the design 15.3.8.13 "Session 285
scope record".

- This plan:
  - added the "Session 285 Amendment" with TASK-508 to TASK-513 and the TASK-507 additions;
  - added the session-285 sandbox verification rows and the non-gating attribution rows;
  - added Session 285 Completion Criteria;
  - adopted the `s285b-` log prefix, so the earlier `s285-*` logs are never overwritten.
- Manifest `impl-plans/active/canvas-cutover-dispatch.json`:
  - CANVAS-EVIDENCE-SCOPE, -SCHED and -FRAMECOST move from `plans` to
    `acceptedDependencies` (commit 2e3361d). `plans` lists only CANVAS-EVIDENCE.
  - CANVAS-EVIDENCE writePaths gain the decision-1 product and test files that were missing,
    and these move back from sharedPaths: `renderer.ts`, `layout.ts`, `gpu.test.ts`,
    `mount.test.ts` and `diagnostics.test.ts`.
  - writePaths also gain the closeout paths `README.md` and
    `tmp/canvas-cutover/framecost/receipt.json` (also an artifactRoot), and the reserved F
    paths `editor/worklet/tick-worker.js` and `editor/test/protocol/tick-worker.test.ts`.
  - Also updated: the seamProtocol, verification, diagnostics, acceptance criteria,
    `resume.session285`, `executionModel.rules`, `reviewContext` and
    `designDocSections`.
- Closeout decision: `canvas-cutover-dispatch.json` stays in `impl-plans/active/`, as the
  session-283 closeout scope decided and as operator decision 6 lists. The intake signal that
  also moves it is not adopted, because the runtime reads the manifest during this run.

No source, threshold or workload change.

### Session: 2026-10-06 (session 286 plan amendment, performance wave)

**Tasks Completed**: Plan amendment only. Added the section "Session 286 Amendment",
TASK-601 to TASK-605.

**Notes**:

- Six serial OPT plans precede this plan: HARNESS, HISTORY, DOM, TEXT, BACKDROP, RENDER.
- The session-286 product and test paths were added to writePaths for re-measure repairs,
  along with the reserved S paths and the OPT plan archive paths.
- The F references are renumbered to the next free 15.3.8.x number.
- The textWork gate is now 8 ms (HARNESS). No other threshold or workload change.

### Session: 2026-10-07 (session 291 implementation)

**Tasks Completed**: Accepted CANVAS-OPT-RENDER-C dependency readiness from the runtime-owned
`acceptedPlanIds`; preserved the shared A/B/C implementation changes. Fixed evidence sampling
to snapshot the bounded frame/key/presented/onset rings every 15 seconds during the 120-second
cycle. Indexed sync onset lookup to avoid repeated whole-ring scans. Added exact sync sample
inputs and post-stall audits (selected frame, expected/actual active ranges, overlapping and
receipt-eligible onsets) so the reported metrics are reproducible from the captured evidence.
The independent review found that downsampled raw rows had not supported reproduction; the
latest JSONL now carries `sync-sample` and `stall-audit` rows while phase metrics remain based
on the complete in-memory trace. Added the session-286 evidence comparison and current triage
to `design-canvas-editor-evidence.md`.

**Final-source verification**:

- `cd editor && npm run check`: exit 0 (`tmp/canvas-cutover/evidence/s291-receipt-audit-check.log`).
- `cd editor && ./node_modules/.bin/vitest run`: 798/798, exit 0 (`tmp/canvas-cutover/evidence/s291-receipt-audit-vitest-full.log`).
- `cd editor && ./node_modules/.bin/vitest run test/e2e/stats.test.ts`: 35/35, exit 0 (`s291-receipt-audit-focused.log`).
- `cd editor && npm run test:style`: 4/4, exit 0 (`tmp/canvas-cutover/evidence/s291-receipt-audit-style.log`).
- `cd editor && npm run test:perf` (alone): 1/1, exit 0 (`tmp/canvas-cutover/evidence/s291-receipt-audit-perf.log`; 5k/20k median ratio 2.83).
- `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001`: runner exit 1, not blocked (`s291-run-001-auditable-final2.log`). Both behavior suites pass 18/18; release wasm is verified (`nameSection: true`, `dwarf: false`). Both silent sinks were installed before first connect with zero direct destination connections, zero post-sink peak and numeric -13.00 dBFS workload peaks. Input/frame/text p95 and beat drift pass. Sync p99 is 140.93 ms Chromium / 200.67 ms WebKit; no-dropped-frame p99 is 22.17 / 200.67 ms, but all samples remain in the gate. Chromium has three post-stall discrepancies; detailed audit rows show 64 active ranges against zero receipt-eligible onsets in each selected frame. These remain unresolved, not acceptable limitations.
- The canonical raw files are under `design-docs/specs/evidence/canvas-cutover/run-001/`; the measure JSONL files are each below the 8 MB source snapshot cap. Full phase distributions and exact TASK-511 overlap counts are calculated before serialization; the JSONL retains phase spans >=50 ms and omits only shorter supplemental phase rows.
- The indexed sync aggregation keeps canonical analysis bounded. The periodic-capture attempt `s291-run-001-periodic.log` was stopped after 24 minutes in quadratic aggregation; earlier source-matched failed runs remain in this evidence directory.

The implementation records pre-edit intent and fresh file hashes in `s291-intent-*.json`
under `tmp/canvas-cutover/evidence/`. Earlier checks exposed and then resolved the missing
test-module type and raw evidence retention issues; their prior logs remain in the same
evidence directory. Measure JSONL files are under the 8 MB source snapshot cap. No source file
reached 1,000 lines. No Rust source changed.

**TASK-602 disposition**: S is not triggered: text-work, input and frame p95 pass, and syntax is not attributed as the dominant late-frame cause. F is not triggered: although each browser has 11 tick gaps over the 120 ms lookahead, exact phase attribution found zero overlaps with spans over 50 ms (`run-001/summary.json`). No Worker file or design amendment was created.

**Remaining work**: TASK-601 acceptance remains incomplete until sync p99 and Chromium's post-stall active-set/receipt-timing discrepancy are resolved and a canonical run passes. Beat drift passes both browsers. Current phase attribution does not authorize a tick Worker; investigate the measured failures within this plan's write paths and use selective redispatch or a serial plan amendment if ownership requires it. Formal review, closeout archival/index updates, closeout gates, commit and push remain downstream workflow work.

### Session: 2026-10-07 (session 291 source-matched instrumentation re-measure)

**Tasks Completed**: Added callback execution timestamps to perf-hook presented rows and used
them only for post-stall receipt eligibility; kept RAF timestamps for synchronization math.
Dropped-frame sample counts now include the gap before the first active presented frame as well
as the following interval. Added regressions for both cases. Rebuilt the release editor bundle
before the canonical run; the earlier same-node run that used the pre-existing `editor/dist`
bundle is retained as diagnostic history and is not final-source evidence.

**Final-source verification**:

- `cd editor && ./node_modules/.bin/vitest run test/e2e/stats.test.ts`: 37/37, exit 0
  (`tmp/canvas-cutover/evidence/s291-final-instrumentation-focused.log`).
- `cd editor && npm run check`: exit 0
  (`tmp/canvas-cutover/evidence/s291-final-instrumentation-check.log`).
- `cd editor && ./node_modules/.bin/vitest run`: 800/800, exit 0
  (`tmp/canvas-cutover/evidence/s291-final-instrumentation-vitest.log`).
- `cd editor && npm run test:style`: 4/4, exit 0
  (`tmp/canvas-cutover/evidence/s291-final-instrumentation-style.log`).
- `cd editor && npm run test:perf` (alone): 1/1, exit 0, ratio 2.66
  (`tmp/canvas-cutover/evidence/s291-final-instrumentation-perf.log`).
- `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`: exit 0
  (`tmp/canvas-cutover/evidence/s291-final-instrumentation-build.log`); emitted JS contains
  the callback timestamp instrumentation.
- `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001`:
  runner exit 1, not blocked (`tmp/canvas-cutover/evidence/s291-final-bundle-e2e.log`). Both
  browser behavior suites pass 18/18. Release wasm is verified with name section and no DWARF;
  each silent sink was installed before first connect, direct destination connections are 0,
  post-sink peak is 0 and workload peak is -13.00 dBFS. Input, frame and text p95 pass in both.
  Sync p99 is 157.422 ms Chromium / 155.667 ms WebKit against the 50 ms gate; WebKit beat drift
  is 1.50 ms against the 1 ms gate. Post-stall active-set mismatches are 0 in both browsers.
  These measurement failures remain unresolved; no threshold or workload changed.
- `git diff --check`: exit 0 (`tmp/canvas-cutover/evidence/s291-final-diff-check.log`).

The source-matched raw records are in `design-docs/specs/evidence/canvas-cutover/run-001/`;
the evidence document's comparison table and failure triage use this run. Each browser recorded
11 tick-start gaps above 120 ms, but zero overlaps with main-thread spans over 50 ms. TASK-602
therefore records F as not triggered. TASK-601 remains unchecked. No Rust source changed and no
source file reached 1,000 lines.

**Remaining work**: Resolve the sync p99 and WebKit beat-drift gates without relabeling failures
as limitations, then produce a passing canonical run. Formal review, closeout archival/index
updates, final closeout gates, commit and non-force push remain downstream workflow work.

**Superseded by the session 293 plan amendment below.**

**Current sync attribution**: the WebKit `syncAbsMsNoDrop.p99` remains 125.333 ms. The two
no-drop samples above 50 ms are at audio times 190 s and 243 s (`webkit-measure.jsonl:25087`
and `:25140`); both onset receipts and first active frames occur immediately after the explicit
250 ms main-thread stalls (`measure.mjs:56`). These are real late visual updates, not a formula
artifact. Their adjacent presented-frame intervals are only 6–11 ms, so the local dropped-frame
counter labels them as no-drop even though the preceding stall audit is present (`:25166` and
`:25171`). Sync remains gated across every sample by design section 15.3.8.14; do not filter
these samples or change thresholds/workload. The synthetic stall is not represented among the
named input/frame/upload phase spans, so the strict F overlap criterion remains unmet in the
current evidence. Keep TASK-601 incomplete and route any changed escalation/design disposition
through the serial plan-author step before implementing a Worker.

### Session: 2026-10-07 (session 293 plan amendment, Session 302 Amendment)

**Tasks Completed**: Plan amendment only. Added the section "Session 302 Amendment",
TASK-701 to TASK-706.

**Notes**:

- **User approval and design.** The user approved Option A on 2026-10-07. Design 15.3.8.15
  (accepted by step 3, comm-004674) defines two parts. Part A classifies sync samples by stall
  windows that `measure.mjs` records, never by latency value. Part B fixes the
  `TransportSample` pairing defect in `src/session/publish.rs`, which caused the WebKit 1.50 ms
  beat drift.
- **Ownership.** New writePaths: `src/session/publish.rs`, `src/session/tests/publish.rs` and
  `editor/test/wasm/canvas-clock.test.ts`. The wasm test asserts the old exact host-time
  pairing at `:104`, `:124` and `:174`, which part B changes. `editor/test/e2e/README.md`
  moves from sharedPaths to writePaths.
- **Superseded tasks.** TASK-601 and TASK-604 are superseded by TASK-703 and TASK-705.
- **Unchanged.** No threshold, workload, stall, dependency, protocol field or frontend clock
  formula changes.
- **Rules.** Never run two heavy suites at once. No mutation or negative-control commands.

### Session: 2026-10-07 (session 293 Step 6 implementation)

**Tasks Completed**: TASK-701 transport-sample pairing; TASK-702 explicit stall-window
classification and recovery evidence. A late-active-range audit fixture now supplies the
full onset set and distinguishes a genuine expired active range from a range reused by a
current onset. Thresholds and targets remain unchanged.

**Final-source verification**:

- `rustfmt --edition 2021 --check src/session/publish.rs src/session/tests/publish.rs`: exit 0
  (`tmp/canvas-cutover/evidence/s302-task701/rustfmt-check.log`).
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests::publish/)'`: 21/21, exit 0
  (`tmp/canvas-cutover/evidence/s302-task701/nextest-publish.log`).
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`: exit 0
  (`tmp/canvas-cutover/evidence/s302-task701/clippy.log`).
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`: exit 0
  (`tmp/canvas-cutover/evidence/s302-task701/host-wasm-build.log`).
- `cd editor && ./node_modules/.bin/vitest run test/wasm/canvas-clock.test.ts`: 7/7, exit 0
  (`tmp/canvas-cutover/evidence/s302-task701/vitest-canvas-clock.log`).
- `cd editor && ./node_modules/.bin/vitest run test/e2e`: 54/54, exit 0
  (`tmp/canvas-cutover/evidence/s302-vitest-e2e-replay-final.log`).
- `cd editor && npm run check`: exit 0 (`tmp/canvas-cutover/evidence/s302-npm-check-final.log`).
- `node --check editor/test/e2e/run.mjs editor/test/e2e/serve.mjs editor/test/e2e/behavior.mjs editor/test/e2e/measure.mjs editor/test/e2e/stats.mjs editor/test/e2e/ios-sim.mjs editor/test/e2e/silent-sink.mjs editor/test/e2e/fixtures/large-doc.mjs`: exit 0
  (`tmp/canvas-cutover/evidence/s302-node-check-final.log`).

**TASK-703 canonical re-measure**: release wasm identity was verified as profile `release`,
`nameSection=true`, `dwarf=false` (SHA-256
`fda9d3bac38b8f47b45d00d2dd890b844398e7899b10ee024aa48a213aaf870f`). Both behavior runs
pass 18/18. The run keeps the silent-sink assertions and stall recovery active, with zero
post-sink peak and zero direct destination connections. Chromium passes non-stall sync and
beat drift. WebKit beat drift is 0 ms and non-stall sync p95/p99 is 21.33/41.33 ms, within
the 33.4/50 ms limits. However, the latest run reports one WebKit stall-window early flash
(-15.33 ms) and exits as failed (`tmp/canvas-cutover/evidence/s302-run-001-e2e-replay-final.log`;
raw records in `design-docs/specs/evidence/canvas-cutover/run-001/webkit-measure.jsonl`).
The automated suite reports 18/18 browser behavior checks; the overall evidence gate fails.

**Disposition**: TASK-703 remains incomplete. Do not relabel the early-flash failure or loosen
the accepted metric. Its sample’s rAF record has `audibleTime=243.0122`, after onset time 243,
while the current proxy-frame calculation compares `next.frameMs` against the correlated
onset-page timestamp and reports -15.33 ms. This exposes a need to confirm the intended
presentation timestamp seam before repair. The approved 15.3.8.15 amendment explicitly keeps
the frontend clock formula unchanged, and the plan’s non-goals prohibit product edits; a
serial plan-author amendment must resolve this conflict and name any changed owner paths and
deterministic proof before further implementation. TASK-704 evidence prose, final gates,
closeout, formal review, commit and push remain pending downstream or blocked by TASK-703.
No source file reached 1,000 lines. No thresholds or workload changed.
