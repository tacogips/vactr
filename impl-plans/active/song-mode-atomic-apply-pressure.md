# Atomic Song Apply pressure and lifecycle

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Apply review](../../design-docs/specs/design-song-mode.md#implementation-review-2026-10-03)

## Scope and dependencies

Complete the pending pressure/lifecycle obligations of
[Atomic Apply DSP](song-mode-atomic-apply-dsp.md), after its independently accepted
four genuine Native/Arena tests. Preserve the existing
[owner handoff](song-mode-atomic-apply-handoff.md) and
[wire contract](song-mode-atomic-apply-wire.md).
Controller publication and original ticket return after rejected/cancelled Apply
remain separate required work; this plan does not narrow the whole-code goal.
No additional protocol, public authority or arbitrary quota is introduced.

Original6247 terminated0: actual runtime four passed; raw
`/tmp/vactr-atomic-runtime-repaired-001.log`, SHA256
`dadf7d96ac065c4e215f0317483c2405fb04b7748ac7245e7802ab784f35edc4`.
Pure WASM build20596 also terminated0. These prove the first source checkpoint,
not pressure/FIFO, reservations, partitions or full atomic Apply.

## Exact eight-path source manifest

| Rust path | Current lines / SHA256 | Status |
|---|---|---|
| `src/dsp/song.rs` | 805 / `ab9f1b8aadc15ea64cbd45a2a3cdf15142cef8ad98927990c44c762763b6c1f2` | In Progress |
| `src/dsp/engine.rs` | 871 / `f5f684a9086d637d59edd68239ba58e59b187f6f808bb4e5d78283d653a80132` | In Progress |
| `src/dsp/engine/song_queue.rs` | 244 / `f31cae7e83b50b4bc1d05bf3b13822269232983e5686cabc34cc0ef1aefbc13e` | In Progress |
| `src/dsp/engine/song_runtime.rs` | 670 / `ba4462581c9dd05183ab904dc93caf39944104916d1a611abe239300ae43e8ac` | In Progress |
| `src/dsp/engine/song_runtime/replacement.rs` | 501 / `a2c1b843944a4876a6c022029c2cb2a2be953d3425997f34150a9f14fe570d55` | In Progress |
| `src/dsp/engine/song_runtime/reuse.rs` | 590 / `2d78ab1b873dca25779215e0f941ace2f2365b66ee69f2ee42a1710d4badb0ff` | In Progress |
| `tests/song_apply_runtime.rs` | 786 / `cc11f53291f2d2a78ea5f5ab227a3548eac9156410bfc14e327ebbaa6c0940bd` | In Progress |
| `tests/song_apply_runtime/pressure.rs` | New | In Progress |

Own plan is the only document write. Every touched Rust file stays below1000.
The child shares the actual parent Rig/candidate/Ready/Transport helpers and
allocator probe; no copied fake Rig, synthetic ACK or public test-only API.
Fresh source release from parent is required before Rust changes. Until then,
all original DSP/handoff/profile compilation inputs remain held.

## Typed deliverables

### Bounded outcomes and endpoint ownership

```rust
pub(crate) struct SongReplacementOutcome {
    pub(crate) acknowledgment: SongHostAck,
    pub(crate) frame: u64,
    pub(crate) before_boundary_commit: bool,
}
// SongReplacementRecord::outcomes becomes:
// [Option<SongReplacementOutcome>; 2]
// SongRuntime gains a preparation-capacity preallocated table:
// endpoint_reservations: Vec<SnapshotEpoch>
impl SongRuntime {
    pub(crate) fn reserve_endpoint(&mut self, epoch: SnapshotEpoch)
        -> Result<(), SongRejectCode>;
    // Caller authenticates existing or reserved ownership before mutation.
    pub(crate) fn commit_endpoint(&mut self, endpoint: SongEpochEnd);
    pub(crate) fn release_endpoint_reservation(&mut self, epoch: SnapshotEpoch);
}
```

Chronological delivery is based on actual outcome frame, with cancellation
processed before boundary adoption at an equal frame. A HostFault accepted
between F and A must precede Applied(A), including when both remain retained
through A under full critical ACK pressure. Outcome cells are separately bounded
and allocated before callback. They join the original FIFO after older buffered
receipts, Engine song_ack/song_followup, and older due command failures. Never
recompute a requested frame from a delayed receipt or deliver a fake Applied.
No timestamp/priority describes an event which did not actually occur.

Endpoint reservations are unique full epochs, including empty owners. Account
existing endpoints plus reservations against the original endpoint capacity
before any push. Ordinary admitted Activate/Endpoints and replacement old/new
closure obligations share this one budget. Existing materialized endpoints need
no second debit. Installing a reserved endpoint transfers its debit into actual
storage. Admission refusal retains original command/state; failed queue insertion
must release only a newly acquired reservation. Pre-arm cancellation releases
only the cancelled new obligation; old accepted endpoints remain unchanged.
Returned owners release residual reservation metadata through authoritative reap.
No physical slot, Arc or Box is retained in a reservation or completion witness.

### Lifecycle/dispatch signatures

```rust
impl Engine {
    pub(in crate::dsp::engine) fn queue_song_replacement(
        &mut self, command: SongReplacement,
    ) -> Result<(), SongRejectCode>;
    pub(in crate::dsp::engine) fn advance_song_replacements(&mut self, frame: u64);
    pub(in crate::dsp::engine) fn cancel_song_replacement(
        &mut self, epoch: SnapshotEpoch,
    ) -> Result<(), SongRejectCode>;
}
```

Intake reserves bounded record/outcome and endpoint obligations. At F, all actual
new resource/adoption capacities and old identity/reset rights are revalidated
read-only before the first old gain change. New physical adoption at F is under
a closed master gate; actual public commit and Applied remain at A. After arm,
no capacity check, Vec growth or fallible late lookup may undermine A. Endpoint
installation at A uses the already admitted unique reservation; ordinary due
work cannot steal it. Cancellation after arm retains explicit failure plus real
commit/finite cleanup, while cancellation before or exactly F leaves old PCM,
queues and endpoints unchanged. Completion history authenticates original
activation/endpoint and actual musical close; elapsed clock alone is not proof.

### Genuine shared fixture seam

```rust
// Test-only child module under the existing integration-test binary.
fn render_block(rig: &mut Rig, frames: usize, drain: bool) -> Vec<f32>;
```

Output allocation is outside the allocator probe. The helper uses actual
Native AudioSide or Arena ByteInbox/Engine and advances exactly rendered frames;
all chunks fit real configured max_block. `drain=false` retains actual critical
receipts/returned owners without fabricating queue contents. Pressure markers
are Engine-produced RequestCapacity reports, not invented cancellation outcomes.
Original ABI capture separates Arena twins. Resource pressure must retain actual
original owners; any deliberately filled garbage carrier is described as physical
pressure, not successful staging or completion evidence.

## Tasks and dependency status

| Task | Deliverables | Depends on | Status | Parallelizable |
|---|---|---|---|---|
| TASK-001 | Chronological retained outcomes and debited endpoint/runtime admission | First DSP four | In Progress | No |
| TASK-002 | Genuine pressure/failure/partition/native deadline/slot-reuse fixtures | TASK-001 | In Progress | No |
| TASK-003 | Scoped formatting, immutable source hold and independent verification | TASK-002 | Completed for current checkpoint | No |

### TASK-001 completion

- [x] Earlier HostFault precedes actual Applied under full ACK pressure.
- [ ] Older critical and cached command failures retain FIFO precedence.
- [x] Ordinary and compound endpoint obligations use original shared capacity.
- [ ] Exact/one-short admission is decided before F with original POD preserved.
- [x] F validation precedes old fade; A cannot grow vectors or fail adoption.
- [x] Pre-arm and after-arm cancellation preserve their distinct real outcomes.
- [x] No callback allocation, deallocation, VM, quota increase or sidecar authority.

### TASK-002 completion

- [x] Actual Native/Arena ACK ring and internal pressure spans F/cancel/A; drain
  reveals exact chronological outcomes once and original full-key guarded returns.
- [ ] Invalid actual previous/new validation refuses before fade; old PCM remains
  bit-identical to unchanged twin and no Applied/commit is emitted.
- [x] The same admitted transcript rendered as16-frame chunks and reordered
  legal small chunks has exact sample/frame outcomes and master gains.
- [x] Real natural old deadlines before F, between F/A and exactly A preserve
  completion timing and do not prolong or double-reset old state.
- [x] Actual returned lease slots are physically reused by new admitted Ready;
  old completion/deadline access cannot fault or reset new audio.
- [x] Existing first four Native/Arena fixtures retain every strict assertion.
- [x] Callback allocator probes demonstrate zero allocations and deallocations
  for actual supported success, pressure, refusal, cancellation and cleanup.

### TASK-003 completion

- [x] All eight Rust paths below1000 and scoped rustfmt check passes.
- [x] Fixed-source source/plan hashes and actual fixture inventory delivered.
- [x] Independent native/WASM compilation and strict Clippy pass.
- [x] Whole original DSP/handoff and new pressure fixture binaries pass.
- [x] Actual result/logs recorded without claiming controller/full-goal completion.

## Verification handoff

Author runs scoped formatting only. Checker owns quietly mise-managed Cargo:
`CARGO_TERM_QUIET=true mise exec -- cargo test --test song_apply_runtime` and
`--test song_apply_host`, plus native/WASM checks and strict all-target Clippy
against the joined held profile/DSP source. No nextest or author Cargo.

## Progress log

### 2026-10-03: Source-grounded pressure/lifecycle preparation

Current outcomes use semantic slots0/1, so retained cancellation can be flushed
after Applied even when cancellation occurred first. Existing endpoint checks
only observe current Vec length; reserved failure/old closure obligations are
not yet debited against intervening due work. This plan directly owns these
production seams, preserves the accepted first four tests, and adds a shared
fixture child to avoid growing the786-line parent past its mandatory limit.
No Rust changes or new readiness/playback claim were made in this doc wave.

### Source release and declaration refinement

Parent authorized these seven original paths after profile/native/browser checkpoints
terminated. Endpoint commit is infallible within its authenticated existing or
reserved epoch obligation; admission and F checks establish the debit before
mutation. Ordinary endpoint application checks ownership before ending branches.
A consumes an existing debit without a new capacity decision. Reap/cancel may
release only obligations which cannot reach Armed/A. Outcome ordering retains
actual occurrence frame and whether cancellation preceded boundary commit.

### Actual pressure marker refinement

Native ACK_CAPACITY is8192 while its public capacity-origin ledger is1024.
Read-only RequestClock commands bypass that ledger and produce genuine Engine
ClockReport records, so Native pressure uses those actual reports. Arena pressure
uses actual RequestCapacity reports and observes the real128-slot producer length.
Neither path invents completion/cancellation markers. A forthcoming narrow
Engine::controls companion is required to stop later control receipts bypassing
a retained earlier cancellation outcome; parent manifest amendment requested
before any Engine write.

### Parent-approved eighth-path FIFO amendment

Before any Engine edit, parent explicitly released the narrow controls seam:
flush earlier retained runtime receipts/outcomes after existing song_ack/followup
and before staging/control intake. Retained actual outcomes gate later control
intake, including immediately after a control record produces one. Rendering and
F/A progression continue independently. Preserve the shared memory constructor
and existing next-frame integration. This closes later immediate control ACK
overtaking which sorting inline outcome cells alone cannot prevent.

### Coherent source checkpoint (verification pending)

Implemented bounded chronological inline outcomes, shared unique endpoint debits,
controls FIFO gating, and active-owner retention while inline outcomes remain.
Added five genuine Native/Arena fixtures in the pressure child: physical ACK
pressure with actual ClockReport/CapacityReport markers, invalid actual new lease
with twin PCM comparison, nonaligned callback partitions, old natural deadlines
before F/between F and A/at A, and returned-key/capacity-forced physical slot reuse.
Original four fixture assertions remain intact; parent helper now retains original
Ready keys/families before consuming ownership. New pressure cleanup consumes all
actual delayed LeaseReturned records. Every callback remains allocation/deallocation
probed. The private endpoint debit test is a budget-table witness, separate from
the genuine host/Engine fixtures. No author Cargo or behavioral pass claim.
Full controller, ticket recovery, and independent pressure acceptance remain pending.

The full-ring transcript also submits a genuine RequestClock immediately after
CancelPreparation and requires that unique report after HostFault, timed Muted,
and Applied. This directly exercises the eighth-path control intake gate.

### First independent compile checkpoint

Original89580 terminated101 before any tests: E0583 at the parent test module
registration resolved tests/pressure.rs instead of the declared child. Added
explicit #[path = "song_apply_runtime/pressure.rs"] registration only; all
production and fixture assertions remain unchanged. Raw failure retained at
`/tmp/vactr-pressure-recovery-checkpoint-001.log`. Behavioral acceptance pending.

### Second independent compile checkpoint

Retry002 foreground81310a terminated101 before behavioral tests: new ClockReport
marker assertions used nonexistent direct epoch and compared its full request to
a nonce. Corrected to request.epoch and exact SongClockRequest equality. Actual
marker count/FIFO assertions and every production body remain unchanged. Full raw
`/tmp/vactr-pressure-recovery-checkpoint-002.log` preserved.

### Third independent checkpoint and source-grounded repair

Original94214 terminated101: host8 passed; runtime7 passed/2 failed. Full raw
`/tmp/vactr-pressure-recovery-checkpoint-003.log` retained. Natural opening
regressed because F unconditionally required an old endpoint reservation although
intake correctly omits it after genuine observed natural reap. F now requires
that debit only while runtime still owns the old epoch; exact actual previous
activation/endpoint and musically_closed witness validation remains mandatory.
The later clock marker was behind actual one-key-per-quantum guarded returns; the
fixed16 fixture quanta ended at19 of20 returned keys. Its pump ceiling now derives
from original retained old/new resource counts plus three outcomes, automatic
capacity retry and exact marker, stopping only when that genuine marker arrives.
No quota or onset changed; exact FIFO marker and original64-frame opening
assertions remain unchanged. Behavioral retry pending; no author Cargo.

### Fourth independent checkpoint and exact unclaimed receipt

Original73756 terminated101: host8 passed/runtime8 passed1 failed. Original
natural64-frame opening and exact later marker ordering now passed. The FIFO
probe submits Mute directly to AudioHost, so Transport correctly returns the
actual unregistered Muted receipt. Fixture dispatcher now retains it and asserts
exact single original SongMute equality instead of blind unwrap. No production
changes or receipt/order/audio assertion weakening. Raw
`/tmp/vactr-pressure-recovery-checkpoint-004.log` preserved; retry pending.

### Fifth independent checkpoint and actual byte ingress fill

Original45099 terminated101: host8/runtime8 passed; Native full pressure advanced
through all repaired assertions, Arena observed32/128 real ACKs before fill.
ByteInbox INBOX_SLOTS is16, so two callbacks admitted32 of128 accepted outbox
records. Arena now pumps without draining until measured producer length equals
capacity, bounded by ceil(actual queued request count/INBOX_SLOTS); cancellation
still explicitly precedes F+20. No reduced pressure capacity or fabricated ACK.
Production unchanged. Raw `/tmp/vactr-pressure-recovery-checkpoint-005.log`
preserved; formatting/check only, behavioral acceptance pending.

### Independently verified pressure checkpoint; plan remains In Progress

Evidence reviewed: focused68596 exit0 host8/runtime9 (17 distinct);
endpoint90595 exit0 exact budget-table1; transport3455 exit0 whole4; total22
distinct tests. Native47225 exit0; pureWASM67317 exit0 artifact SHA256
`f0057a18deeaf7a5b8beff2181be72e3a503eddf324984a23f8be209324ca0de`.
Strict Clippy68476 exit0 after parent-declared unrelated runtime range repair;
scoped15 Rust format0/all below1000 and18 joined before/after hashes exact.
Parent actual fresh browser initial5 passed original52999 exit0 against that
artifact; browser log SHA256
`9fee3fb9b91dbfa137657a25d20baa6b122c486cae222d8abfc612ad7aed7dc1`.

Verified logs include `/tmp/vactr-pressure-recovery-checkpoint-006.log`,
`/tmp/vactr-pressure-endpoint-private-001.log`, and
`/tmp/vactr-pressure-transport-regression-001.log`. Initial strict failure raw
`/tmp/vactr-pressure-recovery-clippy-001.log` remains retained; its failed result
is not relabeled a success. Parent/checker terminal evidence supplies subsequent
strict68476 success. Source-held receipt0006 remains unchanged.

Criterion audit: chronological HostFault/Muted/Applied/later immediate ClockReport
and genuine full-ring guarded cleanup pass on both backends. Original4 plus
new5 preserve exact frame/audio and callback allocation/deallocation probes.
New invalid staged lease twin proves **new** validation rejection bit-identical;
old natural deadlines and returned-key/capacity-forced physical reuse pass.
Endpoint storage is shared/debited and its exact/one-short table is verified; A
uses existing debit without capacity-changing adoption, corroborated by pressure
commit and normal-stack compile/source review.

Remaining specific evidence gaps, so do not archive or mark all tasks complete:

- Older **cached failed timed command** preceding inline outcomes under full
  pressure is source-covered by FIFO/earlier-command guard but has no dedicated
  actual failed-command pressure transcript among these22 tests.
- Exact/one-short endpoint **Engine ingress before F**, preserving original
  refused POD and old PCM, has only the private reservation table proof plus
  successful actual owner paths; the requested full ingress refusal witness is
  not yet written.
- Invalid **previous** ownership/physical validation twin PCM refusal is not
  separately witnessed by the new-invalid-lease fixture. Existing natural old
  ownership success and host foreign-source refusals do not replace that proof.

Controller boundary/overlay/document-race integration remains required separately.
No Rust/Cargo changes in this documentation review.

### Remaining-witness declarations before source

Existing eight-module manifest remains unchanged. Parent fixture helper adds
`start_with_event_authority(&mut Rig,u64) -> (SongTransport,Vec<SongLeaseKey>,Vec<u32>,SongAudioEvent)`
by copying an actual initial pool event before consuming Ready. Existing helpers
keep their signatures and original assertions. Child shares a real-ring fill
helper and adds `cached_failed_event_precedes_boundary_outcome_under_real_pressure`
using actual retained old branch keys with deliberately invalid installed InstId,
and `stale_issued_previous_owner_refuses_without_changing_current_pcm` using
a genuine issued old ticket whose original keys have returned, then a newly
Applied live old owner and original isolated staged candidate. No fabricated
origin/certificate or private capacity mutation. Endpoint ingress saturation
requires parent clarification: endpoint capacity equals preparations capacity
and a new debit requires a distinct genuinely ready preparation, so an extra
owner cannot reach debit before genuine staging refuses. Do not label the
private table test a supported public ingress proof.

### Approved coupled-budget correction before production writes

Parent identified the missing inactive-cancellation path: genuine cancelled
ordinary queued Activate/Endpoints survive PreparationCancelled, retaining their
debit despite a freed preparation slot. Prior invariant statement omitted this
case. Same eight-module manifest now owns narrow song_queue interception and
replacement no-record cancellation cleanup. Original queued activation generates
one actual ActivationRejected(originalA,NotReady); inactive epoch work/debit/prime
metadata clears only after real cancellation succeeds. Active owner is untouched.

Parent capacity criterion refinement: prove endpoint-owner subset of genuine
admitted preparations after this fix; genuine staging exact/one-short refusal
plus defensive private debit-table refusal replaces deliberately preserving the
leaked-debit bug. Parent fixture
`inactive_ordinary_cancel_restores_exact_coupled_capacity_and_real_replacement`
uses actual Ready/full keys for initial cancelled owner, then real Engine
Begin/Seal/queued Activate across the configured preparation ceiling, actual
extra Begin refusal, real cancellations, new actual candidate replacement and
old twin PCM. No private capacity editing. Parent calls child render/drain through
private pub(super) fixture helpers; no production API widening.

Fixture organization refinement: share cohesive real cancelled-Ready twin setup
through `pressure::cancel_ordinary_ready_twins(&mut Rig,&mut SongTransport,
&mut Rig,&mut SongTransport)->SongActivation` in the existing declared child.
This preserves every full-key/receipt/PCM assertion and keeps both parents below
1000 without a new module or compressing statements.

### Remaining-witness coherent checkpoint (verification pending)

Added three genuine tests across the existing parent/child; old9 strict fixtures
retain assertions. Cached bad installed instrument Event remains queued with its
real failure while the two actual runtime receipt slots are full, then must
precede Applied. Stale previous ticket is genuinely issued before natural full
key returns, then rejected after another actual owner has Applied, with current
twin PCM unchanged. Coupled capacity test cancels real pending Ready/full-key
resources, fills real preparation/debit ceiling with actual Begin/Seal/Activate,
checks extra original Begin Capacity refusal and twin PCM, cancels all genuine
queued epochs, admits a real replacement, and renders beyond cancelled activation
frames with new audio and no obsolete Applied. Correlated requested-frame
rejection and inactive debit/work clearing implemented in approved queue/no-record
replacement cancellation seams. No author Cargo or behavioral success claim.
