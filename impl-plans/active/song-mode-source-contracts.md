# Selected song source and route contracts implementation plan

**Status**: Completed
**Plan ID**: SONG-04P
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Canonical queries and routing](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance)

## Intent and source evidence

The canonical query phase needs a SongSource node, including the exhaustive live
input classifier, and typed selected-instrument route metadata. These seven Rust
files are a bounded prerequisite; the eight-file SONG-04 phase then owns actual
canonical realization and edits. No reserved user controls encode routing.

## Manifest

```json
{
  "planId": "SONG-04P",
  "planPath": "impl-plans/active/song-mode-source-contracts.md",
  "dependsOn": [
    "SONG-03",
    "SONG-04B"
  ],
  "writePaths": [
    "src/song/source.rs",
    "src/song/identity.rs",
    "src/song/mod.rs",
    "src/pattern/pat.rs",
    "src/pattern/query.rs",
    "src/pattern/combinators/input.rs",
    "tests/song_source_contracts.rs",
    "impl-plans/active/song-mode-source-contracts.md"
  ],
  "sharedPaths": [
    "src/song/source.rs",
    "src/song/mod.rs",
    "src/pattern/query.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/source.rs",
      "intendedEdit": "Serial SONG-04P source/schema declaration, then SONG-04 real realization/exports. Preserve fresh source and immutable intent."
    },
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Serial SONG-04P source/schema declaration, then SONG-04 real realization/exports. Preserve fresh source and immutable intent."
    },
    {
      "path": "src/pattern/query.rs",
      "intendedEdit": "Serial SONG-04P source/schema declaration, then SONG-04 real realization/exports. Preserve fresh source and immutable intent."
    }
  ]
}
```

## Related plans and dependencies

- **Depends On**: SONG-03, SONG-04B (independently verified)
- **Next**: SONG-04

| Dependency | Required output | Status |
|---|---|---|
| SONG-03 / SONG-04B | Finite types, complete producer traces and current checked sources | COMPLETE |

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
| `src/song/source.rs` | Immutable selected source Part/track/family descriptor; no callback retained. | IMPLEMENTED |
| `src/song/identity.rs` | Typed InstrumentRoute and optional SongEvent route metadata. | IMPLEMENTED |
| `src/song/mod.rs` | Export source and route contracts. | IMPLEMENTED |
| `src/pattern/pat.rs` | SongSource node and stable structural seed hashing, excluding revisions/pointers. | IMPLEMENTED |
| `src/pattern/query.rs` | Exhaustive SongSource dispatch, explicitly unavailable until SONG-04 replaces it. | IMPLEMENTED |
| `src/pattern/combinators/input.rs` | Classify the immutable selected source node as no live input lane. | IMPLEMENTED |
| `tests/song_source_contracts.rs` | Descriptor validation, route identity, live-lane classification and honest unavailable-query behavior. | IMPLEMENTED |

```rust
pub struct SongSource { part: Rc<Part>, track: KwId, selector: InstrumentSelector }
pub struct InstrumentRoute { pub family: InstrumentSelector, pub template: KwId }
pub enum PatNode { SongSource(Rc<SongSource>) }
pub struct SongEvent { pub route: Option<InstrumentRoute> }
impl SongSource {
    pub fn new(part: Rc<Part>, track: KwId, selector: InstrumentSelector) -> Result<Self, Failure>;
    pub fn part(&self) -> &Rc<Part>;
    pub fn track(&self) -> KwId;
    pub fn selector(&self) -> &InstrumentSelector;
}
```

Declaration blocks list additions only. Preserve every existing enum/struct field.
Unavailable dispatch is a temporary explicit diagnostic, never successful silence;
SONG-04 must replace it and prove shared-QState canonical realization before usable
song APIs can be accepted. This prerequisite does not certify a mutable VM as frozen.

## Tasks

