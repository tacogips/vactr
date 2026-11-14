# Atomic Song Apply DSP transaction

**Status**: In Progress
**Created**: 2026-10-03
**Updated**: 2026-10-03

## Design references and related phases

- [Implementation review](../../design-docs/specs/design-song-mode.md#implementation-review-2026-10-03)
- [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)
- [Wire contract](song-mode-atomic-apply-wire.md)
- [Prepared owner handoff](song-mode-atomic-apply-handoff.md)

No Rust writes are authorized by drafting this document. Parent source release
must follow the current clock/checker hold. This plan implements actual compound
DSP behavior; the owner/controller phases retain original candidates, semantic
mute mapping, exact next-cycle selection and requester publication obligations.

## Exact eight-module manifest

| Path | Deliverable | Current lines / SHA256 |
|---|---|---|
| `src/dsp/song.rs` | Preallocated POD transaction/overlay/outcome reservations and timed ordering | 682 / `0d409d01f0a93a956efa7432dd601e866084d50d99fcaef1de823d0623b0f717` |
| `src/dsp/engine.rs` | Replacement frame split and constructor integration if needed | 858 / `6662b1cc2d4eb9e5902ae06177a0c8e2d81cd65cd9436a761144711ced9563d5` |
| `src/dsp/engine/song_queue.rs` | Checked PrimeMute/Replace/cancel intake and pending initial authority | 188 / `68c8dd67654f356d0c68f5ae82efbf688adbfbd608caedcdc551b33e74b205bf` |
| `src/dsp/engine/song_runtime.rs` | Dispatch, master gain and shared activation extraction | 789 / `b4fe2b66be6b351d405a907a2587afcb37d0de2bb6e94b014dd2eee4b9260aff` |
| `src/dsp/engine/song_runtime/replacement.rs` | New cohesive validation/arm/commit/cancellation lifecycle | New |
| `src/dsp/engine/song_runtime/reuse.rs` | Keyed clearing, reservation-aware reaping and completion witness | 573 / `03ee3c4799568c871a511453aabb3c4d180253c1addd1155ee97e0409c022eef` |
| `tests/song_apply_runtime.rs` | Actual Native/Arena audio, receipts, pressure and resource witnesses | New |
| `tests/song_apply_host.rs` | Existing prime-only invalidation compatibility fixture | 580 / `fc37fd632372bfd0af10c1ca335e6efafb42263a0629e99ec69f3b7b42f25311` |

Own plan is the only document write. Each Rust path must stay below 1000 lines.
Extract the existing activate_song_epoch validation/adoption into replacement.rs
before growth; preserve the ordinary wrapper and its established semantics.
No edits to retired render.rs, engine/song.rs, bus, arena, protocol or codec.
New child visibility is within crate::dsp::engine, never a wider public API.

## Existing production seams

SongRuntime::new allocates all active/endpoints/branches/tracks/leases/commands
and receipt tables before callbacks. command_rank and queue own actual sorting;
song_queue::timed_order projects that same rank for generation validation.
Engine::next_song_runtime_frame owns callback frame splitting. Master mixing in
song_runtime.rs already authenticates RuntimeTrack full keys and multiplies
SongGainGate::tail per absolute frame. The independent new epoch gate belongs
there after master FX, not in branch mute gates or legacy output.

Existing activate_song_epoch first validates physical staging but later performs
fallible activation/lookup after changes. This transaction requires complete
preflight plus an infallible bounded commit, not calling that function after old
mutation. Existing apply_song_end immediately ends branches/removes Event work,
so it cannot run at F to implement the fade. It also rejects endpoint extension
and closed old endpoint; compound clear needs a separate authenticated old-owner
path. Existing guarded cancel_song_preparation can be called BEFORE adoption;
protected runtime intake must intercept after-arm cancellation so that helper
never retires live replacement resources prematurely.

## POD declarations and bounded ownership

```rust
pub(crate) enum SongReplacementPhase {
    Queued,
    Armed,
    Applied,
    FailedAfterArm,
    Rejected,
}
pub(crate) struct SongPrimedMute {
    pub(crate) command: SongInitialMute,
}
pub(crate) struct SongAppliedOwner {
    pub(crate) activation: SongActivation,
    pub(crate) endpoint: Option<SongEndpoints>,
    pub(crate) musically_closed: bool,
}
pub(crate) struct SongReplacementRecord {
    pub(crate) command: SongReplacement,
    pub(crate) phase: SongReplacementPhase,
    pub(crate) previous: SongAppliedOwner,
    pub(crate) outcomes: [Option<SongHostAck>; 2],
}
// Add to preallocated SongRuntime:
// primed_mutes: Vec<SongInitialMute>
// replacements: Vec<SongReplacementRecord>
// last_applied_owner: Option<SongAppliedOwner>
```

All record types derive Copy where their real fields permit. They contain no
heap owner, reference, graph, audio buffer or cached physical index. Allocate
primed_mutes with existing branch capacity and replacements with existing
preparation capacity in SongRuntime::new. Records consume actual capacity;
there is no unbounded history. Full tables refuse before old state changes.
No resize, allocation, Box/Arc destruction or ownership transfer occurs in process.

One pending transaction per previous/new epoch pair; no duplicate new activation
or competing old replacement. Primed records are exact new epoch + nonce +
resolved family. Duplicate family is malformed rather than last-write-wins.
Count is exact, including zero. Nonce, family and authentic initial config/full
keys must match; resource ids are not assumed equal to resolved family ids.
Staged graphs/banks/samples must be genuinely Ready, not test-injected flags.

## Internal declarations

```rust
impl Engine {
    pub(in crate::dsp::engine) fn prime_song_replacement(
        &mut self, command: SongInitialMute,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn queue_song_replacement(
        &mut self, command: SongReplacement,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn validate_song_replacement(
        &self, command: SongReplacement,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn arm_song_replacement(
        &mut self, epoch: SnapshotEpoch, frame: u64,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn commit_song_replacements(&mut self, frame: u64);
    pub(in crate::dsp::engine) fn cancel_song_replacement(
        &mut self, epoch: SnapshotEpoch, frame: u64,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn clear_replaced_song_owner(
        &mut self, epoch: SnapshotEpoch, frame: u64,
    );
}
impl SongRuntime {
    pub(crate) fn replacement_gain(&self, epoch: SnapshotEpoch, frame: u64) -> f32;
    pub(crate) fn replacement_next_frame(&self, after: u64, end: u64) -> Option<u64>;
}
```

Refine small reservation/activation helpers before code in this plan if needed.
Do not expose callback internals publicly to make tests pass. Public fixtures
use genuine Stage/Seal/PrimeMute/Replace through NativeRecord or ByteInbox.

## Intake, queue and complete preflight

PrimeMute is immediately staged through protected runtime intake for a Ready
new preparation; it emits no fabricated successful ACK. Invalid records produce
actual ordinary Rejected. Replace intake authenticates distinct old/new epochs,
A >= 64, checked A + 64, F = A - 64 >= actual Engine.frame, all original staged
keys and overlay count, bounded queue/record/outcome/endpoint adoption capacity.
Reject late arm rather than shorten the fade or move A. Accepted initial
Event/Release/Endpoints for new epoch must use actual preparation and frame >= A.
Projected generation/order logic uses the same actual rank, not a second ad hoc
ordering. Failed records retain original reason exactly once through pressure.

Reserve the transaction's actual outcome capacity at intake, before accepting
an arm that requires on-time Applied. At least Applied/rejection plus the first
after-arm cancellation failure outcome must fit reserved bounded storage.
Ordinary receipt producers cannot consume reserved slots. If capacity cannot be
reserved, reject before any old mutation, retaining exact requested activation.
A full physical ACK ring is allowed: reserved internal outcomes preserve on-time
commit. A genuinely full unreserved internal table may cause truthful pre-arm
Capacity rejection, not delayed success. Duplicate failures follow existing
bounded intake guard; no queue enlargement or dropped original reason.

At F repeat complete read-only preflight: new leases/staged banks/sample owners,
branch/template/fx/track/master lookup recipes, exact old identity, current or
natural endpoint, overlay membership, exclusivity, capacity reservations, reset
recipes and all checked frame arithmetic. Before any change all possibly failing
lookups must be proved, including template activation and physical bank views.
After arm, transaction locks reserved adoption slots and keys against foreign
cancel/rebind/stage mutation. Commit rescans exact keys where required; no stale
physical index is treated as authority. A callback cannot race another callback
writer. Illegal commands reject without stealing transaction reservations.

## Arm, exact boundary and gates

At F, adopt and initialize genuinely validated new resources under a closed
new master gate; no new onset can render before A. Apply the staged family mute
state before Event(A), without waiting for critical ACK space. Old master gate
is 1 at F and 0 at A. Do not cancel old onsets in [F,A), reset old FX at F or
run old apply_song_end at F. Existing natural tails/mute gates multiply this
independent epoch master gate; replacement never prolongs an old hard deadline.

At A, before any event/endpoint at that same frame: clear only still-authentic old
voices, private FX, track/master state, cancel old accepted onsets at/after A,
and mark exact old cutoff A/A. Clear owner/users once. New master opens from 0
at A to 1 at A+64; actual new initial Event/short Release then runs at its exact
frame. Applied carries exact requested A, emitted once from reserved outcome
storage. Enqueue/arm alone emits no Applied. Preserve zero-duration endpoint:
compound commit ranks before new Endpoints at A, which can close finite empty
music immediately without missing its real Applied or guarded cleanup.

next_song_runtime_frame includes F, A and A+64 in addition to existing command,
gate and natural deadlines. F/A actions run independently of a full receipt
queue. Master gain is evaluated per absolute sample; callback partition does not
change waveform. Stop using a transaction's opening gate after A+64 but retain
any pending outcome/identity work until its actual handoff completes.

## Before-arm and after-arm cancellation contract

Incoming control intake at exact F is processed before the due arm action; a
CancelPreparation admitted and processed there wins while phase is Queued.
A command processed after arm cannot roll back, even if it was enqueued earlier.
Use actual phase, not host wall time or enqueue time.

Queued cancellation removes exact Replace, all matching primed records and
new pending initial musical commands, retains the original correlated
ActivationRejected { activation: requested, reason: NotReady }, then calls the
existing genuine staged cancellation helper. PreparationCancelled and full-key
returns still come only from authoritative guarded staging. Old PCM/commands
are unchanged, and reserved outcomes cannot be silently freed before reporting.

Armed cancellation does NOT call staged cancel, does NOT emit ActivationRejected
and does NOT mark NeverActivated. It returns an actual Rejected { epoch: new,
reason: HostFault } cancellation outcome, records explicit failed-after-arm intent
and preserves the transaction through boundary clear/normal retirement. Actual
boundary adoption still commits at A and yields its truthful Applied; controller
suppresses user success for the invalidated request and publishes Failed instead.
The authenticated old cutoff commit remains necessary for cleanup. New musical
work is suppressed once invalidated; finite endpoints or exact failure closure
remain available so no adopted owner becomes indefinite. If no genuine new
finite endpoint has yet reached the queue, failure handling reserves an exact
new failure closure at A/A rather than inventing a musical duration. This closes
only the adopted invalidated owner after boundary adoption and cannot mutate
old state before arm. Never fabricate a
successful rollback after the old fade began.

All replacement outcomes use the existing authoritative critical receipt order.
No runtime flush may bypass an older pending song_ack/song_followup outcome;
retain actual original records until FIFO handoff is possible. Repeated cancel
cannot produce duplicate activation outcomes or repeatedly mutate state.

## Natural old completion and safe returns

last_applied_owner records only actual successful activation and authentic
endpoint progression. Before a natural endpoint executes, its original intent
comes from the same-epoch Endpoints already admitted by the authentic timed
queue, not from a caller assertion or guessed duration. Capture this admitted
intent at Replace intake; revalidate any intervening accepted endpoint change
against the retained original identity before arm. It becomes musically_closed only after actual deadline
clear; observing a host clock alone is insufficient. Update it in existing
ordinary Activate and endpoint/finish paths too. The reaper may drop old physical
leases/branches/tracks but preserves the exact current last-applied POD witness.
Old returning work after a newer actual activation cannot replace that witness.

Preflight accepts the exact live old owner OR its most-recent-applied natural
completion witness. This permits A beyond the old tail, including preparation
finishing after old guarded returns. No historical arbitrary epoch is admitted.
Capture exact old activation and original endpoint into the transaction so
natural reaping during [intake,A] cannot lose its authority. At a deadline before
F skip old fade/reset; between F and A allow actual natural silence sooner; at A
close the same old owner once. Never retain physical slots solely for a witness,
never infer old master from its former index, and never reset a reused new slot.

Before A retain old still-live ownership required by its legitimate audio and
reserved new adoption. After A use existing exact-key retirement, actual ACK and
garbage guards; keep staging pumping solely in authoritative controls intake.
Do not add a process-end staging pump or destroy Native graph owners in callback.
Finite empty new arrangements and garbage-full owners must eventually return
only their original exact leases and permit authentic subsequent admission.

## Tasks and completion criteria

### TASK-001: Bounded state, intake and reservations

**Status**: Ready
**Parallelizable**: No
**Deliverables**: dsp/song.rs, song_queue.rs, replacement.rs

- [ ] Exact bounded priming/transaction state, actual ordering and full-key intake.
- [ ] Reserved on-time outcomes; genuine capacity refusal before mutation.
- [ ] Pending initial commands authenticated; original reasons retained exactly once.

### TASK-002: Transaction, fades, cancellation and reaping

**Status**: Ready
**Parallelizable**: No (depends on TASK-001)
**Deliverables**: replacement.rs, song_runtime.rs, reuse.rs, engine.rs

- [ ] Complete preflight precedes adoption/old fade; commit cannot partially fail.
- [ ] Actual 64-frame gates, exact A first onset and old clear with no leaked tail.
- [ ] Cancellation before/at/after F implements declared correlated outcomes.
- [ ] Actual natural completion witness and no reused-slot reset.
- [ ] Full ACK/garbage pressure does not delay commit or permit premature reuse.

### TASK-003: Real Native/Arena audio and ownership evidence

**Status**: Ready
**Parallelizable**: No (depends on TASK-002 and owner phase for paired host tests)
**Deliverables**: tests/song_apply_runtime.rs

- [ ] Real positive oscillator/PCM twins prove failed preflight/cancel leaves old output bit-identical.
- [ ] Successful old closing/new opening 64-frame PCM, exact boundary first onset and no old FX tail.
- [ ] Both backends; varied callback partitions preserve the same exact output/receipts.
- [ ] Full physical ACK and real internal receipt pressure; exact once/FIFO outcomes.
- [ ] Before F, exact F and after F cancellation with accepted-but-not-armed DocChanged race.
- [ ] Natural old deadline before F, between F/A and exactly A; original A retained.
- [ ] Genuine old slot return/reuse before A does not read/reset foreign/new keys.
- [ ] Empty/subframe endpoint, full garbage retention and eventual exact new admission.
- [ ] Callback allocator guard observes zero alloc/dealloc during actual Engine.process.
- [ ] Native/wasm/strict Clippy/focused host+runtime tests independently pass.

## Progress log

### 2026-10-03: Source-grounded transaction declarations

Audited actual SongRuntime allocations, timed rank/queue, projected admission,
activation validation/adoption, per-frame post-master rendering, protected cancel,
finish deadlines and guarded reaper. Seven source paths suffice for this phase;
no Engine staging helper edit or retired render/bus write is required. Existing
activation logic is cohesively extracted before growth. No Rust or Cargo changed.
The paired owner plan's after-arm contract now has explicit outcomes; full
Runtime document invalidation, semantic overlay remapping, boundary selection
and public/browser replacement tests remain mandatory downstream work.

### Implementation checkpoint: 2026-10-03

Parent released the seven-path DSP implementation after actual handoff three,
profile one and opaque compile-fail two checks passed. The accepted late-live-Mute
baseline changes song.rs to 748 lines; preserve its original-POD/max(now) behavior.
Replacement records retain two inline optional outcomes in preallocated storage.
Actual boundary commit is independent of physical acknowledgment pressure; delivery
uses the existing FIFO after older pending critical or cached command failures.
These cells are bounded by preparation capacity, admitted at intake, and never
allocate in the callback. Memory author owns only Engine::allocate constructor
patches; transaction author owns runtime initialization/frame splitting.

### First coherent DSP source checkpoint

Activation lookup and physical readiness checks moved together into replacement.rs.
The ordinary past-finite-end refusal remains intact. Transaction intake retains
original requested A and previous actual activation/endpoint; at F it revalidates
the previous owner and every new resource before adoption. Absolute master gates
use F/A/A+64; old work at or after A is removed, and exact full-key deadline cleanup
clears old resources. Queued cancellation retains original ActivationRejected;
after-arm cancellation retains HostFault plus genuine Applied at A and a finite
failure endpoint if no endpoint was admitted. Observed natural completion retains
only the latest immutable activation/endpoint POD; no physical index is retained.

Written genuine Native/Arena fixtures (not executed by author):
- real_replacement_commits_at_exact_frame_and_primed_first_onset_is_silent
- genuine_prearm_cancellation_preserves_old_pcm_and_original_rejection_frame
- genuine_afterarm_invalidation_keeps_boundary_commit_and_finite_cleanup
- observed_natural_retirement_allows_exact_replacement_and_64_frame_new_opening

Each uses actual candidate/preparation/Ready, original host submission, actual
receipts and callback allocation/deallocation probes. Twin references compare
old and new master gains with strict sample tolerances; no success ACK is forged.
This is an initial compiler/fixture checkpoint, not TASK or phase completion.
Still required: independent compile/Clippy/runtime results; explicit critical ACK
and garbage pressure/FIFO witnesses, failed validation twins, partition variation,
natural deadlines between F/A and exactly A, reused physical slots, wrong-owner
proof retry after a genuine Applied, and controller/document race integration.
No Cargo or nextest executed by author. Parent coordinates the joined fixed-source
check with the memory layout author.

### First checker regression and fixture-only repair

Joined original1452 terminated101; compilation succeeded, handoff tests two passed
and one failed before the new DSP/layout binaries ran. Raw log:
`/tmp/vactr-atomic-layout-checkpoint-001.log`
(SHA256 a52a8adbed58d8c61d4f8af78d00aeda8ca780c9772a268791c025cdec52b817).
Parent authorized the eighth path before writing: the existing prime-only
invalidation fixture incorrectly required unsupported PrimeMute/NotReady.
Valid admitted priming now has no success receipt and no runtime rejection.
Retain actual twice-refused original POD, no Replace/Applied/commit, original
cleanup and unchanged old endpoint/state checks. Production remains held.
Pressure outcome ordering and endpoint reservation gaps remain explicit; arm
adopts the validated new physical owner under a closed gate at F, while actual
public commit/Applied is A. No phase-completion or new runtime pass is claimed.

### DSP fixture causality repair

Joined original73379 terminated101: handoff three passed; DSP one passed and
three failed; layout was then checked separately (public four/private one passed).
Raw DSP log: `/tmp/vactr-atomic-layout-checkpoint-002.log`.
The muted positive submitted only the first Prime rather than reaching Submitted.
The cleanup helper sent old-return receipts to the new owner. The natural-opening
Arena twin queued both hosts into the process-global ABI outbox before capture,
so an ordinary Activate could interfere with the target replacement.
Fixture-only repair drives exactly overlay_count priming records plus Replace
without advancing the callback, dispatches old receipts to the original old
Transport, and captures each backend's actual outbox before switching hosts.
Keep all frame, gain, silence, provenance, cleanup and unhandled-receipt checks.
Production source remains unchanged and held; no author Cargo executed.
