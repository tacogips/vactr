# Vactrol Back End: Native Audio Host and examples/beep.rs (BE-NATIVE) Implementation Plan

**planId**: BE-NATIVE (vactrol-core.md TASK-008 `NativeAudioHost`: cpal callback, timer tick, midir, native `SampleLoader`; `examples/beep.rs`)
**Status**: Completed (accepted by integration review; reconciled by BE-FINAL session 186; archive after the workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 12.8.10 (Cargo, NativeAudioHost, examples/beep.rs), 12.8.1 (session socket moved to TASK-009, B5), 12.8.4, 12.8.5 (native channels), 12.2 (triple buffer, `Arc` samples), 16 (native seams), 17 (SampleLoader directories)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
**dependsOn**: BE-SCHED, BE-DSP, BE-INST
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json

---

## Intent and Context

The native tier is the same core with a cpal output stream instead of the worklet and SPSC rings instead of
`postMessage` (16). This plan implements `host/native/` behind `all(feature = "host-native", not(target_arch = "wasm32"))`
(the module declaration and cfg already exist from BE-CONTRACTS) and fills the `examples/beep.rs` stub. The callback runs
`dsp::Engine::process` with `AtomicCells`; `now()` is frames rendered / sample rate; a timer thread wakes the evaluator
every 5 ms over `std::sync::mpsc`; midir implements `MidiInHost`/`MidiHost`; the native `SampleLoader` reads WAV only.

## Non-Goals

- No session socket and no tungstenite (TASK-009, B5). No UDP OSC host, no CoreAudio-specific code beyond cpal.
- No new DSP; no scheduler changes; no REPL (TASK-009 owns the audible REPL gate).
- The audible check of `beep` is manual and recorded as "pending user confirmation"; it does not block this plan.

## writePaths (exclusive)

- `src/host/native/mod.rs` and new files under `src/host/native/` (`audio.rs`, `midi.rs`, `tick.rs`, `loader.rs`,
  `tests.rs` or `tests/`)
- `examples/beep.rs`
- `impl-plans/active/vactrol-backend-native.md`

## sharedPaths

None.

## File-Level Changes (signatures only)

1. `audio.rs`: `NativeAudioHost::open(cfg: NativeConfig) -> Result<NativeAudioHost, Diagnostic>` (default output device,
   f32 stereo; no device -> `beyond-capability` "not available on this host"); owns the event producer, per-sink control
   producer, ack consumer, `AtomicCells`, frame counter (`AtomicU64`) and the triple-buffer swap slot; the cpal callback
   closure owns `Engine` + consumers and does nothing but `Engine::process` and the frame-counter store; `impl AudioHost`
   (`send` -> ring push, full -> dropped count -> `ring-overflow` reported via `drain`; `control`/`post` -> control ring;
   `drain` -> acks and counters; `now` -> frames / sr; `swap_graph` -> build the native structure and hand it over, drop
   the retired one on the evaluator thread; `analysis` -> amp/fft cells).
2. `tick.rs`: `TickSource::start(period: Duration) -> (TickSource, Receiver<()>)`; the thread only sends wakes; dropping
   `TickSource` stops and joins it.
3. `midi.rs`: `NativeMidiIn` (midir input: first port or `NativeConfig::midi_in_port`; raw bytes -> `MidiInEvent` by a pure
   `parse_midi(bytes, time) -> Option<MidiInEvent>` feeding a bounded queue read by `poll`), `NativeMidiOut` (`impl MidiHost`:
   note on/off, clock, start/stop; `control` sends note-offs for revoked sounding notes per 11.3); absent device ->
   `beyond-capability`.
4. `loader.rs`: `NativeSampleLoader { roots: Vec<PathBuf> }` implementing `SampleLoader` and `ns::load::SourceLoader`
   (file-relative resolution per 6.5.8; paths outside the configured roots are rejected, 17; `SampleSrc::Bank { kw, index }`
   is file `index` (modulo count) of the lexically sorted `*.wav` files in `<root>/<kw>/`); in-house RIFF WAV parser
   (PCM 16/24/32-bit int, 32-bit float, mono/stereo); other formats or malformed files -> `Failure(host-unavailable)` with
   the reason; never panics. A file rate other than the engine's is carried in `SampleData::rate` (playback scales speed).
5. `mod.rs`: re-exports and `NativeHosts::open(cfg) -> Hosts` building the bundle (noop OSC and render hosts).
6. `examples/beep.rs` (12.8.10): builds `Evaluator` + `InstRegistry` + `Runtime` + `NativeHosts`, evaluates
   `s :analog > note [:a4] > once` (or `s {sample PATH} > once` when a WAV path argument is given), loops on tick wakes
   calling `Runtime::tick(host.now())` for two seconds, then exits 0; any setup failure prints the diagnostic and exits 1.

## Required Tests (`src/host/native/tests*`; must not require an audio or MIDI device)

- WAV: 16/24/32-bit int and f32, mono and stereo decode to expected frames; truncated/garbage input is a `Failure`, not a
  panic; a path outside the roots is rejected.
- `parse_midi`: note on/off (velocity 0 = off), CC, clock, start, stop, continue, running status ignored safely.
- `TickSource`: wakes arrive and the thread joins on drop (bounded wait, no sleep-based assertion of exact periods).
- `NativeAudioHost` logic that does not need a device (ring overflow counting, ack draining, `now` from a synthetic
  frame counter) is tested through a device-free constructor used only in tests.

## Invariants

- `std::{thread,time,fs,sync::mpsc}` appear only under `src/host/native/` (V5 excludes it) and `examples/`.
- The callback allocates nothing and takes no lock (it only calls `Engine::process`, already probe-tested by BE-DSP).
- Both wasm32 builds stay green (the module is cfg-gated; no native dependency reaches wasm32).
- No `.rs` file reaches 800 lines.

## Edit Protocol

Common protocol of `vactrol-backend-contracts.md`; evidence under `tmp/be-backend-20260925-s181/BE-NATIVE/attempt-<n>/`.
Runs concurrently with BE-MIDI and BE-WASM on disjoint files.

## Verification

Common table with `<wave>` = `native`: V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8. Plus:
- N1: LOG(`be-native-example`): `CARGO_TERM_QUIET=true cargo build --example beep` -> `exit=0`.
- N2: `CARGO_TERM_QUIET=true cargo tree -e normal --target wasm32-unknown-unknown` -> no `cpal`/`midir` (inline).
- Manual (not a gate): `cargo run --example beep` audible on the native host -> recorded in the progress log as "pending
  user confirmation"; the automated proxy is BE-FINAL's headless render of the same program.

## Completion Criteria (map to vactrol-core.md TASK-008)

- [x] `NativeAudioHost` (cpal callback, timer tick, midir, WAV `SampleLoader`) implemented as listed
- [x] `examples/beep.rs` builds under `host-native` (N1); audible check recorded as pending user confirmation
- [x] V1-V8, N1, N2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-NATIVE implementer)` entry. Edit only this log.)

### Session: 2026-09-25 (session 183, BE-NATIVE implementer)

**Tasks completed**: every File-Level Change 1-6 and the Required Tests. Evidence:
`tmp/be-backend-20260925-s181/BE-NATIVE/attempt-1/` (`intent.md`, `pre-edit-hashes.txt`, `owned-rs.txt`,
`final-hashes.txt`). Only writePaths were edited.

**Work per file**
- `audio.rs` (500 lines): `NativeAudioHost::open(&NativeConfig)` uses the default cpal output device with an f32 config
  (the default one, else an f32 range at the default rate). No device, no f32 format, or a stream that fails to build or
  play gives `beyond-capability` "not available on this host". `AudioSide::render` is the whole callback body: it calls
  `Engine::process` over the preallocated rings and buffers, advances the `FrameClock` (an `AtomicU64` of frames; `now()`
  is frames / sr), and stores `HostSigs` in atomics. Mono devices get the mid signal and wider devices get L/R plus
  silence, through a preallocated stereo scratch. `impl AudioHost` works as follows: `send` pushes to the event ring,
  and a full ring counts a drop and records a `ring-overflow` diagnostic. `control`/`post`/`retire_sample` go on the
  `NativeRecord` control channel. `swap_graph` builds `Template`/`BusTemplate` on the evaluator thread and hands them
  over as `NativeInstall`; a build error is recorded as a `graph-too-large` diagnostic. `install_sample` hands the
  `Arc` over. `drain` returns acks, drops garbage on the evaluator thread, and reports new host-side drops as
  `HostMsg::Counters { dropped }`. `analysis` reads the atomics. `headless()` is the device-free constructor the tests
  use.
- `tick.rs`: `TickSource::start(period) -> (TickSource, Receiver<()>)` runs a named thread with `park_timeout` and a
  `sync_channel(1)` (`try_send`, so wakes coalesce). Drop sets the stop flag, unparks the thread and joins it.
- `midi.rs`: `parse_midi` (pure). `NativeMidiIn` is midir input on the configured port (exact name, else substring)
  or the first port. It stamps events with the audio `FrameClock` and passes them through a bounded channel of 1024
  (full = counted drop) to `poll`. `NativeMidiOut` is a sender thread that owns the midir connection and a
  `MidiOutQueue` (pure, tested). Notes become an on at `time` and an off at `time + dur`, sent when the frame clock
  reaches them. `control` applies 11.3: older-generation note-ons of the slot at or after `effective_time` are dropped
  with their offs, and a `Panic` release pulls the older notes' pending offs forward to `effective_time`. On drop, the
  offs of sounding notes are sent.
- `loader.rs`: `NativeSampleLoader` implements both `SampleLoader` and `SourceLoader`. Clones share the table of
  loaded files. Paths resolve per 6.5.8, then `canonicalize` runs (so symlinks are resolved), and the result must lie
  inside a canonical root. A bank is `<root>/<kw>/`, sorted `*.wav`, `index % count`. `parse_wav` is an in-house RIFF
  parser: PCM 16/24/32, float 32, mono/stereo, `WAVE_FORMAT_EXTENSIBLE`, odd-chunk padding. Any other input is
  `Failure(host-unavailable)` with the reason and never panics. The file's rate is kept in `SampleData::rate`.
- `mod.rs`: `NativeConfig` and `NativeHosts::open(&cfg)`. Audio is required. An absent MIDI in/out becomes a
  diagnostic in `NativeHosts::diags` and falls back to a `NoopHost` sink. OSC and render are `NoopHost`.
- `examples/beep.rs`: `NativeHosts` + `InstRegistry::shared()` + `Runtime` (`Tier::Native` over the host's cells) +
  `Evaluator::with_insts`. It plays `s :analog > note [:a4] > once`, or `s {sample /abs/path.wav} > once` for a WAV
  argument (whose directory becomes a sample root). It ticks on `TickSource` wakes with `clock.now()` for 2 s and
  exits 0. Setup failures print and exit 1.

**Design differences (recorded for BE-FINAL)**
1. `NativeHosts::open` returns `NativeHosts { hosts, cells, clock, loader, diags }`, not a bare `Hosts`. The caller
   needs the shared `AtomicCells` (for `Tier::Native`), the frame clock (for `Runtime::tick`), and the loader (for the
   evaluator's `SourceLoader`).
2. `NativeAudioHost::open` takes `&NativeConfig`. MIDI opens separately (`NativeMidiIn/Out::open(port, clock)`).
3. Graph installs use resource ids from `GRAPH_RESOURCE_BASE = 0x4000_0000` up (gen 1 each), so a graph
   `Installed`/`Retired` ack can never match a `SampleTable` id. Samples keep their table ids.
4. MIDI channels are 1..=16 on both sides (`MidiInEvent::ch`, `MidiEvent::ch`), matching `{midi 1}` and `cc ...
   channel: 1`. The wire nibble is `ch - 1`. BE-MIDI/BE-FINAL should confirm that `sched/midi_in.rs` reads input
   channels the same way.
5. MIDI-out timing follows the audio frame clock, so its resolution is one audio callback.
6. Ack ring 8192, garbage ring 1024, control channel 1024. `drain` empties the garbage ring on every call. This answers
   the BE-DSP repair requests on ack loss and on the `None` garbage ring for the native tier.

**Manual check**: `cargo run --example beep` audible on the native host: **pending user confirmation**. The automated
proxy is BE-FINAL's headless render of the same program.

**Verification (session 183, final stable run on the shared tree after BE-MIDI's test files compiled)**

| Row | Log | Result |
|-----|-----|--------|
| V1 build | be-native-build-s183-3.log | exit=0, no warnings |
| V2 clippy -D warnings | be-native-clippy-s183-3.log | exit=0 |
| V3 nextest | be-native-nextest-s183-4.log | exit=0, 747 run, 747 passed, 1 skipped |
| V3t cargo test | be-native-cargotest-s183-3.log | exit=0, lib 737 passed + spec_fixtures 10 passed (1 ignored) |
| V3f fixtures | be-native-fixtures-s183-4.log | exit=0, 10 run, 10 passed |
| own tests | be-native-own-s183-4.log | exit=0, 36 host::native tests passed |
| V6a wasm32 | be-native-wasm32-s183-2.log | exit=0 |
| V6b wasm32 host-wasm | be-native-wasm32-hostwasm-s183-2.log | exit=0; vactrol.wasm exists |
| N1 example | be-native-example-s183-2.log | exit=0 |
| N2 cargo tree wasm32 | be-native-tree-s183-2.log | exit=0; only `vactrol`, no cpal/midir |
| V4 wc | be-native-wc-s183-2.log | largest 786 (dsp/engine.rs); largest BE-NATIVE file audio.rs ~500 |
| V5 std grep | be-native-stdgrep-s183-2.log | none |
| V8 rustfmt --check (owned) | be-native-fmt-s183-3.log | exit=0 |

Earlier attempts, kept as records:
- `-s183-1` nextest/cargotest/fixtures: exit=101. BE-MIDI's in-progress `src/sched/tests/midi.rs` declared modules
  that did not exist yet.
- `-s183-2` nextest/cargotest: 3 or 4 failures, all in BE-MIDI's in-progress `sched::tests::midi::*`, which contain no
  native references. BE-MIDI's later edits fixed them (be-native-nextest-nofailfast-s183-1.log: 747/747).
- `-s183-3` nextest/fixtures/own: exit=127. zsh did not word-split the env-prefix variable, so no test ran.
- `be-native-fmt-s183-1.log`: formatting diffs in owned files. `rustfmt` was then applied to owned files only.
- Isolated copy `/tmp/vactrol-native-s183`, with only BE-MIDI's test module stubbed:
  be-native-scratch-nextest-s183-3.log (730/730) and be-native-scratch-cargotest-s183-1.log (720 + 10). A mutation run
  (be-native-scratch-mutation-s183-1.log) disabled revocation in `MidiOutQueue::control`, root containment, and
  overflow reporting. Each was caught: 4 tests failed. The copy was restored byte-identical.

**Notes for BE-FINAL**: the check-and-test-after-modify agent was not invoked; its checks ran directly as the logged
gates above (the same deviation as BE-CONTRACTS/BE-SCHED). Formal review and the README/core-plan bookkeeping are
downstream.

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

### Session: 2026-09-25 (session 184, BE-NATIVE implementer, redispatch attempt 2)

**Why a redispatch**: attempt 1 failed when its review output was validated
(`$.findings[0].intentRef additional property is not allowed`, see FINDING KEY NOTE). The source was not at fault.
`reviewFeedback.findings` is empty. The attempt-1 files still match `attempt-1/final-hashes.txt` (every `.rs` and
`examples/beep.rs` OK). Only this plan file differs, because of the operator note.

**Repair requests handled (from the accepted BE-MIDI output, owner BE-NATIVE)**. Evidence:
`tmp/be-backend-20260925-s181/BE-NATIVE/attempt-2/` (`intent.md`, `pre-edit-hashes.txt`, `final-hashes.txt`,
`run-gates.sh`).
1. Channel nibble 0..15 maps to 1..16: already done (`parse_midi` `(status & 0x0F) + 1`), and
   `tests/midi.rs` covers it (`0x9F` gives `ch: 16`; out `ch: 16` gives `0x8F`). No change.
2. Open notes (`MidiEvent::Note` with `dur = inf`, which BE-MIDI live notes use): **defect fixed**. `MidiOutQueue::push`
   clamped every non-finite due to 0.0, so an open note's off went out at t=0, before its note-on. Now `+inf` is kept (the
   off never becomes due by time); only NaN/-inf clamp to 0. An explicit `NoteOff` closes the earliest open off of the
   same slot and wire bytes by moving its due to `time`, which keeps the on/off pairing for revocation and shutdown;
   otherwise it is queued as before. Panic pulls open offs forward to `effective_time` (11.3), and shutdown sends the
   offs of sounding open notes. The sender wait is already clamped to `OUT_POLL`, so an infinite `next_due` is safe.
   New tests: `an_open_note_waits_for_its_explicit_note_off`, `repeated_open_notes_close_earliest_first`,
   `a_panic_cuts_an_open_note`, `shutdown_releases_a_sounding_open_note`.

**Verification (session 184, shared tree after BE-MIDI completed)**

| Row | Log | Result |
|-----|-----|--------|
| V1 build | be-native-build-s184-1.log | exit=0, no warnings |
| V2 clippy -D warnings | be-native-clippy-s184-1.log | exit=0 |
| own tests | be-native-own-s184-1.log | exit=0, 40 run, 40 passed |
| V3 nextest | be-native-nextest-s184-1.log | exit=0, 751 run, 751 passed, 1 skipped |
| V3t cargo test | be-native-cargotest-s184-1.log | exit=0, lib 741 passed + spec_fixtures 10 passed (1 ignored) |
| V3f fixtures | be-native-fixtures-s184-1.log | exit=0, 10 run, 10 passed |
| N1 example | be-native-example-s184-1.log | exit=0 |
| V6a wasm32 | be-native-wasm32-s184-1.log | exit=0 |
| V6b wasm32 host-wasm | be-native-wasm32-hostwasm-s184-1.log | exit=0 |
| N2 cargo tree wasm32 | be-native-tree-s184-1.log | exit=0; only `vactrol`, no cpal/midir |
| V8 rustfmt --check (owned) | be-native-fmt-s184-1.log | exit=0 |
| V4 wc | be-native-wc-s184-1.log | largest 786 (dsp/engine.rs) |
| V5 std grep | be-native-stdgrep-s184-1.log | none |

**Manual check**: `cargo run --example beep` audible: still **pending user confirmation**.

### GATE GAP NOTE (operator, 2026-09-25, after the BE-NATIVE attempt-2 gate block)

- The progress gate treats ANY entry in the step6 `verificationGaps` list as
  "implementation-materially-unverified" and blocks the branch. Attempt 2 was
  blocked only because the manual audible check of `examples/beep.rs` was
  listed there. That check is pending user confirmation and is NOT a gate by
  issue #3's contract: report it under `residualRisks` (or in the progress
  log as "manual check pending user confirmation"), leave `verificationGaps`
  EMPTY, and let the headless render test stand as the automated proxy.

### Session: 2026-09-25 (session 185, BE-NATIVE implementer, redispatch attempt 3)

**Why a redispatch**: the progress gate blocked attempt 2 only because the manual audible check was listed as a
verification gap (see GATE GAP NOTE). No source is at fault, and `reviewFeedback.findings` is empty. Every owned `.rs`
file and `examples/beep.rs` still match `attempt-2/final-hashes.txt`. **No source was changed.** This entry is the only
edit (append-only). Evidence: `tmp/be-backend-20260925-s181/BE-NATIVE/attempt-3/` (`intent.md`, `pre-edit-hashes.txt`,
`final-hashes.txt`, `run-gates.sh`).

**Verification (session 185, fresh run on the shared tree)**

| Row | Log | Result |
|-----|-----|--------|
| V1 build | be-native-build-s185-1.log | exit=0, no warnings |
| V2 clippy -D warnings | be-native-clippy-s185-1.log | exit=0 |
| own tests | be-native-own-s185-1.log | exit=0, 40 run, 40 passed |
| V3 nextest | be-native-nextest-s185-1.log | exit=0, 751 run, 751 passed, 1 skipped |
| V3t cargo test | be-native-cargotest-s185-1.log | exit=0, lib 741 passed + spec_fixtures 10 passed (1 ignored) |
| V3f fixtures | be-native-fixtures-s185-1.log | exit=0, 10 run, 10 passed |
| N1 example | be-native-example-s185-1.log | exit=0 |
| V6a wasm32 | be-native-wasm32-s185-1.log | exit=0 |
| V6b wasm32 host-wasm | be-native-wasm32-hostwasm-s185-1.log | exit=0 |
| N2 cargo tree wasm32 | be-native-tree-s185-1.log | exit=0; only `vactrol`, no cpal/midir |
| V8 rustfmt --check (owned) | be-native-fmt-s185-1.log | exit=0 |
| V4 wc | be-native-wc-s185-1.log | largest 786 (dsp/engine.rs) |
| V5 std grep | be-native-stdgrep-s185-1.log | none |

**Manual check (residual risk, not a gate)**: whether `cargo run --example beep` is audible on the native host is
**pending user confirmation**. The automated proxy is BE-FINAL's headless render of the same program.

### Closing note (BE-FINAL, session 186)

Accepted by the integration review (acceptedPlanIds) and reconciled by BE-FINAL on the joined tree: every final-tree gate exits 0 (`target/fe-logs/be-final-<check>-s186-1.log`), and the TASK-007/008 checkboxes in vactrol-core.md cite this plan's tests. The audible `cargo run --example beep` check stays pending user confirmation (automated proxy `src/host/tests/e2e/beep.rs`). Archive to impl-plans/completed/ in the separate docs commit after the workflow commit.
