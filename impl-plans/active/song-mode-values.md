# Finite symbolic values implementation plan

**Status**: Completed
**Plan ID**: SONG-01
**Plan Path**: impl-plans/active/song-mode-values.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-01
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#finite-values)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Finite symbolic values supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-01",
  "planPath": "impl-plans/active/song-mode-values.md",
  "dependsOn": [],
  "writePaths": [
    "src/song/mod.rs",
    "src/song/part.rs",
    "src/song/song.rs",
    "src/song/limits.rs",
    "src/song/identity.rs",
    "src/lib.rs",
    "tests/song_values.rs",
    "impl-plans/active/song-mode-values.md"
  ],
  "sharedPaths": [
    "src/song/mod.rs",
    "src/song/identity.rs",
    "impl-plans/active/song-mode-values.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Export only the finite value, edit, identity and host contracts as they become available; keep implementation modules separate. Owners execute serially: SONG-01, SONG-04, SONG-06, SONG-08, SONG-12"
    },
    {
      "path": "src/song/identity.rs",
      "intendedEdit": "Define PartRevision, full OccurrencePath, EventHandle, SnapshotEpoch, PlacementPath, InstrumentSelector and SongEvent contracts before other consumers. Owners execute serially: SONG-01, SONG-04"
    },
    {
      "path": "impl-plans/active/song-mode-values.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: None; foundation
- **Next**: [SONG-02](song-mode-value-integration.md), [SONG-04](song-mode-query-edits.md)

| Dependency | Required output | Status |
|---|---|---|
| Accepted design | Step 3 acceptance and current baseline | AVAILABLE |

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
| `src/song/mod.rs` | Export only the finite value, edit, identity and host contracts as they become available; keep implementation modules separate. | IMPLEMENTED |
| `src/song/part.rs` | Define immutable capture, sequence, repeat and edit nodes with checked duration and track keys. No PCM or eager repeat expansion. | IMPLEMENTED |
| `src/song/song.rs` | Define Song and frozen timing/seed settings; separate arrangement endpoint from tail deadline. | IMPLEMENTED |
| `src/song/limits.rs` | Define construction/work limits and checked duration/frame-capacity validation; use actual advertised host limits. | IMPLEMENTED |
| `src/song/identity.rs` | Define PartRevision, full OccurrencePath, EventHandle, SnapshotEpoch, PlacementPath, InstrumentSelector and SongEvent contracts before other consumers. | IMPLEMENTED |
| `src/lib.rs` | Expose song module without changing existing exports. | IMPLEMENTED |
| `tests/song_values.rs` | Exercise zero-repeat/empty sequence, timed silence, invalid durations/counts and rational overflow using public constructors. | IMPLEMENTED |

### Public declaration contract

```rust
pub enum RepeatSeedMode { Same, Vary }
pub struct PartRevision(pub u64);
pub struct SnapshotEpoch(pub u64);
pub struct OccurrencePath { pub producer_ordinals: Vec<u32>, pub cycle: i64, pub onset: Ratio64 }
pub struct PlacementPath(pub Vec<u32>);
pub struct InstrumentSelector { family: Vec<Sound> } // checked new + family/contains getters
pub struct SongEvent { pub handle: EventHandle, pub track: KwId, pub instrument: Sound, pub event: Event, pub placement: PlacementPath, pub tone: Option<ResolvedNote>, pub commit_mode: NoteCommitMode }
pub struct SongLimits { pub max_nodes: u32, pub max_depth: u32, pub max_tracks: u32, pub max_cached_events: u32, pub max_frames: u64 }
pub enum PartEdit { ReplaceTrack { track: KwId, pattern: Rc<Pat> }, TransformInstrument { track: KwId, selector: InstrumentSelector, pattern: Rc<Pat> }, DeleteEvent(EventHandle), OverwriteRegion { track: KwId, region: TimeSpan, pattern: Rc<Pat> }, InstrumentFx { track: KwId, selector: InstrumentSelector, template: KwId } }
pub struct EventHandle {
    revision: PartRevision, track: KwId, placement: PlacementPath,
    occurrence: OccurrencePath, tone: u32
}
// Read-only getters expose each component. No public issuance constructor.
// Crate-internal issuance certifies successful canonical realization (SONG-04).
pub struct SeedIdentity(pub u64);
pub enum ResolvedNote { Int(i64), Ratio(Ratio64), Float32(f32), Float64(f64) }
pub enum NoteCommitMode { Mono }
pub struct SongEventId { pub epoch: SnapshotEpoch, pub handle: EventHandle }
pub struct SongSettings { pub bpm: Ratio64, pub cycle_beats: Ratio64, pub meter: [u32; 2], pub seed: u64, pub tail_seconds: Ratio64 }
pub enum PartNode { Capture(BTreeMap<KwId, Rc<Pat>>), Sequence(Vec<Rc<Part>>), Repeat { child: Rc<Part>, count: u32, seed_mode: RepeatSeedMode }, Edit { source: Rc<Part>, edit: PartEdit } }
pub struct Part { revision: PartRevision, duration: Ratio64, node: PartNode, /* private seed/track/cost metadata */ }
pub struct Song { part: Rc<Part>, settings: SongSettings, /* private exact endpoints */ }
pub fn capture_part(tracks: BTreeMap<KwId, Rc<Pat>>, duration: Ratio64) -> Result<Part, Failure>;
pub fn sequence(parts: Vec<Rc<Part>>) -> Result<Part, Failure>;
pub fn part_repeat(part: Rc<Part>, count: u32, mode: RepeatSeedMode) -> Result<Part, Failure>;
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

- [x] Positive capture duration; zero only for empty derived arrangements; unique track keys and checked arithmetic.
- [x] Root revision is separate from seed identity. Define full producer-path entries, placement ordinals and tone ordinals; no hash-only equality.
- [x] PartEdit declaration is owned here as data: ReplaceTrack, TransformInstrument, DeleteEvent, OverwriteRegion and InstrumentFx. Execution belongs to SONG-04.
- [x] Reject nonpositive BPM/cycle beats and invalid meter; default 120 BPM, 4 beats/cycle, 4/4, eight-second tail.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_values)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — SONG-01 implementation

**Implemented**: Checked finite captures, duplicate-safe capture iterator,
exact sequence sums, symbolic repeats/count validation, immutable prepared edit
data, settings freeze and exact absolute frame endpoints. Construction node,
depth, track/cache and explicit frame capacities reject rather than truncate.
Actual host allocations supply `SongLimits::for_capacities`; a voice limit is
not misrepresented as routing capacity. Frame admission occurs with frozen Song
tempo and the selected host rate, not an invented rate during Part construction.

**Contract refinements approved by root**:
- Part/Song and FrameEndpoints use private validated fields and read-only
  accessors. This prevents forged duration/endpoints and unchecked tail subtraction.
- EventHandle is opaque, issued internally only after successful realization;
  identity includes explicit track, full finite placement, complete producer
  tree/step ordinals, cycle/onset and tone. Delete data validates revision and
  declared root track; it does not incorrectly require edit-generated occurrences
  to belong to a pre-edit capture topology.
- SeedIdentity is structural and separate from checked monotonic control-thread
  revision allocation. Edits retain source seed identity; no hash substitutes
  for full event equality. Native/browser use existing deterministic Hasher.
- ResolvedNote retains Int/Ratio/Float32/Float64, including fractional notes;
  nonfinite notes and chord-list coercion reject. Mono commit mode marks already
  separated tones for downstream commit.
- TransformInstrument stores a prepared Rc<Pat>, not a deferred callable.
  Callback invocation and canonical tracing remain entirely in SONG-04.
- Required song.rs is a private descriptor module behind public facade exports,
  avoiding Clippy module-inception without changing listed file ownership.

**Preservation/intent**: Numbered immutable before/after SHA-256 records are at
`tmp/song-mode-riela/SONG-01/0001-checked-values*.json` through
`0006-plan-contract-evidence*.json`. Each records fresh design/plan/target hashes;
formatting used only owned files with rustfmt skip_children=true. Existing dirty
editor and src/session/tests/publish.rs were not written. Root's approved design
and trace-plan amendments are independent and intentionally preserved.

**Initial evidence**: Public song_values 15/15 and internal identity tests 2/2
passed. Initial cargo check exit 0; one focused Clippy module-inception finding
was corrected by the descriptor alias. Final command/exit/full-log records will
be retained in `tmp/song-mode-riela/SONG-01/foreground-results.json`.

**Remaining verification**: Required independent checker notified that Rust
writes are complete. Finish command evidence and checker reconciliation before
marking this phase complete; plan archive/index updates belong to SONG-16.

### Session: 2026-10-01 — phase verified and complete

All SONG-01 deliverables and phase criteria are complete. Archive/index work
remains assigned to SONG-16; no later song transport/query feature is claimed.

Own foreground commands all exited 0 (full commands/status/logs:
`tmp/song-mode-riela/SONG-01/foreground-results.json`):

| Actual command (CARGO_TERM_QUIET=true) | Exit | Complete log |
|---|---|---|
| mise exec -- cargo check | 0 | tmp/song-mode-riela/SONG-01/check-final.log |
| mise exec -- cargo clippy --lib --test song_values -- -D warnings | 0 | tmp/song-mode-riela/SONG-01/clippy-final.log |
| mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm | 0 | tmp/song-mode-riela/SONG-01/wasm-final.log |
| mise exec -- cargo nextest run -E 'binary(song_values)' (quiet nextest variables enabled) | 0 | tmp/song-mode-riela/SONG-01/song-values-nextest.log |
| mise exec -- cargo test --lib song::identity::tests | 0 | tmp/song-mode-riela/SONG-01/identity-final.log |
| mise exec -- cargo test --doc song::identity | 0 | tmp/song-mode-riela/SONG-01/opaque-handles-doc.log |

Fixtures: 15 public tests, two full-identity/stale-revision tests, and one
opaque-handle compile-fail doctest; each selected run was nonzero.

Independent check-and-test agent cleared constructor/settings/identity audit,
15+2+1 fixtures, nextest15, native and wasm checks, owned formatting,
all-target Clippy and diff check, all exit0. Complete independent command/log
manifest: `/tmp/vactr-song01-checker-results.json`. No outstanding finding.
All owned Rust files remain under1000lines. No foreground session remains.
Fresh numbered post-edit hashes reconcile all owned Rust files; pre-existing
editor/session edits and root's separate plan refinements remain intact.
