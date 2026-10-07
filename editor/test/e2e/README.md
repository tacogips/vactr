# Canvas browser evidence harness

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
