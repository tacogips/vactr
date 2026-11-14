# Correlated activation and exact initial onset implementation plan

**Plan ID**: SONG-ACTIVATION-CORRELATION
**Status**: Planning
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export), [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Purpose and dependencies

Enable the first onset at exact musical zero while preserving truthful activation
failure and resource ownership. Current exclusive Activate followed by waiting
for Applied makes a frame-A first event late: Engine applies controls and timed
commands, then renders A before the control thread sees the receipt. Existing
queue admission already authenticates Ready staged branches and same-frame
Activate-before-Event ordering. The missing boundary is failure correlation.

- **Previous / Depends on**: [Consuming host preparation](song-mode-host-preparation.md); runtime/pipeline shared-owner edits require verified handoff. Independent additive DTO authoring may proceed under ROOT0539; joint checks wait for all holds.
- **Consumer**: [Transport core](song-mode-transport-core.md), then [Runtime integration](song-mode-transport-integration.md).
- **Compatibility**: Existing generic non-activation rejection, Applied actual-frame,
  resource/full-key/capacity/clock receipts and legacy transport remain.

| Dependency | Required evidence | Current status |
|---|---|---|
| Host preparation | Original owner, typed graph uploads and genuine activation/cleanup | Authoring; joined verification pending |
| Timed queue | Authenticated staged generation, exact-frame command ordering | Source reviewed; new pipeline witnesses pending |
| Wire codec | Existing tags0..13 and exhaustive ACK encoder/decoder | Actual codec in host/caps/song.rs; source reviewed |

## Historical unsplit manifest — superseded by child plans

| Path | Deliverable | Status |
|---|---|---|
| `src/song/routing.rs` | Correlated activation failure DTO and epoch routing | NOT_STARTED |
| `src/host/caps/song.rs` | Append activation-specific ACK wire tag and exact decode | NOT_STARTED |
| `src/dsp/engine/song_queue.rs` | Activation admission failures retain requested activation | NOT_STARTED |
| `src/dsp/engine/song_runtime.rs` | Every executed/retained Activate failure uses correlated receipt; exact empty endpoint ordering | NOT_STARTED |
| `src/host/caps/song/preparation.rs` | Once-only activation with authenticated pipeline and matching outcome handling | NOT_STARTED |
| `tests/song_host_preparation.rs` | Update genuine owner activation obligations for the correlated protocol | NOT_STARTED |
| `tests/song_dsp.rs` | Preserve actual activation/rollback/frame/pressure regression semantics | NOT_STARTED |
| `tests/song_activation_correlation.rs` | Real Native/Arena zero-onset, codec, failure and pipeline witnesses | NOT_STARTED |

Eight Rust paths, serial ownership after host hold/checker. Each stays below1000.
Do not touch song_resource_carriers.rs (987lines); new codec cases belong to the
new correlation binary while old carrier tests remain regression gates. Actual
codec implementation is host/caps/song.rs, not its parent caps.rs. Any additional
exhaustive consumer or file split requires a root amendment before writing.

## Declaration contract

```rust
pub enum SongHostAck {
    // All existing variants retained.
    ActivationRejected { activation: SongActivation, reason: SongRejectCode },
}
```

Use the original requested activation epoch/frame in failures, and actual frame
in successful Applied. Append a new ACK tag; never renumber existing messages.
Existing public Ready submit_activation/receive_activation signatures remain.
This is a source-grounded future contract, not an implemented API declaration.

## Required behavior

- Every Activate failure site, including queue admission, timed execution and
  retained failure under ACK pressure, returns activation-specific correlation.
- Allow authentic initial timed events to enqueue after accepted Activate while
  one activation outcome remains in flight. Success enqueue is not Applied and
  does not prove on-time musical execution. Keep actual Ready/full-key/generation
  admission checks and original candidate/cleanup authority.
- Owner consumes only exact pending Applied or matching ActivationRejected.
  Forward generic Event/Release/Endpoints rejection to transport; it is not proof
  activation never occurred. Wrong activation frame/epoch remains unconsumed.
- Known never-active rejection permits genuine cancellation; uncertain/pending
  submission retains retirement obligations. Do not invent safe cleanup from
  elapsed frames, missing receipts, stale epoch or generic rejection.
- Same-frame first Event must render at activation musical zero in real Native
  and Arena processing. No lost onset, musical preroll, fake Applied or delayed
  timestamp substitution. Confirm compatible controls and samples under pipeline.
- Empty sequence/zero repeat and positive durations rounding to zero frames need
  exact Endpoints semantics: activation and endpoint must occur in valid order,
  release/no-onset end exclusivity and tail completion without an indefinitely
  active epoch. Preserve rejection of genuinely expired/invalid preparations;
  distinguish legitimate same-frame zero duration from a stale endpoint intent.
- ACK pressure that delays actual activation is a truthful late/failure outcome;
  do not claim exact music when the actual first event became expired. Retain
  correlated outcome and cleanup until genuine safe returns. Callback remains
  allocation/deallocation/VM free and fixed storage remains bounded.

## Tasks

### TASK-001: Complete failure and wire audit
**Status**: NOT_STARTED
**Parallelizable**: No; wait for original owner hold and verified handoff.
- [ ] Fresh exact source/design/plan hashes; audit every Activate failure/exhaustive consumer.
- [ ] Exact new wire tag/length, existing tag compatibility and stale correlation.

### TASK-002: Correlation, owner pipeline and zero-frame endpoint behavior
**Status**: NOT_STARTED
**Depends On**: TASK-001
**Parallelizable**: No; shared owner paths require serial edits.
- [ ] Implement all source deliverables and preserve original ownership/failure invariants.
- [ ] Support real exact onset and all accepted empty/subframe finite boundaries.

### TASK-003: Actual protocol and audio evidence
**Status**: NOT_STARTED
**Depends On**: TASK-002
**Parallelizable**: No; mandatory joined checker after source hold.
- [ ] Roundtrip correlated ACK, old wire tags unchanged, malformed/truncated records rejected.
- [ ] Genuine Native/Arena first-frame audibility and partition invariance.
- [ ] Generic event rejection cannot masquerade as rejected activation; wrong nonce/frame forwarded.
- [ ] Admission/execution rejection, physical ACK/garbage pressure and once-only safe cleanup.
- [ ] Empty/zero repeat/subframe end and tail semantics, existing rollback/mute/generation regressions.
- [ ] Native/wasm/strictClippy, line/format checks and full nonempty actual inventories.

## Completion criteria

- [ ] Every declared behavior and real production failure site verified.
- [ ] Exact original process/hash/raw log evidence reconciled; no unexpected writes.
- [ ] Transport consumer receives coherent correlation and exact onset authority.
- [ ] Full progression, Apply/mute, Native/browser playback and streaming export remain required.

## Progress log

### 2026-10-02 — ROOT0537 actual initial-onset timing prerequisite

Specialized read-only audit confirmed Engine controls and timed application
precede audio rendering, and post-Applied first-event submission is late. Root
verified actual queue ordering and codec locations, and preserved the large
carrier fixture file by selecting a fresh binary. This plan authorizes no source
writes yet. Host owner verification still proceeds against its existing released
contract; complete playback requires this serial followup before transport Ready.


### 2026-10-02 — ROOT0538 actual sorting owner and bounded split

Actual queue insertion/sorting lives in dsp/song.rs; the projection helper alone
cannot fix execution order. Genuine empty sequence/Repeat0 and positive rational
durations rounding0 can queue Endpoints before Activate, reject the inactive
endpoint, then remain active. Legitimate zero-tail equality also needs distinct
treatment from overdue activation. The ninth required sorting module makes the
original unsplit manifest invalid for a release.

The historical manifest is superseded by [DTO/wire](song-mode-activation-wire.md)
(three Rust modules), then [runtime/pipeline](song-mode-activation-pipeline.md)
(seven Rust modules). This parent owns its document only and releases no Rust.
All exact onset, empty/subframe timing, failure correlation, cleanup and complete
transport obligations remain intact. Host owner still completes/checks its
original released contract before this serial followup.

Public intake always dispatches Activate through song_runtime_record before the
private song_record staging helper; the direct generic fallback is cfg(test)
bypass evidence, not a reachable public activation path. Actual ACK exhaustive
consumers fit the DTO/codec paths; remaining host/Runtime caches forward or ignore
the new typed carrier without clearing unrelated upload ownership. No source
changed and no future checker commands ran during this refinement.


### ROOT0539 independent DTO authoring

DTO/codec paths do not overlap the current host author. Additive carrier source
and codec fixtures are released independently; no production outcome emission
or runtime-owner semantics change in that phase. The shared runtime/pipeline
phase remains serial and unreleased. Joint checking freezes all participating
sources before execution. This preserves the original verification and complete
playback obligations while allowing independent implementation progress.
