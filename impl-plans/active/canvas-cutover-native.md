# Canvas Cutover: Native Output Clock and Session Owner Implementation Plan

**Status**: In Progress
**Plan ID**: CANVAS-NATIVE (wave 1, no dependencies)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.5 (and 15.3.8.4 protocol fields, 15.3.8.7)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

The native host must publish real output-latency provenance from CPAL. It must answer
`clock-probe` so that the frontend can correlate page time with audible engine time, and it must
expose one reusable session-owner loop. The CLI WebSocket server uses that loop now, and the Tauri
IPC shell uses it later (CANVAS-SHELL).

This plan is Rust-only and lives inside the `vactr` crate. It does not touch `editor/`.

Repository facts at c9e5a05:

- `src/host/native/audio.rs` (981 lines) builds the cpal stream in `open_with_outputs`. The
  callback at `audio.rs:459` ignores `cpal::OutputCallbackInfo`.
- `FrameClock` (`audio.rs:68-110`) is frames divided by the sample rate.
- `src/session/publish.rs:374` is `impl Session`. `tick_routed` emits `TransportSample` with
  `latency_kind: "unavailable"` (`publish.rs:520-531`) and the epoch as
  `format!("session-{}-{}", transport_id, transport_epoch)` (`publish.rs:465`).
- `LevelsBody` is defined at `src/session/protocol.rs:720`.
- `ClientMsg` and `ServerMsg` (`protocol.rs:375`, `:767`) use `#[serde(tag = "kind", content = "body", rename_all = "kebab-case")]`
  and derive `PartialEq` (not `Eq`), so `f64` fields are fine.
- `src/cli/serve.rs` owns the only native loop (5 ms `TICK_PERIOD`). `build_session` and
  `HostClock` live in `src/cli/mod.rs:53-78,192-280`. `serve` and `ws` are
  `#[cfg(feature = "host-native")]` (`src/cli/mod.rs:19-22`).
- The frontend wire shape is fixed by `editor/src/protocol/types.ts:204-215` (`ClockProbeBody`,
  `ClockProbeReply`) and validated by `editor/src/protocol/envelope.ts:281-290`. That is the
  existing contract; see the CANVAS-CLOCK reconciliation note. The Rust side must serialize
  exactly that shape:
  - request: `clock-probe { page_send: f64 }`;
  - reply: `clock-probe { page_send, engine_receive, engine_send, epoch, correlation?, latency_seconds, latency_kind, uncertainty_seconds }`;
  - `correlation` is omitted (`None`, `skip_serializing_if`);
  - `engine_receive == engine_send == reading.processing_time` (seconds);
  - `page_send` is echoed exactly.

## Non-goals

- No Tauri code, no TypeScript, and no iOS code (CANVAS-SHELL).
- No stream rebuild after a failed resume. Report a diagnostic instead.
- No change to the browser Wasm tier's behavior. With no reading observed, the output stays
  `unavailable`.
- No new crates, and no `Cargo.toml` or `Cargo.lock` change.

## Ownership

writePaths: see the manifest entry `CANVAS-NATIVE`.

sharedPaths (conditional):

- `src/cli/tests/ws.rs`: edit only if the `spawn_accept_loop` signature change breaks it. Keep
  every assertion.
- `src/cli/repl.rs` and `src/cli/run.rs`: edit only if the `HostClock::Native` pattern change
  breaks compilation. One-line pattern updates only.
- `src/host/native/audio/song.rs`: expected unedited. Edit only if the `audio.rs` split moves an
  item it imports.

## Contracts (pinned)

