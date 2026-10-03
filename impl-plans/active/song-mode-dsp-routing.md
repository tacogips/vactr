# Private branches, tails and audio gates implementation plan

**Status**: In Progress
**Plan ID**: SONG-09
**Plan Path**: impl-plans/active/song-mode-dsp-routing.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-02
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Private branches, tails and audio gates supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-09",
  "planPath": "impl-plans/active/song-mode-dsp-routing.md",
  "dependsOn": [
    "SONG-08"
  ],
  "writePaths": [
    "src/dsp/song.rs",
    "src/dsp/engine.rs",
    "src/dsp/engine/song_runtime.rs",
    "src/dsp/bus.rs",
    "src/dsp/bus/song_runtime.rs",
    "src/dsp/voice.rs",
    "tests/song_dsp.rs",
    "tests/song_dsp/lifecycle.rs",
    "impl-plans/active/song-mode-dsp-routing.md"
  ],
  "sharedPaths": [
    "src/dsp/engine.rs",
    "src/dsp/voice.rs",
    "impl-plans/active/song-mode-dsp-routing.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/dsp/engine.rs",
      "intendedEdit": "Dispatch song activation/mute/end commands and acknowledge actual applied frame. Owners execute serially: SONG-08, SONG-09"
    },
    {
      "path": "src/dsp/voice.rs",
      "intendedEdit": "Serial SONG09 private reset/stop API; preserve legacy voice start/render and current note controls/primitive envelope state. Actual callback-safe reset required."
    },
    {
      "path": "impl-plans/active/song-mode-dsp-routing.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-08](song-mode-audio-contracts.md)
- **Next**: [SONG-10](song-mode-host-adapters.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-08 / SONG-08B / SONG-08D | Complete joined routing geometry, phase checks and recorded hashes required for final SONG-09 acceptance | Final join pending |
| Verified frozen POD/carrier/cell/stager/occupancy contracts | Actual runtime implementation inputs; first audible increment independently verified | Next source release required after ROOT0293 doc amendment |

## Execution and preservation contract

This is a future implementation contract. This planning run changes documents only and executes no future tests.
Prerequisite before native Riela implementation/review fanout: accepted design and reviewed plans must be committed in a separately authorized serial execution. This specialized design/plan workflow does not perform that commit or Git finalization.
Use the repository rust-coding agent for Rust changes and invoke check-and-test-after-modify after every Rust modification batch. Use design-doc/impl-plan skills for documentation and progress.
Before each edit, freshly read the exact target and its dependencies; record SHA-256 before/after in an immutable intent record under tmp/song-mode-riela/<planId>/<taskId>.json. The record contains accepted design hash, plan hash, task ID, exact intended change, target paths and baseline hashes. Do not overwrite intent records: create a new numbered record for revisions.
Compare the fresh pre-edit hash with the recorded baseline immediately before writing. If it drifted, stop that edit, reread the current file and reconcile the intended diff; never restore a stale whole-file copy. Preserve all pre-existing staged, tracked and untracked work. Recheck post-edit hashes at join; unexpected drift requires serial repair by the reconciler, not another worker overwrite.
Only edit listed writePaths. An additional compile-required exhaustive match or file split needs a serial manifest/plan amendment before work resumes; no implicit broad cleanup permission. If a touched Rust source reaches 1000 lines, split it under repository standards and record exact new paths before editing. Current inspected principal integration files are below 1000 lines.
Use the same branch and workspace. No worktrees, private branches, concurrent Git operations, dependency additions or lockfile generation are prescribed. Shared indexes, broad formatting, archive moves and cross-worker repair belong exclusively to SONG-16 after joining.
Each worker updates only its own plan's status, completion checkboxes and dated progress log, including actual command, exit code and complete foreground log path. Retain/poll every foreground session to terminal exit; no orphan processes or detached shell work.

## Scope and invariants

Implement only this plan's deliverables. No tempo-map engine, arbitrary bus graph, browser offline export, music-visual framework, extra track controls, dependency upgrades or unrelated source refactoring.
Preserve existing lazy Pattern/stack/cat/repeat semantics, sound-first parser conventions, immutable data, rational musical time, live incremental eval and native/browser capability diagnostics. New Part/Song are explicit finite values; proposed syntax is not a parser grammar change.
All callback work remains allocation-free and VM-free. Song snapshot identity is revision/epoch-scoped; seeds do not depend on host/query partitioning. Errors must not be called successful complete music.
Use existing Ratio64, Pat, Value, Failure, Tempo, KwId and capability types. Declaration blocks below are signatures/type contracts, not implementation bodies; import paths refer to existing crate modules or prerequisite song modules. Do not add a generic abstraction merely to wrap these declarations.

## Modules and file-level deliverables

| File | Intended change | Status |
|---|---|---|
| `src/dsp/song.rs` | Actual private runtime exists; gate/end and exact returned ownership reaping pending. | In Progress |
| `src/dsp/engine.rs` | Verified actual timed activation; family mute/deadline integration pending. | In Progress |
| `tests/song_dsp/lifecycle.rs` | Real Native/Arena family/reset/ramp/end/pressure fixtures sharing the actual parent Rig. | NOT_STARTED |
| `src/dsp/engine/song_runtime.rs` | New cohesive child for actual lifecycle/render/retirement methods. | NOT_STARTED |
| `src/dsp/bus.rs` | Actual private bus audio; fixed Native/Arena reset recipe and child registration pending. | In Progress |
| `src/dsp/bus/song_runtime.rs` | New exact song bus lifecycle, reset and gated accumulation child. | NOT_STARTED |
| `src/dsp/voice.rs` | Private bounded reset/stop preserving note controls and primitive envelope phase. | NOT_STARTED |
| `tests/song_dsp.rs` | Six actual audio fixtures verified; mute/deadline/Apply and wider phase fixtures pending. | In Progress |

Retired future writes remain read-only: dsp/mod.rs, arena.rs, arena/song.rs, engine/render.rs; exact hashes recorded in the ROOT0293 superseding amendment below. Historical declaration sections describe the already implemented sample lifecycle and do not grant renewed write ownership. engine/song.rs remains read-only.

### Public declaration contract

```rust
pub struct SongBranchState { pub id: SongBranchId, pub epoch: SnapshotEpoch, pub instrument: u32, pub tail_end_frame: u64 }
impl Engine {
    pub fn activate_song(&mut self, command: SongActivation) -> Result<(), SongRejectCode>;
    pub fn apply_song_mute(&mut self, command: SongMute) -> Result<(), SongRejectCode>;
}
```

## ROOT0266 exact lifecycle integration declarations

The eight Rust manifest trades engine/song.rs to read-only staged protocol and
owns arena/song.rs. No existing phase criterion below is removed. This amendment
is document-only; Rust awaits prerequisite acceptance and a separate release.

### Private runtime: src/dsp/song.rs

```rust
pub(crate) struct SongRuntime;
pub(crate) struct SongVoiceOwner {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) branch: SongBranchId,
    pub(crate) generation: u32,
    pub(crate) instrument: SongLeaseKey,
}
pub(crate) struct SongTimedCommand;
pub(crate) struct SongPrivateCells<'a>;
impl CellRead for SongPrivateCells<'_> {
    fn get(&self, cell: CellId) -> f32;
}
impl SongRuntime {
    pub(crate) fn new(preparations: usize, branches: usize, leases: usize,
        voices: usize, commands: usize, receipts: usize) -> Result<Self, SongRejectCode>;
    pub(crate) fn owns_epoch(&self, epoch: SnapshotEpoch) -> bool;
    pub(crate) fn owns_lease(&self, key: SongLeaseKey) -> bool;
}
impl SongResourceStager {
    pub(crate) fn bind_graph(&mut self, binding: SongGraphBanks)
        -> Result<(), SongRejectCode>;
    pub(crate) fn bound_cell(&self, graph: SongLeaseKey, logical: CellId)
        -> Result<CellId, SongRejectCode>;
    pub(crate) fn analysis_bank(&self, bank: SongLeaseKey)
        -> Result<SongFrameRegion, SongRejectCode>;
    pub(crate) fn finish_return(&mut self, key: SongLeaseKey);
}
```

Preallocate runtime branch, command, receipt and voice ownership storage before
processing using measured configured pools. This is not a fresh independent
quota: concurrent Preparing/Ready/active/retiring occupancy must debit the same
actual tables/regions. Do not admit a staging Ready that later cannot fit runtime
ownership. Queued/deferred events carry SongVoiceOwner plus exact frame; no note
key, raw InstId or legacy orbit becomes authority. The existing stager binding,
cell lookup, analysis-range and cleanup implementation may relocate cohesively
into this owned module before arena/song grows; preserve callable interfaces and
bounded ownership checks. `finish_return` must remove only declarations depending
on the returned exact lease, rather than all unrelated branches of its epoch.

### Owned engine.rs / engine/render.rs

```rust
impl Engine {
    pub(super) fn queue_song_runtime(&mut self, command: SongCommand)
        -> Result<(), SongRejectCode>;
    pub(super) fn apply_song_runtime_frame(&mut self, frame: u64,
        acks: &mut AckProducer) -> Result<(), SongRejectCode>;
    pub(super) fn next_song_runtime_frame(&self, end: u64) -> Option<u64>;
    pub(super) fn render_song_branches<C: CellRead + ?Sized>(&mut self,
        legacy: &C, frames: usize) -> Result<(), SongRejectCode>;
    pub(super) fn retire_song_runtime(&mut self, frame: u64,
        acks: &mut AckProducer, garbage: Option<&mut Producer<Garbage>>);
}
```

Record dispatch intercepts Activate/Mute/Endpoints/Release/Event and protected
CancelPreparation/CancelLease before delegating ordinary staging records to the
read-only `song_record` (pub(super)). Generic processing splits at exact u64
command/event frames, applies transitions before matching onsets, and retains
actual-frame receipts under ACK pressure. Scheduling success is not Applied.
These signatures supplement existing activate_song/apply_song_mute methods; they
do not weaken frame acknowledgment semantics or existing generic rejection paths.

`engine/render` constructs per-voice/per-bus contexts after destructuring disjoint
Engine fields. Song cells use the exact bound physical control view; legacy uses
only legacy CellRead. Analysis IDs remain logical bank-local values, with exact
owner-local mutable slices selected from retained graph bindings. No AtomicCells
fallback or pooled physical-index encoding. Native/Arena lookup selects exact
leased template/generation and private destinations, never Templates.live(InstId).

Build complete pre-track-effect sums, process private branch FX, distinct track
sum stages and song master in certified causal order, with strict same-epoch
detectors. Song room/delay has its own admitted memory, never shared OrbitDelay.
Only final song master output joins legacy mix. Hard mute uses postmix gain to
zero within64 frames including tails, clears state before unmute, suppresses
matching queued/deferred onsets, and unmute admits future onsets only.

### Verified exclusive sample view: arena.rs / arena/song.rs (future read-only)

```rust
pub(crate) struct SongReadScope<'a>;
impl SampleStore {
    pub(crate) fn song_read_scope(&mut self, epoch: Option<SnapshotEpoch>)
        -> SongReadScope<'_>;
    pub(crate) fn get_selected(&self, id: u32) -> Option<SampleView<'_>>;
    pub(crate) fn activate_song_sample(&mut self, key: SongLeaseKey)
        -> Result<(), SongRejectCode>;
    pub(crate) fn retire_song_sample(&mut self, key: SongLeaseKey)
        -> Result<(), SongRejectCode>;
}
impl std::ops::Deref for SongReadScope<'_> {
    type Target = SampleStore;
    fn deref(&self) -> &SampleStore;
}
impl Drop for SongReadScope<'_> {
    fn drop(&mut self);
}
```

Private plain Option selector initializes None in every constructor. Guard holds
exclusive &mut SampleStore and previous selector; Deref only, no DerefMut/public
setter; Drop restores exactly. Concurrent/nested peer mutation cannot coexist
with borrowed contexts. AutoSend+Sync is preserved, unlike interior Cell.
Move get implementation into owned arena/song and keep public get facade. None
admits only legacy; Some epoch admits only exact epoch physical resources in
Live/Retiring. Staged never reads under any selector. Full resource key/generation
validation and certified mapped IDs precede selecting a read scope. Existing
voice/UGen/FxCtx concrete &SampleStore APIs remain unchanged.

### Active preparation and exact cleanup

```rust
// Add default-false field to existing pub(crate) Preparation.
active: bool,
impl SongResourceStager {
    pub(crate) fn close_preparation_for_activation(&mut self,
        epoch: SnapshotEpoch) -> Result<(), SongRejectCode>;
}
impl Templates {
    pub(crate) fn activate_song_template(&mut self, key: SongLeaseKey)
        -> Result<usize, SongRejectCode>;
    pub(crate) fn close_song_template_for_return(&mut self, key: SongLeaseKey)
        -> Result<(), SongRejectCode>;
}
impl BusGraph {
    pub(crate) fn activate_song_bus(&mut self, key: SongLeaseKey)
        -> Result<usize, SongRejectCode>;
    pub(crate) fn close_song_bus_for_return(&mut self, key: SongLeaseKey)
        -> Result<(), SongRejectCode>;
}
```

Activation validates complete Ready/noncancelling resources/geometry atomically
before state changes. Retain Preparation with active=true, rather than removing
it and enabling premature epoch reuse. Owned preparation_mut rejects active:
its existing callers are seal and public cancel_song_preparation, so the latter
cannot mark live leases retiring even if invoked directly. Existing open rejects
Ready; no further upload/branch mutation may alter active resources. Runtime
record handling rejects active CancelLease before the staging delegate.

Generic legacy lookup, replacement, retire and collect exclude every song owner.
At branch end retain exact resources until voice references and tail deadline
permit handoff. Only then close exact template/bus state to Staged for the
unchanged staging return pump to find and return owners. Samples/banks remain
leased until graph references end. `pump_song_staging` is pub(super), so parent
can reuse its bounded critical receipt/garbage retry. It must not see a live lease
marked retiring before runtime safety. Branch-preserving cleanup and shared
preparation capacity must survive partial return and ACK/garbage blockage.

### File budgets and mandatory evidence

| Owned file | Current lines | Cohesive strategy / target |
|---|---:|---|
| dsp/song.rs | absent | Private runtime POD/storage and relocated bank helpers; <900 |
| dsp/mod.rs | 31 | Registration only |
| engine.rs | 909 | Runtime ownership + interception; move block/start bodies to render before growth; <950 |
| engine/render.rs | 173 | Private render/timing/runtime Engine helpers; <950 |
| bus.rs | 886 | Private activation/render/collector isolation; relocate cohesive logic to already-owned dsp/song if required; <990 |
| arena.rs | 953 | Plain selector, delegating get, template lifecycle protections; <990 |
| arena/song.rs | 908 | Move cohesive bank/cleanup helpers before guard/lifecycle additions; <990 |
| tests/song_dsp.rs | absent | Actual Native/Arena audio/isolation/frame/pressure fixtures; <990 |

Prior explicit amendment is required before any further source split. Proof must
include Native/Arena sample, wavetable/granular and convolution exact epoch reads,
private control/analysis values conflicting with legacy, active-cancel protection,
partial branch retirement preserving siblings, no callback allocation/drop, actual
64-frame postmix mute, partition invariance and full queue pressure. Acceptance
remains all original criteria below; ordinary source composition and full08D
geometry remain required downstream integration, not waived by this lifecycle.

## Tasks

### TASK-001: Baseline and contract integration
**Status**: In Progress
**Parallelizable**: No; acquire dependencies and fresh hashes first.
**Deliverables**: Manifest, immutable intent snapshot and the declaration/type integration listed above.
- [ ] Read accepted section and prerequisites; record exact ownership and imports.
- [ ] Add declarations without changing legacy semantics; review manifest-required exhaustive consumers.

### TASK-002: Implement the owned behavior
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this plan; cross-plan parallelism follows the DAG and ownership manifest.
**Deliverables**: Every non-test file in the module table, with exactly its stated intended change.
- [ ] Implement the declared behavior and all phase-specific criteria below.
- [ ] Record post-edit hashes and run required modify-agent checks.

### TASK-003: Behavioral evidence and progress
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No; verifies the complete phase.
**Deliverables**: Every listed test/fixture file, full command logs and this plan progress record.
- [ ] Add the specified success, boundary, compatibility and failure fixtures.
- [ ] Run future commands below; record actual exit status and complete output, with no empty selected-test run accepted.
- [ ] Reconcile final hashes with intent; update completion criteria and progress without editing other worker logs.

## Phase acceptance criteria

- [ ] Only bd enters bd effect state; sd/hh branch signal before shared master unchanged by bd-private configuration.
- [ ] Room and delay belong to selected branch; shared master nonlinearity explicitly outside isolation promise.
- [ ] Mute gate reaches zero within 64 frames, suppresses queued matching voices, clears private state before unmute; unmute only admits future onsets.
- [ ] Changed configuration drains old branch privately; unchanged adjacent configuration may retain state.
- [ ] No allocation/VM work in callback; reject resources before old active state is retired.
- [ ] Pending, deferred and active song voices retain epoch/branch/generation ownership; exact template and private destination lookup cannot select a peer snapshot.
- [ ] Timed activation/mute/end takes effect before matching onsets are dequeued at that frame; actual-frame receipts survive acknowledgment backpressure.
- [ ] Branch detector dependencies have an explicit causal order; cycles reject before activation. Track detectors read complete pre-track-effect branch sums, independent of slot and callback partition order.
- [ ] Staged/retiring graph, PCM and cell leases survive generic collectors until their exact deadlines; retirement evidence is retained and delivered before identifiers can be reused.
- [ ] Private postmix gates reach zero within 64 frames, including effect/delay tails; matching pending/deferred onsets are suppressed and private state is cleared before future unmuted onsets.
- [ ] Actual individual bus regions, contiguous sample extents and fixed per-voice regions satisfy admission, beyond aggregate capacity totals.
- [ ] Installed graph/sample/cell references match reserved resource kind, epoch and generation; sample BANK controls and embedded cell references are remapped and validated before Ready.
- [ ] Native install transport with no epoch is associated through an unambiguous reserved lease table; stale, unreserved or wrong-kind records cannot become Ready. Full protocol identities are preserved across any bounded native-ID remap.
- [ ] Failed staging and retirement retain native boxes/Arcs when the garbage queue is full; bounded retry owns that memory and prevents capacity/ID reuse until safe handoff.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_dsp)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

## Completion criteria

- [ ] All module-table changes and phase-specific criteria complete.
- [ ] Tests listed above execute with nonzero fixture count and pass; check/typecheck/build gates pass.
- [ ] All required command exit statuses and complete log paths recorded; no running foreground sessions remain.
- [ ] Legacy behavior preserved; pre-existing changes retained and cross-worker hashes reconciled.
- [ ] Progress status updated; archive/index changes deferred to SONG-16.

## Progress Log

### Session: 2026-09-30 — plan authoring
**Tasks Completed**: Planning only; no implementation task completed.
**Tasks In Progress**: None.
**Blockers**: None for planning; implementation awaits reviewed/committed documents and prerequisite waves.
**Verification**: Read-only source/document consistency review only. Future commands above were not executed.
**Next session**: Implement TASK-001 after dependency outputs and authorization are available.

### Session: 2026-10-01 — private routing and timed command preflight
Existing BusGraph::route falls back to master, collect retires buses at users=0,
and render::mix_voice sends delay into a shared orbit. Song paths must use checked
route lookup, branch-private room/delay and a branch-to-track-to-master stage.
Retiring branch effects retain capacity until their explicit bounded tail deadline,
independently of voice-user count. Existing shared orbit routing alone is not
private effect isolation.
Engine command intake occurs before blocks. Schedule song activation/mute/end
against exact engine.frame boundaries, splitting render work as required, and emit
Applied/Muted only after the timed action actually takes effect. Preserve critical
acknowledgments with bounded pending state/backpressure rather than ignoring a
failed push. Use existing owned engine/song/render/bus/arena paths for branch voice
mapping; voice.rs and dsp/caps.rs are not implicitly authorized. Current wire/ring/
engine/arena modules are near 800 lines; request a bounded plan/manifest split
before additional files or any touched module reaches 1000 lines.

### Session: 2026-10-01 — engine-owned callback commands
Activation and mute operate on an explicit mutable Engine instance. The earlier
free-function declarations provided no owner and would require hidden global
state. Return copied SongRejectCode values on the callback path; constructing
VM Failure messages there would violate allocation-free audio processing.
Scheduling acceptance does not acknowledge Applied/Muted before the actual
effective frame. Use the typed SONG-08 snapshot/branch/generation event envelope
for pending, deferred and active voice mapping; preserve legacy AudioEvent and
its cut/orbit hint semantics. NativeInstall is not owned by this phase: use the
existing encoded prepared-graph transport and owned arena/engine integration,
or obtain an exact prior manifest amendment for a genuinely required consumer.
Keep timed command queues and helper state within owned dsp/song.rs and render
integration so existing engine/arena files remain below 1000 lines. ROOT0036
records a source-grounded correction, not additional Riela acceptance.

### Session: 2026-10-01 — silent staged graph preparation
Legacy Templates::build/adopt and BusGraph::install activate immediately and
retire matching live resources. Candidate preparation must reserve generation-
qualified resources and install them into staged ownership without replacing
active graphs/master or rendering candidate buses. This includes self-noise and
room/delay state, not only absence of voices. SONG-08 supplies POD preparation
begin/resource-reserve/branch-configure/seal commands with exact epoch/resource/
generation/kind identity and bounded acknowledgment/backpressure semantics.
Ready requires complete actual validated staged resources; rejection retains
old active state. Native preparation may use reservation-aware existing native
adoption; browser byte builds need preallocated storage. NativeArc currently
has no empty template boxes for generic byte builds, so never allocate a decoder
template on the callback or silently treat legacy activation as staging.
All branch/track/master slots, private delay memory and overlapping generation
leases consume reported host capacities. Retain graph/sample/cell leases until
finite deadlines regardless of voice users. SONG-07A supplemental source policy
inventory precedes SONG-08 route certification. ROOT0037 records this bounded
source-grounded handoff, not a new Riela acceptance.

### Session: 2026-10-01 — certified detector bindings within restricted routing
Accepted Routing and tails requires validation of selected sidechain selectors.
Actual BusSlot detector snapshots read pre-effect linked-channel inputs across
matching live/retiring BusId generations; they do not redirect summed audio.
SONG-08B therefore certifies copied template/unit -> declared root track/logical
track-stage detector bindings rather than blanket-rejecting all sidechains.
Unknown, non-track, malformed or prohibited self selectors fail before activation.

Remap template bus IDs and every sidechain control to the actual privately
installed snapshot track-stage identities. Old, staged and new snapshots must
never read a peer detector merely because candidate numeric BusIds collide.
Keep branch -> track -> master sums unchanged; retain the actual pre-effect
detector semantics in allocation-free render ordering. Reserve detector snapshot
workspace explicitly and use actual generation leases through tail retirement.
Prove valid kick-keyed private/track compressor behavior, unresolved-selector
rejection and old/new epoch detector isolation on both Native and Arena paths.
Do not substitute silent zero input or master fallback for a resolved binding.
Any genuine unsupported causal/self case receives an explicit diagnostic.
No extra Rust path is authorized. ROOT0046 records this source-grounded handoff,
not a new Riela acceptance or arbitrary audio routing framework.

### Session: 2026-10-01 — actual DSP ownership and capacity preflight

Independent read-only review confirms Engine::record currently rejects Song commands
(engine.rs411), while block694 starts legacy deferred/pending events before rendering.
Legacy start605 selects by InstId; deferred storage loses song epoch/branch identity.
The owned song.rs must retain bounded pending/deferred/voice ownership and exact-frame
actions. Existing millisecond Voice::short_gate is not proof of a 64-frame song gate.

Templates::build/adopt (arena.rs750/775) and BusGraph::install (bus.rs410) activate
immediately. Staging must preserve active resources and exclude candidate self-noise.
NativeArc template slots have no empty decoder boxes; use actual prepared native
ownership or startup preallocation, with failed adoption retaining the original box.

BusGraph::render snapshots raw slot inputs before running branches and sums all slots
to master. Complete pre-track detector inputs require certified branch dependency
ordering; mutually dependent private branches require explicit causal rejection.
Track-stage effects can use assembled pre-track snapshots. Silence, stale snapshots
and numeric slot order cannot stand in for this contract.

Generic template/bus/sample/cell collection must respect song leases and retain each
retirement receipt under pressure before ID reuse. A single rejection slot is not
proof that a burst of resource completions is retained. Native boxes/Arcs retire
through off-thread garbage ownership. Aggregate bus_frames/pcm_bytes admission does
not prove fixed individual bus regions or contiguous sample extents fit. Actual
resource association and sample/cell remapping must be validated before Ready.

Future behavioral evidence must exercise real Native and Arena preparation, failed
staging preservation, detector ordering across callback partitions, queue pressure
during multi-resource retirement, private mute including queued events/tails, exact
activation boundaries and one-short individual memory/extent failures. No Rust edit
or Cargo command was performed by this preflight. ROOT0059/0060 record source hashes
and the documentation refinement. The historical full-SONG-08 implementation hold is superseded by the explicit
audible implementation ordering entry below; complete SONG-08 independent
acceptance remains required for final SONG-09 acceptance. The eight-source
manifest and full goal stay intact.

### Session: 2026-10-01 — native install association and off-thread ownership

NativeInstall (ring.rs256) carries resource/gen and native ownership, but no song
epoch. NativeAudioHost currently supplies fresh_graph/gen1 (audio.rs713–750), with
wrapping graph IDs. Song installs require an authoritative reservation association
with expected kind, exact lease and epoch before staging. Private native transport
IDs/generations may be remapped within bounded tables; never truncate public full
identities, accept ambiguous delayed records or silently reuse wrapping legacy IDs.
Changing NativeInstall itself is outside this manifest and requires prior amendment.

Legacy push_garbage (ring.rs822) leaks boxes/Arcs when the queue is full or absent.
Song-owned rejection/retirement cannot use that leak as proof that reserved memory
is free. Retain bounded native ownership in owned lifecycle state, retry off-thread
handoff and retain completion receipts before capacity/ID reuse. Existing ring APIs
allow checked push; no ring.rs write is implicitly authorized. Prove full garbage
and acknowledgment queues together, failed adoption preservation and no callback
allocation. ROOT0071/0072 record this read-only-derived documentation refinement.


### Session: 2026-10-01 — distinguish implementation prerequisites from final acceptance

Source review confirms an acceptance dependency cycle: SONG-08B requires actual
per-region capacity and staged/retiring lease evidence, while the engine currently
rejects song commands as NotReady and its providers were scheduled after full
SONG-08 acceptance. Aggregate capacity fixtures cannot resolve that cycle.

The earlier blanket statement that all SONG-09 implementation must wait for full
SONG-08 acceptance is superseded for a separately reviewed SONG-09P resource-staging
prerequisite. That prerequisite may consume verified POD preparation contracts,
held route-preparation interfaces and the verified SONG-08C frozen-cell inventory.
Its exact declarations, dependencies and bounded file manifest must be accepted
before Rust changes. It must implement real capacity reporting and silent resource
adoption, preserving active playback and retaining full epoch/generation ownership.
An acknowledgment-only substitute does not satisfy this prerequisite.

SONG-09P supplies actual admission and lease evidence back to SONG-08B. Full
SONG-08 and SONG-09 acceptance still requires their complete joined criteria;
private audio, activation, mute, detector ordering, tails and host integration
remain required. The manifest dependsOn entry describes final acceptance, not a
prohibition on this separately owned prerequisite. This amendment releases no Rust
paths and does not mark any implementation task complete. ROOT0186 records intent.


### Session: 2026-10-02 — consume actual staging ownership in private audio

ROOT0218 records a future manifest amendment before source edits. SONG-09P owns
real staging in engine/song.rs; this phase must explicitly own that module after
09P holds and clears so activation, private-bank selection and retirement use
actual resources. The eight Rust paths are preserved by replacing the former
song/routing.rs write with engine/song.rs. Existing routing POD declarations and
validity rules are read-only inputs; arrangement transition/overlap validation
against actual capacities remains required in owned engine/stager integration
and the joined08B proof. No validation criterion is removed.

Private rendering must bind each song voice/bus to its epoch-owned control and
owner-local analysis banks, with logical holes and ordered overlapping writers
preserved. Stager metadata alone is insufficient: tests must render real private
references while different legacy/global values exist and prove isolation.
Silent staged resources become audible only on checked activation, remain leased
through tail deadlines, and cannot be reused before final receipt/garbage handoff.
This amendment releases no Rust and does not mark any task complete. Source
ownership is serial09P→09; independent routing geometry work remains separate.

### Session: 2026-10-02 — ROOT0266 bounded lifecycle manifest amendment

Replaced future engine/song write with arena/song ownership. Existing staging
protocol becomes read-only; owned Engine dispatch/render helpers, exact pool
state and owner-local sample/bank interfaces provide the actual audio handoff.
Declared active Preparation protection to avoid epoch reuse and unsafe public
cancel, exclusive guard preserving SampleStore Send+Sync, branch-preserving return
and shared staging/runtime capacity. All original acceptance checkboxes retained.
Current source budgets reflect occupancy checkpoint; no Rust/Cargo execution or
phase completion is authorized by this document change.


### Session: 2026-10-02 — actual audible runtime implementation ordering

This entry supersedes earlier blanket source-hold statements requiring complete
SONG-08 acceptance before any SONG-09 runtime implementation. After ROOT0278
terminal success confirms the held staging/occupancy/pressure checkpoint, and
root issues the separate exact source release, this plan may implement actual
audible runtime behavior using frozen verified POD, carrier, cell inventory,
stager and occupancy contracts. Source remains held until that release.

The manifest dependsOn SONG-08 remains unchanged as a final acceptance dependency.
Full SONG-09 acceptance and complete host integration still require complete
SONG-08B/SONG-08D geometry and the parent SONG-08 independent join, including
actual canonical query to resolved route to timed audio integration. No original
criterion, manifest path or required playback behavior is removed. Verified
low-level fixtures cannot stand in for that final integration.

The first coherent audible increment spans the existing eight owned paths:
preallocate shared-quota SongRuntime ownership; validate Ready resources and
GraphBanks bindings before atomic exact-frame activation; retain active
Preparation and protect active cancellation; start exact leased song voices;
render private scalar/owner-local analysis banks and epoch-selected samples
through branch FX, distinct track stages and song master. Only the final song
master joins legacy output. Generic legacy lookup/render/collect must exclude
song owners before resources become Live. Branch-preserving exact lease cleanup
accompanies activation so returning one lease cannot erase unrelated branches.

Native and Arena fixtures must produce actual PCM/oscillator output, preserve
concurrent legacy playback with conflicting cells/analyzers, distinguish two
private branches, and acknowledge the actual activation frame under pressure.
The remaining required 64-frame mute, detector order, tails, partition invariance,
retirement/garbage pressure and complete routing composition remain mandatory
under the unchanged completion criteria.

Use the declared cohesive relocation strategy before growth: engine.rs current909
lines to below950 by moving block/start helpers into owned render.rs current173
(target below950); arena.rs current953 to below990 by delegating sample get;
arena/song.rs current908 to below990 by moving bank/cleanup helpers into new
dsp/song.rs (below900); bus.rs current886 below990; mod.rs current31 registration
only; new song_dsp.rs below990. Every touched Rust file stays below1000, and any
additional split requires a prior exact manifest amendment. This document-only
amendment runs no Cargo and changes no Rust or completion checkbox.

### ROOT0280 actual source implementation started

First audible increment released after ROOT0279 actual staging/occupancy/pressure
acceptance. Numbered0005 captures fresh exact eight source baselines and new-path
absences. Implementation proceeds with the declared private runtime, exclusive
sample scope, actual activation/rendering and protected lifecycle. All Cargo
verification remains checker-coordinated after a held source checkpoint.

### First increment private fixture probe declaration

```rust
#[cfg(test)]
impl Engine {
    pub(crate) fn song_analysis_test_value(&self, bank: SongLeaseKey, logical: usize)
        -> Result<f32, SongRejectCode>;
}
```

Owned engine.rs supplies only a test-build immutable actual owner-bank read;
owned dsp/song.rs exercises a real initialized private analyzer graph and timed
voice. Integration song_dsp supplies real Native/Arena output, timing, ACKpressure,
conflicting legacy cells/analyzers and callback allocation/drop probes. This
adds no public production accessor and does not establish pending full09 detector
ordering, snapshot/event seed, mute64 or full retirement/geometry integration.

### First actual audible checkpoint — held for mandatory checker

Numbered0006/0007/0008 intents precede actual owned source changes. Implemented
exclusive epoch sample scope; retained active preparation and precise NotReady
cancellation; legacy lookup/render/collector exclusion; exact physical activation;
bounded timed POD command/receipt ownership with original deferred failure cause;
private scalar/owner-local analyzer contexts; private branch→track→song-master
mixing and exact inside-quantum onset splits. Successful events proceed without
new receipts under Applied receipt pressure; failed outcomes remain POD-retained
without retrying mutated state or replacing their original rejection cause.

Six unexecuted public song_dsp fixtures cover actual Native/Arena PCM+oscillator,
private BANK Cell translation, independent private destinations, precise active
cancel rejection, current-frame activation+inside-quantum onset, same-frame/later
onset partition comparison, actual full-ring Applied FIFO, original Capacity
rejection retention, and conflicting concurrent legacy cells/analyzers/SlotControl.
Every fixture probes the actual callback for allocation/destruction. One private
dsp::song unit fixture checks a real positive owner-local analyzer write, not
only absence of legacy contamination. These are source obligations pending
checker execution, not passing-count evidence.

All eight source files are held after exact scoped rustfmt (exit0); author ran
no Cargo. Read-only engine/song staging protocol stays unchanged. Remaining
full09 criteria stay unchecked: private admitted delay (nonzero sends currently
return NotReady rather than use shared orbit), mute64/reset/unmute, complete
causal pre-track detector stages, stable event seed, complete retiring epoch
cleanup/shared quotas and full08B/08D query/route/audio integration. Current
Release/Endpoints cleanup is provisional until genuine full-tail/ownership tests.
The implementation does not mark full song playback complete.


## ROOT0293 superseding future manifest and next lifecycle contract

This entry supersedes older write tables/budget estimates and historical sample ownership declarations. Status remains In Progress. No original full phase criterion is removed or considered fulfilled merely by this first increment. The eight future Rust write paths are exactly those in the current manifest. Source work requires separate root release.

Retired read-only baselines:

- `src/dsp/mod.rs` SHA256 `8fd17267751cc6865e4413eaf1ef6539a671a4ef3443351cfbef0ddd1b6e1fa6`.
- `src/dsp/arena.rs` SHA256 `0531f91a6984c60fcbfa9b0fb4167ea6595a70e617e2ac51130f7ab13f263de4`.
- `src/dsp/arena/song.rs` SHA256 `ae26d63841ed089bf3e53ec84431c090d229c68fe6f6e1ed10e6e9f71c45e973`.
- `src/dsp/engine/song.rs` SHA256 `6b9cdbf4f6712fb857c6546ac8ebec007db55fa2c644f15861affb738be20324`.

### Verified first increment — phase scoped

ROOT0291-audible-cycle-matrix-acceptance.json independently audited original77010 terminal0/poll3325c2, all42 gates exit0,889 executions and863 distinct raw test identities; final-results SHA256510fb81384c50a61d2c7b732554460171ffafc7f68521c923d6481ae6c98308c. Evidence directory /tmp/vactr-song09-audible-cycle-independent-005 contains exact inventories, commands, complete logs and log hashes (root receipt remains authoritative for actual directory references). Actual counts include fullDSP647, newaudio6, session78, staging22, capacity/query/wire/carrier/route regressions; counts overlap across filtered library gates and must not be summed as distinct.

ROOT0292 supplemental actual nextest acceptance: original87042 terminal0/poll70d0b3,43 actual PASS identities (song_dsp6 plus song_source_routes37), final-results SHA2565eb929f77456ca88a9684e45085e562fe97f61f265740081cb405d0d3f94c5ac. These43 repeat prior matrix identities, not new distinct coverage. The full53 held input hashes were independently reconciled. All retry failures and parser diagnostics remain retained. This proves the first audible increment, not complete family mute/tails/detectors/delay/seed/Apply/shared quota or full08 join.

### Exact reviewed additive next increment

### Typed declarations (no bodies)

```rust
pub(crate) struct SongGainGate {
    pub(crate) from: f32,
    pub(crate) to: f32,
    pub(crate) start: u64,
    pub(crate) end: u64,
}
impl SongGainGate {
    pub(crate) fn gain(&self, frame: u64) -> f32;
    pub(crate) fn retarget(&mut self, frame: u64, target: f32) -> Result<(), SongRejectCode>;
}
pub(crate) struct SongEpochEnd {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) arrangement: u64,
    pub(crate) deadline: u64,
}
// RuntimeBranch gains a SongGainGate and optional actual release frame.
// SongRuntime gains a constructor-admitted endpoint table, bounded by preparations.
impl Engine {
    pub(super) fn mute_song_family(&mut self, command: SongMute, frame: u64)
        -> Result<SongMute, SongRejectCode>;
    pub(super) fn apply_song_end(&mut self, command: SongEndpoints)
        -> Result<(), SongRejectCode>;
    pub(super) fn finish_song_deadlines(&mut self, frame: u64);
    pub(super) fn reap_returned_song_epochs(&mut self);
}
impl BusGraph {
    pub(crate) fn reset_song_bus<C: CellRead + ?Sized>(&mut self,
        key: SongLeaseKey, cells: &C, sr: f32, caps: &CapabilitySet)
        -> Result<(), SongRejectCode>;
    pub(crate) fn add_song_output_gated(&mut self, from: usize, to: usize,
        frames: usize, first_frame: u64, gate: SongGainGate);
}
impl Voice {
    pub(crate) fn reset_song_voice_state<C: CellRead + ?Sized>(&mut self,
        template: &Template, cells: &C, sr: f32, caps: &CapabilitySet);
    pub(crate) fn stop_song_voice(&mut self);
}
impl PostFx {
    fn reset_song_history(&mut self);
}
// BusSlot gains song_definition: Option<BusTemplate>, fixed POD for both tiers.
```

All new storage is allocated with configured runtime/bus tables before processing. No callback Vec growth, Box replacement, Arc drop, VM execution, or sample cloning. Declaration changes require a new prior intent before implementation.

### Exact mute semantics and reset/ramp conflict

Match `SongMute.instrument` to `SongBranchConfig.family` for every matching track and retained generation in the addressed active epoch. Authenticate before any mutation; absent epoch is StaleEpoch. Produce Muted carrying the actual application frame, using the existing bounded receipt mechanism. Receipt-pressure retries cannot partially reset or replay the operation.

At application frame f: block matching future onsets, remove matching queued Event work while preserving unrelated transitions/events, reset each distinct matching private branch bus using the retained fixed recipe and exact bank views. Shared track/master and sibling private buses are untouched. Already rendered frames before f are immutable because commands split blocks at f.

Set post-effect gate from its current gain to zero over [f,f+64); gain is exactly zero at f+64. A repeated mute/unmute retargets from the actual current gain, not a stale endpoint. Unmute resets private branch state again and admits only new future onsets; skipped notes are never replayed. Gate opens over 64 frames.

Immediate FX reset deliberately discards old delay/reverb history. The fixed64 gate acts on actual post-reset audio, preserving frames already rendered before the command. No lookahead, replay buffer or synthetic held sample is permitted. A sustained SinOsc note must continue producing nonzero post-reset audio during the closing ramp, not merely provide a gate-only silent witness.

At mute, call reset_song_voice_state on the exact matching active voices without invoking Voice::start: retain pvals/pcell, amp/pan/BANK/resource mapping, seed/start/delay, gate_left/open, release/ienv/fade, bus/owner/template identity, tag and routing controls. Reset non-envelope NodeState and admitted per-node history; configure each Effect using the existing fixed template parameters, exact private CellRead and memory region. Reset PostFx.bq to identity while retaining lpf/hpf/res/crush/shape/vowel settings. This is bounded by the same fixed nodes/effects and allocated memory as Voice::start; no owned buffer is replaced or dropped. Preserve primitive EnvPerc/EnvAdsr NodeState so current attack/release position does not retrigger. Their env.rs kernels use scalar state and no external memory. Preserve the voice-level implicit envelope fields rather than reinitializing them to1.

**Precise composite limit:** Node::is_env also includes many integrated synth/drum nodes, not only envelope-only kernels. Their NodeState and memory mix oscillators/resonators and envelopes; blanket preservation would retain the stale DSP history being reset. Generic exact preservation of their embedded envelope while resetting every other state is not derivable through current APIs. Reset those composite node states/regions together, retaining note controls and voice gate/lifetime; explicitly do not claim their internal envelope phase is preserved. They still stop at the closing64 boundary. If exact envelope-phase preservation for every integrated kernel is required, node-specific UGen APIs and a separately bounded manifest are needed. Current accepted design requires private state reset and no stale replay, not universal envelope phase preservation.

The ongoing oscillator witness is feasible: non-envelope SinOsc phase resets, its control and implicit envelope remain, and actual output is multiplied by the post-effect gate. The comparison rig receives the same reset at f but no closing gate; for samples away from oscillator zeros, muted output equals reset-control output times the exact gain. Use actual PCM and per-frame ratios across callback partitions, not only a gate probe.

At the closing gate boundary f+64, stop_song_voice makes the exact voice inactive and clears private history; Engine clears its parallel owner and decrements that actual bus users once. Do not call short_gate (3ms) to implement the64 boundary. At rapid unmute before f+64, first stop all voices tagged as pre-mute closing voices, clear owners/users once, reset matching private buses, then retarget/open the gate and accept only future events. Add a per-voice pre-mute closing marker in the existing fixed owner table (or a closing marker attached to exact owner) so newly started voices cannot be stopped by an old mute completion. Never resume a previously muted note.

### Finite branch and epoch tails

Release marks the addressed generation ended and rejects its future onsets immediately. Keep its effects/private memory until the declared deadline even if voice users become zero. Nonzero available tail gets a final gain fade over [max(release_frame,deadline-64),deadline); zero tail truncates at the deadline.

Endpoints at arrangement frame a stop all epoch onsets and release held voices. Record exact a and deadline d; no inferred-silence termination. For the epoch final output, fade after its private song master so shared track/master effect tails cannot evade the cap. Use [max(a,d-64),d) and exactly zero at d. One epoch's fade never applies to another or legacy output.

At d, stop remaining matching voices, decrement actual bus users once, clear exact owner slots, reset corresponding private state, and mark graph/resource leases retiring. Do not wait indefinitely for envelopes. A Release may retire only resources with no other live/draining referencing branch; shared track/master and sample/bank keys remain until all references are closed. Retiring generations remain counted until owner handoff.

### Exact ownership reaping

Keep the existing control-intake guard as the sole staging pump. Runtime deadline work marks eligible leases only. Native template/PCM boxes/arcs stay retained in existing pending owner storage when garbage is absent/full; no callback destructor. Arena physical resources stay owned while the exact critical LeaseReturned remains pending.

`finish_song_return` removes stager lease only after guarded return/critical receipt progress. Runtime reaping must require: no stager lease for the epoch, no pending native install/validation owner for it, no song_return_key, no pending song_ack/song_followup referring to it, no active runtime voice or branch, and no pending runtime outcome/receipt/command still requiring that identity. Only then remove runtime exact leases/active entry and the corresponding active preparation. No broad epoch cleanup before a sibling's final key returns; no early same-key physical reuse.

ResourceRetired/LeaseReturned/PreparationCancelled must preserve existing meanings. Do not invent a new end receipt or claim a rendering-complete acknowledgment unless an existing protocol contract actually supplies one.

### Tasks and evidence targets

1. Relocate cohesive runtime/bus methods before growth; preserve signatures/legacy bodies and retired hashes. Validate parent and child sizes below1000.
2. Admit fixed reset recipes/gate/endpoint state. Prove Native and Arena use actual initialized memory, cells and logical analyzer IDs.
3. Implement exact family mute/reset/unmute and per-frame gate; preserve earlier Applied/rejection FIFO under real ACK pressure.
4. Implement finite branch/epoch fades, hard deadline closure and exact owner reaping through guarded staging.
5. Add genuine Native/Arena fixtures, then root-coordinated full affected verification. No source checks on a mutating tree.

Required actual fixtures:

- Two tracks sharing a family, one untouched family, real room/chain effect and private cells/analyzer. Apply off-block mute; verify family onsets blocked, sibling actual audio/state unchanged, last pre-application frames unchanged, zero by64frames, unmute no stale voice/effect replay. Actual sustained SinOsc reset-reference comparison proves every ramp sample; the private gate probe is supplemental. Primitive ADSR/Perc and implicit envelope preservation is implemented; dedicated primitive envelope-state comparison remains pending. The sustained real reset-reference fixture exercises the ongoing implicit note envelope. Composite kernel reset policy is explicit.
- Same-frame queued onset versus mute, fractional quantum partitions, repeated/rapid mute/unmute, stale epoch, full actual ACK ring and original outcome retained exactly once.
- Release with envelope longer than tail; zero, shorter-than64 and longer tail caps; private branch drain while sibling continues; exact final song-master silence at deadline.
- Native missing/full garbage queue and Arena full critical receipt: physical slot and private banks cannot be reused before exact return, audio continues for unrelated ownership, one-at-a-time drain eventually frees the completed epoch and admits a fresh preparation.
- Callback allocation/deallocation probes around actual Engine.process, exact Native/Arena parity and partition comparisons; full legacy DSP/staging/capacity/wire/query regression suites.

### Wider criteria retained

Private delay-send storage and optional neutral-bus admission, complete pre-track detector causality, deterministic song seed independent of legacy start ordering, cross-Apply old/new fades and cleanup, shared voice quota proof, full08B/08D geometry/query→route→audio join, native/browser production transport and output/end integration remain mandatory. This amendment alone does not complete full SONG-09 or playback.


### Progress: ROOT0293 document amendment

Current source-only lifecycle repairs and first Native/Arena audio pass are retained. Next implementation proceeds by cohesive relocation before adding reset/gate/tail code, under the exact eight future paths and fresh intents. Read-only sample scope preserves Send+Sync and exact epoch selection. Current voice903 and bus965 require conservative line budgeting; every touched Rust remains below1000. New child modules are declared before writes. No Rust or Cargo changed in this document step. Full SONG08 final geometry/transport join remains required for final SONG09 acceptance.

### Shared stage lifetime declaration refinement (ROOT0295)

```rust
pub(crate) struct RuntimeTrack {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) track: usize,
    pub(crate) master: usize,
}
// SongRuntime.tracks: Vec<RuntimeTrack>, constructor capacity=admitted branches.
```

Track/master stage indices remain retained and rendered independently of private branch generations. A private generation may return at its release deadline while song track/master history remains through the epoch endpoint. Actual track/master full-key ownership stays retained until the epoch deadline and exact return. No additional audio memory is allocated; descriptors use the admitted branch bound.

### ROOT0297 superseding future write trade: lifecycle evidence child

Completed `src/dsp/engine/render.rs` is now read-only; its historical edits and first increment evidence remain retained. Current SHA256 `fa36449c7448c39d50ae3d6d52077fd9adbfb71288a9591a8cfc794c281952f8`,384 lines. Register `mod lifecycle;` from owned tests/song_dsp.rs and own new tests/song_dsp/lifecycle.rs instead. The child accesses only existing parent-private Rig and actual graph/event/upload initializations; no fake public test API or duplicated rig. All lifecycle production behavior lives in engine/song_runtime.rs; later full09 rendering changes require prior serial ownership amendment if they need the retired path.

Exact eight future Rust paths: src/dsp/song.rs, src/dsp/engine.rs, src/dsp/engine/song_runtime.rs, src/dsp/bus.rs, src/dsp/bus/song_runtime.rs, src/dsp/voice.rs, tests/song_dsp.rs, tests/song_dsp/lifecycle.rs. This supersedes the previous prospective eight-path budgets, without restoring retired mod/arena/arena-song/protocol ownership. Parent test target remains below1000 after shared Rig extension; new evidence child is budgeted below700. Existing runtime child~850, voice946, Engine932; every touched file must remain below1000 after scoped formatting.

Lifecycle production is written but not independently verified. ROOT0295 behavior and all previous wider criteria remain mandatory. Source remains held until new release following this document-only amendment.

### ROOT0298 full-key shared stage and pressure dependency refinement

RuntimeTrack additionally retains `track_key: SongLeaseKey` and `master_key: SongLeaseKey`; SongEpochEnd and RuntimeBranch retain `closed: bool`. Physical indices never authorize access. Every saved branch FX and shared stage access authenticates exact fullkey plus Live/Retiring state. Endpoint records cease all rendering after deadline; their completed metadata is retained solely for return/reaping. An old record cannot reset a new epoch that reuses its index. Deadline reset happens once on still-matching owned keys.

Receipt-pressure End/Release may bypass queued receipts only for an already active epoch, never overtake pending activation. When activation finally applies beyond a queued finite endpoint deadline, cancel the inactive preparation and reject NotReady before activating any graph; preserve original failures once. Actual pressure fixtures must prove no indefinitely active epoch or music beyond cap.

### Actual shared-stage lookup evidence declaration

RuntimeTrack::renderable(&BusGraph,frame:u64,Option<&SongEpochEnd>)->Result<bool,SongRejectCode> and owned_track/owned_master(&BusGraph)->bool are the exact saved-key predicates used by production rendering/deadline reset and supplemental physical slot-reuse tests. The supplemental test stages real Native/Arena bus payloads and runs real effects; it does not claim new host ingress is possible while guarded retirement blocks control intake. Live Engine fixtures prove no reuse until handoff and eventual admission separately.

### ROOT0298 lifecycle coherent source checkpoint — written, verification pending

Actual family reset/gates, early unmute suppression, endpoint hard caps, empty epoch lifetime, retained full-key shared stages, and guarded reaping are implemented in the exact eight future paths. Scoped formatting completed; no author Cargo executed. Source-only progress does not mark TASK-002 or TASK-003 complete.

Eight new actual Native/Arena public lifecycle fixtures are written: reset-reference ongoing oscillator64 ramp with same-family two tracks and PCM sibling; early-unmute/no replay plus normal muted late arrivals; zero/short/long finite caps; genuine Engine capacity-report ACK saturation plus full garbage queue and eventual exact return; empty Ready silent tail; actual Applied internal-receipt saturation blocking a second genuine Ready activation past its end; real master-filter tail after private generation retirement; whole versus partitioned callbacks. The parent six verified first-increment tests remain intact. A private actual Native/Arena bus test physically reuses a returned track index while the old master remains and exercises the production saved-key eligibility predicates before rendering actual new Gain output. This supplemental adversarial physical reuse is not presented as host ingress while guarded return blocks intake.

Pressure markers are two actual Engine RequestCapacity results. The active case observes the second Mute blocked by the first actual Muted in the single admitted internal receipt slot. The inactive case requires exactly one first Applied, no second Applied, exactly one NotReady rejection after its past cap, and eventual fresh epoch admission. Callback allocator probes continue to surround actual Engine.process only. All new behavior and fixture expectations await mandatory native/host-wasm/strict-Clippy and broad legacy/staging/audio checker execution.

Broader private delay/neutral optional bus, full detector stage, deterministic seed, cross-Apply, shared voice quota, full08B/08D integration, production host/transport/export and dedicated primitive envelope-state witness remain unchecked. Full SONG-09 stays In Progress.

### ROOT0306 actual room smoothing and real oracle correction

The first behavioral lifecycle run passed DSP648 and song audio12, but strict ongoing-reset amplitude and callback partition fixtures failed. Retained source-only diagnostics ran8 actual lifecycle tests (6pass2fail, terminal101): partition divergence starts before mute atframe3 because FxUnit smooths room targets once per block; the intended reset oracle's new events were correctly cleared by its Mute. Tolerance and target waveform requirements remain unchanged.

Private helper `BusSlot::run_range<C: CellRead + ?Sized>(&mut self,range:std::ops::Range<usize>,cells:&C,dry:&mut[f32],ctx:&mut FxCtx<'_>,snapshots:&[KeySnapshot],key:&mut[f32])` processes only the admitted accumulator range. Existing run remains the legacy range0..frames facade, preserving legacy smoothing exactly. Song BusGraph::run_song processes successive one-frame ranges to advance private scalar effect smoothing/history on a sample clock rather than caller partition; sidechain source indexing uses range.start+local frame and detector scratch remains range-local. No allocation or additional memory descriptor. Existing full detector-stage ordering remains pending separately.

The amplitude oracle uses a real unchanged prefix twin and a separate real PCM-only reference whose untouched Sin branches receive new production events exactly at reset frame. Its initialized zero oscillator/room histories equal the target's actual reset histories; old queued notes are not used as a false reference. Actual target preserves ongoing note controls/envelope, strict analytic64-frame gain/tolerance and >50nonzero samples. No zero-frame Engine behavior added. Further author Cargo waits for host writer hold; mandatory full checker remains required.

### Session: 2026-10-02 — actual lifecycle increment verified

ROOT0313 and ROOT0317 independently matched real DSP648 and song audio14
inventory/run identities. Both previously failing oscillator64-frame mute
amplitude and callback partition proofs now pass with the strict real audio
oracle. Empty epochs, early unmute, endpoint/receipt pressure, private cutoff
and retained shared-master tail fixtures pass. Native/host-wasm/strict Clippy
and finish010 formatting/line/diff checks pass. No full phase completion:
primitive envelope witness, physical private delay, causal detector snapshots,
neutral stage preparation, stable event seeds, shared voice quota/Apply and
final08B/08D routing join remain required. Known sampled-reverse route failure
is retained; complete host transport and export are still pending.
