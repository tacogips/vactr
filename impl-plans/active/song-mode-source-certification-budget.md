# Source certification work accounting implementation plan

**Status**: Completed
**Plan ID**: SONG-07G
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance) and [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and actual source evidence

Nested source routing requires reusable prepared payload certificates and one
shared preparation budget. certify_source_uses owns its remaining-work counter
internally and returns a cover without exposing consumed admission. Preparation
cannot correctly deduct cumulative certificate work; runtime recertification
would reset quotas. Source inspection confirms the missing immutable accounting
result. Add the smallest additive interface, then let SONG-08B consume it.

This supporting refinement follows the accepted design. It is source-grounded,
not a separate Riela review. Full nested authentication, mapped policy geometry,
playback, Apply/mute, transport, export and editor requirements remain unchanged.

## Related plans

- **Previous / Depends On**: [Source-free certification](song-mode-source-free-certification.md), independently cleared SONG-07F.
- **Next / Consumer**: [Route preparation](song-mode-route-preparation.md), SONG-08B in progress.
- **Following**: Parent SONG-08 joined clearance and SONG-09 DSP playback.

## Exact manifest

```json
{
  "planId": "SONG-07G",
  "planPath": "impl-plans/active/song-mode-source-certification-budget.md",
  "writePaths": [
    "src/song/source_uses.rs",
    "tests/song_source_budget.rs",
    "impl-plans/active/song-mode-source-certification-budget.md"
  ],
  "sharedPaths": [],
  "ownershipNotes": "rust_coding owns two Rust paths plus own plan after root release. SONG-08B owns disjoint routing files; prior07F helper/tests and all prior plan documents remain held."
}
```

Fresh numbered immutable SHA intents precede every batch under
tmp/song-mode-riela/SONG-07G/. Preserve failed logs and all original process
handles through terminal observation. Never restart on an observation timeout.
No additional paths, dependencies, lockfiles, broad formatting, Git, index or
archive changes. Keep every touched Rust source below 1000 lines. Do not restore
prior07F seals: this additive interface receives its own joined verification.

## Deliverables and interface

| Path | Deliverable | Status |
|---|---|---|
| src/song/source_uses.rs | Immutable actual certification work on successful cover | COMPLETED |
| tests/song_source_budget.rs | Public admission boundary and cumulative caller-budget fixtures | COMPLETED |

Add private consumed_work storage to FrozenSourceUseCover and a public immutable
consumed_work getter returning u32. Record exact declaration before implementation.
The value is the successful certifier's initial max_nodes minus final remaining,
after all existing traversal, cache, layout, graph-copy and policy-family admission.
The subtraction must be checked or supported by an explicit invariant. Do not
change traversal, cache semantics, limits, diagnostic behavior or admission costs.

This is actual admitted certification work, not a mathematical minimum or a claim
about all CPU time or caller allocations. Cloning a prepared cover retains the
reported value; callers still charge their own separate storage and matcher work.
Preparation can certify with its current remaining quota and subtract the returned
value before another certificate. It must not reset quota, infer a cost from graph
size alone, or recertify during per-event resolution. SONG-08B owns that integration.
Failed certification returns no successful cover/accounting value.

## Tasks

### TASK-001: Add immutable work result
**Status**: Completed
**Parallelizable**: No

- [x] Record fresh source/design/plan hashes and exact declaration.
- [x] Add private value and getter after complete existing admission.
- [x] Preserve all source-free/selected diagnostic and limit semantics.

### TASK-002: Behavioral evidence and author seal
**Status**: Completed
**Parallelizable**: No, depends on TASK-001

- [x] Successful genuine ordinary/nested selected cover reports positive bounded work.
- [x] Same deterministic input succeeds at its reported quota and fails one less where positive.
- [x] Two certificates consume a shared caller budget; insufficient cumulative quota rejects the second.
- [x] Cover cloning retains accounting; invalid inputs retain failure without a successful result.
- [x] Existing source-free12, source-use17, layout9 and relevant candidate/private regressions pass.
- [x] Fresh inventories, native/wasm, strict scoped Clippy, formatting/diff and nextest evidence pass on held sources.
- [x] Author seal records exact hashes, commands, counts, logs and terminal handles.

### TASK-003: Independent verification and handoff
**Status**: Completed
**Parallelizable**: No, depends on TASK-002

- [x] All Rust writers held for independently executed adequate joined matrix.
- [x] Root inspects exact counts/logs/exits/current hashes and reconciles prior07F baseline.
- [x] Owner marks own plan complete with independent evidence; no index/archive work.
- [x] SONG-08B receives immutable accounting interface for cumulative nested preparation.

## Completion criteria

- [x] Three tasks independently proved on unchanged source hashes.
- [x] Returned work includes final copy/family admission; no quota reset or relaxed limits.
- [x] Additive API only; source semantics and unrelated work preserved.
- [x] Consumer integration remains separately tracked; no full routing/playback claim.

## Progress log

### Session: 2026-10-01

Root created this bounded plan after actual nested-preparation source inspection.
ROOT/0112 records prior exact manifest and source baselines. Implementation requires
explicit root release and the required specialized Rust author/checker workflow.

### Author declaration: 2026-10-01

ROOT0113 releases the two Rust files plus this plan. Add private `consumed_work: u32` to `FrozenSourceUseCover` and `#[must_use] pub const fn consumed_work(&self) -> u32`. Successful construction computes checked `limits.max_nodes - *state.remaining` after all traversal and graph/policy-family admission and successful policy copying. No admission call, cost, limit, cache, failure path or caller quota is modified. The reported scalar is admitted work, not minimum CPU time or caller storage cost. Rust changes are active; independent verification and consumer quota integration remain pending.

### Independent completion: 2026-10-01

TASK-001 through TASK-003 are complete for SONG-07G only. Five actual public
budget fixtures prove positive ordinary/nested work, exact quota/one-less
admission, cumulative caller accounting, clone retention and final copy/family
admission. The focused matrix passed 43 tests; original process 48261 exited 0.
The immutable getter was handed to SONG-08B for separately tracked integration.

Final author evidence is `tmp/song-mode-riela/SONG-07G/0005-final-author-readiness-seal.json`:
12 gates exited 0, original process 40477 exited 0, 296 distinct fixtures
(public 192, song library 40, pattern 64; no qualified-name overlap), plus 43
nextest repeats. All 16 held input hashes were unchanged.

Independent evidence is `/tmp/vactr-song07g-independent-001/final-results.json`
(SHA-256 `2595690be0654da450c5e6129efd07df1909a271107d2fba1f250607de1be0b8`).
Original process 49794 exited 0; nine execution and fourteen inventory commands
all exited 0, confirming 296 distinct fixtures and 43 repeats. Root accepted
phase-only clearance in ROOT0122 after inspecting all 16 current hashes and log
hashes. Both Rust deliverables remain unchanged and held. This documentation-only
completion is recorded by immutable 0006 before/after artifacts. No archive or
index changes were made. SONG-08B routing, parent SONG-08 and production playback
remain pending; certification accounting does not establish host readiness.
