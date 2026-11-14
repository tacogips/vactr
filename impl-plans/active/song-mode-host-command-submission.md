# Checked native and browser song command submission implementation plan

**Status**: Completed
**Plan ID**: SONG-10-COMMANDS
**Plan Path**: impl-plans/active/song-mode-host-command-submission.md
**Created / Last Updated**: 2026-10-02
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song design](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Intent and scope

Implement checked admission of every existing SongCommand on the actual native
control queue and browser byte transport, while preserving exact ownership on
refusal. This is a child deliverable of SONG-10, not complete preparation, finite
transport or playback. Source-grounded proposal: tmp/song-mode-riela/SONG-10/
0002-host-transport-proposal.md; ROOT0299 releases documents only.

AudioHost::post_song currently delegates to unit-returning post. Native post_record
reports an overflow after refused queue insertion; browser HostState::post ignores
push_record's result. A finite owner must distinguish admitted commands from
backpressure without waiting for a command that never entered the audio queue.
Existing graph/sample APIs already expose particular refusal/ownership semantics;
retain those contracts and genuine partial-upload pacing.

## Manifest

```json
{
  "planId": "SONG-10-COMMANDS",
  "planPath": "impl-plans/active/song-mode-host-command-submission.md",
  "dependsOn": ["SONG-08A", "SONG-09PC", "SONG-09PB"],
  "writePaths": [
    "src/host/caps.rs",
    "src/host/caps/song.rs",
    "src/host/native/audio.rs",
    "src/host/native/audio/song.rs",
    "src/host/wasm/messages.rs",
    "src/host/wasm/messages/song.rs",
    "tests/song_hosts.rs",
    "impl-plans/active/song-mode-host-command-submission.md"
  ],
  "sharedPaths": [
    "src/host/caps.rs",
    "src/host/native/audio.rs",
    "src/host/wasm/messages.rs",
    "tests/song_hosts.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/host/caps.rs",
      "intendedEdit": "Serial child ownership after current DSP/geometry held checks; parent resumes only after child independent clearance and fresh root release."
    },
    {
      "path": "src/host/native/audio.rs",
      "intendedEdit": "Move cohesive song upload methods into owned child, preserve capacity helper; no parent concurrent writes."
    },
    {
      "path": "src/host/wasm/messages.rs",
      "intendedEdit": "Move cohesive song sender/pacing methods into owned child; parent remains read-only until independent join."
    },
    {
      "path": "tests/song_hosts.rs",
      "intendedEdit": "Checked submission fixtures first; parent adds broader actual preparation/playback fixtures only in a later serial release."
    }
  ]
}
```

## Dependencies and serial release

| Dependency | Required output | Implementation gate |
|---|---|---|
| SONG-08A | actual bounded FIFO polling / unsupported no consumption | verified baseline retained |
| SONG-09PC /09PB | stable existing POD, graph-bank and full-key upload carriers | retain established actual Native/ByteInbox fixtures |
| Current SONG-09 and08D mutation wave | sealed sources and mandatory joined checker terminal result | no source edits before fresh root ownership release |
| Parent SONG-10 | source-grounded contract review and serial delegation | no simultaneous shared path writers |

- **Parent**: [Host adapters](song-mode-host-adapters.md).
- **Next**: exact-clock and complete preparation ownership children, then parent10
  and [finite transport](song-mode-transport.md).

This child may be implemented on frozen verified carriers after current mandatory
joined checks. Full SONG-09 completion is not an implementation prerequisite for
these sender methods, but remains a parent full-playback acceptance prerequisite.
Ready means this document is reviewable; ROOT0299 does not authorize source work.
Future source release names exact current hashes and the seven owned Rust paths.

## Execution and preservation contract

Use required rust-coding and check-and-test-after-modify agents. Record immutable
intent and after/seal records with design, child, parent and each owned source hash
under tmp/song-mode-riela/SONG-10-COMMANDS before and after every batch. Compare
fresh hashes immediately before editing; stop on unexpected drift. Do not restore
stale complete files. Preserve unrelated dirty source, session and canvas work.

No dependencies, Cargo.lock changes, Git/index/archive operations, broad formatting
or new source paths. Every touched Rust file stays below1000 lines. Parent caps and
native modules already approach the threshold: declare the cohesive child modules
listed here before adding substantial implementation. If a child will reach1000,
request a prior bounded manifest amendment before growth; no implicit extra file.
Do not put the later complete preparation state machine in codec/DTO song.rs.

Quiet Cargo through mise: CARGO_TERM_QUIET=true. Nextest additionally uses
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
NEXTEST_HIDE_PROGRESS_BAR=1. Record exact commands, raw logs, nonempty qualified
inventories/actual executed identities, original process handles and terminal exits.
Timeout is not restart permission. Stop/report concrete failure before owner repair.

## Modules and declaration contracts

| File | Intended change | Status |
|---|---|---|
| src/host/caps.rs | declare/reexport child and checked AudioHost capability; preserve legacy APIs | Completed |
| src/host/caps/song.rs | relocate existing song codec; typed checked command refusal DTOs | Completed |
| src/host/native/audio.rs | child declaration/delegation for song-specific methods | Completed |
| src/host/native/audio/song.rs | checked native POD admission and existing graph/sample ownership transfer | Completed |
| src/host/wasm/messages.rs | child declaration/delegation for song sender/pacing | Completed |
| src/host/wasm/messages/song.rs | actual checked encoded POD push and preserved sample pacing | Completed |
| tests/song_hosts.rs | actual Native and browser byte-pressure/ownership/compatibility fixtures | Completed |

```rust
pub enum SongSubmitError {
    Backpressure,
    Unavailable,
    Invalid(Failure),
}
pub struct SongCommandRefusal {
    pub command: SongCommand,
    pub error: SongSubmitError,
}
pub trait AudioHost {
    fn try_song_command(&mut self, command: SongCommand)
        -> Result<(), SongCommandRefusal>;
    fn submit_song_native(&mut self, install: NativeSongInstall)
        -> Result<(), NativeSongInstall>; // retain existing declaration
    fn submit_song_graph(&mut self, lease: SongLeaseKey, graph: &GraphHandle)
        -> Result<(), Failure>; // retain existing declaration
    fn submit_song_sample(&mut self, lease: SongLeaseKey, data: Arc<SampleData>)
        -> Result<(), Arc<SampleData>>; // retain existing declaration
}
```

Imports are existing crate types: routing::SongCommand/SongLeaseKey,
dsp::ring::NativeSongInstall, host::caps::GraphHandle/SampleData,
std::sync::Arc, vm::fail::Failure. Declaration signatures contain no bodies.
Existing AudioHost trait method definitions remain in caps.rs; child reexports
preserve existing encode/decode import paths. Existing native/wasm parents declare
the child without host/mod.rs changes. Unsupported implementations use a default
exact-command Unavailable refusal and consume no unchecked legacy transport.
No new wire tags, command variants, capacity reports, clock protocol or DSP changes.

### Acceptance and backpressure

- Ok means one complete POD record is admitted once into the real bounded sender.
  It means neither resources Ready nor actual activation/mute Applied.
- The method covers every current SongCommand, including activation, mute, event,
  release, endpoints, staging, cell/analysis/bank commands, reservation, sealing,
  cancellation and explicit capacity query. Match the current exhaustive inventory
  at implementation baseline rather than preserving a stale hardcoded count.
- Validate current command encoding/validity before queue mutation. Invalid command
  returns the exact POD and control-thread Failure; audio-side semantic stale epoch,
  wrong resource or NotReady rejection remains an asynchronous existing ACK.
- Native full control queue returns unchanged command and Backpressure; the method
  must not call drop-reporting post_record and then report successful admission.
  Keep native capacity-query origin admission/accounting correct for public requests.
  The existing capacity cache may reserve its query owner only after actual enqueue;
  retry must not create duplicate correlation entries.
- Browser false push_record returns exact POD and Backpressure. An unavailable
  transport is explicit; a complete command record must not be partially committed.
  Validate byte transport's actual full-record transaction behavior in tests. If an
  inspected existing writer cannot guarantee that, record the concrete source blocker
  and obtain prior bounded ownership amendment; do not invent success or edit ABI
  outside this manifest.
- The caller retains a refused command for bounded retry; no new unbounded internal
  command stash and no allocation/VM/lock added to callback. Only accepted records
  advance the caller's submission cursor. Failed retry remains the same refusal.
- Existing unchecked post_song/post/control/send remain legacy compatible. Parent
  finite owners must use try_song_command rather than unchecked post_song.

### Existing upload ownership is preserved

Native submit_song_native returns the original NativeSongInstall with Box/Arc
allocation and complete lease on enqueue refusal. Unsupported host returns it
unchanged. Borrowed graph submission retains caller graph identity on Failure;
sample refusal returns the original Arc allocation. Preserve all lease generation/
kind associations and closed source bytes; no active loader reads in uploads.

The checked all-or-none admission statement above applies to one POD command, not
an entire graph/sample upload. Browser upload state can already contain accepted
steps/slices. Relocation preserves exact full-key SliceAccepted pacing, current
resumption offset, started upload ownership, and lease retention through return.
No duplicate rebegin/replay on refusal, no legacy SliceOk or foreign receipt release,
no reset of accepted cursor, and cancellation uses the exact started lease.
Existing return/error types are retained; do not silently promise stronger atomic
rollback or lose partially staged ownership. Existing capacity providers remain
read-only and must not leak internal reports into legacy Runtime.

## Tasks

### TASK-001: Cohesive splits and checked contract

**Status**: Completed
**Parallelizable**: No — acquire exact shared source ownership.
**Deliverables**: declarations, three child modules, immutable intent/post hashes.

- [x] Reconcile current source/types/import paths and immutable parent delegation.
- [x] Move existing song codec/native upload/browser pacing cohesively, preserve semantics.
- [x] Define typed command refusal and default unsupported behavior.
- [x] Every touched source below1000; no implicit extra path or preparation-owner expansion.

### TASK-002: Actual native and browser checked admission

**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No — same parent/child modules.
**Deliverables**: checked AudioHost methods for actual NativeAudioHost/WasmAudioHost.

- [x] Real full-queue refusal returns exact POD; successful retry admits once.
- [x] Preserve public/internal capacity query correlation and existing graph/sample ownership.
- [x] Actual byte transport records remain complete and callbacks unchanged.
- [x] No false Ready/Applied, no clock/preparation/transport completion claim.

### TASK-003: Genuine behavioral evidence and sealed join

**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No — final stable sources and checker.
**Deliverables**: actual fixtures/logs/inventories and immutable final source seal.

- [x] Actual native control ring full→refused→drained→retried command for every variant;
  preserve epoch/u32 generation/fullwidth frame and valid Event float bits.
- [x] Actual browser ABI/messages sender→bounded bytes→ByteInbox/Engine parity;
  fill transport, prove refusal leaves no partial record and following retry decodes once.
- [x] Native Box Template/BusTemplate and Arc PCM allocation addresses remain identical
  on rejection; success transfers once and exact critical ownership returns remain intact.
- [x] Browser real paced sample first/partial/final slices, foreign/stale receipts,
  cancellation and legacy SliceOk; no cursor reset/duplicate upload/released claim.
- [x] Unsupported default consumes nothing; invalid encoding cannot mutate queue.
- [x] Legacy actual song_audio_wire/resource_carriers/banks/capacity-routing regressions
  and affected native adapter tests preserve behavior, including queue pressure.
- [x] Mandatory independent checker logs actual nonzero names and terminal processes.

## Future verification — not executed during planning

| Gate | Required actual evidence |
|---|---|
| CARGO_TERM_QUIET=true mise exec -- cargo check | stable default native compile |
| same environment cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm | actual browser adapter compile |
| same environment cargo clippy --all-targets -- -D warnings | strict joined warnings, full diagnostics retained |
| cargo test --test song_hosts -- --list and unfiltered run | fresh qualified names equal actual nonzero test identities |
| affected carrier/wire/bank/capacity/native fixtures | no allocation/ownership/pacing/correlation regressions |
| prescribed quiet nextest for binary(song_hosts) | actual nonzero identities if plan gate still requires supplemental nextest; no identical repeat |
| seven-path scoped rustfmt / git diff --check / line counts | no unrelated format edits; all owned Rust<1000 |

Fresh cohort/read-only inputs include current native song_capacity.rs, ring/song.rs,
wire/ABI/worklet and Engine/stager. They are not writable here. If an actual compile
or necessary implementation consumer lies outside the manifest, stop and obtain
root's prior bounded amendment; do not waive tests or enlarge scope silently.

## Completion criteria

- [x] All seven module deliverables and three tasks implemented and independently verified.
- [x] Exact refused/admitted commands and payload ownership/pacing proven on actual hosts.
- [x] Native/host-wasm/strict Clippy/scoped formatting and relevant nonzero tests pass.
- [x] Full logs, current pre/post hashes and terminal original handles recorded.
- [x] Parent delegation remains serial; no parallel shared-path writes.
- [x] Child-only result does not claim host Ready, finite transport, full09/10 or goal complete.

## Progress Log

### Session: 2026-10-02 — ROOT0299 document-only release

Completed: source-grounded bounded child specification and parent serial delegation.
Implementation tasks remain NOT_STARTED. No Rust/Cargo/Git/index/archive execution.
Blocker to implementation: current authors' mandatory joined checks and separate
root source ownership authorization. Full parent acceptance remains unchanged.

## Source release declaration refinement

ROOT0307 releases the seven manifested Rust paths. Private inherent delegates:
```rust
impl NativeAudioHost {
    pub(super) fn try_song_command_checked(&mut self, command: SongCommand) -> Result<(), SongCommandRefusal>;
    pub(super) fn song_native(&mut self, install: NativeSongInstall) -> Result<(), NativeSongInstall>;
    pub(super) fn song_graph(&mut self, lease: SongLeaseKey, graph: &GraphHandle) -> Result<(), Failure>;
    pub(super) fn song_sample(&mut self, lease: SongLeaseKey, data: Arc<SampleData>) -> Result<(), Arc<SampleData>>;
}
impl WasmAudioHost {
    pub(super) fn try_song_command_checked(&mut self, command: SongCommand) -> Result<(), SongCommandRefusal>;
    pub(super) fn song_graph(&mut self, lease: SongLeaseKey, graph: &GraphHandle) -> Result<(), Failure>;
    pub(super) fn song_sample(&mut self, lease: SongLeaseKey, data: Arc<SampleData>) -> Result<(), Arc<SampleData>>;
}
impl HostState {
    pub(super) fn pump_song(&mut self);
    pub(super) fn song_ack(&mut self, ack: SongHostAck);
}
```
Caps preserves `song_codec` visibility through its child facade. Browser declares
`#[path = "messages/song.rs"] mod song` for actual portable integration imports.
All checks happen before queue mutation; exact inline refusal intentionally retains
the complete POD. No Cargo has run for this source batch.

### Genuine fixture declarations
```rust
fn commands() -> Vec<SongCommand>;
fn lease(kind: SongResourceKind) -> SongLeaseKey;
fn take_records() -> Vec<Vec<u8>>;
fn browser_all_commands_refuse_complete_records_and_retry_once();
fn unsupported_and_invalid_commands_consume_no_transport();
fn browser_partial_sample_pressure_preserves_full_key_progress();
fn native_all_commands_refuse_and_retry_without_duplicate_records();
fn native_refused_sample_preserves_original_allocation();
```
Native private tests use the actual headless pair's bounded control ring and drain
records directly, preserving the internal initial capacity request. Browser tests
use actual bounded ABI records and ByteInbox; pacing assertions use actual sender
ACK ingestion. Child inherent delegates have pub(super) visibility for parent calls.

Ownership fixture signatures: `fn native_refused_box_preserves_original_allocation()`
and `fn browser_borrowed_graph_refusal_preserves_allocation()`. Both use actual empty
BusTemplate/BusDef allocations with exact full lease keys, preserving returned Box
and borrowed Arc addresses. Parent checked delegates retain the narrowly justified
inline-POD result_large_err allowance of their contract, not an allocation wrapper.

Event fixtures use actual push_ctl with Const(-0.0) and fullwidth Cell/IDs and
AudioEvent generation. Actual native returned POD is re-encoded and compared byte
for byte; browser actual ByteInbox decoded POD likewise re-encodes exactly. This
proves signed-zero bits rather than relying on floating-point PartialEq.

Genuine success fixture declaration:
`fn browser_sample_pacing_consumes_real_engine_receipts_and_returns_lease()`.
Actual configured Arena Engine consumes ABI records through ByteInbox. Real
SliceAccepted advances sender progress; ResourceReady proves actual adoption,
and actual CancelLease/LeaseReturned releases retained sender reservation. No
successful acknowledgment is fabricated in this fixture.

## Author source checkpoint

ROOT0307 source batch implements cohesive three-child splits and checked actual
Native/browser admission. Written fixtures cover all17 variants, actual POD Event
signed-zero/control bytes, unsupported/invalid refusal, original native Box/Arc
addresses, actual browser graph ByteInbox records, paced partial-pressure/full-key
challenges and configured Arena Engine-produced sample/cancel receipts. These
fixtures are written and scoped-formatted, not yet executed. No Cargo commands or
behavioral clearance claimed. Mandatory joined checker follows both author holds.
Immutable mutation receipts: SONG-10-COMMANDS/0004 through0012. The0009 substitution
assertion wrote declarations only;0010 records the actual subsequent source intent.
All unrelated DSP/geometry and parent plans remain held and untouched.

## Explicit instrument ownership evidence increment

ROOT0315 releases fixture-only private `fn
native_refused_instrument_box_preserves_original_allocation()` in native child.
Build an actual one-node SinOsc InstDef through Template::from_inst with the
headless host build environment; full queue refusal retains exact instrument
Box pointer and full lease, then retry transfers that same pointer exactly once.
All17 native commands attempt a second refusal before any drain; original and
both refused commands encode to identical bytes, preserving signed-zero bits.
The host diagnostic drop count remains unchanged and full ring length is intact.
Actual prior007 private3/public5 and008finish3 pass; new fixture awaits checker.

### Session: 2026-10-02 — ROOT0317 independent scoped acceptance

Checked native and browser command submission is complete. Actual joined009
private native4 and public song_hosts5 passed; the unique sender nextest also
ran exactly9 selected fixtures, all passed. Native/host-wasm/strict Clippy and
finish010 line, scoped formatting and diff gates passed. Root independently
matched raw inventories, executions and all65 held hashes. Actual instrument
Template and BusTemplate Box/PCM Arc ownership, repeated17-command refusal and
real Engine-produced sample receipts are covered. Known sampled-reverse route
failure is preserved; this does not complete full host preparation or playback.
Archive/index changes remain delegated to SONG-16.
