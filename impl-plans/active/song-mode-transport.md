# Finite song transport and atomic activation implementation plan

**Status**: In Progress
**Plan ID**: SONG-11
**Plan Path**: impl-plans/active/song-mode-transport.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-03
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Finite song transport and atomic activation supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Historical unsplit manifest — superseded by child plans

```json
{
  "planId": "SONG-11",
  "planPath": "impl-plans/active/song-mode-transport.md",
  "dependsOn": [
    "SONG-07",
    "SONG-10",
    "SONG-HOST-PREPARATION"
  ],
  "writePaths": [
    "src/sched/song.rs",
    "src/sched/mod.rs",
    "src/sched/runtime.rs",
    "src/sched/commit.rs",
    "src/session/session.rs",
    "src/session/song.rs",
    "src/session/publish.rs",
    "tests/song_transport.rs",
    "impl-plans/active/song-mode-transport.md"
  ],
  "sharedPaths": [
    "src/sched/runtime.rs",
    "src/session/song.rs",
    "impl-plans/active/song-mode-transport.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/sched/runtime.rs",
      "intendedEdit": "Schedule finite queries/end before horizon; select next integer cycle beyond commit lead and 64-frame fade preparation. Owners execute serially: SONG-05, SONG-08, SONG-11"
    },
    {
      "path": "src/session/song.rs",
      "intendedEdit": "Connect candidate Ready to scheduled activation and publish Applied only on host acknowledgment. Owners execute serially: SONG-07, SONG-11"
    },
    {
      "path": "impl-plans/active/song-mode-transport.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-07](song-mode-candidate-evaluation.md), [SONG-10](song-mode-host-adapters.md), [consuming host preparation](song-mode-host-preparation.md)
- **Next**: [SONG-12](song-mode-export-core.md), [SONG-14](song-mode-browser-session.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-07 | Original privately owned PreparedSong; isolated candidate source exists, closure verification underway | Scoped source implemented; joined verification pending |
| SONG-10 | Actual Native/Arena sender, clock, staging, materialization and reusable pools | Scoped prerequisites verified; consuming preparation source in progress, joined verification pending |
| SONG-HOST-PREPARATION | Once-only SongReadyBundle with original PreparedSong, admitted routes/pools/stages, exact leases and observed host clock | Ready contract released; owner implementation underway, actual handoff and cleanup verification pending |

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
| `src/sched/song.rs` | Implement Prepared/Playing/Draining/Ended/Failed transport and rational arrangement endpoint. | NOT_STARTED |
| `src/sched/mod.rs` | Register song transport module. | NOT_STARTED |
| `src/sched/runtime.rs` | Schedule finite queries/end before horizon; select next integer cycle beyond commit lead and 64-frame fade preparation. | NOT_STARTED |
| `src/sched/commit.rs` | Commit mono song events once with branch/epoch; exclude onsets at end, cancel old queued work at swap. | NOT_STARTED |
| `src/session/session.rs` | Dispatch actual Apply and mute requests, retain addressed preparation, and transfer only a genuinely Ready bundle to Runtime. | NOT_STARTED |
| `src/session/song.rs` | Connect candidate Ready to scheduled activation and publish Applied only on host acknowledgment. | NOT_STARTED |
| `src/session/publish.rs` | Publish endpoint/state, epoch, revision and applied/mute frame without confusing candidate readiness. | NOT_STARTED |
| `tests/song_transport.rs` | Check finite end, fractional boundaries, restart, fault states, mute continuity and rollback. | NOT_STARTED |

### Public declaration contract

```rust
pub enum SongTransportState { Prepared, Playing, Draining, Ended, Failed }
pub struct SongTransport { state: SongTransportState, ready: Option<SongReadyBundle>, arrangement_end: Ratio64, activation_frame: u64, tail_end_frame: u64 }
impl Runtime {
    pub fn start_song(&mut self, ready: SongReadyBundle, activation_frame: u64) -> Result<(), Failure>;
    pub fn mute_instrument(&mut self, epoch: SnapshotEpoch, selector: InstrumentSelector, muted: bool) -> Result<(), Failure>;
    pub fn song_state(&self) -> Option<SongTransportState>;
}
```

## Ready ownership and receipt integration prerequisite

Runtime must consume the actual SongReadyBundle by value and retain its original
PreparedSong, closed route/pool/stage authority and accepted lease cleanup
ownership. A PreparedSong alone proves neither host readiness nor ownership of
installed resources. Diagnostic Runtime.pending_song: Option<Rc<Song>> is not
this bundle; do not clone or reuse it as playback authority.

The consuming preparation contract now declares exclusive query, once-only
Ready handoff and cleanup retention. Its implementation is underway under
ROOT0529; no held owner or behavioral verification exists yet. Borrowed
prepared() alone cannot move its private evaluator or cleanup ledger. Keep this
plan Planning until the actual owner handoff and cleanup pass joined verification.

Preparation and transport share one bounded receipt dispatcher. Process owned
ACKs once; preserve foreign, stale and non-song messages intact for their owners.
Independent queue polling must not steal ResourceReady, Applied, mute or lease
return obligations. Ready retains the original cleanup ledger, limits and physical
mappings. Activated or uncertain activation enters AwaitingRetirement; musical
endpoint/voice/tail completion must be established before staging cancellation
releases dependencies. Current Endpoints has no success receipt; acknowledged
completion or a tested actual safe-return protocol is a source prerequisite.
CancelPreparation and elapsed host frames alone do not prove musical completion. Failure and cancellation retain accepted ownership until
actual safe returns. Reobserve the actual host clock for activation; preserve
legacy transport IDs/epochs and MIDI restart generations separately.

The actual session/session.rs dispatcher owns Apply/mute entrypoints and replaces
the previously proposed slots.rs write. Runtime integration continues preserving
legacy slots and rejects mixed playback/tempo changes; that obligation is not
removed by the manifest correction. The future source manifests are now owned by the bounded child plans.
Current runtime900/commit869/session832/song912 are below the line limit; place
cohesive finite helpers in sched/song.rs before integration growth.

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

- [ ] Preserve existing transport telemetry epoch/end_time and MIDI restart-generation semantics. Song snapshot epoch is a separate identity; never overwrite or remove existing transport epoch. Add regression assertions to song transport fixtures.
- [ ] Use absolute round-to-nearest ties-up frame mapping; do not sum rounded child frame durations.
- [ ] End is exclusive for onsets; release held voices then drain exactly tail cap, with bounded final fade.
- [ ] Apply restarts at zero, fades old audio before boundary and new after; clears old song master/private tails at boundary.
- [ ] Mute overlay mapping only retains existing instrument-family selectors across Apply; static export ignores overlays.
- [ ] Fault stops new song onsets and reports Failed without terminating editor session.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_transport)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — cross-snapshot mute identity preflight
Fresh candidate registries allocate custom InstId values in declaration order.
The same integer can name a different custom instrument after whole-code Apply.
Within an epoch retain exact family equality and reject stale commands. Across
Apply, remap instrument family members using their certified immutable declared
identity (including copied instrument names), never by numeric InstId equality
alone. Removed/renamed families drop overlays; reordered declarations of a
remaining family preserve its mute on the new actual ID. Sample paths must use
certified source-origin identity, not reused candidate FileId integers; captured
private buffer IDs require a proven stable logical identity to carry an overlay.
When stable identity cannot be certified, discard that overlay rather than mute
an unrelated new instrument. Preserve full multi-member family matching.
Add regression fixtures for reordered custom declarations, reused numeric IDs
with renamed instruments, changed relative-path source origins, stale epochs,
and remaining-family mute continuity. No new source paths or routing framework.
ROOT0034 records a source-grounded clarification, not a Riela acceptance.

### Session: 2026-10-01 — explicit transport ownership
Root inspected current Runtime, which owns host handles and retained receipt
state. Song transport methods are explicit methods on that Runtime owner; no
hidden process-global transport or snapshot registry. An absent song returns
None without inventing Prepared telemetry. Snapshot transport identity remains
separate from legacy MIDI/transport epochs. This source-grounded signature
correction changes no accepted playback, Apply, mute or end semantics and adds
no Rust ownership paths. ROOT0045 records the future implementation contract.

### Session: 2026-10-02 — Actual consuming transport preflight

Fresh read-only source audit found real Apply/mute dispatch in session/session.rs,
while pending_song in Runtime still retains diagnostic Rc<Song>. Host preparation
remains Planning and SongReadyBundle is absent from Rust. Corrected the future
start_song contract to consume that bundle, kept its original prepared evaluator
and cleanup obligations, and replaced the unused slots.rs manifest entry with
the actual session dispatcher. Explicitly restored Planning until the ownership
and single-receipt-routing contract is implemented. No Rust or Cargo changed;
all playback, finite end, Apply, mute, export and integration criteria remain
unchecked. Existing source work and scoped gate acceptance remain preserved.

### 2026-10-02 — ROOT0468 activated retirement handoff
Read-only host audit refined Ready cleanup retention and the single receipt
router contract. The transport must coordinate real musical completion before
consuming the bundle into active-resource cleanup; keep partial activation and
unknown ACK outcome owned. Preparation/transport remain Planning; no source
or readiness authorization is implied by the doc-only refinement ROOT0469.


### 2026-10-02 — ROOT0531 current transport dependency audit

Read-only inspection confirms sched/song.rs remains absent and Runtime::tick
still queries and commits legacy slots. The consuming owner is now being
implemented under ROOT0529, rather than absent as the historical preflight
recorded. Ready query, original candidate ownership, sample-to-full-key lookup
and exclusive activation are declared; actual behavior awaits held source and
joined verification. No transport criterion is complete.

The time-source effect companion is held under ROOT0530. Root verified all six
source/document hashes and the three preserved inference/callable hashes; this
is source integrity evidence only. It does not establish a passing query route.

Before transport source release, separate finite transport core from Runtime
and session integration into bounded child plans. Current principal integration
files are already large; do not place the complete scheduler in their remaining
line headroom or add unlisted split paths. The split must preserve all existing
endpoint, exact-frame, progression, Apply/mute, failure and export obligations.
A read-only specialized source audit is underway to ground that split in actual
FrozenSongEvent, route, host command and Engine lifecycle authority. No Rust,
Cargo, nextest, dependencies or Git changed in this documentation audit.


### 2026-10-02 — ROOT0534 source-grounded bounded transport split

The historical unsplit manifest above is superseded for future source releases
by [SONG-11A core](song-mode-transport-core.md), then
[SONG-11B Runtime/session integration](song-mode-transport-integration.md).
This parent remains Planning and owns its document only; it releases no Rust
paths. Child plans retain every original acceptance criterion, with seven and
eight respective Rust modules and explicit serial dependencies. Legacy commit
is read-only because its live SampleTable/cell/Clock authority cannot substitute
for frozen private encoding. Runtime/session helpers are extracted coherently
before the existing large files grow.

Actual public owner assembly always installs a Master resource, including
silent songs. Root checked its unconditional stage and actual Engine retirement
predicate: Master cannot normally return before an epoch endpoint deadline.
Full-key normal-return evidence may therefore establish finite completion
without a new wire acknowledgment; this still requires genuine silent-song and
pressure fixtures and exact cancellation/endpoint distinctions. No raw zero-
resource Engine carrier is accepted as a public ReadyBundle counterexample.

No child is Ready; host owner and actual public callable routes still require
joined verification. Dynamic geometry, full progression, Apply/mute, Native and
browser playback, automatic run completion and complete-song export remain
mandatory. No Rust, Cargo, nextest, dependencies or Git changed.

### 2026-10-03 — current integration review and next implementation steps

The preceding entries are historical. Finite core and Runtime/session paths now
exist. Focused evidence proves actual Native finite progression, complete CLI
rendering (including the 50-second generated-parts example), pending-document
cancellation and acknowledged family mute/unmute. See their child plans for
the precise tests and limits; these checks do not prove the full parent scope.

The actual browser tests previously exposed a stale clock observation. That
correlated refresh correction and late interactive mute handling are now verified:
all four real ABI fixtures pass against artifact b29cfd442341ed5544d1fb9058d4fa1acfb0bbff2f7782d9157626d80f54ccca.
See completed/song-mode-browser-runtime-evidence.md for the precise checkpoint.
This proves initial finite playback and mute, not replacement during playback.
New atomic-Apply browser fixtures currently fail with genuine second-candidate
bus capacity refusal against that previous artifact; their baseline is recorded
in song-mode-atomic-apply-controller.md. They remain required acceptance checks.

Atomic replacement is a separate remaining requirement. A compound validated
transaction must preserve old playback on pre-arm rejection, apply exact
64-frame fades at the chosen cycle boundary, cancel old future work, remap
certified mute families and retire original resources. Wire, prepared-owner,
DSP and Session phases will be implemented serially under bounded plans.
Actual constructor capacity must also support the example and overlapping
owners within measured memory limits. Parent completion remains unproven.

### 2026-10-03 — measured resource and transaction follow-up

The authentic bootstrap probe and handoff checkpoint pass:25 buses/22 templates
per unchanged example, after73 prelude templates. Uniform51 full-budget bus
allocation is superseded by song-mode-bus-memory-layout.md: preserve6 full
regions, add45 bounded smaller regions and deterministic best-fit adoption.
The layout and DSP transaction authors are implementing disjoint phases with
coordinated Engine edits. Adapter integration follows verified layout.
Controller integration is planned in song-mode-atomic-apply-controller.md,
including natural old completion during preparation, exact musical boundaries,
semantic mute continuity and requester/document cancellation. No full completion
is claimed by these narrower checkpoints.
