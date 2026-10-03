# Automatic run completion and render command implementation plan

**Status**: In Progress
**Plan ID**: SONG-13
**Plan Path**: impl-plans/active/song-mode-cli.md
**Created**: 2026-09-30
**Last Updated**: 2026-09-30
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Automatic run completion and render command supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-13",
  "planPath": "impl-plans/active/song-mode-cli.md",
  "dependsOn": [
    "SONG-12"
  ],
  "writePaths": [
    "src/cli/args.rs",
    "src/cli/run.rs",
    "src/cli/render.rs",
    "src/cli/mod.rs",
    "tests/song_cli.rs",
    "src/session/song.rs",
    "tests/song_candidate.rs",
    "examples/song-mode/generated-parts.vact",
    "examples/song-mode/README.md",
    "impl-plans/active/song-mode-cli.md"
  ],
  "sharedPaths": [
    "impl-plans/active/song-mode-cli.md"
  ],
  "sharedPathNotes": [
    {
      "path": "impl-plans/active/song-mode-cli.md",
      "intendedEdit": "Owner alone writes progress until join; SONG-16 then reconciles status and archives serially."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-12](song-mode-export-core.md)
- **Next**: [SONG-16](song-mode-reconciliation.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-12 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |

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
| `src/cli/args.rs` | Add render SCORE OUTPUT --sample-rate; reject --cycles when evaluated entrypoint is Song. | NOT_STARTED |
| `src/cli/run.rs` | Stop song documents on Ended; retain indefinite/--cycles legacy run. Propagate Failed as nonzero exit. | NOT_STARTED |
| `src/cli/render.rs` | Evaluate/freeze/prepare song in headless native mode and invoke export core; require exactly one Song entrypoint. | NOT_STARTED |
| `src/cli/mod.rs` | Dispatch render and expose existing build-session dependencies only as needed. | NOT_STARTED |
| `tests/song_cli.rs` | Argument and headless process fixtures: normal song completion, legacy compatibility, mixed entrypoints, no manual cycles and render failure. | NOT_STARTED |
| `src/session/song.rs` | Isolated optional Song-entry probe, reusing candidate evaluation without activating live effects | NOT_STARTED |
| `tests/song_candidate.rs` | No-entry/valid-entry/mixed-effects probe fixtures | NOT_STARTED |
| `examples/song-mode/generated-parts.vact` | Runnable finite composition using nested Part generators and instrument edits. | In Progress |
| `examples/song-mode/README.md` | Explain finite composition and eventual run/render usage. | In Progress |

### Public declaration contract

```rust
pub fn main(source: &Path, output: &Path, sample_rate: u32, cwd: &Path) -> i32;
pub(super) fn asset_limits() -> SongAssetLimits;
pub(super) fn preparation_limits() -> SongPreparationLimits;
pub fn probe_song_candidate(
    code: &str,
    file: &str,
    revision: u64,
    epoch: SnapshotEpoch,
    cx: &CandidateBuildCtx<'_>,
) -> Result<Option<SongCandidate>, Failure>;
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

- [ ] Keep existing CLI usage/general/evaluation exit classes; complete render succeeds only after finalized file.
- [ ] Browser export remains explicit capability diagnostic; no implicit silent host fallback for native render.
- [ ] Reject unsupported clocks and live input; preserve original legacy exporter/example source.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_cli)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |

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

### Session: 2026-10-02 — finite generator example

Created examples/song-mode/generated-parts.vact and its README. The example
uses nested zero-argument Part generators, an immutable bass-drum-only filter
edit, two intro repeats and four developed repeats (24 cycles, 48 seconds plus
a two-second tail). This language shape is covered by existing native/candidate
fixtures; the new example has not yet been executed through the CLI. Final CLI
acceptance must run the file without --cycles and check its automatic endpoint.
The core native/Arena repeat-to-changed-Part retirement tests have passed;
Runtime/session integration is currently being implemented.

### 2026-10-03 — isolated CLI entry classification

The first real session playback and streaming export fixtures passed. CLI must
identify Song entry points in isolated staged evaluation, before any active
namespace evaluation or SlotBind submission. Never suppress an unavailable
PlaySong error after executing live code. The optional probe returns no entry
for valid legacy code, or the original candidate for a Song. Defer staged-effect
admission until entry classification for the probe, then reject mixed Song/live
effects before submission. Existing strict candidate API remains strict.
Probe errors must retain genuine diagnostics; no source-text/string-error
heuristics. Seven Rust modules are released. CLI author exclusively owns
session/song.rs and tests/song_candidate.rs after the integration cancellation
hold releases; Runtime publication author does not edit those paths.
Render owns the explicit bounded asset/preparation helper declarations above.
Use self-contained builtin drum instruments for an executable example rather
than requiring undeclared external sample banks; retain generators, selective
instrument edits, repeats and exact duration.

### Session: 2026-10-03 — actual CLI source implementation

Export's public five fixtures and private WAV header fixture passed. Original
warning fixture27747 passed; the unused export import was removed only after
all focused processes were terminal. CLI source work is released; no author
Cargo runs. Registered CLI integration awaits an isolated NoSong/Candidate
classification API so rejected mixed documents never touch the active namespace.

Additional exact private declarations before implementation:
```rust
fn parse_render(args: &[String]) -> Result<Command, UsageError>;
pub(super) fn asset_limits() -> crate::song::assets::SongAssetLimits;
pub(super) fn preparation_limits() -> crate::host::caps::SongPreparationLimits;
fn run_song(session: &mut Session, clock: &HostClock, out: &mut dyn Write) -> i32;
```
`Command::Render` retains source/output PathBuf and sample_rate:u32. Render
uses original isolated evaluator + NativeSampleLoader factory and consuming
export. Asset ceilings are explicit construction bounds; actual host admission
still consumes measured capacity reports. Run must classify before active eval,
reject manual cycles and unsupported virtual/input clocks for Song, and consume
original PreparedSong. Legacy run fallback is allowed only after typed NoSong.
No CLI behavior has been tested yet.

Private evaluator declaration refinement before source:
```rust
fn evaluate_inner(code: &str, file: &str, revision: u64, edit_epoch: u64,
    epoch: SnapshotEpoch, cx: &CandidateBuildCtx<'_>, probe: bool)
    -> Result<Option<SongCandidate>, Failure>;
```
Strict evaluate unwraps the typed optional candidate with the existing missing
entry diagnostic. Probe defers effect admission until all forms have evaluated
in the isolated sink; no-entry returns None, mixed or multiple entries refuse.

Exact fixture declarations before implementation:
```rust
fn isolated_probe_classifies_legacy_and_returns_original_song();
fn isolated_probe_rejects_mixed_effects_multiple_entries_and_real_errors();
fn isolated_probe_preserves_unresolved_legacy_graph_declarations();
fn render_arguments_validate_native_rate_and_complete_paths();
fn actual_render_writes_complete_fractional_song_and_empty_song();
fn render_failures_preserve_existing_output_and_reject_mixed_entrypoints();
fn finite_run_rejects_noop_and_manual_cycles_while_legacy_cycles_remain();
fn generated_parts_file_renders_exact_fifty_seconds_without_cycles();
```
CLI process fixtures invoke the built binary with real files and inspect actual
exit/PCM/report. Generated-parts witness uses owned repository example and builtin
instruments; no fabricated asset or ACK. Positive live device playback cannot be
claimed from headless export evidence. All command/test execution stays with checker.

### Session: 2026-10-03 — coherent CLI source checkpoint

All seven declared Rust paths are implemented and scoped formatted. Optional
probe runs original isolated evaluation and returns None before shape/freeze for
legacy code; Song effects are admitted only after exact single entry discovery.
ClosedSong numeric/dynamic inputs are merely recorded at lowering, so the feared
premature legacy resource rejection was disproven by source inspection; a real
legacy convolution fixture is written. No active evaluation detects Song.

Render invokes original candidate preparation and verified production exporter.
Run transfers the original probed candidate to consuming host owner; finite
termination uses actual Runtime state. Runtime author must publish failed
preparation as Failed while retaining cleanup (owned outside this plan).

Five real CLI process tests and three probe tests are written, including an
actual repository generated-parts command with exact 400000-frame 8000-Hz WAV
expectation (50 seconds) and nonzero PCM. Builtin instruments retain the original
generator/selective edit/repeat/duration shape. No CLI/probe tests have executed;
positive live-device run remains separate from headless command evidence.
Independent checker will run after all concurrent source writers hold.

### Session: 2026-10-03 — actual first CLI fixture failures

Independent CLI binary original46050 terminal101 ran five fixtures: three passed,
two failed. Generated score referenced digital template names as default-kit
keywords, which are not in the default kit. Explicit named kick/snare/hat
instrument declarations now wrap genuine digital percussion core UGens and the
same immutable selection targets kick. Exact 24cycle/50second/nonzero PCM oracle
remains unchanged. Legacy fixture's `play` was undefined; actual supported `d1`
slot binding replaces that invalid input in both CLI and probe fixtures.
Source repair is fixture-only; no production aliases or admission relaxation.
All focused checker processes are terminal. No author Cargo ran; repaired source
is held for independent verification. Positive native device playback remains
unexecuted; headless complete export is an actual tested boundary.

### Session: 2026-10-03 — real CLI and isolated probe accepted

Independent repaired CLI original74596 terminal0 executed all five process
fixtures; `/tmp/vactr-song-cli-repaired-001.log` records five PASS. The actual
command `vactr render examples/song-mode/generated-parts.vact OUTPUT.wav
--sample-rate 8000` wrote 400000frames of stereo PCM16 (50seconds), nonzero PCM,
and completed Ended. Fractional/empty outputs, alias/mixed-error preservation,
manualcycle/noop refusal and real legacy d1 --cycles compatibility passed.

Probe original96853 terminal0 ran the three new isolated fixtures, all PASS
(16filtered): `/tmp/vactr-song-probe-repaired-001.log`. These prove typed NoSong
legacy handling including unresolved numeric IR declaration, original candidate
identity, and mixed/duplicate/error rejection before active evaluation.

Export/CLI focused behavior is verified, but full phase check/Clippy/portable
matrix and positive physical-device finite run remain unexecuted for this wave.
Browser setup/Apply/fade/mute and broader original song scope remain incomplete.
No author Cargo commands ran; all seven Rust sources stay held.

### 2026-10-03 — Generated Part editing usage

Expanded the existing example README with a nested Part argument/return example,
opaque handle deletion and region overwrite syntax, plus the implemented editing
API table. Syntax follows the already existing nested native fixture; no new
runtime or output artifact was introduced. This documentation change does not
close the remaining verification criteria above.

### 2026-10-03 — Original failure renderer declaration before repair

Broad legacy CLI test102 exposed isolated probe printing custom vactr:type:
without established error[code] prefix, while preserving correct exit3.
Conditional source release now satisfied: original83101 wholelib terminal0,
2016 passed/two ignored and all old918 hashes unchanged. Only src/cli/run.rs
probe Err branch will use session::console::format_failure(&error), preserving
original FailCode/message/origin and return3; Ok(None) active legacy entry and
all other branches remain unchanged. No diagnostic code fabrication, unsafe
fallback, test weakening or new path. Independent cli/song_cli/lint pending.
