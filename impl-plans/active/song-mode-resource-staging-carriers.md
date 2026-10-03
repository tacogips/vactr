# Song resource staging carriers implementation plan

**Status**: Completed
**Plan ID**: SONG-09PC
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Session target**: 1–3 sessions after root ownership approval
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent

Supply full lease identities, explicit frozen-cell and analysis declarations,
and ownership-preserving resource transport to SONG-09P. Current native
NativeInstall identifies only legacy u32 resource/generation; browser install
headers have the same limitation. Neither can certify a song epoch or kind.
The engine currently rejects all song commands as NotReady; this carrier phase
preserves that honest behavior while retaining owned uploads under pressure.

This prerequisite can implement before full SONG-08B acceptance. It does not
adopt resources, issue Ready/Applied/Muted, activate audio, allocate private
DSP branches or prove actual host capacity. Those remain SONG-09P and full
SONG-09/10 obligations. No new dependency, callback VM or callback allocation.

## Exact ownership manifest

```json
{
  "planId": "SONG-09PC",
  "planPath": "impl-plans/active/song-mode-resource-staging-carriers.md",
  "dependsOn": ["SONG-08A", "SONG-08C"],
  "writePaths": [
    "src/song/routing.rs", "src/dsp/ring.rs", "src/dsp/ring/song.rs", "src/host/caps.rs",
    "src/dsp/engine.rs", "src/host/wasm/messages.rs",
    "src/host/native/audio.rs", "tests/song_resource_carriers.rs",
    "impl-plans/active/song-mode-resource-staging-carriers.md"
  ],
  "sharedPaths": ["src/song/routing.rs", "src/dsp/ring.rs", "src/host/caps.rs",
    "src/dsp/engine.rs", "src/host/wasm/messages.rs", "src/host/native/audio.rs"],
  "readOnlyContracts": ["src/host/wire.rs", "src/dsp/cells.rs",
    "src/dsp/arena.rs", "src/song/snapshot/cells.rs", "tests/song_audio_wire.rs"]
}
```

Eight Rust paths and five tasks, including the declared ring/song.rs split. Root must acquire serial ownership before
release. Existing SongHostCapacities remains unchanged: do not add analysis
fields or migrate its explicit literals in prepare/components/public suites.
No cells.rs mutation: private epoch banks belong to the later stager.
Every touched Rust file must remain below1000 lines; a necessary split requires
a bounded manifest amendment before writing the additional module.

## Source interfaces and compile consumers

- routing.rs owns SongCommand/SongHostAck, epoch/valid methods and receipt ledger.
- caps.rs owns song codec match arms and AudioHost capability defaults.
- ring.rs owns NativeRecord/Record/Garbage, NativeControlSource mapping,
  ByteInbox framing, install-credit classification and byte record encoders.
- engine.rs exhaustively consumes Record and retains critical song receipts.
- native/audio.rs owns native enqueue and evaluator-thread garbage destruction.
- wasm/messages.rs owns bounded browser resource upload and receipt consumption.
- wire.rs delegates song serialization to caps.rs and is a read-only regression
  input. Existing maximum command/event sizes remain sufficient below.

## Typed declarations

