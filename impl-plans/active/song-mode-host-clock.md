# Exact song host clock implementation plan

**Status**: Completed
**Plan ID**: SONG-10-CLOCK
**Created / Last Updated**: 2026-10-02
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Purpose

Expose the actual rendered frame and sample rate to the finite song owner.
Native FrameClock already has exact integer observations. Browser HostState.now,
session_tick and worklet_now currently expose floating seconds; they cannot
authenticate a full-width frame. This child supplies the clock prerequisite to
preparation and transport; it does not complete playback.

Source-grounded review: tmp/song-mode-riela/SONG-10/0003-clock-neutral-preparation-preflight.md
and 0004-measured-capacity-owner-addendum.md. The initial preparation owner will
obtain a complete measured capacity report through the existing explicit public
RequestCapacity command and its actual forwarded receipt. It will not reconstruct
report serials from scalar native getters.

## Manifest

```json
{
  "planId": "SONG-10-CLOCK",
  "planPath": "impl-plans/active/song-mode-host-clock.md",
  "dependsOn": ["SONG-10-COMMANDS", "SONG-09PC"],
  "writePaths": [
    "src/song/routing.rs",
    "src/host/caps.rs",
    "src/host/caps/song.rs",
    "src/dsp/engine/song.rs",
    "src/host/native/audio.rs",
    "src/host/wasm/messages.rs",
    "src/host/wasm/messages/song.rs",
    "tests/song_host_clock.rs",
    "impl-plans/active/song-mode-host-clock.md"
  ],
  "sharedPaths": [
    "src/host/caps.rs",
    "src/host/caps/song.rs",
    "src/host/native/audio.rs",
    "src/host/wasm/messages.rs",
    "src/host/wasm/messages/song.rs"
  ]
}
```

## Related plans and dependencies

- **Previous / Depends On**: [Checked commands](song-mode-host-command-submission.md).
- **Parent**: [Host adapters](song-mode-host-adapters.md).
- **Next**: neutral stage materialization, actual preparation ownership, then
  [finite transport](song-mode-transport.md).

| Dependency | Required evidence | Status |
|---|---|---|
| SONG-10-COMMANDS | sealed checked sender, actual native/browser pressure and ownership checks | Completed; ROOT0317 |
| SONG-09PC | established POD framing and retained critical receipts | existing accepted foundation |
| Root release | current hashes, exact serial ownership and terminal joined checker | ROOT0327 scoped acceptance |

No source authorization is supplied by this document. Implement only after the
sender's independent clearance and a fresh root release. Full SONG-09 and route
geometry acceptance remain required for parent playback. Do not modify the
sender while its checker is running. No additional Rust path is implicit.

## Modules and public declarations

| File | Deliverable | Status |
|---|---|---|
| src/song/routing.rs | exact clock/request/report/failure DTOs and exhaustive command/receipt matches | Completed; ROOT0327 |
| src/host/caps.rs | unsupported clock observation default; reexport DTO | Completed; ROOT0327 |
| src/host/caps/song.rs | append clock codec entries, preserve existing bytes | Completed; ROOT0327 |
| src/dsp/engine/song.rs | compact actual-frame dispatch using existing critical receipt retention | Completed; ROOT0327 |
| src/host/native/audio.rs | FrameClock observation and checked read-only request admission | Completed; ROOT0327 |
| src/host/wasm/messages.rs | bounded pending/report clock state owned by HostState | Completed; ROOT0327 |
| src/host/wasm/messages/song.rs | admitted-nonce tracking and actual receipt observation | Completed; ROOT0327 |
| tests/song_host_clock.rs | actual Native/ByteInbox clock, pressure and compatibility witnesses | Completed; ROOT0327 |

```rust
pub struct SongHostClock { pub frame: u64, pub sample_rate: u32 }
pub struct SongClockRequest { pub epoch: SnapshotEpoch, pub request: u64 }
pub struct SongClockReport { pub request: SongClockRequest, pub clock: SongHostClock }
pub struct SongClockFailure { pub request: SongClockRequest, pub reason: SongRejectCode }
pub trait AudioHost {
    fn song_clock(&self) -> Result<SongHostClock, Failure>;
}
```

All names use existing snapshot/routing/failure types. Extend SongCommand and
SongHostAck with typed clock request/report/failure variants. Append unused codec
subtags only after confirming the current inventory; preserve all previous tags,
lengths and field order. No f64-to-u64 recovery, editor wall clock, new worklet
export or new dependency. The existing command sender remains the admission API.

## Invariants

- Native observation reads FrameClock.frames/sample_rate. Browser observation is
  copied from a matching actual Engine.frame/configured rate report; it is pending
  until that report arrives. A getter alone does not enqueue a request.
- Only accepted RequestClock updates the browser pending nonce. Refusal preserves
  the command and previous ownership; a retry does not allocate an unbounded stash.
  Match epoch and nonce. Foreign/stale receipts remain available to their owner.
- Capture frame at actual audio command consumption, not submission. Retain a
  success or rejection under full ACK pressure through the established critical
  receipt guard; do not fabricate Ready/Applied or consume capacity receipts.
- The read-only checked native clock command must not reserve a capacity query
  origin or invalidate a measured resource report. The owned native helper may
  enqueue this read-only command directly; existing legacy APIs retain semantics.
- Pending reports are bounded and never regress the authenticated frame. A rate
  mismatch, unavailable host, exhausted nonce or invalid record is explicit.
- Keep engine/song.rs below1000 lines; its current911 lines leave room for a
  compact dispatch. If actual integration needs a split or exhaustive consumer
  outside these eight paths, obtain a prior bounded amendment.
