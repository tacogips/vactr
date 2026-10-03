# Song replacement-source recovery

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Atomic Apply and playback](../../design-docs/specs/design-song-mode.md)

## Purpose and authority

Restore the unchanged old transport's ability to issue a subsequent Apply after
an unsubmitted refusal or authenticated cancellation of a never-activated
replacement. Move the original opaque source back; never create a second proof.
This document authorizes no Rust writes. Source release and coordinated checker
are separate. Existing atomic DSP transaction/pressure work remains disjoint.

Related plans: [host preparation](song-mode-host-preparation.md),
[transport core](song-mode-transport-core.md), [atomic handoff](song-mode-atomic-apply-handoff.md).
The design's failed Apply leaves the old song unchanged; recovery must preserve
that ownership as well as PCM, mute state and future one-shot issuance.

## Actual source seams

- `src/sched/song.rs:136`: replacement_source sets source_issued once; no reset.
- `preparation/activation.rs:219`: PendingReplacement retains the non-Clone
  source; take_replacement_commit removes it only after actual Applied.
- `preparation.rs:456`: Cancelling requires actual PreparationCancelled, no
  outstanding upload and every admitted full-key lease returned.
- `preparation.rs:768`: take_cancelled additionally requires flight/upload empty
  and returns the original PreparedSong once. Recovery must survive either order
  of candidate/source extraction, because cleanup metadata remains owned.
- `engine/song_runtime/replacement.rs:333` (read-only): queued/rejected
  replacement cancellation emits correlated ActivationRejected and invokes real
  cancellation. Armed/Applied cancellation instead records HostFault and keeps
  guarded retirement; it cannot prove a never-activated source is reclaimable.
- `engine/song.rs` (read-only): owned graph/sample garbage handoff and exact
  LeaseReturned precede PreparationCancelled. ACK pressure delays this proof.

Generic Rejected, elapsed wall time, local invalidation or submitted Cancel alone
are insufficient. No new wire command/receipt or DSP implementation is required.
Actual Applied permanently consumes the source into the existing opaque commit.

## Exact future manifest

Six Rust modules. All currently held; observations are not permission to edit.

| Path | Observation | Deliverable | Status |
|---|---:|---|---|
| `src/host/caps.rs` |456 lines|export refusal|Not started|
| `src/host/caps/song/preparation.rs` |875|terminal cleanup proof|Not started|
| `src/host/caps/song/preparation/activation.rs` |448|opaque source return|Not started|
| `src/sched/song.rs` |580|old reclaim and forwarding|Not started|
| `tests/song_apply_host.rs` |581|child registration only|Not started|
| NEW `tests/song_apply_host/recovery.rs` |absent|Native/Arena fixtures|Not started|

Keep every touched Rust below1000. Parent tests preserve all existing fixtures,
TLS allocator probe and helpers. Child uses `use super::*;` and declared path
`#[path = "song_apply_host/recovery.rs"] mod recovery;`.
No receipts child, runtime/session integration, DSP, adapter or codec writes.
An extra caller path requires prior amendment. No dependencies or Git changes.

## Exact typed deliverables

```rust
pub struct SongReplacementSourceRefusal {
    pub source: SongReplacementSource,
    pub failure: Failure,
}
impl SongHostPreparation {
    pub fn take_cancelled_replacement_source(&mut self)
        -> Result<Option<SongReplacementSource>, Failure>;
}
impl SongTransport {
    pub fn take_cancelled_replacement_source(&mut self)
        -> Result<Option<SongReplacementSource>, Failure>;
    pub fn reclaim_replacement_source(&mut self, source: SongReplacementSource)
        -> Result<(), SongReplacementSourceRefusal>;
}
```

Export refusal through activation/preparation/caps facade. Preserve private
source fields and no Clone/Copy. No raw-public constructor or mutable getter.
Declaration refinement before implementation in authorized scope:

```rust
impl SongPreparationCleanup {
    fn take_cancelled_replacement_source(&mut self, completed: bool)
        -> Result<Option<SongReplacementSource>, Failure>;
}
```

`completed` is passed only by the owner after authenticated Cancelled terminal
state. The private helper also checks actual cancel_posted/cancelled, flight and
upload empty, every admitted resource returned, activation NeverSubmitted or
RejectedNeverActivated, no Applied, and retained source. Submitted replacement
requires exact ActivationRejected already correlated to its requested activation.
Return Ok(None) while proof is incomplete or after once-only extraction; applied
or armed/uncertain state never yields a source. Do not generalize local Failed or
normal Retired to cancellation proof. A terminal cleanup source must be available
before or after take_cancelled consumes PreparedSong, without fabricating epoch.

Any ledger scan uses the original remaining work counter with checked count
before scanning or mutation; exhausting work retains source and cleanup owner.
Extraction moves existing allocation/authority, with no upload, ACK, quota reset,
new source issuance or new per-resource collection. Never callback-allocate.

Reclaim compares source activation to the old transport's actual acknowledged
activation and source endpoints to its exact current endpoints. Require old
source_issued and authentic old epoch/state Playing, Draining or Ended. Validate
all fields before changing source_issued. Wrong owner, changed endpoints, missing
Applied or already reclaimed returns the identical source in refusal. Successful
reclaim consumes source and resets source_issued; subsequent issue stays one-shot.
An old transport that itself was a replacement may reclaim its own issued ticket;
its existing replacement metadata is not a blanket refusal condition.

Unsubmitted prepare_replacement refusal already owns the returned original
source; reclaim can use it directly because no new handoff intent was accepted.
Accepted priming/new Ready requires full actual cancellation cleanup first.
Taking or observing an Applied commit cannot expose reclaimable authority.
Natural Ended retains immutable old identity but never resurrects old resources,
rewinds endpoints, produces notes, or cancels a different owner.

