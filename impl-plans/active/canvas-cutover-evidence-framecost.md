# Canvas Cutover: Per-Frame JS Costs and Release-Wasm Evidence Build (Decisions E and D) Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-EVIDENCE-FRAMECOST (dispatch wave 7 of the session-277 run; runs alone after CANVAS-EVIDENCE-SCHED is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.13 ("Measurement build (decision D; amends 15.3.8.8)", scope record E, "Ownership and order"), 15.3.8.7 (ABI bounds), 15.3.8.10 (no slice over 64 KiB on the edit path)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-EVIDENCE-FRAMECOST`)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

The user wants sustained animation on large documents. The evidence must be measured on the
build that ships. The operator diagnosis (`tmp/canvas-cutover/diag-wasm/REPORT.md` sections 1,
3 and 4) found two problems.

First, run-001 measured the 161 MB debug wasm. `editor/vite.config.ts:31` sets `DEFAULT_WASM`
to `../target/wasm32-unknown-unknown/debug/vactr.wasm`. Debug is 6 to 17 times slower than
release.

Second, the release profile still shows these JS frame costs:

- `gl.isTexture` per draw command (`editor/src/code/renderer.ts:269`, 18 to 22% of main
  thread);
- `gl.getError` every frame (`renderer.ts:242`);
- `Intl.Segmenter` in `TextLayout.boundary` (`editor/src/code/layout.ts:127-138`) for every
  animated range on every frame, reached from `renderer.ts` `animatedRects` (14% of edit time).

Decision D makes the release wasm with the `name` section the only gating evidence build.
Decision E removes the three frame costs without any behavior change.

## Non-goals

- No change to `editor/test/support/wasm.ts`. The vitest wasm default stays debug, so the
  15.3.8.11 perf budget stays calibrated.
- No change to `Cargo.toml` `[profile.release]` (it keeps `strip = true`), `Cargo.lock`, or any
  Rust file.
- No change to `editor/src/code/atlas.ts`, `resources.ts`, `accessibility.ts`, `keyboard.ts`,
  `mount.ts`, `perf-hook.ts`, `stats.mjs`, `measure.mjs` or `behavior.mjs`. CANVAS-EVIDENCE
  owns `stats.mjs` and adds the wasm line to the generated evidence section.
- No change to thresholds, the workload, the silent sink or the e2e exit codes 0/1/2.
- No new dependency. Hashing uses `node:crypto`. Do not add `preserveDrawingBuffer` or any
  new WebGL extension.
- Do not run a browser. Playwright runs outside the sandbox in the CANVAS-EVIDENCE wave.

## Ownership

writePaths:

- `editor/src/code/renderer.ts`
- `editor/src/code/layout.ts`
- `editor/test/canvas/gpu.test.ts`
- `editor/test/canvas/mount.test.ts`
- `editor/vite.config.ts`
- `mise.toml`
- `editor/test/e2e/run.mjs`
- `editor/test/e2e/README.md`
- `editor/test/e2e/wasm-profile.mjs` (new)
- `editor/test/e2e/wasm-profile.test.ts` (new)
- `impl-plans/active/canvas-cutover-evidence-framecost.md` (progress log and checkboxes only)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`, `editor/dist`, `tmp/canvas-cutover/framecost`

sharedPaths: none. `renderer.ts`, `layout.ts`, `gpu.test.ts`, `mount.test.ts`, `run.mjs` and
`README.md` are wave-8 sharedPaths of CANVAS-EVIDENCE. This plan owns them during wave 7, and
CANVAS-EVIDENCE starts only after this plan is accepted.

## Contracts and Key Points

### E1. Texture validity by generation (`renderer.ts`)

- `interface DrawCommand` (line 19) gains `generation?: number`.
- The renderer gains `private textureGeneration = 0` and `private seenEvictions = 0`.
- `addCommand` stamps commands that carry a `texture` with the current `textureGeneration`.
  The texture is live at record time, because recording happens while the texture is being
  drawn or was just uploaded.
- `drawCommand` skips a texture command whose `generation !== this.textureGeneration`. It
  never calls `gl.isTexture`.
- Bump `textureGeneration` wherever textures that recorded commands may reference are deleted:
  - the `loss` handler;
  - the `restore` handler;
  - `fontsChanged` (atlas invalidate);
  - the two `atlas?.invalidate()` sites in `resize`;
  - `dispose`;
  - at the start of `render` when `this.atlas.stats.evictions !== this.seenEvictions` (then
    update `seenEvictions`).
- Evictions already change `cacheKey` and force a rebuild, so the bump is a safety net that
  keeps today's "never draw a deleted texture" guarantee.
- Plan decision: design 15.3.8.13 says each texture records its generation when registered with
  the `ResourceLedger`. This plan records it on the draw command instead, which needs no
  `atlas.ts` or `resources.ts` edit. It is equivalent because a command is created only from a
  live texture, and every later deletion bumps the generation. Record this in the progress log.
- `getError`: remove the per-frame `getError` at line 242. Keep the existing calls:
  - init (line 143);
  - canvas allocation in `resize` (line 162);
  - backdrop upload (line 354);
  - `atlas.ts:90` text upload (not edited).

  Per-frame failure detection relies on the existing `webglcontextlost` handling and the
  `fail` / GPU-unavailable path. Do not add a new diagnostic option.

### E2. Cached grapheme clusters (`layout.ts`)

- `TextLayout.stats` becomes `{ builds, evictions, segmentations }`. `segmentations` counts
  every `segmenter.segment(...)` call made by the layout. Before adding the field, grep
  `editor/test` and `editor/src/code/perf-hook.ts` for whole-object `toEqual` assertions on the
  layout stats, and keep them passing.
- The shape-cache entry becomes `{ line, bytes, clusters }`. `clusters` is either
  `Int32Array | number[]` (pairs of relative `[start, end)` for graphemes longer than one UTF-16
  unit) or `null` (unknown; use the fallback). The exported `ShapedLine` type does not change.
- `shape()` computes `clusters` when it builds a line:
  - A line with no code unit at or above 0x80 gets an empty list and no segmenter call.
  - Otherwise, segment once (`segmentations++`) and keep only clusters longer than one unit.
  - Above `MAX_CLUSTERS_PER_LINE = 4096` pairs, store `null`.
  - Add 8 bytes per pair to the entry's `bytes`, so the existing 8 MiB layout cap still bounds
    the cache.
- `boundary(pos, bias)`:
  - Keep the clamp and the `pos > index.to` branch.
  - Then peek the cache entry of `lineAt(pos)` with `Map.get` (no LRU reorder). With non-null
    `clusters`, binary-search for a cluster where `start < relative < end`. Snap to
    `start` (bias < 0) or `end` (bias > 0); otherwise return `pos`. No `slice` and no segmenter.
  - Without a usable entry, run the existing loop with `segmentations++`.
  - Results must be identical to today for every position and bias.
- `renderer.ts` `animatedRects` snaps only positions inside
  `[visibleLines[0].from, visibleLines.at(-1).to]`. Positions outside are used raw, because
  they are only compared with visible line bounds. Visible lines are shaped (and so cached) on
  every text-dirty frame, so an animation-only frame makes zero segmenter calls. Keep the
  `to > line.to ? 8 : 0` tail behavior unchanged. When `visibleLines` is empty, return `[]`.

### D1. Release wasm with the name section (`mise.toml`, `vite.config.ts`)

- `mise.toml` gains this task:

  ```toml
  [tasks.build-wasm-release]
  description = "Release host-wasm with the name section (gating evidence build, design 15.3.8.13)"
  run = "CARGO_PROFILE_RELEASE_STRIP=debuginfo cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features host-wasm"
  ```

  Session 282 (design 15.3.8.13 D correction): the override is `STRIP=debuginfo`, not `none`.
  `Cargo.toml` `strip = true` links wasm with `--strip-all`, which drops the `name` section.
  `debuginfo` links with `--strip-debug`, which keeps `name` and removes every `.debug_*`
  section, including DWARF that comes in from the prebuilt std. With `none`, that std DWARF
  can stay in the module. Imitate `[tasks.build-release]`. `--lib` matters: without it, the `vactr` bin and the cdylib
  share the uplifted file name (`editor/test/support/wasm.ts:93`).
- `editor/vite.config.ts`:
  - `DEFAULT_WASM = '../target/wasm32-unknown-unknown/release/vactr.wasm'`;
  - update the header comment;
  - the missing-artifact error names `mise run build-wasm-release` and `VACTR_WASM` (for
    example `VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm` for a debug dev
    build);
  - nothing else changes.

### D2. Wasm provenance and gating refusal (`wasm-profile.mjs`, `run.mjs`)

`editor/test/e2e/wasm-profile.mjs` is a new Node ESM module using only `node:fs`, `node:crypto`
and `node:path`. Pinned exports:

- `customSectionNames(bytes: Uint8Array): string[]`. Walks the wasm sections after the 8-byte
  header, reading the id byte and the LEB128 size. For id 0 it decodes the LEB128 name length
  and the UTF-8 name. It does not compile the module, which keeps the 161 MB debug build cheap.
  It throws on bad magic or a truncated section.
- `classifyWasm({ sha256, dwarf }, { releaseSha256, debugSha256 }) -> 'release' | 'debug' | 'unknown'`:
  - `release` iff `releaseSha256 !== null && sha256 === releaseSha256`;
  - else `debug` iff `sha256 === debugSha256` (with `debugSha256` non-null) or `dwarf`;
  - else `unknown`.
- `gatingRefusal({ profile, nameSection, dwarf }) -> string | null`: `null` only for
  `release` + `nameSection` + `!dwarf`. Otherwise a one-line reason that names
  `mise run build-wasm-release`.
- `async inspectWasm(file, { releasePath, debugPath })` returns
  `{ path, bytes, sha256, nameSection, dwarf, profile, releasePath, releaseSha256, debugSha256 }`.
  `nameSection` means a custom section `name` is present. `dwarf` means any custom section
  name starts with `.debug_`. A reference file that does not exist gives a `null` sha.

In `editor/test/e2e/run.mjs` (keep its compact style):

- Right after the `dist/index.html` existence check, call
  `inspectWasm(path.join(editorRoot, 'dist', 'vactr.wasm'), { releasePath: path.join(repoRoot, 'target/wasm32-unknown-unknown/release/vactr.wasm'), debugPath: path.join(repoRoot, 'target/wasm32-unknown-unknown/debug/vactr.wasm') })`.
- Store the result plus `gating: writeEvidence` as `host.wasm`, so it lands in
  `environment.json` and `summary.environment`. Also add a top-level `summary.wasm`.
- If `writeEvidence` and `gatingRefusal(...)` returns a reason:
  - print the usual result JSON with `pass: false`, `blocked: true`, the `wasm` block and
    `fatal` set to the reason;
  - write nothing (no `mkdirSync(out)`, no evidence files, no evidence-document edit);
  - launch no server or browser;
  - exit 2.
- `host.commands` becomes
  `[command, 'mise run build-wasm-release', 'cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build']`.
- Exit-code semantics are unchanged otherwise.

`editor/test/e2e/README.md`: put `mise run build-wasm-release` before the build line. Document:

- the wasm provenance block;
- that a gating (`--write-evidence`) run refuses anything but the release build with the
  name section and no DWARF, with exit 2;
- that non-gating runs record `gating: false`;
- the `VACTR_WASM` debug override for development.

## Tasks

### TASK-E1: Renderer generation and getError
**Deliverables**: `renderer.ts`, `gpu.test.ts`

### TASK-E2: Layout cluster cache and animatedRects snapping
**Deliverables**: `layout.ts`, `renderer.ts`, `gpu.test.ts`, `mount.test.ts`

### TASK-D1: Release build task and vite default
**Deliverables**: `mise.toml`, `vite.config.ts`, `README.md`

### TASK-D2: Wasm provenance and refusal
**Deliverables**: `wasm-profile.mjs`, `wasm-profile.test.ts`, `run.mjs`, `README.md`

### TASK-F1: Verification and progress log

## Test Cases

`editor/test/canvas/gpu.test.ts`. The fixture's `width()` helper itself uses `Intl.Segmenter`,
so count layout segmentation with `f.l.stats.segmentations`, not a prototype spy.

- Animation-only frames:
  - Render `{ textRevision: 1 }`.
  - Spy on `isTexture` and `getError` of the fixture's fake GL, and read
    `f.l.stats.segmentations`.
  - Render 10 frames `{ textRevision: 1, animated: [...] }` with ranges covering the ASCII
    line and the `日本` line.
  - Expect 0 `isTexture` calls, 0 `getError` calls, a segmentations delta of 0, unchanged
    `textBuilds` and atlas uploads, and `drawCalls` increasing.
- A stale generation is never drawn:
  - render, `lose()`, dispatch `webglcontextlost` and then `webglcontextrestored`, then render
    an animation-only frame;
  - every `drawArrays` in the fake runs with a bound texture that is live in the fake's `live`
    map (add a check in the fake's `drawArrays` or `bindTexture` wrapper inside this test file).
- Boundary equivalence:
  - Inputs: ASCII, Japanese, emoji with ZWJ, `e` + combining mark, surrogate pairs, and CRLF
    and CR documents.
  - For every position and both biases, compare `layout.boundary` with a reference copy of the
    old loop written in the test. Do it twice: once with the line shaped (cached clusters) and
    once with `invalidate()` (fallback).
- Cluster cap:
  - A line with more than 4096 emoji gets `null` clusters, and `boundary` on it falls back
    (segmentations increments).
  - `cacheBytes` grows by the pair bytes for a line with a few emoji.

`editor/test/canvas/mount.test.ts` (new `it`):

- Rig: imitate `setup(true, true)`. Add a `WasmCore` over `FakeCore` exports as `deps.core`, in
  the same way as `editor/test/protocol/song.test.ts:245`:
  `const core = new WasmCore(); core.attach(new VactrHost(null, fakeNode(), fake.exports, { wasmUrl: '', processorUrl: '', init: 'session', onRecord: core.onRecord }))`.
  Extend the local `setup` with an optional `core` parameter without changing existing
  callers.
- Load a document of at least 200 KiB (imitate the 20,000-line `largeText` in the
  `does no whole-document work...` test). Emit a `playing` event that covers visible text.
- Start `startFrameLoop` from `editor/src/visual/frame.ts`:
  - `core`;
  - `host: { draw() {} }`;
  - `clock: rig.deps.clock`;
  - `scheduler: { request: cb => rig.host.requestAnimationFrame(cb), cancel: id => rig.host.cancelAnimationFrame(id) }`.
- `rafHost.step` runs only one callback. Add a test-local helper that runs every callback
  registered before the frame began, at the same timestamp. Do not change `rafHost` for
  existing tests.
- Step until `textBuilds` is stable. Then clear the `FakeCore` call log and spy on
  `Intl.Segmenter.prototype.segment`, `isTexture` and `getError` of the rig GL. Run 10
  animation-only frames.
- Expect:
  - exactly 10 `session_frame` calls (one per frame);
  - 0 `session_check` calls;
  - no call whose `text` or `bytes` exceeds 64 KiB;
  - 0 `isTexture`, 0 `getError` and 0 `segment` calls.
- If the segment spy catches a call outside `layout.ts`, record its stack in the progress log
  and report it as a finding. Do not edit that file; it is not in this plan's writePaths.

`editor/test/e2e/wasm-profile.test.ts` (node environment, like `stats.test.ts`):

- `customSectionNames` on handcrafted bytes:
  - header plus a custom `name` section plus a custom `.debug_info` section plus one type
    section -> `['name', '.debug_info']`;
  - bad magic throws;
  - a truncated size throws.
- `classifyWasm` rows:
  - release sha match -> `release`;
  - debug sha match -> `debug`;
  - `dwarf` without match -> `debug`;
  - neither -> `unknown`;
  - `releaseSha256: null` -> never `release`.
- `gatingRefusal` rows:
  - release + name + no DWARF -> `null`;
  - release without name -> a reason containing `build-wasm-release`;
  - release with DWARF -> a reason;
  - debug -> a reason;
  - unknown -> a reason.
- `inspectWasm` on temp files under `os.tmpdir()`: an artifact identical to the "release"
  reference -> `profile: 'release'` and correct `bytes` and `sha256`; a missing reference ->
  `releaseSha256: null`.

## Pitfalls

- Do not keep any `isTexture` call "just in case". The generation is the guard.
- `boundary` is used by `advance`, `rangeRects`, `posAtCoords` and `offsetInRun`. Equivalence
  must hold for all of them, and the gpu tests "never returns a surrogate, combining or ZWJ
  interior" and "uses whole-run shaping and measured Japanese/ligature caret advances" must
  stay green unmodified.
- Do not peek the cache with an LRU reorder (`delete`/`set`) inside `boundary`. That would
  change eviction order in the existing LRU tests.
- `animatedRects` with ranges entirely above or below the viewport must still produce no
  rects, exactly as today.
- Changing the vite default breaks `npm run build` until the release artifact exists. Run
  `mise run build-wasm-release` first. Every later verification and the Tauri and iOS builds
  depend on this ordering.
- In `run.mjs`, the refusal must happen before `fs.mkdirSync(out)` and before
  `startServer()`. The existing write-evidence block at the end must not run on refusal.
- `customSectionNames` must handle LEB128 sizes up to 5 bytes and must not read past the
  buffer.
- Session 282: the artifact contract decides, not the flag. If `inspectWasm` on the built
  release module shows `nameSection: false` or `dwarf: true`, fix only the
  `[tasks.build-wasm-release]` command in `mise.toml`. For example, strip DWARF while keeping
  `name`; do not use `strip = true`/`symbols`. Record the corrected command and the
  inspection result. Never relax `gatingRefusal`, never edit `Cargo.toml`, and never report the
  failure as a pass.

## Verification

Setup (not gating):

- `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  (the debug wasm used by vitest)