```rust
// src/session/publish.rs (re-exported from src/session/mod.rs)
pub enum LatencyKind { Measured, Estimate, Unavailable }      // as_str(): "measured" | "estimate" | "unavailable"
pub struct ClockReading { pub processing_time: f64, pub latency_seconds: Option<f64>,
                          pub latency_kind: LatencyKind, pub uncertainty_seconds: Option<f64> }
impl Session { pub fn observe_clock(&mut self, reading: Option<ClockReading>); pub fn clock_discontinuity(&mut self); }

// src/host/native/clock.rs
pub struct OutputClock { /* Arc of AtomicU64 seqlock; Clone */ }
impl OutputClock { pub fn new(sample_rate: u32) -> Self; pub fn record(&self, info: &cpal::OutputCallbackInfo, frames_before: u64, buffer_frames: u32);
                   pub fn read(&self) -> ClockReading; pub fn set_running(&self, running: bool); }

// src/cli/owner.rs (#[cfg(feature = "host-native")] pub mod owner)
pub enum AudioSessionEvent { Interrupted, Resumed, RouteChanged }
pub struct SessionOwner { /* Sender<OwnerEvent>, JoinHandle, AtomicU32 id counter */ }
impl SessionOwner {
  pub fn spawn(host: crate::cli::args::HostChoice, cwd: std::path::PathBuf) -> Result<SessionOwner, String>; // blocks until built
  pub fn connect(&self, outbound: std::sync::mpsc::Sender<String>) -> u32;
  pub fn send(&self, id: u32, text: String);
  pub fn close(&self, id: u32);
  pub fn audio_event(&self, event: AudioSessionEvent);
  pub fn shutdown(self);           // also performed by Drop; joins the thread
}
```

`OwnerEvent` is `pub(crate)`:
`enum OwnerEvent { Conn(crate::cli::ws::ConnEvent), Audio(AudioSessionEvent), Shutdown }`.

## Tasks

### TASK-001: OutputClock (src/host/native/clock.rs, tests/clock.rs)

**Writer side (audio callback).**

- Bump the sequence number to odd; store `frames_before`, the `Instant` nanoseconds since the
  clock origin (one `Instant::now()`), and
  `playback.duration_since(&callback)` in nanoseconds (or `u64::MAX` when `None`); then bump the
  sequence to even.
- Use only `Release` stores. No allocation, lock, `Vec`, `format!` or logging is allowed in the
  callback.

**Reader side (owner thread).**

- `read()` retries up to 4 times while the sequence is odd or changed, and otherwise returns the
  cached previous reading. Keep that cache in a `Mutex` on the reader side only, never touched
  by the writer. Alternatively, return a reading with provenance `Unavailable`.
- `processing_time = frames / rate + min(now - callback_instant, buffer_frames / rate)`.
- The latency kind:
  - `Measured`: a host playback duration is present and the record is at most 1 s old;
  - `Estimate`: no playback duration, so `buffer_frames / rate` is used;
  - `Unavailable`: never recorded, `set_running(false)`, or the record is over 1 s old.
- The uncertainty is `buffer_frames / rate`.

**Testing without a device.** Use a test-only `record_raw(frames, instant_nanos, playback_nanos)`
under `#[cfg(test)]`, so tests do not need a cpal device. Do not add `allow` or `expect`
attributes.

### TASK-002: audio.rs hook and split (audio.rs, audio/stream.rs, mod.rs)

- Move `open_with_outputs` and its stream-building helpers into
  `src/host/native/audio/stream.rs` (`mod stream;` in `audio.rs`, next to the existing
  `mod song;` and `mod song_capacity;`). `audio.rs` must end below 1000 lines; target 900 or
  fewer.
- In the output callback, call `output_clock.record(info, frames_before, data.len() / channels)`
  before `side.render`. Read `frames_before` from the `FrameClock` frames counter. Clone the
  `OutputClock` into the closure before `build_output_stream`.
- `NativeAudioHost` exposes `output_clock()` and `stream_control()`. `StreamControl` holds an
  `Rc<RefCell<Option<cpal::Stream>>>` and is used on the owner thread only. It provides:
  - `suspend()`: pause, then `set_running(false)`;
  - `resume() -> Result<(), Diagnostic>`: play, then `set_running(true)`, reporting `unavailable(..)`
    on error.
- `NativeHosts` (`mod.rs:90`) gains `pub output: OutputClock` and `pub stream: StreamControl`.
- Add `pub mod clock;` to `mod.rs`.

### TASK-003: Session protocol (protocol.rs, codec.rs, publish.rs, session.rs, mod.rs)

- `protocol.rs`:
  - add `ClientMsg::ClockProbe(ClockProbeBody { page_send: f64 })` and
    `ServerMsg::ClockProbe(ClockProbeReply { .. })`, with the field names as above and
    `correlation: Option<ClockProbeCorrelation { engine_time: f64, output_time: f64 }>`;
  - add `"clock-probe"` to every kind list and match (`protocol.rs:398-420` and the server
    equivalents);
  - `LevelsBody` gains `time: Option<f64>` and `epoch: Option<String>`, both with `#[serde(default, skip_serializing_if = "Option::is_none")]`;
  - `protocol.rs` must stay below 1000 lines, otherwise split the probe types into
    `src/session/protocol/clock.rs` and record that path as a sharedPaths addition.
