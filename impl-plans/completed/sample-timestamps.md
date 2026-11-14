# Sample timestamp controls

**Status**: Completed
**Design Reference**: [Sample timestamp design](../../design-docs/specs/design-sample-timestamps.md#public-syntax)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Deliverables

| Task | Deliverable | Status |
|------|-------------|--------|
| STM-001 | start-ms/stop-ms control registration, types and checked time conversion | Completed |
| STM-002 | Sample runtime resolution and composable exact region selection | Completed |
| STM-003 | Focused native/browser regressions and required checks | Completed |
| STM-004 | Full-WAV breakcore example and documentation | Completed |

Timestamp boundaries use checked integer microseconds internally. Existing pattern region functions and public loader APIs remain integration points; no dependency additions.

## Completion criteria

- [x] Integer and decimal millisecond notation works with microsecond granularity.
- [x] Invalid/overflow/inverted/non-sample/out-of-source values report diagnostics.
- [x] Native chop/rearrangement/reverse and timestamp window composition verified.
- [x] Frame selection and native/browser paths verified with actual sample audio.
- [x] Rust checks pass and example uses the complete downloaded source WAV.

## Progress log

### Session: 2026-09-30
User refined the intended API to `[milliseconds].[three-digit microseconds]`. Supersedes proposed start-us/stop-us interface. Derived external slices are not the score mechanism; use in-language region operators over source.wav.

Coding agent completed exact timestamp compilation, integer/rational commit
windows and exact split-frame transport. Seven timestamp tests and seven
legacy region tests pass; library check and Clippy pass. Native and browser
forward/reverse audio matches at 44.1/48/96 kHz, including long-frame bounds.
Internal frame controls are hidden from editor metadata. Final full checks
and actual tracked score/demo exports pending.

### Final verification: 2026-09-30

All 1699 project tests and 11 renderer regressions passed, with zero failures
and two pre-existing ignored tests. Format, all-target Clippy, native/release
and both wasm builds passed. Glass Teeth exported its actual timestamp score
through the production engine; all 28 songs/previews and 84 listening-page
links passed independent auditing. The timestamp demo also exported all 16
cycles successfully. Whole-source CC0 provenance and checksum were verified.