At start, record `BASE=$(git rev-parse HEAD)` and the fresh-read sha256 values in
`tmp/canvas-cutover/framecost/intent.json`.

Gating, inside the sandbox (logs in `tmp/canvas-cutover/framecost/`):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts test/canvas/mount.test.ts test/e2e/wasm-profile.test.ts` | exit 0; the new cases are listed by name |
| `cd editor && ./node_modules/.bin/vitest run test/canvas test/code test/e2e` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 (default config) |
| `cd editor && npm run check` | exit 0 |
| `node --check editor/test/e2e/run.mjs editor/test/e2e/wasm-profile.mjs` | exit 0 |
| `CARGO_TERM_QUIET=true mise run build-wasm-release` | exit 0; `target/wasm32-unknown-unknown/release/vactr.wasm` exists (record bytes and sha256) |
| `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 with no `VACTR_WASM` set (release default) |
| `cd editor && node --input-type=module -e "import { inspectWasm } from './test/e2e/wasm-profile.mjs'; const w = await inspectWasm('dist/vactr.wasm', { releasePath: '../target/wasm32-unknown-unknown/release/vactr.wasm', debugPath: '../target/wasm32-unknown-unknown/debug/vactr.wasm' }); console.log(JSON.stringify(w)); if (w.profile !== 'release' \|\| !w.nameSection \|\| w.dwarf) process.exit(1);"` | exit 0; prints `profile: "release"`, `nameSection: true`, `dwarf: false`, plus bytes and sha256 |
| `wc -l editor/src/code/renderer.ts editor/src/code/layout.ts` | each under 1000 lines |
| `git diff --name-only $BASE` | only this plan's writePaths; no `.rs`, `Cargo.toml` or `Cargo.lock` |

