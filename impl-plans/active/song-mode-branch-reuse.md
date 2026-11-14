# Bounded song branch slot reuse implementation plan

**Plan ID**: SONG-BRANCH-REUSE
**Status**: In Progress
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and authority

Supply an opt-in physical branch pool lifecycle for symbolic finite repeats. A logical configuration birth selects an admitted physical slot whose preceding generation has finished its bounded tail. The slot retains its immutable instrument and private effect graph leases until final epoch retirement. This plan provides the audio-side primitive; the preparation owner and bounded scheduler must consume it in their own subsequent plans.

This document authorizes no Rust edits or Cargo. Root must serialize ownership and release implementation after review. Current source-whole work owns separate source paths. No source snapshot in this document overrides another author's current work.

## Related plans

- Depends on `song-mode-dsp-routing.md`, `song-mode-resource-staging.md`, `song-mode-host-command-submission.md`, and actual configured-capacity ownership evidence.
- Consumed by later host preparation and finite transport/scheduler plans.
- Route geometry and actual sampled reflection remain independent incomplete dependencies.
- Read-only evidence: `tmp/song-mode-riela/SONG-08D/TASK-003-runtime-reuse-boundary-readonly-040.json`.
- Before document receipt: `tmp/song-mode-riela/SONG-BRANCH-REUSE/0001-document-before.json`.

## Exact future write manifest

| Path | Observed lines | Deliverable |
|---|---:|---|
| `src/song/routing.rs` | 557 | Opt-in PODs, appended commands and receipt, validation |
| `src/host/caps/song.rs` | 561 | Checked codecs for the new PODs |
| `src/dsp/song.rs` | 654 | Keyed runtime/stager metadata types, preallocated runtime vector, queue rank |
| `src/dsp/engine/song.rs` | 935 | Compact dispatch to sibling reuse implementation |
| `src/dsp/engine/song_runtime.rs` | 916 | Child registration, lifecycle integration and bounded cohesive extraction |
| `src/dsp/engine/song_runtime/reuse.rs` | absent | Cohesive reusable registration, preflight, commit and cleanup helpers |
| `src/dsp/arena/song.rs` | 923 | Preallocated keyed staging metadata and final cleanup |
| `tests/song_branch_reuse.rs` | absent | Actual Native/Arena codec, DSP, pressure and allocator witnesses |

All eight are future paths only. Every touched Rust file must remain below 1000 lines. Extract cohesive retirement/reuse helpers from the 916-line runtime parent before growth; keep the 935-line engine dispatch compact. Stop and obtain a prior manifest amendment if the stager's bounded additions do not fit. No implicit ninth fixture or host source path.

`src/dsp/bus/song_runtime.rs` remains read-only: `reset_song_bus`, `can_reset_song_bus`, and `song_is_live` already supply exact-key reset/admission. Keep `RuntimeBranch` unchanged to avoid unowned struct literal changes. No new generic bus framework or resource pricing changes.

## Existing behavior and exhaustive consumers

- `Engine::stage_song_branch` requires an open preparation, unique epoch/branch, adopted full-key resources and admitted branch count. It cannot reconfigure an active branch.
- Runtime branch config remains fixed; Release marks ended and closes the deadline. Current retirement marks resources retiring and removes branches. A bus reset alone does not create a new generation.
- `SongRuntime::new` and `SongResourceStager::new` allocate fixed capacity before processing. New keyed vectors use the same admitted branch ceiling, never positional alignment with `retain` or removal.
- Exhaustive command sites: routing `epoch`/`valid`; codec encode/decode; runtime queue and timed execution; engine `song_command`. Exhaustive acknowledgment sites: routing `epoch` and codec encode/decode. Generic CtlMsg/ByteInbox ingress delegates to the codec.
- Browser/native checked sender methods forward arbitrary checked PODs; their acknowledgment handling has fallback paths. Existing variant enumeration tests are historical cohorts, not proof of the new variants. This plan's actual Native/ByteInbox witnesses cover the additions; any required existing host source change needs separate prior ownership.
- Current command subtags are 0 through 17; ACK subtags are 0 through 12. Append proposed commands 18/19 and ACK 13 only after re-reading the codec at source release. Preserve all existing tags, lengths and holes.

