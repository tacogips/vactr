# Consuming song host preparation implementation plan

**Plan ID**: SONG-HOST-PREPARATION
**Status**: In Progress
**Design Reference**: [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application), [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and authority

Consume an actual PreparedSong, derive and stage its immutable closed resources against genuine measured host capacity, and transfer a once-only Ready bundle after actual acknowledgments. Physical private slots are pooled from admitted reserved_generations; finite placements remain symbolic. The preparation owner never executes an entire score or claims publication from enqueue success.

Ready for implementation against freshly admitted candidates. Root releases the
exact eight-path source manifest separately. Source audits establish the demand,
finite bound, representation and receipt protocols below; genuine owner adoption
and cleanup fixtures must prove the implementation. Existing component evidence
does not establish this consuming bridge or complete song playback.

## Required dependencies

| Contract | Plan | Current dependency |
|---|---|---|
| Actual empty neutral BusDefs/typed targets | song-mode-neutral-stages.md | Scoped implementation accepted ROOT0364; actual regressions revalidated ROOT0400; owner consumption remains pending |
| Fixed graph resource provenance/pinning | song-mode-graph-resource-closure.md | Scoped closed PCM/resource acceptance ROOT0420/0422; consuming union/adoption remains pending |
| Actual-env retained Native compilation | song-mode-graph-materialization.md | Scoped compiler/retained payload behavior accepted ROOT0372, revalidated ROOT0400; owner consumption remains pending |
| Safe acknowledged physical-slot rebind | song-mode-branch-reuse.md | One-slot reuse/queue accepted ROOT0400; distinct-pool/tail/final-generation regressions accepted ROOT0420/0422; scheduler consumption pending |
| Checked commands, fullkey upload/clock | host command and clock children | Accepted lower-level evidence, revalidate APIs |
| Complete immutable route geometry | song-mode-route-geometry.md | In progress; no unresolved mapping exemption |
| Full finite scheduler/public document transfer | parent host-adapters/transport | Subsequent bounded owner handoff, not this plan |

Read-only evidence: HOST-PREPARATION/0001,0012,0013. Actual generation range proof is declared here, not inferred from physical live slot count.

## Proposed future Rust manifest

| Path | Deliverable |
|---|---|
| `src/host/caps.rs` | Public owner reexports and explicit child path registration |
| NEW `src/host/caps/song/preparation.rs` | Owned state machine, public receipt/submit/handoff |
| NEW `src/host/caps/song/preparation/resources.rs` | Closed graphs, samples, IDs and bank rewrite |
| NEW `src/host/caps/song/preparation/demand.rs` | Actual resource demand and finite range proof |
| NEW `src/host/caps/song/preparation/ledger.rs` | Bounded fullkey upload/cleanup receipt ownership |
| NEW `src/host/caps/song/preparation/pools.rs` | Physical pool descriptors without placement expansion |
| NEW `tests/song_host_preparation.rs` | Genuine Native/Arena owner integration |
| NEW `tests/song_host_preparation/pressure.rs` | Cohesive pressure/cancel ownership fixtures |

Register preparation directly from caps.rs using the explicit relative path
`caps/song/preparation.rs`; the attribute is relative to the containing host
directory, as the accepted snapshot test-child registration demonstrates.
Codec caps/song.rs remains read-only. Materialization provides actual host trait
dispatch; Native and Wasm adapters remain read-only in this eight-path wave.
Every touched Rust stays below1000; extra paths need a root amendment before writes.

## Existing owned input and public APIs

PreparedSong privately owns snapshot/evaluator/assets; sample and query expose closed lookup and bounded lazy descriptors. snapshot().routing() borrows immutable graphs, cells and topology. Crate-visible reserve, acknowledge_ready, cancel and acknowledge_applied exist. Only reserve/ready are used by this owner; actual activation publication belongs to the subsequent Ready bundle consumer.

PreparedSong reserves globally unique numeric resource IDs, even across kinds. Generate private full keys with one checked ID space per outstanding epoch, preserve kind and generation, and enforce actual encoded sample/IR numeric limits. Never use candidate InstId/BusId as a lease or infer graph PCM provenance from their numbers.

AudioHost exposes try_song_command, poll_msg, submit_song_native, submit_song_graph, submit_song_sample and exact clock observation. Materialization prerequisite adds materialize_song_native; Some retains actual compiled payload, None follows the existing borrowed graph strategy. Failure never downgrades to None. The owner does not inspect a live InstResolver or mutable namespace.

Borrowed graph admission uses the accepted try_song_graph companion: retry only
SongSubmitError::Backpressure while retaining the original GraphHandle; Unavailable
or Invalid enters exact cleanup. Do not parse Failure messages or use the legacy
submit_song_graph to guess retryability. ROOT0523 independently accepted21actual
graph/materialization/clock/host tests, including real four-kind Engine readiness
and cancellation. Native Some continues to transfer its retained compiled owner.

## Declared public owner contracts

```rust
pub struct SongHostPreparation {
    // Private owned PreparedSong, bounded state and resource/receipt ledgers.
}
pub struct SongPreparationRefusal {
    pub prepared: PreparedSong,
    pub failure: Failure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongPreparationProgress {
    AwaitingObservation,
    Uploading,
    AwaitingReady,
    Ready,
    AwaitingRetirement,
    Retired,
    Cancelling,
    Cancelled,
    Failed,
}
pub struct SongPreparationLimits {
    pub capabilities: CapabilitySet,
    pub song: SongLimits,
    pub max_resources: u32,
    pub max_pending_records: u32,
    pub max_graph_bytes: u64,
    pub max_work: u32,
}
pub struct SongPreparationCleanup {
    // Private limits, accepted full-key ledger and retained retry ownership.
}
pub struct SongReadyBundle {
    prepared: PreparedSong,
    routes: SongRoutePlan,
    resources: Vec<SongLeaseKey>,
    pools: Vec<SongPhysicalBranch>,
    stages: Vec<SongPhysicalStage>,
    clock: SongHostClock,
    cleanup: SongPreparationCleanup,
}
```

```rust
impl SongHostPreparation {
    pub fn begin(
        prepared: PreparedSong, limits: SongPreparationLimits,
    ) -> Result<Self, SongPreparationRefusal>;
    pub fn submit(
        &mut self, host: &mut dyn AudioHost,
    ) -> Result<SongPreparationProgress, Failure>;
    pub fn receive(&mut self, ack: SongHostAck) -> Result<(), SongHostAck>;
    pub fn poll(
        &mut self, host: &mut dyn AudioHost,
    ) -> Result<Option<HostMsg>, Failure>;
    pub fn cancel(&mut self) -> Result<(), Failure>;
    pub fn take_ready(&mut self) -> Result<SongReadyBundle, Failure>;
    pub fn take_cancelled(&mut self) -> Result<PreparedSong, Failure>;
    pub fn take_retired(&mut self) -> Result<PreparedSong, Failure>;
    pub fn progress(&self) -> SongPreparationProgress;
}
impl SongReadyBundle {
    pub fn prepared(&self) -> &PreparedSong;
    pub fn routes(&self) -> &SongRoutePlan;
    pub fn resources(&self) -> &[SongLeaseKey];
    pub fn pools(&self) -> &[SongPhysicalBranch];
    pub fn stages(&self) -> &[SongPhysicalStage];
    pub fn sample_resource(&self, source: &SampleSrc) -> Option<SongLeaseKey>;
    pub fn clock(&self) -> SongHostClock;
    pub fn query(
        &mut self, span: crate::pattern::query::TimeSpan,
        limits: &SongLimits,
    ) -> Result<Vec<FrozenSongEvent>, Failure>;
    pub fn submit_activation(
        &mut self, host: &mut dyn AudioHost, activation: SongActivation,
    ) -> Result<(), SongCommandRefusal>;
    pub fn receive_activation(&mut self, ack: SongHostAck) -> Result<(), SongHostAck>;
}
impl SongHostPreparation {
    pub fn retire(ready: SongReadyBundle) -> Self;
}
```

The transport moves the complete SongReadyBundle into its retained owner once.
The bundle retains the exact immutable SampleSrc-to-sample-key association built
during deduplication; resources() alone cannot recover this mapping for BANK
event encoding. sample_resource borrows that retained association and performs no
live loading or namespace resolution. Each physical branch's banks describes its
instrument graph. Private-FX banks are retained in stages/ledger under the actual
initial.config.private_fx full key; repeated logical branch IDs never identify
distinct physical banks.
Its query method delegates to that bundle's original mutable PreparedSong; no
mutable snapshot/evaluator accessor, diagnostic Rc or cloned evaluator is
introduced. Borrowed routes/pools remain immutable. The scheduler must end the
query borrow before using the returned frozen descriptors for command encoding.

retire consumes the whole Ready bundle and its original cleanup owner,
including PreparedSong, limits, accepted full-key ledger, physical mappings and
any retained transfer allocations. It reconstructs no ownership from resource
vectors. It emits no new Reserve/upload/Seal work and preserves physical pool,
stage and shared sample dependencies until actual safe returns. A bundle proven
never activated may enter Cancelling. Once activation was submitted, observed,
or remains uncertain, enter AwaitingRetirement until actual musical endpoint,
voice release and tail completion are established. Enqueue success, wall time,
a cached host frame or CancelPreparation do not prove that ordering. Applied or
partially activated resources use their exact existing full keys; retirement
must not reinitialize their generations or acknowledge cancellation locally.
The transport relinquishes ownership only by this consuming handoff. A stopped
transport retains the retiring owner until the actual completion contract below
is satisfied; query or clock failure likewise transfers to cleanup before reporting
terminal failure. A drop of the bundle is never a host cleanup acknowledgement.

Normal activated retirement and staging cancellation have distinct completion
contracts. The activated owner reaches Retired exactly once after every expected
full-key LeaseReturned and every retained/refused upload obligation is accounted
for. It does not require a subsequent CancelPreparation/PreparationCancelled:
normal engine reaping may already have removed the preparation and a cancellation
can legitimately return StaleEpoch. StaleEpoch is not completion evidence.
Unactivated cancellation still requires its genuine PreparationCancelled receipt
and exact lease returns. take_retired moves the original prepared authority once;
it does not reactivate that epoch or reconstruct its resources.

The last normal return receipt may precede engine preparation bookkeeping cleanup
by another callback. Retired proves ownership handoff, not immediate capacity or
preparation-slot availability. A later owner must use real observation/admission
and retain allocations on refusal while normal reaping finishes. Endpoint enqueue
or rejected application leaves this owner AwaitingRetirement; it cannot cancel
activated leases to manufacture successful completion.

begin owns the original PreparedSong; failed local validation returns that same ownership. No Rc<Song>, diagnostic placeholder, shallow snapshot clone or hidden mutable evaluator alias. take_ready moves its bundle exactly once. The eventual scheduler requires an explicit consuming bundle interface before its source wave; accessors alone do not authorize cloning the prepared authority.

poll processes at most one real host message. Unowned/non-song/foreign/stale receipts are returned intact to the caller; no drain-and-discard helper. receive never drains queues. Caller must retain returned messages in its bounded public routing ledger. Owned failure enters Failed/Cancelling with all accepted leases retained for cleanup; a returned Failure is not permission to drop host-owned resources.

## Measured observation and stage admission

Submit a genuine RequestCapacity(epoch), retain actual matching CapacityReport including serial and separate analysis field. Only one owner query for this epoch is in flight; capacity requests have no nonce, so same-epoch query sharing requires serialized owner authority. Ignore no public/legacy/internal reply: foreign receipt returns intact. Never synthesize the report from scalar getters, engine totals or assumed native/browser defaults.

RequestClock carries actual owner epoch and nonce; accept only matching ClockReport from Engine. Preserve exact u64 frame/u32 rate, reobserve before eventual activation if required by horizon. Capacity and clock sample rates must agree. Existing observation receipt is not atomic reservation; callbacks may reject subsequent Reserve/adoption because other owners occupied memory. Treat that real failure as a cleanup obligation, never fake success.

The latest admitted report must be invalidated by any owner cancellation/replan. No lifetime beyond actual accepted resource reservation is inferred from its serial. Exact freshness/timeout policy requires the subsequent control-side driver; no wallclock or sleeping music logic is introduced here.

## Demand and private identity contracts

```rust
pub struct SongPreparationDemand {
    pub required: SongHostCapacities,
    pub analysis: SongAnalysisCapacity,
    pub resources: u32,
    pub branches: u32,
}
pub struct SongPhysicalBranch {
    pub logical: SongBranchId,
    pub slot: u32,
    pub initial: SongReusableBranch,
    pub banks: SongGraphBanks,
}
pub struct SongPhysicalStage {
    pub target: SongDetectorTarget,
    pub lease: SongLeaseKey,
    pub banks: SongGraphBanks,
}
```

Demand child exact proposed signatures:

```rust
fn generation_ceiling(branch: &SongBranchRoute) -> Result<u32, Failure>;
fn preparation_demand(
    prepared: &mut PreparedSong, routes: &SongRoutePlan,
    remaining: &mut u32,
) -> Result<SongPreparationDemand, Failure>;
fn verify_observation(
    demand: &SongPreparationDemand, report: SongCapacityReport,
    clock: SongHostClock,
) -> Result<(), Failure>;
```

- prepare_routes already multiplies branch.occurrences by its configurations_per_placement. Use occurrences as conservative total births only after verifying density's finite total proof; multiplying configurations again is wrong.
- Allocate reserved_generations physical slots for each logical template/configuration branch, not occurrences, note count or tones. Each slot has private mutable instrument/FX state. Track/master are per typed topology target. Pool allocation work is bounded by measured physical resources, not symbolic repeat count.
- Every physical slot gets a representable initial generation and admitted maximum. Let O be the already-proved total occurrences, including configuration multiplicity exactly once. With initial generation1, last_generation=O for O in1..=u32::MAX. O0 creates no physical pool; O>u32::MAX rejects before allocation/activation. For another nonzero initial generation, checked initial+O-1 must fit u32. Never divide O by reserved_generations: physical overlap capacity does not prove a smaller per-slot lifecycle. No extra +1, generation wrap or eager-copy fallback is permitted.
- The scheduler will assign births only after safe prior tail completion and actual rebind receipt. This owner does not implement deadline selection or fullscore traversal. No note/onset/hull is a pool key; logical route placement/configuration is authority.
- Union event and fixed-site captured SampleSrc assets, deduplicate exact source/loaded-asset identities, retain original immutable Arcs and read actual PCM. SampleData.frames is interleaved: bytes=frames.len()*sizeof(f32); upload frame count=frames.len()/channels after validating channels and divisibility. Do not multiply interleaved length by channels again. Fixed graph assets are pinned before closing at session/song.rs:419, so snapshot.pcm_bytes/resource_count already include them. Derive the actual upload union without adding fixed PCM a second time; retain conservative existing demand until tighter admission is proved. One shared immutable Sample lease may serve multiple graph instances only with retained dependency ownership until all finish.
- Conservative safe first owner policy: independent control and analyzer banks for every mutable physical graph instance that references them. Copy only that graph's exact referenced scalar IDs into its sparse control bank; logical holes do not consume control slots. Aggregate copied multiplicity explicitly supersedes route.required.cell_slots one-copy counts. Preserve actual Ctl::Cell IDs in graph/event under the corresponding bank; no unpaired dense rewrite.
- Analyzer banks use full owner-local highwater including holes and overlapping ordered writers. Multiply by actual mutable graph instances; controls may not be substituted for analyzer storage. Preserve reference/writer ordering and exact sites.
- Clone graph IDs to physical owner identities, rewrite fixed PCM references to admitted sample IDs, and bind private graph banks. Typed detector target/source track map to actual privately staged bus identities. Ordinary route original-family/FX scope is unchanged.
- Inst compile uses materialization actual host environment; native compiled template mem_total and bus mem requirements must agree with physical admission. Browser reports aggregate capacities, not per-slot region extents. Actual Engine adoption remains necessary per-slot authority. A misleading aggregate fit cannot be declared Ready; real one-short/fragmented rejection must clean up.

## Implementation obligations

1. Exact private graph cell subsets, analyzer extent/multiplicity and the actual event/fixed PCM upload union need implementation and real configured host-fit fixtures; the source-derived demand contract is recorded below. Current route aggregate demand is insufficient for blindly duplicating banks.
2. generation_ceiling must verify branch.occurrences really bounds finite configuration births across every admitted mapping and symbolic placement. Existing live-generation reservation is not that proof; no source expansion allowed.
3. Physical resource ID rewrite must preserve all graph sites and event BANK exact representability; fixed closure prerequisite must exist first.
4. Current report does not describe all per-slot free extents. Actual Reserve/adoption can reject and cleanup must be proven, or a separately approved measured geometry DTO prerequisite is needed before stronger preflight promises.
5. Subsequent scheduler consumes Ready bundle, observes fresh clock, performs bounded pool assignment/rebind and actual Applied publication. This plan does not implement or waive that remaining boundary.

No new source paths are assumed to resolve these seams. A prior bounded prerequisite/manifest trade is required if actual source consumers cannot fit eight paths.

## Upload and full-key cleanup ledger

Bound resources, graphs, command cursor and receipt sets before allocation using passed limits/shared remaining work. No per-resource reset of quotas. IDs remain unique while any accepted/retiring lease exists; no early reuse.

prepare_routes currently owns a fixed1000000-work ceiling. Reserve that complete
allowance from the owner's cumulative budget before invoking it. Similarly reserve
the explicit passed allowance before materialize_route_buses. Neither helper call
silently refreshes owner work; failed work is not refunded. No queue-pressure retry
recompiles graphs or expands the score.

Stage sequence: BeginStaging exact demand; ReserveResource per full key; init controls/ReserveAnalysis; BindGraphBanks before graph upload; actual PCM and graphs; ConfigureReusableBranch physical descriptors; SealPreparation only after complete actual ResourceReady. Samples pace from real SliceAccepted; sender-held accepted data is not Ready. Duplicate/out-of-order matching ACKs cannot advance cursor twice. Rejected owned resource forces cleanup while preserving original failure.

Compiled Native payloads are stored and transferred exactly once; full queue returns same allocation to ledger. Browser keeps original immutable graph/sample ownership and existing paced transfer. Cancel unsent work off callback; staging cancellation submits exact CancelLease/CancelPreparation for accepted keys and retains ownership records until actual lease returns and PreparationCancelled. Normal activated retirement instead consumes the guarded return ledger into Retired. No receipt is simulated. One returned lease cannot complete another kind/generation. Full receipt/garbage pressure retains state and retries bounded work, without unbounded command growth or callback destruction.

Graph and sample uploads require their exact ResourceReady receipts. InitCells
and ReserveAnalysis instead require successful serialized command barriers and
matching final Seal Ready: neither command emits ResourceReady. The complete
receipt ledger invokes PreparedSong::reserve_bounded with the explicit resource
limit and shared remaining work, then acknowledge_ready. Preparation does not
publish the snapshot. Failure retains the cleanup owner and accepted obligations.

## Tasks

### TASK-001: Declare demand and prerequisite contracts

**Status**: Completed
**Parallelizable**: No

- [x] Existing owner prerequisites and freshly admitted finite route bounds grounded in source.
- [x] Source-derived exact demand, admitted occurrence/generation upperbound and encoded ID contracts.
- [x] Actual per-slot adoption/refusal and receipt-driven cleanup contracts reviewed; eight paths declared.
- [x] Root accepts refined signatures and changes status Ready before Rust implementation.

### TASK-002: Closed resource and physical pool owner

**Status**: Not Started
**Parallelizable**: No (depends on TASK-001)

- [ ] Owned PreparedSong, exact graph/site/bank rewrite, deduplicated PCM and actual demand.
- [ ] Bounded symbolic physical pools and generation representation preflight.
- [ ] Genuine capacity/clock observations; compiled allocations retained without repeat compilation.

### TASK-003: Receipt-driven upload, cancellation and Ready handoff

**Status**: Not Started
**Parallelizable**: No (depends on TASK-002)

- [ ] Every actual cursor/receipt transition, fullkey ownership and returned foreign messages.
- [ ] Actual rejection/partial upload cancellation reaches safe exact-once cleanup.
- [ ] Once-only Ready bundle with original PreparedSong; no Applied enqueue claim.

### TASK-004: Genuine Native/Arena acceptance and independent gates

**Status**: Not Started
**Parallelizable**: No (depends on TASK-003)

- [ ] Actual analog/custom instrument candidate, frozen graph/defaults/cells/analyzers, neutral stages, actual capacity/clock; real ResourceReady/Ready and original bundle identity. No live resolver or fake ACK.
- [ ] Captured sample and fixed IR union, multiple graph banks, actual one-short PCM/cell/analyzer/voice/bus extents and fragmented aggregate-fit rejection; capacities and ownership restore after cleanup.
- [ ] Actual bounded symbolic large Repeat pools, nonzero later scheduler audio witness, positive tails/current+retiring graph isolation, A→B→A and upfront unrepresentable generation rejection without expanding placements.
- [ ] Native sole Box/Arc address preservation, actual ByteInbox complete graph/sample upload, full queue/ACK/garbage and cancellation at every accepted boundary; duplicate/stale/foreign receipts cannot steal resources.
- [ ] Full versus fractional/reordered configuration identities and callback partition behavior when consumed by actual finite scheduler; handoff acceptance is separate from scheduler completion.
- [ ] Thread-local configured callback allocation/deallocation probes remain zero; fixture construction/assertions/drops outside callback.
- [ ] Fresh independent Native/browser/strictClippy/format/diff/unfiltered inventories, existing sender/staging/route regression and nextest evidence, all sources held.
- [ ] Remaining general geometry/dynamic requirements stay tracked in their route
      plans and final goal audit; owner Ready does not waive or prove them.

## Status and progress

| Area | Status | Evidence |
|---|---|---|
| Owned API and actual existing ingress | Source-inspected | PreparedSong and AudioHost methods exist |
| Neutral and materialization primitives | Scoped implementation accepted | ROOT0364/ROOT0372, revalidated ROOT0400; not consumed by this owner yet |
| Closed resource union and broader branch pools | In Progress | Exact source releases ROOT0403/ROOT0404; no closed PCM or complete pool clearance yet |
| Exact total demand and admitted finite range | Source-inspected, implementation pending | Explicit checked contracts above |
| Owner implementation/testing | Not Started | No Rust authorization/Cargo |

### 2026-10-02: Document-only owner preflight

Read current owned snapshot/capability interfaces and reviewed child plans. Corrected occurrence double counting, sparse control versus analyzer extent behavior, fixed PCM demand and native environment ownership. This plan remains Planning; it does not treat partial host component success as complete finite Song playback. Only this new document and immutable receipts changed.

### 2026-10-02: ROOT0405 dependency evidence reconciliation

Freshly inspected AudioHost::materialize_song_native, PreparedSong ownership and
the ROOT0400 accepted regression scope. Replaced stale document-only dependency
descriptions with the actual narrow primitive acceptance and current active
closure/pool source waves. The owner remains Planning: exact total demand,
finite birth-range proof, complete geometry, real adoption/cleanup and once-only
Ready bundle transfer are all still required. No owner Rust or Cargo is authorized
by this documentation update; no implementation checkbox was completed.

### 2026-10-02: ROOT0406 exact generation-range alignment

Fresh source inspection confirms prepare_routes already multiplies occurrences
by configurations_per_placement at routing/prepare.rs730–731. The future owner
uses that proved O once and the actual reusable last-generation POD contract:
initial1 implies last=O; zero has no pool; other initial values require
initial+O-1. Removed the ambiguous initial+count wording before implementation.
This arithmetic contract does not establish density for unfinished Slice,
dynamic or conditional geometry. Full route proof and genuine upfront refusal
fixtures remain mandatory and unchecked; no Rust or Cargo was changed.

### 2026-10-02 — Consuming transport and cleanup contract refinement

Read actual PreparedSong::query in src/song/snapshot.rs: it requires an
exclusive mutable borrow and returns frozen descriptors without exposing its
private evaluator. Declared the same bounded query on the future Ready bundle
and a consuming retire handoff to the existing proposed preparation ledger.
Ready ownership, query ownership and accepted-lease cleanup therefore have one
future owner at a time. These are declarations only; host preparation remains
Planning. Demand proof, real host adoption, ACK routing and cleanup verification
remain unchecked, and no Rust implementation or Ready state is inferred.

### 2026-10-02 — ROOT0468 host ownership audit refinement
Source-grounded review confirms the proposed eight host-owner paths remain
sufficient for preparation. Ready now explicitly carries the original cleanup
owner; it must retain accepted keys, limits, physical mappings and retry state.
Runtime already polls and retains song receipts, so receive is driven by one
shared dispatcher; poll is only for an exclusive standalone owner driver.
Do not let Runtime and preparation independently poll the same host queue.
ResourceReady omits kind: use globally unique outstanding numeric resource IDs;
LeaseReturned matches the complete epoch/resource/generation/kind key.
Actual Endpoints currently emits rejection on failure but no success receipt
(`src/dsp/engine/song_runtime.rs`). Activated retirement therefore remains an
explicit source prerequisite: implement and test acknowledged completion or
another source-grounded safe-return protocol before releasing dependencies.
CancelPreparation only marks leases retiring; it is not musical completion.
TASK-001 remains Planning. Dynamic K/B/N, route support and activated retirement
are not proved by metadata capture or by this doc-only refinement.

### 2026-10-02 — Normal retirement source proof refinement

Independent specialized read-only audit and root source inspection found an
existing normal ownership completion path. apply_song_end ends epoch branches;
finish_song_deadlines stops their voices and resets owned buses at the deadline;
retire_song_runtime guards all reusable epoch leases until final retirement and
actual branch/voice conditions hold. The return cursor puts Native owners into
the garbage queue before emitting full-key LeaseReturned, retaining exact owners
under garbage/ACK pressure. These facts support the distinct Retired ledger.

reap_returned_song_epochs subsequently removes the preparation silently. Therefore
a mandatory post-normal-end cancellation ACK is not a valid contract. Staging
Cancelled still waits for PreparationCancelled; normal Retired waits for all
real returns and retained upload obligations. No new completion ACK is assumed.

tests/song_branch_reuse.rs finish_pool already exercises normal Native/Arena
returns and original pointers. Analysis-bank pressure evidence currently uses
unactivated cancellation, so a genuine activated complete pool with analysis,
positive tails, actual ACK/garbage pressure, rejected endpoint paths and later
preparation admission remains a required new owner witness. No tests were run by
this audit. This resolves the declared protocol shape, not the missing consuming
owner implementation or dynamic/demand proofs; the plan remains Planning.

### Exact physical resource demand source audit — 2026-10-02

Specialized read-only audit grounds a safe implementation seam in existing public
metadata. No new production getter or ninth owner path is currently needed.
For each logical branch allocate P=reserved_generations mutable instrument and
private-FX instances; use one graph per actual track target and one master.
Do not allocate mutable state once per total occurrences.

Each physical graph takes distinct logical CellIds from certified references
matching its actual FrozenCellOwner. Instrument matching uses FrozenInstrument.name;
fixed graph resource matching separately uses its actual InstId. Copy certified
cells.value defaults for that exact subset. Sum subset sizes over physical graph
instances. Sparse IDs remain in graph/event controls; arena/song.rs:625 resolves
(bank,logicalCellId), so logical holes need no dense control allocation or rewrite.

Analyzer demand per graph is its certified analysis_banks.slots, the maximum
logical_start+width across writers. Preserve holes, overlapping writer ranges and
writer order; never sum widths or compact IDs. Sum bank extents over actual graph
instances. ReserveAnalysis still needs a contiguous physical extent: aggregate
capacity cannot promise adoption under fragmentation. Actual refusal retains the
cleanup owner and original allocation.

PCM uploads are the exact union of branch.sample and resources.entries.source,
loaded through the original PreparedSong and deduplicated by immutable identity.
Current capture already pins fixed resources before assets close. The prior text
claiming fixed IR was absent from snapshot/route PCM is withdrawn. Each interleaved
float element consumes four bytes; multiplying that length by channels was also
incorrect. Graph sample dependencies retain ownership until all graph users finish.

Preparation lease count includes physical instruments+buses+unique samples+every
nonempty control bank+every nonempty analysis bank. Include these in ledger storage
and conservative ACK demand. Keep existing voice/bus pricing and verify actual
compiled extents through real adoption. Planned sparse-control, overlapping-analysis,
shared-event/IR, multiplicity, one-short and fragmentation fixtures remain unrun.
Generation birth proofs and full dynamic geometry are separate pending requirements.
The owner plan remains Planning; this audit authorizes no Rust writes.

### Admitted finite bounds and owner implementation seam — 2026-10-02

Source audit confirms branch.configurations_per_placement retains the conservative
density components_intersecting(owner.duration), bounded by finite_total. Checked
symbolic placement multiplicity M includes Repeat count without expansion; total
occurrences O=M*C includes configuration count once. Overwrite and effect variants
may conservatively increase the corresponding branch count (prepare.rs:525,619,731).
Physical P=reserved_generations separately limits simultaneous mutable ownership;
never divide O by P to reduce the per-slot generation ceiling.

A consuming owner can advance for fresh admitted routes without waiting for every
future geometry implementation. It rebuilds routes from its original PreparedSong
and actual observed capacity, verifies O/P and encoded IDs, and allocates physical
pools only. O0 allocates none; O in1..=u32::MAX gives initial1/lastO; larger counts
fail before allocation. Unresolved Index authority fails current preparation rather
than producing a guessed finite bound. Other admitted runtime geometry failures
remain explicit failures with exact cleanup; Ready alone proves resource adoption.
General geometry and canonical dynamic result support remain full goal requirements.

The caller explicitly supplies its actual configured CapabilitySet in preparation
limits. AudioHost has no capability-profile getter; Some/None Native materialization
and scalar capacity reports cannot infer it. Exact host compilation and adoption
still validate the configured environment. No additional adapter source is assumed.

The next owner declaration must resolve activation ownership: a Ready bundle begins
NeverSubmitted, an exact refused Activate retains that state/all allocations, and a
successfully posted Activate records Submitted until actual Applied or a genuinely
correlated negative receipt. Generic epoch-only Rejected cannot silently restore
NeverSubmitted. Normal submitted/applied retirement preserves ownership until real
full-key returns; proven unactivated cancellation follows PreparationCancelled.
The exclusive activation protocol below supplies correlation without a new wire
receipt. Rejected Apply must preserve the playing song.
The eight-path owner plan remains Planning until those exact interfaces are ready.

### Serialized control completion and exclusive Apply — 2026-10-02

BeginStaging, ReserveResource, BindGraphBanks, ConfigureReusableBranch, InitCells
and ReserveAnalysis emit no success receipt. Keep one exact mutating command and
one monotonically numbered RequestClock barrier in flight. Admit no later command
for that epoch until the actual barrier reply is consumed. Queue refusal retains
the command, nonce and allocations. A preceding matching Rejected belongs to that
one control flight; consume its barrier before advancing or entering cleanup.
ClockRejected ends observation with failure and retains unresolved ownership.

Track BeginStaging admission separately from lease admission. A locally refused
BeginStaging has no host obligation. A source-correlated rejected BeginStaging,
followed by its consumed barrier, likewise created no preparation: return the
original candidate after local cleanup without awaiting a nonexistent
PreparationCancelled. Once BeginStaging succeeded, staging cancellation requires
its genuine PreparationCancelled even when no lease was accepted. Each accepted
ReserveResource adds one exact return obligation; a rejected reserve adds none.
An unresolved command or barrier retains uncertain ownership until real evidence
resolves it. StaleEpoch alone never substitutes for that evidence.

Engine::controls flushes pending critical replies before processing later controls
(engine.rs:392–440). Thus a real FIFO ClockReport after an admitted silent command,
with no corresponding rejection, proves completion of that command. Serialize
every InitCells element: a later failure retains the accepted bank and cell prefix.
Do not infer a complete bank from its first cell, enqueue success, or a duplicate
receipt. Banks require completed barriers and Seal Ready; graphs and PCM separately
require ResourceReady. Native PCM validation blocks later controls, and a clock
barrier never substitutes for upload readiness.

A Ready bundle records NeverSubmitted, PendingExclusive, Applied, or
RejectedNeverActivated. Before submitting Activate, finish every preparation
transaction and receipt obligation. Submit exactly one candidate Activate; admit
no other same-epoch command until its actual Applied or Rejected. Old active epochs
retain their independent command authority. An exact refused Activate leaves
NeverSubmitted. An accepted command leaves PendingExclusive under queue/ACK
pressure; clock observation and requested frame do not prove application.

Applied must match the candidate epoch and actual frame at or after the requested
frame. Map it to the original PreparedSong revision for acknowledge_applied;
there is no revision field on the wire. Source validation precedes activation
mutation (song_runtime.rs:148–218). Under this exclusive protocol, matching
Rejected proves that the candidate never activated. Generic interleaved Rejected
does not provide that proof. NeverSubmitted or proven rejection may cancel;
PendingExclusive and Applied retain the normal retirement owner. A caller cannot
retire by inventing rejection correlation or discarding a pending candidate.

Read-only specialized audit confirms the eight owner paths can hold the control
flight, exact full-key ledger, physical pools and retained uploads. One remaining
prerequisite is typed borrowed graph admission: existing submit_song_graph returns
Failure for both temporary queue refusal and permanent unsupported/encoding
failure. Add a separately bounded checked companion before automatic queue retry;
never classify failures by parsing messages. This audit ran no new tests and does
not change Planning status or waive scheduler/dynamic geometry requirements.

### ROOT0523 prerequisite acceptance and Ready release — 2026-10-02

The later checked graph companion resolves the typed admission prerequisite.
Root independently verified original73575 terminal0,14gates,21unique actual tests,
all913frozen inputs and raw logs. This proves the bounded sender interface and
component regressions; it does not prove the consuming owner. Previous Planning
entries above retain their historical scope.

Root now accepts the exact eight-path owner manifest and declared public owned
APIs, demand/range contracts, silent-control barrier ledger and exclusive Apply
protocol for implementation. The ledger retains one ControlFlight with exact
command, clock nonce, admission phase and correlated rejection; graph/sample
readiness obligations remain separate from bank barriers and final Seal Ready.
Owned uploads retain Native allocations or borrowed graph/sample authority on
refusal. Fresh routes, exact graph/site rewrite and symbolic physical pools stay
within the approved resources/demand/ledger/pools children.

No complete score expansion, fabricated ACK or callback allocation is permitted.
Implementation fixtures must use actual configured Native/Arena hosts and engines
and preserve old active audio through rejected Apply. General geometry, canonical
dynamic results and final scheduler/playback integration remain full goal tasks.
Status Ready authorizes the following root source release; every behavioral owner
criterion remains unchecked until independently exercised.

### ROOT0529 exact private implementation declarations — 2026-10-02

The private Assembly owns bounded resources, SampleBinding associations, pools,
stages and staged commands. Resource retains a full key, admission/readiness/return
flags and optional Upload. Upload retains original Sample Arc or remapped borrowed
graph with a once-materialized Native install. Step is a silent POD command or
upload index. ControlFlight holds exact command, posted/barrier flags and rejection;
ActivationState keeps requested activation separate from actual Applied frame.

```rust
fn charge(remaining: &mut u32, count: usize) -> Result<(), Failure>;
fn fail(message: &str) -> Failure;
fn build(
    prepared: &mut PreparedSong, routes: &SongRoutePlan,
    limits: &SongPreparationLimits, clock: SongHostClock, remaining: &mut u32,
) -> Result<Assembly, Failure>;
fn generation_ceiling(branch: &SongBranchRoute) -> Result<u32, Failure>;
fn verify_observation(
    demand: &SongPreparationDemand, report: SongCapacityReport, clock: SongHostClock,
) -> Result<(), Failure>;
fn steps(
    assembly: &Assembly, demand: SongPreparationDemand, epoch: SnapshotEpoch,
    remaining: &mut u32,
) -> Result<Vec<Step>, Failure>;
fn finish(
    assembly: &mut Assembly, routes: &SongRoutePlan, clock: SongHostClock,
    remaining: &mut u32,
) -> Result<(), Failure>;
```

Further exact module-local helper signatures will be declared in immutable
author receipts before their corresponding source increments. No Cargo/nextest.

### Owner demand/script refinement (before source)
```rust
fn from_assembly(assembly: &Assembly, routes: &SongRoutePlan,
    remaining: &mut u32) -> Result<SongPreparationDemand, Failure>;
fn steps(assembly: &Assembly, demand: SongPreparationDemand, epoch: SnapshotEpoch,
    max_records: u32, remaining: &mut u32) -> Result<Vec<Step>, Failure>;
fn checked_add(total: &mut u32, value: usize) -> Result<(), Failure>;
```
The script counts its complete records before allocating, enforces max_records,
and charges the output plus inspections against the original work counter.
Demand replaces deduplicated logical cells with actual per-bank initialized cells,
sums analyzer extents separately, and counts union PCM exactly once per sample
resource. Route voice/bus memory remains the existing admission proof. Generation
ceiling is the entire logical occurrences count, checked into u32 without division
by physical slots. Zero occurrences allocates no pool.

### Resource assembly private declarations (before source)
```rust
struct Builder<'a>;
impl Builder<'_> {
    fn key(&mut self, kind: SongResourceKind) -> Result<SongLeaseKey, Failure>;
    fn sample_id(&mut self, source: &SampleSrc) -> Result<u32, Failure>;
    fn banks(&mut self, graph: SongLeaseKey, owner: FrozenCellOwner)
        -> Result<SongGraphBanks, Failure>;
    fn bus(&mut self, graph: &BusDef, target: SongDetectorTarget,
        kind: SongResourceKind) -> Result<SongPhysicalStage, Failure>;
    fn instrument(&mut self, branch: &SongBranchRoute)
        -> Result<(SongLeaseKey, SongGraphBanks), Failure>;
}
fn float_id(id: u32) -> Result<f32, Failure>;
fn replace(params: &mut [(CtlId, Ctl)], parameter: CtlId, id: u32)
    -> Result<(), Failure>;
```
Builder borrows immutable snapshot topology and original remaining counter and
owns only bounded Assembly. Samples are first in the ledger; independent graph
banks precede their graph resource. Every graph resource remap uses issued owner
and exact declaration site. Node BANK/Table sites use their actual header binding,
not coincidental numeric IDs. Physical track stages are allocated before private
FX sidechain remapping. All copied nodes/params/site searches are charged.

### Forward detector references (before source refinement)
Builder retains a bounded Vec<(KwId, SongLeaseKey)> of preassigned track keys.
Every track key is allocated once before graph rewriting. Detector references
resolve through that complete private map, including later declaration order;
track graph materialization uses its preassigned full key. This changes no
public helper signature, price or authority. Arc graph/name extraction precedes
mutable builder calls to avoid borrowing the builder through graph inspection.

### Closed source association cost/active union refinement
```rust
fn source_words(source: &SampleSrc) -> Result<usize, Failure>;
```
Source path byte comparisons and shared descriptor copies are admitted before
lookup. The PCM union includes only actual physical graph owners materialized
for the route plus event branch sample bindings, avoiding historical uninstalled
graph declarations. AudioEvent and graph IDs are u32 in actual encoders; float
roundtrip checks apply only to resource-valued controls. Graph copy/encoding work
also includes embedded effect parameters before allocation.

### Typed resource controls and exact float range refinement (before source)
Instrument node remaps keep separate BANK, table and source IDs derived from
actual issued Header control IDs. Event sample selection uses the captured
resource keyword's issued header site; builtin sample graphs use their explicit
BANK parameter. Header authority never comes from node numeric placeholder IDs.
float_id compares f64::from(encoded) to f64::from(id), avoiding saturating casts.
```rust
#[test] fn resource_control_ids_require_exact_float_representation();
```

### Genuine owner test cohort declarations (before source)
```rust
struct Rig;
enum Backend;
fn candidate(code: &str, epoch: SnapshotEpoch) -> PreparedSong;
fn limits(capabilities: CapabilitySet) -> SongPreparationLimits;
fn callback(run: impl FnOnce());
impl Rig {
    fn new(bytes: bool) -> Self;
    fn tick(&mut self) -> Vec<HostMsg>;
    fn complete(&mut self, owner: &mut SongHostPreparation) -> SongReadyBundle;
    fn cleanup(&mut self, owner: &mut SongHostPreparation);
}
#[test] fn original_candidate_becomes_actual_native_and_arena_ready_once();
#[test] fn closed_pcm_union_and_physical_graph_banks_are_actually_adopted();
#[test] fn exclusive_activation_and_normal_retirement_preserve_original_authority();
#[test] fn never_activated_ready_cancels_only_after_exact_actual_returns();
#[test] fn foreign_receipts_and_wrong_clock_nonce_are_preserved();
#[test] fn local_work_and_resource_failure_preserve_original_candidate();
```
Native uses actual NativeAudioHost headless configuration; Arena uses actual
WasmAudioHost/outbox/ByteInbox/configured Engine and forwards actual HostMsg to
HostState before owner dispatch. Callback probes arm only around actual renders.
No successful receipt is manufactured. Foreign/stale/wrong nonce challenges are
explicit adversarial inputs and cannot establish readiness. Full actual owner
pressure and refused Apply old-active isolation are a separate cohesive child.

### Completion transfer and receipt-work declarations (before source)
```rust
impl SongReadyBundle {
    pub fn applied_activation(&self) -> Option<SongActivation>;
    pub fn activation_rejection(&self) -> Option<SongRejectCode>;
}
```
Ready transfer pre-admits full-key projection before taking the original owner.
The script pre-admits bounded expected receipt-ledger scans (records times
resource count), in addition to command storage, without resetting caller work.
Actual Applied/rejection getters expose only observed immutable results, never
infer transport activation. Full original-candidate refusal intentionally uses
an inline Result owner rather than allocating a Box to appease lint size.

### Actual transport pressure cohort (before source)
```rust
struct GateHost;
fn upload_pointer(install: &NativeSongInstall) -> usize;
fn fill_commands(host: &mut dyn AudioHost) -> usize;
#[test] fn actual_queue_pressure_retains_once_materialized_native_box_and_browser_arc();
#[test] fn refused_exclusive_activation_retains_ready_and_old_active_owner();
#[test] fn cancellation_during_actual_partial_upload_waits_for_all_owned_returns();
#[test] fn finite_repeat_pools_do_not_expand_logical_placements();
```
GateHost delegates to actual Native/Wasm hosts. Its first graph submission fills
actual command transport with foreign muted commands, then records the genuine
refusal and allocation address. Successful receipts all come from Engine.
Refused activation fills real transport while an older activated epoch remains
owned; caller retains exact refused command and Ready state, then retries.
Partial upload uses a PCM larger than one browser slice and actual SliceAccepted
pacing. No synthetic Ready/Applied/LeaseReturned establishes any success.

### Positive PCM and old-active witness refinement (before source)
```rust
fn candidate_with_pcm(code: &str, epoch: SnapshotEpoch, floats: usize) -> PreparedSong;
fn start_old_event(rig: &mut Rig, ready: &SongReadyBundle);
```
Partial-upload fixture uses 2*SLICE_FLOATS immutable floats so cancellation occurs
between genuine sender-paced slices. Rig stores last actual PCM after probe
scope; never-activated staging must remain exactly silent. Old-active pressure
witness starts a real scheduled oscillator event and proves nonzero actual audio
continues during refused activation, in addition to full Ready ownership.

### Bounded browser relay correction (before source)
```rust
Backend::Arena { pending: std::collections::VecDeque<Vec<u8>>, /* existing fields */ }
GateHost { bytes: bool, /* existing fields */ }
fn bound_browser_pressure();
```
Pressure fixes the genuine empty ABI outbox to 4096 bytes. Relay retains every
actual framed record in FIFO outside callback, accepting only what the measured
ByteInbox can hold per block. Outbox growth cannot serve as a refusal witness;
no overflow record is discarded or converted into synthetic success.

### Final atomic transfer and copy-work review (before source)
Existing helper signatures remain unchanged. A local failure clears only an
unposted control flight; submitted ownership remains unresolved until its real
barrier. Record accepted Begin/Reserve ownership before refusing an inconsistent
clock. Ready verifies required fields before moving the original owner. Resource
record copies and boxed Stage/Frame payload arrays are charged before allocation;
script and full-key receipt comparison words share the original work counter.

### ROOT0529 author checkpoint (held for mandatory checker)
Implemented consuming original-candidate owner, actual capacity/clock barriers,
closed graph/sample/bank remap and demand, retained native allocation retries,
serial actual Ready/activation/normal-retirement/cancel receipt ownership, and
finite physical pools. Removed redundant private demand cache; verification and
script use the same computed demand before host submission.
Written ten genuine public Native/Arena owner fixtures plus private exact-f32
resource-ID boundary test. Actual browser pressure fixes the empty outbox to
4096 bytes, retains excess records across the measured 16-slot inbox, and never
fabricates a queue refusal. Callback allocation/deallocation probes are written.
Scoped rustfmt/check passes; no author Cargo or behavior execution. No criterion
is marked tested from source alone. Independent joined verification follows.
The provider sample-reservation metadata ceiling is a separately tracked typed
admission prerequisite; this small fixture cohort does not prove arbitrary PCM
union provider admission. Exclusive activation retains the current reviewed
contract; exact initial-onset pipelining/correlated rejection is a subsequent
prerequisite, not verified timed playback. Full geometry/dynamic/scheduler and
transport consumption remain distinct and incomplete.

### ROOT0544 native module-resolution repair
Joined007 native check failed101 before any behavioral tests executed. Explicit
path-mounted preparation resolves children relative to caps/song; declare
`#[path = "preparation/<child>.rs"]` for demand, ledger, pools and resources.
Only registration paths change; actual child files, algorithms and fixtures
remain unchanged. Original native failure/log hashes retained by ROOT0544.
Scoped format/check0; all eight source files and this plan resealed for the
mandatory checker. No author Cargo or behavior clearance.

### ROOT0546 strict Clippy representation repair (before source)
Native and Wasm gates0; strict Clippy failed before tests (large enum variant and
duplicate cancellation arms). Actual008 logs/hashes retained by ROOT0546.
```rust
enum Step { Control(Box<SongCommand>), Upload(usize) }
fn control(command: SongCommand) -> Step; // ledger-private
```
The ledger charges each boxed command's actual word size before constructing the
script on the control thread. Retries borrow the box and copy its original POD
into the flight; no retry allocation or mutable command reconstruction. Combine
identical begun/flight guards without changing their cancellation truth table.

### ROOT0548 cancellation fixture compile correction
Joined009 native/Wasm0; Clippy fixture compilation failed101 before tests.
`Iterator::any` receives `&SongHostAck`, so the no-Ready cancellation assertion
uses one dereference. Preserve the exact assertion and all production behavior.
Original failure receipt retained; scoped format/check0, no author Cargo.

### ROOT0550 fixture backend representation correction
Joined010 native/Wasm0; strict Clippy failed101 before tests on Backend size.
`Backend::Native(Box<AudioSide>)` boxes the actual AudioSide exactly once during
fixture construction, outside the armed callback probe. Rendering and all real
owner/PCM assertions remain unchanged; no lint allowance or synthetic backend.
Scoped format/check0; original failure retained, no author Cargo.

### ROOT0556 actual owner diagnosis (before semantic fixture repair)
Joined013 native/Wasm/strict Clippy0 and 521 prerequisite tests passed; owner
six passed/four failed. Distinct keyword Builtin and resolved Inst families are
retained by inventory, so physical pool assertions derive sum(reserved_generations)
from authentic routes, and stages = private pools + shared tracks + master.
Compile decisions count actual graph-kind leases, preserving once-only allocation
retention. Applied is a real asynchronous receipt: use bounded callback polling
and preserve foreign replies, not a single-tick success assumption.
Ordinary non-source Repeat at tail0 currently uses global overlap +2, distinct
from source-cover placement overlap1. Million-repeat fixture remains unchanged
pending root decision on the unowned routing admission prerequisite; no music,
capacity, assertion or production cancellation relaxation. Partial PCM cancel
actually passed and remains unchanged.

### ROOT0556 owner fixture repair held; ROOT0557 dependency
Physical counts now follow full admitted route authority and exact graph leases;
initial Apply waits at most16 real callback polls. Native free bus count5 is
source-grounded: default6 slots minus the live legacy master. Ordinary Repeat
zero-tail overlap2 per retained keyword/Inst family can demand6 buses; ROOT0557
releases the disjoint half-open ceil+1 routing repair. The actual million-repeat
input and capacity remain unchanged. After explicit root clarification, assert
O=1_000_000 and P=1 for EVERY retained family route and exactly one matching
physical pool/full-key generation ceiling per route, rather than guessing one
family globally. The separate production bound must pass independently.
All owner sources/plan held after scoped format/check0. No author Cargo; current
three fixture corrections and routing prerequisite remain behavior-unverified.

### ROOT0558 configured Native concurrent preparation prerequisite
A new validated headless_with_config allocates12 actual Native bus slots before
callback, matching Arena's existing configuration. Native fixed render quantum,
integer32768 rate, capabilities/voices/state budgets and full PLAIN music remain
unchanged. The complete old four-bus candidate and new four-bus candidate must
coexist without early retirement. Genuine measured report and invalid-config
fixtures added in song_hosts; independent behavior execution remains pending.

### ROOT0564 actual activation-pressure timing repair (before source)
Joined018 native/Wasm/Clippy0; 534 tests passed and owner9/10 passed. Original
pressure test retried the exact POD after its requested current frame had passed,
without proving Native1024/browser151 foreign command rejection backlog drained.
Request actual activation8192 frames ahead before refusal. Drain every measured
filled command's genuine epoch9999/StaleEpoch Rejected within256 callbacks,
preserving foreign receipt authority and nonzero old PCM. Retry the unchanged POD
strictly before deadline and await real Applied through at most1024 callbacks;
actual application frame must be at least requested. Original music, owner and
configured capacity remain unchanged; no Engine late-validation or protocol edit.

Pressure bound source proof: Native CONTROL_CAPACITY1024 and ACK_CAPACITY8192
permit every foreign command rejection without receipt capacity loss; Arena
relays16 records per callback into128 ACK slots. Fixed4096 bytes permits fewer
than1024 complete Mutes. Thus even using the slower relay64 callbacks suffice;
256-drain guard is conservative, and8192 ahead equals512 sixteen-frame
callbacks. Exact measured filled count and actual returned ACK count are checked,
with retry-before-frame mandatory. No success inferred from these bounds alone.
