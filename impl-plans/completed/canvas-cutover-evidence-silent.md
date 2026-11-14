# Canvas Cutover: Silent Evidence Harness Implementation Plan

**Status**: Completed (2026-10-07)
**Plan ID**: CANVAS-EVIDENCE-SILENT (wave 3, session 267; parallel with CANVAS-EVIDENCE-VIEWPORT, -EDITCOST, -RUNSTART)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.8 (Workload, Head-only control run, Silent automated audio, DPR bullet, profile H keystroke floor)
**Manifest**: impl-plans/completed/canvas-cutover-dispatch.json (entry `CANVAS-EVIDENCE-SILENT`)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

The operator stopped session 266 because the evidence harness played a 64-voice, 2-note raw saw
stack at `amp 0.5` through the Mac speakers. That stack was hard-clipping. Operator decision S
(highest priority) says that no automated run may ever be audible. This plan makes the existing
Playwright-library harness silent, and proves the silence in numbers. It also closes the three
open test-integrity findings (EVID-TI-001, -006, -007) and fixes three harness defects that made
behavior checks fail for harness reasons:

- WebKit canvas readback happens outside the rendered frame (`preserveDrawingBuffer: false`).
- WebKit synthetic touch omits `isPrimary`.
- Keystroke pacing is too slow to reach the 500-key floor.

The harness files are plain Node ESM in `editor/test/e2e/`. They are not type-checked (tsconfig
includes only `.ts`). Their pure helpers are unit-tested with vitest through the non-literal
dynamic-import pattern of `editor/test/e2e/large-doc.test.ts:7-10`. The Codex sandbox cannot bind
localhost, so the implementer cannot run browsers. The outside-sandbox verification step runs the
browser harness (CANVAS-EVIDENCE, wave 4).

Already verified facts (do not re-derive):

- The app connects to the speakers only through `AudioNode.prototype.connect(ctx.destination)`:
  `editor/worklet/host.js:272` and `:274` (`stereo.connect(ctx.destination)` and
  `node.connect(ctx.destination)`).
- `editor/test/e2e/fixtures/large-doc.mjs:12` defines
  `inst pad freq: float = 440 amp: float = 0.5:` over `saw freq` and `> * amp`. The 64 voices are
  `{s :pad > note [60 64]}` in `stack [...] > d1`. `controlText` (line 42) is a single voice. At the
  new amplitude a single voice is about -46 dBFS, which never crosses the -40 dBFS onset threshold.
  The control must therefore be the head stack.
- `editor/src/app/main.ts:142-167` `reportSelfCheck` never evaluates or plays. It reports `tier`,
  `webgl2`, `renderer`, `gpuStatus`, `effectiveDpr` and `latencyKind`.
- `editor/test/e2e/ios-sim.mjs:16` launches with `SIMCTL_CHILD_VACTR_SELF_CHECK=1`. Line 20 passes
  only if the self-check line is present.
- `editor/src/code/pointer.ts:58` ignores events with `isPrimary === false`, and a synthetic
  `PointerEvent` defaults `isPrimary` to false. This is why WebKit `touch-selection` fails. It is
  a harness defect, not a product defect.
- Native audit baseline: CPAL `default_output_device`/`build_output_stream` appear only in
  `src/host/native/audio/stream.rs`. The only opener is `NativeHosts::open_with_bus_names`, reached
  from `src/cli/mod.rs:256` (default `--host native`). `tests/cli.rs` and `tests/song_cli.rs`
  spawn the CLI with `--host noop`. `editor/src-tauri/src/session.rs:132` tests use
  `HostChoice::Noop`.
- Chromium in run-001 rendered with SwiftShader (`run-001/environment.json` `webgl.renderer`).

## Non-goals

- No change to the production audio output stage (`editor/worklet/host.js`, `src/host/native/*`,
  master gain or limiter). No system audio driver. Never read or change macOS volume.
