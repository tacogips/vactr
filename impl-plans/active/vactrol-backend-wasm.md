# Vactrol Back End: Wasm Host, Worklet Glue, Dev Harness (BE-WASM) Implementation Plan

**planId**: BE-WASM (vactrol-core.md TASK-008 `WasmHost`, `editor/worklet/` glue, worklet-as-timebase, latency widen, the 16.1 lifecycle, `editor/dev-harness/` and its real-worklet checks)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 12.8.10 (raw ABI, worklet glue), 12.8.11 (harness, headless runner, evidence, blocked-not-passing), 16, 16.1, 11.3 (browser cell protocol), 11.7 (VoiceRelease), 12.8.4 (latency widen); design-docs/user-qa/pending-backend-questions.md B1, B5
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
**dependsOn**: BE-SCHED, BE-DSP, BE-INST
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json

---

## Intent and Context

The browser tier runs two instances of one `wasm32-unknown-unknown` module: the main-thread instance (evaluator +
scheduler) and the worklet instance (DSP only), with no shared memory (16). BE-CONTRACTS declared `host/wasm` under
`all(target_arch = "wasm32", feature = "host-wasm")` and set `crate-type = ["rlib", "cdylib"]`. This plan implements the
raw `extern "C"` ABI (no wasm-bindgen, 12.8.10), a `WasmAudioHost: AudioHost` on the main half that encodes records into
an outbox (and runs the 16.1 sender-paced slice window for `install_sample`), the worklet half around `dsp::Engine` +
`Mirror` + `SampleArena`, the plain-JS worklet glue, and the standalone dev harness with its node headless runner. The
harness is the ONLY accepted proof for TASK-008 criteria 8 (cell transport in a real worklet) and 11 (browser lifecycle);
headless `Engine::process` tests never substitute (16.1).

## Non-Goals

- No TypeScript, no bundler, no npm dependency, no browser-automation library (node built-ins only).
- No WebMIDI, no WebGL `RenderHost`, no editor code (TASK-010). The browser `CapabilitySet` keeps `midi_in/out = false`.
- No change to scheduler or DSP semantics; a defect found there is recorded for BE-FINAL's serial repair.

## writePaths (exclusive)

- `src/host/wasm/mod.rs` and new files under `src/host/wasm/` (`abi.rs`, `main_half.rs`, `worklet_half.rs`)
- `editor/worklet/processor.js`, `editor/worklet/host.js`
- `editor/dev-harness/index.html`, `editor/dev-harness/harness.js`, `editor/dev-harness/run-headless.mjs`,
  `editor/dev-harness/README.md`
- `impl-plans/active/vactrol-backend-wasm.md`

## sharedPaths

None.

## File-Level Changes (signatures only; every export is `#[no_mangle] pub extern "C"`)

1. `abi.rs`: `alloc(len: u32) -> *mut u8`, `free(ptr, len)`; a single-threaded state cell per half
   (`thread_local!` + `RefCell<Option<..>>`, no `static mut`); an outbox byte buffer of encoded records with
   `outbox_ptr() -> *const u8`, `outbox_len() -> u32`, `outbox_clear()`.
2. `main_half.rs`: `main_init(sample_rate: f32) -> u32` (builds `Evaluator` + `InstRegistry` + `Runtime` with
   `Tier::Browser`, `CapabilitySet::browser()`, `WasmAudioHost`, noop MIDI/OSC/render hosts, and a `SampleLoader` fed by
   `sample_put(id, ptr, len, rate, channels)` from JS-decoded audio); `eval(ptr, len) -> u32` (diagnostic count; the text
   of diagnostics goes to the outbox as a `Console` record), `tick(now: f64)`, `inbox(ptr, len)` (a `HostMsg` from the
   worklet). `WasmAudioHost::now()` is the last `tick` time; late counters from `Counters` feed `Runtime` auto-widen.
3. `worklet_half.rs`: `worklet_init(sample_rate: f32, arena_bytes: u32, voices: u32) -> u32` (allocates everything;
   records the memory size at the end of init); `worklet_inbox(ptr, len)` (queues one record for the next `process`);
   `process(frames: u32) -> *const f32` (applies queued records under the 16.1 credit, then `Engine::process`);
   `report_ptr()`/`report_len()` (counters: late, dropped, stolen, bytes copied per quantum max/total, deferred queue
   length, last value read at voice start per probed cell, active voices, retired resources, memory size at init).
4. `editor/worklet/host.js` (main thread): fetch the module bytes, instantiate wasm #1, create the `AudioContext` and
   `AudioWorkletNode`, post a COPY of the bytes to the worklet, call `tick` on every worklet frame-time message, and post
   each outbox record as a transferred `ArrayBuffer`; decode samples with `decodeAudioData` and hand them to
   `sample_put`. `editor/worklet/processor.js`: instantiate wasm #2 from the posted bytes during init and render silence
   until ready; `port.onmessage` stores the buffer in a preallocated fixed-size slot array (overflow -> drop + count, O(1));
   `process()` drains the slots into `worklet_inbox` (all copying happens inside the credit), calls `process`, copies the
   returned block to the outputs, and posts `currentFrame / sampleRate` every render quantum.