Negative controls (reported separately, never gating; logs in `tmp/canvas-cutover/framecost/`):

1. Run `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm VACTR_REQUIRE_SESSION_ABI=1 npm run build`.
   Then run `cd editor && node test/e2e/run.mjs --browser chromium --profile behavior --write-evidence --run-id s277-refusal --out ../tmp/canvas-cutover/framecost/refusal`.
   Expect exit 2, a result JSON with `wasm.profile: "debug"` and the refusal reason, and no
   file under `tmp/canvas-cutover/framecost/refusal`. No browser is launched, so this runs in
   the sandbox.
2. Afterwards, rebuild the release dist with `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`.
   Re-run the gating `inspectWasm` command (exit 0) so `editor/dist` is left in the release
   state.
3. Mutation: with the `isTexture` call or the per-frame `getError` restored, the new gpu
   animation-only test fails. With `boundary` reverted to the per-call segmenter, the
   segmentation assertions fail.

Outside the sandbox (verification step):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run test:perf` (alone on the host) | exit 0 |

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/framecost/intent.json` and
`receipt.json`. If a file drifted from its fresh read without an edit from this plan, stop
editing it and report. Edit only this plan's progress log.

## Completion Criteria

- [x] No `isTexture` call remains in `renderer.ts`; commands carry a generation that is bumped on loss, restore, font change, DPR invalidation, eviction and dispose
- [x] No per-frame `getError`; init, resize, backdrop and atlas upload checks remain
- [x] `TextLayout` caches multi-unit grapheme clusters per shaped line (capped, byte-accounted); `boundary` is equivalent to the old loop for every position and bias
- [x] Per animation-only frame in `mount.test.ts`: exactly 1 `session_frame`, 0 `session_check`, no ABI argument over 64 KiB, 0 `isTexture`, 0 `getError`, 0 segmenter calls
- [x] `mise run build-wasm-release` exists; the vite default is the release artifact; the debug override works through `VACTR_WASM`
- [x] `wasm-profile.mjs` and its tests pass; `run.mjs` records the wasm block and refuses non-release gating runs with exit 2 and no writes
- [x] README updated; prior assertions retained; default vitest, `npm run check` and `node --check` pass
- [x] `npm run test:perf` passes outside the sandbox; sensitivity is shown by in-test controls (session 283)
- [x] Progress log updated

