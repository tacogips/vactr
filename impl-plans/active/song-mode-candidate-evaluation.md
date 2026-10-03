# Isolated whole-code candidate evaluation implementation plan

**Status**: Completed
**Plan ID**: SONG-07
**Plan Path**: impl-plans/active/song-mode-candidate-evaluation.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-01
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Isolated whole-code candidate evaluation supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-07",
  "planPath": "impl-plans/active/song-mode-candidate-evaluation.md",
  "dependsOn": [
    "SONG-06"
  ],
  "writePaths": [
    "src/session/song.rs",
    "src/session/song/freeze.rs",
    "src/song/part.rs",
    "src/session/mod.rs",
    "src/session/session.rs",
    "src/ns/evaluator.rs",
    "src/song/snapshot.rs",
    "tests/song_candidate.rs",
    "impl-plans/active/song-mode-candidate-evaluation.md"
  ],
  "sharedPaths": [
    "src/session/song.rs",
    "src/session/session.rs",
    "src/song/snapshot.rs",
    "impl-plans/active/song-mode-candidate-evaluation.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/session/song.rs",
      "intendedEdit": "Evaluate entire document in fresh candidate evaluator/registry with staging-only hosts and immutable loaded assets; reject disallowed effects. Owners execute serially: SONG-07, SONG-11"
    },
    {
      "path": "src/session/session.rs",
      "intendedEdit": "Route new requests to candidate preparation, preserving existing incremental eval behavior. Owners execute serially: SONG-06, SONG-07"
    },
    {
      "path": "src/song/snapshot.rs",
      "intendedEdit": "Implement frozen namespace/kit/sample ownership and pending candidate cancellation. Owners execute serially: SONG-06, SONG-07"
    },
    {
      "path": "impl-plans/active/song-mode-candidate-evaluation.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-06](song-mode-snapshot-contracts.md)
- **Next**: [SONG-11](song-mode-transport.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-06 | Independently verified contracts: /tmp/vactr-song06-independent-001/final-results.json, 11 gates exit 0 | Completed |

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
| `src/session/song.rs` | Evaluate entire document in fresh candidate evaluator/registry with staging-only hosts and immutable loaded assets; reject disallowed effects. | Completed; independently verified |
| `src/session/song/freeze.rs` | Bounded exhaustive transitive Value/Pat/Proto/closure and buffer rewriting; preserve sharing and certified identities. | Completed; independently verified |
| `src/song/part.rs` | Crate-private identity-preserving symbolic payload rewrite; retain revisions, seeds, exact durations, tracks and construction metadata. | Completed; independently verified |
| `src/session/mod.rs` | Register song session module. | Completed; independently verified |
| `src/session/session.rs` | Route new requests to candidate preparation, preserving existing incremental eval behavior. | Completed; independently verified |
| `src/ns/evaluator.rs` | Expose minimal construction/freezing access needed for independent candidate evaluator; do not shallow clone active mutable slots. | Completed; independently verified |
| `src/song/snapshot.rs` | Implement frozen namespace/kit/sample ownership and pending candidate cancellation. | Completed; independently verified |
| `tests/song_candidate.rs` | Failure injection for late form failure, missing samples, stale revision and attempts to update active globals through captured closures. | Completed; independently verified |

### Public declaration contract

```rust
pub struct CandidateBuildCtx<'a> {
    pub assets: &'a dyn SongAssetFactory,
    pub asset_limits: SongAssetLimits,
    pub lock: Option<&'a LockFile>,
    pub cache: Option<&'a dyn CacheBackend>,
}
pub fn evaluate_song_candidate(code: &str, file: &str, revision: u64, epoch: SnapshotEpoch, cx: &CandidateBuildCtx<'_>) -> Result<SongCandidate, Failure>;
pub fn cancel_song_candidate(pending: &mut PreparedSong, epoch: SnapshotEpoch) -> Result<(), Failure>;
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

- [x] Allow declarations plus one song entrypoint; reject slot binds, once/at, capture, external clocks/output and live input in transaction.
- [x] No active namespace, graph or transport changes before every candidate form and resource is prepared.
- [x] Tests prove active mutable values/functions/kit cannot change frozen snapshot query results and failed candidates cannot change active bindings.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_candidate)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — source-grounded snapshot preflight
Snapshot ownership must use a fresh prelude, registry, VM, namespace and recording
sink. Active VarSlotRef, package namespaces, closure globals/captures, registry
cells and live signal inputs cannot be shallow-cloned into the candidate. Drop
the complete private candidate on any form/import failure or forbidden effect;
partial successes never publish. Expose snapshot query-only borrowing without
mutable evaluator/registry aliases.

Ready SampleBuf values remain mutable through fill/fill_at/fail. Capture ready PCM
and rate into private candidate-owned copies, or immutable decoded SampleData;
reject pending/failed inputs. All retained Sound::Buffer references must point to
private copies, rather than retaining mutable active readiness/rate aliases.

NativeSampleLoader clones share mutable roots/files/banks/next state. Candidate
source/sample preparation needs an isolated factory/configuration and candidate
file IDs, package namespaces and bank registration. Existing Session::load_package
registers active banks, releases effects to the active sink and drains the active
runtime; it must not be reused for isolated candidate imports. Source analysis
taps must be absent/rejected. The loader capability gap requires an exact bounded
manifest amendment before editing any additional loader or portable boundary file.

Pin a closed candidate asset inventory before Ready; later song queries must not
reread changed disk bytes, mutate active loader maps or register new resources.
Do not replace lazy finite-song realization with a full arrangement scan.
Verification must mutate active globals/functions/captures/kit/import slots, ready
buffer data/rate, bank/file maps and disk bytes after preparation and prove stable
candidate queries/assets. Late form/import/missing-sample failures preserve active
namespace, graph and transport. This preflight records obligations, not completion.

### Session: 2026-10-01 — explicit candidate construction context
Read-only preflight found the free candidate entrypoint could not reach session
asset preparation or verified package configuration. Supply CandidateBuildCtx
explicitly, with factory/budget and read-only lock/cache references. Construct
isolated package sources and register imports only in the fresh candidate. Never
call Session::load_package or retain active namespace/sample loader aliases.
Use package_sources for verified cache reads; freeze source bytes/file IDs in the
candidate inventory. Inspect every evaluator diagnostic and form/import result;
any error discards the entire candidate. Keep substantive traversal and import
logic in the already owned session/song.rs; all touched modules remain <1000 lines.

### Session: 2026-10-01 — typed asset dependency classification
Consume bounded SONG-06A dependency discovery as typed candidates: ambiguous
literal paths, known Sound::Sample paths, sound keywords, and ready buffers.
FnProto constants can contain source-import paths as well as sample paths; sound
keywords may name builtin/custom instruments or banks. Use the frozen candidate
registry and construction context to classify before pinning. Do not decode every
literal path as WAV or treat every sound keyword as a bank. Pin imported source
bytes through the isolated source loader, sample routes through complete inventory,
and rewrite all retained buffer references to private copies consistently.
An unresolved resource that could require a later host/filesystem read must fail
preparation explicitly; closed lookup must not fall back to active loaders.

### Session: 2026-10-01 — immutable routing inventory publication
While certifying the fresh candidate, construct immutable copied routing metadata
for the used frozen instrument families and effect/track/master templates. Provide
a crate-private immutable metadata view for SONG-08 preparation, including strict
named-bus resolution; never expose evaluator or mutable registry RefCell aliases.
The inventory must cover symbolic arrangements and bounded callback dependencies
without expanding repeats or querying the complete song. Actual host generation
residency/admission remains SONG-08/09/10.
Also classify ambiguous literal keyword dependencies added by repaired SONG-06A:
lazy sound functions can keep keywords in prototype constants. Resolve against
this candidate's frozen registry/context, never pin every metadata keyword as a bank.

### Session: 2026-10-01 — identity-preserving freeze and bounded split
Source preflight found Part fields private and public construction assigns fresh
PartRevision values. Transitive rewriting through those constructors would
invalidate certified deletion handles and selected SongSource revisions. Add
only src/song/part.rs for a crate-private payload rewrite that preserves revision,
seed identity, duration, declared tracks, node/depth bounds and arrangement/edit
structure. It must not permit arbitrary structural edits or expose mutable Part
fields publicly. Preserve handle identities and selector/resource rewrites
consistently; verify edited/selected-source queries after freezing.
Add src/session/song/freeze.rs as the bounded exhaustive traversal split for
Value, Pat, FnProto, closures/globals/captures/memo and captured buffers. Use
visited maps and existing checked work/depth limits; never expand finite repeats.
The manifest now has exactly eight Rust modules plus this plan. All touched
modules remain below 1000 lines. Source-grounded ROOT0030 is not a Riela review.

### Session: 2026-10-01 — copied usage and symbolic track routes
Copy the repaired closed asset pcm_bytes scalar into snapshot admission metadata.
Routing inventory must retain bounded symbolic sequence offsets/durations, repeat
counts and edit/FX scopes/source revisions, or equivalent certified topology; a
global graph list alone cannot distinguish placement-dependent effect branches.
Do not expand repeats or scan the full score. Preserve selected-source origin
cutoffs and plate/spring/fast2 semantics from SONG-04. Family identity DTOs copy
buffer IDs and resolved routes, never Rc<SampleBuf> or mutable registry aliases.

Each declared Part track owns a distinct intermediate summing stage. An existing
same-name declared bus supplies that stage's effect-chain template; when none is
declared, explicitly prepare a neutral track stage. This preserves the accepted
24-cycle example, which declares tracks but no bus templates. It is never a
direct branch-to-master fallback: branches still sum through their track stage.
Explicit named bus/FX selections require exact declared registry lookup and
reject unknown names. Keep the restricted branch -> track stage -> master
topology; do not infer arbitrary user routing graphs. Existing event bus controls
must be resolved/validated against this topology, with explicit diagnostics for
unsupported conflicting track destinations. ROOT0031 records a source-grounded
clarification, not an additional Riela acceptance.

### Session: 2026-10-01 — explicit cancellation ownership
The original epoch-only free function cannot reach a session-owned pending
candidate without hidden global state. Replace it with explicit mutable borrow
of the addressed PreparedSong plus epoch. Validate current epoch and pending
Preparing/Ready state, transition cancellation to Failed with a typed reason,
and preserve generation-qualified reservations for owner cleanup. Never cancel
Applied playback, and never drop reserved leases before host cleanup. Session
cancellation delegates to this owned operation; stale/unknown epochs fail.
The temporary unconditional HostUnavailable stub does not satisfy this contract.
No new source paths: session/song.rs and song/snapshot.rs are already owned.
ROOT0032 is a source-grounded contract correction, not a Riela review.

### Session: 2026-10-01 — aggregate metadata budget using existing walker
Use repaired SongAssetDependencies.consumed_work() to debit every per-payload
and family-root dependency traversal from one checked remaining metadata budget.
Pass only the remaining work allowance and reject exhausted admission before
allocation/traversal. Cache repeated payloads by full original identity; compact
NodeId alone does not certify equality. Preserve symbolic topology and borrowed
source traversal, without deep-cloning Pat subtrees or expanding repeats. This
reuses the exhaustive dependency implementation rather than duplicating it in
SONG-07. Root33 authorizes only the disjoint assets/test prerequisite repair;
SONG-07 still owns exactly eight Rust modules and final join waits its independent
re-clearance. Source-grounded clarification, not a Riela review.

### Session: 2026-10-01 — isolated evaluation and transitive ownership implementation
**Tasks Completed**: All eight declared Rust modules implemented. Exact identity-preserving Part payload mapping, complete staged document/import diagnostics, verified nested package source retention, closed PCM/source ownership, bounded transitive prototype/capture/memo/buffer rewriting and copied symbolic routing inventory are exercised.
**Tasks In Progress**: Final author gates and mandatory independent review; no host route admission or actual Ready/Applied playback is claimed.
**Evidence so far**: `tmp/song-mode-riela/SONG-07/tests-thirteenth.log` has 13 public fixtures passing (99664 terminal exit 0); `freeze-tests-eighth.log` has six private fixtures passing (77953 terminal exit 0). `control-tests-second.log` has two aggregate-budget/caller-depth fixtures passing (4291 terminal exit 0), independently checked at `/tmp/vactr-song07-controls-independent-001/results.json`.
**Corrections retained**: Earlier recursive callback syntax failures and normal-stack depth-200 SIGABRT logs remain in the same directory. Light Part/Pattern/Song dispatch and split pattern dispatch now preserve depth 256 with successful 200-depth ordinary test threads and explicit 300-depth failure. Prototype metadata counting moved into already-owned session/song.rs to keep freeze.rs below 1000 lines.
**Preservation**: No dependencies, lockfiles, Git operations, active-loader mutations or unrelated editor/session publish changes. Source intent records 0001–0004 are retained under the phase evidence directory. ROOT0033 asset consumed-work repair is independently cleared and debited across inventory traversal.
**Next**: Run terminal native/host-wasm/Clippy/scoped-format/legacy/nextest gates, seal the exact eight Rust hashes and hand off to the required checker. Final phase completion/archive remain deferred until independent approval.

### Session: 2026-10-01 — author terminal seal
**Author verification**: Foreground session 6772 is terminal exit 0. All ten gates in `tmp/song-mode-riela/SONG-07/author-final-results.json` exited 0: native check, host-wasm check, 13 public native fixtures, six private transitive-freeze fixtures, eight snapshot/control/lease fixtures, 78 legacy session fixtures, strict all-target Clippy, 13 nextest repeated fixtures, scoped rustfmt and diff check. Full output is retained in `author-final-*.log` beside the ledger; total distinct selected fixtures: 105, plus 13 nextest repeats.
**Seal**: `tmp/song-mode-riela/SONG-07/0006-author-source-seal.json` records exact eight Rust SHA-256 values and line counts. Maximum touched source is freeze.rs, 953 lines; snapshot.rs is 948. No foreground author process remains. Sources are stable for the mandatory checker.
**Scope boundary**: Fresh isolated snapshots and honest Preparing ownership are implemented; actual host graph admission, Ready/Applied activation, transport and live mute remain later phases. Independent review has not yet completed, so this plan remains In Progress and archive/index work stays deferred.

### Session: 2026-10-01 — independently verified phase completion
**Tasks Completed**: TASK-001, TASK-002 and TASK-003; all declared module deliverables and phase acceptance criteria are complete.
**Independent evidence**: `/tmp/vactr-song07-independent-001/final-results.json` records all 11 gates exit 0, 106 distinct fixtures (including the privacy compile-fail fixture) plus 13 nextest repeats. Retained checker session 88913 is terminal exit 0 and no foreground process remains. Native/host-wasm compilation, strict all-target Clippy, scoped format, legacy/session behavior and diff gates passed; no outstanding concrete source-review finding.
**Preservation**: All eight source SHA-256 values match the author seal before and after independent verification and were checked again during this documentation-only completion. Largest source remains 953 lines. Immutable completion intent is `tmp/song-mode-riela/SONG-07/0008-independent-completion.json`; this update changes only the owned plan.
**Scope**: SONG-07 isolated whole-code evaluation and immutable preparation are independently approved. This does not claim host admission, actual Ready/Applied playback, transport or live mute; those remain later phases of the active full song-mode goal. Archive/index updates remain deferred to root/SONG-16.
