# Audio commands and acknowledgments implementation plan

**Status**: In Progress
**Plan ID**: SONG-08
**Plan Path**: impl-plans/active/song-mode-audio-contracts.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Audio commands and acknowledgments supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-08",
  "planPath": "impl-plans/active/song-mode-audio-contracts.md",
  "dependsOn": [
    "SONG-06",
    "SONG-07",
    "SONG-07A",
    "SONG-07B",
    "SONG-08A",
    "SONG-08B"
  ],
  "writePaths": [
    "src/song/routing.rs",
    "src/host/caps.rs",
    "src/host/wire.rs",
    "src/dsp/ring.rs",
    "src/dsp/engine.rs",
    "src/sched/runtime.rs",
    "src/song/mod.rs",
    "tests/song_audio_wire.rs",
    "impl-plans/active/song-mode-audio-contracts.md"
  ],
  "sharedPaths": [
    "src/song/routing.rs",
    "src/dsp/engine.rs",
    "src/sched/runtime.rs",
    "src/song/mod.rs",
    "impl-plans/active/song-mode-audio-contracts.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/routing.rs",
      "intendedEdit": "Define immutable branch route plan, private effect templates and resource/transition requirements. Owners execute serially: SONG-08, SONG-09"
    },
    {
      "path": "src/dsp/engine.rs",
      "intendedEdit": "Consume new command variants exhaustively; reject unprepared song activation with acknowledgment until SONG-09 supplies branch engine. Never mark rejected commands Applied. Owners execute serially: SONG-08, SONG-09"
    },
    {
      "path": "src/sched/runtime.rs",
      "intendedEdit": "Handle new host ack variants exhaustively; retain readiness/rejection records for later SONG-11 transport without activating a song. Owners execute serially: SONG-05, SONG-08, SONG-11"
    },
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Export routing contracts. Owners execute serially: SONG-01, SONG-04, SONG-06, SONG-08, SONG-12"
    },
    {
      "path": "impl-plans/active/song-mode-audio-contracts.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-06](song-mode-snapshot-contracts.md)
- **Additional prerequisites**: [SONG-07A selected-source route metadata](song-mode-source-route-inventory.md), [SONG-08A bounded host polling](song-mode-host-ack-polling.md)
- **Next**: [SONG-09](song-mode-dsp-routing.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-06 | Independently cleared snapshot contracts | VERIFIED |
| SONG-07 | Independently cleared candidate inventory, historical scope | VERIFIED |
| SONG-07A | Copied selected-source track/family/root policies for bounded nested placement admission | VERIFIED |
| SONG-07B | Lossless independent source-use topology and placement/configuration cover bounds | IN_PROGRESS |
| SONG-08B | Cohesive bounded symbolic route preparation and relocated complete route fixtures | IN_PROGRESS |
| SONG-08A | Actual non-lossy one-message host polling before receipt saturation handling | VERIFIED |

## Execution and preservation contract

This is a future implementation contract. This planning run changes documents only and executes no future tests.
Prerequisite before native Riela implementation/review fanout: accepted design and reviewed plans must be committed in a separately authorized serial execution. This specialized design/plan workflow does not perform that commit or Git finalization.
Use the repository rust-coding agent for Rust changes and invoke check-and-test-after-modify after every Rust modification batch. Use design-doc/impl-plan skills for documentation and progress.
Before each edit, freshly read the exact target and its dependencies; record SHA-256 before/after in an immutable intent record under tmp/song-mode-riela/<planId>/<taskId>.json. The record contains accepted design hash, plan hash, task ID, exact intended change, target paths and baseline hashes. Do not overwrite intent records: create a new numbered record for revisions.
Compare the fresh pre-edit hash with the recorded baseline immediately before writing. If it drifted, stop that edit, reread the current file and reconcile the intended diff; never restore a stale whole-file copy. Preserve all pre-existing staged, tracked and untracked work. Recheck post-edit hashes at join; unexpected drift requires serial repair by the reconciler, not another worker overwrite.
Only edit listed writePaths. An additional compile-required exhaustive match or file split needs a serial manifest/plan amendment before work resumes; no implicit broad cleanup permission. If a touched Rust source reaches 1000 lines, split it under repository standards and record exact new paths before editing. Current inspected principal integration files are below 1000 lines.
Use the same branch and workspace. No worktrees, private branches, concurrent Git operations, dependency additions or lockfile generation are prescribed. Shared indexes, broad formatting, archive moves and cross-worker repair belong exclusively to SONG-16 after joining.
Each worker updates only its own plan's status, completion checkboxes and dated progress log, including actual command, exit code and complete foreground log path. Retain/poll every foreground session to terminal exit; no orphan processes or detached shell work.

## Scope and invariants

Implement only this plan's deliverables. No tempo-map engine, arbitrary bus graph, browser offline export, music-visual framework, extra track controls, dependency upgrades or unrelated source refactoring.
Preserve existing lazy Pattern/stack/cat/repeat semantics, sound-first parser conventions, immutable data, rational musical time, live incremental eval and native/browser capability diagnostics. New Part/Song are explicit finite values; proposed syntax is not a parser grammar change.
All callback work remains allocation-free and VM-free. Song snapshot identity is revision/epoch-scoped; seeds do not depend on host/query partitioning. Errors must not be called successful complete music.
Use existing Ratio64, Pat, Value, Failure, Tempo, KwId and capability types. Declaration blocks below are signatures/type contracts, not implementation bodies; import paths refer to existing crate modules or prerequisite song modules. Do not add a generic abstraction merely to wrap these declarations.

## Modules and file-level deliverables

| File | Intended change | Status |
|---|---|---|
| `src/song/routing.rs` | Define immutable branch route plan, private effect templates and resource/transition requirements. | NOT_STARTED |
| `src/host/caps.rs` | Extend audio host preparation/activation/mute contract with epoch/frame and readiness acknowledgment. | NOT_STARTED |
| `src/host/wire.rs` | Define branch/epoch-stamped song commands, endpoint/release commands and host acknowledgments. | NOT_STARTED |
| `src/dsp/ring.rs` | Encode bounded commands in native records and browser byte transport with explicit checked capacity. | NOT_STARTED |
| `src/dsp/engine.rs` | Consume new command variants exhaustively; reject unprepared song activation with acknowledgment until SONG-09 supplies branch engine. Never mark rejected commands Applied. | NOT_STARTED |
| `src/sched/runtime.rs` | Handle new host ack variants exhaustively; retain readiness/rejection records for later SONG-11 transport without activating a song. | NOT_STARTED |
| `src/song/mod.rs` | Export routing contracts. | NOT_STARTED |
| `tests/song_audio_wire.rs` | Roundtrip boundaries, reject malformed/overflow records and compare native/browser command fields. | NOT_STARTED |

### Public declaration contract

```rust
pub struct SongBranchId(pub u32);
pub struct SongActivation { pub epoch: SnapshotEpoch, pub frame: u64 }
pub struct SongMute { pub epoch: SnapshotEpoch, pub instrument: u32, pub muted: bool, pub frame: u64 }
pub enum SongRejectCode { StaleEpoch, NotReady, Capacity, Malformed, HostFault }
pub enum SongHostAck { Ready(SnapshotEpoch), Applied(SongActivation), Muted(SongMute), Rejected { epoch: SnapshotEpoch, reason: SongRejectCode } }
pub struct SongRoutePlan { pub branches: Vec<SongBranchRoute>, pub max_live_generations: u32, pub required_bytes: u64 }
pub struct SongBranchRoute { pub id: SongBranchId, pub track: KwId, pub instrument: FrozenSound, pub resolved_instrument: InstId, pub sample: Option<SampleSrc>, pub effect_template: Option<KwId>, pub destination: BusId }
pub struct SongHostCapacities {
    pub sample_rate: u32,
    pub cell_slots: u32,
    pub voice_slots: u32,
    pub template_slots: u32,
    pub bus_slots: u32,
    pub sample_resources: u32,
    pub pcm_bytes: u64,
    pub voice_frames: u64,
    pub bus_frames: u64,
    pub ack_slots: u32,
}
pub fn prepare_routes(snapshot: &SongSnapshot, caps: &CapabilitySet, available: &SongHostCapacities) -> Result<SongRoutePlan, Failure>;
```

## Tasks

### TASK-001: Baseline and contract integration
**Status**: NOT_STARTED
**Parallelizable**: No; acquire dependencies and fresh hashes first.
**Deliverables**: Manifest, immutable intent snapshot and the declaration/type integration listed above.
- [ ] Read accepted section and prerequisites; record exact ownership and imports.
- [ ] Add declarations without changing legacy semantics; review manifest-required exhaustive consumers.

### TASK-002: Implement the owned behavior
**Status**: NOT_STARTED
**Depends On**: TASK-001
**Parallelizable**: No within this plan; cross-plan parallelism follows the DAG and ownership manifest.
**Deliverables**: Every non-test file in the module table, with exactly its stated intended change.
- [ ] Implement the declared behavior and all phase-specific criteria below.
- [ ] Record post-edit hashes and run required modify-agent checks.

### TASK-003: Behavioral evidence and progress
**Status**: NOT_STARTED
**Depends On**: TASK-002
**Parallelizable**: No; verifies the complete phase.
**Deliverables**: Every listed test/fixture file, full command logs and this plan progress record.
- [ ] Add the specified success, boundary, compatibility and failure fixtures.
- [ ] Run future commands below; record actual exit status and complete output, with no empty selected-test run accepted.
- [ ] Reconcile final hashes with intent; update completion criteria and progress without editing other worker logs.

### TASK-004: Independent staged carrier coverage
**Status**: Completed
**Parallelizable**: Yes; SONG-08B holds this exact shared test file.
**Owner**: rust-coding agent; scope is this task and its own parent progress entry.
**Deliverables**: tests/song_audio_wire.rs only; no runtime Rust changes.

This test-only task consumes established POD/codec declarations and is independent
of the changing source-route resolver. ROOT0085/0086 record a serial handoff of
this file from SONG-08B, whose remaining six Rust paths and own plan stay owned by
its author. No parent-wide verification or DSP release is implied. Final parent
acceptance still requires independently cleared SONG-08B and a joined stable tree.

- [x] Exercise all five resource-kind tags through Native and byte carriers.
- [x] Reject invalid kind/optional flags and every truncated staged record.
- [x] Cover all None/Some configuration references and full-width epoch, resource,
      generation and frame values, preserving the following valid record on errors.
- [x] Existing carrier, receipt, legacy and no-allocation fixtures remain intact.
- [x] Fresh immutable intents, scoped formatting, focused tests and required
      independent verification prove this task without claiming parent completion.

## Phase acceptance criteria

- [ ] Branch->track bus->master only; strict unresolved-route diagnostics; no arbitrary routing framework.
- [ ] Wire data includes required mono-note/branch identity so commit never expands song chord twice.
- [ ] Epoch/frame/state roundtrip without float conversion or stale acknowledgement acceptance.
- [ ] Retiring branch lease includes explicit tail deadline; actual arena/cell/graph capacities bound preparation.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_audio_wire)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

## Completion criteria

- [ ] All module-table changes and phase-specific criteria complete.
- [ ] Tests listed above execute with nonzero fixture count and pass; check/typecheck/build gates pass.
- [ ] All required command exit statuses and complete log paths recorded; no running foreground sessions remain.
- [ ] Legacy behavior preserved; pre-existing changes retained and cross-worker hashes reconciled.
- [ ] Progress status updated; archive/index changes deferred to SONG-16.

## Progress Log

### Session: 2026-09-30 — plan authoring
**Tasks Completed**: Planning only; no implementation task completed.
**Tasks In Progress**: None.
**Blockers**: None for planning; implementation awaits reviewed/committed documents and prerequisite waves.
**Verification**: Read-only source/document consistency review only. Future commands above were not executed.
**Next session**: Implement TASK-001 after dependency outputs and authorization are available.

### Session: 2026-10-01 — actual capacity and acknowledgment preflight
CapabilitySet alone does not describe configured/free template, bus, sample, delay
and acknowledgment capacity. Pass an explicit remaining host capacity snapshot,
including reservations and retiring generations. Derive integer frame capacities
from actual EngineConfig and sample rate; do not infer branch capacity from voices.
Host preparation must reserve/admit the complete plan and report Ready only on
actual admission. SONG-09 supplies allocation and timing behavior; SONG-08 interim
commands remain explicitly rejected. Preserve full u64 epochs/frames at every
wire boundary. Critical Ready/Applied/Muted/Rejected acknowledgments require
bounded retry/backpressure; a full acknowledgment queue cannot silently lose them.

### Session: 2026-10-01 — frozen route inventory and actual rate/cells
Prepare routes from SONG-07 certified immutable route metadata, so this phase
now additionally depends on SONG-07. A mutable query bridge is not an immutable
routing inventory, and private evaluator/registry RefCell aliases must not escape.
Consume copied instrument/bus descriptors and strict named-bus lookup from the
snapshot. Include actual sample rate and remaining cell capacity in host inputs;
state memory/frame counts depend on rate and cells need explicit admission.
Native/browser carriers support song POD records generically; keep exact epoch
and frame fields and bounded records. Delegate codec helpers within existing
owned routing.rs if needed to keep wire/ring below 1000 lines. Do not reinterpret
legacy AudioEvent constructors or tags to convey uncertified mono/branch identity.

### Session: 2026-10-01 — copied usage and symbolic track routes
Copy the repaired closed asset pcm_bytes scalar into snapshot admission metadata.
Routing inventory must retain bounded symbolic sequence offsets/durations, repeat
counts and edit/FX scopes/source revisions, or equivalent certified topology; a
global graph list alone cannot distinguish placement-dependent effect branches.
Do not expand repeats or scan the full score. Preserve selected-source origin
cutoffs and plate/spring/fast2 semantics from SONG-04. Family identity DTOs copy
buffer IDs and resolved routes, never Rc<SampleBuf> or mutable registry aliases.

Each declared Part track owns a distinct intermediate summing stage. An existing
same-name declared bus supplies that stage's effect-chain template; when none is
declared, explicitly prepare a neutral track stage. This preserves the accepted
24-cycle example, which declares tracks but no bus templates. It is never a
direct branch-to-master fallback: branches still sum through their track stage.
Explicit named bus/FX selections require exact declared registry lookup and
reject unknown names. Keep the restricted branch -> track stage -> master
topology; do not infer arbitrary user routing graphs. Existing event bus controls
must be resolved/validated against this topology, with explicit diagnostics for
unsupported conflicting track destinations. ROOT0031 records a source-grounded
clarification, not an additional Riela acceptance.

### Session: 2026-10-01 — immutable branch family identity
SONG-08 branch routes consume FrozenSound and the certified resolved InstId and
optional SampleSrc from SONG-07 metadata. InstrumentSelector retains mutable
SampleBuf aliases and cannot be retained in immutable host preparation contracts.
Buffer IDs remain opaque copied identities; PCM is loaded from the closed
inventory only. No evaluator/registry or original mutable buffer escapes.
Reserve existing browser SampleBegin tag 0x1A when extending POD wire tags.
Generic native/browser record consumers require no additional source ownership
in the read-only preflight; owned engine/runtime matches remain exhaustive.
ROOT0035 records this source-grounded correction, not a new Riela acceptance.

### Session: 2026-10-01 — POD transport and source metadata prerequisite
Fresh TASK-001/TASK-002 intents cover only owned sources. Initial typed song command/ack declarations preserve existing tags and constructors; 0x1A remains sample-begin, song command/ack tags append at 0x1B/0x49. Exact epoch/frame/branch/generation and an explicit mono marker survive native and byte carriers. Staged preparation/reservation/configuration/seal POD carries exact host-remapped resource leases and deadlines; interim engine returns NotReady or malformed diagnostics, never Ready/Applied. Critical acknowledgments retain one bounded pending slot and stop control intake until flushed.

Author-wire-002.log has seven passing fixtures, retained process 37965 terminal 0. The first check log retains the owned misplaced-ack dispatch error and check-002 retains the discriminator exhaustiveness correction history; no final seal is claimed. Concrete codecs move into an approved cohesive inline host/caps.rs song_codec module to keep the exact eight-file scope below 1000 lines.

Route certification now depends on SONG-07A: selected-source descriptors must retain copied Part root, selected track and complete frozen family. Existing root Part ancestry cannot certify externally captured source FX scopes alone. Hold route certification until mandatory independent clearance. Preparation will use bounded symbolic nested placement cardinality rather than infer transformed timing from unscaled source durations; distinct nonadjacent source placements cannot silently reuse effect state. No eager fullscore query or runtime failure substitutes for preactivation admission.

### Session: 2026-10-01 — bounded receipt adapter prerequisite and partial baseline
Author-wire-006.log records 13 passing POD/carrier/capacity fixtures, retained process 32766 terminal 0. Callback rejection performs zero allocations; critical queue backpressure is not counted as dropped; malformed byte records preserve the following command identity. Codec move is complete within existing owned caps.rs. TASK-003-checks-002 records native/host-wasm/scopedfmt/diff exit 0; strict all-target gate exit 101 is an in-progress SONG-07A fixture compile failure in externally owned freeze.rs. Prior owned large-POD lint findings were corrected with justified inline transport allowances; failed logs are retained. Process 79094 is terminal 0, no author process remains. This is a partial baseline, never full phase clearance.

Read-only review found the initial receipt ledger evicts after64 and the host drain API consumes entire batches. This known pending defect is not certified. SONG-08A adds actual one-at-time host polling; Runtime then retains one consumed pending POD and stops polling while receipt storage is full, exposing explicit bounded consumption. No critical acknowledgment may be silently evicted or replaced with a terminal discard. Root authorized a serial exact-capability adapter prerequisite; source route certification still waits independent SONG-07A. TASK-003-post-001 seals the current partial eight-source baseline solely for expected serial ownership/drift tracking, not acceptance.

### Session: 2026-10-01 — lossless Runtime receipts and direct-route admission

SONG-08A independently cleared all10 gates,106 distinct plus5 nextest repeats (terminal60279, `/tmp/vactr-song08a-independent-002/final-results.json`). SONG-07A independently cleared all13 gates,135 distinct plus37 repeats (terminal35655, `/tmp/vactr-song07a-independent-002/final-results.json`). Their serial caps/runtime/source metadata drift is authorized and retained, never restored from partial baselines.

Runtime now admits receipts without eviction, retains exactly one consumed pending POD acknowledgment, stops source polling on saturation and exposes explicit FIFO consumption. Stale retirement identities are retained and diagnosed; unsupported active polling never falls back to whole-batch drain. True legacy-only unsolicited song acknowledgments are protocol faults. Receipt expectation remains active through complete ownership retirement; later10 must select it before the first checked command. Author receipts002 has18 passing fixtures (terminal18738), native check terminal82033 exit0; host-wasm and strict all-target Clippy both exit0 in TASK-004-checks-001. No final08 clearance is claimed.

Every branch generation reserves its own private bus/Room and independent stereo four-second delay, even without instrument-fx. Nonclipped BusSlot memory is max(4*Room,Room+chain state), reflecting configure's quarter-slot Room partition. Route plans report branch delay frames separately and include them in aggregate bus arena admission; later09 must allocate and validate the independent partitions, not redirect branches to shared legacy orbit state.

Voice slots and voice frames describe the configured fixed shared engine pool, capped by the host tier; they are validation requirements, not resources leased again for every snapshot. Apply clears/fades the old voices before reusing that pool. Current/retiring subtraction applies true leased template, cell, bus, sample, PCM and acknowledgment resources. It does not subtract voice_slots/voice_frames, and reservation sample rates must agree. Later09/10 must measure the actual uniform per-voice state arena and enforce the advertised pool, never manufacture capacity.

Direct route work now compiles: strict named buses, distinct neutral track stages, immutable scoped topology, checked compressed repeat multiplication and tail overlap. Selected-source certification remains explicitly incomplete because deduplicated source roots do not bound independent fast/slow/stack aliases; SONG-07B must supply typed source-use topology and conservative placement/config cover bounds before final08 seal. No per-note/chord-tone private graph reservation is intended: all voices in the same admitted placement/configuration share a branch.

### Session: 2026-10-01 — serial SONG-08B route ownership handoff
Root0044 authorizes child08B exact routing.rs, routing/prepare.rs, song_audio_wire.rs and song_route_preparation.rs paths. Parent08 pauses its Rust edits during the serial child; carriers/receipt contracts remain unchanged, route helpers/fixtures relocate without semantic restoration. Child depends07A/07B/08A and consumes partial08 declarations without depending full08, avoiding a cycle. Final08 acceptance/seal waits independently cleared child08B and source-use07B. Current source/test sizes845/969 trigger the bounded split before either reaches1000. Sharedmod.rs remains with07B.


### Session: 2026-10-01 — TASK-004 staged carrier fixtures, author focused evidence

ROOT0085/0086 transfers only tests/song_audio_wire.rs to this task author; routing
owns its disjoint sources and child progress. Immutable TASK-004-staged-001 captures
exact parent/test baseline without overwriting prior receipt evidence. Three new
fixtures preserve all eighteen earlier carrier/receipt/legacy/noallocation tests.
Fifteen boundary commands cover all five resource-kind tags, all eight None/Some
configuration combinations, max-width epochs/resources/generations/branches/frames
and capacities. NativeRecord and ByteInbox yield exactly identical command POD.
Every encoded staged prefix truncation fails; invalid kind5/255 and option2/255
reject. Every malformed/truncated queued record preserves the immediately following
full-width ConfigureBranch identity; empty record prefixes are decoded separately.
No runtime source changes, lint allowance or successful host readiness is implied.

Production owner confirmed native15750 terminal0 and briefly held production for
focused suite59444, which exited0:21/21, zero filtered. Full log is
`tmp/song-mode-riela/SONG-08/TASK-004-staged-tests-001.log`. The fixture is924 lines.
Scoped formatter/diff output is retained; targeted Clippy and explicit21-name
inventory await the next brief stable production window. Mandatory independent
focused verification remains pending. TASK-004 and full parentSONG-08 are not
claimed complete, and SONG-09 remains unreleased.


### Session: 2026-10-01 — TASK-004 staged final author readiness

ROOT0087 established a brief stable routing hold for targeted validation. Runner
80849 terminal0: targeted strict `cargo clippy --test song_audio_wire -- -D warnings`,
actual21-name test inventory, scoped rustfmt check and scoped diff check each exit0.
Earlier actual focused21/21 suite59444 terminal0 remains the test evidence; no
empty filter or raised test stack was used. Immutable artifacts are
`tmp/song-mode-riela/SONG-08/TASK-004-staged-author-check-results.json`,
`TASK-004-staged-test-inventory.json`, and their full staged002 logs. The fresh
TASK-004-staged-002 author-evidence intent changes only this progress entry.

Readiness seal retains924-line fixture SHA, parentplan SHA and unchanged relevant
codec/wire/ring/runtime input hashes. Fixture and parentplan are held for mandatory
independent focused verification. No production code or other parent task changed.
TASK-004 independent approval remains pending, fullparent08 still gated on08B
independent clearance, and09 implementation remains unreleased.


### Session: 2026-10-01 — TASK-004 staged independent completion

ROOT0089 accepted independently cleared TASK-004 staged carrier fixtures: six gates
exit0,21 distinct fixtures plus21 separately counted nextest repeats, exact held
hashes matching author readiness seal. TASK-004 alone is Completed; its final
remaining independent-verification checkbox is satisfied. Fresh immutable
TASK-004-staged-004 before/after records this documentation-only task status and
progress update. The924-line fixture and all other Rust remain unchanged/held.
No other parent criteria, task metadata, index or archive is changed. Parent
SONG-08 remains In Progress and requires independently cleared route preparation;
SONG-09 and full playback/Apply/mute/export remain future work.


### Session: 2026-10-02 — verified staging provider handoff

ROOT0279 independently verifies actual native/Arena resource staging, measured
capacity and exact retained ownership: original4235 terminal0,36 gates and153
raw successful fixture names matching fresh inventories,48 current source/plan
hashes. Genuine individual bus memory and fragmented PCM rejection, additive
legacy/song occupancy, exact full-key association, bank remapping, silent live
preservation and simultaneous ACK/garbage pressure are proven in their stated
provider scope. Evidence is recorded in ROOT0279-pressure-browser-scoped-acceptance.json.

SONG-08B consumes actual measured remaining capacities through existing frozen
interfaces. Its route preparation still performs aggregate requirement admission;
per-region rejection and retained lease ownership occur at actual resource adoption.
These provider results do not prove complete route geometry or canonical query to
timed audio integration. SONG-08B/08D and parent SONG-08 remain In Progress.
SONG-09P prerequisite status may reconcile these provider results independently;
its literal nextest gate is separately pending ROOT0281. Full09 implementation
ordering is amended in its own plan, while final full09 acceptance continues to
require complete08B/08D and independently joined parent08. No parent completion
checkbox is changed, and no remaining song behavior is removed.
