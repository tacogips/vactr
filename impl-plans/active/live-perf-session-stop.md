# LP-SESSION-STOP: Scheduler stop modes, output-stop delivery, song stop, output telemetry

**Status**: In Progress. TASK-S1 to TASK-S4 are implemented. TASK-S5 (session-343 scope amendment: host-native `stop_song` tests) is Ready.
**Plan ID**: LP-SESSION-STOP (wave 2; the only wave-2 plan redispatched in session 343)
**Design Reference**: `design-docs/specs/design-live-performance.md` 4.1, 4.4, 4.5, 4.6, 8.1(5), D1, D2, D5
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Created**: 2026-10-08
**Last Updated**: 2026-10-09 (session-343 scope amendment)

## Intent and Context

The session and scheduler decide what a stop means. Per the design:

- **`stop-all`** (stop button, `Mod-.`) is **gentle**:
  - every pattern slot is revoked with `Release::Natural`;
  - live notes close;
  - the engine receives `CtlMsg::OutputStop { mode: Gentle }`;
  - an active song ends early through the existing Draining path.
- **`hush`** (hush button, `Mod-Shift-.`, the `hush` native) is a **cut**:
  - every slot is revoked with `Release::Panic`, as today;
  - live notes close;
  - the engine receives `OutputStop { mode: Cut }`;
  - an active song is cut off now.
- **Per-slot `stop`** is unchanged in the scheduler (Natural). Its
  stronger audio-side meaning is LP-ENGINE's D3 change.
- **Telemetry:** the engine's `HostMsg::OutputState` reports become
  `TransportSample.output`.

LP-CONTRACT already pinned the wiring:

- `StagedEffect::StopAll` and `StagedEffect::Cut`;
- `Runtime::apply` arms calling `self.stop_all(ev, rep)` and
  `self.cut(ev, rep)`;
- the HostMsg arm `self.output.observe(phase, frame)`;
- `self.output_tick()` in `Runtime::tick`;
- `TransportSample.output = self.rt.output.wire()`;
- the session arms (`Hush`, `StopAll`).

All of these live as behavior-preserving stubs in
`src/sched/runtime/live.rs`. This plan implements them.

## Non-goals

- No engine changes (LP-ENGINE), no momentary logic
  (LP-SESSION-MOMENTARY) and no editor changes.
- No change to the MIDI-clock `transport_stop` path (11.7), to MIDI/OSC sink
  semantics, or to `Release` merge rules.
- No change to `runtime.rs`, `session.rs` or `publish.rs`; the contract
  owns those call sites.

## Dependencies

- **dependsOn**: LP-CONTRACT.
- **Blocks**: LP-EVIDENCE.

## writePaths

- `src/sched/runtime/live.rs`
- `src/sched/control.rs`
- `src/sched/runtime/song.rs` (adds `stop_song` only; already done. It is
  973 lines and must not change in the TASK-S5 redispatch.)
- `src/sched/runtime/song/clock_tests.rs` (session-343 scope amendment;
  TASK-S5 only: new host-native `stop_song` tests plus one source-parameterized
  fixture helper. Existing tests stay behaviorally unchanged.)