- No `@playwright/test`, no new dependency, no lockfile change.
- No edit to `editor/src/code/*` (owned by the other wave-3 plans) or `editor/src/code/pointer.ts`.
  The touch failure is fixed in the harness.
- No evidence-document edits and no committed run data. Wave-4 CANVAS-EVIDENCE writes them from an
  outside-sandbox run.

## Ownership

writePaths (manifest `CANVAS-EVIDENCE-SILENT`):

- `editor/test/e2e/silent-sink.mjs` (new)
- `editor/test/e2e/silent-sink.test.ts` (new)
- `editor/test/e2e/run.mjs`
- `editor/test/e2e/behavior.mjs`
- `editor/test/e2e/measure.mjs`
- `editor/test/e2e/stats.mjs`
- `editor/test/e2e/stats.test.ts`
- `editor/test/e2e/fixtures/large-doc.mjs`
- `editor/test/e2e/large-doc.test.ts`
- `editor/test/e2e/ios-sim.mjs`
- `editor/test/e2e/README.md`
- `editor/src/app/main.ts` (only the `reportSelfCheck` field addition)
- `editor/test/app/main.test.ts` (only new self-check rows)
- this plan file
- `tmp/canvas-cutover/evidence-silent/intent.json` and `tmp/canvas-cutover/evidence-silent/receipt.json`
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`, `tmp/canvas-cutover/evidence-silent/logs`

sharedPaths: none.

## Contracts

### `silent-sink.mjs` (new)

- `export function silentSinkInit(): void`: the browser-side function, passed to
  `context.addInitScript(silentSinkInit)`. It must be self-contained, with no closure over module
  scope, because Playwright serializes it.
- `export async function installSilentSink(context): Promise<void>`: calls
  `context.addInitScript(silentSinkInit)`. It must be called before `context.newPage()`.
- `export async function readSinkReport(page): Promise<SinkReport>`: evaluates
  `window.__vactrSink.report()`.
- `window.__vactrSink.now()`: returns `{ ctxTime, pageMs }` (`currentTime` of the first sink
  context and `performance.now()`), or `null` before any sink exists. It is used to split control
  and workload sink onsets at the large-document click.
- Pure helpers, exported for unit tests and also inlined into `silentSinkInit` (duplicated on
  purpose, because the init function cannot import):
  - `dbfs(peak)`: `-Infinity` for 0, otherwise `20*log10(peak)`;
  - `blockRms(samples, sampleRate, blockMs = 10)`;
  - `detectOnsets(blockRmsDb, blockMs, { onDb = -40, quietDb = -50, quietMs = 50 })`, which
    returns the block indexes of onsets.

`SinkReport` (JSON):

```
{ installed: boolean, installedBeforeFirstConnect: boolean, contexts: number,
  wrappedDestinationConnections: number, directDestinationConnections: number,
  mediaElementsForcedMuted: number, violations: string[], suspendedOnViolation: boolean,
  pre: { peak: number, peakDbfs: number, rms: number, rmsDbfs: number, onsetCount: number, onsetTimes: number[] },
  post: { peak: number } }
