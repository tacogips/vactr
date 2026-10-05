# Canvas Cutover: Real-Browser Evidence, Measurements and Closeout Implementation Plan

**Status**: In Progress (session 271: wave 5; browser evidence captured, required defects remain open)
**Plan ID**: CANVAS-EVIDENCE (wave 4 since session 267; depends on CANVAS-CLOCK, NATIVE, RENDER, MOUNT, VISUAL, SHELL and the wave-3 plans CANVAS-EVIDENCE-SILENT, -VIEWPORT, -EDITCOST, -RUNSTART)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.8 (measurement protocol and thresholds, including the session-267 silent automated audio rule), 15.3.8.9 (gates, owner-file defect fixes and closeout)
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
| `cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001` | exit 0 is required for acceptance (session 267). Exit 1 is written to the evidence file and triaged under TASK-008. A product defect blocks acceptance until it is repaired; only a recorded environment limitation (for example software GL frame metrics) may remain as a documented failure, and that is a review decision. Exit 2 is blocked, never a pass. The silent-sink assertions of TASK-007 must hold in every case. |
| `node editor/test/e2e/ios-sim.mjs --app <SHELL .app> --device "<iPad simulator>"` | `ios-sim.json` with the self-check line and `playingEvents === 0` |

Final gates on the closeout commit (design 15.3.8.9):

| Command | Required evidence |
|---------|-------------------|
| `cargo build` | exit 0 |
| `cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Full nextest (timeout >= 1500 s) | all pass |
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
- [x] Active-workload attribution is control-backed: the built-in pad control emits an onset in both browsers; the same toolbar click times out on the 20,000-line document, which records zero workload onset and a product/full-document startup failure. Editing keystrokes and paired samples are reported separately, with the 500-key gate applied to editing keys. (Session 267: the timeout is now a product defect owned by CANVAS-EVIDENCE-RUNSTART.)
- [ ] Simulator evidence recorded: iPad Pro 11-inch (M5) launched, but the generated report did not parse the app self-check; the process-filtered line lacks the required `playingEvents` field. Route the contract/evidence repair to CANVAS-SHELL.
- [x] Session 267: wave-3 plans SILENT, VIEWPORT, EDITCOST and RUNSTART accepted (the runtime fanout item lists all four in `acceptedPlanIds`)
- [ ] Session 269: every seam edit (if any) has a triage row, before/after sha256 values and green owner tests; other defects were routed by selective redispatch
- [ ] Session 267: silent run-001 regenerated; browser TASK-007 sink assertions hold, but simulator `playingEvents === 0` is not yet evidenced
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
