# Producer identity tracing core implementation plan

**Status**: Completed
**Plan ID**: SONG-04A
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
  "planId": "SONG-04A",
  "planPath": "impl-plans/active/song-mode-producer-trace.md",
  "dependsOn": [
    "SONG-01",
    "SONG-02"
  ],
  "writePaths": [
    "src/pattern/occ.rs",
    "src/pattern/eval.rs",
    "src/pattern/query.rs",
    "src/pattern/step.rs",
    "tests/song_trace_core.rs",
    "impl-plans/active/song-mode-producer-trace.md"
  ],
  "sharedPaths": [
    "src/pattern/occ.rs",
    "src/pattern/eval.rs",
    "src/pattern/query.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/pattern/occ.rs",
      "intendedEdit": "Serial producer tracing; later SONG-04 consumes the completed trace without replacing legacy keys."
    },
    {
      "path": "src/pattern/eval.rs",
      "intendedEdit": "Serial producer tracing; later SONG-04 consumes the completed trace without replacing legacy keys."
    },
    {
      "path": "src/pattern/query.rs",
      "intendedEdit": "Serial producer tracing; later SONG-04 consumes the completed trace without replacing legacy keys."
    }
  ]
}
```

## Related plans and dependencies

- **Depends On**: SONG-01, SONG-02
- **Next**: SONG-04B

| Dependency | Required contract | Status |
|---|---|---|
| SONG-01 | Verified preceding phase and immutable current source | PASSED |

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
| `src/pattern/occ.rs` | Optional full producer trace integration; preserve existing untraced behavior. | IMPLEMENTED |
| `src/pattern/eval.rs` | Optional full producer trace integration; preserve existing untraced behavior. | IMPLEMENTED |
| `src/pattern/query.rs` | Optional full producer trace integration; preserve existing untraced behavior. | IMPLEMENTED |
| `src/pattern/step.rs` | Optional full producer trace integration; preserve existing untraced behavior. | IMPLEMENTED |
| `tests/song_trace_core.rs` | Behavioral trace fixtures and legacy compatibility. | IMPLEMENTED |

```rust
pub enum ProducerKind { Child, NestedStep, GeneratedBranch, DynamicExpansion, TimingSource, ContentSource }
pub struct ProducerStep { pub kind: ProducerKind, pub ordinal: u32 }
pub struct ProducerTrace { pub steps: Vec<ProducerStep> }
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

- [x] Typed full producer path is independent of hashed OccKey; legacy scheduler identity and untraced query outputs remain compatible.
- [x] Optional tracing records explicit source tree edges and nested step ordinals before clipping; no returned-vector indices, pointer identities or execution counters.
- [x] Traced canonical entry preserves existing query depth/reentry/work bounds and returns faults without dropping evidence.
- [x] Tests cover nested list leaves, duplicated producer hashes and tracing-disabled compatibility. Simultaneous stack twins are verified in SONG-04B, which owns stack branch propagation.

- [x] Dynamically resolved Part/Song/EventHandle values produce Type faults at pattern value/parameter boundaries; lazy construction and ordinary scalar behavior remain unchanged.
- [x] Embedded Hold/Repeat wrapper child edges remain explicit in complete producer traces.

## Verification commands