## Progress Log

### Session: 2026-10-05 (session 277 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.13 D and E. No source edits.

### Session: 2026-10-06 (session 282 plan revision)
**Tasks Completed**: D1 changed from `CARGO_PROFILE_RELEASE_STRIP=none` to
`CARGO_PROFILE_RELEASE_STRIP=debuginfo` to match the session-282 correction of design
15.3.8.13 D. Added the artifact-contract pitfall. No other contract, writePath or test row
changed. Dispatch remains wave 7, after CANVAS-EVIDENCE-SCHED is accepted.

### Session: 2026-10-06 (session 283 amendment: in-test controls replace negative controls)
**Hard rule (workflowInput, session 283)**: run none of the negative controls 1-3 in the
Verification section. Do not run the debug-build refusal command (expected exit 2) or any
mutation. Structured `verification[]` and `priorVerification[]` list only final-source
commands that exited 0 with positive test counts. If a gate fails, fix it inside the
writePaths, rerun it, and report only the final passing run.

Replacement sensitivity controls, each with both branches asserted inside one passing test:
- **Debug refusal (replaces control 1)**: in `editor/test/e2e/wasm-profile.test.ts`, one
  `it` block builds tiny synthetic modules in a temp dir:
  - a module with a `name` section and no `.debug_*` sections, whose sha256 matches the
    passed `releasePath` -> `inspectWasm` gives `profile: "release"` and
    `gatingRefusal(...) === null`;
  - a module with a `.debug_info` section, or whose sha256 matches `debugPath` ->
    `profile: "debug"` and a non-null refusal reason;
  - a module without a `name` section -> a non-null refusal.

  `run.mjs` has no dist-path flag today (`editor/test/e2e/run.mjs:12-20`), and none is added.
  Instead, `wasm-profile.mjs` exports
  `gatingPreflight({ distWasm, releasePath, debugPath, writeEvidence }) -> Promise<{ wasm, refusal: string | null }>`.
  `run.mjs` calls it before creating the out directory, starting the server or launching a
  browser. When `refusal` is non-null it prints the reason and exits 2.

  One `it` block calls `gatingPreflight` on temp-dir modules and covers four cases:
  - release with `writeEvidence: true` -> `refusal === null`;
  - debug with `writeEvidence: true` -> non-null refusal;
  - nameless with `writeEvidence: true` -> non-null refusal;
  - debug with `writeEvidence: false` -> `refusal === null` and `wasm.gating === false`.

  The reviewer confirms by reading `run.mjs` that the preflight call precedes every write,
  server and browser. `editor/dist` and the committed evidence are never touched by this
  test.
