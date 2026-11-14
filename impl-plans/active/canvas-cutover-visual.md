# Canvas Cutover: Synchronized Visual Composition Implementation Plan

**Status**: In Progress
**Plan ID**: CANVAS-VISUAL (wave 2; depends on CANVAS-CLOCK)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.6 (also the 15.3.5 caps and 15.3.8.7 per-frame uploads)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

Hydra synth effects, oscilloscopes and analyzer scopes, and a single muted video background must
follow the audible clock. The composite visual canvas must also be offered to the code editor as
its background, through the existing `VisualApi.onBackgroundCanvas`.

This plan touches only `editor/src/visual/*` and its tests. CANVAS-MOUNT wires the editor side
in the same wave.

Facts at c9e5a05:

- `visual/frame.ts:61-62` calls `core.frame(clock.now())` and `host.draw(clock.now())`.
- `visual/mount.ts:54-57` sets `deps.visual = { mountSpectrum }` and does not implement
  `onBackgroundCanvas`.
- On the native tier there is no Wasm core. `visual/mount.ts:62-65` shows `NATIVE_NOTICE`.
  That stays: native visual-language execution is out of scope (design 15.3.1). Because
  `GlRenderHost` is created only on the browser tier (`visual/mount.ts:59-66`), video
  composition is also browser-tier only. Native and iPad tiers show the notice and no video,
  which CANVAS-EVIDENCE lists as a limitation. Do not add a native-tier render host.
- `visual/meters.ts:AnalyzerArea` draws scope and spectrogram cells from `store.levels`
  immediately, with no timestamp.
- `ResourceLedger` and `RESOURCE_LIMITS` are in `editor/src/code/resources.ts`. The video cap is
  1280x720 RGBA8, 3,686,400 bytes. The ledger already models one video
  (`gpu.test.ts:71`, "one video").

## Non-goals

- No language syntax, no camera, no streaming, no audio from the video.
- No edits to `code/*`, `app/*` or `protocol/*`. The levels `time`/`epoch` types come from
  CANVAS-CLOCK.
- No native-tier Hydra.
- No new dependency.

## Ownership

writePaths: see the manifest entry `CANVAS-VISUAL`.

| File | Change |
|------|--------|
| editor/src/visual/frame.ts | `FrameLoopOptions.audible?: AudibleClock`; use `audible.sample(frameMs).time` for both `core.frame` and `host.draw` |
| editor/src/visual/mount.ts | Pass `deps.audible ?? uncorrelated(deps.clock)`; implement `onBackgroundCanvas`; create the video source and control; pause video when hidden |
| editor/src/visual/render-host.ts | Optional video base layer under `o0`: `setVideoSource(source: TexImageSource \| null, revision: number)` plus an alpha composite pass |
| editor/src/visual/video.ts (new) | `VideoBackground` |
| editor/src/visual/meters.ts | `LevelsTimeline` (8 frames, latest-wins) used by `AnalyzerArea` |
| editor/src/visual/scopes.ts | Pure helper `pickFrame(frames, t)` used by the timeline |
| editor/src/visual/panes.ts | File-input control ("Video background", `accept="video/*"`) and decode status text |
| editor/test/visual/frame.test.ts, render-host.test.ts, scopes.test.ts, meters.test.ts, panes.test.ts, video.test.ts (new) | Tests |

sharedPaths (conditional): `editor/src/visual/panes-view.tsx` and `editor/src/visual/visual.css`.
Edit them only if the panes control must be rendered through the Solid view.

## Contracts (pinned)

```ts
// video.ts
export class VideoBackground {
  constructor(doc: Document, opts: { ledger?: ResourceBudget; createCanvas?: () => HTMLCanvasElement; onStatus?(s: string): void });
  load(file: Blob): Promise<void>;        // object URL; muted, playsInline, loop; play() after the user gesture that chose the file
  frame(): { source: TexImageSource; revision: number } | null; // revision increments only on a new decoded frame, <= 30 Hz
  setVisible(visible: boolean): void;     // pause when hidden; resume play when visible
  dispose(): void;                        // revoke URL, removeAttribute('src'), load(), release ledger bytes
}
// meters.ts
export class LevelsTimeline { push(body: LevelsBody, receivedAt: number): void; at(t: number): LevelsBody | null; }
```

`onBackgroundCanvas(cb)` behavior:

- `cb(glCanvas)` is called immediately if the canvas exists, and again after each drawn frame,
  so each call is a new revision for the editor renderer.
- It returns an unsubscribe function.
- `cb(null)` is called on dispose.

## Tasks

### TASK-001: Audible Hydra frame (frame.ts, mount.ts)

- Pass the rAF timestamp to `audible.sample(frameMs)` and use the same `t` for `core.frame(t)`
  and `host.draw(t)`.
