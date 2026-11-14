# Song codec fixture reconciliation

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Live Apply](../../design-docs/specs/design-song-mode.md)

## Purpose

Reconcile two actual broad-check fixture failures with existing production DTOs.
No production, wire, ownership or scheduling behavior changes. Static joint
Index remains Planning under its separate plan.

## Exact Rust manifest and declarations

| Path | Baseline lines | Change |
|---|---:|---|
| `src/session/tests/codec.rs` | 512 | Extend existing `fn servers() -> Vec<ServerMsg>` |
| `tests/song_audio_wire.rs` | 924 | Three exact expected activation outcomes |

Existing declarations remain unchanged. Add server values of actual
`SongInstrumentMutedBody { epoch, selector, muted, application_frame }` and
`SongTransportStateBody { epoch, state, instruments }`. Use epoch/frame beyond
2^53, real checked WireInstrumentSelector, and Playing/Draining/Ended states.
Preserve all KINDS and encode/decode equality loops.

Activation-only queue and 130-record saturation fixtures submitted frame0.
Expect actual `SongHostAck::ActivationRejected { activation: SongActivation {
epoch, frame: 0 }, reason: NotReady }` with original exact epoch and full FIFO.
The mixed allocation fixture submitted epoch/frame u64::MAX for its Activate;
check that full POD exactly at original ordinal1. The other nine outcomes remain
generic NotReady. Preserve count10, callback zero allocations, intake and
capacity assertions; do not transform unrelated generic rejection fixtures.

All touched Rust remains below 1000 lines. No new dependencies, production
paths, protocol variants, lint allowances or Cargo commands by author.

## Tasks

### TASK-001: Add missing codec server values

**Status**: In Progress
**Parallelizable**: Yes

- [x] Missing mute and transport-state samples authored with exact wide IDs.
- [x] Existing strict complete KINDS coverage and roundtrip assertions retained.
- [ ] Independent whole codec namespace passes.

### TASK-002: Correct correlated activation expectations

**Status**: In Progress
**Parallelizable**: Yes

- [x] Three sites match exact originally submitted activation POD/reason.
- [x] Nine generic mixed outcomes, count10, full130 FIFO, pressure and zero
  allocation assertions retained.
- [ ] Independent whole song_audio_wire binary passes.
- [x] Scoped formatting passes, exact two-source hold and production unchanged.

## Progress log

### 2026-10-03 — Actual failures and before-edit declaration

Whole lib original75495 exited101: 2010 passed, one failed, two ignored; strict
KINDS coverage found no song-instrument-muted server sample. Production already
has both missing server DTOs. Integration original49253 exited101: first seven
parents passed62, audio_wire18 passed/3 failed because old expectations used
generic Rejected for Activate. No behavior success is inferred for these failed
checks. Before intent and source hashes are retained under
`tmp/song-mode-riela/SONG-CODEC-FIXTURE-RECONCILIATION/0001-before-intent.json`.
Root authorized exactly these test-only exceptions while remaining integration
commands exclude both paths. Independent checker owns all Cargo execution.

### 2026-10-03 — Source-ready held fixture repair

Added actual checked mute selector and three transport states with epochs above
2^53 and exact mute frame above2^53. Mixed activation expectation retains
u64::MAX epoch/frame and original ordinal; nine generic records/count10 remain.
Activation-only full130 FIFO uses each submitted epoch/frame0. Scoped rustfmt
check passed; independent behavior checks pending. Production unchanged.