- All callback operations are allocation-free and VM-free. No Git/index/archive,
  dependency or broad formatting changes. Use immutable before/after receipts and
  quiet Cargo via mise; preserve original process handles to terminal.

## Tasks

### TASK-001: Exact carrier and actual engine report

**Status**: Completed
**Parallelizable**: No; acquire the frozen sender interfaces first.

- [x] Reconcile current enum consumers and unused tags before source release.
- [x] Add full-width typed codec and actual engine report/rejection retention.
- [x] Preserve all prior commands and receipts; no accidental timed-event routing.

### TASK-002: Actual host observations

**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No; shared host modules.

- [x] Native reads actual FrameClock and preserves read-only capacity correlation.
- [x] Browser tracks only admitted requests and matching actual reports.
- [x] Unsupported and pending states are explicit; no floating clock reconstruction.

### TASK-003: Independent behavioral evidence

**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No; seal both paths before checking.

- [x] Actual native rendered increments/rate; genuine browser sender→ByteInbox→Engine reports.
- [x] Full-width epoch/nonce/frame codec round trips, truncation and following record isolation.
- [x] Real full command/ACK queues, retained report, one retry and no duplicated admission.
- [x] Foreign/stale nonce, unavailable/pending getter and rate mismatch witnesses.
- [x] Existing sender/carrier/capacity/lifecycle regressions remain enabled.
- [x] Fresh nonzero inventories equal actual executions; original handles and hashes recorded.

## Completion criteria

- [x] All eight module deliverables and three tasks independently verified.
- [x] Native, host-wasm, strict Clippy, scoped formatting and relevant actual tests pass.
- [x] Native/browser pressure, exact clock and receipt ownership behavior proven.
- [x] All source seals reconcile; no live checker handles remain.
- [x] No full preparation, transport, export or goal completion claim from this child.

## Progress log

### Session: 2026-10-02 — root document preparation

Read the actual clock, sender and preparation interfaces. Selected the eight-path
manifest that retains browser parent state ownership and compact same-file engine
dispatch. Source implementation is pending sender clearance and a fresh release.
No Cargo or Rust execution by this plan.

### Session: 2026-10-02 — native trait ownership amendment

Trade native/audio/song.rs for native/audio.rs in the eight-path manifest.
The actual AudioHost implementation resides in the parent, so its clock getter
and checked read-only RequestClock admission must be implemented there. Parent
currently has 897 lines; keep it below1000. The child remains read-only.
Sender prerequisite is independently verified in ROOT0317. Source release
ROOT0323 supersedes ROOT0320 before any clock Rust changes.

### Session: 2026-10-02 — ROOT0323 implementation batch

Prior source intent: SONG-10-CLOCK/0002-carrier-host-source-before.json. Implement bounded `HostState::observe_song_clock(&mut self, SongHostAck)` and `WasmAudioHost::song_clock_observation(&self) -> Result<SongHostClock, Failure>`; only matching admitted requests update results, preserving authenticated rate and nondecreasing frame. Native actual trait getter and checked read-only enqueue live in the amended parent path. Codec command17/ACK11/ACK12 append exact18/30/19 bytes. No Cargo until held; behavioral verification pending.

Bounded browser nonce refinement (0003 prior intent): retain one last admitted `SongClockRequest`; reject same-epoch nonincreasing nonce before enqueue, including explicit exhausted-u64 rejection, without changing state. Different epoch ownership remains explicit. No automatically generated nonce or allocation. Tests will exercise real EngineIo and portable browser sender.

### Session: 2026-10-02 — coherent clock source held readiness

Seven production modules and new public clock fixture now implement the carrier, actual native trait observation, read-only native admission, actual Engine.frame/rate dispatch, and bounded browser correlation. Seven new public test functions cover genuine Native/ByteInbox increments, ACK/control pressure, read-only capacity cache, full-width codec/truncations/old bytes, failed enqueue unchanged state, foreign/stale nonce, rate/frame mismatch and exhaustion. These tests have NOT executed. Prior sender/staging/geometry suites remain required; known sampled Rev failure retained separately. No Cargo run by this author. Scoped formatting and held SHA receipt follow; all phase completion criteria remain unchecked.

### Session: 2026-10-02 — initial independent clock fixture recovery

Original checker2916 terminal101, poll382034: native/wasm/strict Clippy pass; DSP648/audio14 pass; actual clock7 contains6 pass and1 malformed-record recovery timing failure. Raw log retained at `/tmp/vactr-song10-clock-independent-001/song-host-clock-tests.log`. Fixture-only intent0008 preserves established ByteInbox semantics: rejected first record ends that callback, following valid report arrives exactly next callback at frame16, once. Production bytes and all audio/rate/correlation assertions unchanged. No author Cargo; renewed mandatory verification pending.

### Session: 2026-10-02 — independently verified exact clock child

ROOT0327 reconciles actual seven clock tests and seven unique nextest PASS
identities, all66 held hashes and raw logs. Native/host-wasm/strict Clippy,
DSP648, song audio14, sender/private ownership and staging22 regressions pass.
Original002 session38518 exited101 only at the retained sampled Rev route
failure (38 pass/1 fail). Finish-only and supplemental nextest exited0, chunk
540b8d; max-lines/scoped formatting/diff checks pass. Receipts are in
/tmp/vactr-song10-clock-independent-002 and
/tmp/vactr-song10-clock-finish-independent-001. First001 codec fixture failure
is retained; revised test proves real next-callback frame16 and no duplicate.
Clock child is Completed; no full50-gate pass, full DSP phase, host preparation,
transport, playback/export or goal completion is claimed. Archive/index moves
remain deferred to SONG16. Next: real neutral stages and preparation ownership,
with sampled source whole/geometry separately required.
