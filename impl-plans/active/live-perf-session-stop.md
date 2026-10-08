# LP-SESSION-STOP: Scheduler stop modes, output-stop delivery, song stop, output telemetry

**Status**: Ready (after LP-CONTRACT)
**Plan ID**: LP-SESSION-STOP (wave 2; parallel with LP-ENGINE, LP-SESSION-MOMENTARY, LP-EDITOR-STOP, LP-EDITOR-MOMENTARY)
**Design Reference**: `design-docs/specs/design-live-performance.md` 4.1, 4.4, 4.5, 4.6, 8.1(5), D1, D2, D5
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Created**: 2026-10-08
**Last Updated**: 2026-10-08

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
- `src/sched/runtime/song.rs` (adds `stop_song` only)
- `src/sched/tests/sched/live.rs`
- `src/sched/tests/sched/control.rs` (new rows only; existing assertions unchanged)
- `src/session/tests/live_stop.rs`
- `impl-plans/active/live-perf-session-stop.md` (Progress Log only)
- Artifact roots (also in the manifest's `artifactRoots`): `target`,
  `tmp/live-perf/session-stop`

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
- Keep every touched file under 1000 lines. `runtime/song.rs` is 890 lines
  today: if `stop_song` would push it over, put it in `live.rs` and call
  only the existing public transport accessors.

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
5. `rustfmt --edition 2021 --check src/sched/runtime/live.rs src/sched/control.rs src/sched/runtime/song.rs src/sched/tests/sched/live.rs src/sched/tests/sched/control.rs src/session/tests/live_stop.rs`

Write logs to `tmp/live-perf/session-stop/*.log`.

## Completion Criteria

- [ ] TASK-S1 to TASK-S4 are implemented.
- [ ] Every listed test passes. Existing control, MIDI and sched_gaps
  assertions are unchanged and pass.
- [ ] `stop-all` gives Natural plus Gentle; hush gives Panic plus Cut; a
  per-slot stop gives Natural only.
- [ ] Verification 1-5 exit 0. Touched files are under 1000 lines. The
  Progress Log is updated.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.