- **Restoring the release dist (replaces control 2)**: not needed, because the test uses temp
  dirs. The gating sequence `mise run build-wasm-release` -> release `npm run build` ->
  `inspectWasm` stays.
- **Counters (replaces control 3)**: the gpu.test.ts and mount.test.ts animation-only tests
  also assert, in the same `it` block, that the counting fakes are live. A text-dirty or
  restore frame before the animation-only frames records `getError >= 1` (init or upload
  path). A first shaping of a non-ASCII line records `segmentations >= 1`. A stale-generation
  command after loss and restore is skipped (0 draws with a deleted texture). The
  animation-only frames that follow record 0 `isTexture`, 0 `getError` and 0 segmentations.
  `isTexture` liveness: the counting proxy is asserted to count one direct probe call made
  by the test itself.

Tick the criterion "negative controls recorded separately" as "Sensitivity shown by in-test
controls (session 283)". Contracts D and E, writePaths, the gating commands and the no-Rust
rule are unchanged.

### Session: 2026-10-06 (CANVAS-EVIDENCE-FRAMECOST implementation)
**Tasks Completed**: E1/E2 and D1/D2 implemented. Textured draw commands now carry the
renderer generation and stale generations are skipped; the per-frame `getError` and
`isTexture` probes are removed. Shaped layout cache entries retain bounded multi-unit
grapheme pairs with byte accounting, and animated ranges snap only endpoints within the
visible span. The release wasm task, release Vite default, provenance parser, gating
preflight, refusal path, tests and README are in place. No Rust, Cargo manifest, lockfile or
dependency changes.

