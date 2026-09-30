# Canonical realization and pure edits implementation plan

**Status**: Ready
**Plan ID**: SONG-04
**Plan Path**: impl-plans/active/song-mode-query-edits.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Canonical realization and pure edits supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-04",
  "planPath": "impl-plans/active/song-mode-query-edits.md",
  "dependsOn": [
    "SONG-01"
  ],
  "writePaths": [
    "src/song/query.rs",
    "src/song/edit.rs",
    "src/song/identity.rs",
    "src/song/mod.rs",
    "tests/song_query.rs",
    "tests/song_edits.rs",
    "impl-plans/active/song-mode-query-edits.md"
  ],
  "sharedPaths": [
    "src/song/identity.rs",
    "src/song/mod.rs",
    "impl-plans/active/song-mode-query-edits.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/identity.rs",
      "intendedEdit": "Complete path derivation helpers; mono-note expansion retains individual tone ordinal and revision checks. Owners execute serially: SONG-01, SONG-04"
    },
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Export query/edit APIs after contracts exist. Owners execute serially: SONG-01, SONG-04, SONG-06, SONG-08, SONG-12"
    },
    {
      "path": "impl-plans/active/song-mode-query-edits.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-01](song-mode-values.md)
- **Next**: [SONG-05](song-mode-natives.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-01 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |

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
| `src/song/query.rs` | Canonical whole-cycle source realization with frozen QueryVm, bounded work/cache and caller clipping; local/global placement mapping and stable seeds. | NOT_STARTED |
| `src/song/edit.rs` | Implement immutable edit builders and execution; selectors operate on source instrument identity before transformation. | NOT_STARTED |
| `src/song/identity.rs` | Complete path derivation helpers; mono-note expansion retains individual tone ordinal and revision checks. | NOT_STARTED |
| `src/song/mod.rs` | Export query/edit APIs after contracts exist. | NOT_STARTED |
| `tests/song_query.rs` | Compare normalized union for many query partitions, fractional part edges, seed modes, equal simultaneous twins and callback-size independence. | NOT_STARTED |
| `tests/song_edits.rs` | Verify no input mutation, track replace, bd selection, one-tone deletion, stale handles and onset-based regional overwrite. | NOT_STARTED |

### Public declaration contract

```rust
pub struct SongQueryCtx<'a> { pub vm: &'a mut dyn QueryVm, pub cells: &'a InputCells, pub seed: u64, pub tempo: Tempo, pub limits: &'a SongLimits }
pub fn query_part(part: &Part, span: TimeSpan, cx: &mut SongQueryCtx<'_>) -> Result<Vec<SongEvent>, Failure>;
pub fn replace_track(part: &Part, track: KwId, replacement: Rc<Pat>) -> Result<Part, Failure>;
pub fn transform_instrument(part: &Part, track: KwId, selector: InstrumentSelector, transform: Value) -> Result<Part, Failure>;
pub fn delete_event(part: &Part, handle: &EventHandle) -> Result<Part, Failure>;
pub fn overwrite_region(part: &Part, track: KwId, region: TimeSpan, replacement: Rc<Pat>) -> Result<Part, Failure>;
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

- [ ] IDs assigned before query clipping/filtering; whole spans unchanged and continuations never retrigger.
- [ ] Chord expanded once before edits; identical pitches have distinct tone IDs and mono commit marker.
- [ ] Overwrite removes complete events with onset in [begin,end); retains earlier sustaining notes and onsets at end; inserts replacement at begin.
- [ ] Same repeats preserve local draws; Vary derives from full placement path; caller window/order/cache eviction never changes draws.
- [ ] Existing query faults or budget failures fail song query, rather than returning successful partial music. Current query bounds 256/64/1,000,000 remain enforced.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_query) | binary(song_edits)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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