```

`onsetTimes` are `AudioContext.currentTime` seconds, capped at 4,096 entries.

Graph per realtime `AudioContext` (created lazily on the first destination connection):

```
app node -> preAnalyser (AnalyserNode, fftSize 32768) -> zeroGain (new GainNode(ctx, { gain: 0 }), never automated) -> postAnalyser (AnalyserNode) -> ctx.destination
```

- The wrapper replaces `AudioNode.prototype.connect`. If the target is an
  `AudioDestinationNode` whose `context` is not an `OfflineAudioContext`, it connects to that
  context's `preAnalyser` instead (preserving output/input index arguments) and counts a wrapped
  connection. All other connections call the original.
- The sink's own `postAnalyser -> destination` connection uses the saved original `connect`, so
  it is never counted as direct.
- `AudioNode.prototype.disconnect(destination)` is mapped to disconnect from the sink input.
- Polling (`setInterval`, 100 ms, at most 4,096 report entries): read `getFloatTimeDomainData`
  from both analysers. Process only the samples that are new since the last poll: compute
  `n = round((ctx.currentTime - lastTime) * sampleRate)` and take the last `min(n, fftSize)`
  samples. Without this, overlapping windows duplicate onsets.
- On any violation (post peak > 0, a destination connection seen before install, or a direct
  connection detected), call `ctx.suspend()` immediately, set `suspendedOnViolation`, and record
  the violation.
- Media guard: wrap `HTMLMediaElement.prototype.play` to set `this.muted = true` before
  delegating, and count `mediaElementsForcedMuted` for elements that were not already muted.

### Gating (in `stats.mjs` `evaluate`, applied per browser and per page that ran audio)

| Check | Failure text (prefix) |
|-------|-----------------------|
| `installed && installedBeforeFirstConnect` | `silent-sink: not installed before first destination connection` |
| `directDestinationConnections === 0` | `silent-sink: direct destination connections=<n>` |
| `post.peak === 0` (exactly) | `silent-sink: post-sink peak=<v>` |
| `violations.length === 0` | `silent-sink: <violation>` |
| Workload `pre.peakDbfs <= -1` | `pre-sink peak <v> dBFS exceeds -1` |
| Control `onsetCount >= 1 && pre.peakDbfs > -60` | `control: harness failure ...` |
| Hush quiet wait reached (no new onset row for >= 1 s within 10 s) | `control: hush did not silence control` |
| Workload onsets `attributeWorkloadOnsets(...).count > 0` (post-hush baseline excluded, `receivedMs > largeRunClickMs`) | existing no-playing-onset failure text, with attribution |

`renderEvidence` adds rows per browser: `Silent sink post-sink peak` (threshold `== 0`),
`Direct destination connections` (`0`), `Pre-sink peak (dBFS)` (`<= -1`), `Pre-sink RMS (dBFS)`
(reported), `Pre-sink onsets (count)` (reported), and `Control onsets / peak dBFS`
(`>= 1 / > -60`). It also adds one sentence stating the silent-sink method.

## Tasks

### TASK-001: Silent sink module and unit tests

Files: `silent-sink.mjs` and `silent-sink.test.ts`. The test is `// @vitest-environment node` and
loads the module through the non-literal import pattern (`large-doc.test.ts:7-10`).

Test cases (situation -> expected):

- `dbfs(0)` -> `-Infinity`; `dbfs(1)` -> `0`; `dbfs(0.5)` -> about `-6.02`.
- A 1 s buffer that is silent, then a 200 ms tone at RMS -20 dBFS, then silent again ->
  `detectOnsets` returns exactly 1 onset at block 0 of the tone.
- Two tones separated by 30 ms of silence -> 1 onset (quiet gap < 50 ms). Separated by 80 ms -> 2.
- A tone at -45 dBFS RMS -> 0 onsets.
- The wrapper, run against minimal fake classes installed on a fresh `globalThis`-like object
  (`AudioNode`, `AudioDestinationNode`, `AudioContext`, `OfflineAudioContext`, `GainNode`,
  `AnalyserNode` fakes that record `connect` calls; call `silentSinkInit` with them bound as
  globals through `new Function` or `vm.runInNewContext`, loaded via the non-literal import of
  `node:vm`):
  - `node.connect(ctx.destination)` -> recorded path is `node -> preAnalyser -> zeroGain ->
    postAnalyser -> destination`; the gain value is 0; the report shows wrapped 1 and direct 0.
  - `node.connect(otherNode)` -> unchanged.
  - A second `connect` to the same context's destination -> reuses the same sink (`contexts === 1`).
  - An `OfflineAudioContext` destination -> not wrapped, not counted as direct.