5. `editor/dev-harness/`: `index.html` + `harness.js` load the host-wasm build and the glue, and run the checks below.
   JS sits between the two halves, so it can hold, delay, drop, duplicate or reorder records for fault injection. The page
   POSTs a JSON report `{checks: [{id, pass, detail}], memoryStable, userAgent}`. `run-headless.mjs` (node built-ins
   `http`, `child_process`, `fs`, `os`, `path`) serves the repo root on `127.0.0.1:<free port>`, launches `$CHROME` or
   `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome` with `--headless=new
   --autoplay-policy=no-user-gesture-required --no-first-run --user-data-dir=<tmp>`, writes the report to
   `target/fe-logs/be-wasm-harness-s<S>-<n>.json`, and exits 0 when all checks pass, 1 on a failed check or a missing
   `target/wasm32-unknown-unknown/debug/vactrol.wasm`, and 2 when blocked (no Chrome, or the `AudioContext` never reaches
   `running`), with a 120 s timeout; `--headed` runs a visible browser for the operator path (B1). `README.md` says how to
   run both modes.

## Harness Checks (each one `{id, pass}`; ids are cited in the progress log)

TASK-008 criterion 8 (real worklet): `cell-batch-next-voice` (a batch applied between quanta changes the next voice
start's value); `cell-delayed-hop` (a held batch leaves the earlier value for voices inside the hop); `cell-first-before-ack`
(first event before `CellInitAck` sounds its `Const` value, never an uninitialized read); `cell-init-replay` (replayed
`CellInit` after a newer batch keeps the newer value); `cell-reuse` (a reused id goes Vacant -> Live); `cell-stall-burst`
(stalled worklet: at most one in-flight + one pending batch, converges on resume); `cell-stale-epoch` (retired-epoch
update after reuse is inert); `cell-reconnect` (snapshot completes before new voice starts read the mirror);
`release-after-genbump` (`VoiceRelease` releases exactly its tagged voice after a slot-gen bump); `release-tombstone`
(forced release-before-start hits the tombstone).
TASK-008 criterion 11 (16.1 lifecycle): `load-during-playback` and `graph-replace-during-playback` (no dropout: no
underrun counter increase and a sustained tone's block RMS never drops to zero across the swap); `arena-exhausted`,
`graph-too-large`, `deferred-queue-overflow` (diagnostics reported); `unload-while-playing` (storage held until queued
events and voices release it, then `Retired`); `install-burst-credit` (max bytes copied per `process()` across the
whole burst <= `INSTALL_BYTES_PER_QUANTUM`, and `SliceOk` withheld until the deferred copy ran); `memory-stable` (worklet
memory size unchanged from end of init to end of run).

## Required Tests (Rust, host target)

The ABI and `WasmAudioHost` logic are target-gated, so Rust unit tests cover what is target-independent through the
existing crates: the outbox encoding path uses `host/wire.rs` (already tested by BE-CONTRACTS). No additional Rust test
is required; the proof for this plan is the harness report plus both wasm32 builds.

## Invariants

- The core needs no JS imports; the module builds with `--no-default-features --features host-wasm` and with default
  features (where `host/wasm` is compiled out).
- The worklet message handler is O(1); all copying happens in `process()` under the credit; no allocation after
  `worklet_init` (the harness `memory-stable` check).
- A blocked harness run is never reported as passing (12.8.11).

## Edit Protocol

Common protocol of `vactrol-backend-contracts.md`; evidence under `tmp/be-backend-20260925-s181/BE-WASM/attempt-<n>/`.
Runs concurrently with BE-MIDI and BE-NATIVE on disjoint files. The JS files are hashed like `.rs` files.

## Verification

Common table with `<wave>` = `wasm`: V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8. Plus:
- W1: after V6b, `node editor/dev-harness/run-headless.mjs`, run as `(set -o pipefail; node editor/dev-harness/run-headless.mjs 2>&1 | tee target/fe-logs/be-wasm-harness-s<S>-<n>.log); echo "exit=$?" >> target/fe-logs/be-wasm-harness-s<S>-<n>.log`.
  `exit=0` with every check id passing in the JSON report = criteria 8 and 11 proven. `exit=2` = BLOCKED: record it,
  leave criteria 8 and 11 unchecked, and state that the operator must run `node editor/dev-harness/run-headless.mjs
  --headed` (B1). `exit=1` = failure to fix.
- W2: `node --check editor/dev-harness/run-headless.mjs editor/dev-harness/harness.js editor/worklet/host.js editor/worklet/processor.js`
  -> exit 0 (syntax only).
- W3: `ls -l target/wasm32-unknown-unknown/debug/vactrol.wasm` exists after V6b.
- W4: LOG(`be-wasm-clippy-wasm`): `CARGO_TERM_QUIET=true cargo clippy --target wasm32-unknown-unknown --no-default-features --features host-wasm -- -D warnings`
  -> `exit=0` (host-target clippy never compiles `host/wasm`).

## Completion Criteria (map to vactrol-core.md TASK-008)

- [ ] Raw ABI, `WasmAudioHost`, worklet half, worklet glue, worklet-as-timebase, latency widen implemented as listed
- [ ] Dev harness and headless runner implemented; W1 report cited
- [ ] Criterion 8 proven by W1 (all criterion-8 check ids pass), or recorded as BLOCKED with the operator step (B1)
- [ ] Criterion 11 proven by W1 (all criterion-11 check ids pass), or recorded as BLOCKED with the operator step (B1)
- [ ] Criterion 9 (`--features host-wasm` wasm32 build) passes (V6b)
- [ ] V1-V8, W1-W3 recorded with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-WASM implementer)` entry: work done, harness report path and
per-check results, blocked status if any, design differences, evidence per row. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-008)
- **Previous**: vactrol-backend-sched.md, vactrol-backend-dsp.md, vactrol-backend-inst.md
- **Next**: vactrol-backend-finalize.md
