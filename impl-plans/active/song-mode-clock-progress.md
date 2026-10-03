# Actual song clock progress

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Song realization and playback](../../design-docs/specs/design-song-mode.md)

## Purpose

Refresh actual correlated audio-side clock observations throughout finite
playback. A browser cached observation must not freeze the realization horizon.
Retain the last authentic frame while a newer report is pending. No conversion
of JavaScript time into an audio frame, wire change, or prediction is permitted.

## Exact modules and declarations

Three Rust write paths; every touched file must remain below 1000 lines.

1. `src/sched/runtime/song.rs`

```rust
struct ClockProgressFailure {
    epoch: Option<SnapshotEpoch>,
    failure: Failure,
}
impl SongRuntime {
    fn receive_clock(&mut self, ack: SongHostAck) -> bool;
    fn refresh_clock(&mut self, host: &mut dyn AudioHost,
        epoch: Option<SnapshotEpoch>) -> Result<(), ClockProgressFailure>;
}
impl Runtime {
    fn fail_song_clock(&mut self, error: ClockProgressFailure, rep: &mut TickReport);
    pub(super) fn song_enabled(&self) -> bool;
}
```

The existing Runtime dispatch consumes only the exact outstanding request.
Actual reports preserve monotonic frame/rate validation; newer native direct
observations are retained if a queued report describes an earlier real frame.
One refresh is outstanding; refused Backpressure does not advance its nonce.
Do not issue runtime refreshes while a preparation owner is using barriers.
An already accepted older-owner refresh is still dispatched during preparation;
FIFO receipt processing precedes a new preparation barrier. An outstanding
refresh retains song receipt dispatch even after the final owner retires. Exact ClockRejected
clears only that pending request and publishes an explicit failure. An exact
malformed report also clears its request and fails the actual runtime owner,
retaining cancellation/normal-retirement authority; no silent cached playback.
Request-specific failure retains its exact epoch and affects only that owner.
Invalid direct host rate is a global timebase failure, affecting all owners.

2. `src/host/caps/song/preparation.rs`

```rust
impl SongReadyBundle {
    pub fn clock_request_floor(&self) -> u64;
}
```

Return retained cleanup.nonce, the checked preparation barrier counter.
Ready can only be issued after all control flights complete. This safe floor
also permits consuming direct Ready without guessing the provider nonce.
Runtime start_song updates its counter from this original authority.

3. `src/sched/runtime/song/clock_tests.rs`

```rust
fn cached_observation_refreshes_with_exact_nonce_and_retained_pending_clock();
fn refused_refresh_retries_same_nonce_and_preserves_foreign_reports();
fn rejected_refresh_is_correlated_and_nonce_exhaustion_is_explicit();
fn actual_native_clock_advances_while_a_report_is_pending();
fn exact_invalid_clock_aborts_original_runtime_owner_and_retains_cleanup();
fn rejected_new_epoch_clock_preserves_other_genuine_ready_owner();
fn genuine_ready(audio: &mut NativeAudioHost, side: &mut AudioSide,
    caps: CapabilitySet, epoch: SnapshotEpoch) -> SongReadyBundle;
```

Private production helper calls test cached clock, exact requests, stale/foreign
reports, actual refusal and checked nonce exhaustion. Native fixture uses the
actual configured audio-side callback and exact clock, not a mock frame.

## Tasks and dependencies

| Task | Status | Depends on |
|---|---|---|
| TASK-001: original Ready floor and correlated refresh | Written, held | Existing clock/owner authority |
| TASK-002: focused production helper and Native witnesses | Written, held | TASK-001 |
| TASK-003: independent native/wasm/browser verification | Pending | Held source |

Browser three real ABI fixtures remain owned by the parent evidence plan.
Full atomic Apply and wider song completion remain separate requirements.

## Completion criteria

- [x] Original Ready and Session preparation both supply a valid nonce floor.
- [x] One actual request at a time; no clock loss while pending or pressured.
- [x] Stale/foreign reports retained, exact failure correlation preserved.
- [x] Native actual frame progress verified.
- [ ] Fresh wasm compilation and actual browser 3 fixtures: Applied, nonzero PCM, Ended.
- [ ] Scoped format and independent strict checks pass.

## Progress

2026-10-03: Source-derived failure: cached Wasm song_clock succeeds forever;
Runtime requests only on failure. Activation is cached frame plus lookahead,
so the same observation gives a zero query horizon forever. Endpoints retire
resources without any Events. Correct-epoch LeaseReturned cannot be consumed
while the transport still holds Ready. Actual failed browser logs retained;
no claim of empirical repair yet. Before source intent: SONG-CLOCK-PROGRESS/0001.

2026-10-03 source checkpoint: exact three Rust paths written and scoped rustfmt
check passed. Six tests written (not executed): cached/pending, refusal and
foreign report, correlated rejection/exhaustion, actual Native clock progress,
genuine original Ready Runtime invalid report cleanup, and two genuine Native
Ready owners with actual old Applied followed by adversarial exact new-request
rejection. New epoch failure preserves the old Playing owner. Final pending
refresh remains owned after musical retirement. No Cargo author commands.
Independent native/wasm checks and fresh browser three fixtures remain pending.

Independent focused evidence: clock 6/6 PASS, original83639/0 chunkda3ab3,
/tmp/vactr-clock-progress-tests-002.log. Native check0; existing activation4,
atomic4, hostclock7 all PASS original45088/0 chunk4911e8,
/tmp/vactr-clock-wire-codecs-001.log. Pure wasm build10656 terminal0; fresh
artifact SHA e71d596eb866b46fab19acc10088bbeea886ebf1430d3ba92c09d581e0157956.
Actual browser ABI fixtures remain pending; no full playback/Apply acceptance.
