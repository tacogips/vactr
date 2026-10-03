# Song Runtime and session integration implementation plan

**Plan ID**: SONG-11B
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application), [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Scope and related plans

Connect the complete finite transport core to actual Runtime and session Apply,
start, mute and publication paths. Preserve generated reusable Parts, static
sequence/repeat progression, selective edits and complete-song output as the
full goal. Browser protocol and native export consumers remain required.

- **Parent**: [SONG-11](song-mode-transport.md).
- **Previous / Depends on**: [Transport core](song-mode-transport-core.md), [Host preparation](song-mode-host-preparation.md).
- **Next**: [Browser session](song-mode-browser-session.md), [CLI](song-mode-cli.md), [Export](song-mode-export-core.md).

| Dependency | Required output | Current status |
|---|---|---|
| Transport core | Actual owned finite progression and retirement | Implementation in progress in disjoint scheduler modules |
| Host preparation | Genuine original Ready handoff and activation | Owner verification passed; activation correction in progress |
| Session/protocol | Existing actual Apply/mute requests and publication schema | Source sites inspected; compatibility review required |

## Exact future Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/sched/runtime.rs` | Retain transport/preparation state and connect tick/drain | In Progress |
| `src/sched/runtime/song.rs` | Cohesive song receipt, preparation and transport integration | In Progress |
| `src/session/session.rs` | Dispatch actual whole-code Apply and mute entrypoints | In Progress |
| `src/session/song.rs` | Transfer original pending PreparedSong into consuming preparation | In Progress |
| `src/session/song/transport.rs` | Cohesive candidate lifecycle, Apply and mute helpers | In Progress |
| `src/session/publish.rs` | Separate candidate-ready/applied and finite-state publication | In Progress |
| `tests/song_transport.rs` | Actual progression, Apply/mute, rollback, pressure and legacy compatibility | In Progress |
| `src/dsp/engine/song_runtime.rs` | Earlier endpoint replacement and cancellation of superseded queued work for Apply | In Progress |

Eight Rust paths. Runtime900, session832 and session/song912 require cohesive
child extraction before implementation growth; every touched source stays
below1000. Preserve all existing behavior and same-owner baselines when moving
methods. New source or protocol.rs changes require root manifest amendment.
Legacy commit.rs is not an integration write path; core owns frozen encoding.

## Public declaration contract

```rust
impl Runtime {
    pub fn start_song(&mut self, ready: SongReadyBundle, activation_frame: u64)
        -> Result<(), SongTransportRefusal>;
    pub fn mute_instrument(&mut self, epoch: SnapshotEpoch,
        selector: InstrumentSelector, muted: bool) -> Result<(), Failure>;
    pub fn song_state(&self) -> Option<SongTransportState>;
}
```

The refusal returns the same Ready owner. Integrate through current Runtime host
handles and real session pending candidate; diagnostic Rc<Song> is not authority.

## Required behavior

- One bounded ACK dispatcher routes exact old draining/new pending/active owners,
  preserving unconsumed messages. Current one-epoch SongReceipts cannot silently
  replace expected_epoch across Apply and discard old lease returns.
- Use actual correlated SongHostClock observations for activation and progression.
  Legacy host_now f64 is not an authenticated integral song frame. Preserve legacy
  transport epochs/end_time and MIDI restart generations as separate identities.
- Consume isolated whole-code candidates and prepare every resource before start.
  Reject mixed legacy slots/song and tempo changes as designed. Preserve legacy
  slot tick, commit, transport and at-thunk behavior for legacy documents.
- Apply selects next integer cycle beyond commit lead and old 64-frame fade;
  restarts at local zero. Old onsets at/after boundary are cancelled, old song
  private/master tails are cleared, and new audio fades in. Actual Applied alone
  publishes applied epoch/revision/frame. Failed preparation leaves old song intact.
- Mute matches frozen instrument families across all tracks and uses actual
  application ACK/frame. Consume skipped occurrences; reset private state and
  gate precommitted audio, so unmute cannot replay stale notes/tails.
- Remap overlay identity across snapshots by certified instrument/source identity,
  never reused integer InstId/FileId. Drop removed/uncertifiable families; preserve
  remaining families under declaration reorder. Static export ignores live overlays.
- Fault stops new onsets and reports Failed without killing editor session;
  retained resources continue through actual safe cleanup.
- Confirm existing public reply schema before publishing new state. If insufficient,
  split an explicit protocol prerequisite; do not silently expand this manifest.

## Tasks

### TASK-001: Cohesive extraction and bounded owner dispatcher
**Status**: In Progress
**Parallelizable**: Yes with core implementation in disjoint files; coordinate public API, verify together before acceptance.
- [ ] Record fresh baselines and extract only cohesive song helpers.
- [ ] Preserve legacy APIs; retain exact multi-owner ACK obligations under pressure.

### TASK-002: Actual start, Apply, mute and publication
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this manifest.
- [ ] Replace explicit unavailable branches with complete production integration.
- [ ] Complete all required timing, ownership, rollback and overlay behavior.

### TASK-003: Production session witnesses
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No; verifies complete held source.
- [ ] Actual Runtime/session song progresses through parts/repeats and terminates.
- [ ] Real Apply during playback/drain, failed candidate rollback, acknowledged mute/unmute.
- [ ] Reordered/renamed custom instruments and relative sample origins cannot mis-mute.
- [ ] Cross-epoch delayed ACKs/returns, queue pressure, failure and legacy/MIDI regressions.
- [ ] Native/wasm/strictClippy, scoped format/line limits, full nonempty test inventories.

## Completion criteria

- [ ] All module deliverables and full parent criteria verified.
- [ ] Actual public behavior and original held hashes/process logs reconciled.
- [ ] Browser playback and complete streaming export remain tracked by next plans.
- [ ] No full goal completion claim before all required consumers are verified.

## Progress log

### 2026-10-02 — ROOT0534 bounded integration split

Source audit found actual Runtime PlaySong and Session ApplySong/mute still report
unavailable. Existing large files and one-epoch receipt state require bounded
cohesive integration. Planning only; no Rust edits or implementation release.

### 2026-10-02 — Production integration source release

Root releases the eight declared Rust paths. Implement against the existing
original-candidate owner and coordinated transport core API. Core owns only
sched/song modules and sched/mod.rs; activation owns host/DSP changes. This
author exclusively owns runtime/session integration files. Coordinate APIs
directly rather than adding competing transport state machines. Final acceptance
requires focused combined verification after all sources are held.

Deliver the real PlaySong/Apply path and preserve the full stated Apply/mute
requirements. Browser/CLI/export remain subsequent required consumers. No Cargo
execution by authors; the independent checker runs it after joint hold.

Implementation assignment is queued until an existing Rust author finishes its
current source scope; the available agent thread limit prevented another author.
No integration implementation is claimed by this release.

The existing rust-coding author is now assigned after holding the activation
repair and immutable sample accessor. Implementation proceeds in the eight
declared integration paths, coordinated with the core author.

### Apply endpoint replacement: manifest correction

The actual core schedules finite endpoints ahead. apply_song_end currently
rejects a second endpoint and retains the obsolete future endpoint, preventing
an earlier Apply cutoff from replacing the original finite end. Root replaces
the not-yet-created separate Apply test module with engine/song_runtime.rs in
this eight-module phase; all Apply tests belong in song_transport.rs.

Implement a checked earlier cutoff which never extends an existing finite end,
removes superseded future endpoints and cancels subsequent onsets/rebinds/releases
for the ended epoch. Preserve failed retained receipts and exact full-key normal
retirement. Coordinate transport cutoff and projected-receipt cleanup with core
author. Successful whole-code replacement must observe original rollback and
64-frame fade semantics; early cutoff alone is not proof of atomic Apply.
This source release takes effect only after the current early core check ends.

### Production declarations before source

Runtime adds `prepare_song(prepared: PreparedSong, limits: SongPreparationLimits)
-> Result<(), SongPreparationRefusal>`, consuming original authority and retaining
its exact cleanup. `start_song` consumes Ready into actual SongTransport; activation
is posted in the tick driver, with refusal retained rather than reconstructing Ready.
The child owns at most four running/draining epochs and one pending preparation,
actual correlated clock requests and bounded notices. Runtime uses real AudioHost
drain for ownership dispatch; external legacy receipt APIs remain available.
Session child consumes pending PreparedSong, never Runtime's diagnostic Rc<Song>.
Earlier Engine Endpoints replaces its own later queued endpoint/onsets and never
extends a closed cap. Existing mute reply-schema gap remains separately tracked.

### Initial Session consuming playback checkpoint

Written actual isolated Apply candidate transfer into Runtime host preparation,
explicit public preparation-limit configuration, and asynchronous Ready/Applied/
Failed routing using the original connection and request sequence. The Runtime
retains old and new resource owners until actual guarded cleanup; Session removes
request correlation only after notices are routed and the runtime owner is gone.
Parsed, evaluated-form and loaded non-error diagnostics are copied once into the
candidate warning metadata. Added genuine Native Session A-repeat → B → finite
stop fixture with actual PCM and correlated receipt assertions. Scoped format
passes; no author Cargo. This checkpoint is not full TASK completion.

Pending: document-edit invalidation after candidate handoff must retain exact
file/revision metadata and cancel only unactivated owner state. Atomic Apply must
coordinate new execution success with old fade/cut, not cut old music ahead of a
possibly rejected activation. Mute overlay remapping and dedicated acknowledged
publication remain mandatory. These gaps are not proven by initial playback.

### Verified initial playback and document invalidation implementation

Independent focused checks passed actual Session1/export5/writer1/warning1,
including original_session_candidate_repeats_changes_and_stops_with_correlated_acknowledgments
and accepted_candidate_warnings_survive_preparation_and_query. Initial Session
command terminal32354/0; the checker retained the original focused logs. These
prove initial finite playback and warning propagation, not atomic Apply/mute.

Correlated request storage now retains the original file, revision and edit epoch.
DocChanged cancels only Runtime preparation or Ready/transport whose activation
has never been submitted. Ready cancellation consumes its actual cleanup through
SongHostPreparation::retire; pending cancellation retains its original in-flight
uploads, leased resources and receipts until guarded return. Submitted/uncertain
or active epochs remain intact. A retained invalidation outcome is published once
before cleanup can erase its request identity, including notice backpressure.

Written delayed-host race fixture covers edits before the first host reply and
after eight actual staging callbacks, duplicate notifications, absence of Ready/
Applied/audio for the cancelled candidate, then actual fresh preparation/playback.
The original finite fixture now also verifies edits preserve already-applied audio.
Tests await independent execution; no author Cargo. Atomic replacement/fade and
acknowledged mute/remapping remain pending in this plan and publication phase.

Cancellation focused test binary passed2/2, original70678 terminal0,
/tmp/vactr-session-transport-doc-change-001.log. Added standalone preparation
failure state retention for actual direct consuming CLI ownership: no active
transport means Failed persists after cleanup, while an existing transport's
state remains authoritative. Genuine insufficient measured bus-capacity test
uses direct submit_prepared_song and verifies silence plus finite Failed state.

### 2026-10-03 — Exact catalog type declaration before lint repair

Authorized single existing manifest source src/session/song/transport.rs.
Declare public type FrozenAppliedSongCatalog = (Vec<FrozenInstrument>,
FrozenCellInventory, FrozenGraphResources) using existing public frozen DTOs;
private RetainedAppliedSongCatalog = (SnapshotEpoch, Rc<FrozenAppliedSongCatalog>).
SongRequests.applied becomes Option<RetainedAppliedSongCatalog>; existing public
applied_song_catalog getter returns Option<(SnapshotEpoch, &FrozenAppliedSongCatalog)>.
Type aliases preserve exact underlying tuple/Rc/borrow, metadata remains immutable
and published only after authentic Applied. No extra paths, lint allowance or
behavior changes. Actual strict lint reported only these two type_complexity
sites; independent verification pending. Source author uses scoped rustfmt, no
Cargo. Original full integration criteria remain incomplete.
