# Capacity query result carriers implementation plan

**Status**: Completed
**Plan ID**: SONG-09PQ-C
**Created**: 2026-10-02
**Design Reference**: [Song mode design](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Purpose

Internal native capacity queries must not become unsolicited song receipts in legacy playback. A capacity query may fail with NotReady, checked capacity failure or serial exhaustion. Its failure must be distinguishable from a rejected activation with the same epoch before a FIFO origin ledger can safely consume internal results. Public capacity replies remain observable. Full song activation, private routing, finite transport and export remain required by the parent goal.

## Manifest

```json
{
  "planId": "SONG-09PQ-C",
  "writePaths": [
    "src/song/routing.rs",
    "src/host/caps.rs",
    "tests/song_resource_carriers.rs",
    "impl-plans/active/song-mode-capacity-result-carriers.md"
  ]
}
```

## Related plans

- Previous: graph-bank bindings and resource-staging carriers.
- Parent: [Actual resource staging](song-mode-resource-staging.md).
- Next: bounded native capacity query origin routing.
- Source ownership is serial after held09PB/09PC; other source and plans remain held.

## Deliverable contract

```rust
// Append to existing SongHostAck, retaining all existing variants.
CapacityRejected { epoch: SnapshotEpoch, reason: SongRejectCode }
```

The epoch method returns the exact full epoch. Append ACK subtag10; preserve tags0–9, command tags, 74-byte maximum, framing and existing legacy messages. Failure record carries the epoch and checked rejection-code byte, using existing failure-code conversion rules. No dependency or global protocol redesign is authorized.

| Module | Change | Status |
|---|---|---|
| src/song/routing.rs | Add dedicated capacity failure POD and exact epoch classification | Completed |
| src/host/caps.rs | Encode/decode append-only checked failure result | Completed |
| tests/song_resource_carriers.rs | Actual roundtrip/framing/recovery fixtures and pending producer expectation | Completed |

## Tasks

### TASK-001: Contract and immutable intent

**Status**: Completed
**Parallelizable**: No

- [x] Read current POD/codec and record exact before hashes.
- [x] Confirm all exhaustive consumers before mutation.

### TASK-002: Dedicated result codec

**Status**: Completed
**Depends On**: TASK-001

- [x] Implement exact failure identity and append-only native/byte semantics.
- [x] Preserve existing codec maxima and malformed-frame recovery (verified).

### TASK-003: Evidence and handoff

**Status**: Completed
**Depends On**: TASK-002

- [x] Execute nonempty carrier fixtures and current wire/bank regressions with native/browser checks and strict Clippy.
- [x] Record actual process handle, terminal result and final source hashes.
- [x] Retain producer ownership in SONG-09PQ; its actual Engine now emits CapacityRejected for failed queries, while other commands keep generic Rejected. This integration passed the joined verification.

## Execution and completion

- [x] Only three Rust paths and this plan modified; every touched Rust file remains below1000 lines.
- [x] Source intents precede mutations; all writers hold during joined checks.
- [x] Specialized author/checker, quiet Cargo and nonduplicated test inventories used.
- [x] All dedicated-result fixtures pass; old tags and full-width epochs remain exact.
- [x] Parent handoff recorded; no full staging or playback completion inferred.

## Progress log

### 2026-10-02 — source-grounded planning

ROOT0234 records before hashes and new-plan intent. Actual retry006 passed36 staging/bank/carrier fixtures but exposed internal reports in the wire runtime. Root confirmed constructor queries escape poll/drain into legacy unsolicited-song faults. The dedicated failure result closes same-epoch error ambiguity for the forthcoming native FIFO origin ledger. No Rust changes or Cargo execution in this planning step.

### Exact private declarations before Rust (ROOT0235)
```rust
// caps::song_codec, shared old/new rejection-code validation
fn get_reject(r: &mut Decode<'_>) -> Result<SongRejectCode, WireError>;
// existing public carrier fixture file
fn capacity_rejected_preserves_full_identity_reason_and_following_frame();
```
Subtag10 carries common full epoch plus one strict reason byte. The existing
subtag3 decoder uses the same helper, preserving every old numeric code.
Native unconfigured RequestCapacity still expects generic Rejected until the
serial producer child changes it; contract-only implementation must not alter
that assertion. All exhaustive consumers were inspected: owned ACK epoch/codec
need new arms; receipt/runtime consumers retain full ACK without variant match.
No author Cargo. Existing source and geometry plans outside this manifest held.

### ROOT0235 contract/codec ready and held
Receipts0001–0003 precede declarations and Rust mutations. CapacityRejected
is appended after all existing ACK variants; epoch classification retains full
u64. ACKsubtag10 encodes11 bytes; shared strict rejection helper preserves old
subtag3 numeric codes. Existing maxima/tags and native unconfigured generic
Rejected fixture are unchanged. One genuine new fixture tests all five reasons,
full epoch, exact wire bytes, every truncation, undersized buffer, unknown reason
bytes, concatenated old-result framing and native SPSC ordering. Browser
song_ack wildcard preserves dedicated capacity result without clearing leases;
runtime receipts retain the complete POD. Carrier remains987 lines. All three
Rust paths and this plan are held; scoped formatting done, no Cargo/test run.
The serial native producer owns future result emission and expectation change.
No staging or full song-mode completion inferred from this written contract.

### 2026-10-02 — independently verified carrier completion

ROOT0247 independently audited retry010 original39469 terminal0, all32 gates,
135 executed fixtures and43 current hashes. Carrier19, wire21 and graph-bank4
fixtures passed, including the append-only CapacityRejected codec/full-width
identity/malformed framing fixture and the unchanged130-rejection saturation
fixture. Native/host-wasm compilation and strict Clippy passed. The serial
SONG-09PQ producer expectation change preserves generic rejection for unrelated
commands and typed query failure, verified by actual Engine and browser transport.
ROOT0250 independently audited original13729 terminal0,12 gates and13 query
fixtures, including actual Engine ACK/control pressure with real typed failures.
The codec source and all19 carrier fixtures retained their verified hashes.

All criteria of this carrier child are proven; its status is Completed. Earlier
log entries retain their historical written-only state. This is a document-only
handoff; full resource staging, audible activation, finite transport and export
remain unfinished. Archive/index reconciliation remains deferred to SONG-16.
