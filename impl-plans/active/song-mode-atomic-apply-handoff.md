# Prepared owner handoff for atomic Song Apply

**Status**: In Progress
**Created**: 2026-10-03
**Updated**: 2026-10-03

## Design references

- [Implementation review](../../design-docs/specs/design-song-mode.md#implementation-review-2026-10-03)
- [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)
- [Additive wire phase](song-mode-atomic-apply-wire.md)

## Scope and prerequisites

Implement the consuming Ready/scheduler handoff for the additive Replace and
PrimeMute intents. This phase does not implement the DSP transaction or claim
an applied replacement from enqueue success. Source release must follow the
clock author's held preparation.rs baseline and the current checker's terminal
review. This document is source-grounded and Ready for parent review; Rust
writes remain unauthorized until the parent releases the exact manifest.

The future DSP phase owns seven modules: dsp/song.rs, engine.rs,
engine/song_queue.rs, engine/song_runtime.rs, new
engine/song_runtime/replacement.rs, engine/song_runtime/reuse.rs, and
new tests/song_apply_runtime.rs. Its prior manifest must be declared before
writing. Positive replacement tests here require that implementation; until
then actual Engine NotReady refusal is expected, never synthetic Applied.
The future DSP phase must define before-arm and after-arm cancellation outcomes
as part of its exact source contract before either implementation is released.

## Exact six-module write manifest

| Rust path | Change |
|---|---|
| `src/host/caps.rs` | Export opaque commit/progress/refusal types through existing explicit facade |
| `src/host/caps/song/preparation.rs` | Extract activation helpers/methods; retain original cleanup owner and replacement state |
| `src/host/caps/song/preparation/activation.rs` | New cohesive initial/replacement submission and receipt logic |
| `src/sched/song.rs` | Prepare/submit replacement, expose one-shot commit, observe authenticated old cutoff |
| `src/sched/song/receipts.rs` | Route exact activation outcomes through Ready or already-retiring original owner |
| `tests/song_apply_host.rs` | New genuine ownership/refusal/compound handoff witnesses |

Only this plan may also change. Every Rust file must remain below 1000 lines.
Preparation parent is currently 975; extract the shared receive_activation helper
and Ready activation/initial-command methods before adding state. The existing
sample_data and clock accessor stay with their actual source owner and are
preserved. No imports through inaccessible private modules: caps.rs has an
explicit export list, so its facade update is a necessary sixth path.

## Baseline and source seams

- preparation.rs: currently 975 lines; clock author owns a pending accessor
  change. Capture its exact held SHA immediately before source release, not now.
- caps.rs: 455 lines, `0f4639208a75c850eb3d0a8e2c8345bf3e80579abeff509872d1857308e5334d`.
- sched/song.rs: 439 lines, `b97754ba5f4ff14a33fa5c3cca22789b123c6ce21a12f51938b422762b3b2b92`.
- sched/song/receipts.rs: 50 lines, `78bf41a1ea0fbe49839adc84a6802fc7f214622cbcae626abeabd5051c6da0f2`.

Current Ready retains PreparedSong, physical pools, exact resource keys and the
original cleanup Assembly. retire moves those same values; NeverSubmitted and
RejectedNeverActivated cancel, while PendingExclusive and Applied await guarded
normal returns. Cleanup.receive already handles an activation outcome after
retirement begins. Core begin_retirement moves Ready into that actual cleanup;
its cutoff method independently enqueues Endpoints and must not implement this
compound transaction by sending a second endpoint after Applied.

## Public declarations

```rust
pub enum SongReplacementProgress {
    Priming,
    Submitted,
}
pub struct SongReplacementPreparationRefusal {
    pub replacement: SongReplacement,
    pub source: SongReplacementSource,
    pub overlay: Vec<SongInitialMute>,
    pub failure: Failure,
}
// Opaque old-owner ticket; constructed only from the actual transport.
pub struct SongReplacementSource {
    activation: SongActivation,
    endpoints: SongEndpoints,
}
impl SongReplacementSource {
    pub fn activation(&self) -> SongActivation;
    pub fn endpoints(&self) -> SongEndpoints;
}
// Opaque: fields and constructor are private to the activation owner module.
pub struct SongReplacementCommit {
    replacement: SongReplacement,
    source: SongReplacementSource,
}
pub struct SongReplacementCommitRefusal {
    pub commit: SongReplacementCommit,
    pub failure: Failure,
}
impl SongReplacementCommit {
    pub fn replacement(&self) -> SongReplacement;
    pub fn previous_endpoints(&self) -> SongEndpoints;
    pub fn cutoff(&self) -> SongEndpoints;
}
impl SongReadyBundle {
    pub fn prepare_replacement(
        &mut self,
        replacement: SongReplacement,
        source: SongReplacementSource,
        overlay: Vec<SongInitialMute>,
    ) -> Result<(), SongReplacementPreparationRefusal>;
    pub fn submit_replacement(
        &mut self,
        host: &mut dyn AudioHost,
    ) -> Result<SongReplacementProgress, SongCommandRefusal>;
    pub fn take_replacement_commit(&mut self) -> Option<SongReplacementCommit>;
    pub fn invalidate_replacement(&mut self) -> Result<(), Failure>;
}
impl SongHostPreparation {
    pub fn take_replacement_commit(&mut self) -> Option<SongReplacementCommit>;
    pub fn invalidate_replacement(&mut self) -> Result<(), Failure>;
}
impl SongTransport {
    pub fn replacement_source(&mut self) -> Result<SongReplacementSource, Failure>;
    pub fn prepare_replacement(
        &mut self,
        source: SongReplacementSource,
        overlay_nonce: u64,
        overlay: Vec<SongInitialMute>,
    ) -> Result<(), SongReplacementPreparationRefusal>;
    pub fn submit_replacement(
        &mut self,
        host: &mut dyn AudioHost,
    ) -> Result<SongReplacementProgress, SongCommandRefusal>;
    pub fn take_replacement_commit(&mut self) -> Option<SongReplacementCommit>;
    pub fn invalidate_replacement(&mut self) -> Result<(), Failure>;
    pub fn observe_replacement(
        &mut self,
        commit: SongReplacementCommit,
    ) -> Result<(), SongReplacementCommitRefusal>;
}
```

Existing submit_activation, submit_initial_command, receive_activation, query,
sample access, take_cancelled and take_retired signatures remain unchanged.
SongTransport builds the replacement.activation from its actual epoch/frame;
no separate caller assertion can change its original activation map.

## Private state and extraction declarations

PendingReplacement stores the exact replacement POD, original opaque old-owner ticket,
original overlay Vec, accepted cursor and one-shot commit availability. It is
owned by SongPreparationCleanup, so retiring Ready preserves uncertain admission
and later receipt authority. The stage state distinguishes planned/priming, Replace accepted but arm unknown,
invalidation requested, and matching Applied. The DSP alone owns authoritative
armed state; accepted host submission is not the beginning of activation. It is not an Applied flag inferred from
accepted PrimeMute or a clock report.

The extracted shared helper has the existing signature:

```rust
pub(super) fn receive_activation(
    prepared: &mut PreparedSong,
    state: &mut ActivationState,
    ack: SongHostAck,
) -> Result<(), SongHostAck>;
```

Refine the helper to additionally access retained replacement metadata through
an explicit cleanup reference when implementing, declared before corresponding
source changes. Do not duplicate activation state across Ready and Cleanup.

## Authority, bounded priming and refusal

Before retaining overlay, use cleanup.remaining and the existing max_resources
and max_pending_records limits to admit actual storage and all pool/family/key
inspection work. Do not impose a guessed family profile cap. Reject duplicates,
foreign epoch or nonce, unmatched installed family, stale/non-ready/returned
keys, count mismatch and repeated planning. Actual instrument is the resolved
family stored in physical config; installed event InstId remains a distinct
physical resource identity. Do not reinterpret original frozen selector identity
as a numeric ID or implement semantic remapping in this phase.

Validation is completed before any host command. Local refusal returns the same
original overlay ownership and exact intent. Repeated host submission posts one
retained PrimeMute at a time, advances cursor only after checked admission, then
posts exact Replace. Backpressure returns the same refused POD with cursor and
activation ownership unchanged. The retained overlay cannot change between
retries. Ordinary initial Activate is mutually exclusive with any priming plan.
Prime-only cancellation is still NeverSubmitted cancellation; accepted Replace
sets uncertain activation ownership before any initial Event/Release/Endpoints.
The future DSP cancellation must remove staged overlay records with that epoch.

New events and short first Release use the unchanged authentic initial command
validation only after Replace admission. No musical onset is delayed until
Applied. No success is published while priming or merely Submitted.

## Exact old endpoint and receipt semantics

SongReplacementSource is issued only by the old SongTransport from its actual
Applied activation and original finite endpoint intent. It has private fields,
no public constructor, no Clone/Copy and a once-only issue flag. Issue is valid
for Playing, Draining or genuinely Ended owners, not unacknowledged preparation
or failed activation. The caller cannot submit arbitrary endpoints as authority. A crate-private
issuer in the activation child is called only by replacement_source on the
authentic transport; no public field or unit-struct constructor is exposed.
If planning refuses, the original ticket is returned inside the refusal.

New planning validates the ticket's actual epoch equals replacement.previous,
activation <= A, endpoint ordering and different new epoch. A may exceed the
old tail_deadline: preparing the candidate is not required to finish before the
old song ends. The Runtime must retain the original ticket and semantic mute
identity while the old owner completes its normal cleanup. DSP still validates
actual old-owner identity; a host ticket is not a portable DSP credential.

### Naturally completed old owner

Current engine/song_runtime/reuse.rs removes runtime.active, preparations and
endpoints after the original full-key guarded work completes. The paired DSP
phase must therefore retain one bounded POD completion witness for the current
most-recent-applied owner: exact actual epoch, activation, original endpoint and
observed musical deadline closure. It contains no physical index, lease, graph,
Box or Arc. The most-recent-applied identity is updated only by actual successful
activation, never from a request or new preparing owner. Its completion witness
is installed by actual finish/reaping state, not inferred from a host clock.

Compound preflight authenticates either the still-owned old epoch or that exact
natural-completion witness. A new noncurrent/foreign retired epoch remains
StaleEpoch; there is no unbounded history or guessed retired state. Before a
witness is retired, pending replacement metadata carries the exact captured
activation/endpoint facts in bounded storage. A witness must survive actual old
lease returns and physical slot reuse until the addressed compound transition
finishes. Completion after new actual application cannot overwrite the new
current-owner identity with an older retiring epoch.

If the old natural deadline precedes F, the old music already ended and no old
fade or reset is performed. If it lies between F and A, actual old audio may
close sooner under its existing hard deadline; arm must never prolong that tail.
At exactly A, deadline and replacement jointly close the same authentic old
owner once. New activation still occurs at the original selected A, with staged
overlay and the new 64-frame opening ramp. There is no arbitrary later boundary,
rejection solely because the old tail expired, or nonce-less Activate workaround.
Reset/clear is performed only on authenticated still-owned old full keys; the
completion-witness path cannot access saved indices or reset reused new slots.

The committed cutoff is Endpoints { epoch: previous, arrangement: A,
tail_deadline: A }. Clearing old state at A does not preserve an old tail.
The opaque commit retains original previous Endpoints for exact old-owner
comparison. Only a matching actual Applied at EXACT requested new frame A can
issue it; foreign, late, duplicate, wrong-frame or rejected activation cannot.
Initial Activate's established receipt behavior is preserved separately.

If Ready already entered retirement before Applied, Cleanup retains the same
intent, accepts the exact correlated outcome and supplies the same one-shot
commit. Rejection returns new ownership to genuine cancellation and produces no
old cutoff token. The old scheduler consumes commit only for its exact epoch
and original endpoint intent, removes its unaccepted Event/Release/Rebind at or
after A, stops queries, marks finite endpoint submitted, and starts guarded
normal cleanup without sending another host Endpoints. Engine is responsible
for canceling old already-accepted onsets at or after A atomically.

Full-key safe return receipts arriving before commit observation remain returned
unchanged to the existing bounded dispatcher; after commit enters retirement
retry those original records. Do not swallow them, synthesize returns or end
old ownership before authenticated cutoff. Finite empty songs still retain a
master lease and must await genuine exact returns.

## Pending-document invalidation and arm boundary

DocChanged may invalidate a replacement even after the host accepted Replace,
until the DSP begins the old fade at F = A - 64. Enqueue success is not that
boundary. Current Session's activation_posted guard deliberately needs a later
controller fix; do not retain it as the final document-race policy. This phase
provides invalidate_replacement on Ready, already-retiring cleanup and transport,
retaining the exact intent and a bounded pending CancelPreparation rather than
discarding ownership. Existing ordinary Activate cancellation rules stay intact.

Before arm, actual DSP CancelPreparation must remove the queued replacement and
all primed records, retain exact original ActivationRejected, and complete actual
new-owner cancellation. Old scheduler/PCM remains bit-identical; no commit token
can issue. Owner switches to NeverActivated cancellation only from that genuine
correlated rejection, not from accepted CancelPreparation or elapsed wall time.

After arm, rollback is unavailable: invalidation produces explicit Failed with
normal guarded retirement. Cancel rejection must not be relabeled as the
never-activated ActivationRejected. Actual Applied, if it follows, still gives
the controller the authentic old cutoff commit while the invalidated candidate
is not published as a successful user Apply. The original finite new endpoint
and cleanup authority survive. No direct cancel/free path may reset adopted
leases or abandon old cleanup. The exact after-arm cancellation outcome must be
fixed by the paired DSP phase; generic epoch-only rejection is not an activation
rejection certificate. Any generic command rejection marks failure while keeping
uncertain activation ownership and its genuine endpoint obligations.

Genuine race witnesses must cover Cancel admitted before F but processed before
and after F, exact F ordering, delayed receipt pressure, DocChanged after Replace
acceptance, and invalidation after arm. Owner phase provides retained intent;
Session/controller wiring and user Failed publication are required later phases.

## Tasks

### TASK-001: Cohesive activation extraction and original behavior

**Status**: Ready
**Parallelizable**: No
**Deliverables**: caps.rs, preparation.rs, activation.rs

- [ ] Move original activation methods/shared matcher without changing initial behavior.
- [ ] Preserve physical Ready/cleanup ownership and exact cancellation semantics.
- [ ] Public facade exports opaque commit/refusal/progress; fields stay private.

### TASK-002: Priming, correlated replacement and scheduler handoff

**Status**: Ready
**Parallelizable**: No (depends on TASK-001)
**Deliverables**: activation.rs, sched/song.rs, receipts.rs

- [ ] Charged overlay authority validation and exact retained retries.
- [ ] No initial command before admitted Replace; exact initial onset remains possible.
- [ ] Actual exact Applied issues one opaque commit, including after retirement begins.
- [ ] Old owner observes full intent and cleared cutoff with no independent host command.
- [ ] Rejection preserves old scheduler/music and cancels only original new owner.
- [ ] Invalidation preserves accepted-but-arm-unknown intent, exact cancel retry and normal ownership after arm.

### TASK-003: Genuine host and paired DSP witnesses

**Status**: Ready
**Parallelizable**: No (depends on TASK-002 and positive cases on DSP transaction)
**Deliverables**: tests/song_apply_host.rs

- [ ] Actual Native/ByteInbox queue pressure retains overlay cursor and original POD.
- [ ] Foreign/duplicate/missing overlay locally refuses before host mutation.
- [ ] Initial Activate still works; prime-only real NotReady refusal cannot issue commit.
- [ ] With DSP phase: exact onset A, cleared old endpoint and accepted overlay before onset.
- [ ] Failed pre-arm replacement leaves old output/queue untouched against real twin.
- [ ] Applied after cleanup transition retains one-shot commit and exact normal returns.
- [ ] Old Draining Apply and empty replacement both complete genuine guarded cleanup.
- [ ] Old natural deadline before F, between F/A and exactly A preserves selected boundary and authentic completion.
- [ ] Old slots reused after exact returns are never read/reset by the completion witness.
- [ ] Before/at/after-arm invalidation races prove actual outcomes; no enqueue-as-commit shortcut.
- [ ] Scoped formatting/native/wasm/strict Clippy/focused tests independently pass.

## Progress log

### 2026-10-03: Source-grounded handoff review

Read current Ready methods, cleanup transitions, core cutoff and receipt routing.
Parent approved opaque one-shot actual Applied commit in principle. Explicit
caps.rs export requires six paths rather than the provisional five. No Rust,
Cargo or tests changed. Parent must review and release the baseline after clock
author/checker holds end. DSP transaction, exact cycle selection, frozen semantic
mute remapping and full Runtime/controller atomic replacement remain mandatory.

### 2026-10-03: Expired-tail and old-ticket review correction

Removed the unintended A <= old tail_deadline constraint. Parent approved an
opaque old-owner source ticket plus an actual bounded last-applied completion
witness in the paired DSP phase. Original epoch/activation/endpoint remain exact;
Runtime retains the ticket and original semantic overlay through normal old
cleanup. This is a contract amendment only; no Rust or Cargo changed.

### 2026-10-03: Source release and helper refinement

Parent releases exact six source paths. preparation.rs baseline 979 lines, SHA
`e8b569acc50e5c455463d324e158038c93fb404377877b8eb8c2609e7f5ef681`;
clock floor accessor is retained. Shared receipt helper becomes
`pub(super) fn receive_activation(prepared: &mut PreparedSong, cleanup: &mut SongPreparationCleanup, ack: SongHostAck) -> Result<(), SongHostAck>`
to preserve exact replacement and uncertain invalidation state in one owner.
PendingReplacement retains Option<opaque source>, original overlay, admitted
cursor, submitted/invalidated/cancel_posted and actual applied state. The
crate-private source issuer is called only by actual SongTransport. A bounded
cleanup cancellation retry preserves accepted-but-arm-unknown ownership.
No author Cargo or synthetic successful DSP outcomes are permitted.

### 2026-10-03: Prepared-owner source checkpoint

Implemented all six declared Rust paths. Original activation and clock-floor
methods live in the cohesive activation child. Ready retains the original old
ticket and overlay; checked priming advances only after accepted original POD.
Applied matching is exact for Replace and unchanged for initial Activate.
Uncertain invalidation retries cancellation through original cleanup, and an
actual post-arm failure retains normal ownership; core failure is not overwritten
by a later actual Applied. Opaque source issue requires real Applied and an
already-submitted original endpoint. Old cutoff observation sends no command.

Three genuine fixtures use the existing real candidate/Native/Arena staging
recipe and callback allocation guard. They cover actual initial audio/one ticket,
foreign/duplicate/missing/nonce/count overlay refusals with original pointers,
and genuine finite sender pressure with retained first PrimeMute followed by
prime-only invalidation and actual guarded cancellation. Current Engine still
returns NotReady for PrimeMute; no fake Applied or positive transaction outcome
is supplied. Positive compound DSP/commit/after-arm races remain unverified until
the separate DSP implementation. No author Cargo; scoped rustfmt/check exit 0.

Additional actual private declarations:
`SongReplacementSource::issued(activation: SongActivation, endpoints: SongEndpoints) -> Self`
is crate-private and called only from SongTransport::replacement_source;
`SongPreparationCleanup::submit_replacement_cancel(&mut self, host: &mut dyn AudioHost) -> Result<(), Failure>`
retains refused intent; `PendingReplacement` keeps original source/overlay plus
submitted, invalidated, cancel_posted and actual applied flags.

Focused checker handoff: `cargo test --test song_apply_host`, original
`cargo test --test song_host_preparation`, `cargo test --test song_transport_core`,
and the two compile-fail opaque source/commit doc tests. Native/wasm checks and
strict all-target Clippy remain independent reviewer work. Every process is
reserved to checker; tests are written but not claimed passed by this author.

### 2026-10-03: Genuine handoff checkpoint and proof-preserving refusal

Independent handoff binary: all three real Native/Arena fixtures PASS, original
81209 terminal 0, chunk ba9554, `/tmp/vactr-song-apply-host-initial-001.log`.
This proves baseline/refusal behavior, not successful DSP replacement.
Wrong-owner observe_replacement now returns SongReplacementCommitRefusal with
the exact original opaque commit and Failure. Validation completes before any
local cutoff mutation, so the correct owner can retry without losing one-shot
authority. This is the same manifest's public facade/type/core refinement.
Actual wrong-owner positive-token testing waits for genuine DSP Applied issuance.
