# Finite symbolic values implementation plan

**Status**: Ready
**Plan ID**: SONG-01
**Plan Path**: impl-plans/active/song-mode-values.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
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
| `src/song/mod.rs` | Export only the finite value, edit, identity and host contracts as they become available; keep implementation modules separate. | NOT_STARTED |
| `src/song/part.rs` | Define immutable capture, sequence, repeat and edit nodes with checked duration and track keys. No PCM or eager repeat expansion. | NOT_STARTED |
| `src/song/song.rs` | Define Song and frozen timing/seed settings; separate arrangement endpoint from tail deadline. | NOT_STARTED |
| `src/song/limits.rs` | Define construction/work limits and checked duration/frame-capacity validation; use actual advertised host limits. | NOT_STARTED |
| `src/song/identity.rs` | Define PartRevision, full OccurrencePath, EventHandle, SnapshotEpoch, PlacementPath, InstrumentSelector and SongEvent contracts before other consumers. | NOT_STARTED |
| `src/lib.rs` | Expose song module without changing existing exports. | NOT_STARTED |
| `tests/song_values.rs` | Exercise zero-repeat/empty sequence, timed silence, invalid durations/counts and rational overflow using public constructors. | NOT_STARTED |

### Public declaration contract

```rust
pub enum RepeatSeedMode { Same, Vary }
pub struct PartRevision(pub u64);
pub struct SnapshotEpoch(pub u64);
pub struct OccurrencePath { pub producer_ordinals: Vec<u32>, pub cycle: i64, pub onset: Ratio64 }
pub struct PlacementPath(pub Vec<u32>);
pub struct InstrumentSelector { pub family: Vec<Sound> }
pub struct SongEvent { pub handle: EventHandle, pub track: KwId, pub instrument: Sound, pub event: Event, pub placement: PlacementPath, pub tone: Option<i64> }
pub struct SongLimits { pub max_nodes: u32, pub max_depth: u32, pub max_tracks: u32, pub max_cached_events: u32, pub max_frames: u64 }
pub enum PartEdit { ReplaceTrack { track: KwId, pattern: Rc<Pat> }, TransformInstrument { track: KwId, selector: InstrumentSelector, transform: Value }, DeleteEvent(EventHandle), OverwriteRegion { track: KwId, region: TimeSpan, pattern: Rc<Pat> }, InstrumentFx { track: KwId, selector: InstrumentSelector, template: KwId } }
pub struct EventHandle { pub revision: PartRevision, pub occurrence: OccurrencePath, pub tone: u32 }
pub struct SongSettings { pub bpm: Ratio64, pub cycle_beats: Ratio64, pub meter: [u32; 2], pub seed: u64, pub tail_seconds: Ratio64 }
pub enum PartNode { Capture(BTreeMap<KwId, Rc<Pat>>), Sequence(Vec<Rc<Part>>), Repeat { child: Rc<Part>, count: u32, seed_mode: RepeatSeedMode }, Edit { source: Rc<Part>, edit: PartEdit } }
pub struct Part { pub revision: PartRevision, pub duration: Ratio64, pub node: PartNode }
pub struct Song { pub part: Rc<Part>, pub settings: SongSettings }
pub fn capture_part(tracks: BTreeMap<KwId, Rc<Pat>>, duration: Ratio64) -> Result<Part, Failure>;
pub fn sequence(parts: Vec<Rc<Part>>) -> Result<Part, Failure>;
pub fn part_repeat(part: Rc<Part>, count: u32, mode: RepeatSeedMode) -> Result<Part, Failure>;
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

## Phase acceptance criteria

- [ ] Positive capture duration; zero only for empty derived arrangements; unique track keys and checked arithmetic.
- [ ] Root revision is separate from seed identity. Define full producer-path entries, placement ordinals and tone ordinals; no hash-only equality.
- [ ] PartEdit declaration is owned here as data: ReplaceTrack, TransformInstrument, DeleteEvent, OverwriteRegion and InstrumentFx. Execution belongs to SONG-04.
- [ ] Reject nonpositive BPM/cycle beats and invalid meter; default 120 BPM, 4 beats/cycle, 4/4, eight-second tail.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_values)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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
