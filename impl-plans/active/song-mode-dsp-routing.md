# Private branches, tails and audio gates implementation plan

**Status**: Ready
**Plan ID**: SONG-09
**Plan Path**: impl-plans/active/song-mode-dsp-routing.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Private branches, tails and audio gates supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-09",
  "planPath": "impl-plans/active/song-mode-dsp-routing.md",
  "dependsOn": [
    "SONG-08"
  ],
  "writePaths": [
    "src/dsp/song.rs",
    "src/dsp/mod.rs",
    "src/dsp/engine.rs",
    "src/dsp/engine/render.rs",
    "src/dsp/bus.rs",
    "src/dsp/arena.rs",
    "src/song/routing.rs",
    "tests/song_dsp.rs",
    "impl-plans/active/song-mode-dsp-routing.md"
  ],
  "sharedPaths": [
    "src/dsp/engine.rs",
    "src/song/routing.rs",
    "impl-plans/active/song-mode-dsp-routing.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/dsp/engine.rs",
      "intendedEdit": "Dispatch song activation/mute/end commands and acknowledge actual applied frame. Owners execute serially: SONG-08, SONG-09"
    },
    {
      "path": "src/song/routing.rs",
      "intendedEdit": "Validate all arrangement transitions/overlap against prepared capacities; no eager repeat expansion. Owners execute serially: SONG-08, SONG-09"
    },
    {
      "path": "impl-plans/active/song-mode-dsp-routing.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-08](song-mode-audio-contracts.md)
- **Next**: [SONG-10](song-mode-host-adapters.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-08 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |

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
| `src/dsp/song.rs` | Preallocated instrument branches with private effects, gain gates and finite retiring deadlines. | NOT_STARTED |
| `src/dsp/mod.rs` | Register DSP song module. | NOT_STARTED |
| `src/dsp/engine.rs` | Dispatch song activation/mute/end commands and acknowledge actual applied frame. | NOT_STARTED |
| `src/dsp/engine/render.rs` | Mix private branches into declared track buses and master; redirect song room/delay out of shared orbit paths. | NOT_STARTED |
| `src/dsp/bus.rs` | Keep song generation state alive to its tail deadline independently of voice refcount; reset before reuse. | NOT_STARTED |
| `src/dsp/arena.rs` | Encode private prepared route graphs and lifecycle resources; reject malformed or undersized arena payloads before active retirement. | NOT_STARTED |
| `src/song/routing.rs` | Validate all arrangement transitions/overlap against prepared capacities; no eager repeat expansion. | NOT_STARTED |
| `tests/song_dsp.rs` | Impulse and sibling probes, hard mute, stale epoch, callback partitions and tail deadline tests. | NOT_STARTED |

### Public declaration contract

```rust
pub struct SongBranchState { pub id: SongBranchId, pub epoch: SnapshotEpoch, pub instrument: u32, pub tail_end_frame: u64 }
pub fn activate_song(command: SongActivation) -> Result<(), Failure>;
pub fn apply_song_mute(command: SongMute) -> Result<(), Failure>;
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

- [ ] Only bd enters bd effect state; sd/hh branch signal before shared master unchanged by bd-private configuration.
- [ ] Room and delay belong to selected branch; shared master nonlinearity explicitly outside isolation promise.
- [ ] Mute gate reaches zero within 64 frames, suppresses queued matching voices, clears private state before unmute; unmute only admits future onsets.
- [ ] Changed configuration drains old branch privately; unchanged adjacent configuration may retain state.
- [ ] No allocation/VM work in callback; reject resources before old active state is retired.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_dsp)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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
