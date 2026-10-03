# Streaming complete-song WAV export implementation plan

**Status**: In Progress
**Plan ID**: SONG-12
**Plan Path**: impl-plans/active/song-mode-export-core.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Streaming complete-song WAV export supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-12",
  "planPath": "impl-plans/active/song-mode-export-core.md",
  "dependsOn": [
    "SONG-11"
  ],
  "writePaths": [
    "src/song/export.rs",
    "src/song/wav.rs",
    "src/song/mod.rs",
    "tests/song_export.rs",
    "src/song/snapshot.rs",
    "src/session/song.rs",
    "tests/song_candidate.rs",
    "impl-plans/active/song-mode-export-core.md"
  ],
  "sharedPaths": [
    "src/song/mod.rs",
    "impl-plans/active/song-mode-export-core.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Export native-only export entry point with target/feature gates; portable core remains wasm clean. Owners execute serially: SONG-01, SONG-04, SONG-06, SONG-08, SONG-12"
    },
    {
      "path": "impl-plans/active/song-mode-export-core.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-11](song-mode-transport.md)
- **Next**: [SONG-13](song-mode-cli.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-11 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |

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
| `src/song/export.rs` | Render SongSnapshot through production headless host, scheduler and finite transport; return metadata. | NOT_STARTED |
| `src/song/wav.rs` | Adapt existing examples/render_track/wav.rs checked streaming writer into reusable core without changing the legacy example. | NOT_STARTED |
| `src/song/mod.rs` | Export native-only export entry point with target/feature gates; portable core remains wasm clean. | NOT_STARTED |
| `tests/song_export.rs` | Exact frame counts, zero/silent songs, natural release, deterministic seeds, RIFF overflow and incomplete-output cleanup. | NOT_STARTED |
| `src/song/snapshot.rs` | Retain isolated candidate warning messages through PreparedSong and expose immutable metadata | NOT_STARTED |
| `src/session/song.rs` | Capture actual non-error candidate diagnostics; owned only by integration author | NOT_STARTED |
| `tests/song_candidate.rs` | Warning retention through actual isolated candidate construction | NOT_STARTED |

### Public declaration contract

```rust
pub struct SongExportOptions { pub output: PathBuf, pub sample_rate: u32 }
pub struct SongExportReport { pub arrangement_frames: u64, pub tail_frames: u64, pub total_frames: u64, pub epoch: SnapshotEpoch, pub seed: u64 }
pub fn export_song(song: PreparedSong, options: &SongExportOptions) -> Result<SongExportReport, Failure>;
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

- [ ] No manual cycles, normalization, unbounded PCM buffering or required audible energy.
- [ ] Graph readiness at musical zero; reject RIFF/frame overflow and source/output alias before final output creation.
- [ ] Temporary sibling output finalized only after every realization/host check; failure leaves prior final output untouched.
- [ ] Report all accepted metadata: revision, seed policy, tempo/meter, rational duration, frames/rate, warnings and final transport state.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_export)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-01 — owned frozen export candidate
Root inspected current SongSnapshot::query/sample and PreparedSong::query/sample:
they require an exclusive mutable owner of the private VM and pinned inventory.
An immutable &SongSnapshot cannot drive production transport without interior
mutability or an uncertified shallow clone. Export therefore consumes a freshly
prepared, privately owned PreparedSong from the isolated candidate builder.
Native CLI builds this fresh candidate, then export stages resources on its own
headless host and starts at musical zero after actual readiness acknowledgments.
Reject an already applied candidate or leases owned by another host; never reuse
a live snapshot's private mutable state, host IDs, pending receipts or mute
overlays. Static export retains deterministic frozen assets and ignores live
overlays by construction. Failure cleanup owns its exact headless reservations.
The existing snapshot privacy and one-shot ownership contracts remain intact.
No additional Rust path is authorized. ROOT0045 records this source-grounded
signature correction, not a new Riela acceptance.

### Session: 2026-10-02 — production export source release

Root releases the four declared Rust paths to the core author after its coherent
transport checkpoint. Runtime/session integration is independent; export uses
the same owned finite transport directly. Initial native/Arena finite/empty
transport checks passed; latest pressure/cutoff/chord fixtures await combined
checking. Preserve these pending obligations and the original full goal.

Use actual post-readiness activation and omit all preparation PCM from output.
Round absolute duration plus tail once, render the exact final partial block,
and then continue receipt/garbage retirement without writing more frames.
Configure real native capacities and let actual reports admit or reject the
complete song; do not silently omit families or rely on the default six buses.
Keep temporary output atomic and protect source/output aliases. The legacy
writer may be adapted, but source-library code must respect the existing
restriction on std::process outside CLI. Use a local collision-safe temporary
name without invoking subprocesses or introducing dependencies.

Report source revision, settings (BPM/cycle beats/meter/root seed), rational
duration, effective seed policy, rate, arrangement/tail/total frames, warnings
and actual final state. If actual warning authority is unavailable, report the
precise needed source contract rather than claiming warnings were preserved.

Author may write unregistered export/wav modules while integration is in
progress. Coordinate module registration and a joint source hold before checks.
No Cargo by authors; independent checker runs focused export/core tests and
necessary native/wasm/Clippy gates after coherent source. No broad old suites
are repeated without a changed behavior or unresolved concern.

### 2026-10-02 — Exact implementation declarations

WAV child: `pub(super) fn header(frames: u64, rate: u32) -> Result<[u8;44], Failure>`;
`pub(super) struct WavOutput`; `create(destination: &Path, frames: u64, rate: u32) -> Result<Self, Failure>`;
`write(&mut self, samples: &[f32]) -> Result<(), Failure>`;
`finish(self) -> Result<(), Failure>`. Collision-safe sibling creation uses
create_new plus monotonic local nonce, not process execution or truncation.
Export private: `fn alias_check(song: &PreparedSong, output: &Path) -> Result<(), Failure>`;
`fn drain(host: &mut NativeAudioHost, transport: &mut SongTransport) -> Result<(), Failure>`;
`fn render(song: PreparedSong, options: &SongExportOptions, output: &mut WavOutput) -> Result<SongExportReport, Failure>`.
Actual provider allocations are configured before pair creation; actual capacity
receipts remain admission authority. No fabricated scalar capacity reports.
Warnings and opaque pinned bank path access remain genuine metadata prerequisites,
reported to Root; no fabricated empty-warning preservation is asserted.

### Actual warning metadata contract

Root adds three exact paths above (seven Rust modules total). Export author
exclusively owns snapshot.rs and candidate tests for this change; integration
author already owns session/song.rs and alone implements diagnostic collection
there. Coordinate SongCandidate::set_warnings(Vec<String>) and immutable
SongSnapshot::warnings()->&[String], transferring the original warning list
through preparation. Constructor defaults remain empty for candidates without
warning diagnostics; real parsed/evaluated/load warnings must be retained,
not guessed empty or promoted to errors. Keep touched sources below 1000 lines.
Existing SCORE source/output alias protection includes known frozen source files;
no opaque bank-member filename contract is added to this phase.

Export private helper declarations before fixture/source extension:
`fn canonical_output(path: &Path) -> Result<PathBuf, Failure>`;
`fn pump_owner(host: &mut NativeAudioHost, side: &mut AudioSide, owner: &mut SongHostPreparation) -> Result<(), Failure>`;
`fn cleanup(host: &mut NativeAudioHost, side: &mut AudioSide, core: &mut SongTransport, duration: Ratio64, limits: SongLimits) -> Result<(), Failure>`.
Actual configured native profile uses 32 allocated buses, original native voice/
state budget and 4096 cells; actual RequestCapacity and graph stage validates fit.
This is an explicit finite profile and addressed admission failure, not unlimited
or synthetic capacity. Tests: `fractional_song_and_tail_write_exact_partial_block`,
`empty_and_silent_scores_need_no_audibility`, `repeated_export_is_bit_exact`,
`source_alias_and_failed_export_preserve_existing_output`, and
`riff_overflow_leaves_existing_output_untouched`.

Root-approved warning amendment declaration before snapshot source:
`SongCandidate::set_warnings(&mut self, warnings: Vec<String>)` crate-visible;
`SongSnapshot::warnings(&self) -> &[String]`. Candidate constructor starts empty,
then isolated session author installs actual accepted diagnostic messages. Single
ownership transfers into Snapshot without a clone. Existing candidate constructor
signature and privacy remain unchanged; integration author owns session collection.
Public genuine test `accepted_candidate_warnings_survive_preparation_and_query`.

### 2026-10-02 — Registered coherent export source

Root reports corrected core6/6 PASS, terminal0 raw
`/tmp/vactr-transport-core-pressure-repaired-001.log` SHA
`bff8d37dc511c6997835a84079cffef63d3ea8b9bb7ba2ea08e1040e08afbfdf`.
Registered export/wav under host-native/nonwasm cfg; six owned Rust paths
including snapshot warning transfer and candidate warning test are held for actual
native/wasm/strictClippy/export verification. Five public export fixtures and one
private WAV header fixture are written; none has executed yet. Session warning
capture is independently owned by integration author and must join the source
hold. No author Cargo. Full song-mode completion is not inferred from this phase.