### TASK-001: Source and route schema
**Status**: Completed
**Parallelizable**: No
**Deliverables**: All exact contracts and immutable pre/post intents.
- [x] Fresh source/schema integration and exhaustive input classifier.

### TASK-002: Behavioral evidence
**Status**: Completed
**Depends On**: TASK-001
**Deliverables**: Nonzero contract fixtures, native/wasm/format/Clippy logs and independent checker.
- [x] Prove immutable selectors, honest interim dispatch and legacy compatibility.

### TASK-003: Reconcile
**Status**: Completed
**Depends On**: TASK-002
**Deliverables**: Own plan status, terminal logs and final hashes; archive deferred.
- [x] Record actual exits and seal exact owned sources.

## Phase acceptance criteria

- [x] SongSource retains an immutable Part, existing track and validated family; no callback or mutable host state.
- [x] SongEvent route data is explicit and typed; route family/template does not use user controls or alter unaffected data.
- [x] Structural seed hash excludes edit revision and allocation identity; full producer identity remains collision-independent.
- [x] Exhaustive live-input classification compiles; Source dispatch produces an explicit fault until real realization lands in SONG-04.
- [x] Existing type/pattern behavior and depth guards remain compatible.

## Verification commands

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise exec -- cargo test --test song_source_contracts` | Nonzero source/route fixtures pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check` | Native integration compiles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable integration compiles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo clippy --all-targets -- -D warnings` | Strict warnings gate passes. |

## Completion criteria

- [x] Every phase criterion has current behavioral evidence and independent checker approval.
- [x] All source files stay below1000lines; exact current hashes and full terminal logs retained.
- [x] Interim unsupported realization remains an explicit later SONG-04 obligation, not a full song-mode success claim.

## Progress log

### Session: 2026-10-01 — source-grounded prerequisite
No Rust written or test result claimed. The input-lane exhaustive match and route
schema are separated to preserve bounded ownership before canonical realization.

### Session: 2026-10-01 — author implementation seal
Implemented the seven exact Rust targets, including checked immutable descriptors,
typed route metadata, stable seed fingerprints, exhaustive no-live-lane classification
and explicit unavailable realization. Buffer seed fingerprints intentionally omit
allocation identity and PCM state; complete selector equality remains authoritative.
Typed ordinary Event provenance is deferred to the separate SONG-04Q prerequisite;
actual canonical realization remains SONG-04 work. No callback or hidden controls added.

Author checks terminated successfully: retained session 79207 exit 0, ledger
`tmp/song-mode-riela/SONG-04P/author-check-results.json`. Every Cargo command used
`CARGO_TERM_QUIET=true mise exec -- cargo`. Native check, host-wasm check and
all-target Clippy with `-D warnings` each exit 0; their full logs are
`native-final.log`, `wasm-final.log`, `clippy-final.log` in that directory.
`cargo test --lib pattern::tests` passed 64,
`cargo test --lib song::identity::tests` passed 3,
`cargo test --test song_values` passed 15, and
`cargo nextest run -E 'binary(song_source_contracts)'` passed 7.
Full logs are `pattern-final.log`, `identity-final.log`, `values-final.log`,
`nextest-final.log`. Initial `cargo test --test song_source_contracts` also
passed 7 with terminal session 27515 exit 0 (`tests-first.log`).
Scoped rustfmt completed for all seven targets; `git diff --check` passed.
All seven Rust files remain below 1000 lines (largest 342). Immutable source
intent/post hashes are `0001-source-route-schema{,-after}.json`.
Independent mandatory checker approval is pending; archive/index changes deferred.

### Session: 2026-10-01 — independent verification
Mandatory checker sealed `/tmp/vactr-song04p-checker-results.json`: 98 distinct
tests and nextest 7 passed; native, host-wasm, scoped format, strict Clippy and
diff gates all exited 0. No foreground processes remained. All phase criteria
are verified; interim realization remains explicitly unsupported until SONG-04.
Archive/index reconciliation is deferred to root.
