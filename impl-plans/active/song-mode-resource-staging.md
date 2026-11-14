# Actual capacity and silent resource staging implementation plan

**Status**: Completed
**Plan ID**: SONG-09P
**Plan Path**: impl-plans/active/song-mode-resource-staging.md
**Created**: 2026-10-01
**Last Updated**: 2026-10-02
**Session target**: 1–3 sessions after carrier prerequisite clearance
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent and accepted boundary

Provide real configured resource geometry, epoch-owned leases, silent adoption,
and pressure-safe cancellation before full routing acceptance. Existing frozen
snapshot and prepared route contracts are inputs. Staged resources must never
replace active legacy/song resources or execute callbacks, loaders or VM code.
This prerequisite supplies actual capacity evidence to SONG-08B; it does not
implement audible activation, scheduling, private branch mixing, mute, detector
causality or transport playback. Those remain SONG-09/10/11 obligations.

The acceptance cycle is explicit: SONG-08 waits for SONG-08B actual leased
capacity evidence, SONG-09 currently waits for full SONG-08, and SONG-10 defines
actual host capacity providers after SONG-09. Implementation ordering must allow
this bounded prerequisite before full08 acceptance. Final joined acceptance is
preserved; aggregate construction arithmetic alone cannot certify host fit.

## Source-grounded interfaces

- EngineConfig in ring.rs709 configures template/bus slots, per-bus and per-voice
  float budgets, and sample store mode. NativeAudioHost::pair receives this
  configuration and a separate AtomicCells table; analysis_cells is telemetry,
  not the actual control-cell capacity returned by AtomicCells::capacity.
- Engine::record currently rejects valid Song commands as NotReady. Staging can
  consume BeginPreparation/ReserveResource/ConfigureBranch/SealPreparation only
  after genuine resource adoption. Activate/Mute/Event stay explicit NotReady
  here; this phase never emits Applied or Muted.
- NativeInstall has owned Template/BusTemplate boxes or SampleData Arc and only
  legacy u32 resource/gen. It cannot independently identify a full song lease.
- Arena SampleStore owns contiguous PCM extents; NativeArc has zero arena-byte
  sentinel and needs a separate real PCM ceiling and occupied sample-slot proof.
- SONG-08C supplies FrozenCellInventory, finite scalar defaults, exact graph
  reference sites and owner-local ordered analysis banks. Physical mapping and
  resource reuse belong here and downstream; frozen metadata is not installed.

## Required carrier prerequisite: proposed SONG-09PC

Root must create and approve `song-mode-resource-staging-carriers.md` before
SONG-09P implementation. This section proposes that separate plan; it does not
authorize changes to its paths. The carrier work cannot be implicit in09P.

Proposed seven Rust paths: `src/song/routing.rs`, `src/dsp/ring.rs`,
`src/host/caps.rs`, `src/dsp/engine.rs`, `src/host/wasm/messages.rs`,
`tests/song_resource_carriers.rs`, `tests/song_audio_wire.rs`. Existing command/resource IDs stay frozen;
new kinds/records append. SnapshotEpoch remains u64; SongResourceRef id and generation remain u32,
with no truncation or generation wrap/reuse while a lease or receipt survives.

