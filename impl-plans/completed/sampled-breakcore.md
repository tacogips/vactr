# Sampled breakcore

**Status**: Completed
**Design Reference**: [Sampled breakcore](../../design-docs/specs/design-genre-tracks.md#sampled-breakcore-addition)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Deliverables

| Task | Deliverable | Status |
|------|-------------|--------|
| BRK-001 | CC0 recreated break and provenance | Completed |
| BRK-002 | Renderer native sample loading using existing public APIs | Completed |
| BRK-003 | Original 64-bar score, full WAV, preview, collection update | Completed |
| BRK-004 | Required checks and final artifact inspection | Completed |

Renderer uses the existing `NativeSampleLoader` type, registered to the score's FileId. No new public types or dependencies.

## Completion criteria

- [x] Downloaded break and explicit CC0 license recorded with source URL/checksum.
- [x] Native sample loading and missing-file/path rejection regressions pass.
- [x] Breakcore arrangement includes slicing, retriggers, reverse passages and contrast.
- [x] Score and full stereo render saved; listening page updated.
- [x] Required Rust verification passes; completed plan archived.

## Progress log

### Session: 2026-09-30
Found a SampleLoom synthetic Amen-pattern recreation explicitly labeled CC0; downloaded original float32 stereo 44.1 kHz WAV. Shimmer ambient addition remains separate and its three full renders have passed.

User clarified that edits belong in Vact. Removed derived slice WAVs and
external slicing helper; source.wav and provenance remain. Glass Teeth score
uses start-ms/stop-ms over the whole sample, chop64, splice64 with authored
index patterns, nested retriggers and negative-speed reverse playback.
Native sample loader and eleven renderer regressions pass; timestamp controls
are being implemented under the separate sample-timestamps plan.

New timestamp API is implemented; final release renderer rebuild is underway
so Glass Teeth can render with its actual timestamp controls. No externally
cut sample assets remain. Final exported score proof still pending.

### Final verification: 2026-09-30

All 1699 project tests and 11 renderer regressions passed, with zero failures
and two pre-existing ignored tests. Format, all-target Clippy, native/release
and both wasm builds passed. Glass Teeth exported its actual timestamp score
through the production engine; all 28 songs/previews and 84 listening-page
links passed independent auditing. The timestamp demo also exported all 16
cycles successfully. Whole-source CC0 provenance and checksum were verified.