- `evaluate` receives a sink report with `post.peak = 0.001` -> it fails with the
  `silent-sink: post-sink peak` text. `directDestinationConnections = 1` -> fails. A clean report
  -> no silent-sink failure.

### TASK-002: Workload amplitude and control (EVID-TI-001, decision S.4)

- `fixtures/large-doc.mjs`: change the synth definition to `amp: float = 0.005`. Keep 64 voices,
  `note [60 64]`, the `stack [...] > d1` line, the four visual chains, the line count and the
  byte window.
- Change `controlText` to exactly `synthDefinition` plus the head `stack [...] > d1` line (the
  same 64 voices). Return `controlText` and `head` as today.
- `large-doc.test.ts`: change the asserted definition string to `amp: float = 0.005`. Assert
  that `controlText` contains `stack [` and `] > d1` and does not contain `s :pad > note [60 64] > d1`
  as a standalone line. Assert that `controlText` passes the real host-wasm `session_check` with
  zero error diagnostics (same pattern as the existing full-document check). Keep every other
  assertion.
- `measure.mjs` control and workload sequence (PR-S267-001). This replaces the existing early
  baseline snapshot at `measure.mjs:13`, which never stops the control. The control and the
  workload head are the same 64-voice stack, so `from`/`to` cannot separate them; only stopping,
  re-baselining and time-splitting can. Use exactly this order:
  1. Fill `controlText`, click `.vact-run`, and wait (bounded, 10 s) for `onsets().length >= 1`.
     Record `controlOnsetCount` and read `controlSink = readSinkReport(page)`.
  2. Click `.vact-hush` (`editor/src/ui/transport-view.tsx:199`). This existing toolbar button
     sends hush and clears the highlight scheduler (`EvalController` `onHush`).
  3. Quiet wait: poll `onsets().length` until no new row has arrived for at least 1 s, bounded at
     10 s total. If rows keep arriving, record the failure
     `control: hush did not silence control` and continue, so later metrics are still reported.
  4. Re-snapshot the full onset baseline: the JSON keys of every `onsets()` row at this moment
     (the post-hush baseline).
  5. Fill the 1 MiB workload text. Immediately before the large-document `.vact-run` click, read
     in one `page.evaluate` both `largeRunClickMs = performance.now()` and
     `largeRunCtxTime = window.__vactrSink.now().ctxTime`. `now()` is added to the sink contract
     and returns `{ ctxTime, pageMs }` for the first sink context, or `null` if none exists. Then
     click.
  6. After the click returns, wait as today and read `workloadSink = readSinkReport(page)`.
- Attribution definitions:
  - Workload onsets are the `onsets()` rows with `receivedMs > largeRunClickMs` whose key is not
    in the post-hush baseline.
  - Workload sink onsets are the `pre.onsetTimes` entries `> largeRunCtxTime`. Control sink onsets
    are the entries `<= largeRunCtxTime`.
  - The cumulative `pre.onsetCount` is reported only as `sinkOnsetsTotal` and is never used as the
    workload count.
  - `onsetAttribution` keeps its three outcomes, computed from these definitions. The workload
    count 0 with control count >= 1 gives `product/full-document failure` (or the startup-failure
    text when the click timed out).
- Pure helpers in `stats.mjs`, called from `measure.mjs`:
  - `attributeWorkloadOnsets(rows, baselineKeys, clickMs)` returns
    `{ workload: rows[], count, excludedBaseline, excludedPreClick }`;
  - `splitSinkOnsets(times, ctxTime)` returns `{ control: number, workload: number }`.
  - Row keys use the same `JSON.stringify(row)` form as today.
- `evaluate` receives `controlSink` and `workloadSink` and applies the gating table. The workload
  pre-sink peak gate uses the cumulative peak, which is stricter because the control is the same
  program. The workload-onset gate (`> 0`) uses `attributeWorkloadOnsets(...).count` only.
  `summary.json` records `controlOnsetCount`, `hushQuiet` (boolean and quiet-wait ms),
  `largeRunClickMs`, `largeRunCtxTime`, workload onset count, the sink split and
  `sinkOnsetsTotal`.
