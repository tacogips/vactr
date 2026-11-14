# Browser song runtime verification

**Status**: Completed
**Created**: 2026-10-03
**Design Reference**: [Song design](../../design-docs/specs/design-song-mode.md#native-and-browser-capabilities)

## Purpose and dependencies

Verify the rebuilt host-wasm artifact through actual raw ABI calls. Native Rust
library tests cannot execute session_half fixtures because host::wasm is gated
to wasm32. Pure target checking/building proves compilation; it does not prove
catalog behavior, frozen PCM or audible finite playback.
Depends on browser-session production ABI and acknowledged publication.

## Module and contracts

One module: `editor/test/wasm/song.test.ts`.
Use existing `loadVactrWasm()` with the fresh cdylib. Fixtures cover:

1. Catalog refusals and invalid sample rates preserve the previous valid catalog.
2. Updating uploaded PCM/future factory after submitting a candidate leaves its
   playback identical to the unchanged baseline.
3. Original whole-code Apply crosses actual separate session/worklet instances,
   produces real correlated Applied and nonzero PCM, reaches Ended and silence.
4. Mute/unmute requests receive actual correlated worklet acknowledgements,
   close the audio gate and reopen future onsets without replaying missed notes.

Helpers may initialize a pair, upload PCM, pump real framed command records and
read worklet output. No fabricated acknowledgements or nominal-only selection.

## Tasks and completion

1. Implement four actual ABI fixtures using the existing test infrastructure.
2. Execute against the fresh artifact and retain complete output/exit status.
3. Reconcile results with browser-session evidence and remaining Apply work.

- [x] Actual catalog/refusal behavior verified.
- [x] Original candidate PCM immutability verified through audio output.
- [x] Actual finite playback and silence verified.
- [x] Actual acknowledged mute/unmute and no replay verified.
- [x] No consumer-only tests treated as browser runtime acceptance.

## Progress

2026-10-03: Pure wasm check/build passed. The combined native library command
selected zero adapter tests; recorded as missing evidence. Fresh artifact
`target/wasm32-unknown-unknown/debug/vactr.wasm`, SHA-256
`932945a9ce2047efcbbd07a58e814597b890cd54dd9bb54077f03aa4ba7c8e08`.

2026-10-03 review: the first actual ABI run exited 1; all three fixtures failed
at the Ended assertion. Full output: `/tmp/vactr-browser-song-real-abi.log`.
No browser runtime acceptance is claimed. Inspection also found the harness
fault-tag assertion uses 0x40 whereas the production fault tag is 0x60;
framing/routing/clock diagnosis is required before treating the missing Ended
as a production defect. See the implementation review in the design reference.

2026-10-03 follow-up: added the actual telemetry subscription (Ended is a
broadcast) and corrected the fault tag. The rerun still failed all three
fixtures: `/tmp/vactr-browser-song-real-abi-repaired.log`. Added bounded failure
diagnostics and ran again: `/tmp/vactr-browser-song-real-abi-diagnosis.log`.
Real Ready, Applied and Playing messages are now observed, but PCM remains
silent and Ended is absent after 8.192 seconds. Several actual acknowledgements
are reported stale. This is stronger evidence of an unresolved transport or
adapter issue; it is not browser acceptance. A specialized Rust review is
tracing ownership and receipt dispatch before any production fix.

The receipt trace `/tmp/vactr-browser-song-receipt-diagnosis.log` shows actual
Applied followed by exact-epoch LeaseReturned receipts, without rejection.
Source review identified the cause: WasmAudioHost returns its last captured
ClockReport indefinitely, while Runtime requests another only on observation
failure. The scheduling horizon remains at local zero, so no events are realized
and the transport never hands off to retirement. A specialized Rust author is
implementing correlated clock refresh; verification must rerun the real ABI
fixtures after rebuilding the modified artifact.

2026-10-03 clock checkpoint: independent native check passed, six clock tests
passed, and activation/atomic/host-clock codec suites passed 4/4/7 tests.
Fresh pure wasm build passed; artifact SHA-256
`e71d596eb866b46fab19acc10088bbeea886ebf1430d3ba92c09d581e0157956`.
The actual four-fixture run exited 1 with three passing: catalog refusal,
original PCM immutability and finite whole-code playback with genuine Applied,
nonzero audio, Ended and silence. Full output:
`/tmp/vactr-browser-song-clock-fixed.log`. Editor TypeScript check passed.

The new mute/unmute fixture fails with an actual Malformed rejection, not a
fabricated receipt. Focused trace: `/tmp/vactr-browser-song-mute-diagnosis.log`.
Inspection found the timed queue rejects a mute whose requested frame is already
past when the asynchronous browser command arrives. Interactive mute must apply
at the next actual callback frame and report that frame, while retaining exact
time admission for arrangement events and activation. A bounded Rust repair is
underway. No browser mute/unmute or atomic replacement acceptance is claimed.

2026-10-03 final scoped checkpoint: extracted activation logic retained its
original behavior; independent clock/activation/owner tests passed 6/4/12.
Fresh pure wasm build passed with artifact SHA-256
`b29cfd442341ed5544d1fb9058d4fa1acfb0bbff2f7782d9157626d80f54ccca`.
All four actual ABI fixtures passed, original process 35830 exited 0:
`/tmp/vactr-browser-song-late-mute-fixed.log`. This proves the declared catalog,
immutable audio, finite playback and actual acknowledged mute/unmute/no-replay
scope. It does not prove atomic replacement or a browser UI/audio-device run.

The Native/Arena late-mute oracle was repaired to compare hosts with identical
shared effect history. Both focused tests passed, original process 6560 exited
0: `/tmp/vactr-late-mute-public-002.log`. No production weakening was needed.
Full song-mode completion still requires atomic Apply, the real host capacity
profile and the remaining joined verification.
