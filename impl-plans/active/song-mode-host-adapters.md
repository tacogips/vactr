# Native and worklet song adapters implementation plan

**Status**: In Progress
**Plan ID**: SONG-10
**Plan Path**: impl-plans/active/song-mode-host-adapters.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-02
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Native and worklet song adapters supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-10",
  "planPath": "impl-plans/active/song-mode-host-adapters.md",
  "dependsOn": [
    "SONG-09",
    "SONG-10-COMMANDS"
  ],
  "writePaths": [
    "src/host/caps.rs",
    "src/host/native/audio.rs",
    "src/host/wasm/messages.rs",
    "src/host/wasm/worklet_half.rs",
    "tests/song_hosts.rs",
    "impl-plans/active/song-mode-host-adapters.md"
  ],
  "sharedPaths": [
    "src/host/caps.rs",
    "src/host/native/audio.rs",
    "src/host/wasm/messages.rs",
    "tests/song_hosts.rs",
    "impl-plans/active/song-mode-host-adapters.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/host/caps.rs",
      "intendedEdit": "Checked submission/size-safe song child extraction delegated serially to SONG-10-COMMANDS; parent is read-only until child independent clearance and fresh root source release. Parent later full preparation/Apply work retains separate acceptance."
    },
    {
      "path": "src/host/native/audio.rs",
      "intendedEdit": "Checked submission/size-safe song child extraction delegated serially to SONG-10-COMMANDS; parent is read-only until child independent clearance and fresh root source release. Parent later full preparation/Apply work retains separate acceptance."
    },
    {
      "path": "src/host/wasm/messages.rs",
      "intendedEdit": "Checked submission/size-safe song child extraction delegated serially to SONG-10-COMMANDS; parent is read-only until child independent clearance and fresh root source release. Parent later full preparation/Apply work retains separate acceptance."
    },
    {
      "path": "tests/song_hosts.rs",
      "intendedEdit": "Checked submission/size-safe song child extraction delegated serially to SONG-10-COMMANDS; parent is read-only until child independent clearance and fresh root source release. Parent later full preparation/Apply work retains separate acceptance."
    },
    {
      "path": "impl-plans/active/song-mode-host-adapters.md",
      "intendedEdit": "Parent-only progress; child author updates only child plan. ROOT0299 documents do not release Rust."
    }
  ],
  "delegatedWritePaths": {
    "SONG-10-COMMANDS": [
      "src/host/caps.rs",
      "src/host/caps/song.rs",
      "src/host/native/audio.rs",
      "src/host/native/audio/song.rs",
      "src/host/wasm/messages.rs",
      "src/host/wasm/messages/song.rs",
      "tests/song_hosts.rs"
    ]
  }
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-09](song-mode-dsp-routing.md)
- **Child / serial sender deliverable**: [SONG-10-COMMANDS](song-mode-host-command-submission.md)
- **Next**: [SONG-11](song-mode-transport.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-09 | Full accepted DSP behavior and recorded stable phase evidence; first audible increments alone are insufficient | PENDING FULL ACCEPTANCE |
| SONG-10-COMMANDS | independently verified checked all-command submission / exact ownership / actual browser pacing | NOT_STARTED |

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
| `src/host/caps.rs` | Checked capability delegated to serial child; parent later readiness/capacity preparation integration. | DELEGATED / PARENT PENDING |
| `src/host/native/audio.rs` | Prepare resources and forward epoch/frame commands; maintain readiness/applied acknowledgments through native engine. | NOT_STARTED |
| `src/host/wasm/messages.rs` | Carry identical prepared route resources and song commands through bounded worklet arena/ring. | NOT_STARTED |
| `src/host/wasm/worklet_half.rs` | Consume commands and return actual frame acknowledgments, including rejection and stale-epoch behavior. | NOT_STARTED |
| `tests/song_hosts.rs` | Run command parity, resource exhaustion, preparation cancellation and fault acknowledgment fixtures. | NOT_STARTED |

### Public declaration contract

```rust
pub trait AudioHost {
    fn try_song_command(&mut self, command: SongCommand)
        -> Result<(), SongCommandRefusal>; // typed child capability
}
```

Activation and mute use the same typed all-command admission capability. The previous
two Result<(), Failure> convenience declarations are superseded; do not introduce
unrelated checked wrappers that discard the exact refused command. Checked enqueue
success still does not mean Ready/Applied. SONG-10-COMMANDS owns its seven Rust paths
serially, including newly declared children; parent must not write them concurrently.
The parent resumes only after mandatory child clearance and fresh explicit source
release. Parent manifest original paths remain eventual preparation/worklet owners;
new preparation/clock helpers need prior bounded ownership review before addition.

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

- [ ] Both hosts apply same frame/epoch semantics and never report Applied before actual audio-side acknowledgment.
- [ ] Native and browser actual graph, voice, arena and cell limits reject oversized candidates before swap.
- [ ] These are adapter methods on existing host types, not new free-standing process/service lifecycle.
- [ ] Host construction retains an immutable actual configured capacity snapshot; free admission subtracts active/staged/retiring leases and checks individual state regions.
- [ ] NativeArc's zero arena-byte sentinel is not zero PCM capacity; an explicit bounded native PCM budget and actual sample-table capacity are reported.
- [ ] Closed snapshot samples supply uploads without late active loader/bank/disk reads; every resource/cell install is associated with the reserved epoch/generation before readiness.
- [ ] Full sender and garbage queues retain commands/native ownership or reject explicitly while old playback and leases remain intact.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_hosts)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — checked sender admission and host ownership
Root source review finds current AudioHost::post_song delegates to fire-and-forget
post; NativeAudioHost::post_record drops the control record on a full ring.
Actual song preparation/activation/mute/end submission must expose checked
acceptance or retain exact commands for bounded retry. Never begin waiting for
Applied/Muted after silently losing its command. Saturate the actual sender
queue and verify retry or explicit failure with old snapshot/resources intact.
Default checked capability is explicit unavailable and cannot call an unchecked
legacy post as successful admission. Preserve existing legacy APIs and POD tags.
AudioHost methods use an explicit host owner rather than hidden global functions;
control-thread Failure diagnostics are allowed, callback POD rejection remains
allocation-free. Acknowledgments still report actual audio-side frames, and
expired/stale submissions cannot manufacture application at a requested past
frame. Resource IDs/generations must be checked against current/retiring ownership,
not blindly reuse legacy wrapping fresh_graph IDs. caps.rs is the fifth owned
Rust source, serialized after08/08A; no other additional path is authorized.
ROOT0041 records this source-grounded host admission correction, not new Riela
acceptance.