Required declarations to place in routing/ring (declarations only):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongLeaseKey {
    pub epoch: SnapshotEpoch,
    pub resource: SongResourceRef,
    pub kind: SongResourceKind,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SongCellInit {
    pub lease: SongLeaseKey,
    pub cell: CellId,
    pub value: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongAnalysisReservation {
    pub lease: SongLeaseKey,
    pub slots: u32,
}
pub struct NativeSongInstall {
    pub lease: SongLeaseKey,
    pub payload: NativeInstall,
}
```

Append `ControlCells` and `AnalysisBank` resource kinds, explicit song cell-init
and analysis reservation records, cancellation record and cancellation receipt.
A native song-install Record variant wraps NativeSongInstall without erasing
legacy payload ownership. Browser graph/sample install framing carries the same
SongLeaseKey; no wrapped legacy ID is accepted as public lease identity. Define
exact codec size/tag declarations before edits. Malformed records retain the
next record, and all critical receipts survive backpressure. Ready retains its
meaning of complete resource adoption, not enqueue success.

09PC retains the existing SongHostCapacities layout and every existing capacity
literal. Add a separate typed staging analysis contract in routing.rs:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongAnalysisCapacity { pub slots: u32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongStagePreparation {
    pub preparation: SongPreparation,
    pub analysis_required: SongAnalysisCapacity,
}
```

Append `BeginStaging(SongStagePreparation)` to SongCommand, plus the explicit
cell/analysis/cancellation records declared above; preserve BeginPreparation
legacy decoding. At BeginStaging and Seal, validate actual private analysis
capacity and already reserved banks separately from old aggregate capacities.
Do not migrate near1000-line prepare/components or unrelated literals merely
to add a field. New resource-kind codec tags require existing malformed-kind
fixtures in song_audio_wire.rs to retain an actually invalid tag.

Private scalar cells and analysis storage live in SongResourceStager; this
prerequisite does not mutate cells.rs or use epoch-blind AtomicCells::set as
adoption. Full lease ownership protects each private physical bank; staged
compiled graphs reference a bank only through exact remapping. Legacy external
CellStore remains unchanged, and its actual capacity is reported independently.

09PC owns every exhaustive consumer: append NativeRecord::SongInstall and
Record::SongNative(NativeSongInstall), update NativeControlSource mapping and
record budget classification, and add the engine record arm. Before09P adoption
exists, the engine rejects NotReady while retaining owned payload for checked
off-thread return. A bounded pending native-return field and polling stop are
required under garbage pressure, rather than dropping/leaking uploads. Append
Garbage::SongInstall(NativeSongInstall) for off-thread destruction; native host
currently drops dequeued Garbage uniformly, so no new host path is required.
Tests prove full queues retain ownership and old active state. This is carrier
rejection/ownership only, never staging Ready. If that retained-owner mechanism
requires another compile consumer, amend the proposed seven-path carrier plan
before its release; no undeclared writes are implied here.

Returned native ownership on failed enqueue/adoption must remain recoverable:
proposed bounded upload capability in caps.rs is
`fn submit_song_native(&mut self, install: NativeSongInstall) -> Result<(), NativeSongInstall>`.
Arena upload uses a typed SongLeaseKey plus existing InstallRequest and bounded
byte chunks; admission precedes allocation/copy. The proposed carrier plan must
include actual default unsupported behavior and both FIFO adapters. No abstract
trait implementation or codec-only success certifies adopted resources.

## Manifest and ownership

```json
{
  "planId": "SONG-09P",
  "planPath": "impl-plans/active/song-mode-resource-staging.md",
  "dependsOn": ["SONG-09PC", "SONG-09PB", "SONG-08A", "SONG-08C", "SONG-07"],
  "writePaths": [
    "src/dsp/engine/song.rs", "src/dsp/arena/song.rs", "src/dsp/engine.rs",
    "src/dsp/arena.rs", "src/dsp/bus.rs", "tests/song_resource_staging/live.rs",
    "tests/song_resource_staging.rs", "tests/song_resource_staging/acceptance.rs",
    "impl-plans/active/song-mode-resource-staging.md"
  ],
  "sharedPaths": ["src/dsp/engine.rs", "src/dsp/arena.rs", "src/dsp/bus.rs",
    "src/host/native/audio.rs", "src/host/wasm/worklet_half.rs"],
  "readOnlyContracts": ["src/host/native/audio.rs", "src/host/wasm/worklet_half.rs", "src/song/routing.rs", "src/dsp/ring.rs",
    "src/dsp/cells.rs", "src/song/snapshot/cells.rs"]
}
```

Acquire serial ownership from09/10 before any writes; coordinate root stable
holds for shared Rust. No automatic restoration of historical hashes. If any
required carrier/provider declaration cannot fit these manifests, root must
approve a bounded manifest amendment or further prerequisite before edits.

## Modules and declaration contract

All declarations below are new planned APIs grounded in existing types, not
claims that they exist. Concrete module imports use crate-owned types; no new
dependencies. Types that own Vec/Box allocate only off callback or construction.

### src/dsp/engine/song.rs — engine-owned leases and physical geometry

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongFrameRegion {
    pub slot: u32,
    pub offset: u64,
    pub frames: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongLeaseState {
    pub key: SongLeaseKey,
    pub staged: bool,
    pub retiring: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongStagingConfig {
    pub preparations: u32,
    pub leases: u32,
    pub branches: u32,
    pub control_slots: u32,
    pub analysis_slots: u32,
    pub native_pcm_bytes: u64,
    pub critical_receipts: u32,
}
pub struct SongResourceStager;
impl SongResourceStager {
    pub(crate) fn new(
        config: SongStagingConfig, actual: SongHostCapacities,
        bus_regions: &[SongFrameRegion], voice_regions: &[SongFrameRegion],
        pcm_extents: &[SongFrameRegion],
    ) -> Result<Self, SongRejectCode>;
    pub fn remaining_capacities(&self) -> SongHostCapacities;
    pub fn remaining_analysis_capacity(&self) -> SongAnalysisCapacity;
    pub fn bus_regions(&self) -> &[SongFrameRegion];
    pub fn voice_regions(&self) -> &[SongFrameRegion];
    pub fn pcm_extents(&self) -> &[SongFrameRegion];
    pub fn lease_states(&self) -> &[SongLeaseState];
}
```

Constructor is crate-internal. Engine alone supplies actual configured capacities
and region views from its constructed BusGraph/VoicePool/SampleStore, as the
explicit new arguments. It copies descriptors into construction-preallocated
staging tables; external callers cannot certify an invented actual capacity.
NativeAudioHost configuration supplies AtomicCells::capacity and true bounded
ring capacity. configure_song_staging rejects any mismatch or absent provider.
Tables contain actual capacities and occupied/staged/retiring owners, with
checked counters and immutable views. Every logical cell-reference site remaps
to its leased control bank; analysis logical holes and overlapping writers
retain ordered owner-local addressing, with a fresh bank per private generation.
Reuse is blocked until final retirement receipt and native garbage handoff.

### Engine, SampleStore and BusGraph

```rust
impl Engine {
    pub(crate) fn configure_song_staging_with_transport(
        &mut self, config: SongStagingConfig, measured_ack_slots: u32,
    ) -> Result<(), SongRejectCode>;
    pub fn configure_song_staging(
        &mut self, config: SongStagingConfig,
    ) -> Result<(), SongRejectCode>;
    pub fn song_remaining_capacities(&self) -> Result<SongHostCapacities, SongRejectCode>;
    pub fn song_analysis_capacity(&self) -> Result<SongAnalysisCapacity, SongRejectCode>;
    pub fn song_bus_regions(&self) -> &[SongFrameRegion];
    pub fn song_voice_regions(&self) -> &[SongFrameRegion];
    pub fn begin_song_preparation(&mut self, command: SongStagePreparation) -> Result<(), SongRejectCode>;
    pub fn reserve_song_resource(&mut self, command: SongResourceReservation) -> Result<(), SongRejectCode>;
    pub fn stage_song_native(&mut self, install: NativeSongInstall) -> Result<(), NativeSongInstall>;
    pub fn stage_song_branch(&mut self, command: SongBranchConfig) -> Result<(), SongRejectCode>;
    pub fn seal_song_preparation(&mut self, epoch: SnapshotEpoch) -> Result<(), SongRejectCode>;
    pub fn cancel_song_preparation(&mut self, epoch: SnapshotEpoch) -> Result<(), SongRejectCode>;
}
impl SampleStore {
    pub fn song_free_extents(&self) -> &[SongFrameRegion];
    pub fn song_free_sample_slots(&self) -> u32;
    pub fn song_reserve(&mut self, key: SongLeaseKey, request: InstallRequest) -> Result<(), Fault>;
    pub fn song_cancel(&mut self, key: SongLeaseKey) -> Result<(), Fault>;
    pub fn song_begin(&mut self, key: SongLeaseKey, request: InstallRequest) -> Result<(), FaultCode>;
    pub fn song_write_slice(&mut self, key: SongLeaseKey, offset: usize, bytes: &[u8])
        -> Result<bool, FaultCode>;
}
impl BusGraph {
    pub fn song_regions(&self) -> &[SongFrameRegion];
    pub fn stage_song_bus<C: CellRead + ?Sized>(
        &mut self, key: SongLeaseKey, template: Box<BusTemplate>,
        cells: &C, sr: f32, caps: &CapabilitySet,
    ) -> Result<(), Box<BusTemplate>>;
}
```

configure_song_staging is construction-only and rejects once processing starts;
control_slots constructs the actual song-only pool and all song references
remap to it. Its physical allocation is distinct from legacy AtomicCells; actual
remaining private slots are reported, never inferred from telemetry or a declared
external pool. Analysis capacity is separately reported and admitted via the
SongStagePreparation contract. Fixed voice
regions remain shared; they are not subtracted once per snapshot. PCM extents
are float units in SongFrameRegion, while pcm_bytes is byte capacity; checked
conversion must account channels and f32 size. SampleStore::largest_extent is
currently private and free_floats is aggregate: song_free_extents exposes
actual maintained allocator extents, not a synthetic one-span sum; NativeArc occupancy uses a checked
real byte ceiling and sample slots. Individual bus state regions must hold the
actual prepared template requirement; aggregate free bus_frames is insufficient.

The native staging callback receives existing Boxes/Arcs. Decode/init touches
only preallocated storage. Rejected uploads return original ownership; callback
must not drop, allocate, lock or invoke a VM/loader. Arena staging shares the
exact lease/kind/epoch checks, validates lengths and contiguous extents, preserves
active state on malformed/truncated uploads, and never uses an active legacy
GraphInstall as staging. Private branch state remains silent and omitted from
legacy summing until later09 activation.

### NativeAudioHost and worklet consumer

```rust
impl NativeAudioHost {
    pub fn song_remaining_capacities(&self) -> Result<SongHostCapacities, Failure>;
    pub fn song_analysis_capacity(&self) -> Result<SongAnalysisCapacity, Failure>;
    pub fn submit_song_preparation(&mut self, preparation: SongStagePreparation) -> Result<(), Failure>;
    pub fn submit_song_native(&mut self, install: NativeSongInstall) -> Result<(), NativeSongInstall>;
}
```

Native pair/configuration preserves real EngineConfig, AtomicCells capacity,
actual ring capacities and reported resource geometry. The worklet consumer
uses identical full identities and adoption receipts through existing byte
transport. Mutable callback state is published through existing bounded message
channels; a host getter cannot call into or lock the live Engine. Last reported
capacity plus outstanding host reservations is authoritative and conservative;
missing reports return HostUnavailable, not guessed totals. Full browser JS
wiring remains14, but an actual ByteInbox/Engine staging fixture is required now.

Root ROOT0186 has already clarified the parent09 implementation-vs-acceptance
ordering; its appended amendment supersedes the blanket implementation hold
for separately reviewed09P. This does not release Rust or alter final criteria.

## Related plans and dependencies

| Input | Required reviewed output | Status |
|---|---|---|
| SONG-09PC + ABI child | Full-identity native/byte/cell/capacity carriers and pressure-safe return APIs | INDEPENDENTLY_CLEARED_ROOT0215 |
| SONG-08C | Complete frozen cells and private analysis owner inventory | INDEPENDENTLY_CLEARED_ROOT0200 |
| SONG-07 | Immutable isolated snapshot and closed resource handles | VERIFIED |
| SONG-08A/partial08 | Exact carriers, bounded one-message ack polling | VERIFIED_PARTIAL08 |
| SONG-08B | Stable reviewed SongRoutePlan/required DTO for fixture input | IN_PROGRESS |

Previous: proposed09PC and08C. Next: actual08B capacity evidence, joined08,
then full09 private rendering and10 adapters. This plan consumes reviewed08B
contracts without depending on08B's final capacity acceptance. Neither a stale
partial plan nor fabricated public DTO is a trusted source of host allocation.

## Tasks

### TASK-001: Seal declarations and real construction geometry
**Status**: Completed
**Parallelizable**: No
**Depends On**:09PC/08C independent clearance and root ownership release
Declare exact allocation handoff and verify eight-path scope before edits;
record current hashes. Add real per-slot/voice/PCM descriptors and constructor
validation. Any touched Rust at1000+ must split within approved paths or require
prior amendment. Engine854/Arena835/native838 leave limited headroom.

### TASK-002: Epoch leases and private cells
**Status**: Completed
**Parallelizable**: No
**Depends On**:TASK-001
Implement bounded full64 epoch/u32-generation leases, actual control-site remapping, private ordered
analysis banks and no-reuse receipt states. Compare frozen values without live
candidate/registry reads. Reject cell-ID wrap/collision and insufficient banks.

### TASK-003: Silent native and Arena adoption
**Status**: Completed
**Parallelizable**: No
**Depends On**:TASK-002
Stage real Template/BusTemplate/SampleData and bounded byte uploads into leased
storage; preserve old active graph/sample/cell behavior on every failure. Seal
only after all required adopted resources and branch references are complete.

### TASK-004: Pressure-safe rollback and actual host reports
**Status**: Completed
**Parallelizable**: No
**Depends On**:TASK-003
Return/retry native ownership with full garbage queues; retain critical receipt
cursor when ack queues fill. Cancel cannot free IDs before ownership handoff.
Publish actual conservative remaining capacities through native/worklet FIFO.

### TASK-005: Production fixtures and independent clearance
**Status**: Completed
**Parallelizable**: No
**Depends On**:TASK-004
Run actual Engine/native and Arena fixtures, legacy compatibility, callback
allocation proof, stable inventories and independent sealed gates. Root then
amends prerequisite status; no full08/09/10 completion inferred.

## Acceptance and validation matrix

- [x] Real Engine construction and NativeAudioHost cell/ring allocations match
  all capacity fields; one-below required rejects before adoption.
- [x] Aggregate-fit/per-slot-too-small bus template rejects; fragmented PCM
  aggregate-fit/contiguous-too-small rejects; NativeArc real PCM cap enforced.
- [x] Full64 epoch and exactu32 resource generation/kind prevents stale/delayed association,
  including legacy u32 transport-ID reuse; wrong-kind cells/graphs reject.
- [x] Existing audible native and Arena active graphs remain identical before
  and after successful silent staging, failed upload and cancellation.
- [x] Frozen scalar cells and complete header/UGen/effect reference sites resolve
  against private epoch banks; analyzer gaps/overlaps/writer order remain local.
- [x] Ready/ResourceReady occur only after actual resource adoption; enqueue is
  insufficient. Activate/Mute/Event reject explicitly and never report Applied.
- [x] Current/staged/retiring extents remain subtracted until garbage ownership
  and critical receipts are safely released; queue pressure neither leaks nor
  falsely frees memory/IDs. Cancellation preserves unrelated active resources.
- [x] Actual native and ByteInbox parity, malformed frame recovery and FIFO
  receipts; callback zero allocations, no locks/VM/loader/disk reads.
- [x] Exact nonempty inventories include new staging tests and affected legacy
  ring/native/arena/cell/bus/session tests; no duplicate counts or ignored failures.

## Required verification — independently completed

`CARGO_TERM_QUIET=true mise exec -- cargo check`, host-wasm check with
`--target wasm32-unknown-unknown --no-default-features --features host-wasm`,
strict scoped/all-target Clippy, eight-path rustfmt/diff, unfiltered
`cargo test --test song_resource_staging`, relevant legacy native/Arena/cell
suites, and nextest with required fail-only environment. Specialized checker
must independently verify held source/plan hashes and terminal process receipts.
Actual audible activation tests belong full09; actual silent preservation audio
is required here. No warning suppression or neutral/stub graph acceptance.

## Completion criteria

- [x] Carrier and08C prerequisites independently cleared.
- [x] Every declaration and eight-file deliverable implemented with fresh hashes.
- [x] All actual staging/capacity/ownership fixtures and independent gates pass.
- [x] Root reconciles08B actual evidence and parent dependency amendments.
- [x] Only09P marked completed; full08B/08/09/10 requirements remain explicit.

## Progress log

### Session: 2026-10-01 — bounded prerequisite planning
**Tasks Completed**: Planning source audit and dependency-cycle decomposition.
**Tasks In Progress**: Root review of proposed09PC and09P manifests/interfaces.
**Blockers**:09PC not created;08C independent clearance pending; no implementation release.
**Notes**: Only this new plan and immutable09P planning receipts were written.
No Rust, existing plans, indexes, archives or Cargo execution changed.


### Session: 2026-10-02 — carrier identity and copied graph integration

The reviewed09PC carrier contract retains existing ResourceReady/ResourceRetired
acknowledgments. Therefore (epoch,resource id,generation) is globally unique
across kinds throughout outstanding leases, uploads, delayed receipts and garbage
ownership. The stager rejects conflicting-kind reservations for the same triple;
a delayed receipt cannot ready or release a future assignment. Resource generation
remains u32 and epoch u64, with checked no-reuse ownership throughout retirement.

Rewrite complete copied InstDef/BusDef controls and resource references before
off-thread compilation, as existing native graph construction already permits.
Use the certified08C header/node/embedded-effect/bus sites, all frozen sample/table
references, convolution IR and sidechain selectors. Rebuilding templates preserves
internal node lookup IDs, reference lists and seed wiring. Membership-only
reads_cell/reads_resource checks cannot certify an exhaustive closed inventory.
Native and Arena must validate the same remapped reference contract before Ready.
IR identifiers carried through f32 control constants must round-trip exactly;
physical IDs use representable checked mappings without truncating public leases.

The stager initializes and retains actual private scalar and owner-local analysis
banks while candidate resources remain silent. Audible per-voice/per-bus bank
selection is a full09 rendering obligation in its already owned engine/render.rs;
existing RenderCtx can borrow those views without adding voice.rs ownership. This
prerequisite does not claim private rendering from the legacy shared CellRead and
FxCtx.analysis path. ROOT0202 records the amendment intent and source review.


### Session: 2026-10-02 — cohesive staging modules before parent growth

Engine is already above900 lines during09PC integration, and arena.rs is835.
The eight-path staging manifest now uses engine/song.rs for engine-owned staging
state and methods, and arena/song.rs for allocator/template staging helpers.
Engine and Arena register their respective child modules. The planned independent
dsp/song.rs and dsp/mod.rs registration are removed from this prerequisite; later
full09 private rendering retains its separately reviewed ownership.

Public staging declarations are re-exported from dsp::engine for callers. Child
modules retain access to their owner's private allocation state and avoid exposing
Engine fields across unrelated modules. Parent files contain registration and
small integration calls; complete stage/lease/extent methods live in the children.
All constructor, silent adoption, capacity, rollback and acceptance requirements
remain unchanged. This is a prior module/path amendment, not an implementation
claim. ROOT0204 records current source size evidence and mutation intent.


### Session: 2026-10-02 — staging source feasibility contracts

ROOT0210 records the prior document amendment. All eight Rust paths remain
unchanged and unreleased; this review closes concrete declaration gaps:

- A sample lease progresses Reserved → Installing → Adopted. SampleBegin checks
  its full key and request, then uses its already reserved exact extent; it must
  not allocate a second extent through legacy begin. Where request geometry
  first arrives at SampleBegin, extent admission happens there before upload
  begins. song_write_slice requires offset equal to received, checked lengths,
  finite floats and the same full key. Duplicate/overlap/out-of-order slices
  cannot increment received or certify completion with holes.
- Adopted staged samples remain unreadable to legacy get(), and staged buses
  never enter legacy audio sums or sidechain snapshots. Define explicit staged
  state in the owned arena/bus modules or separate bounded staged storage.
  Existing bus run processes every non-Free slot, so a new state needs explicit
  exclusion in both processing and detector snapshot selection. Existing worklet
  state reporting must handle any added sample state without claiming Live.
- Successful stage_song_bus retains its original Box in bounded storage until
  adoption/retirement or pressure-safe garbage handoff. Copying a template then
  dropping the Box in the callback is prohibited. Its exact initialization uses
  the leased private CellRead bank, actual sample rate and CapabilitySet.
  Cancellation retains full lease identity until garbage handoff and its critical
  receipt. Native sample Arc ownership has the same silent retention requirement.
- Native and worklet construction supply actual ACK producer capacity through
  configure_song_staging_with_transport; config.critical_receipts alone is not
  capacity evidence. Public configure_song_staging rejects a missing measured
  provider. No live Engine lock or guessed capacity is permitted.
- Actual free-extent descriptors track every legacy and song allocation/release;
  a constructor-time copied free list cannot remain authoritative after mutation.

TASK001 must seal these contracts before source implementation; TASK003/004
fixtures must exercise duplicate slices, silent staged self-noise/detectors,
actual garbage pressure, ring geometry and fragmented allocator mutation.

### Session: 2026-10-02 — Ready handoff after ROOT0215

Carrier and ABI child plans are Completed with immutable completion receipts; ROOT0215 accepts59 distinct carrier/wire/engine-native tests, native+host-wasm+strict Clippy+format/diff and33 unchanged input hashes.08C independent acceptance is ROOT0200. The separate08D geometry failures do not alter these prerequisite bytes or grant full routing/playback acceptance. Exact current eight-source baselines/new absences are recorded in0007 before any source mutation. No Cargo or Rust writes in this handoff.

Typed review retains the declared Engine/SongResourceStager/arena/bus/native methods and separate analysis capacity contract. Actual constructor measurement comes from ACK producer capacity and constructed private pools/real slot memory; report RequestCapacity/CapacityReport with full-u64 serial uses the cleared09PC carriers. Private control init validates finite scalars against epoch-owned physical IDs; rewritten graph references must close against those initialized leases. Owner-local analysis holes and overlapping writer order are retained; audible per-voice/per-bus views remain full09.

Reserved samples use one admitted exact extent, Installing accepts only sequential slices with exact full key, and Adopted remains excluded from legacy get. Staged buses are excluded from both render and detector snapshots while their original native Boxes stay retained. Cancellation releases allocator IDs only after ownership return and critical receipt handoff. Live allocator mutation refreshes physical descriptors; constructor-time free snapshots do not certify later space.

No extra path blocker was found. Parent engine/native/arena files have limited headroom; keep complete staging implementations in the already-declared engine/song.rs and arena/song.rs children, with only registration/dispatch in parents. All touched paths must stay below1000; request prior amendment if a cohesive owned split cannot fit. Root source implementation release remains required.

## Historical module status at implementation release

| Module | Exact path | Status | Evidence |
|---|---|---|---|
| Engine-owned staging state | src/dsp/engine/song.rs | Ready, new | 0007 absence |
| Sample/template staging allocator | src/dsp/arena/song.rs | Ready, new | 0007 absence |
| Engine registration/dispatch | src/dsp/engine.rs | Ready | 0007 current SHA |
| Arena registration/legacy mutation integration | src/dsp/arena.rs | Ready | 0007 current SHA |
| Silent private bus initialization | src/dsp/bus.rs | Ready | 0007 current SHA |
| Measured native provider/reports | src/host/native/audio.rs | Completed provider, retired read-only | ROOT0261 / acceptance0019 current SHA |
| Measured worklet provider/records | src/host/wasm/worklet_half.rs | Ready | 0007 current SHA |
| Genuine production staging fixtures | tests/song_resource_staging.rs | Ready, new | 0007 absence |

### Implementation batch 0008 — actual resource ownership

ROOT0216 releases the exact eight Rust paths. SampleStore entries retain an optional
full SongLeaseKey; Reserved and Staged samples are excluded from legacy reads.
Private helpers maintain preallocated actual free-extent descriptors after every
legacy or song allocator mutation. Native PCM admission uses an explicit configured
ceiling and occupied entries. Cancellation returns owned Arcs to the engine
without callback destruction; reserved extents are released only after that handoff.
No author Cargo is authorized. Physical staging and behavior fixtures remain in progress.

### ROOT0219 — explicit graph bank association

The separately owned SONG-09PB carrier adds `SongGraphBanks { graph: SongLeaseKey,
controls: Option<SongLeaseKey>, analysis: Option<SongLeaseKey> }` and
`SongCommand::BindGraphBanks`. Stager retains one checked association per graph.
Bindings validate exact reserved keys, kinds and same epoch before adoption.
Control references resolve only through the explicitly bound initialized bank;
analysis writers retain logical IDs and ordered overlap within the associated
owner-local slice. No pooled analysis offset is encoded into an analyzer ID.
Missing banks are accepted only when graph has no corresponding references/writers.

Additional immutable Engine APIs (within engine/song.rs facade):
`pub fn song_control_cell(&self, bank: SongLeaseKey, logical: CellId)
-> Result<CellId, SongRejectCode>` and
`pub fn song_analysis_region(&self, bank: SongLeaseKey)
-> Result<SongFrameRegion, SongRejectCode>` expose exact admitted addresses.
Physical bank views do not fabricate incoming graph association. Full09 later
selects the bound owner-local analysis slice at voice/bus execution.

### Construction/provider refinement 0019

`Engine::configure_song_staging_for_transport(&mut self, config: SongStagingConfig,
acknowledgments: &AckProducer) -> Result<(), SongRejectCode>` obtains the actual
bounded producer capacity before forwarding construction checks. This supports
real public native/ByteInbox fixtures without a guessed numeric provider.
Native/worklet providers use the same measured construction path.
`Engine::song_pcm_extents(&self) -> &[SongFrameRegion]` exposes the maintained
SampleStore free list; constructor-copied PCM geometry is not an authoritative
mutable allocator view. Current physical PCM admission is always store-derived.

### First coherent implementation hold 0025

TASK001–005 are in progress. Actual SampleStore full-key reserved/installing/staged
entries, private physical sample IDs, maintained extents and strict sequential
finite slices are written. Bus slots retain original owned native templates and
are excluded from both rendering and detector snapshots. The engine has bounded
preparation/lease/branch tables, private initialized scalar pools, owner-local
analysis banks and the reviewed09PB graph binding table. Fixed compiled arrays
are validated/remapped without candidate or active AtomicCells reads. Native PCM
validation is a retained-owner per-quantum scan. Critical cancellation and garbage
handoff retain full identities; Ready is only a complete resource/branch seal.
Actual producer-measured Native/worklet setup and matching report-cache retries
are written. Ten genuine public fixtures are written, not yet executed.

Cohesive implementations fit the exact eight modules: the storage/host cache
helper is in arena/song.rs and bounded analyzer resource validation in bus.rs,
with engine/song.rs retaining its public facade and Engine staging dispatch.
All source and own plan writes are held for mandatory first compile/checker.
No author Cargo has run. Remaining acceptance includes complete callback
allocation/legacy preservation and adversarial capacity/ownership evidence,
any actual compile/behavior repairs and full independent phase verification.
Silent staging is not audible activation or full routing/playback acceptance.


### First build repair 0027 — ROOT0221/0222/0223

The first joined checker process39637 terminated101 at native compilation with
one missing SongHostAck import; its immutable logs remain in
`/tmp/vactr-song09p-first-independent-001`. No subsequent behavioral gate passed.
The bounded repair imports the existing acknowledgment type and computes reported
bus frames as a checked sum of current free BusGraph regions. Voice-frame
semantics retain the shared physical pool. New genuine fixtures exercise staged
and cancelled bus ownership, legacy occupancy and actual capacity receipts.

Before changing any compiled-template controls, map_template validates every
Effect index against declared n_fx and its fixed parameter bounds, plus every
SamplePlay/Wavetable/Granular resource against the complete fixed ownership refs
and actual staged sample lease. Existing BANK/IR/ref validation remains. New
malformed native fixtures preserve original Box pointers, node data and bound
header controls on rejection. These fixtures are written, not yet executed.
All edits remain within the existing eight Rust modules; no author Cargo ran.
Silent staging and the full phase acceptance remain unfinished.


### ROOT0225 private helper declaration — construction atomicity

`SampleStore::song_remaining_pcm_with_limit(&self, bytes: u64) -> Option<u64>`
computes prospective NativeArc remaining PCM using checked occupied-length byte
sums and subtraction, without changing the live ceiling. Arena geometry remains
actual allocator-derived. Engine constructs and validates the replacement stager
against that result before committing both the new ceiling and stager. A rejected
configuration preserves the previous provider and capacities. By-value failed
NativeRecord queue pushes retain their original payload with a narrowly scoped
result_large_err allowance, avoiding callback allocation merely for an error.


### ROOT0225 held lint/atomicity repair 0028

Retry002 original4534 terminated101 at strict Clippy after native and host-wasm
checks passed. Behavioral fixtures had not run. The three reported lints are
repaired: unused retained config removed, by-value NativeRecord error ownership
kept under the documented single-helper allowance, and final garbage ownership
moved without a needless reborrow. Configuration now computes prospective PCM
capacity and successfully constructs the replacement before committing its live
limit. A new genuine fixture checks rejected reconfiguration preserves prior
capacities, then successful reconfiguration and actual native sample staging.
Fourteen public fixtures are now written; no new Cargo or behavioral execution
has run. All eight sources remain below1000 lines and are held for the next
joint checker cohort. Full silent-staging acceptance remains in progress.

### ROOT0238 serial test-only acceptance ownership
Finished worklet_half.rs is retired from future writes and remains read-only.
The cohesive child tests/song_resource_staging/acceptance.rs replaces it in the
eight-Rust future manifest. Current release writes only parent test registration,
child acceptance tests and this owned plan. Production Rust remains held or
owned by the disjoint native capacity-query author. Existing14 tests untouched.
Parent registers `#[path = "song_resource_staging/acceptance.rs"] mod acceptance;`
Child imports `use super::*;`. Exact declarations before Rust:
```rust
fn physical_slot_and_pcm_fit_rejects_one_short_without_owner_loss();
fn fragmented_pcm_rejects_aggregate_fit_and_preserves_extents();
fn native_and_byte_graph_adoption_closes_headers_nodes_effects_and_samples();
fn live_legacy_audio_remains_sample_exact_through_silent_self_noise_staging();
fn full_ack_and_garbage_pressure_retains_all_owner_leases_exactly_once();
fn configured_callback_adoption_cancel_and_pressure_allocate_and_drop_nothing();
fn graph(id: u32, sample: Option<u32>, cell: Option<CellId>) -> vactr::dsp::graph::InstDef;
fn native_template(def: &vactr::dsp::graph::InstDef) -> Box<vactr::dsp::ugen::Template>;
fn bus_def(id: u32) -> vactr::dsp::graph::BusDef;
fn callback(rig: &mut Rig) -> [f32; 32];
fn byte_callback(rig: &mut Rig, inbox: &mut ByteInbox) -> [f32; 32];
fn install_live(rig: &mut Rig) -> ring::EventProducer;
fn drain_owners(rig: &mut Rig, rounds: usize) -> (Vec<SongHostAck>, Vec<Garbage>);
```
Probe GlobalAlloc forwards to System; thread-local counts wrap only actual
Engine.process after off-callback graph ownership/preparation. Tests retain
full keys/pointers/receipts and compare configured Native/Arena live audio twins.
No author Cargo. Production findings require root's prior owner release.

Acceptance auxiliary declarations before source:
```rust
fn closed_owners(rig: &mut Rig, epoch: u64) -> [SongLeaseKey; 5];
fn submit_graphs(rig: &mut Rig, owners: [SongLeaseKey; 5], bytes: bool);
fn self_noise_bus() -> Box<BusTemplate>;
struct Probe;
unsafe impl std::alloc::GlobalAlloc for Probe;
```
Global allocator is test-binary-local; alloc/realloc/dealloc counters are
thread-local and inactive during construction, enqueueing, garbage collection
and assertions. Callback wrappers compare exact zero alloc+zero dealloc.

### ROOT0238 physical acceptance checkpoint (remaining scopes pending)
Prior manifests/declarations0001–0003 precede parent registration and child
Rust. Two actual unexecuted fixtures now cover per-bus aggregate-fit versus
contiguous slot rejection with exact Box preservation, compiled Delay voice
one-over versus exact-region adoption, NativeArc PCM one-over/exact cap with
retained Arc, and fragmented Arena aggregate12/largest8 rejecting9 then fitting8.
Existing14 parent tests and assertions remain intact. Child scopedformatted.
Positive complete Native/encoded InstDef/BusDef graph closure, sustained audible
legacy twin-engine preservation, simultaneous full ACK+garbage ownership and
configured callback zero allocation/deallocation remain explicitly unfinished.
No author Cargo or empirical capacity claim. All test/ownplan writes held at
this first coherent checkpoint to permit the native-query joined checker.

### ROOT0253 positive complete closure declarations
```rust
fn native_complete_graph_closure_adopts_remaps_and_seals_silently();
fn byte_complete_graph_closure_adopts_remaps_and_seals_silently();
fn closure_inst(pcm: u32, logical: CellId) -> vactr::dsp::graph::InstDef;
fn closure_bus(pcm: u32, logical: CellId) -> vactr::dsp::graph::BusDef;
fn run_byte(rig: &mut Rig, inbox: &mut ByteInbox) -> [f32; 32];
fn collect_receipts(rig: &mut Rig, rounds: usize) -> (Vec<SongHostAck>, Vec<Garbage>);
fn complete_graph_closure(bytes: bool);
```
Five full-key leases close sample, instrument, track, control and analyzer banks.
Native and actual ByteInbox ingress must emit exact resource receipts, remain
silent, seal only after adoption, and return exact owners/capacities on cancel.
Measured per-slot extents and compiled mem_total prove actual fit. Native returned
templates prove header/input/effect/sample/Table/IR private remapping. Byte graph
Ready proves the shared validating build/map path; no synthetic host-fit claim.
Live-audio twins, simultaneous pressure and allocator probes remain unfinished.

ROOT0253 positive closure checkpoint: two fixtures and declared helpers are
written, scoped formatted, unexecuted pending mandatory checker. Actual native
transfers and ByteInbox encodings cover instrument header/UGen/effect controls,
SamplePlay/Table/IR closure, explicit analyzer banks, silent Ready sealing and
all five full-key retirement receipts. Native returned allocations prove remap
and exact pointers. Actual compiled Convolution memory and measured single bus
region fit are asserted. Original14+physical2 unchanged. No author Cargo.
Live audible twins, combined pressure and callback allocator criteria remain open.

ROOT0257: first positive cohort stopped at Clippy test compilation E0308/E0277
(node.mem_len u32 versus actual memory usize); no behavioral tests ran. Checked
u32 conversion preserves exact extent equality. Failed clippy.log retained in
/tmp/vactr-song09p-positive-graph-independent-001; no author Cargo.

### ROOT0261 future manifest handoff and live fixture declarations
PLAN ONLY. Completed native capacity provider retires from future writes with
SHA decdc6cdfe37f5b21e40f337ac60a37da60b9de1cdb98785a38224ec5d66c741.
Historical native/provider evidence and shared ownership records remain valid.
The eight future Rust paths trade that provider for cohesive
`tests/song_resource_staging/live.rs`; positive acceptance child remains unchanged.
Parent registration will be `#[path = "song_resource_staging/live.rs"] mod live;`
and child `use super::*;`. Rig retains `events_tx: ring::EventProducer`;
constructor retains both EventRing endpoints. Existing14+physical2+positive2
fixtures remain intact. No Rust/Cargo is released by this document mutation.

Exact declarations before the later source release:
```rust
fn native_live_legacy_audio_is_unchanged_by_staged_self_noise();
fn arena_live_legacy_audio_is_unchanged_by_staged_self_noise();
fn live_twins(bytes: bool);
fn audible_rig(store: StoreKind) -> Rig;
fn install_audible_legacy(rig: &mut Rig, bytes: bool);
fn live_block(rig: &mut Rig, inbox: Option<&mut ByteInbox>) -> [f32; 32];
fn compare_live_block(subject: &mut Rig, reference: &mut Rig, inbox: Option<&mut ByteInbox>);
fn self_noise_bus() -> Box<BusTemplate>;
fn probed_block(rig: &mut Rig, inbox: Option<&mut ByteInbox>) -> [f32; 32];
struct Probe;
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8;
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout);
    unsafe fn realloc(&self, pointer: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8;
}
static GLOBAL: Probe;
```
`#[global_allocator]` applies to the test binary only. Thread-local const
`CALLBACK_MEMORY: Cell<Option<(usize, usize)>>` counts alloc/dealloc; realloc
counts both. Forward System contracts with safety comments. Arm only around
actual Engine.process and disarm before assertions, receipt collection or drops.
Apply to configured sample/graph/bus adoption, failed adoption, cancellation and
blocked ACK/garbage pumping; every probed callback requires exact (0,0).

Actual audible twin engines allocate8 bus slots (not synthetic capacities),
4800 voice frames, NativeArc with actual16384-byte PCM ceiling or32768-byte Arena stores,
with configured song pools. The ceiling includes legacy and song PCM.
Legacy PCM2048 mono frames0.25 plus real oscillator/Add instrument3 use actual
NativeInstall or encoded legacy sample/InstDef/BusDef ByteInbox ingress. Retained
EventProducer schedules slot1/gen1, bus91, legato1second. Each block must retain
active voice and nonzero output; paired engines render identical block counts.
A live keyed compressor/master reads bus91 as a detector-sensitive reference.
Public bus frames, active bus identity/master and exact output are compared;
private KeySnapshot storage has no getter, so no direct snapshot claim is made.

Subject alone stages actual NoiseBlend level0dB/mix1 candidate Track ALSO bus91
and candidate Master. Exact output must remain equal through success/seal,
malformed unbound-cell failure and cancellation, despite audible candidate
self-noise if accidentally rendered. Native exact Boxes/Arcs return; Arena
encoded candidates retain full-key receipts/capacities. Frame limit<=64blocks
keeps2048-frame legacy sample alive. Constructors/encoding/enqueues stay outside
probes. Complete combined ACK+garbage saturation ownership acceptance remains
required; if helper growth reaches limit, request prior split before1000 lines.

ROOT0260 accepted positive closure only: original49398terminal0, eight gates,
18 actual staging tests,43 verified hashes. It does not complete live/pressure
acceptance, full09 playback, or allocator criteria. Proposed live child is absent.

ROOT0263 additional exact cohesive fixture declarations before Rust:
```rust
impl Rig { fn with_bus_slots(store: StoreKind, bus_slots: usize) -> Self; }
fn live_inst() -> vactr::dsp::graph::InstDef;
fn live_bus(master: bool) -> vactr::dsp::graph::BusDef;
fn enqueue_candidate(rig: &mut Rig, inbox: &mut ByteInbox, lease: SongLeaseKey, bytes: bool, malformed: bool) -> Option<*const BusTemplate>;
```
Constructor genuinely allocates eight buses; default Rig::new still allocates4.
All encoding/owner creation occurs outside probe; native legacy garbage drained
after each install. Active playback comparison stays within64 blocks of2048
frame sample, excluding startup wait before scheduled0.05second event.

ROOT0263 source framing refinement: legacy encoded graphs use actual
ring::encode_graph_record(id, gen, rawgraph, out) before ByteInbox; raw graph
bytes alone are not install records. Existing song graph wrapper stays full-key.
Active live Master Compressor kind and exact cancellation/failed-owner receipts
are asserted. Neither current source-only review nor written tests prove behavior.

ROOT0263 live checkpoint: Native/Arena twin fixtures and TLS System allocator
probe written, scoped formatted and unexecuted. Parent retains actual event
producer and constructor with measured eight-bus allocation; original18
assertions and positive child unchanged. Real legacy sample/instrument/track/
keyed master ingress, nonzero per-block equality, same-bus91/master self-noise
candidate success/seal/failure/cancel, exact returned native pointers/full-key
receipts and capacity restoration are asserted. Probe disarms before assertions,
receipt collection and owner drop. No author Cargo; all writes held for joined
checker. Simultaneous saturated ACK+garbage acceptance is still unfinished.

ROOT0264 corrects audible constructor before processing: legacy2048 floats
consume8192 PCM bytes from the SAME global NativeArc ceiling as song PCM.
Audible Rig reconfigures actual16384 ceiling through measured ACK producer;
normal Rig4096 and original18 fixtures remain unchanged. Prior separate-budget
premise withdrawn. No production change or author Cargo; new setup unexecuted.

### ROOT0274 combined retirement pressure declarations
```rust
fn native_full_reports_and_garbage_retirement_preserve_every_owner();
fn arena_full_reports_and_garbage_retirement_preserve_every_owner();
fn combined_pressure(bytes: bool);
fn pressure_adopt(rig: &mut Rig, bytes: bool) -> ([SongLeaseKey; 4], Option<*const vactr::dsp::ugen::Template>, Option<*const SampleData>);
fn fill_capacity_reports(rig: &mut Rig) -> [SnapshotEpoch; 4];
pub(super) fn closure_inst(pcm: u32, logical: CellId) -> vactr::dsp::graph::InstDef;
```
Last declaration changes acceptance helper visibility only. Sole original PCM
Arc/Template Box transfer after raw-pointer copy; no extra strong references.
Four real RequestCapacity reports fill measured producer capacity4; one actual
owned GC marker fills garbage1. Cancel while both full, free garbage first,
then consume ACK one at a time: four capacity reports FIFO, Instrument/Sample/
Control/Analysis LeaseReturned exactly once, then PreparationCancelled. Engine
release boundary is successful critical enqueue, not external POP. Before
that handoff, capacities, staged slot and banks stay charged; exact key cannot
be reserved early. Native original owners return once, Arena no graph Box.
Probe every actual callback; drops and observations after disarm. No Cargo.

ROOT0273 accepted prior live/occupancy cohort: original46165terminal0,36 gates,
150 distinct actual tests,48 current hashes. This acceptance is scoped; no full
song activation is implied. ROOT0274 now writes two combined Native/Arena
pressure fixtures, unexecuted. Four real reports fill actual ACK producer4,
owned marker fills GC1, complete four-key graph/PCM/banks already Ready.
Blocked callbacks preserve charges; GC-first handoff then one report slot opens
to admit Instrument receipt and free ONLY its slot. Sample/banks remain charged
behind full ACK, then FIFO exact four reports/four lease receipts/cancel and
sole Box/Arc ownership/capacity/extents restoration are asserted. Every actual
callback uses alloc+dealloc probe; observations/drops happen after disarm.
Acceptance helper visibility alone changed for shared genuine complete InstDef.
All sources/plan held, no author Cargo; mandatory checker proof pending.

### Final literal criterion audit — ROOT0279 / ROOT0282 / ROOT0283

**Tasks Completed**: TASK-001 through TASK-005; only SONG09P Completed.
ROOT0279 independently verified 36 gates, 153 actual distinct names and all48
held source/plan hashes; original4235 terminal0. The staging binary passed22
fixtures: original14, physical2, complete native/Arena closure2, audible twins2
and combined pressure2. Actual callback probes count both allocation and
deallocation; physical fits, full-key ownership/FIFO, private graph remapping,
silent adoption and cancellation are proven by that cohort. Earlier failed
logs and historical unexecuted checkpoints remain retained.
ROOT0283 separately satisfies the literal nextest gate:31 raw PASS/list names,
actual command exit0; parser-only harness exit1 reconciled without a rerun.
Evidence: `/tmp/vactr-song09p-nextest-followon-independent-001/reconciliation-001.json`.
ROOT0282 reconciles actual provider/physical/ownership/bank/live/pressure
evidence in parent audio-contracts (SHA19d138b0311b19c63dd041c87192be6a037932258ff536f8c79a13ec407e0cfc).
All acceptance matrix and completion criteria above refer to these current
independent receipts, not the historical preparation-only prose.
Full08B route admission,08D remaining geometry, full08/09 audible timed song
activation and10 adapters remain incomplete. No Rust, index or archive edits.
