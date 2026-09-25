# Vactrol Back End: Wasm Host, Worklet Glue, Dev Harness (BE-WASM) Implementation Plan

**planId**: BE-WASM (vactrol-core.md TASK-008 `WasmHost`, `editor/worklet/` glue, worklet-as-timebase, latency widen, the 16.1 lifecycle, `editor/dev-harness/` and its real-worklet checks)
**Status**: Completed (accepted by integration review; reconciled by BE-FINAL session 186; archive after the workflow commit)
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

- [x] Raw ABI, `WasmAudioHost`, worklet half, worklet glue, worklet-as-timebase, latency widen implemented as listed
  (session 182: `src/host/wasm/{abi,messages,cells,main_half,worklet_half}.rs`, `editor/worklet/{host,processor}.js`)
- [x] Dev harness and headless runner implemented; W1 report cited (`target/fe-logs/be-wasm-harness-s182-1.json`)
- [x] Criterion 8 proven by W1 (all criterion-8 check ids pass), or recorded as BLOCKED with the operator step (B1)
  (W1 exit=0: the 10 criterion-8 ids pass in headless Chrome 154)
- [x] Criterion 11 proven by W1 (all criterion-11 check ids pass), or recorded as BLOCKED with the operator step (B1)
  (W1 exit=0: the 7 lifecycle ids and `memory-stable` pass)