- `codec.rs`: decoding rejects a `page_send` that is non-finite or negative.
- `publish.rs`:
  - `ClockReading` and `LatencyKind` are defined here;
  - `TransportSample.latency_*` comes from `self.observed_clock` when it is `Some`, and stays
    the current `None`/`"unavailable"` otherwise;
  - levels set `time: Some(host_now)` and `epoch: Some(epoch.clone())`, computed before levels
    (move the epoch computation above levels if needed).
- `session.rs`:
  - add the `observed_clock` field, `observe_clock`, and
    `clock_discontinuity()`, which does `transport_epoch += 1`, clears pending telemetry, and
    leaves control replies untouched;
  - handle `ClientMsg::ClockProbe` in `apply_text` (`session.rs:477`) by replying to the
    requesting connection with `re` set. Without an observed reading, reply with
    `latency_kind: "unavailable"`, `latency_seconds: None`, and
    `engine_receive = engine_send = last host_now`.

### TASK-004: Session owner (cli/owner.rs, cli/serve.rs, cli/ws.rs, cli/mod.rs)

- `HostClock::Native` becomes `Native(FrameClock, NativeLink)`, where `NativeLink` holds an
  `OutputClock` and a `StreamControl`. `build_session_native` fills it from `NativeHosts`.
  `now()` and `advance()` are unchanged.
- `owner.rs`:
  - `pub(crate) fn run_loop(session, clock, rx: Receiver<OwnerEvent>, mut route: impl FnMut(&Session, Vec<Outgoing>))`
    is the body moved out of `serve.rs`;
  - before every `apply_text` and every tick, it calls
    `session.observe_clock(native.map(|n| n.output.read()))`;
  - `Audio(Interrupted)` calls `stream.suspend()` then `clock_discontinuity()`;
  - `Audio(Resumed)` calls `stream.resume()` (pushing the diagnostic on error) then
    `clock_discontinuity()`;
  - `Audio(RouteChanged)` calls `clock_discontinuity()`;
  - `Shutdown` breaks the loop;
  - `SessionOwner::spawn` builds the session on the new thread, because `Session` is `!Send`,
    and reports the build result through a `sync_channel(1)`.
- `serve.rs` keeps its CLI behavior (prints the URL and token, exit codes) and calls `run_loop`
  on the main thread.
- `ws::spawn_accept_loop` takes `Sender<OwnerEvent>` and wraps each event in `OwnerEvent::Conn`.
- Add `#[cfg(feature = "host-native")] pub mod owner;` to `cli/mod.rs`.

### TASK-005: Tests

| Test | Situation | Expected outcome |
|------|-----------|------------------|
| `src/host/native/tests/clock.rs` | `record_raw(48000, t, Some(10 ms))` at rate 48000, buffer 512 | `processing_time` is in [1.0, 1.0 + 512 / 48000], kind `Measured`, latency 0.010, uncertainty 512 / 48000 |
| same | Playback `None` | kind `Estimate`, latency 512 / 48000 |
| same | Never recorded, or `set_running(false)` | kind `Unavailable`, latency `None` |
| same | Record over 1 s old (inject instants) | `Unavailable` |
| same | Writer thread hammering while the reader reads 10,000 times | No reading with `processing_time` going backwards or NaN |
| `src/session/tests/codec.rs` | Probe request JSON `{"v":1,"seq":3,"kind":"clock-probe","body":{"page_send":12.5}}` | Decodes |
| same | `page_send: -1` | Rejected |
| same | Reply serialization | Exact keys, with `correlation` absent |
| `src/session/tests/publish.rs` | `observe_clock(Some(Measured 0.012))` then tick | `TransportSample.latency_kind == "measured"`, `latency_seconds == Some(0.012)` |
| same | No observation | `unavailable` unchanged |
| same | Levels | Carry `time == host_now` and the same epoch as the transport sample |
| same | `clock_discontinuity()` | Next epoch string differs; queued playing telemetry is cleared; a pending eval reply is still delivered |
| `src/cli/tests/owner.rs` | `SessionOwner::spawn(HostChoice::Noop, <fresh dir under std::env::temp_dir()>)`, `connect(tx)`, send a `clock-probe` (no new dev-dependency) | A `clock-probe` reply on `rx` echoes `page_send` |
| same | `shutdown()` | Joins within 2 s |
| same | `Audio(RouteChanged)` | Changes the epoch in the next tempo telemetry |

