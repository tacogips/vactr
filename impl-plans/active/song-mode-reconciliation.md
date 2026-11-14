# Serial integration evidence and documentation implementation plan

**Status**: Ready
**Plan ID**: SONG-16
**Plan Path**: impl-plans/active/song-mode-reconciliation.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#plan-author-handoff)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Serial integration evidence and documentation supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-16",
  "planPath": "impl-plans/active/song-mode-reconciliation.md",
  "dependsOn": [
    "SONG-13",
    "SONG-15"
  ],
  "writePaths": [
    "tests/song_end_to_end.rs",
    "examples/song-mode.vact",
    "design-docs/specs/command.md",
    "design-docs/specs/design-song-mode.md",
    "impl-plans/README.md",
    "impl-plans/active/song-mode-reconciliation.md",
    "impl-plans/active/song-mode-values.md",
    "impl-plans/completed/song-mode-values.md",
    "impl-plans/active/song-mode-value-integration.md",
    "impl-plans/completed/song-mode-value-integration.md",
    "impl-plans/active/song-mode-checker.md",
    "impl-plans/completed/song-mode-checker.md",
    "impl-plans/active/song-mode-query-edits.md",
    "impl-plans/completed/song-mode-query-edits.md",
    "impl-plans/active/song-mode-natives.md",
    "impl-plans/completed/song-mode-natives.md",
    "impl-plans/active/song-mode-snapshot-contracts.md",
    "impl-plans/completed/song-mode-snapshot-contracts.md",
    "impl-plans/active/song-mode-candidate-evaluation.md",
    "impl-plans/completed/song-mode-candidate-evaluation.md",
    "impl-plans/active/song-mode-audio-contracts.md",
    "impl-plans/completed/song-mode-audio-contracts.md",
    "impl-plans/active/song-mode-dsp-routing.md",
    "impl-plans/completed/song-mode-dsp-routing.md",
    "impl-plans/active/song-mode-host-adapters.md",
    "impl-plans/completed/song-mode-host-adapters.md",
    "impl-plans/active/song-mode-transport.md",
    "impl-plans/completed/song-mode-transport.md",
    "impl-plans/active/song-mode-export-core.md",
    "impl-plans/completed/song-mode-export-core.md",
    "impl-plans/active/song-mode-cli.md",
    "impl-plans/completed/song-mode-cli.md",
    "impl-plans/active/song-mode-browser-session.md",
    "impl-plans/completed/song-mode-browser-session.md",
    "impl-plans/active/song-mode-editor-controls.md",
    "impl-plans/completed/song-mode-editor-controls.md",
    "impl-plans/completed/song-mode-reconciliation.md"
  ],
  "sharedPaths": [
    "impl-plans/active/song-mode-values.md",
    "impl-plans/active/song-mode-value-integration.md",
    "impl-plans/active/song-mode-checker.md",
    "impl-plans/active/song-mode-query-edits.md",
    "impl-plans/active/song-mode-natives.md",
    "impl-plans/active/song-mode-snapshot-contracts.md",
    "impl-plans/active/song-mode-candidate-evaluation.md",
    "impl-plans/active/song-mode-audio-contracts.md",
    "impl-plans/active/song-mode-dsp-routing.md",
    "impl-plans/active/song-mode-host-adapters.md",
    "impl-plans/active/song-mode-transport.md",
    "impl-plans/active/song-mode-export-core.md",
    "impl-plans/active/song-mode-cli.md",
    "impl-plans/active/song-mode-browser-session.md",
    "impl-plans/active/song-mode-editor-controls.md"
  ],
  "sharedPathNotes": [
    {
      "path": "impl-plans/active/song-mode-values.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-value-integration.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-checker.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-query-edits.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-natives.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-snapshot-contracts.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-candidate-evaluation.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-audio-contracts.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-dsp-routing.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-host-adapters.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-transport.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-export-core.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-cli.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-browser-session.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-editor-controls.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-13](song-mode-cli.md), [SONG-15](song-mode-editor-controls.md)
- **Next**: Serial completion

| Dependency | Required output | Status |
|---|---|---|
| SONG-13 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |
| SONG-15 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |

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
| `tests/song_end_to_end.rs` | Exercise real nested generator/edit/sequence, native render and transport fault scenarios using common public entrypoints. | NOT_STARTED |
| `examples/song-mode.vact` | Add accepted 24-cycle example plus targeted edits and declared bd-private chain using only implemented proposed natives. | NOT_STARTED |
| `design-docs/specs/command.md` | Document finite run endpoint, render flags/exit behavior and new session messages after implementation. | NOT_STARTED |
| `design-docs/specs/design-song-mode.md` | Record implemented behavior and verification evidence; keep accepted decisions unless review authorizes change. | NOT_STARTED |
| `impl-plans/README.md` | Serially reconcile song entries/statuses; preserve unrelated existing index edits. | NOT_STARTED |

### Public declaration contract

```rust
pub struct SongIntegrationEvidence { pub arrangement_frames: u64, pub tail_frames: u64, pub native_browser_event_digest: String }
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

- [ ] Recheck all joined outputs against immutable intent snapshots and hashes, repair drift serially, then rerun only impacted tests.
- [ ] End-to-end fixture asserts single-note/sibling preservation, automatic finite completion, native frame counts and browser command/event parity.
- [ ] No lockfile/dependency changes required. Broad formatting only serially and only within authorized modified files; record any unavoidable existing-file split.
- [ ] Global archiving occurs only after complete verification; keep concrete completed plan destinations recorded in this plan. No concurrent Git operation.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_end_to_end)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |
| `CARGO_TERM_QUIET=true mise run fmt-check` | Serial formatting reconciliation covers only authorized edits. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise run test` | Full joined Rust suite passes, with complete foreground log. |
| `mise exec -- npm --prefix editor run test` | Full joined editor suite passes. |
| `CARGO_TERM_QUIET=true mise exec -- cargo run -- run examples/song-mode.vact --host noop` | Ends automatically after declared arrangement/tail, with no --cycles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo run -- render examples/song-mode.vact tmp/song-mode-riela/song.wav --sample-rate 48000` | Native headless complete-song WAV and metadata match 24 cycles at 120 BPM plus eight-second tail: 2,304,000 arrangement frames + 384,000 tail frames. |

### Serial completion ownership

Only after all phase evidence passes, reconcile impl-plans/README.md and move each listed song-mode plan from impl-plans/active/ to the same basename under impl-plans/completed/. Each destination is explicitly recorded in the execution manifest before moving. No other plan may edit the index or perform archiving. These moves are deferred completion work, not actions in this planning node.

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
