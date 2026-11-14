# Staging activation regression implementation plan

**Status**: Completed
**Plan ID**: SONG-09-STAGING-REGRESSION
**Created / Last Updated**: 2026-10-02
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Purpose and evidence

Reconcile the established native sample staging witness with actual finite
runtime activation. Joined checker006 passed DSP648 and song audio14, then
staging21/22. native_sample_validation_is_incremental_and_completion_stays_silent
seals a ready sample-only preparation and requests frame0 after two rendered
callbacks. SongRuntime::queue correctly rejects its past frame as Malformed;
the fixture still expects the earlier staging-only NotReady implementation.

Accepting empty ready epochs is an explicit finite song requirement. Do not
disable their activation to preserve this outdated diagnostic. Preserve every
incremental validation, silence, capacity, original Arc and cancellation proof.

## Manifest

```json
{
  "planId": "SONG-09-STAGING-REGRESSION",
  "planPath": "impl-plans/active/song-mode-staging-activation-regression.md",
  "dependsOn": ["SONG-09P", "SONG-09"],
  "writePaths": [
    "tests/song_resource_staging.rs",
    "impl-plans/active/song-mode-staging-activation-regression.md"
  ],
  "sharedPaths": ["tests/song_resource_staging.rs"]
}
```

## Related plans and ownership

- **Previous**: [Resource staging](song-mode-resource-staging.md).
- **Parent / Next**: [DSP routing](song-mode-dsp-routing.md), joined host checks.

Acquire this single test path serially after checker006 is terminal. Other
cohort sources and plans remain held. No production changes, weakened tolerance,
manual successful Ready/Applied, dependencies, Git, index or archive operations.
Record immutable intent/post hashes; use the required Rust author and independent
checker. Quiet Cargo through mise; preserve original processes to terminal.

## Modules

| File | Deliverable | Status |
|---|---|---|
| tests/song_resource_staging.rs | exact past-frame rejection and unchanged real ownership witness | WRITTEN / VERIFICATION PENDING |

No public production declaration is introduced.

## Tasks

### TASK-001: Confirm actual finite contract

**Status**: In Progress
**Parallelizable**: No.

- [x] Confirm ready sample-only epoch and actual positive rendered clock.
- [x] Preserve actual frame0 request and all existing validation/return assertions.

### TASK-002: Correct and strengthen the witness

**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No.

- [x] Expect exact Malformed rejection for the past frame.
- [x] Assert the attempted activation remains silent and emits no Applied.
- [x] Retain cancellation, original Arc identity and restored physical capacity.

### TASK-003: Independent joined evidence

**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No.

- [x] Fresh unfiltered staging inventory/run executes all22 fixtures.
- [x] Joined DSP, audio, host, carrier, geometry and legacy gates remain enabled.
- [x] Record original terminal handles, raw results and unchanged cohort hashes.

## Completion criteria

- [x] Correct actual diagnostic and all unchanged positive ownership proofs pass.
- [x] Native/host-wasm/strict Clippy and scoped formatting pass.
- [x] All22 staging tests pass with actual nonzero inventory/run equality.
- [x] No broader phase or full song playback completion claim.

## Progress log

### Session: 2026-10-02 — checker006 failure review

Original87770 terminal101/b2faf5. Root read the actual staging fixture and
SongRuntime::queue: requested frame0 is past and rejection is Malformed.
DSP648/audio14 passed; later host/reflection gates were not executed.
This plan authorizes no source until its fresh root release.

### Session: 2026-10-02 — ROOT0312 scoped witness repair

Source inspection confirms two actual16-frame callbacks precede the sealed sample-only frame0 request. The fixture retains frame0 and all incremental ResourceReady, PCM reservation, original Arc pointer, cancellation and restored-capacity assertions. It now checks actual Engine.now()>0, silence on rejected activation, exact Malformed and absence of any Applied in those actual receipts. Empty Ready epoch acceptance remains production behavior. Scoped formatting only; no Cargo. TASK-003 and passing acceptance remain pending mandatory joined checker. Original87770/101/b2faf5 and staging21/22 retained.

### Session: 2026-10-02 — ROOT0317 independent scoped acceptance

Actual joined009 staging22/22 passed with frame0 retained, positive rendered
clock, exact Malformed rejection, silence/no Applied and unchanged original Arc
return/capacity proofs. Native/host-wasm/strict Clippy and finish010 gates passed.
Original55440 exited101 solely on the retained sampled-reverse route failure;
that separate geometry failure does not change this single-test child evidence.
ROOT0317 independently reconciles all source seals and raw names. Full song
playback is unfinished; archive/index changes remain delegated to SONG-16.
