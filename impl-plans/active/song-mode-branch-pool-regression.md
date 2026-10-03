# Song branch pool regression implementation plan

**Plan ID**: SONG-BRANCH-POOL-REGRESSION
**Status**: In Progress
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose

Prove the remaining audio-side pool requirements needed by a finite scheduler:
independent positive tails, distinct immutable graph pools across A/B/A, exact
generation and final-retirement refusal, and all ordinary/reusable private lease
alias directions. Existing one-slot verification does not prove these cases.

## Related plans

- Depends on `song-mode-branch-reuse.md` and `song-mode-reuse-queue-admission.md`.
- ROOT0400 accepts the existing nine public reuse and eight private reuse/queue
  tests, with complete old regressions; it does not clear the remaining cases.
- Feeds later host preparation and finite transport. Scheduler query-window
  integration remains a separate unchecked requirement.
- Disjoint from the active `song-mode-graph-resource-closure.md` source wave.

## Exact write manifest

| Path | Current size | Deliverable |
|---|---:|---|
| `tests/song_branch_reuse.rs` | 823 | Preserve existing nine test names and assertions; add pool regressions |
| new `tests/song_branch_reuse/rig.rs` | absent | Cohesive existing Rig extraction and genuine multiple-pool setup |

Only those two Rust paths and this plan are writable. Core production and the
core reuse plan remain read-only during this increment. Extract the existing
Rig before growing the parent; keep all files below 1000 lines. Preserve the
thread-local callback allocator probe, exact owner/pointer comparisons and real
Native/Arena transports. Additional paths or production corrections require a
prior companion manifest and source release.

## Fixture interfaces

The child uses existing engine, POD and transport interfaces. Rig remains private
to the test binary; no new library API is prescribed. Existing fixture signatures
remain compatible. Intended additional test declarations:

```rust
fn distinct_private_slots_preserve_independent_positive_tails();
fn positive_tail_overlap_and_safe_rebind_preserve_other_stage_audio();
fn immutable_graph_pools_support_a_b_a_without_old_tail_leak();
fn all_private_alias_directions_refuse_before_mutation();
fn stale_foreign_exhausted_and_final_epoch_transitions_never_reopen();
```

## Required evidence

- Run every new case using real NativeRecord and actual encoded ByteInbox Arena
  records. Begin/reserve/adopt/seal/activate and returned receipts are genuine.
- A and B have separate admitted instrument and private FX full leases. Shared
  read-only track/master definitions are permitted. No overwrite of one immutable
  graph in place or invented readiness is allowed.
- Positive tails must be audible after the old source ends. Compare an actual
  independent reference render to prove the surviving branch and track/master
  contribution remain unchanged while another branch is reset.
- Premature same-slot reuse refuses before mutation. Keep generation, measured
  physical layout/capacity and actual subsequent PCM unchanged. After its reached
  deadline, actual BranchRebound accompanies the same physical slot reset; the
  other pool's tail continues and old history cannot leak into the new generation.
- A/B/A advances only A's generation on the final A birth; B retains its admitted
  graph and tail ownership. Establish nonzero signal references rather than a
  silence-only or all-muted success oracle.
- Ordinary/ordinary, ordinary/reusable and reusable/ordinary full PrivateFx alias
  attempts each refuse before adoption/config mutation. Retain the accepted
  physical nonzero-users-without-selected-voice witness in existing private tests.
- Actual stale/foreign event and release, skipped/wrapped/exhausted rebind and
  final endpoint reopening cases check refusal and no resulting new audio. A
  post-final proof must use actual final cleanup/receipts, not merely enqueue.
- Resource return uses exact native payload and Arc addresses or actual Arena
  capacity restoration. Pressure and final retirement must not permit early ID
  reuse or callback destruction.
- Retain block partition invariance and zero callback allocations/deallocations
  for overlapping pools, with setup, encoding and assertions outside the probe.
- Existing nine tests retain their names and semantic assertions. No weakening
  of earlier expectations, blanket dead-code allowance or fabricated ACKs.

## Tasks

| Task | Deliverable | Dependency | Status |
|---|---|---|---|
| TASK-001 | Preserve and extract existing Rig | Accepted source | Written; body hash preserved, execution pending |
| TASK-002 | Genuine distinct-pool and refusal witnesses | TASK-001 | Written; execution pending |
| TASK-003 | Independent joined verification | TASK-002 and closure held | Not Started |

