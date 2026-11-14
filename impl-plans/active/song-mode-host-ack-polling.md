# Bounded host acknowledgment polling implementation plan

**Status**: Completed
**Plan ID**: SONG-08A
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Session target**: 1–3 sessions
**Design Reference**: [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent and source evidence

Preserve critical acknowledgments while Runtime applies bounded receipt backpressure. AudioHost::drain consumes an arbitrary entire batch, so a full receipt queue cannot stop that consumption. The original 64-entry receipt log silently evicted ownership acknowledgments. Explicitly failing after discarding an already-consumed critical acknowledgment does not provide retention.
Add generic one-message polling for real adapters; keep legacy full-drain semantics. SONG-08 owns later Runtime integration and can retain a single consumed POD until capacity returns. This narrow source-grounded prerequisite is not additional Riela acceptance.

## Manifest

```json
{
  "planId": "SONG-08A",
  "planPath": "impl-plans/active/song-mode-host-ack-polling.md",
  "dependsOn": [
    "SONG-07"
  ],
  "writePaths": [
    "src/host/caps.rs",
    "src/host/native/audio.rs",
    "src/host/wasm/messages.rs",
    "src/host/noop.rs",
    "src/host/testing.rs",
    "tests/song_host_ack.rs",
    "impl-plans/active/song-mode-host-ack-polling.md"
  ],
  "sharedPaths": [
    "src/host/caps.rs",
    "impl-plans/active/song-mode-host-ack-polling.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/host/caps.rs",
      "intendedEdit": "Serial bounded poll contract between completed SONG08 POD codec batch and remaining SONG08 receipt integration; same author owns both."
    },
    {
      "path": "impl-plans/active/song-mode-host-ack-polling.md",
      "intendedEdit": "Owner alone writes progress; archive/index deferred SONG16."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-07](song-mode-candidate-evaluation.md), independently completed.
- **Next**: [SONG-08 audio commands](song-mode-audio-contracts.md), receipt integration.
- **Concurrent independent work**: [SONG-07A selected-source metadata](song-mode-source-route-inventory.md), disjoint source paths.

| Dependency | Required evidence | Status |
|---|---|---|
| SONG-07 | Native/browser baseline and private preparation boundary | Completed |
| SONG-08 POD batch | Existing concrete HostMsg transport, current owned caps.rs baseline | 13 authored fixtures passed; not full phase clearance |
| SONG-07A | No source overlap; join before shared full phase verification if needed | In Progress |

## Execution and preservation contract

Use mandatory rust-coding author and independent check-and-test-after-modify agent. Fresh-read targets/dependencies, record immutable source/design/plan before/after SHA intents under tmp/song-mode-riela/SONG-08A, and compare current hashes immediately before edits. Own only six listed Rust files and this plan; all remain below1000 lines. Additional paths/splits need exact prior manifest amendment. Same author serializes caps.rs against SONG-08 codec writes; no concurrent caps mutation. Preserve existing native/browser queue order, drop counters, garbage collection and legacy drain. No dependency changes, lockfile edits, Git mutations, broad formatting, indexes or archives. Poll every foreground handle to terminal. Owner alone updates this plan; archive reconciliation is SONG-16.

## Modules and declarations

| File | Deliverable | Status |
|---|---|---|
| src/host/caps.rs | Explicit one-message AudioHost polling with default unsupported capability | Completed |
| src/host/native/audio.rs | Poll exact one ring record; retain counter/garbage housekeeping semantics | Completed |
| src/host/wasm/messages.rs | Poll one real WasmAudioHost deque record | Completed |
| src/host/noop.rs | Explicit empty supported poll | Completed |
| src/host/testing.rs | Recording host FIFO poll for deterministic fixtures | Completed |
| tests/song_host_ack.rs | Saturation, order, unavailable and legacy compatibility fixtures | Completed |

```rust
pub trait AudioHost {
    fn poll_msg(&mut self) -> Result<Option<HostMsg>, Failure>;
    fn drain(&mut self, out: &mut Vec<HostMsg>);
}
```

This is an addition to the existing trait, not a replacement/redefinition of its other methods. Default poll returns explicit HostUnavailable without calling drain or consuming source data; never fabricate empty success for a host lacking this capability. Actual built-in adapters implement the method. Host polling runs on the control thread, with no callback VM or new callback allocation. One call returns at most one record and leaves every following identity in its source queue. Native counter acknowledgments remain available and report deltas only when actually returned. Existing browser CellPort dummy drain is not an AudioHost implementation and requires no change.

## Tasks

### TASK-001: Capability and concrete adapters
**Status**: Completed
**Parallelizable**: No
**Deliverables**: Generic trait contract and all four concrete adapters.
- [x] Record fresh hashes and verify exact implementation enumeration.
- [x] Add explicit unsupported default and FIFO one-message adapter methods.
- [x] Preserve legacy drain, native housekeeping/drop reports and browser ingress handling.

### TASK-002: Behavioral fixtures
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No
**Deliverables**: Nonzero phase fixtures using actual adapters.
- [x] More than64 critical identities remain FIFO and exactly once under repeated polling.
- [x] Each poll leaves remaining source records available for later drain/poll.
- [x] Unsupported default consumes nothing and never calls legacy drain.
- [x] Native headless adapter ring/counter behavior and recording/noop behavior verified.
- [x] Browser concrete override compiles; actual browser queue parity carried into later host/end-to-end gates.

### TASK-003: Verification and clearance
**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No
**Deliverables**: Logs, terminal process evidence and exact six-source seal.
- [x] Run required gates with actual commands/exit status/nonzero counts.
- [x] Independent checker verifies all final source hashes, ownership and compatibility.
- [x] Update own plan only after independent clearance, then return caps ownership to remaining SONG-08.

## Handoff to SONG-08

Runtime song receipt handling uses bounded poll, never full-batch drain. Receipt admission returns the exact unconsumed acknowledgment on full; Runtime retains one pending POD, stops further source polling and retries after explicit receipt consumption. No silent eviction or discarded critical message. The host acknowledgment ring/deque retains following records; engine critical retry/backpressure then propagates naturally. Preserve legacy full drain when song receipt mode is absent. Later actual transport must process generation-qualified Ready/Applied/Muted/ResourceReady/Retired/Rejected before treating receipts as discardable logs. Acknowledgment overflow cannot falsely acknowledge preparation or release leases.

## Future verification

Set CARGO_TERM_QUIET=true for every Cargo command; nextest also NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1.

| Gate | Required evidence |
|---|---|
| mise exec -- cargo check --features host-native | Native adapters compile |
| mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm | Real browser override compiles |
| mise exec -- cargo clippy --all-targets --features host-native -- -D warnings | Strict joined warnings gate |
| mise exec -- cargo test --features host-native --test song_host_ack --test song_audio_wire | Nonzero phase and carrier fixtures |
| mise exec -- cargo test --lib host::native::tests::audio | Native adapter compatibility; exact selector validated before accepting count |
| mise exec -- cargo test --lib session::tests | Legacy Runtime/session behavior |
| mise exec -- cargo nextest run --features host-native -E 'binary(song_host_ack)' | Nonzero repeated phase fixtures |
| Scoped rustfmt --check --config skip_children=true and git diff --check | Owned formatting/whitespace |

## Completion criteria

- [x] All concrete built-in hosts preserve one-at-time FIFO and legacy behavior.
- [x] No unavailable fallback drains data; no hidden unbounded stash introduced.
- [x] All required gates pass with nonzero exact counts and complete logs/terminal evidence.
- [x] Independent hashes match author seal and every touched Rust file is below1000 lines.
- [x] SONG-08 can integrate true retained receipt backpressure; own progress updated.

## Progress Log

### Session: 2026-10-01 — source-grounded bounded polling prerequisite
**Tasks Completed**: Planning only.
**Tasks In Progress**: None.
**Evidence**: Required checker identified silent receipt eviction; author confirmed whole-batch trait cannot implement requested bounded stop-consumption semantics.
**Verification**: No Rust changes in this plan creation step. Current SONG-08 POD/carrier authored batch passes13 fixtures, remains partial phase evidence.

### Session: 2026-10-01 — concrete polling author seal
AudioHost::poll_msg default returns explicit HostUnavailable without calling drain. Actual NativeAudioHost pops one ring item and leaves the rest, collecting already-retired garbage on the control thread; pending local drop-counter deltas are marked reported only when returned after queued acknowledgments. WasmAudioHost pops one actual HostState acknowledgment deque item. RecordingAudioHost removes one FIFO reply, preserving shared alias behavior; NoopHost explicitly reports supported empty polling. Every existing legacy drain body is unchanged. No callback allocations, VM work, private stash, dependency or Git mutations are added.

Actual native and recording fixtures preserve130 identities, consume70 individually and then prove60 following records remain for legacy drain. Native sample Arc proof prevents live-sample release and confirms retired garbage collection. Unsupported-default proof returns HostUnavailable repeatedly without consuming data or invoking drain. Native drop deltas follow queued critical acknowledgment order and are reported once. Browser override compiles in the actual host-wasm target; later browser host/end-to-end verification remains a handoff.

TASK-003-checks-002 records all ten author gates exit0; retained13766 terminal0 and no active author processes. Host integration5 + carrier13 + recording2 + nativeadapter8 + legacy session78 =106 distinct fixtures; nextest repeats5. Initial focused48878 terminal0 passed4 host fixtures before the added garbage proof. Runner79976 retained terminal1 after three successful gates because exclusive author-phase-001.log already existed; no log overwritten and no source defect. New -002 logs/ledger preserve that failure history and complete the batch.

TASK-003-post-001 seals exact six-source hashes, largest838 lines, including the authorized serial caps.rs drift from partial08 baseline. Independent checker clearance is pending; plan remains In Progress. SONG-08 receipt integration is held until this clearance and then must retain one pending consumed POD, stop polling on full storage, and expose bounded consumption without silent eviction. This polling phase does not certify actual Ready/activation/routing, browser JS transport or snapshot audio installation. No further source writes are planned absent a concrete finding.

### Session: 2026-10-01 — mandatory polling independent completion
Independent /tmp/vactr-song08a-independent-002/final-results.json confirms ten gates0,106 distinct fixtures plus5 nextest repeats,60279 terminal0/noactiveprocesses and exact six pre/post SHA matches (maximum838lines). Initial independent001 joined compilefailure history is retained;002 is the complete passing retry. No adapter findings remain. TASK-004 records this doc-only Completed update and unchanged source seals; no Rust/archive/index/Git changes.

Caps ownership now returns serially to SONG-08. That phase must separately repair receipt saturation using this polling contract, retaining one consumed pending POD and stopping intake until capacity returns. Host polling clearance does not certify receipts, actual routing/Ready/activation, browser JS host end-to-end transport or export. Those obligations and SONG-07A route-metadata dependency remain explicit.