- `stats.test.ts` new rows (situation -> expected):
  - Only control rows, all with `receivedMs` before the click -> workload count 0, and with
    control count >= 1 the attribution is `product/full-document failure`.
  - Rows with `receivedMs` after the click and not in the baseline -> counted.
  - A row in the post-hush baseline, even with a late `receivedMs` -> excluded (`excludedBaseline`
    1).
  - Sink `onsetTimes [0.5, 1.0, 3.2, 4.0]` split at `ctxTime 2.0` -> control 2 and workload 2,
    reported separately. `sinkOnsetsTotal` 4 is never the workload count.

### TASK-003: Silent sink wiring, Chromium flags and harness fixes

- `run.mjs`:
  - Chromium launch `args` must include `--mute-audio`. Also add the hardware-GL flags
    `--use-angle=metal`, `--enable-gpu` and `--ignore-gpu-blocklist`. Record the WebGL renderer
    string per browser from the measurement page (not from a separate probe browser) in
    `environment.json` as `webgl.<browser>`. If the renderer contains `SwiftShader`, push the
    limitation `Chromium rendered with software GL (SwiftShader); frame metrics are recorded
    failures, never relabeled as passes`.
  - Call `installSilentSink(context)` for the measurement context before `newPage()`.
  - Add `--run-id <id>` (default `run-001`), replacing the hardcoded `runId` and the out
    directory.
  - Write `sink` (control and workload reports) into `summary.json` per browser.
  - Keep exit semantics 0/1/2 exactly.
- `behavior.mjs`:
  - Call `installSilentSink` for every context it creates, including the WebKit DPR-2 context at
    line 125.
  - Canvas readback (check `canvas-only-text`): after the `fill`, register a
    `requestAnimationFrame` callback in the page (and await a fresh text edit first, so the app's
    frame is already scheduled ahead of the harness callback). Inside that same callback, read
    pixels from the code canvas into a 2D canvas (`drawImage` of the WebGL canvas) and count the
    distinct colors there. Do not call `getContext('webgl2')` on the app canvas: it returns the
    app context, but reading outside its frame gives a cleared buffer. Keep the 1x1 backing
    assertion as a separate, explicit failure text.
  - WebKit synthetic touch: pass `isPrimary: true, button: 0, pointerType: 'touch', pointerId: 1`
    in every synthetic pointer event. Keep the WebKit limitation label.
  - DPR (EVID-TI-006): keep the existing session lifetime (detach only in `finally` after all
    assertions). Do not regress it.
- `measure.mjs`:
  - DPR cycle (EVID-TI-006): keep the CDP session attached for the whole cycle loop (already
    true). Assert that the cycle samples contain both `devicePixelRatio` 1 and 2 on Chromium.
    If not, add failure `dpr-cycle: devicePixelRatio never changed`.
  - Keystroke pacing (EVID-TI-007, design profile H): the editing loop targets about 10
    keystrokes per second. Use `page.keyboard.press` and a time-based schedule. Sample
    `counters().usedBytes` at most once per second instead of per iteration. Keep `editKeyCount`
    as the 500-floor input, report `editPairedKeyCount`, and require paired > 0 and unpaired
    <= 10% (already in `stats.mjs`; keep).
  - The heap CDP sessions may detach immediately, because they do not set an override.
- `ios-sim.mjs`: parse the JSON after `VACTR_SELF_CHECK `. Pass only if the line is present and
  `playingEvents === 0`. Record `silent: true` or `false` and the reason.
- `README.md`: document the silent sink, `--mute-audio`, the gates, `--run-id`, and that automated
  runs never touch system volume.

### TASK-004: Self-check playing counter (decision S.2)

