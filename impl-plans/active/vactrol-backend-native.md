# Vactrol Back End: Native Audio Host and examples/beep.rs (BE-NATIVE) Implementation Plan

**planId**: BE-NATIVE (vactrol-core.md TASK-008 `NativeAudioHost`: cpal callback, timer tick, midir, native `SampleLoader`; `examples/beep.rs`)
**Status**: Ready
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

- [ ] `NativeAudioHost` (cpal callback, timer tick, midir, WAV `SampleLoader`) implemented as listed
- [ ] `examples/beep.rs` builds under `host-native` (N1); audible check recorded as pending user confirmation
- [ ] V1-V8, N1, N2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-NATIVE implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-008)
- **Previous**: vactrol-backend-sched.md, vactrol-backend-dsp.md, vactrol-backend-inst.md
- **Next**: vactrol-backend-finalize.md