- `src/sched/tests/sched/live.rs`
- `src/sched/tests/sched/control.rs` (new rows only; existing assertions unchanged)
- `src/session/tests/live_stop.rs`
- `impl-plans/active/live-perf-session-stop.md` (Progress Log only)
- Artifact roots (also in the manifest's `artifactRoots`): `target`,
  `tmp/live-perf/session-stop`

## trackedPaths

The source snapshot paths, meaning the writePaths minus the artifact roots:

- `src/sched/runtime/live.rs`
- `src/sched/control.rs`
- `src/sched/runtime/song.rs`
- `src/sched/runtime/song/clock_tests.rs`
- `src/sched/tests/sched/live.rs`
- `src/sched/tests/sched/control.rs`
- `src/session/tests/live_stop.rs`
- `impl-plans/active/live-perf-session-stop.md`

## sharedPaths

None. Read-only:

- `src/sched/runtime.rs` (`Runtime::revoke` callers, `close_live_notes`)
- `src/sched/song.rs` (`SongTransport::cutoff` at :544, `endpoints()`,
  `state()`)
- `src/host/caps/song/preparation/activation.rs:200` (the `cutoff()`
  endpoint shape)
- `src/host/testing.rs` (`RecordingAudioHost`)
- `src/session/tests/support.rs`

## Tasks

### TASK-S1: Release-parameterized revoke (`src/sched/control.rs`)

- Add `pub(crate) fn revoke_with(&mut self, key: SlotKey, release: Release)`
  holding today's body of `revoke`, with `release` passed in.
- `revoke(key)` stays as a wrapper that keeps today's mapping exactly:
  `All` maps to Panic, and a slot maps to Natural.
- Texture slots still get the empty program, and ephemeral `once` slots
  are still removed.

### TASK-S2: `stop_all` and `cut` (`src/sched/runtime/live.rs`)

Both read `now = self.hosts.audio.now()` once.

**`stop_all(ev, rep)`:**

1. `self.revoke_with(SlotKey::All, Release::Natural)`.
2. `self.close_live_notes(SlotKey::All, now)`.
3. `self.hosts.audio.post(CtlMsg::OutputStop { mode: OutputMode::Gentle })`.
4. `self.output.request(OutputMode::Gentle, tick)`.
5. If a song is Playing, `self.stop_song(OutputMode::Gentle, rep)`.

**`cut(ev, rep)`:** first increments the `OutputTracker` cut counter
returned by `cuts()` (pinned by LP-CONTRACT;
LP-SESSION-MOMENTARY reads it). Then it follows the same order as
`stop_all`, with these differences:

- Step 1 uses `Release::Panic`.
- Step 3 posts `OutputMode::Cut`.
- Step 5 runs `stop_song(OutputMode::Cut, rep)` when the song is Playing or
  Draining.

**Ordering.** The `OutputStop` must be posted **after** the `SlotControl`
records of step 1. `control.immediate` sends synchronously; confirm this by
reading `src/sched/control.rs::immediate` and assert it in a test.

### TASK-S3: `OutputTracker` delivery (`src/sched/runtime/live.rs`)

Fields:

- `phase: Option<OutputPhase>`
- `frame: u64`
- `requested: Option<(OutputMode, u32 /*ticks waiting*/, u8 /*resends*/)>`

Behavior:

- **`observe(phase, frame)`** stores both. Any report whose
  `frame >= request frame` clears `requested`, because the engine
  acknowledges every `OutputStop` (LP-ENGINE). Use the latest
  `self.hosts.audio.now()` mapped to frames with
  `cfg.sample_rate`, or simply clear on the first report after the
  request.
- **`output_tick()`**, while `requested` is set:
  - increment the wait;
  - at `cfg.resend_ticks`, re-post the same `OutputStop` and reset the
    wait;
  - re-send at most 3 times;
  - after that, push one diagnostic using the existing host-transport
    diagnostic code, the same pattern as the unacked slot-control
    diagnostic in control.rs.
- **Merging:** a `Cut` request supersedes a pending `Gentle` request, and
  a `Gentle` request never replaces a pending `Cut`.
- **`wire()`** maps `Running` to `"running"`, `Draining` to `"draining"`,
  `Cutting` to `"cutting"`, `Idle` to `"idle"`, and `None` to `None`.

### TASK-S4: Song stop (`src/sched/runtime/song.rs`)

Add
`pub(crate) fn stop_song(&mut self, mode: OutputMode, rep: &mut DrainReport)`.

**Find the transport.** Use the active `SongTransport` that this file
already holds, and its `endpoints()`, `activation` frame and `epoch`. Get
the current host frame the same way the existing song code maps host time
to frames (search this file for `song_clock` or the frame mapping helper).

**Gentle** (state Playing only):

- `arrangement = clamp(now_frame + round(cfg.commit_lead * sr), activation.frame, old.arrangement)`
- `tail_deadline = min(arrangement + (old.tail_deadline - old.arrangement), old.tail_deadline)`

**Cut** (state Playing or Draining):

- `arrangement = clamp(now_frame, activation.frame, old.arrangement)`
- `tail_deadline = clamp(now_frame, arrangement, old.tail_deadline)`

**Call and outcomes.** Call `transport.cutoff(&mut *self.hosts.audio, SongEndpoints { epoch, arrangement, tail_deadline })`:

- a `SongCommandRefusal` pushes a `Failure` into `rep.faults`; never panic
  or unwrap;
- with no song, or a song in another state, nothing happens.

These rules satisfy `SongTransport::cutoff`'s validation at
src/sched/song.rs:544-556. Never pass an arrangement beyond the old one, or
a tail deadline beyond the old one.

### TASK-S5: Host-native `Runtime::stop_song` tests (`src/sched/runtime/song/clock_tests.rs`, session-343 amendment)

**Why.** The open finding LP-SESS-STOP-TI-2-STOP-SONG-UNTESTED (mid): no
test runs `Runtime::stop_song` (src/sched/runtime/song.rs:458) with a
genuinely installed song. The pure `stop_endpoints` table rows in
`src/sched/tests/sched/live.rs` do not cover the call path through
`SongTransport::cutoff`, the host submission, or the Draining/Ended
transitions. This task adds only tests. It changes no product code.

**Where and why it is reachable.**
- `clock_tests` is declared at song.rs:18 (`#[cfg(test)] mod clock_tests;`)
  and starts with `use super::*;`.
- It is a descendant of `sched::runtime`, so it can read the private field
  `Runtime.song` (runtime.rs:155), `Runtime.cfg` (runtime.rs:146), and the
  private fields of `SongRuntime` (`owners`, `clock`) and of `Running`
  (`transport`, `activation`).
- The existing tests already do this: `runtime.song.owners[0].transport`
  at clock_tests.rs:279 and :320.
- Do not widen any visibility. Do not edit `song.rs`, `runtime.rs`,
  `live.rs` or `src/sched/song.rs`.

**Pattern to copy.** Use
`clock_tests.rs:rejected_new_epoch_clock_preserves_other_genuine_ready_owner`
(lines 288-369):

- `EngineConfig::new(&caps, 8000., MAX_BLOCK, StoreKind::NativeArc)` with
  `caps.max_voices = 2` and `bus_slots = 12`;
- `NativeAudioHost::headless_with_config(config, 64)`;
- `genuine_ready(...)`;
- `activation = audio.song_clock().unwrap().frame + 960`;
- `Runtime::new(hosts, Rc::new(NoopHost), caps, RuntimeConfig::default())`
  and `start_song(ready, activation)`;
- a bounded loop of `take_host_msgs` + `tick_song` + `side.render(&mut pcm, 2)`
  until `transport.applied_activation().is_some()` and
  `transport.state() == SongTransportState::Playing`.

Every loop is bounded (for example `for _ in 0..N`) and fails with a
message if its exit condition is never reached. Never write
`loop {}` / `while` without a bound.

**Fixture change (the only change to existing code).**
- Split `genuine_ready` into `genuine_ready_with(audio, side, caps, epoch,
  source: &str)`.
- `genuine_ready(audio, side, caps, epoch)` delegates with today's exact
  source string. The two existing callers and their assertions are
  unchanged.
- Gate the new helper with `#[cfg(feature = "host-native")]`, the same as
  `genuine_ready`.

**Test source.**
- The new tests need a song tail longer than zero, so the gentle tail
  window is observable.
- Use the same source with `tail-seconds: 1` instead of `tail-seconds: 0`
  (`SongSettings.tail_seconds`, src/song/song.rs:16; the default is 8).
- If `duration: 4` ends the arrangement before the stop can be issued
  after Playing, raise the duration in the test source only.

**New tests.** All are `#[cfg(feature = "host-native")] #[test]`. Names
are suggestions.

1. `stop_song_gentle_drains_then_ends_within_the_song_tail`
   - Reach Playing. Capture:
     - `old = transport.endpoints()`;
     - `now = runtime.hosts.audio.song_clock().unwrap().frame`;
     - `lead = (runtime.cfg.commit_lead * 8000.).round() as u64`.
   - Call `runtime.stop_song(OutputMode::Gentle, &mut DrainReport::default())`
     with a named report and check it.
   - Expected:
     - `rep.faults` is empty;
     - `new = transport.endpoints()` has `new.epoch == old.epoch`;
     - `new.arrangement == (now + lead).max(activation).min(old.arrangement)`;
     - `now <= new.arrangement <= old.arrangement`;
     - `new.tail_deadline - new.arrangement == (old.tail_deadline - old.arrangement)`,
       capped so that `new.tail_deadline <= old.tail_deadline`;
     - `new.tail_deadline > new.arrangement`. The tail window exists:
       tails continue after the arrangement end.
   - Then run the bounded render/tick loop. Record the song clock frame and
     state on each iteration. Expected:
     - Draining is observed before Ended;
     - the first Draining frame is `>= new.arrangement`;
     - Ended is observed within the bound;
     - Ended is first observed at a song clock frame `>= new.tail_deadline`.
       This is design 4.4: private branch tails run to the song's tail
       deadline.
   - `runtime.song_state()` ends as `Some(Ended)`. Faults from every tick
     report stay empty.
2. `stop_song_cut_ends_at_now_and_silences_the_song_branch`
   - Reach Playing, then render further until a rendered block has a peak
     `> 0`. This is the in-test control that proves the song is audible
     before the cut. If the fixture never sounds, fix the test source,
     never the product.
   - Capture `now` as above and call `stop_song(OutputMode::Cut, ..)`.
   - Expected:
     - `rep.faults` is empty;
     - `new.arrangement == new.tail_deadline == now.clamp(activation, old.arrangement)`;
     - `new.epoch == old.epoch`.
   - Run the bounded loop. Expected:
     - the state reaches Ended;
     - every sample rendered after `now + 64` frames (the existing 64-frame
       song tail fade) plus one 128-frame block of slack is exactly `0.0`.
       This shows the song's branch and effect state are cleared: nothing
       rings after a cut.
3. `stop_song_refusal_becomes_a_fault_and_never_panics`
   - Reach Playing. Swap in a refusing host with
     `std::mem::replace(&mut runtime.hosts.audio, Box::new(CachedHost { clock: <current native song_clock()>, refused: true, commands: Vec::new() }))`.
     Reuse the `CachedHost` already in this file (lines 8-41), whose
     `try_song_command` returns `SongSubmitError::Backpressure`.
   - Call `stop_song(OutputMode::Gentle, &mut rep)`. Expected:
     - no panic;
     - `rep.faults.len() == 1` with `FailCode::BeyondCapability`. Match the
       mapping at song.rs:520-532 and check the code field, not the message
       text;
     - `transport.endpoints() == old`: the refusal leaves ownership intact;
     - `transport.state() == Playing`.
   - In-test control: restore the native host (swap back), call
     `stop_song(OutputMode::Gentle, ..)` again, and expect `faults` empty and
     `endpoints().arrangement < old.arrangement`.
   - Refusal through the ready path: if `transport.ready` is still `Some`,
     `cutoff` submits through `ready.submit_initial_command(host, ..)`. That
     still calls the host, so the refusing host covers both paths. Do not
     reach into the private `ready` field.

**Silence and resources.** These tests use the headless native host
(`NativeAudioHost::headless_with_config`). It never opens a device, so the
tests stay silent. Do not use `NativeAudioHost::open` or any CPAL device
path.

**Size.** `clock_tests.rs` is 369 lines. Keep it under 700 lines after
TASK-S5, and well under 1000. If the tests need shared setup, add one
private helper `fn playing_runtime(source: &str) -> (Runtime, AudioSide, u64 /*activation*/)`
in this file. Do not add new files or modules (that would require edits to
song.rs).

## Key Points a Careless Implementation Gets Wrong

- `stop_all` must use **Natural**, not Panic. `hush` keeps **Panic**.
  `revoke(SlotKey::All)` keeps its Panic mapping, because existing tests
  rely on it (src/host/tests/e2e/sched_gaps.rs).
- `OutputStop` goes through `AudioHost::post`, which works on both tiers
  (native: `NativeRecord::Msg`; browser: the encoded wire record). Do not
  route it through the `CellPort`.
- On a pure-visual session with no pattern slots, `stop-all` still posts
  `OutputStop`.
- The song stop must never touch song code rules: `hush`/`stop` inside song
  code stay rejected (src/session/song.rs:552-566).
- Do not edit `runtime.rs`, `session.rs`, `publish.rs`, `stage.rs` or
  `effects.rs` (contract-owned). If a call site there is wrong, record it
  in the Progress Log as a contract defect for serial repair. Do not fix
  it here.
- Keep every touched file under 1000 lines. `runtime/song.rs` is now 973
  lines. TASK-S5 must not touch it; the tests go only in `clock_tests.rs`.
- TASK-S5 is tests only. If a TASK-S5 assertion that follows design 4.4 or
  8.1(5) fails, the product disagrees with the design. Examples: Ended
  before `tail_deadline`, non-zero output after a cut, or a panic on
  refusal. Do not weaken, skip or `#[ignore]` the assertion, and do not
  edit product code outside writePaths. Stop and report it as an
  unrepaired blocking finding with the log path, so the workflow can route
  the repair.
- Do not `#[ignore]` the new tests, and do not gate them on anything other
  than `feature = "host-native"`. The default features include
  `host-native` (Cargo.toml `default = ["host-native"]`), so the focused
  nextest filter `sched::runtime::song` runs them.

## Tests to Add (input -> expected)

**`src/sched/tests/sched/live.rs`**, using `RecordingAudioHost` and the
existing sched test helpers:

- Two pattern slots bound, then `StagedEffect::StopAll` applied:
  - a `SlotControl` with `Release::Natural` for each slot;
  - then exactly one `CtlMsg::OutputStop { mode: Gentle }`, posted after
    them (assert the order in the recorded control log).
- `StagedEffect::Cut` gives `Release::Panic` for each slot, then one
  `OutputStop { Cut }`.
- `StagedEffect::Revoke(d1)` gives Natural for d1 only and no `OutputStop`.
- With no `OutputState` arriving, after `resend_ticks` ticks the
  `OutputStop` is re-posted, at most 3 times, then one diagnostic.
  Injecting `HostMsg::OutputState { Draining, .. }` stops the re-sends.
- A `Gentle` request after a pending `Cut` request does not downgrade it:
  the re-posted record is `Cut`.
- `observe(Idle, f)` makes `wire()` return `"idle"`; `None` before any
  report.
- `cuts()` is 0 at the start, and 1 after one `StagedEffect::Cut`.
  `StopAll` leaves it unchanged.
- **Song, gentle:** with a song Playing (use the runtime song test fixtures
  that `src/sched/runtime/song.rs` tests already build), `stop_song(Gentle)`
  submits `Endpoints` with `arrangement >= now_frame`,
  `arrangement <= old.arrangement` and
  `tail_deadline - arrangement <= old tail length`, and the transport state
  reaches Draining and then Ended.
- **Song, cut:** `stop_song(Cut)` gives
  `arrangement == tail_deadline == clamp(now_frame, ..)`.
- **No song:** a no-op.

**`src/sched/runtime/song/clock_tests.rs`** (TASK-S5, host-native, genuine
installed song through `genuine_ready_with`). These are the behavioral
counterparts of the Song gentle and Song cut rows above:

- Playing song, then `stop_song(Gentle)`: bounded early `Endpoints`,
  `tail_deadline > arrangement`, Draining seen before Ended, and Ended no
  earlier than `tail_deadline`. No faults.
- Audible Playing song, then `stop_song(Cut)`:
  `arrangement == tail_deadline == clamp(now)`, Ended, and every sample
  exactly 0 after `now + 64 + 128` frames. No faults.
- Playing song with a refusing host, then `stop_song(Gentle)`: exactly one
  `BeyondCapability` fault, no panic, endpoints and Playing state
  unchanged. After the native host is restored, a second stop succeeds.

**`src/sched/tests/sched/control.rs`:** one new row asserting that
`revoke_with(slot, Release::Panic)` gates a single slot. All existing rows
are unchanged.

**`src/session/tests/live_stop.rs`**, through the `Session::apply` envelope
path:

- `{"kind":"stop-all"}` records `OutputStop{Gentle}` on the session's test
  audio host;
- `{"kind":"hush"}` records Panic plus `OutputStop{Cut}`;
- after the test host returns `HostMsg::OutputState { Draining, frame }`
  and the session ticks with telemetry subscribed, the published `tempo`
  `transport.output == Some("draining")`;
- before any report, the `output` key is absent from the serialized JSON.

## Verification (exact commands; all must exit 0)

1. `CARGO_TERM_QUIET=true cargo build --all-targets`
2. `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/sched::tests::sched::(live|control)|session::tests::live_stop|sched::runtime::song|host::tests::e2e::sched_gaps|sched::tests::midi/)'`
   passes with testsRun > 0.
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
5. `rustfmt --edition 2021 --check src/sched/runtime/live.rs src/sched/control.rs src/sched/runtime/song.rs src/sched/runtime/song/clock_tests.rs src/sched/tests/sched/live.rs src/sched/tests/sched/control.rs src/session/tests/live_stop.rs`
6. (TASK-S5 evidence) `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/sched::runtime::song::clock_tests::stop_song_/)'`
   passes with testsRun >= 3. This proves the three new tests exist and run
   under the default features.
7. `git diff --exit-code HEAD -- src/sched/runtime/song.rs src/sched/runtime.rs src/sched/runtime/live.rs`
   exits 0. HEAD is the session-343 checkpoint commit, and TASK-S5 changes
   none of these files.
   `wc -l src/sched/runtime/song/clock_tests.rs` reports fewer than 1000
   lines (target: under 700).

Write logs to `tmp/live-perf/session-stop/*.log`. Run every command serially
and never concurrently with another cargo or nextest run. The focused
nextest run does not need the measurement lock; the full suite belongs to
LP-EVIDENCE.

## Completion Criteria

- [x] TASK-S1 to TASK-S4 source behavior is implemented.
- [x] Every listed test passes. Existing control, MIDI and sched_gaps
  assertions are unchanged. OutputStop timeout diagnostics defer while slot
  controls remain outstanding, allowing one slot diagnostic to own the
  outage. The diagnostic test disables the rig's automatic acknowledgments
  so this scenario actually retains outstanding slot controls.
- [x] Added scheduler/session tests for stop-all Natural plus Gentle, hush
  Panic plus Cut, per-slot Natural only, output ordering, retry/ack behavior,
  telemetry omission/mapping, no-song no-op, and OutputStop/slot diagnostic
  de-duplication. Added pure Prepared/Playing/Draining endpoint behavior table
  tests in `src/sched/tests/sched/live.rs`; the Draining-to-Ended transition
  remains covered by existing `SongTransport::cutoff` tests. All focused tests
  pass.
- [x] Verification 1-5 exit 0 (pre-amendment file set). Touched files are
  under 1000 lines. The Progress Log is updated. Rustfmt passes and
  `src/sched/runtime/song.rs` is 973 lines.
- [x] TASK-S5: the three host-native `stop_song` tests (gentle, cut,
  refusal) are in `src/sched/runtime/song/clock_tests.rs`, using
  `genuine_ready_with`. The existing clock_tests are unchanged in behavior.
  No product file changed.
- [x] TASK-S5: verification 1-7 exit 0 on the final source. Verification 5
  includes `clock_tests.rs`, and verification 6 reports testsRun >= 3. Log
  paths are recorded in the Progress Log.
- [x] Finding LP-SESS-STOP-TI-2-STOP-SONG-UNTESTED is reported as repaired
  only in addressedFeedback/resolvedFindings, with the test names and the
  nextest log as evidence.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.

### Session: 2026-10-08 (step 6 implementation)
**Tasks Completed**: TASK-S1 to TASK-S4 source implementation; added explicit
revoke, scheduler output-stop, session envelope, telemetry and no-song tests.
**Notes**: `OutputTracker` retries three times and routes its one diagnostic
through the existing `ControlChannel::tick()` collector. Rustfmt ran on the
six touched Rust files. No tests/builds were run by this implementation pass;
parent-designated verification remains pending. Song Playing/Draining endpoint
tests are pending because the available `genuine_ready` fixture is private to
`src/sched/runtime/song/clock_tests.rs`, not an authorized writePath.

### Session: 2026-10-08 (step 6 verification)
**Tasks Completed**: Required build and clippy gates passed; focused nextest
was run and exposed one existing test assertion conflict.
**Notes**: `CARGO_TERM_QUIET=true cargo build --all-targets` and
`CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
both exited 0. The required focused nextest exited 1: 53/54 tests passed and
`sched::tests::sched::control::repeated_loss_resends_and_raises_the_transport_diagnostic_once`
still expects one HostTransport diagnostic, while the new OutputStop tracker
correctly reports its unacknowledged Cut in addition to the slot timeout. No
source-only change can preserve both diagnostic schedules and that old
assertion. `src/sched/tests/sched/control.rs` is explicitly restricted to new
rows with existing assertions unchanged, so reconciling the test requires an
ownership clarification. The wasm build and final exact rustfmt gate were not
run after the focused test failure. Song Playing/Draining endpoint behavioral
tests also remain outside the authorized test paths because their genuine
fixture is private to `src/sched/runtime/song/clock_tests.rs`.

### Session: 2026-10-08 (step 6 review repair)
**Tasks Completed**: Added the OutputStop/slot diagnostic de-duplication seam,
two retry-exhaustion regression tests, pure song stop endpoint computation,
`stop_song` delegation, and table-driven Playing/Draining endpoint cases.
**Notes**: The original assertion in
`repeated_loss_resends_and_raises_the_transport_diagnostic_once` was not
edited. OutputStop retries remain enabled; retry exhaustion clears the
request, then defers its diagnostic while slot controls remain outstanding.
It is suppressed if a slot reports HostTransport, emitted if the controls
clear without a report, and canceled if OutputState acknowledges the request.
Endpoint tests assert the same epoch and bounds checked by
`SongTransport::cutoff`. Verification is pending the parent-designated
checker/gates; no pass is claimed here.

### Session: 2026-10-08 (step 6 final-source verification)
**Tasks Completed**: Exact plan rustfmt check passed. Re-ran the exact
all-targets build on the formatted final source; compilation is blocked by
unowned LP-SESSION-MOMENTARY code.
**Notes**: `rustfmt --edition 2021 --check src/sched/runtime/live.rs
src/sched/control.rs src/sched/runtime/song.rs src/sched/tests/sched/live.rs
src/sched/tests/sched/control.rs src/session/tests/live_stop.rs` exited 0
(`tmp/live-perf/session-stop/review-repair-final2-rustfmt.log`).
The first check found formatter-only differences (`review-repair-rustfmt.log`);
rustfmt was applied to the plan's exact touched-file set before this passing
rerun.
`CARGO_TERM_QUIET=true cargo build --all-targets` exited 101
(`tmp/live-perf/session-stop/review-repair-final2-build.log`) with errors in
`src/session/momentary.rs`: wrong `StaleBindingBody` import, private
`Runtime::invalidate_uncommitted` calls, `Box<dyn AudioHost>` coercions and a
mutable-borrow conflict. This file is outside LP-SESSION-STOP writePaths.
Clippy, focused nextest and wasm build could not be meaningfully run until
that compile blocker is repaired. The review findings' source and test
changes are present, but behavioral verification and plan completion remain
open.

### Session: 2026-10-09 (step 6 final-source completion)
**Tasks Completed**: Corrected two regression-test setups, reran verification
1-5 on final sources, and completed the assigned implementation criteria.
**Notes**: `slot_and_output_stop_timeout_share_one_transport_diagnostic` now
sets `ack_delay = None`, ensuring the outstanding-control diagnostic path is
exercised. `output_telemetry_omits_unknown_state_and_publishes_draining`
advances the mock clock to the next transport sample interval before checking
the reported state. Focused nextest passed 63/63 with zero failures
(`tmp/live-perf/session-stop/agent-focused-nextest.log`).
The first focused run exited 100 (61/63 passed, 2 failed;
`tmp/live-perf/session-stop/step6-focused-nextest.log`): it exposed those two
test setup gaps, which were fixed before the passing rerun.

- `CARGO_TERM_QUIET=true cargo build --all-targets` passed
  (`tmp/live-perf/session-stop/final-build-all-targets.log`).
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  passed (`tmp/live-perf/session-stop/final-clippy.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/sched::tests::sched::(live|control)|session::tests::live_stop|sched::runtime::song|host::tests::e2e::sched_gaps|sched::tests::midi/)'`
  passed, 63 tests (`tmp/live-perf/session-stop/agent-focused-nextest.log`).
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  passed (`tmp/live-perf/session-stop/final-wasm-build.log`).
- `rustfmt --edition 2021 --check src/sched/runtime/live.rs src/sched/control.rs src/sched/runtime/song.rs src/sched/tests/sched/live.rs src/sched/tests/sched/control.rs src/session/tests/live_stop.rs`
  passed (`tmp/live-perf/session-stop/final-rustfmt-check.log`).

