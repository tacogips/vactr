# Explicit song graph bank binding implementation plan

**Status**: In Progress
**Plan ID**: SONG-09PB
**Created**: 2026-10-02
**Design Reference**: [Private routing](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent

Native and browser graphs need an explicit association to their epoch-owned
control and analysis leases. Physical global analyzer remapping cannot represent
two full logical65536-slot banks because runtime analyzer IDs clamp to65535.
Keep logical IDs and ordered overlapping writers; later render selects the exact
owner-local bank slice from the authenticated binding.

## Exact ownership

Rust write paths: `src/song/routing.rs`, `src/host/caps.rs`,
`tests/song_graph_banks.rs` (new). Plan write path: this file.
Read-only: ring.rs, host/wire.rs and existing carrier/wire tests.
No extra dependency or callback allocation. This is a separate bounded carrier
prerequisite; SONG-09P consumes it in its already-owned engine/song.rs.
ROOT0219 authorizes implementation after owner records exact pre-edit intent.

## Typed declarations

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongGraphBanks {
    pub graph: SongLeaseKey,
    pub controls: Option<SongLeaseKey>,
    pub analysis: Option<SongLeaseKey>,
}
```

Append `BindGraphBanks(SongGraphBanks)` to SongCommand. Its epoch is graph.epoch.
Validity requires graph kind Instrument/PrivateFx/Track/Master; optional controls
kind ControlCells and analysis kind AnalysisBank; every present key shares the
exact graph epoch. Full resource id/generation and epoch widths are retained.

## Wire contract

Append song command subtag16; legacy tags remain frozen. Common command tag,
subtag and epoch occupy10 bytes; graph key tail resource/generation/kind adds9.
Each optional bank uses strict boolean presence plus9-byte key tail; its epoch
is the common epoch after native validity verification. Lengths are21/30/39.
Existing SONG_COMMAND_MAX_LEN must remain sufficient; no larger ACK or buffer.
Malformed flags, kind holes, mismatched native epochs, incomplete tails and
trailing record framing must never normalize into accepted ownership.

## Tasks

| Task | Deliverable | Status | Dependency |
| --- | --- | --- | --- |
| BANK001 | Typed command validity/epoch and checked codec | Written; verification pending | ROOT0219 |
| BANK002 | Full-width native/byte framing fixtures and held receipt | Written; verification pending | BANK001 |
| BANK003 | Independent joined staging/carrier verification | Ready | BANK002 +09P hold |

## Completion criteria

- [ ] Full-width graph/control/analysis identities and optional forms roundtrip.
- [ ] Invalid epochs/kinds/flags and every truncation reject without ownership loss.
- [ ] Native and ByteInbox yield identical POD commands; following valid record survives malformed frame.
- [ ] Native/browser compile, strict Clippy, scoped formatting and affected tests pass.
- [ ] Complete pre/post source hashes and command/log evidence retained.

## Downstream behavior

09P owns actual binding storage. Binding requires existing authenticated,
nonretiring leases in the same preparation; duplicate/conflicting binding rejects.
Graph adoption validates all remapped control sites within initialized bound
control ownership and all logical analyzer ranges within the explicitly bound
analysis bank. Missing banks are valid only when the graph has no such reads or
writers. Bank metadata alone does not activate private audio. Full09 rendering
selects the exact bank slice and proves audible isolation.

## Related plans

- [Actual staging](song-mode-resource-staging.md).
- [Carrier foundation](song-mode-resource-staging-carriers.md).
- [Private audio](song-mode-dsp-routing.md).

## Progress log

### 2026-10-02 — Association gap found during real staging

ROOT0219 records prior intent. Existing graph uploads identify graph lease only;
existing analysis reservations provide size and bank lease without graph owner.
An explicit binding avoids ambiguous lookup and preserves multi-owner logical
analysis semantics. No test execution or private audio completion is claimed.

### Exact codec and fixture declarations (ROOT0219)
Before Rust edits, retain the typed SongGraphBanks declaration above and append
only BindGraphBanks; command epoch/valid methods consume that exact full key.
Private helpers in caps::song_codec:
```rust
fn put_optional_key(w: &mut Codec<'_>, key: Option<SongLeaseKey>);
fn get_optional_key(r: &mut Decode<'_>, epoch: SnapshotEpoch)
    -> Result<Option<SongLeaseKey>, WireError>;
```
The strict boolean decoder rejects flags other than0/1; present keys decode
with the common full epoch. Native validity runs before encoding and after
decoding. Actual tests declare:
```rust
fn graph_bank_options_roundtrip_full_width_native_and_byte_records();
fn graph_bank_native_epoch_and_kind_rules_reject_invalid_ownership();
fn graph_bank_bytes_reject_flags_kinds_truncation_and_recover_following_record();
fn graph_bank_append_preserves_existing_command_tags_and_maximum_buffer();
```
No author Cargo; coordinated mandatory checker follows all-author hold.

Private test helpers before fixture implementation:
```rust
fn key(kind: SongResourceKind) -> SongLeaseKey;
fn binding(kind: SongResourceKind, controls: bool, analysis: bool) -> SongCommand;
fn encoded(command: SongCommand) -> Vec<u8>;
fn budget() -> Budget;
```

### 2026-10-02 — BANK001/002 held for joined checker
Exact pre-edit receipts0001 through0003 preceded declared Rust/fixture changes.
SongCommand appends BindGraphBanks subtag16; epoch remains graph.epoch and
validity checks graph kind, optional bank roles and common epoch. Codec uses
strict flags and existing full key tails; old tags, kind holes and maximum
buffers remain unchanged. Four unexecuted fixtures cover all graph kinds and
optional forms, full u64/u32 widths, 21/30/39-byte sizes, every truncation,
wrong native epochs/kinds, invalid byte flags/holes/roles, and actual native
SPSC versus ByteInbox parity. Malformed and trailing byte records must leave
the following valid binding available. Existing command byte tags are asserted.
Only the three declared Rust paths and this own plan changed. Scoped rustfmt
completed; all files are below1000. No author Cargo or test execution; all paths
are held for coordinated mandatory checker and09P actual binding integration.
No bank metadata alone proves activation, audible isolation or staging success.

### ROOT0228 actual maximum-buffer fixture repair
Original joined retry31274/101 retained the sole strict Clippy finding:
assertion_on_constants at test216. Replace the constant assertion with an
actual both-bank command encode into fixed SONG_COMMAND_MAX_LEN storage,
asserting nonzero consumption, exact39 bytes and equality to the independent
encoded bytes. All smaller-buffer rejection and append-only tag assertions
remain. No production changes or author Cargo; reseal test and own plan only.
Geometry and all other source/plan holds remain in effect.