- When the sample is invalid, use `uncorrelated(deps.clock).now()` (unsynchronized) so that
  visuals keep running.
- Keep the existing visibility pause.

### TASK-002: Timestamped scopes (meters.ts, scopes.ts)

- `LevelsTimeline.push` keeps at most 8 entries. When full, drop the oldest.
- `at(t)` returns the newest entry with `time <= t` and age at most 2 s.
- Entries without `time` are returned immediately, marked unsynchronized
  (`data-sync="unsynced"` on the area).
- An entry older than 2 s results in `null`, and the displays are hidden
  (`data-sync="hidden"`).
- `AnalyzerArea` presents from `timeline.at(t)` on each visual frame. Without the visual loop
  (native tier), it presents on store updates using the latest entry, as today.

### TASK-003: Video background (video.ts, render-host.ts, panes.ts, mount.ts)

- **Frame detection.** Use `requestVideoFrameCallback` when present; otherwise compare
  `currentTime` on each visual frame. A new frame is accepted only if at least 33 ms passed since
  the last accepted frame (30 Hz cap).
- **Size cap.** If `videoWidth > 1280` or `videoHeight > 720`, draw an aspect-fit downscale into
  one reused 2D canvas and return that canvas as the source. Reserve 1280x720x4 bytes in the
  ledger before the first texture upload. If the reservation fails, set status "video exceeds
  GPU budget" and do not upload.
- **Upload.** `GlRenderHost.setVideoSource` uploads into one reused texture (`texSubImage2D`
  when the size is unchanged), only when the revision changed. The base pass draws the video;
  then `o0` is composited using its alpha. When no video is present, the output is identical to
  today.
- **Errors.** Decode errors (`error` event, or a rejected `play()`) set the pane status text.
  The audio track stays muted (`muted = true`, `volume = 0`).
- **Visibility.** `mount.ts` calls `video.setVisible(doc.visibilityState !== 'hidden')` in the
  existing `onVisibility` handler.

### TASK-004: Tests

| File | Situation | Expected outcome |
|------|-----------|------------------|
| frame.test.ts | Fake audible sample `t = 12.5` at frame 1000 | `core.frame` and `host.draw` both receive 12.5 |
| same | Invalid sample | Fall back to `deps.clock.now()` |
| meters.test.ts | Push 10 entries | Holds 8 |
| same | `at(t)` with entries at 1.0, 1.1 and 1.2, `t = 1.15` | Returns the 1.1 entry |
| same | `t = 3.5` | `null` and the area shows `data-sync="hidden"` |
| same | Entries without `time` | Shown immediately and marked unsynced |
| video.test.ts (jsdom with a fake video element) | `load(blob)` | `video.muted === true`, an object URL created |
| same | Two `currentTime` changes within 33 ms | One revision increment |
| same | 1920x1080 source | Frame source is the 1280x720 reused canvas |
| same | Ledger refusal | Status set, no upload |
| same | `setVisible(false)` | `pause()` called |
| same | `dispose()` | `revokeObjectURL` called, `src` removed, `load()` called, ledger bytes released |
| render-host.test.ts | Video revision unchanged across 3 draws | One texture upload |
| same | Revision changed | `texSubImage2D` reuse |
| same | Video then `o0` | Draw order verified on the fake GL |
| panes.test.ts | File input change | Calls `video.load`; status text shows decode failure on error |
| mount-level (in frame.test.ts or panes.test.ts) | `deps.visual.onBackgroundCanvas(cb)` after 2 frames | `cb` called 1 + 2 times with the GL canvas; unsubscribe stops the calls |

## Pitfalls

- Using `video.currentTime` as the musical clock. Musical uniforms always use audible time.
- Allocating a new texture or canvas per frame. Reuse both.
- Leaving the decoder alive on dispose. All of `revokeObjectURL`, `removeAttribute('src')` and
  `load()` are required.
- Calling `onBackgroundCanvas` callbacks on every rAF even when nothing was drawn. Call them only
  after an actual `host.draw`.
- Editing `code/*`, `app/*` or `protocol/*`.

## Session 266 Amendment (operator decisions A and D)

- **Artifact roots** (manifest `CANVAS-VISUAL.artifactRoots`, each also a writePath): `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm`, `tmp/canvas-cutover/visual`, `editor/node_modules/.vite`.
  These are gitignored outputs and are never committed. If a command writes any other gitignored
  in-repo path, stop and record it.
- **Setup (not gating)**: `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`,
  then the host-wasm library build.
- **Rule D**: the gating list contains only final-source commands that are expected to pass.
  Mutation and negative-control runs go under `mutationEvidence`, and failed-then-fixed runs go
  into history notes only.

## Verification (record in tmp/canvas-cutover/visual/checks.log)

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/visual` | all pass |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` then `cd editor && ./node_modules/.bin/vitest run` | full suite passes |
| `wc -l editor/src/visual/*.ts` | each < 1000 |