Formal test-integrity/adversarial/integration reviews, LP-EVIDENCE, shared
closeout, commit, and push remain downstream workflow steps.

### Session: 2026-10-09 (step 6 adversarial repair)
**Tasks Completed**: Repaired `LP-SS-ADV-1-SONG-STOP-OWNER-SELECTION`.
**Notes**: `stop_song` now iterates all owners, skips Ended/Failed owners,
applies bounded endpoints to Prepared owners, invalidates an unposted
uncommitted replacement through `invalidate_replacement`, and records cutoff
refusals and invalidation failures in `rep.faults`. Prepared gentle/cut rows
assert the activation-clamped endpoints while retaining all existing rows.
The first post-change rustfmt check found formatting-only differences
(`tmp/live-perf/session-stop/repair-rustfmt-check.log`); the two owned source
files were formatted and the final check passed. Final focused nextest passed
63/63 (`tmp/live-perf/session-stop/repair-focused-nextest-quiet-complete.log`).

- `CARGO_TERM_QUIET=true cargo build --all-targets` passed
  (`tmp/live-perf/session-stop/repair-build-complete.log`).
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  passed (`tmp/live-perf/session-stop/repair-clippy-complete.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/sched::tests::sched::(live|control)|session::tests::live_stop|sched::runtime::song|host::tests::e2e::sched_gaps|sched::tests::midi/)'`
  passed, 63/63 (`tmp/live-perf/session-stop/repair-focused-nextest-quiet-complete.log`).
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  passed (`tmp/live-perf/session-stop/repair-wasm-complete.log`).
- `rustfmt --edition 2021 --check src/sched/runtime/live.rs src/sched/control.rs src/sched/runtime/song.rs src/sched/tests/sched/live.rs src/sched/tests/sched/control.rs src/session/tests/live_stop.rs`
  passed (`tmp/live-perf/session-stop/repair-rustfmt-complete.log`).

Independent re-review of the repaired finding is pending; no review acceptance
is claimed.

### Session: 2026-10-09 (session 343, operator-authorized scope amendment; plan step)
**Tasks Completed**: Plan and manifest scope amendment only. No source change.
**Notes**:
- Review finding LP-SESS-STOP-TI-2-STOP-SONG-UNTESTED (mid): no test runs
  `Runtime::stop_song` (src/sched/runtime/song.rs:458) with an installed
  song.
- It could not be repaired inside the previous writePaths, for four
  reasons:
  - `Runtime.song` is private (src/sched/runtime.rs:155);
  - `SongRuntime` is `pub(super)` (song.rs:110);
  - `song.rs` is 973 lines;
  - the `genuine_ready` fixture is private to
    `src/sched/runtime/song/clock_tests.rs:165`, which is cfg host-native
    and 369 lines.
- The operator authorized adding `src/sched/runtime/song/clock_tests.rs`
  to this plan, as recorded in the workflowInput of
  opus-luna-design-and-implement-review-loop-session-343. It was added to:
  - writePaths and the new trackedPaths section above;
  - the LP-SESSION-STOP entry (`writePaths`, `trackedPaths`) in
    `impl-plans/active/live-perf-dispatch.json`;
  - the rustfmt gate (Verification 5).
- TASK-S5 and Verification 6-7 were added.
- The historical verification records in the earlier entries above are
  left unchanged, because they describe what those sessions actually ran.
- The design is reused unchanged (design-live-performance.md D5, 4.4,
  8.1(5)).
- The checkpoint commit follows this entry. Only LP-SESSION-STOP (TASK-S5)
  is redispatched, then LP-EVIDENCE.

### Session: 2026-10-10 (step 6 TASK-S5 implementation)
**Tasks Completed**: Added genuine installed-song host-native gentle, cut,
and refusal tests; completed final-source verification 1-7.
**Notes**:
- `genuine_ready_with` accepts the test source; existing `genuine_ready`
  delegates using its original source string. The bounded fixture helper
  drives the real headless native host until the installed song is Playing.
- `stop_song_gentle_drains_installed_song_until_tail_deadline` asserts the
  exact bounded endpoint formula and epoch, observes Draining at/after the
  arrangement endpoint, then Ended at/after the tail deadline with no faults.
- `stop_song_cut_ends_now_and_clears_output_after_bounded_fade` proves the
  song is audible before cut, asserts the clamped immediate endpoints and
  epoch, then verifies exact output zeros after 64 + 128 frames and Ended.
- `stop_song_refusal_is_fault_and_later_stop_succeeds` temporarily swaps in
  the existing refusing `CachedHost`; it verifies exactly one
  BeyondCapability fault, unchanged endpoints and Playing state, then restores
  the exact native host and confirms a later gentle stop succeeds.
- The first focused attempt exited 100 (0/3 passed) because it assumed the
  transport state changed on command submission and indexed a retired owner;
  `task-s5-focused-nextest.log` records the complete run. The second focused
  attempt exited 100 (1/3 passed) because Draining is reached only after the
  host clock advances to the arrangement endpoint;
  `task-s5-repair-focused-nextest.log` records the complete run. Tests were
  corrected to follow engine clock/acknowledgment transitions, and the final
  source-matched reruns below pass.
- `CARGO_TERM_QUIET=true cargo build --all-targets` passed
  (`tmp/live-perf/session-stop/task-s5-final2-build-all-targets.log`).
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  passed (`tmp/live-perf/session-stop/task-s5-final2-clippy.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/sched::tests::sched::(live|control)|session::tests::live_stop|sched::runtime::song|host::tests::e2e::sched_gaps|sched::tests::midi/)'`
  passed 66/66 (`tmp/live-perf/session-stop/task-s5-final2-combined-nextest.log`).
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  passed (`tmp/live-perf/session-stop/task-s5-final2-wasm-build.log`).
- `rustfmt --edition 2021 --check src/sched/runtime/live.rs src/sched/control.rs src/sched/runtime/song.rs src/sched/runtime/song/clock_tests.rs src/sched/tests/sched/live.rs src/sched/tests/sched/control.rs src/session/tests/live_stop.rs`
  passed (`tmp/live-perf/session-stop/task-s5-final2-rustfmt-check.log`).
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib -E 'test(/sched::runtime::song::clock_tests::stop_song_/)'`
  passed 3/3 (`tmp/live-perf/session-stop/task-s5-final2-focused-nextest.log`).
- `git diff --exit-code HEAD -- src/sched/runtime/song.rs src/sched/runtime.rs src/sched/runtime/live.rs`
  passed (`tmp/live-perf/session-stop/task-s5-final-product-scope.log`);
  `wc -l src/sched/runtime/song/clock_tests.rs` reports 612 lines
  (`tmp/live-perf/session-stop/task-s5-final2-line-count.log`).
- No product file changed. The test file remains below the plan's 700-line
  target. Formal reviews, LP-EVIDENCE, shared closeout, commit, and push remain
  downstream workflow steps.
