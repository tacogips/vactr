# Checker types and native signatures implementation plan

**Status**: Completed
**Plan ID**: SONG-03
**Plan Path**: impl-plans/active/song-mode-checker.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#proposed-language-surface)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Checker types and native signatures supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-03",
  "planPath": "impl-plans/active/song-mode-checker.md",
  "dependsOn": [
    "SONG-02"
  ],
  "writePaths": [
    "src/types/ty.rs",
    "src/types/infer_call.rs",
    "src/types/natives_domain.rs",
    "tests/song_checker.rs",
    "impl-plans/active/song-mode-checker.md",
    "src/types/unify.rs",
    "src/types/song_rules.rs"
  ],
  "sharedPaths": [
    "impl-plans/active/song-mode-checker.md"
  ],
  "sharedPathNotes": [
    {
      "path": "impl-plans/active/song-mode-checker.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-02](song-mode-value-integration.md)
- **Next**: [SONG-05](song-mode-natives.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-02 | Reviewed declarations, passing phase checks and recorded post-edit hashes | COMPLETE: `/tmp/vactr-song02-04a-checker-results.json` |

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
| `src/types/ty.rs` | Add Part, Song and EventHandle nominal leaf types; support scheme parsing and display. | COMPLETE |
| `src/types/infer_call.rs` | Give song constructors/edits explicit result types and callback checks; no automatic Part-to-Pattern coercion. | COMPLETE |
| `src/types/natives_domain.rs` | Declare exact arity, named args, forcing masks and capability needs for every proposed song native. | COMPLETE |
| `src/types/unify.rs` | Reject finite leaves in implicit pattern-step unification; preserve explicit any values. | COMPLETE |
| `src/types/song_rules.rs` | Bounded song-native literal validation extracted from infer_call.rs, retaining all eleven signatures. | COMPLETE |
| `tests/song_checker.rs` | Typecheck accepted syntax and reject malformed handles, callbacks, meter, counts, keys and accidental existing repeat/cat changes. | COMPLETE |

### Public declaration contract

```rust
pub enum Ty { Part, Song, EventHandle }
pub fn check(nodes: &[Node], env: &CheckEnv, manifest: &HostManifest) -> CheckResult;
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

- [x] The enum block lists only added Ty variants; preserve all existing variants.
- [x] Native table covers part, part-repeat, sequence, replace-track, transform-instrument, part-events, delete-event, overwrite-region, instrument-fx, song and play-song.
- [x] Duration/count/selector values are eager; transform callback is retained callable. Reject invalid literal inputs at checking and dynamic inputs at runtime.
- [x] Existing parser needs no grammar changes; brace groups, tab fn bodies, dict pairs and subject-first pipes stay authoritative.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_checker)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — descriptor contract clarification
The accepted design returns read-only dictionary descriptors from part-events,
with the opaque EventHandle under :handle, alongside local whole span, instrument
and individual note value. Its result scheme must describe descriptors rather
than claim bare handles. delete-event continues to require the explicit handle.

### Session: 2026-10-01 — checker ownership amendment
Source inspection found unify_step accepts arbitrary scalar leaves against
pattern any. Add explicit finite-type rejection in unify.rs. Put song-native
literal validation in song_rules.rs registered locally from infer_call.rs to
keep touched sources below1000lines; no unrelated module registry edits.

### Session: 2026-10-01 — checker implementation and owner evidence
**Completed**: Part/Song/EventHandle nominal leaf types, scheme parsing/display;
eleven native entries appended after every existing native ID, with exact
arity/keyword/mask contracts. Only play-song is effectful; all entries are
portable construction/playback APIs (no native-only export capability).
part-events returns `[ [keyword: any] ]` descriptors; its opaque handle value is
under :handle, not a bare-handle result list.

Literal validation requires explicit positive exact duration and keyword track
dictionaries, detects duplicate keys/arguments, rejects fractional/negative/
overflowing counts and unsupported seed modes, validates metadata/meter, pure
pattern transforms, opaque-handle arguments, and strict region/known-track
bounds. Region comparisons and integral ratio admission inspect exact rational
values rather than rounded floating values. Known finite return values of lazy
callables also fail Pattern coercion; ordinary Any/collection/function value
transport remains legal. Runtime dynamic validation belongs SONG-05.

**Source-grounded syntax refinement**: A zero-parameter user generator is
explicitly invoked with `{drums & []}`. Bare `drums` retains the Fn; `{drums}`
alone is a block returning that Fn. Empty splat is existing syntax, confirmed
through actual evaluation of a zero-argument list generator and nested song
checker fixture. No parser or closure semantics changed. Root updated the
accepted design example.

**Owner commands**: All following exited 0 through mise with quiet Cargo:
- `cargo test --test song_checker`: 10 passed;
  `tmp/song-mode-riela/SONG-03/tests-third.log`.
- `cargo check`: `tmp/song-mode-riela/SONG-03/native-final.log`.
- `cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm`:
  `tmp/song-mode-riela/SONG-03/wasm-final.log`.
- `cargo clippy --all-targets -- -D warnings`:
  `tmp/song-mode-riela/SONG-03/clippy-final.log`.
- `cargo test --lib types::tests`: 136 passed;
  `tmp/song-mode-riela/SONG-03/types-final.log`.
- nextest `-E 'binary(song_checker)'`: 10 passed;
  `tmp/song-mode-riela/SONG-03/nextest-final.log`.
Exact command/status ledger: `tmp/song-mode-riela/SONG-03/author-check-results.json`.
All foreground sessions reached terminal exit. Historic initial fixture failures
and full type-clash diagnostics remain in tests-first/second.log and
 generator-diagnostics.log; they are not reported as passing evidence.

**Freshness/preservation**: Immutable records 0001–0004 cover six authorized
Rust paths and own plan. Largest touched source is 782 lines. Scoped formatting
only; parallel combinator traces and unrelated editor/session changes retained.
**Remaining**: independent checker verification, final phase progress evidence;
archive/index deferred to SONG-16.

### Session: 2026-10-01 — grouped literal and purity evidence
Owner follow-up found that eager single-expression braces could bypass direct
literal bounds. Added an iterative 256-level unwrap for static groups, retaining
multi-statement/function bodies as dynamic inputs. Twelve fixtures now pass,
including grouped invalid durations/counts/settings/regions and a positive grouped
settings fixture. The purity fixture uses a known captured :drums track and a d1
callback returning the same Pattern; it asserts exactly the purity diagnostic,
paired with a passing gain callback, so unknown-track/result-type errors cannot
mask missing purity enforcement.

Final corrected owner gates all exited 0, sessions 47387 and 47289 are terminal:
`cargo test --test song_checker` (12), native check, browser host check, all-target
Clippy, `cargo test --lib types::tests` (136), nextest selected song_checker (12).
Full logs `tmp/song-mode-riela/SONG-03/tests-grouped.log`,
`native-grouped.log`, `wasm-grouped.log`, `clippy-grouped.log`,
`types-grouped.log`, `nextest-grouped.log` in that same directory. Actual command
ledger: `tmp/song-mode-riela/SONG-03/author-grouped-check-results.json`.
Immutable record 0005 contains the two corrected Rust paths; 0006 records this
progress. Independent joined checker remains pending the parallel trace repair.

### Session: 2026-10-01 — independent phase completion
Mandatory checker cleared the stable joined SONG-03/04B/helper-repair tree:
`/tmp/vactr-song03-04b-checker-results.json`. It records 242 distinct passing
fixtures (33 selected song/trace, 136 type, 73 pattern), and all native/host-wasm,
owned formatter, all-target Clippy and nextest commands exited 0. Complete logs
are listed in that manifest; there are no active foreground sessions. The
checker specifically verified all eleven masks/signatures, grouped literals,
purity, and preservation of existing depth/stack behavior. Runtime song natives
remain the explicit downstream SONG-05 phase, not claimed by this checker phase.

All seven owned target hashes were reconciled after progress; Rust files remain
below 1000 lines, existing native IDs are preserved by append-only registration,
and unrelated editor/session/trace-worker changes are retained. Phase completed;
archive/index remains the SONG-16 serial join responsibility.