### Session: 2026-10-01 — actual initial and Apply capacity
Root inspected actual EngineConfig defaults: DEFAULT_BUS_SLOTS is six, while
song routing reserves each private live branch, each distinct track stage and
master, including staged/retiring generations. The accepted 24-cycle example
and whole-code Apply must work on ordinary supported host initialization, not
only tests with synthetic oversized capacities. Measure and configure bounded
actual native/worklet allocations before callback construction, or use explicitly
reported dedicated song allocations supplied by SONG-09; never invent free bus
slots or allocate during callback/Apply. Keep legacy graph behavior intact and
account for its occupied slots alongside the current, staged and retiring song.

Run the actual accepted example through real native and Arena/worklet preparation
without a --cycles argument. Verify a second compatible snapshot can prepare and
Apply while the old snapshot is active; fixed voice pool is shared and old voices
are faded/cleared before reuse, private graph/cell/bus/sample arenas are genuinely
leased. Also verify an intentionally smaller actual configuration rejects before
activation with old playback/resources intact. Oversized songs still receive
explicit capacity diagnostics. ROOT0043 records this source-grounded validation
requirement within existing host-adapter paths, not new Riela acceptance.

### Session: 2026-10-01 — certified detector bindings within restricted routing
Accepted Routing and tails requires validation of selected sidechain selectors.
Actual BusSlot detector snapshots read pre-effect linked-channel inputs across
matching live/retiring BusId generations; they do not redirect summed audio.
SONG-08B therefore certifies copied template/unit -> declared root track/logical
track-stage detector bindings rather than blanket-rejecting all sidechains.
Unknown, non-track, malformed or prohibited self selectors fail before activation.

Remap template bus IDs and every sidechain control to the actual privately
installed snapshot track-stage identities. Old, staged and new snapshots must
never read a peer detector merely because candidate numeric BusIds collide.
Keep branch -> track -> master sums unchanged; retain the actual pre-effect
detector semantics in allocation-free render ordering. Reserve detector snapshot
workspace explicitly and use actual generation leases through tail retirement.
Prove valid kick-keyed private/track compressor behavior, unresolved-selector
rejection and old/new epoch detector isolation on both Native and Arena paths.
Do not substitute silent zero input or master fallback for a resolved binding.
Any genuine unsupported causal/self case receives an explicit diagnostic.
No extra Rust path is authorized. ROOT0046 records this source-grounded handoff,
not a new Riela acceptance or arbitrary audio routing framework.