## Pitfalls

- Doing any work in the cpal callback beyond `record`. In particular, never touch `Mutex`,
  `Vec`, logging or the session there.
- Serializing `correlation: null`. It must be omitted, because the TypeScript validator
  rejects a non-object.
- Making the reply a telemetry message. It is a control reply routed to the requester and must
  never be coalesced or dropped.
- Changing the epoch on every tick. Change it only through `clock_discontinuity` and the
  existing restart and state rules.
- Running crate-wide `cargo fmt`. Run `rustfmt` on touched files only.
- Adding `#[allow]` or `#[expect]`. Dead code gets `#[cfg(test)]` or is removed.

## Verification (record exit codes and log paths in tmp/canvas-cutover/native/checks.log)

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build` | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0, zero diagnostics |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/clock|owner|publish|codec|serve|ws/)'` | all pass |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown` | exit 0 (Session changes compile for Wasm) |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 (shell still builds against the changed crate) |
| `rustfmt --edition 2021 --check <each touched .rs file>` | exit 0 |
| `wc -l src/host/native/audio.rs src/session/protocol.rs` | each < 1000 |
| Outside the sandbox: full nextest with a timeout of at least 1500 s | all pass; a timeout kill counts as neither pass nor fail |

## Overwrite and Drift Protocol

- Record fresh-read sha256 values before each edit in `tmp/canvas-cutover/native/intent.json`,
  and post-edit hashes in `receipt.json`.
- On drift in a file you have not yet edited, stop editing that file and repair it serially
  after the join.
- Edit only this plan's progress log.

## Completion Criteria

- [x] Contracts exactly as pinned; reply JSON matches `editor/src/protocol/types.ts:205-215`
- [x] The audio callback adds only the `record` call (reviewed by diff)
- [x] `audio.rs` and `protocol.rs` < 1000 lines
- [x] Build, clippy, focused nextest, Wasm build and src-tauri check pass
- [x] Stale clock coverage uses nonzero samples on both sides of the one-second threshold; mutation without the age predicate fails
- [x] Clock discontinuity drops pending telemetry while preserving diagnostics and requester replies

## Progress Log

### Session: 2026-10-05
**Tasks Completed**: TASK-001 through TASK-005 implemented; protocol contract, callback scope, file-size limits, and all assigned verification gates confirmed.
**Evidence**: `tmp/canvas-cutover/native/checks.log`; final full-suite log `tmp/canvas-cutover/native/full-nextest-final-rerun.log`; post-edit hashes in `tmp/canvas-cutover/native/receipt.json`.
**Verification**: build, strict clippy, focused nextest (493/493), wasm build, Tauri cargo check, touched-file rustfmt, and full nextest (2,810/2,810; 3 skipped) all exited 0. `audio.rs` is 896 lines and `protocol.rs` is 940 lines.
**Notes**: The first final-suite wrapper completed nextest with all tests passing but then exited 1 because zsh reserves the variable name `status`; it is retained in `tmp/canvas-cutover/native/full-nextest-final.log`. The clean rerun captured exit 0 in the rerun log above. Independent review and workflow finalization remain downstream.

### Session: 2026-10-05 (integrity review repairs)
**Findings Addressed**: NATIVE-TI-001 and NATIVE-TI-002.
**Changes**: Added a test-only backdated OutputClock constructor. The stale test now injects a nonzero sample 2 seconds old and a 0.5-second-old measured sample. `clock_discontinuity` now removes only Telemetry, Levels, and Tempo broadcasts; the regression test queues Playing, Diag, and Manifest messages and checks what is dropped and delivered.
**Verification**: Mutation removal of `age_seconds <= 1.0` failed at the stale assertion (expected). Mutation to the old requester-only outbox predicate failed at the new Diag-preservation assertion (expected). Restored-source focused nextest passed 493/493, clippy with `-D warnings` passed, wasm build passed, rustfmt check passed, and full nextest passed 2,810/2,810 with 3 skipped. See `tmp/canvas-cutover/native/checks.log` and its referenced complete logs.
