# Checked song graph admission implementation plan

**Plan ID**: SONG-CHECKED-GRAPH-ADMISSION
**Status**: In Progress
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and related plans

Supply typed borrowed graph admission for the consuming host owner. Existing
submit_song_graph conflates queue pressure, unsupported hosts and invalid uploads.
The owner needs an exact retry decision while retaining its original graph.

- **Consumer**: [Host preparation](song-mode-host-preparation.md).
- **Depends on**: Existing graph codec, song upload wire and materialization.
- **Subsequent**: Complete owner, scheduler, Apply and playback integration.

This prerequisite does not establish Engine adoption or Ready. Legacy submission,
wire format and Native materialization remain compatible. No new dependencies.

## Manifest and declarations

Five Rust paths only; every touched source stays below 1000 lines.

| Path | Deliverable | Status |
|---|---|---|
| `src/host/caps.rs` | Checked trait companion using existing SongSubmitError | Not Started |
| `src/host/wasm/messages.rs` | Actual browser trait delegation | Not Started |
| `src/host/wasm/messages/song.rs` | Checked complete graph encoder/admission | Not Started |
| `src/host/wasm/abi.rs` | Pure empty-outbox capacity predicate | Not Started |
| NEW `tests/song_graph_submission.rs` | Genuine portable browser/Engine fixtures | Not Started |

```rust
pub trait AudioHost {
    fn try_song_graph(
        &mut self, lease: SongLeaseKey, graph: &GraphHandle,
    ) -> Result<(), SongSubmitError>;
}
pub(crate) fn record_fits_empty_outbox(record_bytes: usize) -> bool;
```

The trait method defaults to Unavailable and borrows the graph. Existing error
variants are Backpressure, Unavailable and Invalid(Failure). No clone or transfer
of the caller's graph authority is implied by success or refusal.

## Private author declarations

```rust
impl WasmAudioHost {
    pub(super) fn try_song_graph_checked(
        &mut self, lease: SongLeaseKey, graph: &GraphHandle,
    ) -> Result<(), SongSubmitError>;
}
fn graph(id: u32, kind: SongResourceKind) -> GraphHandle;
fn lease(id: u32, kind: SongResourceKind) -> SongLeaseKey;
fn host() -> WasmAudioHost;
fn take_records() -> Vec<Vec<u8>>;
fn encoded(lease: SongLeaseKey, graph: &GraphHandle) -> Vec<u8>;
fn callback(run: impl FnOnce());
impl BrowserRig {
    fn new() -> Self;
    fn push(&mut self, command: SongCommand);
    fn step(&mut self) -> Vec<SongHostAck>;
}
```

## Admission contracts

- Validate graph kind and graph-handle identity, structural encoding, finite
  controls, checked sizes and complete record bounds before publishing bytes.
  Use the existing codec's structural validation rather than duplicate rules.
  Actual configured capabilities, banks and memory adoption remain Engine checks.
- Payload maximum is arena::SLICE_BYTES (65536). Existing song wire adds 22 bytes;
  ABI framing adds 4. Every accepted record fits the existing 65600-byte inbox
  slot. Oversize, encoding failure and malformed graphs produce Invalid.
- An upload that cannot fit an empty configured fixed outbox produces Invalid.
  Capacity zero preserves the existing growable-outbox convention. The predicate
  uses checked arithmetic and has no queue or counter side effects.
- After permanent checks, refusal caused by occupied outbox space produces
  Backpressure. No partial record is published. Retrying the same borrowed graph
  after draining succeeds when actual space is sufficient.
- Success publishes exactly one unchanged wire record. It does not synthesize
  ResourceReady or bypass full-key reservation/bank/adoption validation.
- Existing submit_song_graph keeps its public Result<(), Failure> behavior.
  Its encoding may share a helper if legacy observable behavior is preserved.

## Tasks

### TASK-001: Trait and checked browser admission

**Status**: In Progress
**Parallelizable**: No

- [ ] Default unsupported host returns Unavailable without submission.
- [ ] Checked browser implementation and pure size predicate meet contracts.
- [ ] No wire changes, graph ownership loss or permanent Backpressure loops.

### TASK-002: Genuine queue, malformed and adoption witnesses

**Status**: In Progress
**Parallelizable**: No (depends on TASK-001)

- [ ] Occupied full outbox refuses with Backpressure, preserves existing bytes
      and borrowed Arc identity, then drain/retry emits the complete record.
- [ ] Empty outbox one byte too small, oversized payload and malformed graph
      refuse with Invalid and publish no partial bytes.
- [ ] Unsupported host returns Unavailable; valid wire equals existing encoder.
- [ ] Instrument, private FX, track and master graphs pass through actual ABI,
      ByteInbox and configured Engine to genuine ResourceReady receipts.
- [ ] Following record framing remains intact; callback allocation/deallocation
      probe surrounds actual Engine processing only and observes zero.

### TASK-003: Independent acceptance and consumer handoff

**Status**: In Progress
**Parallelizable**: No (depends on TASK-002)

- [ ] Held exact source hashes, scoped format and line limits.
- [ ] Native/browser compilation, strict Clippy and nonempty actual test inventory.
- [ ] Existing graph materialization, command/clock/upload regressions pass.
- [ ] Host preparation plan consumes the accepted typed interface.

## Progress log

### 2026-10-02: Source-grounded Ready declaration

Specialized read-only audit and root inspection confirmed actual payload/header/
inbox limits in ring/song.rs:81, ring.rs:399 and arena.rs:42. ABI push_record
currently exposes only a boolean and fixed empty capacity is private. Five paths
are sufficient for the companion and tests. This plan declares implementation;
no source or behavior acceptance is claimed. Full song mode remains unfinished.

### ROOT0517 coherent source hold — 2026-10-02

Implemented the typed companion and checked browser path with existing encoder
and bounded decoder, finite controls/nodes and exact handle identity checks.
Permanent payload/header/inbox/empty-outbox failures precede publishing; occupied
space alone reports Backpressure. Pure ABI predicate has no queue/counter effects.
Legacy submit/song_graph and wire remain unchanged. Six public fixtures written:
unsupported host, retained-pressure retries/wire/following record, permanent size,
malformed controls/structure/identity, genuine four-kind Engine adoption/seal/
cancel exact receipts with callback zeroalloc/zerodealloc, and predicate bounds.
Actual supported32768Hz fixture uses measured Engine allocation with bus_seconds40
to accommodate the full private region; no default/production capacity change.
Scoped rustfmt/check passed. No Cargo/nextest or behavioral execution by author.
Sources and this plan HELD for mandatory joined independent verification; all
behavioral criteria remain unchecked. Receipts under SONG-CHECKED-GRAPH-ADMISSION:
0001-before-source.json and0002-source-ready-held.json. Full song owner pending.
