# Value and VM compatibility implementation plan

**Status**: Completed
**Plan ID**: SONG-02
**Plan Path**: impl-plans/active/song-mode-value-integration.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-01
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#proposed-language-surface)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Value and VM compatibility supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-02",
  "planPath": "impl-plans/active/song-mode-value-integration.md",
  "dependsOn": [
    "SONG-01"
  ],
  "writePaths": [
    "src/value/value.rs",
    "src/value/eq.rs",
    "src/value/print.rs",
    "src/vm/call.rs",
    "src/pattern/build.rs",
    "src/value/tests/mod.rs",
    "tests/song_value_integration.rs",
    "impl-plans/active/song-mode-value-integration.md"
  ],
  "sharedPaths": [
    "impl-plans/active/song-mode-value-integration.md"
  ],
  "sharedPathNotes": [
    {
      "path": "impl-plans/active/song-mode-value-integration.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-01](song-mode-values.md)
- **Next**: [SONG-03](song-mode-checker.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-01 | Reviewed declarations, passing phase checks and recorded post-edit hashes | COMPLETE — independent checker `/tmp/vactr-song01-checker-results.json` |

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
| `src/value/value.rs` | Add immutable Rc<Part>, Rc<Song> and EventHandle value variants. | COMPLETE |
| `src/value/eq.rs` | Keep Part/Song opaque for deep equality like Pattern; compare event handles by full revision/path/tone. | COMPLETE |
| `src/value/print.rs` | Print stable opaque summaries, never recursive whole DAG dumps. | COMPLETE |
| `src/vm/call.rs` | Add kind names and reject calling Part/Song/handles as generic functions; do not alter closure calling semantics. | COMPLETE |
| `src/pattern/build.rs` | Reject Part/Song/EventHandle in implicit pattern and numeric parameter coercions; preserve existing scalar behavior. | COMPLETE |
| `src/value/tests/mod.rs` | Extend existing variant inventory and variant-kind exhaustive matches. | COMPLETE |
| `tests/song_value_integration.rs` | Check equality, printing, type names, opaque failure and existing callable list/dict compatibility. | COMPLETE |

### Public declaration contract

```rust
pub enum Value { Part(Rc<Part>), Song(Rc<Song>), EventHandle(Rc<EventHandle>) }
pub fn deep_eq(a: &Value, b: &Value) -> Result<bool, Failure>;
pub fn kind_name(value: &Value) -> &'static str;
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

- [x] Existing list repeat, sound-first coercion and Pattern behavior are unchanged.
- [x] Do not introduce structural equality for closures or whole pattern DAGs.
- [x] New variants are accepted only by explicit song APIs, not implicit pattern coercion. The enum block lists only added Value variants; preserve all existing variants.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_value_integration)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — source-grounded ownership amendment
Root inspection found pattern_of and param_of wildcard scalar fallbacks. Explicitly
own build.rs to reject finite song values at those boundaries, with behavioral
fixtures. This is required by the existing no-implicit-coercion acceptance criterion.

### Session: 2026-10-01 — finite runtime value integration
**Tasks completed**: Value variants, opaque Part/Song equality, full EventHandle
equality, fixed opaque printing, kind diagnostics and static container coercion
guards. No additional exhaustive-match ownership was needed. Iterative coercion
validation bounds depth at 256 and work at 1,000,000 visited values; it does not
execute lazy callbacks or traverse pattern DAGs. Existing scalar, sound-first,
Pattern and collection callable behavior is preserved.

**Evidence**: `CARGO_TERM_QUIET=true mise exec -- cargo check` exited 0;
complete output `tmp/song-mode-riela/SONG-02/check-first.log`.
`CARGO_TERM_QUIET=true mise exec -- cargo test --test song_value_integration`
exited 0, seven fixtures;
`tmp/song-mode-riela/SONG-02/integration-second.log`.
The first combined test command selected only 65 library value tests and zero
integration fixtures (exit 0), so that run alone was not counted as integration
evidence. Initial dedicated integration run failed one incorrectly expressed
identity fixture: bare parameter statements invoke a zero-argument call in
existing block semantics. Corrected it to `first [x]`, preserving actual nested
calls `identity {identity x}` without changing closure forcing.

**Preservation/freshness**: Immutable records 0001 and 0002 retain exact baselines
and post-edit hashes. Only seven authorized Rust paths changed, all below 1000
lines. Trace worker ownership and pre-existing editor/session work are preserved.
**Remaining**: independent native/browser compilation, warning gates and focused
checks; final phase status follows checker evidence. Archive/index remains SONG-16.

### Session: 2026-10-01 — independent static phase verification
**Status**: SONG-02 completed; archive/index deferred to SONG-16. Root assigned
query-time rejection of dynamically returned finite values to trace prerequisite
SONG-04A, whose joined correction is still pending; this phase implements the
finite values and static coercion boundaries, not query evaluation. Do not
interpret this status as completion of song playback or dynamic realization.

**Independent checker**: All listed commands exited 0, retained complete logs:
- `cargo test --lib value::tests`: 66 tests,
  `/tmp/vactr-song02-value-unit.log`.
- Focused song integration/trace tests: 7 + 7 tests,
  `/tmp/vactr-song02-04a-focused.log`.
- nextest selected both song binaries: 14 tests,
  `/tmp/vactr-song02-04a-nextest.log`.
- Native `cargo check`, browser target `cargo check`, owned-path rustfmt check,
  and `cargo clippy --all-targets -- -D warnings`:
  `/tmp/vactr-song02-04a-native.log`, `/tmp/vactr-song02-04a-wasm.log`,
  `/tmp/vactr-song02-04a-format.log`, `/tmp/vactr-song02-04a-clippy.log`.
All Cargo commands used CARGO_TERM_QUIET=true through mise. No foreground
sessions remain. Final joined checker manifest will follow A's correction.

**Owner evidence**: 66 library value tests exited 0, complete log
`tmp/song-mode-riela/SONG-02/value-tests-final.log`; actual command/status ledger
`tmp/song-mode-riela/SONG-02/foreground-results.json`. Freshness records 0001–0004
cover all exact authorized edits, and owned files remain below 1000 lines.

### Session: 2026-10-01 — sealed joined verification
Independent joined correction cleared: `/tmp/vactr-song02-04a-checker-results.json`.
All recorded native/browser, formatter and all-target warning gates exited 0;
147 distinct fixtures passed. A now rejects dynamically resolved finite values
at reference/callback/parameter/atomic-step boundaries using the shared bounded
`pub(crate) reject_finite` validator. Guard visibility is the only additional
SONG-02 Rust edit (immutable record 0005); query implementation belongs A.
This resolves the previously recorded dynamic rejection finding. No source,
pattern-DAG or identity contract remains provisional within SONG-02.