## Tasks and dependencies

| Task | Deliverable | Depends on | Parallelizable | Status |
|---|---|---|---|---|
| TASK-001 |terminal source return/refusal exports|source release|No|Not started|
| TASK-002 |exact old reclaim/core forwarding|TASK-001|No|Not started|
| TASK-003 |real Native/Arena recovery fixtures|TASK-002|No|Not started|

### TASK-001 completion

- [ ] Return only the original opaque source after full authenticated cancellation.
- [ ] Candidate/source extraction works once in either order; no generic Failed shortcut.
- [ ] Partial priming, queue/ACK pressure, foreign receipts and outstanding uploads
      retain ownership until actual safe terminal proof.
- [ ] Armed/Applied paths cannot return reclaimable source; existing commit preserved.

### TASK-002 completion

- [ ] Wrong-owner refusal preserves exact activation/endpoints and original source.
- [ ] Successful reclaim enables exactly one later issue without changing old PCM,
      endpoints, activation, settings, mute state or resource ownership.
- [ ] Direct unsubmitted refusal and natural Ended ownership are both handled safely.

### TASK-003 exact fixture declarations

```rust
fn cancelled_replacement_reclaims_old_source_and_second_apply_succeeds();
fn unsubmitted_refusal_and_wrong_owner_preserve_original_source();
fn partial_priming_and_ack_pressure_delay_recovery_until_real_cleanup();
fn applied_or_armed_replacement_never_returns_reclaimable_source();
fn naturally_ended_old_source_reclaims_without_resource_resurrection();
```

Each relevant fixture runs both real Native and encoded Arena ingress with
actual configured Engine, original PreparedSong and real ACK dispatcher. Use
existing test helpers/allocator. Capture immutable source fields before moving;
assert exact same fields after refusal/recovery and actual new Applied for the
second valid Apply. Preserve old nonzero PCM through cancelled attempt against a
same-history reference; natural Ended asserts silence and no restored leases.
Future activation frames must be genuinely ahead of observed clock.
After-arm witness must cross actual arm boundary, not infer it from queued POD.
Foreign/stale receipts remain intact; no fabricated successful ACK or certificate.
Every actual callback remains zero allocation and zero deallocation. Constructors,
assertions and garbage destruction stay outside probe. Full-key lease returns and
original PreparedSong are each accounted exactly once; source recovery is also
once-only under repeated polling and extraction-order permutations.

## Verification and completion

- [ ] All declared fixtures actually pass under independent checker.
- [ ] Existing handoff fixtures, transport tests and ownership regressions pass.
- [ ] Native check, pure wasm check/build, strict Clippy and scoped rustfmt pass.
- [ ] All touched Rust below1000, exact source/plan hold recorded.
- [ ] Source recovery neither replaces atomic transaction proof nor claims full
      controller/transport/overall Song completion.

Checker commands use CARGO_TERM_QUIET=true; nextest additionally uses the repo
required failure-only environment. Author runs no Cargo. Coordinated source hold
is required before checker; root's active DSP pressure author remains separate.

## Progress log

### 2026-10-03: declaration-ready, implementation not started

Actual source review establishes retained opaque authority and existing correlated
pre-arm rejection/cancellation receipts. No new protocol prerequisite found.
Immutable before declaration: SONG-HANDOFF-RECOVERY/0001-document-intent.json.
Rust remains unchanged; fixtures above are expectations, not executed evidence.

### 2026-10-03: source release and exact fixture helper declarations

Root released the six-path manifest; before intent0005 preserves current hashes.
Additional child-only fixture signatures before writing:

```rust
fn horizon() -> TimeSpan;
fn receive_old_or_new(rig: &mut Rig, old: &mut SongTransport, new: &mut SongTransport)
    -> Vec<SongHostAck>;
fn drive_cancel(rig: &mut Rig, old: &mut SongTransport, new: &mut SongTransport)
    -> (Vec<SongHostAck>, Vec<(u64, [f32; 32])>);
fn assert_old_pcm(bytes: bool, epoch: u64, actual: &[(u64, [f32; 32])]);
fn make_replacement(rig: &mut Rig, old: &mut SongTransport, epoch: u64, ahead: u64)
    -> SongTransport;
fn submit_all(rig: &mut Rig, new: &mut SongTransport);
fn apply_and_retire(rig: &mut Rig, old: &mut SongTransport, new: &mut SongTransport);
```

No test execution claims; independent checker follows the complete source hold.

Before pressure helper source: `fn render_retaining_receipts(rig: &mut Rig);`
retains actual ACK records; Native8192 is actual constructor ACK_CAPACITY, Arena
uses actual producer.capacity(). No synthetic status or callback allocation.

Physical ACK pressure uses monotone real RequestClock reports; RequestCapacity
origin ledger1024 is separate from Native ACK8192 and is not a ring-full proof.

### 2026-10-03: source-complete held checkpoint

Six authorized Rust paths implemented and scoped rustfmt/check passed. Parent
handoff tests are byte-identical to before0005 except child registration. Five
new Native/Arena fixtures are written, not executed. Physical ACK pressure uses
retained real clock reports and no callback ACK pop; command admission pressure
also remains. Same-history old PCM is compared bit-for-bit in isolated sequential
Arena runs, avoiding global-outbox interference. Candidate-first and source-first
terminal extraction orders, exact once-only returns, wrong-owner refusal and
after-arm/Applied exclusion are covered by written assertions. No Cargo run.
Independent joined checker still required; overall Apply/Song remains incomplete.
