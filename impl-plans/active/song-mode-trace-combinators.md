# Producer trace propagation through combinators implementation plan

**Status**: Completed
**Plan ID**: SONG-04B
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance)
**Created**: 2026-10-01
**Last Updated**: 2026-10-01

## Intent and source evidence

SONG-04 requires complete collision-independent producer identity before event selection.
Current OccKey stores compact NodeId hashes and cannot prove that contract by itself.
This bounded prerequisite preserves the existing scheduler identity and adds optional
full tracing only for song realization. It does not change the accepted end behavior.

## Manifest

```json
{
  "planId": "SONG-04B",
  "planPath": "impl-plans/active/song-mode-trace-combinators.md",
  "dependsOn": [
    "SONG-04A"
  ],
  "writePaths": [
    "src/pattern/combinators/control.rs",
    "src/pattern/combinators/music.rs",
    "src/pattern/combinators/random.rs",
    "src/pattern/combinators/region.rs",
    "src/pattern/combinators/sound.rs",
    "src/pattern/combinators/structure.rs",
    "src/pattern/combinators/time.rs",
    "tests/song_trace_combinators.rs",
    "impl-plans/active/song-mode-trace-combinators.md"
  ],
  "sharedPaths": [],
  "sharedPathNotes": []
}
```

## Related plans and dependencies

- **Depends On**: SONG-04A
- **Next**: SONG-04

| Dependency | Required contract | Status |
|---|---|---|
| SONG-04A | Verified preceding phase and immutable current source | Completed |

## Execution and preservation contract

This is a future implementation contract. This planning run changes documents only and executes no future tests.
Prerequisite before native Riela implementation/review fanout: accepted design and reviewed plans must be committed in a separately authorized serial execution. This specialized design/plan workflow does not perform that commit or Git finalization.
Use the repository rust-coding agent for Rust changes and invoke check-and-test-after-modify after every Rust modification batch. Use design-doc/impl-plan skills for documentation and progress.
Before each edit, freshly read the exact target and its dependencies; record SHA-256 before/after in an immutable intent record under tmp/song-mode-riela/<planId>/<taskId>.json. The record contains accepted design hash, plan hash, task ID, exact intended change, target paths and baseline hashes. Do not overwrite intent records: create a new numbered record for revisions.
Compare the fresh pre-edit hash with the recorded baseline immediately before writing. If it drifted, stop that edit, reread the current file and reconcile the intended diff; never restore a stale whole-file copy. Preserve all pre-existing staged, tracked and untracked work. Recheck post-edit hashes at join; unexpected drift requires serial repair by the reconciler, not another worker overwrite.
Only edit listed writePaths. An additional compile-required exhaustive match or file split needs a serial manifest/plan amendment before work resumes; no implicit broad cleanup permission. If a touched Rust source reaches 1000 lines, split it under repository standards and record exact new paths before editing. Current inspected principal integration files are below 1000 lines.
Use the same branch and workspace. No worktrees, private branches, concurrent Git operations, dependency additions or lockfile generation are prescribed. Shared indexes, broad formatting, archive moves and cross-worker repair belong exclusively to SONG-16 after joining.
Each worker updates only its own plan's status, completion checkboxes and dated progress log, including actual command, exit code and complete foreground log path. Retain/poll every foreground session to terminal exit; no orphan processes or detached shell work.


## Modules and declarations

| File | Deliverable | Status |
|---|---|---|
| `src/pattern/combinators/control.rs` | Optional full producer trace integration; preserve existing untraced behavior. | Implemented |
| `src/pattern/combinators/music.rs` | Optional full producer trace integration; preserve existing untraced behavior. | Implemented |
| `src/pattern/combinators/random.rs` | Optional full producer trace integration; preserve existing untraced behavior. | Implemented |
| `src/pattern/combinators/region.rs` | Optional full producer trace integration; preserve existing untraced behavior. | Implemented |
| `src/pattern/combinators/sound.rs` | Optional full producer trace integration; preserve existing untraced behavior. | Implemented |
| `src/pattern/combinators/structure.rs` | Optional full producer trace integration; preserve existing untraced behavior. | Implemented |
| `src/pattern/combinators/time.rs` | Optional full producer trace integration; preserve existing untraced behavior. | Implemented |
| `tests/song_trace_combinators.rs` | Behavioral trace fixtures and legacy compatibility. | Implemented |

```rust
pub fn query_traced(p: &Pat, span: TimeSpan, cx: &mut QueryCtx<'_>) -> QueryResult;
```

## Tasks

### TASK-001: Integrate trace contract
**Status**: Completed
**Parallelizable**: No; preceding trace contracts must be verified.
**Deliverables**: Listed source files and immutable pre-edit intent records.
- [x] Read exact current sources; add the smallest explicit tracing integration.

### TASK-002: Prove propagation and compatibility
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No within this phase.
**Deliverables**: Listed test file and complete focused logs.
- [x] Add nonzero behavioral fixtures and run mandatory Rust checker.

### TASK-003: Reconcile evidence
**Status**: Completed
**Depends On**: TASK-002
**Deliverables**: This plan's status, checked criteria and post-edit hashes.
- [x] Record exact commands/exits and leave no active test processes.

## Phase acceptance criteria

- [x] All structural, dynamic-transform, sound-selection and sample-region branches propagate deterministic explicit producer ordinals before filtering/clipping.
- [x] When a control gives structure, preserve both timing-source and content-source trace components unambiguously.
- [x] Trace survives fast/slow/rev/cat/stack/ply/chop/slice/splice/hold/grid/chord/arp and dynamic expansion, with distinct identical sibling events.
- [x] For stable source realizations, traces retain identity across fractional, reordered and split windows; existing OccKey equality/dedup and query failures stay unchanged. Full dynamic-value/control partition invariance is verified by SONG-04 canonical whole-cycle realization, rather than asserted for legacy clipped-start evaluation.