| Command | Evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise exec -- cargo check` | Joined native code compiles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo clippy --all-targets -- -D warnings` | No new warnings; preserve unrelated work. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable trace core compiles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo test --test song_trace_core` | Nonzero phase tests pass. |

## Completion criteria

- [x] Every phase criterion has current behavioral evidence.
- [x] Required checker ran after Rust writes; compilation and compatibility pass.
- [x] Logs, immutable intent and final hashes recorded; source under 1000 lines.

## Progress log

### Session: 2026-10-01 — correctness amendment
Source inspection exposed the hash-only identity gap. Split tracing into bounded
serial prerequisites rather than assigning all pattern modules to SONG-04.
Implementation not started; no test result claimed.

### Session: 2026-10-01 — tracing core implementation
**Tasks Completed**: TASK-001; owned Rust implementation and seven core fixtures.
**Tasks In Progress**: TASK-002/TASK-003 await independent combined SONG-02/SONG-04A checker.
**Notes**: Event producer traces retain full typed edges independently of OccKey. Nested and replicated list paths use source ordinals before clipping; dynamic scopes restore after faults. Nested Value::List traversal previously bypassed q() depth accounting; subdivision now uses the shared guard, including a programmatic 300-level adversarial fixture for both modes. Simultaneous combinator branches remain SONG-04B. Canonical dynamic-value realization remains SONG-04.
**Intent / Hash Evidence**: `tmp/song-mode-riela/SONG-04A/TASK-001-001.json` through `TASK-001-003.json`, `TASK-002-001.json` / `TASK-002-002.json`, and `TASK-002-post-001.json`. All five Rust files are below 1000 lines; no foreground sessions remain.

| Actual command | Exit | Complete foreground log | Evidence |
|---|---:|---|---|
| `CARGO_TERM_QUIET=true mise exec -- cargo check` | 0 | `tmp/song-mode-riela/SONG-04A/logs/native-check-001.log` | Native compilation |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | 0 | `tmp/song-mode-riela/SONG-04A/logs/wasm-check-001.log` | Portable compilation |
| `CARGO_TERM_QUIET=true mise exec -- cargo clippy --all-targets -- -D warnings` | 0 | `tmp/song-mode-riela/SONG-04A/logs/clippy-001.log` | All-target warnings gate |
| `CARGO_TERM_QUIET=true mise exec -- cargo test --test song_trace_core` | 0 | `tmp/song-mode-riela/SONG-04A/logs/trace-tests-001.log` | 7 passed |
| `CARGO_TERM_QUIET=true mise exec -- cargo test pattern::tests --lib` | 0 | `tmp/song-mode-riela/SONG-04A/logs/legacy-pattern-tests-001.log` | 64 passed |
| `CARGO_TERM_QUIET=true mise exec -- rustfmt --edition 2021 --check src/pattern/occ.rs src/pattern/eval.rs src/pattern/query.rs src/pattern/step.rs tests/song_trace_core.rs` | 0 | `tmp/song-mode-riela/SONG-04A/logs/fmt-001.log` | Scoped format verification |

### Session: 2026-10-01 — joined review correction
Independent review found dynamic finite values could bypass static constructor
guards. This phase already owns eval.rs and step.rs; add rejection after dynamic
resolution, with faults retained and lazy construction unchanged. Its dependency
on SONG-02 makes the finite variants explicit. Preserve every embedded wrapper
child edge in source traces. Independent checks must be rerun after this batch.

### Session: 2026-10-01 — dynamic admission correction verified
**Rust writes sealed**: eval.rs reuses the SONG-02 bounded finite-value guard after dynamic resolution and at parameter admission; step.rs applies it to atomic values and retains explicit embedded Hold/Repeat wrapper child edges. No lazy references/functions/thunks are forced during construction. New tests preserve unrelated sibling events after a finite-valued subtree fails.
**Original failure retained**: `/tmp/vactr-song02-04a-final-focused.log` records the transient callable fixture prototype indexing failure; the fixture now compiles expanded forms and accepts a top-level block prototype.
**Author verification**: Every command in the preceding table was rerun with logs ending `-002.log`, all exit 0. `song_trace_core` now runs 9 passing fixtures; legacy `pattern::tests` runs 64 passing fixtures. Additional `CARGO_TERM_QUIET=true mise exec -- cargo test finite_song_values_event_handles --lib` runs 1 passing opaque-handle fixture, full log `tmp/song-mode-riela/SONG-04A/logs/finite-handle-test-002.log`.
**Hash evidence**: `TASK-002-003.json` through `TASK-002-005.json` document fresh correction intent; `TASK-002-post-002.json` records final sealed Rust hashes and exits. All author foreground processes reached terminal exit. Independent checker completion remains pending. No SONG-04B source has changed.

### Session: 2026-10-01 — independent joined verification complete
SONG-02/SONG-04A independent checker passed every gate with 147 distinct joined fixtures;
evidence: `/tmp/vactr-song02-04a-checker-results.json`. Author phase evidence remains
9 trace integration tests, 64 legacy pattern tests, and 1 separate runtime-handle unit.
Core Rust remains sealed. Archive/index reconciliation is deferred to SONG-16.

### Session: 2026-10-01 — traced stack compatibility repair
B adversarial 200/300-chain testing exposed trace-helper debug stack overhead.
Reopen this phase narrowly for inline with_producer/q_child and a non-cloning
QState::is_traced accessor in the already owned eval.rs/query.rs. Preserve
MAX_DEPTH and all query work/reentry counters; do not raise test thread stacks.
B consumes these helpers in its owned files and owns the traced deep-chain
fixture. Mandatory joined checker must rerun A core, deep-chain and legacy
regressions before this amendment is considered complete.

### Session: 2026-10-01 — inline helper repair evidence
Root authorized inline `with_producer`/`q_child` and non-cloning `is_traced`;
intent `TASK-002-006.json`. B now uses the mode accessor. Traced adversarial
reverse chains still expose callback-frame overhead in debug mode, retained
`/tmp/vactr-song04b-traced-deep-002.log` (exit 101). A completion is reopened
for the narrow repair; no depth/work/reentry limits or test stacks changed.

### Session: 2026-10-01 — direct scoped traversal amendment
Root authorized direct inline producer push/pop and q_child query traversal in
the existing eval.rs/query.rs ownership. Inline attributes alone did not remove
the debug FnOnce callback frames. The child query records failures as values,
so direct enter/query/leave restores the producer path on success and recorded
failure. Keep all existing depth/reentry/work counters and unchanged test stacks.
Record: `TASK-002-007.json`; rerun traced successful200/rejected300 and legacy.

### Session: 2026-10-01 — narrow stack repair author verification complete
Direct trace push/query/pop, inline scope helpers, and non-cloning mode checks
are implemented. B owned reverse mapping extraction removes event-loop temporaries
from recursive query frames. Reverse and alternating fast/slow depth200 succeed,
depth300 returns DepthExceeded in traced and untraced modes, without changed limits
or raised test stacks. Joined author gates all exit0 in
`tmp/song-mode-riela/SONG-04B/logs/*-002.log`: native/wasm/clippy/fmt,
12 B fixtures, 1 collision unit, 9 A core fixtures, 1 finite-handle unit,
64 legacy pattern fixtures. All foreground processes are terminal; no further
writes planned. Mandatory independent joined A/B/03 checker remains pending.

### Session: 2026-10-01 — joined independent verification complete
Joined SONG-03/B/A checker passed every gate with242 distinct fixtures;
`/tmp/vactr-song03-04b-checker-results.json` records native, host-wasm,
all-target clippy, scoped formatting, cargo/nextest and legacy evidence.
A/B author counts remain12 combinator fixtures,1 separate striate collision,
9 core fixtures,1 separate finite-handle unit and64 legacy pattern fixtures.
Sealed Rust hashes remain unchanged from final author evidence. All foreground
sessions are terminal. Archive/index reconciliation remains deferred to SONG-16.
