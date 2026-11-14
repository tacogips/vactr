# Song regression fixture reconciliation

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Song mode](../../design-docs/specs/design-song-mode.md), exact ACK authority, finite timing, resource ownership and unsupported diagnostics.

## Purpose and exact manifest

Reconcile nine obsolete fixture sites and diagnose the actual blocked-activation
setup. No production changes, dependencies, quota reductions or fabricated ACKs.

| Rust path | Baseline lines | Deliverable |
|---|---:|---|
| `tests/song_candidate.rs` | 606 | Actual missing preparation limits refusal preserves original pending candidate |
| `tests/song_capacity_routing.rs` | 165 | Two original activation POD rejection expectations |
| `tests/song_dsp/lifecycle.rs` | 390 | Actual receipt/intake diagnostic, then reviewed genuine initial Applied setup |
| `tests/song_host_ack.rs` | 178 | Explicit one-internal-record polling plus typed activation outcomes |
| `tests/song_protocol.rs` | 398 | Unknown active mute epoch Type, preserve no-side-effect assertions |
| `tests/song_resource_carriers.rs` | 987 | Cancellation retains accepted sender charge until exact full-key return |
| `tests/song_resource_staging.rs` | 899 | Late activation correlated Malformed outcome |

Seven modules; each must stay below1000. Carrier headroom is only12 lines: add
compact actual existing fixture challenges, no new helper or general framework.
No Cargo by author. Existing strict codec and whole130-order witnesses stay.

## Declarations before editing

Existing test signatures remain. Add only test helper:

```rust
// tests/song_host_ack.rs; existing generic ack stays for Unsupported tests.
#[cfg(feature = "host-native")]
fn activation_ack(epoch: u64, frame: u64) -> HostMsg;
```

Use actual SongHostAck::ActivationRejected with the original full epoch/frame
and NotReady in capacity/poll fixtures. Staging late frame0 instead expects
Malformed, with unchanged sample completion, silence, return pointer and PCM
capacity checks. Do not broaden generic ACK conversion.

Native poll intentionally reads one physical record; assert actual first
startup internal result None before public expected outcomes. Preserve130 FIFO,
>64 continuation, queued ACK before drop delta, exact delta once and Arc
strong-count transitions. No unbounded loop that treats None as all-drained.

Candidate missing configured preparation limits is HostUnavailable; its returned
original PreparedSong remains pending. Invalid isolated evaluation retains it.
No Ready/Applied is introduced. Unknown acknowledged mute epoch is Type before
any POD; preserve namespace/file/clock/slot assertions and full epoch/re fields.

PreparationCancelled stops transfer but does not authenticate physical lease
return. Assert unchanged metadata/PCM charge, then existing exact full-key
LeaseReturned releases it. Retain wrong kind, same numeric IDs, completed upload,
legacy byte-quota and pointer preservation assertions.

## Tasks

### TASK-001: Nine stale fixture sites

**Status**: In Progress
**Parallelizable**: Yes

- [ ] Exact expected fields and generic cases preserved.
- [ ] Poll consumption and sender retention match actual provider contracts.
- [ ] No namespace/file/clock, PCM, pointer or callback allocation oracle removed.

### TASK-002: Lifecycle evidence before setup correction

**Status**: In Progress
**Parallelizable**: Yes

- [ ] Capture backend/frame, actual control/intake queue length, physical ACK
  length and complete actual receipts at each original scheduling boundary.
- [ ] Keep original expected Applied count1 while independent diagnostic runs.
- [ ] Report actual cause before any setup correction; count1 must not become0.
- [ ] If confirmed first Activate was late/unadmitted, establish genuine epoch11
  Applied before filling ACK ring, retain blocked epoch99 original POD/past end,
  zero PCM/no active99/finite cleanup and exactly one first Applied.

### TASK-003: Independent closure

**Status**: Not Started
**Parallelizable**: No

- [ ] All seven whole binaries pass independent checker after source holds.
- [ ] Scoped rustfmt, line ceilings and production unchanged verified.
- [ ] Honest actual evidence logged; static Index/dynamic full goal unchanged.

## Progress log

### 2026-10-03 — Prior declaration

Original46202 exited101, remaining matrix530 passed/10 failed in seven targets.
Nine sites follow already changed exact ACK/poll/capability/ownership contracts.
Lifecycle Applied0 versus1 remains causally unproved; no expected count change
authorized. Initial source hashes/intent are retained in
`tmp/song-mode-riela/SONG-REGRESSION-FIXTURES/0001-before-intent.json`.
Author source-only; independent checker executes all Cargo commands.

### 2026-10-03 — Diagnostic source-held checkpoint

Nine obsolete expectation sites reconciled, preserving their original authority
and assertions. Lifecycle setup/count1 remains unchanged; failure diagnostics
now report frame/native/Arena queue/physical ACK lengths at three actual
boundaries and the complete real receipt vector. This checkpoint deliberately
expects the lifecycle diagnostic may still fail. Scoped rustfmt/check0 and all
production Rust unchanged verified. Carrier991 remains under1000. Independent
behavior execution pending; no Cargo by author.

### 2026-10-03 — Second diagnostic and exact retirement consumption

Independent six repaired binaries original11591 exited101:78 passed/one failed
(host retirement). Lifecycle original92601 exited101: first Applied count1
passed, disproving first-activation-lateness speculation; the failing second
NotReady count1 lacked diagnostics. Setup and reason are unchanged; full actual
records now attach to that exact assertion. Host retirement uses real public
drain across internal refresh reports and asserts Retired7 exactly once plus
original Arc1. Earlier live Arc2/Installed generation/startupNone are retained.
Scopedfmt/check0; no production drift/Cargo. Independent checks pending.

### 2026-10-03 — Authentic cleanup after rejected late endpoints

Exact third diagnostic passed correlated Malformed count but failed duplicate99
re-admission StaleEpoch. Source arena/song.rs481 checks existing preparation,
not a global epoch floor. Actual receipts contain both late Endpoints rejected
Malformed, so no cleanup was authenticated. Preserve the entire original
pressure/count/PCM scenario. New valid current-frame Endpoints11 and explicit
never-activated CancelPreparation99 drive real normal returns/cancellation.
Assert all11 original full keys returned once, reclaimed measured templates,
buses/sample/PCM, old11 bookkeeping gone, then fresh100 identical geometry
admission and genuine cancellation. DSP has no Ended ACK; Session Ended is a
separate transport publication. Scoped format/check0; behavior pending checker.
