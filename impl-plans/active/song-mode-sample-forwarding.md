# Checked sample contract forwarding in genuine host fixtures

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Sample admission reassessment](../../design-docs/specs/design-song-mode.md#focused-design-and-implementation-reassessment-2026-10-03)
**Dependency**: [Checked sender admission](song-mode-sample-admission.md)

## Intent

Existing recording and pressure adapters must delegate the new checked sample
contract to their actual provider. The default checked host refusal is genuinely
Unavailable; it must not infer a transient result from the ambiguous legacy
Arc-only method. Preserve every existing physical host assertion and pressure
gate. The controller adapter is handled by its current controller plan.

## Exact source manifest

| Path | Deliverable |
|---|---|
| `tests/song_bus_memory_layout.rs` | Delegate typed sample transfer and sender capacity to the retained actual host |
| `tests/song_host_profile.rs` | Delegate the same checked contract without changing production profile assertions |
| `tests/song_host_preparation/pressure.rs` | Delegate checked sample transfer and sender capacity while retaining real command pressure |

## Declaration contract

Each existing AudioHost adapter implements the current trait declarations:

- `try_song_sample(&mut self, lease: SongLeaseKey, data: Arc<SampleData>) -> Result<(), SongSampleRefusal>`
- `song_sample_sender_capacity(&self) -> Result<SongSampleSenderCapacity, Failure>`

Forward the original lease and Arc unchanged. Do not manufacture capacity,
acknowledgments, success, or a refusal category. Existing legacy forwarding stays
available. All touched Rust files must remain below 1000 lines; declare any
necessary split before adding a path.

## Tasks

1. **Implemented**: Revalidate actual source and checked API; record source baselines.
2. **Implemented**: Specialized Rust author implements only these three delegations.
3. **Pending**: Independent checker executes the full bus layout, host profile and
   host preparation suites alongside sample admission, with native/WASM gates.

## Completion criteria

- [x] All three adapters forward both checked operations to their actual provider.
- [x] Original fixtures retain complete assertions and real pressure/ownership.
- [ ] Full affected suites pass independently after held source verification.
- [x] Source formatting and under-1000 line requirements pass.

## Progress log

### 2026-10-03 — Root source audit

Identified three adapters still overriding only the legacy sample operation.
This plan declares their exact compatibility work without enlarging the eight
production admission paths. No Rust change or new verification has run here.

### Source release: current immutable baselines

Parent released these three thin delegation paths separately from admission8.
- `tests/song_bus_memory_layout.rs`: 802 lines, SHA256 `869c059f81b48c3981c0cbfa257a51de2d204364be2086551dad36624dd90bff`.
- `tests/song_host_profile.rs`: 901 lines, SHA256 `e61137cbde306d3b753c8447be8aa67fc5b2a96d24d397abd834b553dd9f4a8f`.
- `tests/song_host_preparation/pressure.rs`: 419 lines, SHA256 `994e2c64eceb1dd0feb7428b05ea0339b14ac7e82c066024e9a443662a4ab251`.

### Source ready / held checkpoint

All three real adapters delegate checked sample admission and sender capacity
without touching existing legacy forwarding or pressure/resource assertions.
Scoped formatting passes and each Rust file is below1000 lines. Independent
full bus-layout/profile/preparation suites remain pending; no author Cargo.