- `editor/src/app/main.ts` `reportSelfCheck`: register `deps.client.on('playing', ...)` at the
  start of the function and count envelopes. Add `playingEvents: <count>` to the report JSON.
  Unsubscribe in all paths (including the catch path). Do not change anything else in `main.ts`.
- `editor/test/app/main.test.ts`: add rows (imitate the existing self-check/boot rows):
  - self-check enabled and no playing envelopes -> report contains `"playingEvents":0`;
  - one playing envelope emitted before the report -> `"playingEvents":1`.
  Keep every existing row.

### TASK-005: Native silence audit (decision S.3)

Run these and record each output path in the progress log:

- `grep -rn "default_output_device\|build_output_stream\|NativeHosts::open\|NativeAudioHost::open\|open_quad" src tests editor/src-tauri/src`
- `grep -rn "CARGO_BIN_EXE\|cargo_bin" tests src`
- `grep -rn '"--host"' tests`

Confirm that every CLI spawn either passes `--host noop` or uses a verb that never opens audio
(`render`, `check`, `fmt`, `lsp`). If a test opens a real device, change that test to
`--host noop` and record the extra path as a new sharedPath in this plan before editing it. No
production change is allowed.

## Pitfalls

- Installing the init script after `newPage()`: it then misses the first page. Always install on
  the context first.
- Counting the sink's own `postAnalyser -> destination` link as direct. Use the saved original
  `connect` for it.
- Using `setValueAtTime` or any automation on `zeroGain`. It must be constructed at 0 and never
  touched.
- Reading the full analyser window each poll, which duplicates onsets.
- Comparing `post.peak` with a tolerance. The gate is exactly `0`.
- Letting the control program keep playing into the workload phase. Always click `.vact-hush`,
  wait for quiet, and re-baseline before the large-document Run. The control and the workload
  head are identical, so a still-playing control would fake workload onsets.
- Reporting the cumulative sink onsets (`pre.onsetCount`) as workload onsets. Split them at
  `largeRunCtxTime`.
- Editing product source to help attribution. Use only the existing `.vact-hush` button and the
  `__vactrPerf` and `__vactrSink` APIs.
- Removing the 1x1 check, or relabeling SwiftShader frame failures as passes.
- Writing to `design-docs/specs/evidence/**` or the evidence document from this plan.
- A literal `import 'node:vm'` in a `.ts` test, adding `@types/node`, or `@ts-nocheck`.

## Verification (inside the sandbox; logs under `tmp/canvas-cutover/evidence-silent/logs/`)