## Verification commands

| Command | Evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise exec -- cargo check` | Joined native code compiles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo clippy --all-targets -- -D warnings` | No new warnings; preserve unrelated work. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable trace core compiles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo test --test song_trace_combinators` | Nonzero phase tests pass. |

## Completion criteria

- [x] Every phase criterion has current behavioral evidence.
- [x] Required checker ran after Rust writes; compilation and compatibility pass.
- [x] Logs, immutable intent and final hashes recorded; source under 1000 lines.

## Progress log

### Session: 2026-10-01 — correctness amendment
Source inspection exposed the hash-only identity gap. Split tracing into bounded
serial prerequisites rather than assigning all pattern modules to SONG-04.
Implementation not started; no test result claimed.

### Session: 2026-10-01 — implementation and author checks
A independently cleared (`/tmp/vactr-song02-04a-checker-results.json`). Seven owned
combinators now preserve explicit static child, dynamic transform, and generated
branch traces. Timing/content merges frame both complete paths by typed role and
two-word length. Traced striate matches full traces and retains widening faults;
untraced compact matching and fault dropping stay unchanged. Nine integration
fixtures passed in `/tmp/vactr-song04b-second.log`; author verification continues.
Initial twin-fixture ordering failure is retained in `/tmp/vactr-song04b-first.log`.

### Session: 2026-10-01 — stack compatibility regression isolated
Author native, wasm, clippy, 9 B fixtures, 1 collision unit, and 9 A fixtures
passed in `tmp/song-mode-riela/SONG-04B/logs/*-001.log`. The legacy pattern
run exited 101 on native stack overflow; its complete failed log is retained.
Inline direct-q legacy fast path restored the unchanged deep-chain regression
(`/tmp/vactr-song04b-deep-002.log`, exit 0, 1 fixture). New traced 200/300
reverse-chain fixture still aborts (`/tmp/vactr-song04b-traced-deep-001.log`
and `-002.log`, exit 101). Required core A amendment is coordinated with root;
no depth limit lowering or test stack raising. Callable nested sound pattern
trace fixture passed (`/tmp/vactr-song04b-sound-001.log`, exit 0, 1 fixture).
All author foreground processes reached terminal exit; readiness remains held.

### Session: 2026-10-01 — final author verification and sealed writes
All nine final author gates exited 0; all foreground processes are closed.
12 B integration fixtures, 1 striate full-trace collision unit, 9 A core fixtures,
1 finite-handle unit, and 64 legacy pattern fixtures passed separately. Adversarial
reverse and alternating fast/slow chains succeed at200 and report DepthExceeded
at300 in both modes. Tests use unchanged default stacks and evaluator limits.
Traced source scopes use direct push/query/pop with a non-cloning mode accessor;
reverse event mapping executes after recursive query returns, reducing live-frame
storage. Full trace matching fixes striate compact collisions without using rank
as identity. Traced widening faults are retained; legacy fault dropping is preserved.
All touched Rust files are below1000 lines (maximum569). Final source hashes,
command/log hashes and exact counts: `TASK-002-post-001.json`. Initial failures
remain in prior numbered logs. Independent joined A/B/03 verification is pending.

| Actual command | Exit | Complete log | Evidence |
|---|---:|---|---|
| `CARGO_TERM_QUIET=true mise exec -- cargo check` | 0 | `tmp/song-mode-riela/SONG-04B/logs/native-check-002.log` | gate passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | 0 | `tmp/song-mode-riela/SONG-04B/logs/wasm-check-002.log` | gate passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo clippy --all-targets -- -D warnings` | 0 | `tmp/song-mode-riela/SONG-04B/logs/clippy-002.log` | gate passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo test --test song_trace_combinators` | 0 | `tmp/song-mode-riela/SONG-04B/logs/trace-tests-002.log` | 12 passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo test striate_full_trace_equality --lib` | 0 | `tmp/song-mode-riela/SONG-04B/logs/collision-unit-002.log` | 1 passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo test --test song_trace_core` | 0 | `tmp/song-mode-riela/SONG-04B/logs/core-tests-002.log` | 9 passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo test finite_song_values_event_handles --lib` | 0 | `tmp/song-mode-riela/SONG-04B/logs/finite-handle-unit-002.log` | 1 passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo test pattern::tests --lib` | 0 | `tmp/song-mode-riela/SONG-04B/logs/legacy-pattern-tests-002.log` | 64 passed |
| `mise exec -- rustfmt --edition 2021 --check src/pattern/eval.rs src/pattern/query.rs src/pattern/combinators/control.rs src/pattern/combinators/music.rs src/pattern/combinators/random.rs src/pattern/combinators/region.rs src/pattern/combinators/sound.rs src/pattern/combinators/structure.rs src/pattern/combinators/time.rs tests/song_trace_combinators.rs` | 0 | `tmp/song-mode-riela/SONG-04B/logs/fmt-002.log` | gate passed |

### Session: 2026-10-01 — joined independent verification complete
Joined SONG-03/B/A checker passed every gate with242 distinct fixtures;
`/tmp/vactr-song03-04b-checker-results.json` records native, host-wasm,
all-target clippy, scoped formatting, cargo/nextest and legacy evidence.
A/B author counts remain12 combinator fixtures,1 separate striate collision,
9 core fixtures,1 separate finite-handle unit and64 legacy pattern fixtures.
Sealed Rust hashes remain unchanged from final author evidence. All foreground
sessions are terminal. Archive/index reconciliation remains deferred to SONG-16.
