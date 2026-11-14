# Song native execution implementation plan

**Status**: Completed
**Plan ID**: SONG-05
**Plan Path**: impl-plans/active/song-mode-natives.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-01
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#proposed-language-surface)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Song native execution supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-05",
  "planPath": "impl-plans/active/song-mode-natives.md",
  "dependsOn": [
    "SONG-03",
    "SONG-04"
  ],
  "writePaths": [
    "src/vm/natives/song.rs",
    "src/vm/natives/mod.rs",
    "src/ns/stage.rs",
    "src/sched/runtime.rs",
    "tests/song_natives.rs",
    "impl-plans/active/song-mode-natives.md",
    "src/ns/evaluator.rs"
  ],
  "sharedPaths": [
    "src/sched/runtime.rs",
    "impl-plans/active/song-mode-natives.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/sched/runtime.rs",
      "intendedEdit": "Handle PlaySong exhaustively with a pending immutable descriptor; report playback unsupported until SONG-11 consumes it. Do not activate early or return false success. Owners execute serially: SONG-05, SONG-08, SONG-11"
    },
    {
      "path": "impl-plans/active/song-mode-natives.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-03](song-mode-checker.md), [SONG-04](song-mode-query-edits.md)
- **Next**: [SONG-06](song-mode-snapshot-contracts.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-03 | Reviewed declarations, passing phase checks and recorded post-edit hashes | VERIFIED |
| SONG-04 | Reviewed declarations, passing phase checks and recorded post-edit hashes | VERIFIED |

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
| `src/vm/natives/song.rs` | Register thin song natives; force structural arguments and resolve thunks once; use pure callback construction for edits. | Completed |
| `src/vm/natives/mod.rs` | Register song native module after existing natives; do not replace repeat or cat. | Completed |
| `src/ns/stage.rs` | Add PlaySong staged effect carrying immutable Song; ensure form failure discards it. | Completed |
| `src/sched/runtime.rs` | Handle PlaySong exhaustively with a pending immutable descriptor; report playback unsupported until SONG-11 consumes it. Do not activate early or return false success. | Completed |
| `src/ns/evaluator.rs` | Classify PlaySong as non-replayable during reactive rebuilds; explicit staging only. | Completed |
| `tests/song_natives.rs` | Evaluate nested part generators and all proposed APIs; check returned values and staging rollback. | Completed |

### Public declaration contract

```rust
pub fn register(prelude: &mut Prelude);
pub enum StagedEffect { PlaySong(Rc<Song>) }
pub fn instrument_fx(part: &Part, track: KwId, selector: InstrumentSelector, template: KwId) -> Result<Part, Failure>;
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

- [x] The enum block lists only the new StagedEffect variant; preserve every existing variant.
- [x] Exercise the accepted 24-cycle example and function-within-function partial overwrite/delete.
- [x] PlaySong only stages playback; constructing/editing Part has no audio or namespace mutation.
- [x] Returned event descriptors contain opaque handles; no mutable event vector is exposed.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_natives)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — reactive replay ownership
Source-grounded preflight found evaluator::one_shot controls non-replayable
staged effects. Add PlaySong there and prove reactive dependent rebuilds do not
replay playback. This file is explicitly owned before implementation begins.

### Session: 2026-10-01 — authorized native implementation
Prerequisites independently verified: SONG-03 `/tmp/vactr-song03-04b-checker-results.json`; SONG-04 `/tmp/vactr-song04-independent-001/final-results.json` (153 distinct fixtures and 32 nextest repeats; all twelve gates exit 0). Implementing only the six owned Rust paths. Structural forcing uses a bounded iterative work stack; callable pattern transforms use an immediate effect-restricted VmQuery borrow, without a snapshot claim. Runtime retains explicit pending requests and reports unsupported playback until SONG-11. Archive/index deferred.

### Session: 2026-10-01 — SONG-05 author seal, independent verification pending
**Implemented**: All eleven native registrations, checked immutable constructors/edits, full selected-family kit resolution plus registry fallback, dictionary event descriptors with certified handles and original numeric note types, declared FX-template validation. Structural traversal uses a bounded explicit work stack, charges scheduled children before allocating queues, and evaluates actual nested Thunks once in Query mode with prior captured output restored. Pattern/function internals remain lazy. Existing eager evaluation of literal brace expressions before a native call is preserved; fixtures for deferred structural forcing use actual Value::Thunk closures rather than claiming those literal blocks were lazy.
**Staging**: PlaySong carries Rc<Song>; failed forms discard it; reactive owners are marked non-replayable. Runtime retains a pending immutable descriptor and returns explicit BeyondCapability until SONG-11 adds playback. No early activation or completed-playback claim.
**Behavioral evidence**: 19 native fixtures, 12 checker fixtures, 7 value-integration fixtures and 97 existing VM unit fixtures = 135 distinct tests, plus 19 nextest repeats. Includes accepted 24-cycle nested generators, function-within-function deletion/overwrite, fractional descriptors, stale handles, zero/billion symbolic repeats, timed silence, dynamic invalid settings/counts/regions/equal bounds, all native arities and unknown named arguments, complete bank freezing, selected-only kit evaluation, explicit effect/output rejection and restoration, actual once-only structural/callback execution, pre-admission bounds for wide nested containers, FX template validation, staging rollback, unsupported-runtime request retention, and reactive dependency/read-edge/non-replay proof.
**Commands and terminal evidence**: `author-sealed-check-results.json` under `tmp/song-mode-riela/SONG-05/` lists exact commands, exits, timestamps and complete logs. All eight final gates exit 0: quiet `mise run check`; quiet host-wasm cargo check; focused cargo test (38 fixtures); cargo test --lib vm:: (97); quiet mise run clippy; nextest binary(song_natives) (19); scoped rustfmt check; scoped diff check. Author runner session 36997 polled to terminal exit 0; corrected Clippy session 92174 polled to terminal exit 0. The initial Clippy 101 from parallel SONG-06A type aliases is retained in `author-check-results.json`/`author-clippy.log`; its owner corrected their files and the rerun passed. Earlier compile/fixture failures and malformed/eager-literal assumptions are retained in tests-first through tests-eighth logs; tests-ninth has nineteen passing fixtures before final strengthening of reactive assertions, which final gates also pass.
**Integrity**: Exact six Rust paths are sealed in `author-source-seal.json`, all below 1000 lines; immutable intent records 0001–0003 retain baseline/design/plan and post-edit hashes. No dependency, Git, unrelated editor, session/publish or other phase Rust modifications. Independent checker handoff is required before phase completion. Archive/index deferred to root reconciliation.

### Session: 2026-10-01 — independent SONG-05 verification completed
**Status**: Completed (native construction/staging phase only).
**Independent evidence**: `/tmp/vactr-song05-independent-001/final-results.json`; all eight independent gates exit 0, 135 distinct fixtures plus 19 nextest repeats. Checker session 13281 was polled to terminal exit 0, with no foreground processes remaining. All six Rust SHA-256 values match the author seal both before and after verification; maximum touched Rust length is 822 lines. Full independent command/log paths, log hashes, source hashes and fixture counts are in the immutable result manifest. No outstanding review findings.
**Completion**: TASK-001, TASK-002 and TASK-003 and all phase criteria are complete. The accepted eleven native APIs execute through the ordinary VM; the synchronous construction context deliberately is not a transitive frozen snapshot. Runtime playback remains an explicit pending descriptor plus unsupported diagnostic until SONG-11, as required by this phase contract.
**Preservation and next phase**: This completion batch changes only this owned plan after a fresh immutable intent/hash verification; no Rust, archive, index or Git edits. SONG-06 implementation remains held until SONG-06A is independently cleared and root authorizes the next phase. Full-song playback/export completion is outside this completed phase.
