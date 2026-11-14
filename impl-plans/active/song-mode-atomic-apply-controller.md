# Atomic song Apply controller integration

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Implementation review](../../design-docs/specs/design-song-mode.md#implementation-review-2026-10-03)

## Purpose and dependencies

Complete whole-code Apply while a finite song is Playing or Draining, with an
exact musical boundary, acknowledged mute continuity, authenticated old-owner
retirement, and document-edit cancellation. Initial playback remains supported.
Depends on the prepared-owner handoff and verified atomic DSP transaction.
The heterogeneous host profile is required for production acceptance of the
unchanged example; it does not replace controller correctness.

Current Runtime starts every Ready candidate independently through start_song;
it has no persistent acknowledged mute overlay. Therefore current initial Apply
checks are not evidence of playback replacement. Original scope includes Native,
separate browser session/worklet instances and requester publication.

## Manifest: eight modules

| Path | Deliverable |
|---|---|
| `src/sched/runtime/song.rs` | Retained owner fields, receipt dispatch, integration and cohesive extraction |
| NEW `src/sched/runtime/song/apply.rs` | Exact boundary selection, retained replacement handoff and commit |
| NEW `src/sched/runtime/song/overlay.rs` | Acknowledged family state and bounded semantic equivalence |
| `src/song/assets.rs` | Bounded readonly closed-asset provenance lookup, without backend access |
| `src/song/snapshot.rs` | Readonly immutable certificate access through the snapshot boundary |
| `src/session/song/transport.rs` | Correlated requester success/failure and revision cancellation |
| NEW `tests/song_apply_controller.rs` | Genuine Native/Arena controller, audio and pressure evidence |
| NEW `editor/test/wasm/song_apply.test.ts` | Parent-owned real separate session/worklet replacement evidence |

Rust author owns the seven Rust paths; parent owns TypeScript. Runtime parent is
currently 888 lines: extract existing mute handling cohesively into overlay.rs
before adding Apply integration. Every touched Rust file must remain below
1000 lines. Assets and snapshot parents are both currently 934 lines. Keep each accessor
small; if either needs substantial growth, split certificate preparation into
a prerequisite plan with a cohesive extraction before touching it. No additional
source paths are authorized. Source implementation starts only after the root
releases these seven Rust paths to the specialized author.

## Types and API contracts

```rust
struct FrozenFamilyCertificate {
    declaration_name: Rc<str>,
    instrument: FrozenInstrument,
    cells: FrozenCellInventory,
    assets: Vec<FrozenAssetBinding>,
}
struct FrozenAssetBinding {
    site: GraphResourceSite,
    logical_source: Rc<str>,
    member_index: Option<u32>,
    data: Arc<SampleData>,
}
struct AcknowledgedMute {
    sound: FrozenSound,
    instrument: u32,
    muted: bool,
}
struct PendingReplacement {
    previous: SnapshotEpoch,
    next: SnapshotEpoch,
    activation_frame: u64,
    overlay_nonce: u64,
    invalidated: bool,
}
fn replacement_boundary(
    clock: SongHostClock,
    old_activation: u64,
    settings: SongSettings,
    minimum_safe_frame: u64,
) -> Result<u64, Failure>;
fn certify_frozen_family(
    prepared: &PreparedSong,
    sound: &FrozenSound,
    remaining_work: &mut u32,
) -> Result<Option<FrozenFamilyCertificate>, Failure>;
fn equivalent_frozen_family(
    old: &FrozenFamilyCertificate,
    next: &FrozenFamilyCertificate,
    remaining_work: &mut u32,
) -> Result<bool, Failure>;
```

These are proposed internal contracts. Build certificates while the genuine
Ready owner exposes its original PreparedSong, before consuming it into transport.
Retain immutable certificate data through old cleanup; do not add an accessor
that lends the original mutable evaluator or resource authority. Validate closed
asset/provenance access before implementation; the readonly contracts below
resolve this readiness prerequisite. A name or numeric ID alone cannot certify
semantic equality. Keep the opaque source/commit APIs from the handoff phase;
never construct proof objects or manufacture Applied receipts in fixtures.

## Exact timing and ownership

Choose the earliest old-song cycle boundary after the maximum already committed
old scheduling horizon and the genuine current clock plus host delivery lead,
with 64 frames reserved before activation for arming. Use checked rational
seconds-per-cycle and sample-rate arithmetic. Evaluate each candidate boundary
from old_activation + round_ties_up(k * seconds_per_cycle * sample_rate), rather than
repeated addition of rounded frame durations. No float-derived audio clock.
Overflow is a typed refusal retaining original ownership and old playback.

Bind a preparing replacement to the acknowledged old owner when preparation
starts, not only when Ready arrives. The old song may naturally finish while
the new candidate is preparing. Retain that transport's activation/endpoint
authority and immutable family certificates until the pending replacement has
a definitive outcome, even if ordinary resource retirement finishes meanwhile.
Do not let the current Ended-owner removal path discard a promised old ticket.
Failed or cancelled preparation releases this retention without extending audio.

The selected boundary can exceed the old arrangement or natural tail deadline.
Do not postpone replacement until Ended or extend old tails. Retain the genuine
old source ticket/completion witness while its ordinary resources retire.
The new arrangement starts at local zero at its selected exact frame.

Wait for outstanding old mute acknowledgements before freezing the overlay;
continue old playback while waiting. Allocate an exact checked nonzero overlay
nonce and prime the newly certified physical instrument families before any
new onset. Retain the actual source ticket and overlay on host backpressure or
handoff refusal. Do not repeatedly ask the old transport for a one-shot ticket.

Keep old scheduling through pre-boundary time, but do not submit old work at or
after the selected boundary once the replacement is admitted. Route genuine
Applied(new,A) through the new original owner, take its opaque commit exactly
once, and deliver it to the authenticated old owner. Wrong-owner refusal returns
the proof for correct retry. Never queue a second old Endpoints cutoff.

Only publish requester Applied after the genuine exact host outcome. Ready is
preparation only. Publish the newly acknowledged immutable definition/resource
catalog alongside that outcome; rejected or invalidated Apply must retain the
old applied catalog. Use retained immutable candidate metadata, without lending
its evaluator or switching its frozen resource lookups to current upload maps. Rejection before arm retains the old catalog and audible owner.
A late success after invalidation must not publish requester success or reset a
Failed request; retain actual ownership for safe cleanup according to the DSP
cancellation outcome. Concurrent pending Apply is bounded and explicitly refused
or serialized without losing either original owner.

## Semantic mute equivalence

Record only acknowledged family state. Update the persistent physical-family
state for each authenticated actual Muted receipt, including aliases sharing
that physical instrument. A partially acknowledged multi-family request may
have changed some actual gates even before its complete requester result; do
not infer that all families reverted if remaining submissions fail. Serialize
the pending request to a definitive outcome before taking replacement overlay.
Numeric InstId, CellId, FileId, analysis
indices and allocation generations can change when declarations are reordered.
Compare declared names, normalized graph topology/parameters and copied defaults
by their frozen declaration sites. Preserve alias/sharing relationships where
observable. Resource-valued controls require their issued graph bindings and
closed immutable asset identity, including bank member order and decoded PCM.
Asset lookup is read-only over the closed pinned inventory. It must not call
current upload maps, backends or mutable evaluator state. Retain Arc PCM data
without copying the full waveform into the certificate; compare canonical
source/bank identity and exact decoded contents within the work budget. A buffer
with no stable logical provenance cannot issue a certificate.
Do not use a hash alone as equality proof. Charge all graph/asset comparison work
against a finite bound; use existing immutable snapshots, not current catalogs.

Unchanged named definitions survive declaration reorder. Changed definitions,
renamed/removed families, changed banks/PCM and uncertifiable dynamic provenance
do not inherit a mute. This intentional drop must not accidentally unmute an
unchanged family because numeric IDs shifted. Old acknowledged state survives
rejected replacement. Static export ignores interactive mute state.

## Tasks

| Task | Status | Dependency |
|---|---|---|
| TASK-001: validate snapshot access and extract existing mute code | Pending | Handoff API |
| TASK-002: bounded acknowledged semantic overlay | Pending | TASK-001 |
| TASK-003: exact boundary and retained controller transaction | Pending | TASK-002, DSP acceptance |
| TASK-004: cancellation and requester publication | Pending | TASK-003 |
| TASK-005: real Native/Arena and fresh WASM acceptance | Pending | TASK-004, physical host profile |

## Completion criteria

- [ ] Initial playback and finite Ended remain correct.
- [ ] Playing and Draining Apply use exact old-cycle boundaries and local-zero restart.
- [ ] Failed pre-arm Apply leaves old PCM and active catalog unchanged.
- [ ] No old onset/tail leaks after A; new muted families have no first onset.
- [ ] Reorder preserves unchanged mutes; changed/removed/renamed definitions drop them.
- [ ] Pending mute, command pressure and repeated Apply retain exact original owners.
- [ ] Document edit cancels accepted pending replacement before arm through actual receipts.
- [ ] After-arm cancellation preserves actual activation ownership without false success publication.
- [ ] Natural old completion before A works without endpoint extension or reused-slot reset.
- [ ] Real Native/Arena and fresh WASM tests verify audio and exact correlated outcomes.
- [ ] Independent checks pass and files satisfy size limits.

## Progress

2026-10-03: Planning from current Runtime and Session source inspection. DSP and
resource layout implementation proceed in their own phases. No Rust source is
changed or authorized by this plan; semantic snapshot access remains a required
readiness decision. Full song-mode completion remains unproven.

2026-10-03 parent evidence: added two actual separate session/worklet fixtures in
editor/test/wasm/song_apply.test.ts (169 lines). TypeScript check passed. The
previous artifact b29cfd442341ed5544d1fb9058d4fa1acfb0bbff2f7782d9157626d80f54ccca
produces actual second-Apply bus_slots capacity refusal in both tests; original
process9149 exited1. Log /tmp/vactr-song-apply-baseline-001.log. Initial Playing
audio and, in the second fixture, actual mute acknowledgement precede the
refusal. Neither test is accepted: fresh profile/DSP/controller implementation
and rebuild are still required. No skipped fixture or fabricated host outcome.

2026-10-03 fresh profile/DSP artifact baseline: pure WASM build20596 exited0,
artifact fc5c9ee0bb1dbfa92b6df13365e4d7c61f4b3eeedbd37b0f1da0d610b8413ceb.
Five initial-playback/mute/profile ABI tests pass (process51841exit0), including
genuine bootstrap55 templates/50 free buses and invalid reinitialization retaining
a pending exact-u64 capacity request. Log /tmp/vactr-browser-song-profile-001.log.
The two replacement fixtures now reach real second Applied without capacity
refusal, but still fail: actual boundary differs from the old cycle grid by
7424 frames, and the supposedly carried mute permits new PCM. Process99164exit1,
log /tmp/vactr-song-apply-profile-baseline-001.log. This isolates the missing
controller/overlay integration; neither fixture is passing acceptance.

Readiness follow-up: recovery of the one-shot old source after genuinely
never-activated cancellation is a prerequisite for repeated Apply. The existing
source_issued flag otherwise stays true permanently. The handoff recovery phase
must return the exact original opaque source only through authenticated cleanup,
then reclaim it on the matching old transport; after-arm Applied cannot return
that source. The controller must preserve/reuse that returned authority after
failed requests, without inventing a new ticket.


### 2026-10-03: readonly certificate access reviewed; Ready

Closed assets already own the original `Arc<SampleData>` in a private BTreeMap.
Existing SampleLoader lookup only reads that closed map, including validated bank
index wrapping; it does not need backend access. Add small crate-only readonly
accessors in the two already listed parents and reuse their closed lookup logic.
Do not clone SampleData, because that copies its boxed PCM waveform.

```rust
impl PinnedSongAssets {
    pub(crate) fn closed_sample(&self, source: &SampleSrc)
        -> Result<Arc<SampleData>, Failure>;
    pub(crate) fn closed_bank_geometry(&self, bank: KwId)
        -> Option<(u32, bool)>;
}
impl SongSnapshot {
    pub(crate) fn closed_sample(&self, source: &SampleSrc)
        -> Result<Arc<SampleData>, Failure>;
    pub(crate) fn closed_bank_geometry(&self, bank: KwId)
        -> Option<(u32, bool)>;
}
```

The bool denotes the original closed bank wrapping policy. PreparedSong already
exposes a readonly snapshot, so no third forwarding path is necessary. Charge
lookup and retained metadata against the controller's remaining work budget.

Issued FrozenGraphResource provides owner/site/source accessors. Its bank binding
currently records member zero, not a certificate for the whole bank. Compare the
closed bank count, wrapping policy and every member in original order; retaining
only member zero is insufficient. Frozen routing exposes source paths, cell
reference sites, copied scalar defaults and analysis writer order. These allow
allocation IDs to be normalized without opening the evaluator. Relative paths
need their original declaring-file provenance; missing or ambiguous provenance
means no certificate. Buffers without stable provenance likewise cannot certify.
Preserve graph cell sharing and writer relationships instead of comparing only
scalar values. Resource comparison checks exact immutable PCM under a finite
work budget, with Arc identity available as a safe fast path after provenance
checks. No current catalog lookup or hash-only proof.

Pressure and source-recovery phases remain prerequisites for joined acceptance.
This readiness review adds no Rust implementation and claims no passing controller
fixture. The two real browser failures remain required acceptance targets.


2026-10-03 parent cancellation/retry acceptance: added a third real browser
fixture. It observes the genuine emitted Replace command before F, sends an
actual document revision change, requires correlated rejection at original A,
actual PreparationCancelled and once-only resource returns, preserves audible
old playback, and only then submits a new Apply. The helper permits only the
expected failed request22; other candidate failures and real worklet faults
still fail. No synthetic ACK or elapsed-time cleanup shortcut.
TypeScript check exited0. Existing fc5c9ee0bb1dbfa92b6df13365e4d7c61f4b3eeedbd37b0f1da0d610b8413ceb
artifact ran all three fixtures in original process63198, exit1. Original two
failures remain; third fails because no compound Replace is posted by the old
controller. Log /tmp/vactr-song-apply-retry-baseline-001.log. This is required red
acceptance evidence, not a completion claim; fresh controller build is pending.


Controller timing seam review: SongTransport does not expose its private cursor,
and its own advance may realize one canonical cycle per call while retaining
pending commands. Track a checked monotonic bound in the already-authorized
Running controller record for every offered query horizon. An offered horizon
is a conservative bound even when command submission is backpressured; it need
not claim all offered work actually committed. Select A beyond that bound and
actual clock/delivery lead. Retain the exact selected integer cycle k alongside A
so subsequent old queries can be clamped at that cycle once Replace is admitted,
without queuing a second Endpoints or accessing the private cursor. Controller
tracking is required from initial owner creation onward. A new source path or
transport accessor is not authorized by this seam review.


Applied overlay publication review: editor Store deliberately clears the old
per-epoch mute rows on CandidateApplied. It cannot remap changed numeric IDs from
old UI history. After genuine Applied certifies the exact primed new gates,
publish the carried mute state for each new selector at A using existing
SongInstrumentMutedBody with no requester correlation. Emit CandidateApplied
before these carried-state publications, so Store accepts their new epoch. Emit
only from actual Applied and the retained certified overlay; PrimeMute submission
or Ready is not sufficient. Do not manufacture a separate DSP Muted receipt.
A dedicated bounded runtime notice may carry this state to the already listed
Session transport module; no new protocol/editor module is required.
Changed/removed/uncertifiable definitions receive no inherited muted row. Failure
before activation retains the old epoch's rows. Native/Arena and browser tests
must verify carried-state publication as well as silent new audio.


Overlay serialization contract: before overlay freeze, let any already accepted
mute request reach its genuine definitive receipts. Once priming starts, the
existing DSP protocol forbids changing a primed family or reprime with the same
nonce. Until that replacement has a definitive outcome, reject any new old-epoch
mute request explicitly before sending a mute command. This serializes the
exclusive handoff without acknowledging a gate change absent from the carried
overlay. Rejection preserves old acknowledged state and lets the caller retry
against the actual active epoch after the transaction. Ordinary mute remains
available while candidate preparation is still unprimed. Verify this boundary
with actual commands and receipts; do not silently ignore or optimistically
acknowledge a submitted request.

Current browser acceptance rerun after carried-state assertion: TypeScript
check exited0. Original86625 exited1 against the same fc5 artifact, all3 red:
old cycle offset7424; no actual-Applied carried state publication; no compound
Replace for cancellation/retry. Log /tmp/vactr-song-apply-overlay-baseline-001.log.


Controller prerequisites behavioral checkpoint: independent original68596exit0
passes all17 focused host/runtime tests, including all five source-recovery and
all five pressure/lifecycle additions. Root read raw complete output and
independently matched all16 held source/plan hashes to pressure0006/recovery0008.
Private endpoint1 and fulltransport4 also pass. Broad native/WASM/lint remains
running; seven controller Rust sources remain unmodified/held. This advances
the original live Apply implementation prerequisites, without accepting any
of the three red separate-session/worklet controller tests.

### 2026-10-03: scoped lint source release before controller features

Root releases only runtime/song.rs future_frame range expression after joined
checkpoint: host8/runtime9/endpoint1/transport4 (22 actual tests) and native/pure
WASM passed; strict Clippy found manual_range_contains at runtime866. Preserve
finite guard and inclusive0..3600 duration validation. Before intent
SONG-APPLY-CONTROLLER/0001-range-lint-intent.json. Controller features remain
unauthorized until joined lint/format terminal. Recovery source/plan unchanged.
No author Cargo or overall acceptance claim.

### Controller before-source declarations

Resolve family through Ready's actual route resolved instrument, rather than
guessing a builtin's declared name. The certificate keeps normalized copied
graph/default/site relationships and original ordered Arc assets. Per-candidate
controller comparison work is admitted from supplied max_work and retained through
Ready, construction and all retry stages, without reset on pressure. Direct Ready
uses the explicit finite SongLimits.max_nodes controller ceiling.

```rust
struct FamilyRecord { sound: FrozenSound, instrument: u32,
    certificate: Option<overlay::FrozenFamilyCertificate>, muted: bool }
fn certify_frozen_family(prepared: &PreparedSong, sound: &FrozenSound,
    instrument: crate::dsp::graph::InstId, remaining: &mut u32)
    -> Result<Option<FrozenFamilyCertificate>, Failure>;
fn build_family_records(ready: &SongReadyBundle, remaining: &mut u32)
    -> Result<Vec<FamilyRecord>, Failure>;
fn charge(remaining: &mut u32, count: usize) -> Result<(), Failure>;
fn normalize_control(control: &mut crate::host::wire::Ctl,
    cells: &crate::song::snapshot::FrozenCellInventory,
    ids: &mut Vec<crate::dsp::cells::CellId>, values: &mut Vec<u32>,
    remaining: &mut u32) -> Result<(), Failure>;
fn normalize_graph(graph: &crate::dsp::graph::InstDef,
    cells: &crate::song::snapshot::FrozenCellInventory, remaining: &mut u32)
    -> Result<(crate::dsp::graph::InstDef, Vec<u32>), Failure>;
fn asset_binding(prepared: &PreparedSong, site: Option<crate::dsp::build::GraphResourceSite>,
    source: &crate::host::caps::SampleSrc, remaining: &mut u32)
    -> Result<Option<FrozenAssetBinding>, Failure>;
impl Runtime {
    fn start_ready(&mut self, ready: SongReadyBundle, clock: SongHostClock)
        -> Result<(), SongTransportRefusal>;
    fn drive_replacements(&mut self, rep: &mut TickReport);
    fn retain_promised_old(&self, epoch: SnapshotEpoch) -> bool;
}
```

Existing Runtime mute methods move intact before refinements. Additional exact
helper declarations will precede their source writes within this same manifest.

Controller metadata refinement before writing: certificate asset bindings retain
`site: Option<GraphResourceSite>`, `logical_source: Rc<str>`, `members:
Vec<Arc<SampleData>>`, and `wrapping: bool`; all bank members are represented.
Certificates retain normalized InstDef, scalar bit defaults and analyzer-relative
relationships. Unsupported opaque Buffer provenance produces no certificate.

```rust
pub struct SongAppliedCatalog {
    epoch: SnapshotEpoch,
    instruments: Vec<crate::song::snapshot::FrozenInstrument>,
    cells: crate::song::snapshot::FrozenCellInventory,
    resources: crate::song::snapshot::FrozenGraphResources,
}
impl SongAppliedCatalog {
    pub fn epoch(&self) -> SnapshotEpoch;
    pub fn instruments(&self) -> &[crate::song::snapshot::FrozenInstrument];
    pub fn cells(&self) -> &crate::song::snapshot::FrozenCellInventory;
    pub fn resources(&self) -> &crate::song::snapshot::FrozenGraphResources;
}
```

Applied Runtime notice carries original immutable `Rc<SongAppliedCatalog>`;
Session retains it only on actual Applied and exposes readonly
`pub fn applied_song_catalog(&self) -> Option<&SongAppliedCatalog>;`. Protocol
CandidateApplied stays unchanged; no new wire payload/tag. Carried notice is
`CarriedMute { epoch, sound: FrozenSound, frame: u64, muted: bool }`, published
through the existing selector/status body after CandidateApplied. Catalog clone
admission charges complete copied instrument/default/cell/resource metadata;
original graph and PCM Arcs are shared. No evaluator or live resolver is exposed.

Exact catalog refinement approved by root: use public DTO tuple rather than a
named unexported child type. `type AppliedCatalog = (Vec<FrozenInstrument>,
FrozenCellInventory, FrozenGraphResources);` retained as `Rc<AppliedCatalog>`.
Session accessor is `pub fn applied_song_catalog(&self) -> Option<(SnapshotEpoch,
&(Vec<FrozenInstrument>, FrozenCellInventory, FrozenGraphResources))>;`.
Additional before-source controller signatures:

```rust
fn replacement_boundary(clock: SongHostClock, old_activation: u64,
    settings: SongSettings, minimum_safe_frame: u64) -> Result<(u64, Ratio64), Failure>;
fn capture_catalog(ready: &SongReadyBundle, remaining: &mut u32)
    -> Result<Rc<AppliedCatalog>, Failure>;
fn normalize_analyzers(graph: &mut InstDef, instrument: KwId,
    cells: &FrozenCellInventory, remaining: &mut u32) -> Result<bool, Failure>;
```

Retain selected exact integer cycle separately from rounded absolute frame;
old offered horizon is conservative, monotonic and clamped only after accepted
Replace. Retained handoff record includes submitted/committed/invalidation flags,
optional exact returned source/commit for refusal retry, and overlay nonce.

Normalization refinement: certificate retains copied DeclaredParam metadata in
addition to graph/cell alias structure. Normalize static analyzer addresses by
subtracting the authenticated owner minimum, retaining ordered effect/kind/width
and relative overlap/gaps; cell-driven analyzer addresses are uncertifiable for
mute equivalence rather than guessed. No routing/geometry admission changes.
Existing live tempo-change refusal is removed: new local-zero arrangement uses
its own actual settings; boundary is still certified on the old exact clock.

Historical initial compilation used a ceil boundary. Superseded by original
nearest/ties-up conversion below; its initial regression covered ties only.
Private apply tests `fractional_cycle_uses_earliest_rounded_boundary` and
`replacement_boundary_rejects_unrepresentable_frame` exercise actual helper.
Remove accidental loop-only guard from invalidation; pending replacements
remain invalidatable both before submission and after arm via authentic receipts.
This checkpoint has unfinished genuine controller acceptance fixtures.

Browser checkpoint: five initial fixtures passed; three replacement fixtures
failed. Alignment passed. Authentic carried state must explicitly publish an
Outgoing with Dest::Topic(Topic::Telemetry), existing ServerMsg body and
self.envelope(message,None), after Applied; requester mute routing is retained.
Spectral and cancellation fixture outcomes remain under diagnosis.

Before controller fixture source: `Rig::new(bytes:bool)->Self`,
`Rig::tick(&mut self)`, `Controller::new(bytes:bool)->Self`,
`Controller::apply(&mut self,seq:u64,code:String,revision:u64,edit_epoch:u64)`,
`Controller::step(&mut self)->[f32;32]`,
`Controller::wait_applied(&mut self,seq:u64)->SongApplyAck`,
`tone(frequency:u32,duration:&str,reordered:bool)->String`.
Tests `playing_replacement_uses_old_cycle_and_actual_new_audio` and
`acknowledged_alias_mute_is_broadcast_after_real_reordered_apply` run both
actual Native and portable WasmAudioHost/ByteInbox/Engine backends, retain
original Session authority and use callback alloc/dealloc probes. Remaining
controller scenarios stay required after these first coherent witnesses.

Before remaining fixture source: `RecordedHost` borrows original actual host
through Rc<RefCell<Box<dyn AudioHost>>>; logs only successfully admitted PODs
and real drained HostMsg receipts, delegates original upload/materialization.
`Controller::change(revision:u64,base:u64,edit_epoch:u64)`;
`Controller::replacement()->Option<SongReplacement>`;
`Controller::mute(seq:u64,epoch:SnapshotEpoch,muted:bool)`;
`Controller::wait_failed(seq:u64)` and `Controller::wait_replacement()` retain
actual Session publication and exact engine receipt ordering.
Tests `draining_and_natural_end_keep_original_source_until_new_tempo_commit`,
`changed_renamed_and_removed_definitions_do_not_carry_old_mute`,
`posted_document_cancel_recovers_source_and_after_arm_retains_applied_catalog`,
and `pending_mute_and_real_host_pressure_serialize_repeated_apply` use both
backends and original candidates, never fabricated successful acknowledgments.

Additional exact fixture declaration before source: `bank_factory(second:u32)
->Rc<DecodedSongAssetFactory>` returns original two-member ordered PCM Arcs;
`unused_closed_bank_member_change_prevents_mute_carry` changes only unplayed
member one while member zero/graph/name remain identical. Both actual hosts
must adopt original closed bank and compare whole bank, not member-zero-only.
The command-pressure witness is not physical ACK-ring saturation evidence;
that remaining pressure obligation stays explicitly pending.

RecordedHost forwards additive actual `try_song_sample(lease,data)` and
`song_sample_sender_capacity()` unchanged to the original provider; default
unsupported compatibility is never treated as real sample admission.

Actual extended checkpoint94323: six controller tests PASS, pressure fixture
FAILED before Replace because its foreign command owner did not consume real
Rejected(epoch9999) receipts. RecordedHost drain now retains all exact actual
receipts in its ledger and consumes only that explicitly owned foreign rejection;
all Session-owned and other foreign receipts still forward unchanged. Production
unknown-ACK retention remains unchanged. Remove unused fixture clock import.

Before-source timing correction: original `SongLimits::frames_at` is nearest
with ties up, so for R>0 earliest k is ceil(((2R-1)*d)/(2*n)); R=0 gives zero.
Checked u128 quotient/remainder implements this without ceil-add overflow.
Final absolute boundary rounds the whole k*n/d once, retaining exact integer k.
Private `non_half_fractional_boundary_matches_original_frame_contract` checks
16.2 frames/cycle at minima16/17 and bpm123 k3 against actual frames_at, plus
existing ties and overflow regressions. No core frame-conversion changes.

Actual controller whole seven tests PASS16572/0, raw
`/tmp/vactr-controller-public-002.log` SHA40e33d17d8280e0987526e2cd4371adcb4d7ffbb5d29b37326c79622594d3774.
Distinct physical ACK saturation is now source-written under its separate
followup plan; independent execution remains pending. Root authorized only
apply.rs original rounding correction during that fixture wave.

ACK followup first actual execution: seven existing tests PASS, new witness
failed Native public count8191 because an internal report was filtered. Repair
uses nonmutating public capacity queries with exact explicit result ownership;
physical count8192 and original timing correction remain unchanged.

## Bounded reassessment repairs — before-source declarations

- `PendingMute.failed: bool`, initially false. Preserve already admitted family
  receipts and physical alias updates after a permanent refusal. Publish the
  correlated MuteFailed once posted obligations are acknowledged or owner ended;
  never treat Invalid/Unavailable as queue pressure or issue remaining families.
- `Runtime::drive_song_mutes(&mut self)` in existing overlay child extracts the
  old posting loop before parent growth. Backpressure alone retries original POD.
- `Runtime::retire_failed_ready(&mut self,refusal:SongTransportRefusal)` in apply
  consumes same original Ready into SongHostPreparation::retire, retains previous
  old source promise through cleanup, and schedules one Failed notice.
- Tick guard skips realization for unadmitted replacement with no failure. The
  earlier replacement driver always retries priming/Replace; invalidated/failed
  owners still advance authentic cancellation/retirement cleanup.
- Private actual Native adopted Ready fixture
  `permanent_startup_failure_retires_original_ready_once`: exhaust the retained
  controller certificate counter, require one Failed, no activation, exact full
  resource-key returns and preparation record removal using actual callbacks.
- Actual provider saturation after first accepted Prime witness
  `partial_prime_pressure_retries_or_cancels_without_premature_advance`;
  typed permanent-refusal fixture
  `permanent_mute_refusal_resolves_request_and_allows_later_apply` tests both
  Invalid/Unavailable, no fabricated success/ACK, original PCM/catalog intact.

All three are existing state transitions; no new protocol, limits, snapshots,
resource budgets or alternative playback architecture. Criteria remain unchecked
until genuine independent execution including current eight controller tests.

### Reassessment repair checkpoint

Actual original adopted-Ready permanent startup regression plus shared timing
helpers: four private tests PASS41204/0, raw
`/tmp/vactr-controller-transition-private-001.log`
SHA04e5ca898dc3d23aefbe5c1fce2668165b07aad513fe9f1f3292e75f7450b9b4.
Actual sample six prerequisite fixtures PASS97916/0 independently. Controller
integration fixture additions are written and unexecuted: actual provider queue
saturation immediately after accepted Prime, retained retry/cancel/reclaim;
typed permanent mute Invalid/Unavailable failures with no fake ACK/success,
correlated single requester failure, unchanged original PCM/catalog and later
Apply. Existing eight controller tests remain intact. Root serially added
crate-private `owns_song_resources()->bool` for the separately owned isolation
consumers; it checks only actual preparation/Ready/retiring/live owners. No
last-state substitute, clock prediction, altered DSP budgets or protocol tags.
