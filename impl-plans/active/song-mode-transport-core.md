# Finite song transport core implementation plan

**Plan ID**: SONG-11A
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export), [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Scope and related plans

Consume the genuine SongReadyBundle and run its original private evaluator through
finite arrangement progression, exact host frames, private route generations and
normal resource retirement. This supplies the shared playback engine required by
live playback and streaming export. No finite duration is replaced by a manual
cycle count. The full parent requirements remain authoritative.

- **Parent**: [SONG-11](song-mode-transport.md).
- **Depends on**: [Host preparation](song-mode-host-preparation.md), [Candidate evaluation](song-mode-candidate-evaluation.md), [Route preparation](song-mode-route-preparation.md).
- **Next**: [Runtime/session integration](song-mode-transport-integration.md), [Streaming export](song-mode-export-core.md).
- **Required prerequisite**: [Correlated activation](song-mode-activation-correlation.md).
- **Required asset prerequisite**: [Checked sender sample admission](song-mode-sample-admission.md).

| Dependency | Required evidence | Current status |
|---|---|---|
| Host preparation | Original Ready ownership, actual pools/banks/sample mapping and cleanup | Focused owner verification passed; original Ready ownership available |
| Candidate and routes | Actual public callable routes, timing/partition invariance and complete geometry | Joined verification and remaining general cases required |
| Audio lifecycle | Actual Activated/Rebound/safe-return behavior under pressure | Existing source; full transport witnesses pending |
| Initial onset correlation | Exact frame-zero pipeline with authenticated activation outcome | Activation implementation in progress; independent core modules released concurrently |

## Exact future Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/sched/song.rs` | Owned finite transport and public lifecycle API | NOT_STARTED |
| `src/sched/song/frames.rs` | Exact arrangement positions and absolute host frames | NOT_STARTED |
| `src/sched/song/encode.rs` | Frozen mono/chord event encoding with certified private resource mappings | NOT_STARTED |
| `src/sched/song/pools.rs` | Logical configuration to physical generation lifecycle | NOT_STARTED |
| `src/sched/song/receipts.rs` | Exact owned receipt routing and retirement evidence | NOT_STARTED |
| `src/sched/mod.rs` | Register the finite transport module | NOT_STARTED |
| `tests/song_transport_core.rs` | Genuine Native/Arena core, silence, boundary and pressure witnesses | NOT_STARTED |

Seven Rust paths. Own progress changes are confined to this plan. Every touched
Rust file stays below1000 lines; new paths require an explicit root amendment.
Legacy commit.rs remains read-only: its live Clock, mutable Value, SampleTable
and global ControlCells are not frozen song/private-bank authority.

## Public declaration contract

```rust
pub enum SongTransportState { Prepared, Playing, Draining, Ended, Failed }
pub struct SongTransport;
pub struct SongTransportRefusal { pub ready: SongReadyBundle, pub failure: Failure }
pub struct SongTransportProgress { pub state: SongTransportState, pub committed: usize }
impl SongTransport {
    pub fn new(ready: SongReadyBundle, activation_frame: u64, limits: SongLimits)
        -> Result<Self, SongTransportRefusal>;
    pub fn submit_activation(&mut self, host: &mut dyn AudioHost)
        -> Result<(), SongCommandRefusal>;
    pub fn receive(&mut self, ack: SongHostAck) -> Result<(), SongHostAck>;
    pub fn advance(&mut self, host: &mut dyn AudioHost, clock: SongHostClock,
        through: TimeSpan) -> Result<SongTransportProgress, Failure>;
    pub fn mute(&mut self, host: &mut dyn AudioHost, mute: SongMute)
        -> Result<(), SongCommandRefusal>;
    pub fn state(&self) -> SongTransportState;
}
```

SongTransport is opaque; its declaration does not prescribe a unit implementation.
Use actual existing crate types/imports. Public refusals retain original ownership.
No generic framework, new dependency, callback VM or callback allocation.

## Required behavior and completion authority

- Keep original candidate, route proof, pools, banks, sample source associations
  and cleanup ledger owned exactly once. Do not reconstruct authority from IDs.
- Activate exclusively until actual Applied or correlated Rejected. Preserve
  foreign/stale ACKs and every refused POD through retry. Enqueue is not Applied.
- Query lazily with bounded storage and work, deterministic seeds and complete
  route/source identity. Avoid duplicate onsets across query partitions/retries.
- Map absolute rational positions with SongLimits.frames_at; checked activation
  offsets, exclusive end, exact tail deadline and final bounded fade. Never sum
  individually rounded child durations or convert legacy floating host time.
- Encode frozen controls through actual control layouts and private bank authority;
  use original resolved instrument and actual source-to-sample lease association.
  Expand chords once, preserve supported delay/room/sidechain behavior and reject
  unsupported values explicitly. AudioEvent Inst/Bus IDs are u32; only controls
  encoded as f32 require checked integer roundtrip.
- Physical allocation follows complete resolved configuration/placement/source
  identity, not note/onset. Enqueue Release/Rebind and matching exact-boundary
  Event ahead of the transition through the authentic projected generation chain.
  Actual BranchRebound confirms committed ownership; enqueue never commits or
  publishes a new generation. Waiting for that ACK before submitting its first
  same-frame event would make the onset late. Respect voice/private tail/overlap
  bounds, retain exact refusals, and stop/clean up on a failed transition. Never
  reset quota or recompile on queue retry.
- Schedule finite endpoint/release ahead of commit horizon. Failed stops new
  onsets and retains cleanup obligations; it never reports successful music.
- Normal retirement requires actual exact full-key returns and all upload/receipt
  obligations. Do not send CancelPreparation after a normal end to invent proof.
- Genuine owner assembly always retains Master, including silence. Verify its
  normal safe return follows accepted endpoint/tail intent and cannot be confused
  with cancellation. Existing Engine raw zero-resource states alone do not justify
  a new protocol. If actual public owner completion remains unprovable, document
  the exact reachable counterexample and amend a bounded prerequisite first.

## Tasks

### TASK-001: Grounded ownership and exact time contracts
**Status**: In Progress
**Parallelizable**: Yes with activation implementation; disjoint scheduler files, public interface coordinated between authors.
- [ ] Record source/design/plan baselines and exact ownership imports.
- [ ] Preserve refusal ownership, frame overflow/rate diagnostics and empty/silent semantics.

### TASK-002: Finite query, encoding, pools and receipts
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this manifest.
- [ ] Implement all seven module deliverables and required behavior.
- [ ] Preserve all parent progression, Apply/mute and eventual export obligations.

### TASK-003: Genuine core evidence
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No; joined checking requires complete source hold.
- [ ] Actual Native/Arena full song with repeated and edited parts, chords and private effects.
- [ ] Exact fractional boundaries, no onset at end, tail/fade and silence completion.
- [ ] Partition/retry seed invariance, real rebind, ACK/queue/garbage pressure and failure cleanup.
- [ ] Native/wasm/strictClippy, scoped formatting/line limits and nonempty actual test inventories.

## Completion criteria

- [ ] Every task and required behavior verified against actual production paths.
- [ ] Original process logs, exact source hashes and full test names retained.
- [ ] No unexpected dirty-work changes or unowned source writes.
- [ ] Parent scope remains intact; full goal not inferred from core acceptance.

## Progress log

### 2026-10-02 — ROOT0534 bounded split

Read-only specialized audit identified the real live-commit authority mismatch,
large integration files and actual consuming Ready interfaces. Root reviewed
actual Master assembly and Engine normal-retirement predicates. This document
splits the future implementation; no source release or behavioral acceptance.


### 2026-10-02 — ROOT0536 projected rebind and initial activation audit

Specialized review found the provisional ACK-before-Event wording made the first
onset of a reused generation late. Root confirmed song_queue authenticates
queued Release/Rebind projections and ranks Event after Rebind at the same frame.
Corrected that wording; actual receipt still confirms generation ownership.

The same issue is now under source audit at initial activation: current host
owner exclusively waits Applied before another same-epoch command, while the
Native callback may already render the exact activation-frame onset before the
controller observes Applied. This is an unresolved full-playback prerequisite.
Do not silently drop the first onset, add musical preroll, weaken activation
failure ownership or claim Ready. If a pipeline is required, generic epoch-only
Rejected cannot automatically prove activation failed when other commands were
submitted; correlated failure authority must be designed and explicitly owned
before changing the host contract. Existing owner fixtures still verify their
released contract; they do not establish this full transport behavior.

### 2026-10-02 — Scheduler implementation release

Root releases the seven Rust paths in the manifest for implementation
concurrently with the activation repair. The activation author retains exclusive
ownership of host/DSP files; coordinate the pending-command API without editing
those files here. Actual activation acceptance remains a prerequisite for final
integration acceptance, not for writing independent scheduler code. Sample
admission remains required for the complete goal and is not waived. The user continuation explicitly requires verified completion: focused tests
are authorized in the declared test path; independent checking follows held
source. Existing evidence is retained and is not proof of this new phase.

Priority is the complete original finite transport: exact absolute frames,
frozen event encoding, pool reuse, receipt handling and normal retirement.
Do not introduce a second transport or special-case example-only playback.

### 2026-10-02 — Concrete module declarations before implementation

Private frame contract (frames.rs): `pub(super) struct FrameMap`; 
`FrameMap::new(settings: SongSettings, duration: Ratio64, activation: u64, sample_rate: u32, limits: SongLimits) -> Result<Self, Failure>`;
`FrameMap::at(&self, cycle: Ratio64) -> Result<u64, Failure>`;
`FrameMap::deadline(&self, cycle: Ratio64) -> Result<u64, Failure>`;
`FrameMap::endpoints(&self, epoch: SnapshotEpoch) -> Result<SongEndpoints, Failure>`.
All endpoints use absolute rational seconds plus checked activation offset.
No cumulative rounding or floating time authority. Encoding contract (encode.rs):
`pub(super) fn encode(ready: &SongReadyBundle, row: &FrozenSongEvent, branch: &SongBranchRoute, pool: SongPhysicalBranch, generation: u32, frame: u64, seconds_per_cycle: Ratio64) -> Result<SongAudioEvent, Failure>`;
`fn scalar(value: &FrozenControl) -> Result<Value, Failure>`;
`fn push(event: &mut AudioEvent, id: CtlId, value: f32) -> Result<(), Failure>`.
Closed declared controls override generic rows; resource references require original
Ready sample mappings. Supported scalar controls are checked, not truncated.
The core module remains unregistered during the activation-focused checker.

Private pool declarations before source: `pub(super) struct PoolBook`;
`pub(super) struct Assignment { physical: SongPhysicalBranch, generation: u32, rebind: Option<SongBranchRebind>, release: Option<SongBranchRelease> }`;
`PoolBook::new(pools: &[SongPhysicalBranch]) -> Self`;
`PoolBook::assign(&mut self, route: SongResolvedRoute, onset: u64, end: u64, deadline: u64) -> Result<Assignment, Failure>`;
`PoolBook::receive(&mut self, rebound: SongBranchRebound) -> bool`.
Projected generations are separate from acknowledged generations. Complete resolved
route identities remain retained until safe reuse; no note or clipped-span keys.
Every new projected generation has a retained expected full transition receipt.

Core private declaration before source: `fn flush(&mut self, host: &mut dyn AudioHost) -> Result<usize, Failure>`;
`fn realize(&mut self, end: Ratio64) -> Result<(), Failure>`;
`fn begin_retirement(&mut self)`;
`fn failure(message: &str) -> Failure`.
Retain boxed Copy commands across refusal; one canonical cycle per realization,
full immutable route key per pool and original Ready query authority. Public
read-only accessors: `epoch() -> SnapshotEpoch`, `settings() -> SongSettings`,
`endpoints() -> SongEndpoints`, `applied_activation() -> Option<SongActivation>`,
`failure() -> Option<&Failure>`, `take_retired() -> Result<PreparedSong, Failure>`.
Receipt child: `pub(super) fn receive(transport: &mut SongTransport, ack: SongHostAck) -> Result<(), SongHostAck>`.

Bound refinement before next source write: `PoolBook::new(pools: &[SongPhysicalBranch], max_pending: usize) -> Self` retains the actual caller cache ceiling;
all expected rebound receipt storage checks that ceiling before allocation.
Frozen query/route batch allocations remain bounded by the same event ceiling;
queue retries retain existing allocations rather than realizing another batch.

Receipt-state refinement before source: Applied is retained independently of Ready;
owned same-epoch runtime rejection transitions Failed and preserves retirement.
Draining is exposed only once the observed host clock reaches arrangement end,
not when advance has merely submitted all finite intent ahead of time.
Genuine test declarations: `finite_native_and_arena_transport_repeats_then_changes_and_retires()`;
`empty_native_and_arena_transport_returns_original_owner()`.
Fixtures use actual candidate evaluation, preparation, Native/Arena rendering and
real ACK drain; callback allocator probe arms only around existing render/process.

### 2026-10-02 — Initial coherent source checkpoint

Seven scoped files now written and formatted; no Cargo has run. Initial source
implements original Ready ownership, one canonical-cycle query per advance,
absolute endpoints, complete resolved configuration pool identity, projected
Release/Rebind/Event submission, actual receipt confirmation and guarded cleanup.
Two genuine Native/Arena fixtures are written (finite changed sequence and empty
arrangement), with existing real Engine callback allocation/deallocation probe.
This checkpoint is held for early compiler/behavior feedback, not completion.
Full pressure/partition/chord/effect witnesses and exhaustive encoding audit remain
pending. Apply earlier-cutoff replacement has a concrete Engine endpoint seam,
reported to Root: second same-epoch end currently rejects, original queued endpoint
is not removed. This checkpoint does not claim Apply or full playback completion.

### 2026-10-02 — Actual initial checkpoint and completion wave

Raw `/tmp/vactr-transport-core-initial-002.log` reports both genuine Native/Arena
fixtures passing (2/2). This verifies audible finite completion and empty owner
return only; precise repeated-part event identity and pressure remain pending.
Before next source: `fn fail_and_retire(&mut self, error: Failure)` clears only
unaccepted pending intents, retains actual failure and original guarded cleanup.
`PoolBook` expected receipt count is bounded globally by max_cached_events, not
that ceiling independently for every physical slot. Stored sample_rate remains
validated after Ready transfers into cleanup. Test helper `struct RecordedHost`
borrows the real host and records only actually accepted exact command PODs;
pressure is produced by actual bounded Native/Arena sender saturation.

Cutoff declaration before source: `pub fn cutoff(&mut self, host: &mut dyn AudioHost, endpoints: SongEndpoints) -> Result<(), SongCommandRefusal>`.
Caller supplies exact boundary/deadline (Apply may clear tails). Require sameepoch,
activation<=boundary<=retained endpoint and boundary<=deadline<=retained deadline.
On actual host admission stop new realization and remove unaccepted commands at
or after boundary; refusals preserve exact endpoints and leave original state.
Integration author owns Engine earlier-endpoint supersession implementation.

Focused fixture declarations before source:
`fn run_variant(code: &str, bytes: bool, epoch: u64, pressure: bool, partitioned: bool) -> Vec<SongAudioEvent>`;
`fn fill_commands(host: &mut dyn AudioHost) -> usize` uses actual foreign Mute
records until exact Backpressure. `RecordedHost::try_song_command` records only
actual accepted commands and delegates real ingress. Tests:
`canonical_partition_and_real_queue_retry_preserve_exact_events()` and
`frozen_chords_are_encoded_once_against_private_graphs()`.
Exact accepted frame offsets, control bit patterns, generations, no end onset and
actual full foreign rejection count are independent of partition/retry shape.

Failure API declaration: `pub fn abort(&mut self, error: Failure)` retains original
owner. Never-submitted/rejected activation enters cancellation; uncertain submitted
activation retains and retries its authentic finite Endpoints before normal cleanup.
No implicit cancellation of an active epoch. Integration calls advance thereafter
to service retained commands and exact returns even in Failed state.

Failure ownership accessor before source: `pub fn take_cancelled(&mut self) -> Result<PreparedSong, Failure>` returns original owner only after real never-activated cancellation.
Tests before source: `accepted_cutoff_stops_future_onsets_and_returns_full_epoch()`
and `abort_before_activation_preserves_original_cancelled_owner()` use actual
Native/Arena callbacks/ACKs; cutoff requires integration-owned earlier-endpoint
semantics, no fabricated endpoint receipt or early lease reuse.

### 2026-10-02 — Cutoff/failure/pressure coherent checkpoint

Six real fixtures now written: initial two plus exact repeated-part Event frames/
frequency, real Native/Arena queue saturation and fractional partition equality,
mono chord expansion, exact earlier cutoff and neveractivated original-owner abort.
Only initial two have executed; new four await mandatory independent checking.
All scoped sources are formatted and held, no author Cargo. Existing full encoding
resource/window/seed and larger waveform criteria remain unchecked rather than
inferred from command-record assertions. The next checkpoint permits real compiler
feedback and integration/export work; it does not mark TASK002/003 completed.

### 2026-10-02 — corrected six-fixture transport checkpoint

Independent checker ran all six core fixtures: 6 passed, 0 failed, 0 filtered;
original session 5384 exited 0. Log: `/tmp/vactr-transport-core-pressure-repaired-001.log`,
SHA-256 `bff8d37dc511c6997835a84079cffef63d3ea8b9bb7ba2ea08e1040e08afbfdf`.
The two earlier failures were corrected in fixtures: simultaneous notes now use
the actual chord constructor with four physical voice slots; the browser outbox
is configured before accepting Activate, preserving that accepted command during
subsequent real queue saturation. Production retry policy and assertions were
preserved. All ten checked core/runtime source hashes were unchanged during the
run. Native/Arena finite/empty completion, cutoff, preactivation abort, chord
encoding and partition/pressure event invariance are proved by this checkpoint.
Runtime/session playback, atomic Apply/fades, acknowledged mute, sample admission,
broader routing/encoding coverage and final native/wasm gates remain incomplete.
