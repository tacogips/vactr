# Canvas browser evidence harness

## Examples scenario

Run `cd editor && npm run e2e:examples` after building `editor/dist` with the
debug wasm artifact:

```sh
cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build
cd editor && npm run e2e:examples
```

The Chromium-only scenario checks first-open showcase behavior, moderate
playback, keyboard example selection with undo/redo, draft restoration, the
saved-document library, default-template persistence/reset, draft recovery
under an explicit example URL, and startup when localStorage is unavailable.
Each browser context installs the silent sink before navigation. Playback
must remain at or below -1 dBFS before the sink, with post-sink peak exactly
zero and no direct destination connections. The runner prints and writes its
eight-check JSON result to `tmp/examples-e2e/result.json`; the complete command
log belongs in `tmp/examples/EX-70/e2e-examples.log`. Exit 0 means all checks
passed, exit 1 means one or more checks failed, and exit 2 means the browser or
built distribution is unavailable.

This is a plain Node.js Playwright-library harness. It does not use `@playwright/test`.

```sh
# Required gating artifact: release wasm with the name section and no DWARF.
mise run build-wasm-release
cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build
cd editor && npm run e2e -- --browser all --profile all --write-evidence
cd editor && ./node_modules/.bin/vitest run test/e2e
node editor/test/e2e/ios-sim.mjs --app <path-to-ios-app> --device "iPad Pro 11-inch (M5)"
```

`--browser` accepts `chromium`, `webkit`, or `all`; `--profile` accepts `behavior`, `measure`, or `all`. `--run-id` selects the evidence run directory (default `run-001`). Chromium starts with `--mute-audio` and hardware-GL flags. Before each page is created, the harness routes realtime audio through an analyser tap, a zero-gain node, and a post-sink analyser. Reports include pre-sink peak and RMS dBFS, onset counts/times, direct connection count, and post-sink peak; a run passes the audio gate only when post-sink peak is exactly zero and direct connections are zero. Automated runs never change system volume or use a system audio driver. `--headed-webkit` retries WebKit with a visible window. Exit 0 means every gated check passed, 1 means a check/threshold failed, and 2 means the run was blocked. A blocked result is never a pass.

`--write-evidence` writes environment, browser behavior, raw measurement JSONL, summary and harness-generated result rows beneath `design-docs/specs/evidence/canvas-cutover/run-001/`. Full stdout/stderr logs belong beneath `tmp/canvas-cutover/evidence/`. The iPad simulator record includes its self-check line and screenshot when available; it records an exact blocker when the app or simulator is unavailable.

Each injected 250 ms main-thread stall records its `performance.now()` start and end. Sync samples are classified from onset page time and the first frame at or after the recorded start. Early flashes are gated in the audio domain: a frame-range pair is early only when its sampled audible time precedes its onset by more than one 128-frame render quantum (`128 / AudioContext.sampleRate`). The page-time proxy remains informational. Stall-window samples are gated by active-set recovery, no replayed or audio-domain early highlights, and beat residuals; remaining sync samples retain the existing p95/p99 limits. The classification and recovery rules are defined in design 15.3.8.15.

Gating evidence runs (`--write-evidence`) require `editor/dist/vactr.wasm` to match the release artifact produced by `mise run build-wasm-release`, include the `name` custom section, and contain no DWARF sections. The harness records the wasm path, byte size, SHA-256, profile, reference hashes, `nameSection`, `dwarf`, and whether the run is gating in both the environment and summary. A non-release, nameless, or DWARF-bearing wasm is refused with exit 2 before output files are created and before a server or browser is started. Non-gating runs record `gating: false`. For a local debug build, set `VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm` explicitly; Vitest continues to use its debug wasm independently.

For canonical session-303 measurements, wait until the 1-minute load average is below 9 and at most two build/test processes are using 50% CPU or more. Acquire `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock` only after the host is quiet, then re-check the same conditions while holding the lock. The runner independently enforces this precondition for `--write-evidence` and records `hostLoad` in `summary.json` and each browser result.