TASK-001 and TASK-002 are sequential. Their exact test paths are disjoint from
the closure author, so source authoring may proceed concurrently. No Cargo until
all source writers are held and root releases an immutable joined cohort.

## Completion criteria

- [ ] Existing Rig extraction preserves its accepted behavior.
- [ ] Every specified pool/tail/A-B-A/alias/generation/final-retirement case has a
  nonempty actual Native/Arena test and required audio/ownership assertions.
- [ ] Existing nine reuse tests and related one-shot regressions still pass.
- [ ] Actual new tests pass independently with exact inventory/run names.
- [ ] Native/browser builds, strict Clippy, scoped formatting and line bounds pass.
- [ ] Final source hashes remain fixed throughout independent verification.
- [ ] Core reuse plan is reconciled later only for proven criteria; scheduler
  and preparation consumption remain explicitly unfinished.

## Progress log

### Session: 2026-10-02

Root inspected accepted test fixture size and the remaining core plan checkboxes.
This prior companion owns the fixture split and broader witnesses explicitly.
ROOT0400 scoped success is retained; no broader pool completion is inferred.

### Exact private additional helper declarations before source
```rust
impl Rig {
    pub(super) fn prepared_pools(bytes: bool, private_gains: [f32; 2], reusable: [bool; 2]) -> Self;
    pub(super) fn prepared_alias_probe(bytes: bool, reusable: bool) -> (Self, SongBranchConfig);
}
fn pool_event(branch: u32, generation: u32, frame: u64, feedback: f32) -> SongCommand;
fn pool_release(branch: u32, generation: u32, frame: u64, deadline: u64) -> SongCommand;
fn pool_rebind(branch: u32, expected: u32, frame: u64) -> SongCommand;
fn finish_pool(rig: &mut Rig);
fn assert_actual_rejection(rig: &Rig, reason: SongRejectCode);
```
Child extraction occurs before parent additions; all old test bodies and allocator
remain byte-identical. Additional setup shares actual PCM/graphs/transport; private
A/B graphs differ in dry chain and full immutable keys, shared track/master unchanged.
Only new regression plan and exact two test paths writable, no Cargo.

The retained prepared API preserves arbitrary supplied track/master gains through:
```rust
impl Rig {
    fn prepared_graphs(bytes: bool, private_gains: [f32;2], track_gain: f32, master_gain: f32, reusable: [bool;2]) -> Self;
}
```
Both additional public setup wrappers call this same real ingress, no guessed state.

Rig adds pub(super) initial_capacity: SongHostCapacities measured from the same
configured Engine before any resource reservation. finish_pool compares all actual
free capacity fields to that value after exact final ownership/receipt handoff.
This is actual provider measurement, not a fabricated nominal capacity.

### ROOT0404 source-only ready checkpoint
Exact original Rig extraction preceded growth; original nine-test/helper tail SHA
remains equal to0001 immutable receipt after scoped formatting. Legacy prepared
wrapper preserves both supplied track/master gains and identical reusable setup.
Five additional public fixtures written with actual Native/Arena graph/PCM ingress:
independent nonzero positive tails and exact reference PCM sum; premature refusal
with untouched reference/layout/capacity followed by safe reset while B tail remains;
A/B/A cold-A reference equality with unchanged immutable B key/layout; all four
ordinary/reusable alias directions; foreign/stale/skip/wrap/exhausted/final cleanup
refusal and exact owner return/all-field actual free-capacity restoration.
All callbacks use the unchanged TLS allocation/deallocation probe. Actual queue/
ACK/garbage APIs produce success receipts; no manual Ready or Rebound inserted.
No author Cargo; no compilation/behavioral clearance claimed. Two Rust paths only,
all broader preparation/scheduler/transport consumption remains outside this child.

### ROOT0412 exact feedback argument correction
ROOT0411 checker original75239 terminal101: strict Clippy compilation found one
missing fourth pool_event argument in A/B/A rebirth. Raw independent002 clippy.log
retained. Added only feedback0.8 consistent with preceding pool events; all
assertions, helpers, child and production unchanged. No behavioral clearance or
author Cargo; scopedfmt0 and source held for mandatory retry.