## Overwrite and Drift Protocol

Record fresh-read sha256 values before each edit (`tmp/canvas-cutover/visual/intent.json`) and
after it (`receipt.json`). On drift, stop editing that file and repair serially after the join.
Edit only this plan's progress log.

## Completion Criteria

- [x] Hydra uses audible time, with the same `t` for frame and draw
- [x] Scopes are timestamp-presented with the 8-entry bound and the 2 s hide
- [x] Video background is capped, reused, paused when hidden, and disposed correctly
- [x] `onBackgroundCanvas` is implemented per the contract
- [x] `npm run check` exit 0; full vitest passes

## Progress Log

### Session: 2026-10-05
**Tasks Completed**: Plan authored

### Session: 2026-10-05 (session 266 plan amendment)
**Tasks Completed**: Plan amended per operator decisions A and D. Artifact roots declared, setup separated from gating. Tasks and contracts are unchanged.

### Session: 2026-10-05 (CANVAS-VISUAL implementation)
**Tasks Completed**: TASK-001 through TASK-004 implemented. Hydra core and render host consume one audible-clock sample per rAF timestamp. Analyzer levels use an eight-entry timeline, timestamp selection and the two-second stale hide; native presentation uses the latest levels update. Added a muted local video background with 30 Hz frame admission, a reused 1280x720 aspect-fit canvas, budget reservation, one reused GL texture and alpha composition below `o0`. Added the visual-pane file control/status and `onBackgroundCanvas` immediate, per-drawn-frame, unsubscribe and dispose notifications.
**Verification**: `npm run check` exit 0; focused visual Vitest 7 files/37 tests passed; full Vitest 83 files/644 tests passed; host-wasm build exit 0; visual TypeScript files all below 1000 lines; scoped `git diff --check` exit 0. Final logs and final-source SHA-256 values are in `tmp/canvas-cutover/visual/checks.log` and `receipt.json`.
**History**: Initial TypeScript/visual attempts exposed callback typing, test assumptions, source-size narrowing, and a decoded-frame notification that could be consumed twice after a rate-limit skip. The implementation and regression test were corrected; the final2 source-matched gates pass. Complete logs are retained under `tmp/canvas-cutover/visual/initial-*`, `retry-*`, `fix2-*`, `final-*`, and `final2-*`.
**Handoff**: Implementation and required behavioral verification are complete. Independent test-integrity, adversarial and combined-tree integration reviews remain assigned to later workflow steps. When CANVAS-MOUNT supplies `deps.resourceBudget`, video reservations share the code renderer ledger; visual mount uses its own bounded `ResourceLedger` if that optional dependency is absent.

### Session: 2026-10-05 (VIS-TI-001 / VIS-TI-002 revision)
**Tasks Completed**: Strengthened the existing compositor test to associate each `drawArrays` call with its preceding `bindTexture`, asserting video first and `o0` second while preserving draw-count, blend-order and program-identity assertions. Added media `error` event status coverage including post-dispose silence, and rejected `play()` status coverage. No production source change remains.
**Verification**: `cd editor && ./node_modules/.bin/vitest run test/visual/render-host.test.ts test/visual/video.test.ts` passed 2 files/14 tests; `cd editor && ./node_modules/.bin/vitest run test/visual` passed 7 files/39 tests; `cd editor && npm run check` exit 0; full Vitest passed 83 files/646 tests; host-wasm build exit 0. Mutation controls are recorded separately: swapping compositor draws failed at the video texture assertion, and removing the error listener failed at the media error assertion. Both production visual source SHA-256 values match their pre-mutation values. Logs and final hashes are in `tmp/canvas-cutover/visual/revision-*`, `mutation-vis-ti-001.log`, `mutation-vis-ti-002.log`, `checks.log` and `receipt.json`.
**Handoff**: The two test-integrity findings are implemented with current-source behavioral evidence. Independent Opus test-integrity and adversarial re-review, then combined-tree integration review, remain downstream.

### Session: 2026-10-05 (Step 6 final-source rerun)
**Tasks Completed**: Re-ran the assigned final-source checks on the combined working tree; no source behavior changed. `npm run check` passed; focused visual Vitest passed 39/39; full Vitest passed 650/650; host-wasm build passed; all seven visual TypeScript files remain below 1000 lines; scoped `git diff --check` passed.
**Evidence**: Complete logs are `tmp/canvas-cutover/visual/step6-final-npm-check-rerun.log`, `step6-final-visual.log`, `step6-final-full-vitest.log`, and `step6-final-host-wasm.log`. Line counts and diff-check were observed directly in the Step 6 command output.
**Handoff**: Implementation verification is current. Independent review and combined-tree integration review remain downstream.
