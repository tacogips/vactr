# Activation correlation DTO and wire implementation plan

**Plan ID**: SONG-ACTIVATION-WIRE
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Scope and related plans

Add an exact requested-activation failure carrier without changing existing ACK
wire tags or inventing successful application. This is the first bounded phase
of [activation correlation](song-mode-activation-correlation.md); complete
behavior remains the parent and [runtime pipeline](song-mode-activation-pipeline.md)
obligation. DTO acceptance alone does not enable initial-onset playback.

- **Previous / Depends on**: [Host preparation](song-mode-host-preparation.md), joint source hold before checks; DTO source is independent of its eight-path author manifest.
- **Next**: [Activation pipeline](song-mode-activation-pipeline.md).

| Dependency | Required output | Current status |
|---|---|---|
| Original host cohort | Complete joint source hold before mandatory checker | Independent authoring; verification pending |
| Existing ACK codec | Tags0..13, epoch and opaque transport routing | Source reviewed |

## Exact future Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/song/routing.rs` | ActivationRejected DTO and exact epoch extraction | In Progress |
| `src/host/caps/song.rs` | Append tag14, checked payload encoding and exact decoding | In Progress |
| `tests/song_activation_codec.rs` | Real wire roundtrips, unchanged tags, malformed/truncated inputs | In Progress |

Three Rust paths plus own plan progress. Every source below1000. Additional
exhaustive consumers need root amendment before writes. Do not touch the987line
carrier test file; existing carrier suite remains regression evidence.

## Public declaration contract

```rust
pub enum SongHostAck {
    // Existing variants unchanged.
    ActivationRejected { activation: SongActivation, reason: SongRejectCode },
}
```

Preserve original requested epoch/frame on rejection. Applied continues reporting
actual application frame. Production generic non-activation failures retain their
existing carrier. Native/ByteInbox actual Activate failures are enabled only in
the dependent runtime phase; no fabricated receipt is emitted in this phase.

## Tasks

### TASK-001: Exact carrier and epoch contract
**Status**: In Progress
**Parallelizable**: Yes; independent three-path source ownership; checks await all holds.
- [ ] Audit exhaustive ACK consumers and preserve foreign/stale opaque forwarding.
- [ ] Add carrier without renumbering existing enum wire messages.

### TASK-002: Checked canonical codec
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this manifest.
- [ ] Encode/decode epoch, requested frame and valid reject reason exactly.
- [ ] Preserve existing framing, old tags, and malformed inputs and exact trailing-stream consumption.

### TASK-003: Independent evidence
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No; frozen sources and mandatory checker.
- [ ] Actual codec fixtures, old carrier regression and opaque host forwarding.
- [ ] Native/wasm/strictClippy, nonempty actual inventories, scoped format/line limits.

## Completion criteria

- [ ] All declared carrier/wire behavior verified; exact logs/hashes reconciled.
- [ ] No unowned writes, dependencies, Git changes or callback allocation.
- [ ] Parent exact-onset/failure/empty-song behavior remains explicitly pending runtime phase.

## Progress log

### 2026-10-02 — ROOT0538 bounded prerequisite split

Actual sorting authority adds a ninth module to the original unsplit correlation
scope. Separate the three DTO/codec modules from seven runtime modules while
preserving every parent requirement. Planning only; no source authorization.


### ROOT0539 independent additive carrier source release

Root's exhaustive consumer review confirmed current host receive and all caches
forward or ignore unrecognized variants; its eight-path author is disjoint from
this three-path DTO/codec phase. Adding the carrier and codec without emitting it
changes no existing activation semantics. Authoring may proceed concurrently
with owner fixtures under fresh exact baselines. Joint mandatory checker waits
for both complete source holds. Runtime/pipeline shared-owner edits still require
verified owner/wire handoff and remain unreleased. No existing tag or fixture is
weakened, and complete initial-onset/empty-song behavior is not claimed here.

### ROOT0540 actual stream framing refinement

Private fixture helpers: `message(epoch: u64, frame: u64, reason: SongRejectCode)
-> HostMsg` and `encoded(message: HostMsg) -> Vec<u8>`. Actual HostMsg::decode
is a stream-prefix API, and existing carriers expressly permit concatenated ACKs.
Preserve that contract. The new carrier consumes exactly19 bytes; trailing bytes
remain untouched for the next message. Fixtures require malformed/truncated and
unknown-reason rejection, and decode the following original Applied record from
the remaining stream. No strict-frame API or test-only rejection helper is added.
This supersedes the original trailing-rejection wording for actual framing.

### ROOT0539/ROOT0540 source-ready checkpoint

Written ActivationRejected with the original requested SongActivation and exact
epoch extraction. Actual codec appends tag14 with requested frame and checked
existing rejection codes; new payload is19 bytes. Tags0..13 and their payload
branches remain unchanged. No runtime producer or owner behavior was altered.

Written genuine canonical-codec fixtures:
- `activation_rejection_roundtrips_full_requested_identity_and_reasons`
- `old_song_ack_tags_and_payload_bytes_remain_unchanged`
- `activation_codec_rejects_truncation_invalid_reason_and_short_output`
- `activation_codec_preserves_exact_stream_consumption_and_trailing_records`

These use actual HostMsg encode/decode, including all original song ACK tags
with exact expected bytes and full-width new identity. Concatenated records
remain a stream; the returned19-byte consumed length leaves original Applied
bytes for the next decode. No strict helper or unrelated decoder API was added.

Scoped rustfmt/check succeeded and every touched Rust stays below1000. No author
Cargo or nextest executed. All behavior/check criteria remain unchecked pending
the mandatory joined checker; zero-onset and empty-song pipeline remain pending.
