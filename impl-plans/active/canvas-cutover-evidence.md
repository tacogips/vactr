# Canvas Cutover: Real-Browser Evidence, Measurements and Closeout Implementation Plan

**Status**: In Progress (session 274: single-plan redispatch with the five session-271 integration-review briefs; see "Session 274 Amendment")
**Plan ID**: CANVAS-EVIDENCE (dispatch wave 5; all ten other canvas-cutover plans, including CANVAS-EVIDENCE-SILENT, -VIEWPORT, -EDITCOST and -RUNSTART, are accepted dependencies)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.8 (measurement protocol and thresholds, including the session-267 silent automated audio rule), 15.3.8.9 (gates and closeout), 15.3.8.11 (serial perf gate), 15.3.8.12 (session-274 evidence repair wave)
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
| `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 |
| `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` | exit 0 is required for acceptance (session 267). Exit 1 is written to the evidence file and triaged under TASK-008. A product defect blocks acceptance until it is repaired; only a recorded environment limitation (for example software GL frame metrics) may remain as a documented failure, and that is a review decision. Exit 2 is blocked, never a pass. The silent-sink assertions of TASK-007 must hold in every case. |
| `tmp/canvas/tools/cargo-tauri ios build --target aarch64-sim` (debug, unsigned; cwd and flags as in `canvas-cutover-shell.md`), then `git status --porcelain editor/src-tauri` | exit 0; the `Vactr.app/Vactr` mtime is later than the build start; no tracked change is kept |
| `node editor/test/e2e/ios-sim.mjs --app editor/src-tauri/gen/apple/build/arm64-sim/Vactr.app --device "iPad Pro 11-inch (M5)" > tmp/canvas-cutover/evidence/s271b-ios-sim.log 2>&1` | exit 0; `ios-sim.json` has a non-null `selfCheck` from a Vactr-process line, `playingEvents === 0`, `silent === true`, `pass === true` and `appBinary.mtimeIso` |
| Diagnostic, not gating: `cd editor && npm run e2e -- --browser chromium --profile measure --profile-trace --run-id s274-trace --out ../tmp/canvas-cutover/evidence/s274-trace` | writes `tmp/canvas-cutover/evidence/s271b-trace-chromium-{edit,cycle}.json`; the top-10 `rankSelfTime` output is recorded in the progress log |

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
| `rustfmt --check` on touched Rust files | exit 0 |

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