```sh
until node --input-type=module -e 'import os from "node:os"; import {execFileSync} from "node:child_process"; import {parseBusyProcesses,quietHostGate} from "./editor/test/e2e/stats.mjs"; const ps=execFileSync("ps",["-Ao","pid,ppid,pcpu,comm"],{encoding:"utf8"}); process.exit(quietHostGate({load1:os.loadavg()[0],busy:parseBusyProcesses(ps,[process.pid])}).quiet?0:1)'; do sleep 10; done
until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done
printf '%s\n' 'CANVAS-EVIDENCE' > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner
trap 'rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock' EXIT
until node --input-type=module -e 'import os from "node:os"; import {execFileSync} from "node:child_process"; import {parseBusyProcesses,quietHostGate} from "./editor/test/e2e/stats.mjs"; const ps=execFileSync("ps",["-Ao","pid,ppid,pcpu,comm"],{encoding:"utf8"}); process.exit(quietHostGate({load1:os.loadavg()[0],busy:parseBusyProcesses(ps,[process.pid])}).quiet?0:1)'; do rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock; sleep 10; until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done; printf '%s\n' 'CANVAS-EVIDENCE' > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; done
cd editor && npm run e2e -- --browser all --profile all --write-evidence --run-id run-001
```

The `EXIT` trap releases the lock after the run. If the in-lock re-check fails, the procedure releases and reacquires the lock after the host quiets; do not wait while holding the lock.

The archived DOM comparison results are in [design-renderer-comparison.md](../../../design-docs/specs/design-renderer-comparison.md); its harness was removed with the DOM backend and is recoverable from commit `56601c2`.

## Code-pane pixel regression

`node test/e2e/code-pixels.mjs --backend metal|swiftshader|all --dpr 1|2|all --out <dir>` compares the canvas code pane with an opaque DOM reference using the same monospace font. It exercises typed `use-bpm 108`, completion-open multiline typing, a three-line insert, and `examples/kalimba.vact`. Per-character ink and width checks cover missing, blank, clipped and heavy glyphs; the harness also checks caret placement, diagnostic underline rows, backdrop uniformity and WebGL errors. Each run verifies the requested renderer and writes `report.json` plus canvas/reference PNG captures.

The CLI starts its own server on `127.0.0.1` port 0 and installs the silent audio sink before each page. The Chromium behavior profile adds `code-pixels-dpr1` and `code-pixels-dpr2` on ANGLE Metal. Run standalone Metal/SwiftShader pixel matrices under `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`; never use ports 5173 or 5174.

## Vim mode checks

The behavior profile runs `vim-toggle`, `vim-dcyp`, `vim-visual`, `vim-dot`,
`vim-ime-normal`, and `vim-shortcut` in their own browser context, isolated from
the existing canvas behavior checks. Run the non-gating checks with:

```sh
cd editor && npm run e2e -- --browser all --profile behavior --run-id vim-001 --out ../tmp/vim-mode/e2e/vim-001
```

The `vim-keymap.mjs` checks run in the behavior profile after the Vim checks:
`vim-keymap-preset`, `vim-keymap-kj`, `vim-keymap-hints`, `vim-keymap-hop`,
and `vim-keymap-hop-off`. Run VK-40 gate 6 under the measurement lock. The
session ABI build must run immediately before e2e because the runner serves
the prebuilt `editor/dist`:

```sh
cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build && npm run e2e -- --browser all --profile behavior --run-id vim-keymap-001 --out ../tmp/vim-keymap/VK-40/e2e/vim-keymap-001
```

## Header and playback controls

After rebuilding the release wasm and editor, run `npm run e2e:header` from
`editor/`. The test uses a silent audio sink and checks actual session playback
in Chromium and WebKit: empty/constant-only documents keep the counter stopped,
Play/Stop and Pause/Resume update real playback, and menus remain usable within
1440, 768 and 390 pixel viewports. Screenshots and results are written to
`tmp/editor-header/` at the repository root.


## Source-comment backgrounds

After rebuilding release wasm and the editor, run `npm run e2e:background`
from `editor/`. This requires `ffmpeg` on PATH to generate a temporary silent
H.264 fixture. The test serves proper media MIME types and byte ranges, then
checks actual image compositing, real master-waveform scope pixels, all four
overlays, muted/looping video playback, undo/removal, invalid URLs, and media
load failures in Chromium and WebKit. The audio sink is silent. Evidence is
written to `tmp/editor-background/`; the temporary video is removed after the
run. User syntax and the bundled example are documented in the root README.