### Session: 2026-10-01 — configured capacities and native preparation handoff

Read-only native preflight finds pair (audio.rs503) does not retain the full
EngineConfig capacity snapshot. Capture measured template/bus/voice memory, cells,
sample table and PCM allowances at actual native/worklet construction; CapabilitySet
alone is insufficient. NativeArc SampleStore::capacity_bytes returns zero as a
no-arena sentinel (arena.rs205), not a zero usable PCM allowance. Native song upload
requires an explicit bounded budget. Browser sample arena and largest contiguous
extent remain separate from aggregate PCM totals and fixed per-slot DSP regions.

NativeSongAssetFactory (loader.rs267–306) already supplies fresh configuration-only
preparation. Upload pinned closed snapshot data rather than consulting an active
loader or reopening files. Coordinate checked sender submission, exact reserved
kind/epoch/generation association and actual ResourceReady/SealPreparation with09;
legacy Installed is not song Ready. Full native boxes/Arcs remain owned on rejection
or queued cleanup; wrapping legacy graph IDs cannot provide safe snapshot leases.

Real native/Arena fixtures must cover checked queue saturation, wrong/stale install
association, silent staged master/self-noise, missing/received cell init, overlapping
snapshot leases, and one-short individual state/fragmented sample failures. Accepted
example playback and Apply still require actual configured capacity. No additional
Rust path or future implementation is released. ROOT0071/0072 record this refinement.

### Session: 2026-10-02 — ROOT0299 checked sender child delegation

The reviewed temporary proposal revision0002 grounds
[song-mode-host-command-submission.md](song-mode-host-command-submission.md),
SONG-10-COMMANDS. Its seven Rust paths are delegated serially for actual checked
all-command admission, cohesive size-safe splits and exact upload refusal/pacing.
This is a separate implementable sender deliverable on verified frozen carrier
contracts after current authors' mandatory joined checks, not a declaration that
full09 is complete or a release of Rust work. Parent/source shared ownership remains
held until explicit root release; no parallel author may duplicate child edits.

All parent readiness and full09/full10 criteria remain required: actual per-region
and staged/retiring capacity; complete closed PCM/graph/control/analysis admission;
neutral private branch/track/master preparation for plain songs; exact frame+rate
observation; one bounded Ready/Applied/cancellation owner; genuine native/browser
resource state and actual application frames. Enqueue is not host Ready. A sender
child alone cannot implement finite playback or consume an Rc<Song> as a frozen
snapshot. Session/Runtime handoff must move the closed PreparedSong, retain its
route/full lease metadata, and publish Applied only on actual matching host ACK.

Preserve accepted post-private-FX / pre-track mute gating, including private tails
and exclusive epoch/branch/generation graph/bank ownership. Browser partial uploads
retain exact accepted-step/slice cursors and cancel started full leases; no fabricated
all-or-none upload rollback. Actual neutral stages must be uploaded and priced in
route/admission requirements before activation; no implicit callback allocation,
shared orbit or master fallback. Such extra route/preparation/clock source changes
require separately reviewed bounded manifests, not this sender child's scope.

The accepted24-cycle example, ordinary default host allocation and simultaneous old/
new Apply capacity proof above remain mandatory. Full Apply restarts local zero at
the next integer cycle beyond commit lead, fades old/new64 frames, clears old tails,
retains surviving immutable family overlays and rejects stale epochs. End-exclusive
finite onsets, exact tail cap/final fade, isolated candidate failure preserving old
playback and editor, native automatic run/export and browser playback stay required
through SONG-11/12/session consumers; export ignores mute overlays and permits silence.
Legacy transport telemetry identity remains separate from snapshot epoch.

Document-only planning complete. No implementation task or fullphase acceptance is
marked complete; no source/Cargo/index/archive/Git changes. Immutable prior/after
receipts are under tmp/song-mode-riela/SONG-10-COMMANDS.

### Session: 2026-10-02 — checked sender child independently complete

SONG-10-COMMANDS is independently verified by ROOT0317: native private4,
public song_hosts5 and unique nextest9 all pass; actual complete-record pressure
and upload ownership/pacing preserve real Engine receipts. Full parent is
In Progress. Exact clock child is Ready in song-mode-host-clock.md; neutral
stages, measured preparation owner, consuming Session/Runtime handoff, finite
transport, Apply and native export remain unfinished. The sampled-reverse route
failure and full09/08B/08D prerequisites are preserved. No host Ready/playback
claim is inferred from command enqueue. Archive/index/Git work is deferred.