**Verification** (final-source logs under `tmp/canvas-cutover/framecost/`):
- Focused FRAMECOST tests: `focused-final.log`, 61/61.
- Scoped Vitest: `scoped-vitest-final.log`, 350/350.
- Full default Vitest: `full-vitest-final.log`, 720/720.
- `npm run check`: `npm-check-final.log`, exit 0.
- Node syntax checks: `node-check.log`, exit 0.
- `CARGO_TERM_QUIET=true mise run build-wasm-release`: `build-wasm-release.log`, exit 0.
- `VACTR_REQUIRE_SESSION_ABI=1 npm run build`: `release-vite-build.log`, exit 0.
- Release inspection: `wasm-dist-inspection.log`, 6,451,635 bytes,
  sha256 `17c8630bb5d3f3acf90ea73b49ed475f1a497f3f1a363732e4136ccedf743a2d`,
  profile `release`, name section present, DWARF absent.
- Serial `npm run test:perf`: `test-perf-final.log`, 1/1, ratio 2.89.
- Renderer/layout line counts: 402 and 221. `git diff --check` passed; no Rust or Cargo
  manifest/lockfile diff. Other worktree changes belong to the accepted SCHED and SCOPE
  predecessors and were preserved.

**Sensitivity Controls**: The passing GPU and mount tests prove counter liveness with explicit
test probes, then assert zero `isTexture`, `getError`, and segmenter calls across animation-only
frames. The wasm preflight test covers release acceptance and debug, nameless, and non-gating
branches using temporary artifacts. Per session-283 instructions, no separate mutation or
negative-control command was run.