## Declared types and interfaces

### Routing PODs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongReusableBranch {
    pub config: SongBranchConfig,
    pub last_generation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongBranchRebind {
    pub epoch: SnapshotEpoch,
    pub branch: SongBranchId,
    pub expected_generation: u32,
    pub generation: u32,
    pub transition_frame: u64,
    pub tail_deadline: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongBranchRebound {
    pub epoch: SnapshotEpoch,
    pub branch: SongBranchId,
    pub generation: u32,
    pub frame: u64,
}
// Additive variants in the existing enums:
// SongCommand::ConfigureReusableBranch(SongReusableBranch)
// SongCommand::RebindBranch(SongBranchRebind)
// SongHostAck::BranchRebound(SongBranchRebound)
```

Initial generation must not exceed `last_generation`. Rebind requires checked `expected_generation + 1 == generation`, and generation must not exceed the admitted last generation. Reject wrap, skipping, stale epoch/generation and reversed timing. No fallback to eager repeat expansion. Preparation must prove a conservative maximum birth count per physical slot fits the remaining u32 range before activation; this declaration is an explicit later preparation-owner prerequisite, not a proof from live-slot capacity alone.

The later owner supplies proved total logical births O without dividing by physical
slot count or multiplying already-included configurations again. With initialgen1,
last_generation=O (no extra+1); O0 allocates no pool, O in1..=u32::MAX is checked,
O>MAX rejects upfront. Other initial generations require checked initial+O-1.
These are symbolic bounds, not an eager Repeat schedule or evidence that unfinished
Slice/dynamic component admission is complete. Reuse child checks the supplied range;
it does not acquire an unowned route-density helper.

Codec declarations retain existing functions:

```rust
pub(crate) fn encode_command(command: SongCommand, out: &mut [u8]) -> usize;
pub(crate) fn decode_command(bytes: &[u8]) -> Result<(SongCommand, usize), WireError>;
pub(crate) fn encode_ack(ack: SongHostAck, out: &mut [u8]) -> usize;
pub(crate) fn decode_ack(bytes: &[u8]) -> Result<(SongHostAck, usize), WireError>;
```

ConfigureReusableBranch encodes the existing ConfigureBranch body plus last_generation, with its own appended subtag. Existing branch option flags and resource fields remain exact. Its length is existing ConfigureBranch encoded length plus four, not a guessed fixed padding. Rebind is 38 bytes (two tag bytes, epoch8, branch4, expected4, next4, frame8, deadline8). Rebound ACK is 26 bytes (two tag bytes, epoch8, branch4, generation4, frame8). Verify all additions fit existing command/ACK maxima; preserve old record consumption, truncation rejection and following-record framing.

### Keyed metadata in existing bounded stores

```rust
#[derive(Clone, Copy)]
pub(crate) struct SongReusableSlot {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) branch: SongBranchId,
    pub(crate) last_generation: u32,
}
// SongRuntime field:
// pub(crate) reusable: Vec<SongReusableSlot>
// SongResourceStager field:
// pub(crate) reusable: Vec<SongReusableSlot>
impl SongRuntime {
    pub(crate) fn reusable_slot(
        &self, epoch: SnapshotEpoch, branch: SongBranchId,
    ) -> Option<SongReusableSlot>;
}
impl SongResourceStager {
    pub(crate) fn reusable_slot(
        &self, epoch: SnapshotEpoch, branch: SongBranchId,
    ) -> Option<SongReusableSlot>;
}
```

Both vectors have capacity equal to configured branch count and are initialized outside processing. Registration preflights both metadata and branch capacities before any push. Duplicate full keys reject. Every lookup is bounded by admitted slots; no map allocation, auxiliary unbounded registry, vector index coupling or RuntimeBranch field change.

### Engine child and sibling visibility

```rust
impl Engine {
    pub fn stage_song_reusable_branch(
        &mut self, branch: SongReusableBranch,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn rebind_song_branch(
        &mut self, rebind: SongBranchRebind, frame: u64,
    ) -> Result<SongBranchRebound, SongRejectCode>;
    pub(in crate::dsp::engine) fn validate_song_private_exclusivity(
        &self, config: SongBranchConfig,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn validate_reusable_private_slot(
        &self, branch: RuntimeBranch,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn retains_reusable_song_lease(
        runtime: &SongRuntime, key: SongLeaseKey, final_retirement_due: bool,
    ) -> bool;
    pub(in crate::dsp::engine) fn cleanup_song_reusable_slots(&mut self);
}
```

`engine/song_runtime.rs` declares its reuse child explicitly. Sibling `engine/song.rs` calls methods with `pub(in crate::dsp::engine)` visibility; child-only `pub(super)` is insufficient. Private reset uses the existing admitted immutable BusTemplate and owner-local cells. Instrument template, controls, analyzers, samples and effect dependencies retain their actual full lease keys.

## Current private-delay integration and exact extraction

ROOT0385 independently proves private delay physical reset and memory. The reusable
logical generation keeps the SAME PrivateFx SongLeaseKey: reset_song_bus clears full
Room/FX/delay histories, line cursors, sends and output buffers, but preserves current
delaytime/feedback plus Room controls. New physical lease owner defaults are restored
by adoption, not by changing logical RuntimeBranch generation. Do not alter this
contract or reset shared track/master state. Future events supply explicit controls
when the new immutable policy needs different values.

All ordinary stage_song_branch calls must invoke validate_song_private_exclusivity
before borrowing the stager mutably or pushing a config. Reusable registration and
rebind also use it. Compare complete epoch/resource/generation PrivateFx identity
against every OTHER staged/live branch; exclude only the same epoch/branch being
preflighted for rebind. A later ordinary stage cannot alias an already-registered
reusable private lease. Existing ordinary absent-PrivateFx staging behavior remains,
while actual reusable config requires a full PrivateFx key. Shared immutable
instrument definitions and epoch track/master buses are unaffected.

validate_reusable_private_slot proves exclusive exact live slot, users==0, no active
old owner voices and reached old tail deadline before reset. can_reset_song_bus
currently proves only stored immutable definition presence; strict original adoption,
fixed supported Engine sample rate and immutable slot geometry establish reset fit.
Actual full-key layout must match that admitted slot. Failed preflight cannot clear
histories/config/voices or advance generation. Old voices from another branch may
not be hidden by checking only the selected old owner.

Borrowed retains_reusable_song_lease avoids an &Engine borrow while retirement owns
mutable runtime/stager fields. final_retirement_due is derived from the actual epoch
cancelling state or reached final endpoint deadline, not merely logical Release.
The bounded declared immutable epoch resource closure remains pinned while an opt-in
pool may reference it; foreign epoch keys are never pinned. Current runtime/stager
lease identity remains authoritative; no array index coupling or new side registry.

Extract these existing called helpers cohesively into the declared reuse child:
```rust
impl Engine {
    pub(in crate::dsp::engine) fn retire_song_runtime(&mut self, frame: u64);
    pub(in crate::dsp::engine) fn finish_song_deadlines(&mut self, frame: u64);
    pub(in crate::dsp::engine) fn reap_returned_song_epochs(&mut self);
}
```
Engine::process calls all three from engine.rs383–385. Child pub(super) would be too
narrow; preserve explicit engine ancestor visibility. Move existing retirement/
deadline/reaping bodies before substantive parent growth. engine/song.rs935 contains
only compact dispatch plus preflight calls; arena/song.rs923 adds bounded vector
initialization/keyed lookup/cleanup only. Stop before any file reaches1000; no ninth
path is implicitly authorized. Current observed sizes are not frozen source baselines.

## Lifecycle and atomicity contract

- Opt-in staging preserves the old ConfigureBranch/Release one-shot behavior. Only reusable keyed branches retain pool ownership after a logical Release. Track/master remain shared epoch stages.
- Before mutation: exclusive full PrivateFx lease among all other staged/live ordinary and reusable branches, actual slot.users==0, and active epoch, no final endpoint/cancel state, registered reusable key, exact expected generation, representable next generation, actual frame equals scheduled transition, legal new deadline, previous branch ended and deadline reached, no active old owner voice, exact live instrument/private bus/track/master leases, and immutable private bus reset eligibility.
- Preflight receipt capacity before reset. Validate all resource identities and all possible failure conditions before touching voices, buffers, config or keyed state. Existing bus reset eligibility must prove its reset call cannot subsequently fail during the same callback; no concurrent callback mutation. If this cannot be established, revise the contract before implementation.
- At commit, reset only the admitted private graph, clear old voice ownership after proving no active users, update generation and timing, reopen the new private gate consistently with family mute state, and emit exactly one BranchRebound containing actual frame. Preserve track/master state and original immutable template/graph keys. Do not transfer old effect memory between configuration pools.
- Previous generation Event/Release cannot start, end or mute the new generation accidentally. A family mute remains a family operation; a successful rebind does not undo a still-muted family.
- Initial registered branch still uses activation/transition rules. Rebind is a timed command and cannot be applied as immediate unchecked staging.
- Same-frame queue order: Release/Endpoints first, Rebind next, other transitions next, Event last; stable FIFO within rank. Finish reached deadlines AFTER each successful same-frame Release and BEFORE the next Rebind preflight/commit. Current normal apply loop finishes only at entry; integration must add this explicit safety step outside the atomic rebind transaction. Final Endpoints forbids reopening the epoch.
- Full critical receipt storage blocks rebind before mutation; do not use the existing pressure shortcut to commit it without receipt storage. Following new-generation events cannot bypass a blocked rebind. Deadline/release safety actions continue under the existing bounded pressure contract.
- Logical completion retains reusable instrument/private FX and their dependency leases. Final epoch endpoint/cancellation switches to ordinary full retirement. Shared samples/banks remain pinned while any active reusable pool can reference them. Do not retain unrelated foreign epochs.
- Remove staging/runtime reusable metadata by explicit epoch/branch keys only after the corresponding final lifecycle closes; preserve exact resource ownership until existing safe garbage/receipt handoff. No early ID reuse, callback destruction, fake Ready, or client-POP-dependent lease handoff.

## Tasks

### TASK-001: Protocol and bounded metadata

**Status**: Completed
**Parallelizable**: No (serialized shared DTO ownership)
**Deliverables**: routing, caps/song, dsp/song, arena/song

- [x] Exact POD validation, proposed appended tags revalidated, fullwidth and option codec proof.
- [x] Keyed preallocated vectors initialized before callback, checked register capacity and cleanup.
- [x] Existing one-shot commands, ACKs and wire fixtures remain valid.

### TASK-002: Cohesive runtime reuse

**Status**: In Progress
**Parallelizable**: No (depends on TASK-001)
**Deliverables**: engine/song, engine/song_runtime, new reuse child

- [x] Split before parent growth, correct sibling visibility, atomic preflight/reset/commit.
- [ ] Timed order, exact acknowledgment pressure semantics and generation range checks.
- [ ] Retain only opt-in pool dependencies through logical tails; final cancellation/end cleanup.
- [x] Legacy one-shot release/retirement preserved.

### TASK-003: Actual transport, audio and ownership witnesses

**Status**: In Progress
**Parallelizable**: No (depends on TASK-002)
**Deliverable**: tests/song_branch_reuse.rs

```rust
fn reusable_branch_native_and_arena_produce_nonzero_audio_across_generations();
fn reusable_branch_rejects_overlap_stale_keys_and_generation_wrap_atomically();
fn reusable_branch_transition_order_is_partition_invariant();
fn reusable_branch_pressure_retains_exact_owners_until_safe_handoff();
fn reusable_branch_codec_preserves_full_keys_and_following_records();
fn reusable_branch_callbacks_allocate_and_deallocate_nothing();
fn ordinary_and_reusable_branches_reject_shared_private_full_keys();
fn reusable_branch_nonzero_users_and_tail_preflight_preserve_state();
fn reusable_branch_same_owner_delay_reset_retains_controls_clears_history();
```

- [x] Real configured Engine Native/Arena setup, actual adoption/Ready/Applied/Rebound receipts, complete instrument/private/track/master closure and measured slot/PCM/voice extents.
- [x] Nonzero reference audio; many logical births through fewer physical slots, constant allocated capacity and bounded command horizon, no expanded score/repeat array.
- [ ] Ordinary→ordinary, ordinary→reusable and reusable→ordinary alias attempts reject before mutation, using full lease keys; actual nonzero slot users reject even without a selected old-owner voice.
- [x] Same private lease rebind retains delaytime/feedback and clears sends/history/cursors, with actual dry-Mute delayed-return and silence witnesses.
- [ ] Distinct tail-positive slots retain independent nonzero old tails across reusable transitions.
- [ ] Tail-positive old/new overlap uses distinct physical slots; premature same-slot rebind fails without buffer/config/voice changes, safe post-deadline rebind resets private state and preserves unrelated track/master audio.
- [ ] A→B→A uses separate admitted immutable graph pools; no old tail leaks into A's new generation.
- [ ] Full epoch/resource generations, foreign/stale events/releases, skipped/wrapped/exhausted generation limits and final-epoch reopening reject without mutation.
- [x] Exact same-frame ranked Release/Rebind/Event, reversed ingress proof and identical actual audio/receipts across callback block partitions.
- [ ] Full/fractional/reordered scheduler query-window integration remains a later owner/scheduler requirement.
- [x] Actual full ACK ring and garbage pressure, one-at-time handoff, FIFO/exact-once receipts, retained Native Box/Arc addresses and Arena resources, no early lease/ID reuse.
- [x] Thread-local allocation/deallocation probe around actual Engine processing only; all setup, encoding, assertions and owner drops outside probe.
- [x] Real ByteInbox complete records, truncations, invalid option flags/generation ranges, fullwidth values and following-record recovery; no manually invented success receipts.

## Module status and dependencies

| Area | Status | Tests |
|---|---|---|
| POD/codec and keyed metadata | Scoped verified | ROOT0400 public9 + regression matrix |
| Atomic runtime and final ownership | In Progress; same-slot scope verified | public9, physical users1, queue7; broader pools pending |
| Actual Native/Arena audio/pressure | Scoped verified; broader criteria pending | public9 + actual nextest repeats |

| Dependency | Requirement |
|---|---|
| Root ownership release | All shared writers held; fresh baselines before Rust |
| Existing bus reset | Read-only exact-key primitives reused |
| Preparation/scheduler follow-on | Prove generation range and bound horizon before actual full song playback |
| Geometry | Complete authentic source/configuration identity remains required |

## Completion criteria

- [ ] All three tasks and every actual fixture criterion proven independently.
- [x] Native/browser checks, strict Clippy, scoped format/diff, unfiltered new suite, legacy lifecycle/host/codec/staging regression and fresh nextest evidence.
- [x] Every touched Rust below 1000 lines, same eight-path manifest, no dependency changes.
- [ ] Actual live/retiring physical pool bounds and receipt/garbage ownership remain correct.
- [ ] Parent plan reconciles proven primitive scope; full song scheduling, playback/export and remaining geometry are not declared complete by this child alone.

## Progress log

### 2026-10-02: Document-only preparation

Inspected actual current DTO/codec, runtime ordering and pressure handling, branch retirement, stager construction and existing bus reset primitives. Current source sizes are observation-only baselines in 0001. No Rust changed and no Cargo ran. Plan is Ready for root review; implementation has no source authorization.

### ROOT0387 prior Ready contract refinement, document only
ROOT0385 accepts actual delay/reset evidence; logical reuse remains unimplemented.
Current codec has commands0..17 and ACK0..12, so proposed18/19/13 remain future-only.
Exact full-key exclusivity for ALL stage paths, users/voices/deadline guards, borrowed
retention, extracted ancestor visibility and Release→deadline→Rebind FIFO are now
declared before source authorization. Existing eight future Rust paths suffice;
Ready is retained as a feasible contract, not source or behavioral acceptance.
Generation O proof remains an explicit owner prerequisite. No Rust/Cargo ran.

### ROOT0389 source declarations before implementation
Exact codec inventory commands0..17/ACK0..12 revalidated. Additional private
helpers, no bodies, under the same eight-path manifest:
```rust
impl SongRuntime {
    fn command_rank(command: SongCommand) -> u8;
}
impl SongReusableBranch { pub fn valid(self) -> bool; }
impl SongBranchRebind { pub fn valid(self) -> bool; }
```
Extraction retains existing retirement/deadline/reaping algorithms before adding
keyed pool retention. Registration copies metadata at activation after complete
capacity preflight; no RuntimeBranch literal fields change. Source authorized by
ROOT0389; no Cargo until both writers held. Independent checks remain pending.

### Test fixture declarations before new source
The owned public test uses a local cohesive copy of the proven private-delay Rig;
its existing source remains read-only. The copy registers reusable metadata at
staging and retains the actual Native/Arena PCM/graphs/ACK/garbage paths.
```rust
fn key(id: u32, kind: SongResourceKind) -> SongLeaseKey;
fn effect(kind: EffectKind, name: &str, value: f32) -> EffectSpec;
fn linear_gain_chain(gain: f32) -> Box<[EffectSpec]>;
fn event_for(generation: u32, frame: u64) -> SongCommand;
fn rebind_for(expected: u32, frame: u64) -> SongCommand;
fn release_for(generation: u32, frame: u64, deadline: u64) -> SongCommand;
fn assert_rebound(rig: &Rig, generation: u32, frame: u64);
impl Rig {
    fn new(bytes: bool, ack_slots: usize, sr: f32, bus_seconds: f32) -> Self;
    fn command(&mut self, command: SongCommand);
    fn process(&mut self, frames: usize, drain: bool) -> Vec<f32>;
    fn drain(&mut self);
    fn graph(&mut self, lease: SongLeaseKey, inst: Option<&InstDef>, bus: Option<&BusDef>);
    fn prepared(bytes: bool, private_gain: f32, track_gain: f32, master_gain: f32) -> Self;
    fn activate(&mut self, frame: u64);
    fn audio(&mut self, frames: usize, partition: usize) -> Vec<f32>;
}
struct Probe;
unsafe impl std::alloc::GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout);
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8;
}
TLS counter is armed only around actual Engine processing; all encoding, owner
allocation, assertions and returned owner destruction are outside the probe.

### Additional exact test seams before source
```rust
fn event_preserving_controls(generation: u32, frame: u64) -> SongCommand;
#[cfg(test)] fn physical_reuse_engine() -> Engine;
#[cfg(test)] fn real_private_slot_users_block_rebind_without_an_old_voice();
```
The private helper builds/adopts actual Native immutable graphs and activates the
real configured Engine, then injects only the otherwise-independent users count
for a negative preflight witness. No public/test-only production API is added.
The controls witness omits new delaytime/feedback after reset and requires delayed
nonzero output within128frames with dry Mute, proving retained .001s vs .25s default.
Authentic future Event enqueue is Rebind→Release→Event, with execution rank
Release→Rebind→Event; missing Release proof is refused by the caller child.

### ROOT0389 first coherent implementation, held for independent check
POD/codec commands18/19 and ACK13 written after inventory; keyed preallocated
runtime/stager vectors and lookup/cleanup written. Retirement/deadline/reap bodies
extracted to reuse child with engine ancestor visibility. Ordinary/register/rebind
PrivateFx alias preflight, users/voice/deadline/resource/range/receipt guards,
same-owner private reset, retained family mute and epoch lease pinning written.
Release explicitly finishes reached deadlines before following ranked Rebind.
All nine declared public fixtures and a real-adoption private users guard written;
TLS probes wrap actual callbacks. No author Cargo and no tests claimed passing.
ROOT0390 caller child owns actual engine.rs future-generation queued-chain admission;
first valid partition fixture enqueues Rebind→Release→Event with authoritative
Release→Rebind→Event execution. Pressure queues real next Event behind blocked
Rebind and requires silence, unchanged leases and late rejection. Tests retain
exact Native returned addresses and Arena actual ingress. Broader A→B→A distinct
pool/tail overlap and additional adversarial generation/final-state cases remain
criteria to audit against actual gates; this held increment is not full acceptance.

### ROOT0393 exact private fixture type correction
Independent001 original32120 terminal101: native/browser checks passed, strict
Clippy test compilation failed at reuse.rs489 because NativeInstall::Bus needs
Box<BusTemplate>. No behavioral tests executed. Retained clippy.log SHA
5913fe7496648dd15cfe80e977aa575f8ed9da269e66d7a64eb7d4b064182d6e.
Only Box::new around actual fixture template corrected; production and all semantic
assertions preserved. Scoped format checked; no author Cargo, acceptance pending.

### ROOT0395 deterministic ownership fixture ordering
Independent002 original69208 terminal101: native/browser checks passed; strict
Clippy test compilation found SongLeaseKey lacks Ord for two sort_unstable calls.
Retained clippy.log SHA4fd8a182f52ee2c573aa7960e6b711ecc3739e9420431de253383053cf8e41db;
no behavioral execution. Both calls now sort by full epoch/kind/id/generation/pointer
identity, preserving original vectors and exact final ownership equality assertion.
No production ordering trait or semantics changed; scopedfmt0, no author Cargo.

### ROOT0397 unused fixture storage removal
Independent003 original81758 terminal101: strict Clippy rejected unused Rig.caps
and Rig.baseline storage. Retained clippy.log SHA
faa404b50fdf9ab3df8290f8c52130dcb7d7b9e5ccfbfb39a51e53387049ee81.
Removed only both fields, assignments and unused initial baseline measurement.
Actual local caps construction and every subsequent measured capacity/PCM/owner/
allocator assertion remain unchanged. Compile-only failure; no behavior claim.
Scopedfmt0; no author Cargo or production changes.

### ROOT0400 independent scoped acceptance (ROOT0402 document-only reconciliation)
Matrix /tmp/vactr-freeze-reuse-independent-004/final-results.json SHA
f34310087d469cdf8de2edadf2d68000cc25ddc8c50e81b2ea40812855c8953e;
original27304 terminal0,67 gates,1030 distinct actual names (1026 tests+4privacy).
Nine public reuse fixtures, actual private physicalusers guard and companion queue7
passed. Supplemental nextest foreground4f743a exit0:18 selectedPASS,0selectedSKIP;
result SHAd405431dc16956e4eae985688518ccedd788b3077f93774d603849c4485ccbd2.
All102 held hashes matched. Earlier compile-only failures remain immutable.
Actual proof covers codec/fullkey, same-slot repeated generations, retained controls/
cleared history, alias/user/tail preflight, ranked enqueue/execution, pressure Event
blocking/late rejection, exact owner returns and all callbacks zeroalloc/zerodealloc.
Combined broader clauses remain unchecked rather than inferred from same-slot tests:
distinct pool A→B→A, simultaneous independent positive tails, all alias directions,
final-generation/final-state adversarial runtime audits and scheduler query windows.
Whole logical O range remains owner prerequisite; preparation/scheduler not consumed.
Plan In Progress. No Rust/Cargo or other plan changes in this reconciliation.
