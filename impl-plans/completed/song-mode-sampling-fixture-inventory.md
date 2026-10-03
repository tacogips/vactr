# Sampling fixture original inventory access

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Current completion path](../../design-docs/specs/design-song-mode.md#design-and-implementation-review-current-completion-path)

## Purpose

Prepare genuine frozen owner dependencies for sampling fixtures using their
own isolated evaluator and original frozen Song. A separately evaluated
candidate has different authority and cannot supply this inventory.

## Related plans

- **Depends On**: [Sampling relations](song-mode-canonical-sampling-relations.md).
- **Companion**: [Replay binding](song-mode-sampled-replay-binding.md).
- **Next**: Independent joined sampling acceptance, then immutable consumers.

## Exact one-path manifest

| Module | Deliverable | Status |
| --- | --- | --- |
| `src/session/song.rs` | Narrow crate-private cfg(test) wrapper around existing original inventory capture | COMPLETED |

The existing inventory function has parent visibility; use an owning wrapper
if Rust visibility prevents re-export. Do not change production visibility,
duplicate traversal, manufacture topology or borrow another candidate's IDs.
The companion adds this one path to the existing ten-path verification union.
The caller changes remain in the sampling plan's existing clock test child.

## Test-only declaration

```rust
#[cfg(test)]
pub(crate) fn capture_original_test_routing(
    evaluator: &Evaluator,
    song: &crate::song::Song,
    limits: SongAssetLimits,
) -> Result<crate::song::snapshot::FrozenRoutingInventory, Failure>;
```

Use existing capture_routing with its authentic default prepared-shape context
where sufficient for these fixtures; no invented shape certificate is permitted.
Any need for additional prepared shape access requires explicit scope review.

## Tasks

### TASK-001: Owning test access

**Status**: Completed
**Parallelizable**: No; same author as sampling repair.

- [x] Add only the test-gated owning wrapper; keep source below1000 lines.
- [x] Caller obtains routing from the same evaluator and frozen Song.
- [x] Run actual prepare_owner_dependencies before observing the selected owner.

### TASK-002: Joined verification

**Status**: Completed
**Parallelizable**: No; depends on TASK-001 and sampling repairs.

- [x] Genuine prerequisite execution is retained and callback reads denied on replay.
- [x] Source/clock/occupancy tests pass on held eleven-path union.
- [x] Native/WASM, strict lint and scoped format pass without production API changes.

## Completion criteria

- [x] Original authority and dependency preparation are authentic.
- [x] All joined acceptance gates pass with exact held input hashes.
- [x] Existing candidate capture behavior remains unchanged.

## Progress log

### 2026-10-03 — Test visibility seam authorized

Read source confirms inventory::capture_routing is cfg(test), parent-visible,
and delegates to existing capture_routing_prepared. Session song has889 lines.
Root authorizes this one-path companion while no checker command is live.
The author must include it in the next full held receipt. No passing result
is claimed; production retention remains subsequent work.

### 2026-10-03 — Authorized original fixture inventory implementation

The owning cfg(test) crate-private wrapper delegates existing capture_routing on the fixture's same actual evaluator and frozen original Song, preserving original issuance. Fixture stores this genuine inventory and prepares authenticated dependencies on the inherited ledger before observe_part, as production retention does. No reconstructed DTOs or synthetic PreparedShapes. Exact owning session/song.rs was889 lines before edits; now898. Independent acceptance pending.

### 2026-10-03 — Bounded phase accepted

Root accepts held0004 after364 fresh distinct Rust tests (clock7, sampling9,
occupancy19, affected202, public127), native compilation, strict all-target
Clippy, pure WASM and eleven-file formatting/line checks. All928 inputs match
and all checker processes are terminal. Receipt:
/tmp/vactr-canonical-sampling-final-004.json, SHA256
e7fd38dee8e054a9aedbadf67e2b0ddeee51ca2342030ca4013df75e30b0de38.
Extraction-only stored source evidence was checked before acceptance.

Fresh WASM b24694ea0994b0d2d2e867f33f41959b682fbc124a8713d6dfe9f828fa78f2f1
passes590 editor tests in78 files and frontend build; dist matches. Root test
37866/fd5b50/0 and build43380/ecb391/0 are terminal. Root independently rechecks
all928 source hashes after frontend gates. Receipt:
tmp/song-mode-riela/ROOT-editor-canonical-sampling-20261003.json.
Historical compiler/fixture failures remain recorded. Full song mode remains
in progress: actual invocation retention, immutable geometry/admission,
production pre-Reserve integration and useful uniform varying-seed support
are mandatory subsequent phases.
