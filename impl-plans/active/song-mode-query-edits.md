# Canonical realization and pure edits implementation plan

**Status**: Completed
**Plan ID**: SONG-04
**Plan Path**: impl-plans/active/song-mode-query-edits.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-01
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
    "SONG-01",
    "SONG-04B",
    "SONG-04P",
    "SONG-04Q",
    "SONG-04R"
  ],
  "writePaths": [
    "src/song/query.rs",
    "src/song/edit.rs",
    "src/song/mod.rs",
    "tests/song_query.rs",
    "tests/song_edits.rs",
    "impl-plans/active/song-mode-query-edits.md",
    "src/song/source.rs",
    "src/pattern/query.rs",
    "tests/song_source_contracts.rs"
  ],
  "sharedPaths": [
    "src/song/mod.rs",
    "impl-plans/active/song-mode-query-edits.md",
    "src/pattern/query.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Export query/edit APIs after contracts exist. Owners execute serially: SONG-01, SONG-04, SONG-06, SONG-08, SONG-12"
    },
    {
      "path": "impl-plans/active/song-mode-query-edits.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    },
    {
      "path": "src/pattern/query.rs",
      "intendedEdit": "After SONG-04A/B tracing is verified, add only the SongSource dispatch, retaining QState budgets."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-01](song-mode-values.md), [SONG-04B](song-mode-trace-combinators.md), [SONG-04P](song-mode-source-contracts.md), [SONG-04Q](song-mode-source-provenance.md)
- **Next**: [SONG-05](song-mode-natives.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-01 | Reviewed declarations, passing phase checks and recorded post-edit hashes | Author verified |
| SONG-04B | Complete typed producer traces across combinators, verified after SONG-04A | Author verified |

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
| `src/song/query.rs` | Canonical whole-cycle source realization with frozen QueryVm, bounded work/cache and caller clipping; local/global placement mapping and stable seeds. | Author verified |
| `src/song/edit.rs` | Implement immutable edit builders and execution; selectors operate on source instrument identity before transformation. | Author verified |
| `src/song/source.rs` | Immutable selected source stream for existing pattern transforms; shared recursion/work context. | Author verified |
| Prerequisite SONG-04P | Source node/hash, live-input exhaustive match and typed route schema; no writes here. | Ready prerequisite |
| `src/pattern/query.rs` | SongSource dispatch using the existing traced QState rather than resetting query bounds. | Author verified |
| `src/song/mod.rs` | Export query/edit APIs after contracts exist. | Author verified |
| `tests/song_query.rs` | Compare normalized union for many query partitions, fractional part edges, seed modes, equal simultaneous twins and callback-size independence. | Author verified |
| `tests/song_edits.rs` | Verify no input mutation, track replace, bd selection, one-tone deletion, stale handles and onset-based regional overwrite. | Author verified |

### Public declaration contract

```rust
pub struct SongQueryCtx<'a> { pub vm: &'a mut dyn QueryVm, pub cells: &'a InputCells, pub seed: u64, pub tempo: Tempo, pub limits: &'a SongLimits }
pub struct SongBuildCtx<'a> { pub vm: &'a mut dyn QueryVm, pub limits: &'a SongLimits }
pub fn query_part(part: &Part, span: TimeSpan, cx: &mut SongQueryCtx<'_>) -> Result<Vec<SongEvent>, Failure>;
pub fn replace_track(part: &Part, track: KwId, replacement: Rc<Pat>) -> Result<Part, Failure>;
pub fn transform_instrument(part: &Part, track: KwId, selector: InstrumentSelector, transform: Value, cx: &mut SongBuildCtx<'_>) -> Result<Part, Failure>;
pub fn instrument_fx(part: &Part, track: KwId, selector: InstrumentSelector, template: KwId) -> Result<Part, Failure>;
pub fn delete_event(part: &Part, handle: &EventHandle) -> Result<Part, Failure>;
pub fn overwrite_region(part: &Part, track: KwId, region: TimeSpan, replacement: Rc<Pat>) -> Result<Part, Failure>;
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

- [x] Instrument effect edits retain the selected family and declared template as immutable route metadata; replacement and later edits preserve unaffected routes.
- [x] IDs assigned before query clipping/filtering; whole spans unchanged and continuations never retrigger.
- [x] Chord expanded once before edits; identical pitches have distinct tone IDs and mono commit marker.
- [x] Overwrite removes complete events with onset in [begin,end); retains earlier sustaining notes and onsets at end; inserts replacement at begin.
- [x] Same repeats preserve local draws; Vary derives from full placement path; caller window/order/cache eviction never changes draws.
- [x] Existing query faults or budget failures fail song query, rather than returning successful partial music. Current query bounds 256/64/1,000,000 remain enforced.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_query) | binary(song_edits)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

## Correctness amendment: 2026-10-01

SONG-04A/B establish full producer trace before this phase. The immutable internal
SongSource pattern node exposes only the selected frozen source stream to existing
transforms. The public builder takes a pure SongBuildCtx and invokes the callback
once, storing an explicitly prepared transformed pattern. Never invoke it on query
or cache miss. Opaque handles are issued only after successful canonical realization;
track, placement and full producer/tone identity remain distinct. Fractional numeric
notes are retained exactly as supported by existing commit semantics. All source
queries retain the existing shared recursion/reentry/work budgets.

Additional fixtures: hash collisions, same-pitch/fractional chord tones, family bank
selection, callback call count, cache disabled/evicted, immutable edit chains and
canonical nested faults. Revision checks reject stale handles before editing.

### Session: 2026-10-01 — nested repeat seed composition
Keep event placement and seed placement separate. Same normalizes only its own
repeat iteration ordinal for seed derivation, so the complete child repeats
identically, including nested Vary decisions. Vary retains its ordinal and all
other seed-path edges. Event identity always retains every original placement
ordinal. Add Same(Vary(child)), Vary(Same(child)), and nested sequence fixtures;
compare corresponding decisions while requiring distinct full event handles.

### Session: 2026-10-01 — source/context integration amendment
SONG-04P establishes the SongSource node and typed optional InstrumentRoute first.
This phase consumes that schema and replaces the temporary explicit unavailable
dispatch. Query only touched canonical whole cycles and overlapping placements;
use checked repeat index math rather than scanning/materializing the full finite
arrangement. Canonical-window admission is independent of caller clipping. Keep
shared QState limits inside SongSource and optional bounded cache correctness.
SongQueryCtx requires a caller-certified fixed/frozen view; actual transitive VM
snapshot freezing remains required in SONG-06/07, not supplied by cloning VmQuery.
SongBuildCtx may be a separate effect-restricted synchronous construction borrow;
reject emitted output and restore prior output, call once, and publish no edit on
failure. SONG-05 must not advertise that live construction borrow as a transport
snapshot. Full song completion requires actual snapshot freeze verification.

### Session: 2026-10-01 — provenance prerequisite
SONG-04 additionally waits for independently verified SONG-04Q typed Event origin,
song-only intrinsic whole-onset dynamic sampling and inherited QState SongLimits.
Selected source transformations must retain original family/route/tone identity.
Build context is a separate synchronous effect-restricted borrow, not a persistent
frozen query view. Preserve route declarations through replace/overwrite chains;
prove intended family routing on newly inserted matching notes and unaffected notes.

### Session: 2026-10-01 — scoped effect-policy preservation
InstrumentFx declarations remain immutable scoped policy in the Part graph rather
than annotations on already realized notes alone. New replacement/overwrite notes
resolve the applicable route using their original instrument and whole onset;
later matching declarations override earlier matching policy in the applicable
track/placement scope. Selected transforms retain typed source origin and route.
Bounded policy lookup traverses only relevant finite placements and shares current
work/depth limits; it performs no full repetition scan or audio realization.
Verify FX then replace/overwrite, unmatched families/tracks, chained overrides and
continuations across a source-placement boundary.

### Session: 2026-10-01 — SONG-04 authorized implementation
**Tasks In Progress**: TASK-001/002, after independently verified SONG-04Q.
**Intent**: `tmp/song-mode-riela/SONG-04/TASK-001-intent-001.json`. Lazy canonical cycles and touched symbolic placements, no whole-Part materialization; nested sources share counters and policy. No cache; immutable edits retain source seeds and scoped FX policies.
**Verification**: Pending author gates and independent checker; no completion claimed.

### Session: 2026-10-01 — exact n classification dependency
SONG-04 final seal additionally waits independently verified SONG-04R QueryVm
audio-resource classification. Explicit note takes precedence; n expands notes
only for resolved audio routes without a sample resource, matching commit semantics.
Author may continue disjoint explicit-note/edit fixtures while this prerequisite
lands. No Sound enum-tag approximation is accepted.

### Session: 2026-10-01 — source contract migration and track admission
SONG-04 additionally owns tests/song_source_contracts.rs as its eighth Rust module.
The temporary unavailable dispatch is retired: plain legacy query must now fail
explicitly for missing traced song context/limits, while actual canonical song
context succeeds. Preserve every other prerequisite fixture.
ReplaceTrack skips its obsolete source track BEFORE realization, preserving
unrelated track faults. Selected SongSource realizes only its selected root track,
so discarded/unrelated parent tracks cannot reappear through a transformed stream.
Keep shared counters and caller limits; add transformed-drums then replaced-faulty-
bass regression and paired unrelated-fault atomic failure.

### Session: 2026-10-01 — sealed author completion, independent review pending
**Tasks Completed**: TASK-001/002 implementation and author evidence for TASK-003.
**Sources**: Exact eight Rust paths sealed in `tmp/song-mode-riela/SONG-04/TASK-003-post-001.json`; `src/pattern/query.rs` consumed with its verified dispatch unchanged. All remain below 1000 lines (maximum: edits tests 923).
**Behavior**: Lazy touched canonical cycles and checked finite placement ranges; caller clipping follows full identity/admission. Full producer identities, exact numeric mono tones, Same/Vary seed scopes, offset-normalized certified deletion, typed source origins, callback-once/output rollback, onset overwrite with cross-region releases, authoritative n resource classification and explicit freq precedence. Replaced tracks and selected-source siblings are skipped before querying. Inherited routes survive timing transforms via origin-revision cutoff; later applicable FX overrides remain supported. No cache and no transport-snapshot claim.
**Bounded identities**: Noncloning producer length is charged before copying; checked input/output identity words share current QState budget; direct role/length framing avoids temporary identity arrays and retains full revision/track/placement/typed occurrence/cycle/onset/tone. Nested16 selected transforms explicitly fail FuelExhausted before oversized encoding.
**Author gates**: `TASK-003-checks-003.json` records every final command, exit0 and complete log. Native cargo check, host-wasm cargo check, all-target Clippy, scoped rustfmt and scoped diff pass. Focused74 (query16, edits16, source contracts8, provenance4, trace core9, trace combinators12, route classification9); separate legacy64, VM Query-mode7, runtime-handle1, song-context6 and producer-length1: **153 distinct selected fixtures**. Nextest repeats32 own query/edit fixtures successfully, not additional distinct fixtures.
**Failure preservation**: Earlier failed compile/count/invalid-return/interim-contract logs remain. The zero-selected old runtime-handle filter is not evidence; final corrected filter executes1 test.
**Processes**: Final gate process89349 polled terminal0; all author foreground sessions closed.
**Pending**: Independent check-and-test-after-modify review before phase completion/SONG-05; no further Rust writes absent an explicit checker finding. Archive/index reconciliation deferred SONG-16.
- [x] Independent checker approves the sealed SONG-04 batch.

### Session: 2026-10-01 — independent phase clearance
**Tasks Completed**: TASK-001/002/003 and all completion/phase criteria. SONG-04 is **Completed**.
**Independent evidence**: `/tmp/vactr-song04-independent-001/final-results.json`, complete retained logs in the same directory. All12 checker gates exit0; native/host-wasm/fmt/all-target Clippy/scoped diff pass. **153 distinct selected fixtures**, plus32 repeated nextest executions (not extra distinct fixtures). Checker process26455 polled terminal0, no active handles or remaining source-review findings.
**Seals**: All8 Rust hashes match before/after independent checks and were rechecked for this documentation-only completion. Maximum923lines. Author seal remains `tmp/song-mode-riela/SONG-04/TASK-003-post-001.json`; completion intent/post records under TASK-004. No Rust changed after author seal.
**Scope**: Canonical Part query and immutable selective edits are complete; this is phase-only clearance. Actual transitive snapshot freeze, playback and export remain later required phases. SONG-05 may proceed under root authorization. Archive/index reconciliation remains deferred to SONG-16.