| Command | Required evidence |
|---------|-------------------|
| `node --check editor/test/e2e/silent-sink.mjs editor/test/e2e/run.mjs editor/test/e2e/behavior.mjs editor/test/e2e/measure.mjs editor/test/e2e/stats.mjs editor/test/e2e/ios-sim.mjs editor/test/e2e/fixtures/large-doc.mjs` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/e2e/silent-sink.test.ts test/e2e/stats.test.ts test/e2e/large-doc.test.ts` | all pass, including the new `silent-sink.test.ts` cases and the updated `large-doc.test.ts` (requires the host-wasm build from setup). `test/e2e/large-eval.test.ts` belongs to CANVAS-EVIDENCE-RUNSTART. |
| `cd editor && ./node_modules/.bin/vitest run test/app/main.test.ts` | all pass, including the two `playingEvents` rows |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | all pass; total >= 665 plus the new tests. If a failure lies only in another wave-3 plan's files, report it with the log path and do not fix it; the post-join run is authoritative. |
| The three audit greps (TASK-005) | outputs saved; conclusion recorded |

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`,
then `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.

Outside the sandbox (wave 4, by CANVAS-EVIDENCE): the browser run must show post-sink peak
exactly 0, 0 direct connections, pre-sink peak <= -1 dBFS, and control >= 1 onset with peak above
-60 dBFS in both browsers.

## Overwrite and Drift Protocol

Before editing each file, record its sha256 in `tmp/canvas-cutover/evidence-silent/intent.json`,
with the intended edit. After editing, record the new sha256 in `receipt.json`. If a file's hash
differs from the wave start before you edit it, stop editing that file and report it for serial
repair. Edit only this plan's progress log.

## Completion Criteria

- [x] `silent-sink.mjs` and its unit tests are in place and pass
- [x] Workload `amp` is 0.005 with 64 voices; `controlText` is the head stack and passes `session_check`
- [x] The control is hushed and re-baselined before the large-document Run. Workload onsets are post-click only (`attributeWorkloadOnsets`), sink onsets are split at `largeRunCtxTime` (`splitSinkOnsets`), and the four `stats.test.ts` attribution rows pass
- [x] `run.mjs` launches Chromium with `--mute-audio` and the GPU flags, installs the sink on every context, records the renderer per browser, and accepts `--run-id`
- [x] Silence gates and evidence rows are implemented in `stats.mjs` and tested
- [x] Canvas readback happens in-frame; WebKit synthetic touch sends `isPrimary: true`
- [x] DPR cycle assertion and 10/s keystroke pacing are done; the 500 floor stays on `editKeyCount`
- [x] `ios-sim.mjs` asserts `playingEvents === 0`; `main.ts` self-check reports `playingEvents` and is tested
- [x] Native audit recorded (with any test fix)
- [x] All verification commands pass with log paths recorded

## Progress Log

### Session: 2026-10-05 (session 267 plan)
**Tasks Completed**: Plan authored from design 15.3.8.8 (session-267 amendment). Implementation pending.

### Session: 2026-10-05 (session 267 plan revision, PR-S267-001)
**Tasks Completed**: TASK-002 now requires the following sequence between the control and the workload Run: control Run, controlSink, `.vact-hush`, a bounded 1 s quiet wait, a post-hush re-baseline, `largeRunClickMs`/`largeRunCtxTime`, then the large-document click. It also requires post-click-only workload onsets, the sink onset split, the pure `attributeWorkloadOnsets`/`splitSinkOnsets` helpers with four tests, `__vactrSink.now()`, the related pitfalls and a completion checkbox. Gates, ownership and paths are unchanged.


### Session: 2026-10-05 (Riela Step 6 implementation, CANVAS-EVIDENCE-SILENT)
**Tasks Completed**: Added `silent-sink.mjs` and unit coverage; routed browser contexts through the virtual sink; lowered workload amplitude and switched the control to the 64-voice head stack; added hush, quiet wait, post-hush baseline and click/time attribution; added 10-key/s editing and DPR checks; captured per-browser renderer; required the iOS self-check to report zero playing events; added `playingEvents` to the self-check and tests; documented the silent method. No Rust tests opened a real output device.

**Native silence audit**: `src/host/native/audio/stream.rs` contains the only CPAL output-device/stream openers; the production open path is `src/cli/mod.rs`. CLI test spawns are either `--host noop` for run/repl/serve or non-output verbs (`render`, `lsp`). No offending test needed modification. Audit logs: `tmp/canvas-cutover/evidence-silent/logs/native-audio-audit.log`, `native-cli-spawn-audit.log`, `native-host-flags-audit.log`.

**Final verification** (exit 0 unless shown): `node --check ...` — `tmp/canvas-cutover/evidence-silent/logs/node-check-final3.log`; host-wasm build — `wasm-build.log`; focused evidence tests, 22/22 — `focused-vitest-final5.log`; self-check tests, 9/9 — `main-vitest.log`; `npm run check` — `npm-check-final5.log`; full Vitest, 674/674 across 87 files — `full-vitest-final5.log`; native audit greps — logs above.

**Prior verification**: The initial focused run failed 2 assertions (fake connection edge count and a baseline row timestamp); both fixtures were corrected. `npm run check` then found one excess-property typing issue in the expanded attribution test; the local test interface was corrected. Final focused and full Vitest runs pass. Browser and simulator execution is assigned to outside-sandbox wave 4.