### src/song/routing.rs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongLeaseKey {
    pub epoch: SnapshotEpoch,
    pub resource: SongResourceRef,
    pub kind: SongResourceKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongAnalysisCapacity { pub slots: u32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongCapacityReport {
    pub epoch: SnapshotEpoch,
    pub serial: u64,
    pub available: SongHostCapacities,
    pub analysis: SongAnalysisCapacity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongStagePreparation {
    pub preparation: SongPreparation,
    pub analysis_required: SongAnalysisCapacity,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SongCellInit {
    pub lease: SongLeaseKey,
    pub cell: CellId,
    pub value: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongAnalysisReservation {
    pub lease: SongLeaseKey,
    pub slots: u32,
}
```

Append resource kinds ControlCells=7 and AnalysisBank=8; tags5/6 remain reserved invalid holes. Existing kinds0–4
stay frozen. SnapshotEpoch is u64; resource id and generation are u32. The
complete triple plus kind is ownership identity; no truncation or reuse while
any retained lease, pending upload, receipt or garbage reference survives.

Append these SongCommand variants:

```rust
BeginStaging(SongStagePreparation),
InitCells(SongCellInit),
ReserveAnalysis(SongAnalysisReservation),
CancelPreparation(SnapshotEpoch),
CancelLease(SongLeaseKey),
RequestCapacity(SnapshotEpoch),
```

Append these SongHostAck variants for subsequent genuine staging consumers:

```rust
CapacityReport(SongCapacityReport),
LeaseReturned(SongLeaseKey),
PreparationCancelled(SnapshotEpoch),
SliceAccepted { lease: SongLeaseKey, offset: u32 },
```

Update epoch/valid/receipt methods and codec exhaustive matches. InitCells
requires kind ControlCells and a finite value; ReserveAnalysis requires kind
AnalysisBank. Analysis slots0 is valid as an empty declaration. Commands
preserve existing sample-rate validation. Invalid resource-kind tags reject.
The carrier-only engine emits Rejected(NotReady/Malformed), never the success
variants above. Cancellation success requires later stager ownership release.
Legacy ResourceReady/ResourceRetired remain unchanged. The triple
(epoch,id,generation) must be globally unique across resource kinds while any
lease, pending upload or delayed receipt exists. The later stager resolves that
triple against its retained expected-kind table; no same-triple/different-kind
reservation is valid. Full-key return/slice messages still compare kind.
Carrier rejection cannot create this allocator proof; SONG-09P must enforce it
before emitting any resource success acknowledgment. Required fixtures retain
duplicate/wrong-kind attempts and a delayed old receipt after cancellation; the
old receipt cannot release or ready a future differently assigned lease.

### src/dsp/ring/song.rs — cohesive new carriers; ring.rs integration

```rust
pub struct NativeSongInstall {
    pub lease: SongLeaseKey,
    pub payload: NativeInstall,
}
// Appended enum variants, preserving existing variants:
// NativeRecord::SongInstall(NativeSongInstall)
// Record::SongNative(NativeSongInstall)
// Garbage::SongInstall(NativeSongInstall)
// Record::SongGraph { lease: SongLeaseKey, bytes: &'a [u8] }
// Record::SongSampleBegin {
//     lease: SongLeaseKey, frames: u32, channels: u8, rate: u32,
// }
// Record::SongSlice { lease: SongLeaseKey, offset: u32, data: &'a [u8] }

pub fn encode_song_graph_record(
    lease: SongLeaseKey, bytes: &[u8], out: &mut [u8],
) -> usize;
pub fn encode_song_sample_begin(
    lease: SongLeaseKey, frames: u32, channels: u8, rate: u32,
    out: &mut [u8],
) -> usize;
pub fn encode_song_slice(
    lease: SongLeaseKey, offset: u32, samples: &[f32], out: &mut [u8],
) -> usize;
```

The new module owns NativeSongInstall, all new byte encoders/checked decoding
and tag/header constants. ring.rs declares `mod song;` and publicly reexports
NativeSongInstall plus the three encoder functions at their specified existing
ring namespace; crate-private decoder/view helpers are reexported only inside
ring. ring.rs retains Record/NativeRecord/Garbage variants, mapping, ByteInbox
integration and install-credit classification. The new module borrows byte views
and NativeInstall through its parent, without allocating during decoding.
Neither ring.rs nor its child may exceed999 lines. Engine.rs is currently854
lines; bounded owner retry/record dispatch must fit below1000, using cohesive
carrier helper methods within these owned modules. A ninth file is not implied;
request root's prior manifest amendment if the actual implementation cannot fit.

NativeInstall remains unchanged. Its embedded resource/gen must exactly equal
lease.resource; Instrument pairs only with Template, PrivateFx/Track/Master
with Bus (master flag agrees with kind), Sample with Sample. ControlCells and
AnalysisBank are POD declarations, not native graph/sample payloads.
No new native wrapper allocation: transfer existing Box/Arc ownership inline.
NativeControlSource maps the appended native variant; Record::install_bytes
and ByteInbox classification charge actual graph/slice byte payload lengths.
Native ownership handover itself copies no PCM and has byte cost0.

## Frozen wire layout

Little-endian fields; lengths include tags. Command common header is existing
0x1B, subtag:u8, epoch:u64 =10 bytes. Capacity payload is the unchanged52 bytes
(seven u32 and three u64). Full key is epoch:u64/id:u32/gen:u32/kind:u8 =17.

| Record | Appended tag | Length/fields after common header |
|---|---|---|
| BeginStaging | command10 | 74 total: branches4/resources4/capacities52/analysis_slots4 |
| InitCells | command11 | 27 total: id4/gen4/kind1/cell4/value4 |
| ReserveAnalysis | command12 | 23 total: id4/gen4/kind1/slots4 |
| CancelPreparation | command13 | 10 total |
| CancelLease | command14 | 19 total: id4/gen4/kind1 |
| RequestCapacity | command15 | 10 total |
| LeaseReturned | ack6 | 19 total |
| PreparationCancelled | ack7 | 10 total |
| SliceAccepted | ack8 | 23 total: id4/gen4/kind1/offset4 |
| CapacityReport | ack9 | 74 total: serial8/capacities52/analysis_slots4 |
| SongGraph | outer0x1C | 22-byte header: tag1/key17/byte_len4, then bytes |
| SongSampleBegin | outer0x1D | 27 total: tag1/key17/frames4/channels1/rate4 |
| SongSlice | outer0x1E | 26-byte header: tag1/key17/float_offset4/float_count4, then f32 bytes |

Existing command subtags0–9, ack0–5, outer0x1B/0x49 and all legacy tags stay
frozen. SONG_ACK_MAX_LEN becomes74 for CapacityReport; existing HostMsg and ack transport buffers derive their sizes from it. The report is descriptive and never Ready. Actual monotonic serial issuance and measured available capacity publication remain SONG-09P, while this engine rejects RequestCapacity as NotReady. Keep command
maximum at existing event-derived maximum; assert it covers74. INBOX_SLOT_BYTES
already has64 bytes overhead over SLICE_BYTES; assert all new headers fit.
Reject truncation, trailing bytes, invalid kinds, length multiplication/offset
addition overflow, wrong payload kinds, nonfinite values and invalid channels.
Graph decoding enforces exact declared payload length. Slice count is bounded
by SLICE_BYTES/4. Rejected framing consumes only its own inbox slot and retains
the following independently framed valid record. Do not collapse full keys to
legacy graph/sample IDs, including during budget classification.

### src/host/caps.rs and concrete adapters

```rust
pub trait AudioHost {
    // Existing methods retained.
    fn submit_song_native(
        &mut self, install: NativeSongInstall,
    ) -> Result<(), NativeSongInstall>;
    fn submit_song_graph(
        &mut self, lease: SongLeaseKey, graph: &GraphHandle,
    ) -> Result<(), Failure>;
    fn submit_song_sample(
        &mut self, lease: SongLeaseKey, data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>>;
}
```

All additions have default unsupported behavior preserving the owned argument,
so unrelated AudioHost implementations continue compiling without source edits.
Default borrowed graph submission returns HostUnavailable. NativeAudioHost
implements native/sample submission via controls.push with exact payload
returned on failure; it must not reuse post_record, which discards errors.
WasmAudioHost implements graph/sample byte submission with the full-key encoder.
Its existing sender-paced sample queue retains full keys and waits for matching
SliceAccepted; ordinary SliceOk can never advance a song upload. Failed queue
admission returns the original Arc. Existing legacy upload states/acks remain
separate. Native graph building and browser graph encoding allocate only on the
control side. No successful enqueue is a resource adoption acknowledgment.

### src/dsp/engine.rs — carrier-only bounded return

Add one pending NativeSongInstall owner in Engine and reuse its existing bounded
critical receipt slot. Before consuming another control record, retry pending
Garbage::SongInstall handover. If no garbage channel exists or its ring is full,
retain the exact owner and stop control consumption, while legacy audio keeps
rendering. Never drop/leak Box/Arc on callback, or overwrite a pending owner.
Malformed native payload also follows this ownership-return path. Rejected
receipt uses exact epoch; returning ownership is not Ready or retirement.
Only later09P can emit LeaseReturned after actual matching return, and successful
cancellation only after all pending payloads/leases are released.

Carrier browser records reject explicitly without touching legacy resources or
private bank state. No callbacks, sample decoding, graph installation, cell
mutation or active graph replacement occurs in this phase. Resource success
acks declared above are tested as serialization only, not emitted by stubs.

## Tasks and dependency table

| Task | Deliverables | Depends on | Parallelizable | Status |
|---|---|---|---|---|
| TASK001 Typed lease/POD | routing.rs | approved ownership | No | Completed |
| TASK002 Wire and inbox | caps.rs/ring.rs/ring/song.rs | TASK001 | No | Completed |
| TASK003 Ownership consumer | engine.rs/native audio | TASK002 | No | Completed |
| TASK004 Browser sender | wasm/messages.rs | TASK002 | Yes with TASK003 | Completed |
| TASK005 Verification | new carrier tests + read-only wire regression, independent checker | TASK003/004 | No | Completed |

### TASK001 — complete typed carrier contract

- [x] Add append-only kinds/commands/acks and exact numeric tags above.
- [x] Preserve full epoch/u32 generation and existing capacity layout.
- [x] Validate finite scalar/kind/embedded resource identity without fallback.

### TASK002 — exact framing and native/byte parity

- [x] Update all named exhaustive codec/Record mapping and budget consumers.
- [x] Preserve legacy wire source/bytes unchanged; test new kind7/8 and invalid5/6/9 in song_resource_carriers.
- [x] Native/ByteInbox produce identical full keys and scalar values.
- [x] Assert fixed/max encoded lengths; hostile lengths never overflow or allocate.

### TASK003 — return retained native ownership

- [x] Full control enqueue returns exact owned object on control thread.
- [x] Full garbage/ack rings retain pending owner and receipt across callbacks.
- [x] No callback destruction, allocation, locking, cell writes or audio changes.
- [x] Recover every object after pressure clears; old installed graph keeps sounding.

### TASK004 — full-key browser upload

- [x] Typed graph and bounded paced sample upload preserve lease through each chunk.
- [x] Wrong/stale/legacy slice acks cannot advance a song upload.
- [x] Queue-full failures preserve caller ownership; legacy upload remains unchanged.

### TASK005 — independent verification and handoff

- [x] Fresh unfiltered song_resource_carriers and song_audio_wire inventories.
- [x] Full-width epochs, u32MAX identities, all kinds and every appended variant.
- [x] Every truncated prefix, invalid kind/value/length and following valid record.
- [x] Exact native control-return and callback garbage/ack pressure tests.
- [x] Duplicate triples with conflicting kinds reject; delayed/foreign kind receipts never satisfy another lease; retain old acknowledgment bytes.
- [x] Real browser sender+ByteInbox receiver identity/FIFO tests, not codec alone.
- [x] Quiet native/host-wasm checks, strict all-target Clippy, scoped fmt/diff.
- [x] Nonzero unfiltered carrier/wire and relevant legacy native/browser/engine tests pass under the root-approved scoped matrix (ROOT0215); no nextest execution is claimed.
- [x] No-allocation callback test and no-fallback/no-false-success assertions.
- [x] Required independent checker verifies source seals and terminal processes.
- [x] All touched Rust under1000 lines; no new dependencies or undeclared writes.

## Completion and downstream boundary

A completed carrier plan proves transport and retained rejection ownership only.
SONG-09P must supply configured physical geometry, actual capacities, private
cells/analysis, silent adoption, Ready after Seal, and truthful cancellation.
Full SONG-09 must add activation/frame timing/private DSP/detector causality and
pressure-safe retirement; SONG-10 must supply real host providers. Full08/08B
acceptance still requires joined actual per-slot/current-retiring lease evidence.

## Related plans

- Previous: song-mode-audio-contracts.md and song-mode-frozen-cell-inventory.md.
- Next: song-mode-resource-staging.md, then song-mode-dsp-routing.md.
- Implementation ordering differs from final acceptance; ROOT0186 retains the
  complete final acceptance contracts while admitting bounded prerequisites.

## Progress log

### Session: 2026-10-02 — document-only draft

Inspected actual native/byte consumers and existing codec tags/sizes. Included
native audio and the cohesive ring/song.rs split within eight paths so the ownership-return capability is concretely
implemented, retained capacity layout, and declared explicit browser slice
acknowledgments. Planning only; no Rust, Cargo, commits or existing-plan edits.

### Session: 2026-10-02 — ROOT0203 implementation release

Recorded fresh0007 baselines before code. Implementing append-only typed carriers and retained upload rejection in the exact eight-path manifest. Existing routing ownership is transferred serially; geometry remains disjoint. Cargo waits for both authors to hold. No resource adoption or success acknowledgment is authorized by this carrier phase.

### Session: 2026-10-02 — Prior capacity carrier amendment ROOT0205

Declared full-u64 serial CapacityReport and RequestCapacity before source edits, within the unchanged eight Rust paths. Ack maximum74 propagates through derived HostMsg buffers. Exact-length/truncation/all-capacity/native-byte fixtures are required. Actual serial issuance and physical capacity publication remain09P; carrier-only engine rejects requests as NotReady. Immutable0013 records document before/after hashes.

### Session: 2026-10-02 — TASK001-004 written; first joined compile hold

Immutable0007-0014 capture each declaration/source batch. All eight source paths are held after scoped formatting, with no Cargo execution. Complete lease POD/commands/acks, checked inbox framing and byte credits, native allocation-preserving enqueue/rejection, one retained pending callback owner and critical receipt, and full-key paced browser sample queue are written. Browser reservations persist beyond final chunk until matching return; stale/foreign-kind/legacy slice receipts cannot advance or release them. Control polling retries unsent outbox uploads without changing in-flight ownership. CapacityRequest/Report follow ROOT0205 and report length74; actual measured capacity publication remains09P and engine rejects requests NotReady.

Eighteen public fixture functions are written, not executed: exact wire lengths/truncations/full-width fields, all kinds and reserved holes, malformed slot recovery, actual garbage/ack pressure and no callback allocation/destruction, exact control enqueue return, retained legacy voice rendering, real browser sender→ByteInbox/pacing/outbox pressure/queue limits, and capacity request/report parity. The native test compiles actual portable browser abi/messages via read-only path modules and crate reexports; no modeled sender substitute. Existing song_audio_wire remains unchanged. No behavioral or phase clearance is claimed. Root must release combined checker only after disjoint geometry author also holds.

### Session: 2026-10-02 — Retained browser sample capacity repair

ROOT0209 and immutable0016 refine remaining_song_sample_limits to subtract checked combined legacy and retained full-key song resource/PCM reservations. Existing actual browser fixture now asserts capacity before upload, after final SliceAccepted, after foreign-kind return, matching LeaseReturned, addressed cancellation, and legacy retirement. No Cargo; source stays held pending root coordinated checker. Carrier/source admission no longer overreports retained PCM or slots.

The same0017 refinement includes legacy install admission: checked existing legacy+song+incoming PCM totals prevent legacy uploads consuming retained song reservations. The existing actual sender fixture probes a competing legacy upload before and after exact full-key return, retaining legacy diagnostic and queue behavior.

### Session: 2026-10-02 — Independent scoped acceptance ROOT0215

Completed carrier transport and retained rejection ownership only. Independent003 passed18 carrier and21 unchanged wire fixtures, plus native/host-wasm/strict all-target Clippy; its separate geometry suite failed3 of32 and remains08D work. Root accepted this prerequisite separately using003 evidence SHA `e6a008f9713d7c3344a0e09f5b08762602cdd992f6459fb8fd6596a9b8a387f8` and continuation `/tmp/vactr-song09pc-continuation-001/final-results.json` SHA `8ea49555aeec36a87ddee12ffc1ecc9c9413914cdec176180e16fbd57bb8fe49`. Continuation original90257 terminated0;20 engine/native regressions and scoped formatting/diff passed. Total59 distinct passing fixtures are18+21+20; no repeats or failed geometry cases are added. Root verified33 source/plan hashes and all logs. No author Cargo or nextest execution is represented as completed.

The actual portable browser fixture revealed native ABI symbol interposition in failed002 inventory54005/101. The independently accepted [ABI child](song-mode-wasm-abi-exports.md) preserves Rust signatures/bodies and exports raw names only on wasm32; all actual sender fixtures remain. Constructor and ownership semantics are unchanged. Immutable0018 records this document-only completion and unchanged source hashes. Plans remain active pending final integration archival. Actual measured reports, private cells/analysis, silent adoption/Seal Ready belong09P; audible routing, activation, hosts and export remain pending.