- [x] Criterion 9 (`--features host-wasm` wasm32 build) passes (V6b) (`be-wasm-wasm32-hostwasm-s182-1.log` exit=0)
- [x] V1-V8, W1-W3 recorded with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-WASM implementer)` entry: work done, harness report path and
per-check results, blocked status if any, design differences, evidence per row. Edit only this log.)

### Session: 2026-09-25 (session 182, BE-WASM implementer)

**Admission**: `dependsOn` BE-SCHED, BE-DSP, BE-INST are all in the fanout item's `acceptedPlanIds`.

**Work done** (all files new except the CONTRACTS stub `src/host/wasm/mod.rs`; no file outside writePaths edited):
- `src/host/wasm/abi.rs` (134 lines): `alloc`/`free`, the per-instance framed outbox (`[u32 LE len][record]`,
  `outbox_ptr/len/clear`; the worklet fixes its capacity at init and counts refused records), extension tags
  `TAG_CONSOLE` 0x70 (main -> page), `TAG_FAULT` 0x60 and `TAG_SIGS` 0x61 (worklet -> main).
- `src/host/wasm/messages.rs` (408): `WasmAudioHost: AudioHost` (records into the outbox; 16.1 sender-paced window:
  arena admission first with an immediate `arena-exhausted` diagnostic, `SampleBegin`, then ONE slice in flight, the
  next posted only on its `SliceOk`; graphs via `dsp::arena::encode_inst/encode_bus` + `ring::encode_graph_record`
  with graph resource ids from `0x4000_0000`; the engine's cumulative `Counters` become deltas so `Runtime::widen`
  counts each late event once = latency widen; `HostSigs` from the worklet feed `analysis()`), `WasmCellPort:
  CellPort`, `WasmSamples: SampleLoader` (keys `bank:index` or path text).
- `src/host/wasm/cells.rs` (134): `ProbeMirror`, the worklet `CellStore` over `dsp::cells::Mirror` with fixed-size read
  probes (reads, uninitialized reads, last read, a 16-entry read log); no allocation after construction.
- `src/host/wasm/main_half.rs` (540): `main_init`, `eval`, `tick`, `inbox`, `sample_put`, `main_resync`
  (`Runtime::resync_cells` on port re-establishment), observation exports (`main_stat`, `main_sent`,
  `main_retired`, `main_probe_ctl`, `inst_id`, `ctl_id`, `sample_id`) and the `harness_*` record hooks (they only
  encode wire records into the outbox; JS moves them over the real port). Registry caps set to the browser preset.
- `src/host/wasm/worklet_half.rs` (474): `worklet_init` (allocates engine + arena, `ByteInbox`, rings, staging, outbox,
  report; records the memory size), `staging_ptr`, `worklet_inbox`, `process` (planar output), `worklet_now`,
  `report_ptr/len` (`REPORT_LEN` = 162 `f64`, indices `R_*`), `worklet_probe_cell/resource`, `worklet_reset_watch`.
- `editor/worklet/processor.js`: O(1) `onmessage` (buffer reference into a 64-slot preallocated array, overflow drop +
  count), wasm #2 instantiated from the posted copy, silence until ready, per-quantum drain -> `worklet_inbox` ->
  `process` -> outputs, posts `{t: worklet_now(), o: outbox, js, r every 4 quanta}`.
- `editor/worklet/host.js`: fetch + instantiate wasm #1, AudioContext + AudioWorkletNode, posts a COPY of the bytes,
  `inbox` + `tick` (every 5 ms of worklet time) + outbox flush on every worklet message, console routing,
  `decodeAudioData` sample loading, and the harness `filter`/`release`/`postRaw`/`workletCall` hooks.
- `editor/dev-harness/{index.html,harness.js,run-headless.mjs,README.md}`: the 18 checks below; the runner uses node
  built-ins only and exits 0 / 1 / 2 (BLOCKED) as specified.

**W1 report** `target/fe-logs/be-wasm-harness-s182-1.json` (log `be-wasm-harness-s182-1.log`, `exit=0`, HeadlessChrome
154, 48 kHz, module served from `target/wasm32-unknown-unknown/debug/deps/vactrol.wasm`, see difference 7):
criterion 8: `cell-first-before-ack` PASS (5 `Const(0.5)` events and 0 mirror reads while `CellInit` was held; after the
ack reads 0.5, 0 uninitialized), `cell-batch-next-voice` PASS (0.50,0.50 before; 0.80 x4 after), `cell-delayed-hop`
PASS (0.80 x4 inside the hop, 0.30 x3 after), `cell-init-replay` PASS, `cell-reuse` PASS (Live 1 -> Vacant with
`CellRetired` -> Live 2), `cell-stale-epoch` PASS, `cell-stall-burst` PASS (in flight <= 1, pending <= 1, 1 distinct
seq in 7 held batch records, converged to 0.46), `cell-reconnect` PASS (0.90 x4 after the commit lead),
`release-after-genbump` PASS, `release-tombstone` PASS (dropped 0 -> 1).
criterion 11: `load-during-playback` PASS (6 slices, 0 silent quanta, 0 frame gaps), `graph-replace-during-playback`
PASS, `arena-exhausted` PASS (main admission diagnostic + worklet `ArenaExhausted` fault), `graph-too-large` PASS,
`deferred-queue-overflow` PASS (3 faults, `install-queue-overflow` diagnostic), `unload-while-playing` PASS (Retiring
while the voice played, `Retired` 11 ms after the voice was last seen), `install-burst-credit` PASS (4 x 512 KB, max
65536 bytes copied per `process()`, 1 `SliceOk` withheld to a later quantum), `memory-stable` PASS (50331648 bytes
at init and at the end). Dev iterations: `tmp/be-backend-20260925-s181/BE-WASM/attempt-1/dev-run-{1,2,3}.{log,json}`.

**Design differences** (signatures only; behavior as planned):
1. `main_init(sample_rate, arena_bytes)`: the arena size drives the 16.1 admission check on the main side.
2. `sample_put(key_ptr, key_len, data_ptr, len, rate, channels)`: keyed by `bank:index` or path text (the loader's
   `SampleSrc`), data read as little-endian bytes (no alignment requirement).
3. `worklet_inbox(len)` reads the record from the preallocated `staging_ptr()` buffer and returns 0 when the inbox is
   full (JS keeps the record and the order); `inbox(ptr, len)` takes the worklet's framed records of one quantum.
4. The posted frame time is `worklet_now()` (the engine's rendered-frames clock), not raw `currentFrame / sampleRate`,
   which also counts the silent quanta before wasm #2 was ready; both advance together afterwards.
5. JS -> wasm is one staging copy per record in `process()`; the engine's arena copy is the credit-governed one
   (`R_BYTES_MAX` <= 65536). The staging copy is reported separately (`R_STAGED_MAX` 65571 = one slice + the forced
   graph record of the burst check); with the one-slice window it is one slice per round trip.
6. `unload-while-playing` holds the sample through a live `analog` voice whose event carries `bank` (the engine counts
   `Voice::bank` as a user), because the prelude `sampler` template does not install (finding 1). `graph-too-large` uses
   a balanced 10 x 15 graph (finding 3).
7. The runner serves the module that exports the ABI: after a full `cargo build`, `debug/vactrol.wasm` is the `vactrol`
   bin (output filename collision, finding 2) and `debug/deps/vactrol.wasm` is the cdylib.

**Findings for BE-FINAL (outside BE-WASM writePaths; not fixed here)**:
1. The prelude templates `sampler`, `wavetable` and `granular` do not install on the audio side: `Template::build`
   returns `BadEdge` for the `InstDef` that `dsp::build::lower_inst` produces (node probe in
   `attempt-1/notes.md`; the worklet reports them as `GraphTooLarge` because `Templates::build` maps every build error
   to that code). The native tier builds templates with the same `Template::build`. Owners BE-INST/BE-DSP.
2. `cargo build --target wasm32-unknown-unknown` warns `output filename collision` (bin and cdylib both `vactrol.wasm`);
   the uplifted file is the bin. `Cargo.toml` (CONTRACTS-owned) should rename one target.
3. A ~130-term `+` chain in an `inst` body overflows the Chrome main-thread stack in the debug wasm build (recursion in
   `dsp::build::Graph::node/input`, `dev-run-1.log`); a trap leaves the main half's `RefCell` borrowed. Owner BE-INST.

**Evidence** (`target/fe-logs/`, all `exit=0`): V1 `be-wasm-build-s182-1.log`; V2 `be-wasm-clippy-s182-1.log`; W4
`be-wasm-clippy-wasm-s182-1.log`; V3 `be-wasm-nextest-s182-1.log` (747 run, 747 passed, 1 skipped); V3t
`be-wasm-cargotest-s182-1.log` (lib 737 passed; spec_fixtures 10 passed, 1 ignored; 0 failed); V3f
`be-wasm-fixtures-s182-1.log` (10 run, 10 passed); V6a `be-wasm-wasm32-s182-1.log`; V6b
`be-wasm-wasm32-hostwasm-s182-1.log`; W3 `be-wasm-w3-s182-1.log`; W2 `be-wasm-w2-s182-1.log`; W1
`be-wasm-harness-s182-1.{log,json}`; V4 `be-wasm-linecount-s182-1.log` (largest `.rs` 786, BE-WASM largest
`main_half.rs` 540); V5 `be-wasm-stdgrep-s182-1.log` (`none`); V8 `be-wasm-fmt-owned-s182-1.log`; read-only
`cargo fmt --check` `be-wasm-fmtcheck-s182-1.log`. Hashes: `tmp/be-backend-20260925-s181/BE-WASM/attempt-1/final-hashes.txt`.

**Downstream**: formal test-integrity/adversarial review, the core-plan checkboxes and README (BE-FINAL), the commit.

### Session: 2026-09-25 (session 183, BE-WASM rerun)

**Reason**: attempt-1 failed only in the review output contract (`$.findings[0].intentIncerence`, see the operator
note below), not in the implementation. Admission is unchanged: BE-SCHED, BE-DSP and BE-INST (and BE-MIDI) are in `acceptedPlanIds`.

**Work done**: no source change. All 12 BE-WASM source/JS files match `attempt-1/final-hashes.txt`
(the only mismatch is this plan file, because of the operator note). The only edit is this entry. Intent is recorded in `tmp/be-backend-20260925-s181/BE-WASM/attempt-2/intent.md`.

**Evidence on the current shared tree** (`target/fe-logs/`, all `exit=0`): V1 `be-wasm-build-s183-1.log`; V2
`be-wasm-clippy-s183-1.log`; W4 `be-wasm-clippy-wasm-s183-1.log`; V8 `be-wasm-fmtcheck-s183-1.log` (`cargo fmt --check`);
V3 `be-wasm-nextest-s183-1.log` (747 run, 747 passed, 1 skipped); V3t `be-wasm-cargotest-s183-1.log` (lib 737 passed;
spec_fixtures 10 passed, 1 ignored; 0 failed); V3f `be-wasm-fixtures-s183-1.log` (10 run, 10 passed); V6a
`be-wasm-wasm32-s183-1.log`; V6b `be-wasm-wasm32-hostwasm-s183-1.log`; W3 `be-wasm-w3-s183-1.log`; W2
`be-wasm-w2-s183-1.log`; W1 `be-wasm-harness-s183-1.log` + report `be-wasm-harness-s182-2.json` (the runner's report
name keeps the s182 prefix). All 18 checks PASS in HeadlessChrome 154, `memoryStable=true`, harness `exit=0`. V4
`be-wasm-linecount-s183-1.log` (largest `.rs` 786); V5 `be-wasm-stdgrep-s183-1.log` (`none`).

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-008)
- **Previous**: vactrol-backend-sched.md, vactrol-backend-dsp.md, vactrol-backend-inst.md
- **Next**: vactrol-backend-finalize.md

### FINDING KEY NOTE (operator, 2026-09-25, after the BE-NATIVE and BE-WASM attempt-1 failures)

- The step6-test-integrity-check and step7-adversarial-review output contracts
  reject unknown keys INSIDE each `findings[]` item. BE-NATIVE failed with
  `$.findings[0].intentRef additional property is not allowed` and BE-WASM with
  `$.findings[0].intentIncerence ...`: both were misspellings of the accepted key
  `intentReference`. Use ONLY the keys the riela contract defines for a finding item:
  `findingId`, `severity`, `category`, `file`, `line`, `message`, `evidence`
  (confirmed against the riela binary); put anything else, such as an intent
  reference or a fix-cost note, inside the `message` or `evidence` text. `findings` must
  be present (empty array when none) and the outputs must not carry `planId`.

### Closing note (BE-FINAL, session 186)

Accepted by the integration review (acceptedPlanIds) and reconciled by BE-FINAL on the joined tree: every final-tree gate exits 0 (`target/fe-logs/be-final-<check>-s186-1.log`), and the TASK-007/008 checkboxes in vactrol-core.md cite this plan's tests. The F8 final-tree harness re-run passed 18/18 (`target/fe-logs/be-final-harness-s186-1.json`). Archive to impl-plans/completed/ in the separate docs commit after the workflow commit.
