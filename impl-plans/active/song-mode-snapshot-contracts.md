# Snapshot and session request contracts implementation plan

**Status**: Completed
**Plan ID**: SONG-06
**Plan Path**: impl-plans/active/song-mode-snapshot-contracts.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-01
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Snapshot and session request contracts supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-06",
  "planPath": "impl-plans/active/song-mode-snapshot-contracts.md",
  "dependsOn": [
    "SONG-05",
    "SONG-06A"
  ],
  "writePaths": [
    "src/song/snapshot.rs",
    "src/session/protocol.rs",
    "src/session/codec.rs",
    "src/session/session.rs",
    "src/song/mod.rs",
    "tests/song_protocol.rs",
    "src/session/tests/codec.rs",
    "impl-plans/active/song-mode-snapshot-contracts.md"
  ],
  "sharedPaths": [
    "src/song/snapshot.rs",
    "src/session/session.rs",
    "src/song/mod.rs",
    "impl-plans/active/song-mode-snapshot-contracts.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/snapshot.rs",
      "intendedEdit": "Define isolated candidate/snapshot ownership, preparation state and epoch; retain frozen assets/namespace/registry independently. Owners execute serially: SONG-06, SONG-07"
    },
    {
      "path": "src/session/session.rs",
      "intendedEdit": "Handle new ClientMsg cases exhaustively; return explicit unsupported/not-ready diagnostics until SONG-07 installs candidate dispatch. Preserve incremental eval. Owners execute serially: SONG-06, SONG-07"
    },
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Export snapshot contracts. Owners execute serially: SONG-01, SONG-04, SONG-06, SONG-08, SONG-12"
    },
    {
      "path": "impl-plans/active/song-mode-snapshot-contracts.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-05](song-mode-natives.md)
- **Next**: [SONG-07](song-mode-candidate-evaluation.md), [SONG-08](song-mode-audio-contracts.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-05 | Reviewed declarations, passing phase checks and recorded post-edit hashes | VERIFIED |
| SONG-06A | Isolated/closed assets, including ROOT0028 literal-keyword repair | VERIFIED |

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
| `src/song/snapshot.rs` | Define isolated candidate/snapshot ownership, preparation state and epoch; retain frozen assets/namespace/registry independently. | NOT_STARTED |
| `src/session/protocol.rs` | Add apply-song, mute-instrument and candidate-ready/applied/failed message shapes with revisions, epoch and actual application frame. | NOT_STARTED |
| `src/session/codec.rs` | Encode/decode new shapes; use checked decimal strings for u64 epochs/frames on JSON boundary; include unknown/stale-field diagnostics. | NOT_STARTED |
| `src/session/session.rs` | Handle new ClientMsg cases exhaustively; return explicit unsupported/not-ready diagnostics until SONG-07 installs candidate dispatch. Preserve incremental eval. | NOT_STARTED |
| `src/song/mod.rs` | Export snapshot contracts. | NOT_STARTED |
| `tests/song_protocol.rs` | Roundtrip all messages and reject stale revisions/epochs, malformed selectors and invalid frames. | NOT_STARTED |
| `src/session/tests/codec.rs` | Add five new-message samples required by exhaustive legacy all-kinds roundtrip fixtures. Preserve every existing fixture. | NOT_STARTED |

### Public declaration contract

```rust
pub struct ApplySongBody { pub file: String, pub code: String, pub doc_revision: u64, pub edit_epoch: u64 }
pub enum WireSongSound {
    Builtin { name: String },
    Instrument { id: u32 },
    Sample { path: String, file: Option<u32> },
    Buffer { id: u64 },
}
pub struct WireInstrumentSelector { pub family: Vec<WireSongSound> }
pub struct SongInstrumentMuteBody { pub epoch: SnapshotEpoch, pub selector: WireInstrumentSelector, pub muted: bool }
pub struct SongCandidate { evaluator: Evaluator, song: Rc<Song>, revision: u64, epoch: SnapshotEpoch }
pub struct SongSnapshot { evaluator: Evaluator, song: Rc<Song>, epoch: SnapshotEpoch, samples: Vec<Rc<SampleBuf>> }
pub struct SongResourceLease { pub resource: u32, pub generation: u32 }
pub struct PreparedSong { snapshot: SongSnapshot, resources: Vec<SongResourceLease>, state: SongPreparationState }
pub enum SongPreparationState { Evaluating, Preparing, Ready, Applied, Failed }
pub struct SongApplyAck { pub epoch: SnapshotEpoch, pub application_frame: u64, pub doc_revision: u64 }
pub fn prepare_song(candidate: SongCandidate) -> Result<PreparedSong, Failure>;
```

## Tasks

### TASK-001: Baseline and contract integration
**Status**: Completed
**Parallelizable**: No; acquire dependencies and fresh hashes first.
**Deliverables**: Manifest, immutable intent snapshot and the declaration/type integration listed above.
- [x] Read accepted section and prerequisites; record exact ownership and imports.
- [x] Add declarations without changing legacy semantics; review manifest-required exhaustive consumers.

### TASK-002: Implement the owned behavior
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No within this plan; cross-plan parallelism follows the DAG and ownership manifest.
**Deliverables**: Every non-test file in the module table, with exactly its stated intended change.
- [x] Implement the declared behavior and all phase-specific criteria below.
- [x] Record post-edit hashes and run required modify-agent checks.

### TASK-003: Behavioral evidence and progress
**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No; verifies the complete phase.
**Deliverables**: Every listed test/fixture file, full command logs and this plan progress record.
- [x] Add the specified success, boundary, compatibility and failure fixtures.
- [x] Run future commands below; record actual exit status and complete output, with no empty selected-test run accepted.
- [x] Reconcile final hashes with intent; update completion criteria and progress without editing other worker logs.

## Phase acceptance criteria

- [x] PreparedSong retains candidate resource leases; Ready and Applied are distinct.
- [x] SongCandidate and SongSnapshot retain an independently owned Evaluator/InstRegistry and immutable sample references, never old mutable VarSlotRef objects.
- [x] Specify cancel/reject behavior for pending revision changes and outdated mute requests.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_protocol)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

## Completion criteria

- [x] All module-table changes and phase-specific criteria complete.
- [x] Tests listed above execute with nonzero fixture count and pass; check/typecheck/build gates pass.
- [x] All required command exit statuses and complete log paths recorded; no running foreground sessions remain.
- [x] Legacy behavior preserved; pre-existing changes retained and cross-worker hashes reconciled.
- [x] Progress status updated; archive/index changes deferred to SONG-16.

## Progress Log

### Session: 2026-09-30 — plan authoring
**Tasks Completed**: Planning only; no implementation task completed.
**Tasks In Progress**: None.
**Blockers**: None for planning; implementation awaits reviewed/committed documents and prerequisite waves.
**Verification**: Read-only source/document consistency review only. Future commands above were not executed.
**Next session**: Implement TASK-001 after dependency outputs and authorization are available.

### Session: 2026-10-01 — source-grounded snapshot preflight
Snapshot ownership must use a fresh prelude, registry, VM, namespace and recording
sink. Active VarSlotRef, package namespaces, closure globals/captures, registry
cells and live signal inputs cannot be shallow-cloned into the candidate. Drop
the complete private candidate on any form/import failure or forbidden effect;
partial successes never publish. Expose snapshot query-only borrowing without
mutable evaluator/registry aliases.

Ready SampleBuf values remain mutable through fill/fill_at/fail. Capture ready PCM
and rate into private candidate-owned copies, or immutable decoded SampleData;
reject pending/failed inputs. All retained Sound::Buffer references must point to
private copies, rather than retaining mutable active readiness/rate aliases.

NativeSampleLoader clones share mutable roots/files/banks/next state. Candidate
source/sample preparation needs an isolated factory/configuration and candidate
file IDs, package namespaces and bank registration. Existing Session::load_package
registers active banks, releases effects to the active sink and drains the active
runtime; it must not be reused for isolated candidate imports. Source analysis
taps must be absent/rejected. The loader capability gap requires an exact bounded
manifest amendment before editing any additional loader or portable boundary file.

Pin a closed candidate asset inventory before Ready; later song queries must not
reread changed disk bytes, mutate active loader maps or register new resources.
Do not replace lazy finite-song realization with a full arrangement scan.
Verification must mutate active globals/functions/captures/kit/import slots, ready
buffer data/rate, bank/file maps and disk bytes after preparation and prove stable
candidate queries/assets. Late form/import/missing-sample failures preserve active
namespace, graph and transport. This preflight records obligations, not completion.

### Session: 2026-10-01 — isolated asset dependency
SONG-06 waits independently verified SONG-06A factory/pinned-inventory contracts.
Snapshot preparation must consume isolated assets; shallow active loader/buffer
aliases are not frozen snapshot evidence.

### Session: 2026-10-01 — checked wire mute families and snapshot ownership
InstrumentSelector and Sound are internal immutable typed values, not serde
objects. Use checked WireInstrumentSelector for JSON mute requests; encode u64
buffer IDs with the same checked decimal-string policy as epochs/frames. Validate
nonempty, bounded, duplicate-free families, sound kind fields and identifiers.
Later SONG-11 resolves the complete wire family against the addressed active
snapshot's original frozen Sound families. Unknown/stale IDs reject the request;
never manufacture a SampleBuf or forge a typed selector from an arbitrary ID.
Retain InstrumentSelector internally for routing and overlays. Wire roundtrips
prove representation validation, not active snapshot resolution or host application.
Candidate/snapshot certified-ownership constructors are crate-private and do not
publicly accept arbitrary active Evaluators. Tests may use fresh isolated fixtures;
actual fresh whole-code construction and transitive freezing remain SONG-07.

### Session: 2026-10-01 — authorized contracts implementation
SONG-05 and SONG-06A independently verified; implementing only the six owned Rust paths. Checked wire family uses private storage plus read-only family() accessor, so malformed or duplicate members cannot bypass construction/deserialization validation. Epoch/frame/buffer u64 values use canonical decimal strings; legacy envelope numbers remain unchanged. Ownership transfers are crate-private, with actual isolated fresh whole-code evaluation/transitive freezing reserved to SONG-07.

### Session: 2026-10-01 — exhaustive legacy codec fixtures
Existing session codec tests enumerate every client/server kind and require sample
bodies. Add only the new song message samples in the exact seventh Rust path;
retain existing roundtrip assertions and all legacy protocol representations.

### Session: 2026-10-01 — SONG-06 author seal, independent verification pending
**Implemented contracts**: Private non-clonable candidate/snapshot evaluator, registry and closed asset ownership; no public mutable evaluator/registry/Part/Song/buffer aliases. The crate-private constructor transfers caller-certified fresh ownership; actual whole-code evaluation and transitive rewriting/freezing remain SONG-07, not a claim of these constructor checks. Public snapshot metadata is copied/scalar-only. Crate-private with_query supplies an immediate Query-mode borrow. Private preparation state cannot be forged publicly. Preparing is not Ready; host adapters must initialize reservations exactly once (even an explicitly empty plan), match complete generation-qualified acknowledgements and only then transition to Ready. An actual application acknowledgement must match Ready, epoch and document revision. Pending revision/edit-epoch invalidation retains leases for later host cleanup; applied generations are not stopped by mere document edits. Old mute epochs and non-Applied states reject.
**Wire contracts**: apply-song/mute-instrument and distinct song-candidate-ready/applied/failed replies. Ready has no application frame; Applied carries an acknowledgement. A failed request before epoch issuance uses null rather than inventing an epoch. Checked private-storage WireInstrumentSelector validates nonempty/bounded/duplicate-free audio family representations and never creates a SampleBuf from a buffer ID. Typed sound-family membership resolution remains SONG-11. New u64 epoch/frame/buffer identities are canonical decimal JSON strings including values above JS safe precision; legacy numeric envelopes/revisions remain unchanged. Unknown song body fields reject. Session intake is explicitly unsupported/not-ready before candidate implementation and cannot evaluate proposed code, register files or activate slots. Stale known document revision/edit epochs reject first.
**Behavioral proof**: Seven public protocol fixtures, six crate-private snapshot ownership/state fixtures, one privacy compile-fail doctest, seven legacy codec fixtures and eight existing write-authority fixtures = 29 distinct tests, plus seven nextest repeats. Includes every new message, full u64 identities, malformed decimal and selector fields, unknown/stale fields, readiness/application distinction, stale acknowledgements/mutes, one-shot reservation initialization, actual generation matches, retained cleanup leases, fresh fixture registry/namespace separation, stable private captured PCM after original readiness/rate/data mutation, no premature session evaluation/activation and legacy wire/eval compatibility. Fresh fixture evidence is prerequisite-only; complete public candidate evaluation remains SONG-07. Existing exhaustive all-kinds codec samples required ROOT0027's exact seventh Rust path; all five new samples are additive.
**Terminal commands/logs**: `tmp/song-mode-riela/SONG-06/author-sealed-check-results.json` contains all eleven exact commands, log paths, terminal timestamps and zero exits: quiet native check, host-wasm check, protocol tests, snapshot tests, privacy doctest, legacy codec and authority tests, all-target Clippy, nextest, scoped format and diff checks. Runner session 80654 was polled to terminal exit 0, with no orphan foreground work. Early incorrect Unicode escape and buffer.fail signature fixture compile failures remain in `tests-first.log` and `snapshot-second.log`; corrections are sealed separately, never hidden as passes.
**Prerequisite re-clearance**: Original SONG-06A `/tmp/vactr-song06a-independent-001/final-results.json` and repaired ROOT0028 `/tmp/vactr-song06a-repair-independent-001/final-results.json` (51 distinct +20 repeats, all ten gates exit0, session46805terminal0) are verified before this author join. No assets repair source edits by this owner.
**Integrity/readiness**: Seven exact owned Rust SHA-256/line counts are in `author-source-seal.json`, all below 1000 lines. Immutable intents0001–0003 record current design/plan and target hashes; ROOT0027 manifest amendment was freshly reconciled before touching legacy codec tests. Preserve all unrelated editor/session/publish changes. Mandatory independent checker must verify this seal before phase completion; archive/index/Git deferred.

### Session: 2026-10-01 — independent SONG-06 completion
**Status**: Completed for snapshot ownership/wire/state contracts only. Actual whole-code freezing remains SONG-07; host admission and playback remain later phases.
**Independent evidence**: `/tmp/vactr-song06-independent-001/final-results.json`: all eleven gates exit 0, 29 distinct fixtures plus seven nextest repeats; retained session83755 terminal exit0, seven pre/post SHA-256 values match the author seal, maximum864 Rust lines. Privacy, one-shot reservation initialization, generation-qualified readiness/application and legacy protocol compatibility independently verified with no outstanding findings.
**Integrity**: Immutable intent0005 records this doc-only completion; no Rust, archive/index or Git changes.
