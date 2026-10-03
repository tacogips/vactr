# Serial integration evidence and documentation implementation plan

**Status**: In Progress
**Plan ID**: SONG-16
**Plan Path**: impl-plans/active/song-mode-reconciliation.md
**Created**: 2026-09-30
**Last Updated**: 2026-10-03
**Session target**: 1–3 sessions
**Design Reference**: [Accepted song-mode design](../../design-docs/specs/design-song-mode.md#plan-author-handoff)

## Intent and repository context

User intent: reusable generated multi-track parts can be copied, selectively edited, repeated finitely and sequenced into a complete automatically terminating song; live users can mute instruments and apply whole code. Serial integration evidence and documentation supplies the corresponding accepted boundary.
Source baseline: design-song-mode.md capability audit distinguishes existing closure/list/pattern composition, shared bus/orbit effects, per-form eval and manually bounded native rendering from the missing finite song APIs. Use current dirty working-tree behavior, not an assumed clean main baseline.

## Manifest

```json
{
  "planId": "SONG-16",
  "planPath": "impl-plans/active/song-mode-reconciliation.md",
  "dependsOn": [
    "SONG-13",
    "SONG-15"
  ],
  "writePaths": [
    "tests/song_end_to_end.rs",
    "examples/song-mode.vact",
    "design-docs/specs/command.md",
    "design-docs/specs/design-song-mode.md",
    "impl-plans/README.md",
    "impl-plans/active/song-mode-reconciliation.md",
    "impl-plans/active/song-mode-values.md",
    "impl-plans/completed/song-mode-values.md",
    "impl-plans/active/song-mode-value-integration.md",
    "impl-plans/completed/song-mode-value-integration.md",
    "impl-plans/active/song-mode-checker.md",
    "impl-plans/completed/song-mode-checker.md",
    "impl-plans/active/song-mode-query-edits.md",
    "impl-plans/completed/song-mode-query-edits.md",
    "impl-plans/active/song-mode-natives.md",
    "impl-plans/completed/song-mode-natives.md",
    "impl-plans/active/song-mode-snapshot-contracts.md",
    "impl-plans/completed/song-mode-snapshot-contracts.md",
    "impl-plans/active/song-mode-candidate-evaluation.md",
    "impl-plans/completed/song-mode-candidate-evaluation.md",
    "impl-plans/active/song-mode-audio-contracts.md",
    "impl-plans/completed/song-mode-audio-contracts.md",
    "impl-plans/active/song-mode-dsp-routing.md",
    "impl-plans/completed/song-mode-dsp-routing.md",
    "impl-plans/active/song-mode-host-adapters.md",
    "impl-plans/completed/song-mode-host-adapters.md",
    "impl-plans/active/song-mode-transport.md",
    "impl-plans/completed/song-mode-transport.md",
    "impl-plans/active/song-mode-export-core.md",
    "impl-plans/completed/song-mode-export-core.md",
    "impl-plans/active/song-mode-cli.md",
    "impl-plans/completed/song-mode-cli.md",
    "impl-plans/active/song-mode-browser-session.md",
    "impl-plans/completed/song-mode-browser-session.md",
    "impl-plans/active/song-mode-editor-controls.md",
    "impl-plans/completed/song-mode-editor-controls.md",
    "impl-plans/completed/song-mode-reconciliation.md"
  ],
  "sharedPaths": [
    "impl-plans/active/song-mode-values.md",
    "impl-plans/active/song-mode-value-integration.md",
    "impl-plans/active/song-mode-checker.md",
    "impl-plans/active/song-mode-query-edits.md",
    "impl-plans/active/song-mode-natives.md",
    "impl-plans/active/song-mode-snapshot-contracts.md",
    "impl-plans/active/song-mode-candidate-evaluation.md",
    "impl-plans/active/song-mode-audio-contracts.md",
    "impl-plans/active/song-mode-dsp-routing.md",
    "impl-plans/active/song-mode-host-adapters.md",
    "impl-plans/active/song-mode-transport.md",
    "impl-plans/active/song-mode-export-core.md",
    "impl-plans/active/song-mode-cli.md",
    "impl-plans/active/song-mode-browser-session.md",
    "impl-plans/active/song-mode-editor-controls.md"
  ],
  "sharedPathNotes": [
    {
      "path": "impl-plans/active/song-mode-values.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-value-integration.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-checker.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-query-edits.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-natives.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-snapshot-contracts.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-candidate-evaluation.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-audio-contracts.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-dsp-routing.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-host-adapters.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-transport.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-export-core.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-cli.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-browser-session.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    },
    {
      "path": "impl-plans/active/song-mode-editor-controls.md",
      "intendedEdit": "Serial status reconciliation and archive only after owning worker joins; preserve its progress log."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-13](song-mode-cli.md), [SONG-15](song-mode-editor-controls.md)
- **Next**: Serial completion

| Dependency | Required output | Status |
|---|---|---|
| SONG-13 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |
| SONG-15 | Reviewed declarations, passing phase checks and recorded post-edit hashes | NOT_STARTED |

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
| `tests/song_end_to_end.rs` | Exercise real nested generator/edit/sequence, native render and transport fault scenarios using common public entrypoints. | NOT_STARTED |
| `examples/song-mode.vact` | Add accepted 24-cycle example plus targeted edits and declared bd-private chain using only implemented proposed natives. | NOT_STARTED |
| `design-docs/specs/command.md` | Document finite run endpoint, render flags/exit behavior and new session messages after implementation. | NOT_STARTED |
| `design-docs/specs/design-song-mode.md` | Record implemented behavior and verification evidence; keep accepted decisions unless review authorizes change. | NOT_STARTED |
| `impl-plans/README.md` | Serially reconcile song entries/statuses; preserve unrelated existing index edits. | NOT_STARTED |

### Public declaration contract

```rust
pub struct SongIntegrationEvidence { pub arrangement_frames: u64, pub tail_frames: u64, pub native_browser_event_digest: String }
```

## Tasks

### TASK-001: Baseline and contract integration
**Status**: In Progress
**Parallelizable**: No; acquire dependencies and fresh hashes first.
**Deliverables**: Manifest, immutable intent snapshot and the declaration/type integration listed above.
- [ ] Read accepted section and prerequisites; record exact ownership and imports.
- [ ] Add declarations without changing legacy semantics; review manifest-required exhaustive consumers.

### TASK-002: Implement the owned behavior
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this plan; cross-plan parallelism follows the DAG and ownership manifest.
**Deliverables**: Every non-test file in the module table, with exactly its stated intended change.
- [ ] Implement the declared behavior and all phase-specific criteria below.
- [ ] Record post-edit hashes and run required modify-agent checks.

### TASK-003: Behavioral evidence and progress
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No; verifies the complete phase.
**Deliverables**: Every listed test/fixture file, full command logs and this plan progress record.
- [ ] Add the specified success, boundary, compatibility and failure fixtures.
- [ ] Run future commands below; record actual exit status and complete output, with no empty selected-test run accepted.
- [ ] Reconcile final hashes with intent; update completion criteria and progress without editing other worker logs.

## Phase acceptance criteria

- [ ] Recheck all joined outputs against immutable intent snapshots and hashes, repair drift serially, then rerun only impacted tests.
- [ ] End-to-end fixture asserts single-note/sibling preservation, automatic finite completion, native frame counts and browser command/event parity.
- [ ] No lockfile/dependency changes required. Broad formatting only serially and only within authorized modified files; record any unavoidable existing-file split.
- [ ] Global archiving occurs only after complete verification; keep concrete completed plan destinations recorded in this plan. No concurrent Git operation.

## Future verification — not executed during planning

| Command | Required evidence |
|---|---|
| `CARGO_TERM_QUIET=true mise run check` | Rust integration compiles; no undeclared implicit coercion/exhaustive-match gap. |
| `CARGO_TERM_QUIET=true mise run clippy` | All-target warnings gate passes for joined workspace. Existing unrelated failures must be recorded, not silently changed or treated as a pass. |
| `CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm` | Portable song core and browser adapter compile without native-only dependencies. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise exec -- cargo nextest run -E 'binary(song_end_to_end)'` | All phase fixtures execute and pass; selected count must be nonzero for every listed test binary. |
| `CARGO_TERM_QUIET=true mise run fmt-check` | Serial formatting reconciliation covers only authorized edits. |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise run test` | Full joined Rust suite passes, with complete foreground log. |
| `mise exec -- npm --prefix editor run test` | Full joined editor suite passes. |
| `CARGO_TERM_QUIET=true mise exec -- cargo run -- run examples/song-mode.vact --host noop` | Ends automatically after declared arrangement/tail, with no --cycles. |
| `CARGO_TERM_QUIET=true mise exec -- cargo run -- render examples/song-mode.vact tmp/song-mode-riela/song.wav --sample-rate 48000` | Native headless complete-song WAV and metadata match 24 cycles at 120 BPM plus eight-second tail: 2,304,000 arrangement frames + 384,000 tail frames. |

### Serial completion ownership

Only after all phase evidence passes, reconcile impl-plans/README.md and move each listed song-mode plan from impl-plans/active/ to the same basename under impl-plans/completed/. Each destination is explicitly recorded in the execution manifest before moving. No other plan may edit the index or perform archiving. These moves are deferred completion work, not actions in this planning node.

## Completion criteria

- [ ] All module-table changes and phase-specific criteria complete.
- [ ] Tests listed above execute with nonzero fixture count and pass; check/typecheck/build gates pass.
- [ ] All required command exit statuses and complete log paths recorded; no running foreground sessions remain.
- [ ] Legacy behavior preserved; pre-existing changes retained and cross-worker hashes reconciled.
- [ ] Progress status updated; archive/index changes deferred to SONG-16.

## Progress Log

Earlier checkpoints are preserved in the [reconciliation history](../../design-docs/references/song-mode/reconciliation-checkpoints-20261003.md), including failures, terminal process receipts and acceptance limitations. The full completion criteria above remain active.

Sampling/invocation checkpoints are preserved verbatim in the
[second checkpoint archive](../../design-docs/references/song-mode/reconciliation-invocation-checkpoints-20261003.md),
including the unresolved historical extraction-equivalence limitation.

Lookup checkpoints are preserved verbatim in the
[lookup checkpoint archive](../../design-docs/references/song-mode/reconciliation-lookup-checkpoints-20261003.md),
including the measured default-budget limitation and every failed check.

Geometry checkpoints are preserved verbatim in the
[geometry checkpoint archive](../../design-docs/references/song-mode/reconciliation-geometry-checkpoints-20261003.md),
including all failed fixtures, lint repair and scoped frontend acceptance.

### Query authority checkpoint archive

The421-line query-authority checkpoint body is preserved verbatim in
[the query checkpoint archive](../../design-docs/references/song-mode/reconciliation-query-checkpoints-20261003.md),
including all failed runs and the overwritten historical focused-log limitation.
Original body SHA256: `57c533754d76dd5af4ed9377f30de35957be9d6428d3c835c5e972b1a58fd97f`.
The accepted joined checkpoint establishes405 Rust tests plus590 frontend tests
and build; current frozen/membership work continues below.

### 2026-10-03 — Frozen handoff advances; route prerequisite remains separate

Author reports frozen core and seven genuine private fixtures plus one public
compatibility fixture compiling. Compile00127841/6de3c8/0,
00216224/9a24d2/101 (public Ratio64 constructor),
0033346/0e45ee/0 and00438146/6a6ff3/0 are terminal. Runtime evidence
for this phase is pending; these compile results do not establish acceptance.
The shared collector survives query, authentication, copying and failure
writeback. Nonallocating comparison charges replace discarded deep copies.

Read-only route review requires builder.rs ownership to remove the issued path's
hardcoded preparation budget. The eight-path route draft trades snapshot.rs for
builder.rs. Exact retained execution membership needs a separate declared
two-path provenance prerequisite, now indexed in the plan README. It remains
Planning and releases no source edits during frozen5. Actual routing, scheduler
consumption, pre-Reserve admission and permitted varying-seed executions remain
required; accepting retained cache hits alone cannot complete song mode.

### 2026-10-03 — Frozen held0001 enters focused execution

Root independently checks all five file hashes/strict line caps and all940
current input hashes against held0001. Baseline938 differs only in snapshot.rs,
source.rs and origin.rs, plus the declared issued.rs/public-test additions.
Source receipt SHA256
2f9789587d415f8c3ccdd572fc29145296b9930f44c1aa04862495855154cd3a;
full940 receipt SHA256
b16c7d0942a761a547604902653c45cf4abcc1004882992875422389747f2f7a.
Author compile00519635/65af8c/0 and scoped format are terminal. Root releases
independent focused seven owning fixtures and one public compatibility fixture.
Checker original19497/ca25f7 is live, with distinct focused001 private/public
logs; no broad gate or behavioral acceptance yet. Source remains held.

Author read-only confirms the separate retained-membership2 API and genuine
owning fixtures fit the declared scope; that prerequisite remains Planning
until the frozen gate is terminal and reviewed.

### 2026-10-03 — Frozen focused0001 finds nested preservation rejection

Checker original19497/d5184d terminates101: six of seven private fixtures pass;
cached_nested_chord_retains_every_inherited_frame_and_sealed_member_without_reads
fails at issued.rs386 with Type foreign augmented source contribution. Public
compatibility and broad checks are not run. All940 inputs remain unchanged
after terminal; root independently verifies that cohort before repair release.
Raw log SHA256
b7cb57c43fc9c0b0f83239fdda792b9702ac9e523b4beca3d961849883cb6607;
final receipt SHA256
05dc1484f6366813bef9615eb0734a615e4dcfe156b56fedfcb6944dd7c27ea5.

Full five pre-repair source texts are saved in
tmp/song-mode-riela/frozen-issued-before-repair-originals-0001.json, SHA256
51e04caad434b94f42ab8bf1c68306f0b8764591b01ccbf90319b2e739305f7e.
Root releases repair within the same five paths only, requiring the exact
actual nested mismatch to be identified without relaxing proof identity or
dropping any origin field/timing-prefix witness. No behavioral acceptance is
claimed. Retained membership and route source release remain pending.

### 2026-10-03 — Derived timing depth identified in nested source review

Root inspects copy_origin: authority_depth is computed from the complete chain's
frame count, timing count and maximum producer depth, then stamped into every
copied FrozenSliceTiming. Adding an outer timing can therefore change the
derived admission_depth on inherited existing timings, even when those live
inherited origin pointers are unchanged. Frozen timing PartialEq includes that
field; inherited full equality or naive frozen timing-prefix equality can
reject a legitimate nested append. This source fact supplies a concrete repair
candidate, not yet behavioral proof of the failed fixture's exact mismatch.

Repair must preserve exact musical timing fields, all source-frame fields,
chain length/order and authentic live bag identities; retain both genuine
copied admission values and their actual budget/depth checks. It must not
substitute descriptor equality for original proof authority or silently discard
depth. Author remains within frozen5 and source<1000; all gates require rerun.

### 2026-10-03 — Frozen repaired0002 enters focused verification

Author compile00681661/4173fb/0 and00744671/92380b/0 are terminal; scoped
five-file formatting passes. Root independently verifies every held940 hash,
five file hashes and strict line caps905/777/972/375/31. Relative to frozen0001,
only the declared source.rs, origin.rs and issued.rs repair files changed.
Source receipt SHA256
e387e0aa1564d6e82bd0116e5ca0331f3ee13eead684d20f95d8e78df6924adb;
full940 receipt SHA256
97237914dce5eb48f27510dc6c43ba0ad7795d16aa9d7bdd559b407bccdeb6b0.

Private timing preservation now compares all original musical timing fields,
requires the independently admitted augmented depth to be at least raw depth,
and retains both actual copied certificates. Original public equality remains
unchanged. Charged live-chain checks authenticate corresponding original bags,
route/commit fields, chain length and inherited depth. The genuine nested
fixture requires and prints an actual inherited certificate change before
freezing. Root releases the same seven private plus one public focused tests;
behavioral diagnosis and acceptance remain pending, with failed001 preserved.

### 2026-10-03 — Remaining structural clock work bounded

Independent source audit identifies actual behavior of all eight Unknown hooks.
Root records a six-path Planning structural-clock-hooks phase: extract dispatch
from the994-line parent, instrument Euclid's actual timing-event sample, then
prove unchanged child clocks and authentic generated copy behavior for six
other operators. Chunk needs a separate output-anchor applicability proof.
No Rust release occurs during frozen repair; the new plan is indexed with
resolving local references. Temporary barriers remain outstanding full-goal work.

### 2026-10-03 — Frozen focused0002 passes all eight genuine fixtures

Checker original36723/10573d is authoritative terminal0, all handles terminal.
Seven private fixtures and one public compatibility fixture pass with exact
actual names. The nested source diagnostic captures four genuine inherited
augmented/raw depth certificates7/6 across chord tones0/1, confirming the
derived whole-chain stamp mismatch. Original fields and timing identity checks
execute before freezing; callbacks remain denied after retention. Root inspects
both raw logs and independently reverifies every940 current input hash.

Private raw SHA256
91984ab18a610a254a3659074b8b206a5c562278e49d1833d241ce567965d366;
public raw SHA256
f90cd22d011f12531e6210da619411ace39404846d63cf7e2a8c279d9edb4172;
final focused receipt SHA256
18f2f3a236eb69ea6d4e770f74d86a698b90c9a652c9400b17afcc33565a5210.
Failed001 evidence and five original pre-repair texts remain preserved.

Root releases fresh broad002 on the same held940 cohort: preceding405 plus
eight new distinct fixtures, EndToEnd coverage, native, strict all-target lint,
fresh WASM and scoped five-file format. Separate broad-prefixed logs prevent
overwriting focused evidence. Source stays held; full acceptance requires all
terminal gates and root frontend tests/build on that exact fresh WASM. Following
retained membership, route/resolver/playback, admission/vary and structural
clock support remain required full-goal work.

### 2026-10-03 — Frozen broad and frontend acceptance; membership2 released

Fresh broad002 passes413 distinct actual Rust names:52 occupancy,7 clock,
9 sampling,217 affected and128 public, including EndToEnd2. Native, strict
all-target Clippy, fresh pure WASM and scoped five-file format pass with all940
inputs unchanged after each gate. Original62164/f5a524/0 is terminal with no
live checker handles. Final broad receipt SHA256
cbd6667ec5647d4a39fad8f6b416edb176818ff3aa111acc8cd650d7b9388ede.

Root independently verifies full940 plus exact fresh top/deps WASM, then runs
frontend tests19516/bc787a/0 (590 tests,78 files) and build72991/6374a5/0.
Both root handles are terminal. Post-verification full940 still matches and
top/deps/dist WASM are155284193 bytes with SHA256
9b3f471d68b9f29f1c6032024b6ac2595782844929123dad6b15175076610a07.
Root frontend receipt SHA256
cd8cef3de20b34e2bbc0b7007da65079cfd7af06970c3097032149b7d79ec4f4.
This accepts the frozen handoff source/tests, not its still-missing production
route/scheduler consumers or default admission.

Root marks retained-execution-membership Ready and releases its exact two
paths after saving all original texts/null-new child. Ready plan SHA256
303faa0d86813b9cd564cbc5b5797db1133fcf4ae9a084dbc182a1603e1d37f8;
intent SHA256
b6e31474cf53a6d8b563af859275285ccaa7c28d2681db865cd2d4610d3a6d07.
Expected next cohort941 differs only in provenance parent plus new owning test
child. The narrow check certifies shared genuine raw execution, not site or
fresh placement/seed/entry/clock policy; downstream trusted lookup must consume
those actual issued values. Author owns implementation and compile-only handles;
independent runtime gates follow. No other Rust/dependency/Git edits released.

### 2026-10-03 — Membership source held0001; explicit route adapter refined

Author compile00110868/f136ec/101 exposed private remaining-field access in
fixtures;00215427/fb8176/0 and00345057/9d5e65/0 pass after the accessor fix
and strengthened full descriptor comparisons. All author handles and scoped
two-file formatting are terminal. Root independently verifies full941 and
parent978/child416 hashes: only provenance parent changes and the declared
retained_tests child is added relative to accepted frozen940.
Source receipt SHA256
d41f7c37310480c82fb109dbbe2ac902ed07431716e4910cbe54ecbe69daa22f;
full941 receipt SHA256
2e9abcbbd293fabb25153cb1dbaffa8d6da375eb3169c432ab0e55aa3e00b146.
Root releases four actual focused fixtures; checker45342/3cc100 is live.
No runtime acceptance or broad release yet. Source remains held.

Read-only reviewer supplies a concrete bind_issued_owner(site, request,
transcript, seal, work, depth) entry in the declared lookup authority child.
It binds genuine retained execution membership and trusted request attachment,
then constructs its address with the actual fresh invocation/clock/observations.
Existing getters suffice; no ninth provenance/replay path is needed. Root
updates the route draft and requests author feasibility without source edits.

### 2026-10-03 — Membership focused0001 finds fixture constructor-cost omission

Checker45342/9a0507 terminates101: three genuine fixtures pass; exact-work
fixture fails at retained_tests.rs376 before its later budget/depth assertions.
Measured total15 versus expected14 includes CanonicalIndexCollector::new's
actual one-unit constructor charge. This establishes a fixture expectation
defect, not a production membership defect. All941 inputs remain unchanged
after terminal and root reverifies them. Raw log SHA256
30f0a88f4032022d2da4c12e1cbc7da8eef966b69cc89fcddd530fcd7b8119cb;
final receipt SHA256
5fd13fa3e907cd47496e510a191ba18df90f0805fb3d99f3c080c9e58727ff8c.
Full two original texts are preserved with SHA256
7e1af8cdb461987f82e7952b29b8f602b9fc8976de2f66480d2318311d911474.
Root releases same-two-path repair: include genuine constructor work in total
expectation and compare deep failure against post-construction remaining. Keep
actual exact/one-less and boundary checks; independent execution must rerun.

Author route feasibility also identifies two genuine preparation seams outside
route8: nested certification creates default depth and restarts roots at1,
while source-use certification exposes consumed work only on success. A
separate bounded metering prerequisite must propagate actual caller depth and
write back spent work on failure. Route8 remains Planning; no hidden ninth or
tenth source path and no duplicated alternate certification algorithm released.

### 2026-10-03 — Membership focused0002 passes; fresh broad417 running

Repair changes only two accounting assertions in the declared fixture child;
parent API remains identical to held0001. Compile00473077/f0bd45/0 and scoped
format are terminal. Root verifies all941 hashes and the sole child delta.
Held0002 full941 SHA256
8f435e4cb8d6e1d5a6f761c405bfac93580b3771fbcc038c4a1a61e442e41b9c.
Checker91390/ca9a30/0 runs all four actual tests successfully, including the
previously unreached exact/one-less and depth checks; all handles terminal and
post941 unchanged. Raw SHA256
8b466cd9b6caf4b5082ddd95b68f37b706d65fd6977b91517ce3bd27005ea78c;
final focused receipt SHA256
7c70e7077bc46ce24370ec9654df781a94dc2c03ddfaf5873eadf1040821cc3b.
Root independently rechecks941 and releases fresh broad002: preceding413 plus
four new distinct fixtures, native/strict/freshWASM/scoped2format and full941
gate checks. Original65019/d97404 is live. Source remains held; fresh frontend
evidence follows only after every checker handle is terminal.

Root records the separate five-path route-preparation-meter Planning draft
with cohesive certification and nested traversal extractions. Its candidate
owning test child sits under nested preparation to preserve private access
without adding routing.rs. Author confirmation remains pending; no Rust release.

### 2026-10-03 — Membership broad0002 passes417; test-only lint repair released

Fresh417 distinct Rust tests and native pass, including EndToEnd2. Strict
all-target Clippy fails on needless_borrow at retained_tests.rs263; WASM and
format gates do not run downstream. Original65019/c3e5d1 is terminal101 with
no live checker handles. All941 inputs remain unchanged after terminal and
root independently reverifies them. Complete lint log SHA256
525495b255ac6e73354798d6c1a2dadb59419b92e116dd6732d06c20a63113cc;
final broad receipt SHA256
0d647b3e8fcd0cc8c7bfa01ceed40254896c9441ed6ec9e3aba25d8f2347425a.
Full two pre-lint-repair texts are saved with SHA256
d36cf1cfc6e85bd74efff05dd35d93f1bc1a06664e06884ffce7f0b41ca38164.
Root releases removal of that unnecessary fixture borrow within the same two
paths, preserving authentic Rc distinction and all owning assertions. Fresh
source/compile/format receipts and independent gates must follow; no overall
membership acceptance or routing source release is claimed.

Root archives the preceding421-line query checkpoint body verbatim to the
indexed query checkpoint reference. Body SHA256
57c533754d76dd5af4ed9377f30de35957be9d6428d3c835c5e972b1a58fd97f;
all failed checks, accepted joined receipts and the historical overwritten-log
limitation remain intact. This keeps the active plan below its1000-line limit.

### 2026-10-03 — Membership lint-only held0003 passes focused retry

Author compile00583679/fa9e44/0 and scoped two-file format are terminal.
Root verifies all941 inputs and the sole fixture-child delta versus0002;
production membership code is unchanged. Held003 source SHA256
a01b20b46c08e7f83676fd10a00e3387bad598f5833219783fb0c712d34458fd;
full941 SHA256
f9a47dfa754d5a2baa84b470bfc6799f3c76f482439bbb66b7c1d4436d463bd0.
Fresh focused003 passes all four actual fixtures,52370/95b924/0 terminal,
post941 unchanged. Raw SHA256
3083a86e643f46bc5d33f979fcf09cad686ee8d7160d453cb16f5b39477346fd;
final focused receipt SHA256
8cb2d6febb1185993033320dfe245f14c9cf1a21478d4bbf41effa3f0a8db00c.
Root's conditional release starts fresh broad003 only after that focused
terminal/postseal. Original70458/68111b is live; source remains held.

Both read-only metering reviews confirm five paths with nested-owned fixtures.
Keep thin Certification/CoverCacheKey in the original parent and make moved
admit/enter/node methods parent-visible for its untouched source_free sibling.
Nested metering must retain the original supplied SongLimits: ResolutionBudget
limits() reconstructs other fields from defaults. Bridge only remaining and
actual current depth, without also subtracting depth from max_depth. These
requirements are added to the Planning precursor; no concurrent source release.

### 2026-10-03 — Membership0003 accepted; preparation-meter5 released

Fresh broad003 passes417 distinct actual names, native, strict all-target
Clippy, fresh WASM and scoped two-file format. All941 inputs match after every
gate and terminal70458/a527c4/0, with no live checker handles. Final receipt
SHA2563d6f3e25a8825f68ca228b315704e354177085244e582eef1d6a3beba6308a99.
Root independently verifies941 and fresh top/deps WASM, then runs frontend
tests8637/57eb43/0 (590 tests,78 files) and build78575/4067a4/0. Both handles
are terminal; post941 and top/deps/dist artifacts match. Fresh WASM155284194
bytes SHA256e4a5b54eb8ab375a61d67547ef099aae075e390a9799694d26378f1be2513fed.
Root frontend receipt SHA256
6b82a04b27f0f96e23e3e0adc18ec0dc0dcaba247912cb0ba62012c94e53335a.
This accepts the narrow membership source/tests; actual routing adoption is
still required, and no site/placement/clock authority is inferred from its boolean.

Root marks route-preparation-meter Ready and releases exactly five paths after
saving two complete original parents and three null-new child entries. Ready
plan SHA256f6bc47c8cdf0eb53e55c3656ddd46ac652d90c010a8da477d0accd5b7ddf0344;
source intent SHA256
b6a0f717ed009dfb5fb75abef3c42c58e486bb3ba792e70c5d638729c82ab5a5.
Expected next cohort944 differs only in source_uses/nested parents and declared
certification/preparation/fixture children. Preserve original complete policy,
caller remaining, real inherited depth, all error debits and legacy wrappers;
no success-only debit or duplicated independent algorithm. Following Route8
must actually consume these primitives. All prior failures remain preserved.

Query source/task gates are verified, but its explicit downstream-consumer
completion criterion remains unfulfilled; do not archive that overall plan yet.
Its named bank fixture also needs concrete owning evidence beyond analog-only
query8, included in the next handoff scope. Compatibility may archive after its
own complete criteria. Actual route-view/resolver/scheduler, pre-Reserve
admission, useful varying seeds and remaining clock hooks are still mandatory.
Full song-mode goal stays active; this is implementation release, not completion.

### 2026-10-03 — Meter0001 focused pass; review requires compatibility repair

Root independently verified held five-path hashes/caps and all944 cohort entries.
Source held0001 SHA171b347e0933ee778109072353a833f950c887d284ec546cdeacae452e1fb1fd;
full cohort SHA8a6ed8b87eceef2750f7b9d51ea5cdfb8bb8af3d1590e2191985b2739463272d.
Independent focused001 original30087 terminated0 with four actual tests passed
and all944 unchanged. Final receipt SHA9078aaa7699b7161e6ce12a8eaaf660c45f8d959bbd148f90041d66bce290a5e.
This is narrow fixture evidence, not phase acceptance or consumer integration.

Read-only review found the new entry calls Certification::enter before node,
which also enters and charges one unit. Original extraction had no entry call.
Both new wrapper-comparison paths share the changed engine and therefore mask
the historical consumed_work regression. Root inspected the actual held body
returning self.admit(1); the author's initial noncharging recollection was
incorrect and subsequently corrected. No broad gates released.

After all checker handles were terminal, root preserved complete originals of
all5 in route-preparation-meter-before-review-repair-originals-0001.json
SHA281abcfa0e84cae2bd66a456594a3324da939ae2025a215a5ea20ef48bc608f7
and released the same exact5 for repair. Required evidence: historical cost,
smallest sufficient and one-less depth with nonzero inherited start, explicit
noncharging empty nested depth validation, and genuine nested child-certification
failure preserving its spent work. Held0002 and independent verification remain
pending. Route8 Planning now records audited precharges for detector/graph helper
work and topology copying within its declared eight paths. Full goal stays open.

### 2026-10-03 — Meter0002 repair verified; exact failure seam witness released

Root verified held0002 SHA92bf4551725c6ed0f1711fb52eedeb6d4b3452d3fd8dfacb7a66c35191b590fb
and all944 SHAe6fbdee249b482ffaaa1f200d76f098ed0ab84c0e9294ad07186da9d1932637a.
Only four previously declared paths changed; every touched file remains below1000.
Root historical-reference audit SHA9881c68aff264b0b01d0692d766a1228d60a4c7e2ad59b528df45167eb91e399
confirms saved extraction entry equality after function-name/visibility and
whitespace normalization. Shared Certification methods limit that evidence.

Focused002 original32392 terminated0, eight actual fixtures passed, all944
unchanged. Final receipt SHA8b6fb0d1fc740819cd84d0661d704f655bea21e373dc958487af36b80631bf20.
Independent source review confirms production duplicate debit fixed and new
minimum/nonzero/empty depth boundaries covered. Nested dynamic fixture is genuine
but total spent greater than direct child cost does not alone prove child debit
writeback; traversal-only work could satisfy it. Broad gates remain unreleased.

After all handles terminal, root backed up five complete originals
route-preparation-meter-before-seam-witness-originals-0002.json
SHAad2e788f417fdcb241f29029eceeb80281cb8b5467e968a63429c13637b80365
and released same5 for a test-only read-only trace at the actual failing child
seam. Require direct cost equality for identical payload/window/depth and final
nested remaining equality to child-after. Production behavior needs no further
change. Route8 draft additionally carries authentic resource totals, privately
selected attached requests after evaluator disposal, and executable isolated
work-bridge/extra invocation-scan charging contracts. No Route8 source release.

### 2026-10-03 — Meter0003 seam source held and focused execution released

Author compile006 original19045/d8a379/101 failed on NodeId module path; the
correct reader::span path was changed only after that handle terminated.
Compile007 original67810/212324/0 and scoped5 format/check0 are terminal.
Held0003 source SHA299cf7e6d68dd917923906fb3bb6c3ca8772520b01817158ce5ce7f02af0bd43;
full944 SHAae897a3ab6a33fd027e8e726c7a7d5a861770d70020cd60cbb45524378d027fe.
Root independently verified every944 hash and all5 caps; only the preparation
and test children differ from0002. Read-only reviewer confirms actual-child
trace and same-query direct cost equality rule out traversal-only masking.
Focused003 original89989 launched under checker ownership; not yet terminal
at this checkpoint. Prepared broad runner remains unexecuted pending focused
acceptance; expected425 distinct prior417 plus meter8. All Rust source held.

### 2026-10-03 — Meter0003 focused accepted and broad425 released

Focused003 original89989/16366a terminated0, all eight actual names passed,
including exact actual child counter handoff; all944 unchanged. Raw log
SHA2336341b19c096f01f0771f5bbfe53f105521300e85cf06e07b2c835c5432149;
final receipt SHA81f7b803149a1f2eecb37084d25f652ec3258563dacaefafdb7e27cec8d33f0d.
Root accepts focused source evidence, releases prepared broad003425 runner
SHAa89f272c33ad6bd73d4e4f850d43baebb1b386476401a064c3892facfa43aee1
for native/regressions/strict alltargets/freshWASM/scoped5 formatting under
full944 source hold. Broader terminal evidence and fresh frontend are pending.
Author's read-only Route8 ownership and genuine fixture feasibility confirm
exact8 is sufficient; Ready/source release still waits predecessor gates.

### 2026-10-03 — Meter0003 fully accepted; actual Route8 implementation released

Broad003 original62099 terminated066282/0, all handles terminal. All425 distinct
actual tests (297 lib and128 public) and native/strict alltargets/freshWASM/scoped5
format gates passed, full944 exact before/after every gate and final. Final
receipt SHA176932ec20d76cc149c8c722be765a74adfb764f0879b57781e02335e534ef76.
Root independently inspected result counts and all944 hashes; fresh frontend
test84601/4eb12a/0 passed590 in78 files, build19085/2b1acd/0. All root handles
terminal, full944 exact; top/deps/dist artifacts identical155098702 bytes
SHA0175699ff49e9fd5cb48f88caaf266368a3358f93c0fa9b8a62b8326a725b8b8.
Root receipt SHA886e97e0f8235d78f303e641065033b0bfc7140f1ad2d15ab544db3ab68b932c.

Root marks immutable-route-authority Ready529
SHA8af4327d28488b00644073b29643f2b31b1a49004a1cf34d52ed9d7c1fbcf7fd
and releases exact8 to author for actual implementation. Source intent preserves
five complete existing originals and three null new children,
SHA799165ad50ea9f9346580c74cd0e571baf592e3e95c6ba0e2036bb9375c70ba5.
Accepted baseline944; expected947 after three additions. Preserve extraction
texts before semantic changes, keep touched sources below1000. Runtime ownership
fixtures and final acceptance pending. Resolver, Ready/scheduler, preReserve
admission, useful varying/first executions and structural hooks remain fullgoal
requirements; no additional Rust paths released.

### 2026-10-03 — Route8 lookup extraction compile and independent comparison

Author extracted payload/request/policy/site authentication before semantic
backend changes. Original compile00131276/a3796a/0 terminal, empty log. Complete
extraction receipt SHAd70c9e93cc6e728823eef1bd44b9d9976545f39c2673c358e64c1671b32a5bee.
Root verifies all four moved function texts after visibility/whitespace
normalization and the retained parent after removing those exact spans and the
declared module/import registration. All saved text hashes match. Amended root
audit SHA6f3a6f76fde0253e2b8d705f72cc23ba00ced6cfd86b85a73c634106079e66a0
replaces its earlier functions-only hash0141b51b59a7f710ca166c096ea6085701dcadee20346b67cc1e8dc16cb82861.
No byte/runtime/issued-backend equivalence claim. Author continues exact8 owning
view and preparation implementation. Future resolver draft now uses actual
transcript/shared work APIs and explicitly requires every augmented contributor
and member before route reconciliation; no additional Rust release.

### 2026-10-03 — Owning view implementation and future transaction gaps

Author resumed after one transient model-capacity failure; existing extraction
and intent preserved. Actual route_view source now contains authentic original
site/policy issuance, retained request records, charged inventory copy and
resource/PCM totals. Compile002 original75986/15dd6d/101 terminated on invalid
EventHandle occurrence/producer API assumptions; corrected only after terminal.
Root noted that initial TrustedSiteCopy resolver accepted arbitrary caller
inventory with only slot existence. Author replaced raw copy tuple with opaque
TrustedRouteCopy owning private immutable inventory/bindings and requiring token
allocation membership. Integration and independent acceptance remain pending.

Independent future playback review confirms query_issued returns an immutable
batch after dropping its collector; a fresh collector cannot prove same-ledger
state across query/freeze/resolution. Root declares separate Planning3path shared
issued-query-work prerequisite, including private owning fixture child to keep
parents below1000. No current Route8 path expansion. Current scalar work bridges
cannot establish future same-collector playback completion.

PoolBook/Slot own private mutable generation/receipt projections with no staging
API. Root declares pools.rs as future playback fifth path for charged staging
and all-or-nothing pool/command publication. Retain original pools and prior
pending commands on later route/encoding/admission failure. No future source
release; full goal still includes admission/vary/first executions/structural work.

### 2026-10-03 — Independent in-progress view trust/copy-accounting review

Read-only review confirms opaque TrustedRouteCopy has private inventory/bindings,
no Clone or mutable getter, and exact binding membership plus original view seal
validation. Bare mutable inventory copies cannot mint bindings. Deep-owned
vector categories are charged; graph/timing/text Rc/Arc remain shared. This is
preliminary source evidence, not held-source/runtime acceptance.

Two concrete missing precharges were sent to author for the next terminal edit
window: publication authenticates original request through Capture track search
before its later site charge; copy-site resolution scans its binding vector.
Charge those scans before calls, including failures, and similarly document
subsequent policy/member lookup scans and fixed seal/allocation units. View
publication authenticates original records before copied inventory and discards
unpublished local records on failure; full core/backend fixtures remain pending.

### 2026-10-03 — Borrowed lookup backend compiles after transition repair

Actual LookupAuthority Snapshot/Issued backend and concrete borrowed record
iterator replace snapshot-only lookup fields without heap record iteration.
Compile003 original33491/b74a15/101 terminated on one borrowed invocation
conversion and two old snapshot-field accesses; repairs followed terminal.
Compile004 original86032/9636f0/0 now terminal. Capture payload lookup charges
before search/authentication, including errors; immutable-copy token membership
scan likewise charges before pointer validation. Pending unused warnings reflect
not-yet-wired issued/prepared consumers, not blanket suppressions.
Issued binding, metered core and genuine owning fixtures remain next; no runtime
backend equivalence or phase completion claim. All changes stay declared Route8.

### 2026-10-03 — Metered core and private prepared owner implemented

Author compile00587236/82b5b5/0 and00682497/a7b1fb/0 are terminal. Actual
prepare core now threads original limits/quota/inherited depth through builder,
direct certification, density and nested metered preparation. Counter upper
bound check and charge-before-operation/checked writeback replace saturation.
PreparedRoutes owns private plan/view/opaque copy; site/policy wrappers validate
copy-owner allocation membership and original seals with precharged scans.
Compile00784025 live at this checkpoint; no edits during live compiler.
Input-size helper precharges, fresh issued binding and owning fixtures remain
next. Root read finds prepare_routes_issued still accepts arbitrary settings;
requires original Song settings validation before trusted copy/preparation and
mutated tempo/tail/seed evidence before acceptance. Not held/runtime proof.

Shared-work future review identifies ReplayView::seed_collection destructively
replacing collector executions. Root explicitly expands that Planning companion
to four paths with song_replay.rs and charged authentic additive retention,
preserving accumulated-state scope instead of a fresh-only collector shortcut.
No additional Rust path is released into current Route8.

## Session 249 batch contract (wave 4, serial finalization)

The source of truth is the design section
[Production integration contract](../../design-docs/specs/design-song-mode.md#production-integration-contract-route-authority-to-playback-2026-10-03).
This section governs only the session-249 batch. The SONG-16 manifest above
stays the long-run song-mode reconciliation scope and is unchanged.

### Batch wave DAG

| Wave | planId | Plan | dependsOn |
|---|---|---|---|
| 1 | SONG-ROUTE8 | song-mode-immutable-route-authority.md | none |
| 2 | SONG-ISSUED-RESOLUTION | song-mode-issued-route-resolution.md | SONG-ROUTE8 |
| 2 | SONG-SHARED-WORK | song-mode-shared-issued-query-work.md | SONG-ROUTE8 |
| 2 | SONG-STRUCTURAL-CLOCK | song-mode-structural-clock-hooks.md | SONG-ROUTE8 |
| 3 | SONG-ISSUED-PLAYBACK | song-mode-issued-playback.md | SONG-ROUTE8, SONG-ISSUED-RESOLUTION, SONG-SHARED-WORK |
| 4 | SONG-16 (this batch) | song-mode-reconciliation.md | all five above |

Plans in the same wave have disjoint writePaths. `src/song/routing/prepared.rs`
is written in waves 1, 2 and 3. `src/song/snapshot/occupancy/lookup/authority.rs`
is written in waves 1 and 2. Each is edited only after the previous wave has
joined.

Session 251 amendment: wave 2 runs serially as 2a (SONG-ISSUED-RESOLUTION),
2b (SONG-SHARED-WORK) and 2c (SONG-STRUCTURAL-CLOCK), with concurrency 1.
Each sub-wave is joined and committed before the next one starts.
SONG-ISSUED-RESOLUTION also owns `src/song/routing/density/index.rs`
(operator-authorized). If that file was edited, the final rustfmt `--check`
and the cohort audit include it as a declared writePath. The cohort stays 952
after wave 2.

### Join protocol (after each wave, run serially by the root reviewer)

1. Compare every writePath's current sha256 with the worker receipts. An
   unexpected change is repaired serially here, never by another worker.
2. Re-run that wave's gates once, serially, on the joined tree. Write the logs
   to `tmp/song-mode-riela/session249-join<wave>-<gate>.log`.
3. Cohort count projection, which the join receipt must confirm:
   - 947 after wave 1;
   - 952 after wave 2, adding `source/issued.rs`, `nested/issued.rs`,
     `snapshot/issued/shared_work_tests.rs`, `song_clock/dispatch.rs` and
     `song_clock/structural_tests.rs`;
   - 953 after wave 3, adding `tests/song_issued_transport.rs`, or 954 if
     `preparation/issued.rs` was needed.

```json
{
  "planId": "SONG-16",
  "batch": "session-249",
  "planPath": "impl-plans/active/song-mode-reconciliation.md",
  "wave": 4,
  "dependsOn": ["SONG-ROUTE8", "SONG-ISSUED-RESOLUTION", "SONG-SHARED-WORK", "SONG-STRUCTURAL-CLOCK", "SONG-ISSUED-PLAYBACK"],
  "writePaths": [
    "impl-plans/active/song-mode-reconciliation.md",
    "design-docs/specs/design-song-mode.md",
    "impl-plans/README.md",
    "tmp/song-mode-riela/session249-final-receipt.json",
    "tmp/song-mode-riela/session249-final-cohort.sha",
    "tmp/song-mode-riela/session249-final-build.log",
    "tmp/song-mode-riela/session249-final-clippy.log",
    "tmp/song-mode-riela/session249-final-nextest-focused.log",
    "tmp/song-mode-riela/session249-final-nextest-full.log",
    "tmp/song-mode-riela/session249-final-wasm.log",
    "tmp/song-mode-riela/session249-final-fmt.log",
    "tmp/song-mode-riela/session249-final-editor-test.log",
    "tmp/song-mode-riela/session249-final-editor-build.log"
  ],
  "sharedPaths": [
    "impl-plans/active/song-mode-immutable-route-authority.md",
    "impl-plans/active/song-mode-issued-route-resolution.md",
    "impl-plans/active/song-mode-shared-issued-query-work.md",
    "impl-plans/active/song-mode-issued-playback.md",
    "impl-plans/active/song-mode-structural-clock-hooks.md"
  ],
  "sharedPathNotes": [
    {"path": "impl-plans/active/song-mode-immutable-route-authority.md", "intendedEdit": "Set Status Completed and tick the completion criteria only if the final gates pass. Keep the worker's progress log."},
    {"path": "impl-plans/active/song-mode-issued-route-resolution.md", "intendedEdit": "Status and criteria reconciliation only, after the final gates."},
    {"path": "impl-plans/active/song-mode-shared-issued-query-work.md", "intendedEdit": "Status and criteria reconciliation only, after the final gates."},
    {"path": "impl-plans/active/song-mode-issued-playback.md", "intendedEdit": "Status and criteria reconciliation only, after the final gates."},
    {"path": "impl-plans/active/song-mode-structural-clock-hooks.md", "intendedEdit": "Status and criteria reconciliation only, after the final gates."}
  ]
}
```

### Intent

Produce the single authoritative final evidence that every acceptance signal of
this batch holds on the joined tree. Then record it and reconcile the statuses.

### Non-goals

- This wave edits no Rust or test files. A gate failure is reported back to the
  review loop with the log path; it is not fixed here.
- Do not archive plans into `impl-plans/completed/` in this batch. Moving them
  would break relative links in other active song-mode plans. Global archiving
  stays deferred to song-mode completion, which also needs SM1, SM2 and the
  other active plans.
- Do not edit, format or stage the unowned paths
  `src/song/snapshot/resources.rs`, `src/song/snapshot/reservations_tests.rs`
  and `src/sched/runtime/song/clock_tests.rs`. Do not run a crate-wide format.

### Final gates (run serially in the foreground; record the exit status and full log path in the final receipt)

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-final-build.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-final-clippy.log 2>&1` must exit 0.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker --test song_issued_transport --test song_export > tmp/song-mode-riela/session249-final-nextest-focused.log 2>&1`
  must exit 0 with a nonzero count per binary.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session249-final-nextest-full.log 2>&1`
  must exit 0. The distinct-test count must be at least 425, the held0003 count.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-final-wasm.log 2>&1` must exit 0.
- `rustfmt --edition 2021 --check <every Rust file touched in waves 1-3> > tmp/song-mode-riela/session249-final-fmt.log 2>&1`
  passes when no `Diff in` line names a touched file.
- `npm --prefix editor run test > tmp/song-mode-riela/session249-final-editor-test.log 2>&1`
  and `npm --prefix editor run build > tmp/song-mode-riela/session249-final-editor-build.log 2>&1`
  must both exit 0. This is regression evidence against the held0003 frontend
  baseline of 590 tests.
- Cohort: `git ls-files -co --exclude-standard -z -- '*.rs' Cargo.toml Cargo.lock | xargs -0 shasum -a 256 > tmp/song-mode-riela/session249-final-cohort.sha`.
  The count must equal the wave-3 join projection, and the only additions are
  the declared new files. Changed hashes appear only on declared writePaths of
  waves 1-3. Any other changed hash fails the gate and is listed under
  `unownedChanges` (design: Unowned working-tree paths). No pre-existing drift
  exception applies.
- `wc -l` on every touched Rust file: each must be below 1000.

### Documentation and status

- Append a dated evidence checkpoint (about 15 lines) to the design section
  "Production integration contract". It lists the gate exits, the log paths,
  the cohort count and the SM1/SM2 outcomes.
- Update the five plan statuses only if the final gates pass. Route8 becomes
  Completed only when the clippy exit is 0.
- Update the six corresponding lines in `impl-plans/README.md`.
- Add one progress-log entry to this plan.

### Done criteria

- [ ] Every final gate exits 0, with full logs recorded.
- [ ] The cohort count and its additions match the projection.
- [ ] The design checkpoint is appended.
- [ ] Statuses are reconciled.
- [ ] No Rust or test file was edited in this wave. Check that
  `git diff --stat` for wave 4 shows only the writePaths and sharedPaths.

### Session 252 amendment (wave 4)

Source: the design section "Session 252 resume amendments".

- **No new seams.** This wave edits no Rust or test file. It declares no Route8
  sharedPath.
- **Format gate file list.** The final `rustfmt --check` list must add
  `src/song/snapshot/occupancy/tests.rs` (2a). It must also add any declared
  seam that a receipt records as edited: `src/song/query/issued.rs` (2b),
  `src/song/routing/nested/issued.rs` (2c, already listed) and
  `src/song/routing.rs` (3, already listed).
- **Cohort.** The cohort stays at 952 through 2c and 953 after wave 3 (954
  with `preparation/issued.rs`). The 2a regression test is appended to an
  existing file and adds nothing. Changed hashes may additionally appear on:
  - the four session-252 2a paths;
  - any conditional seam that a receipt records as edited.
- **Requirement evidence.** The final receipt records:
  - each sub-wave's `seams` entries;
  - the 2a `jointGeometryFixture` value;
  - the four named 2a tests from the full run
    (`distinct_equal_handle_invocations_are_all_resolved`,
    `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`,
    `actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct` and
    `distinct_sites_sharing_one_execution_are_retained_separately`), plus the 2c
    Euclid witness if it was deferred.

### Session 253 amendment (wave 4)

Source: the design section "Session 253 resume amendments". It supersedes the
session 252 cohort numbers above. Everything else in the session 252 section
still applies.

- **Cohort.** 2a adds `src/song/routing/nested/issued/members.rs`. The cohort
  is:
  - 953 from the 2a join through 2c;
  - 954 after wave 3, or 955 with `src/host/caps/song/preparation/issued.rs`;
  - unchanged in wave 4.

  The 2a join may also show changed hashes on these paths:
  - `src/song/snapshot/occupancy/lookup/authority.rs` and
    `src/song/snapshot/occupancy/lookup.rs` (the `selected_policy` delegation);
  - `src/song/routing/configuration/index/canonical.rs`;
  - `src/song/routing/nested/issued.rs`;
  - `src/song/routing/nested/issued/members.rs`.
- **Format gate file list.** Add `src/song/routing/nested/issued/members.rs`.
  Also add `src/song/routing/density/index.rs` if the 2a receipt records it as
  edited.
- **Requirement evidence.** The final receipt lists these five 2a resolver
  tests from the full run, in addition to the session 252 list:
  - `cached_nested_slice_events_resolve_from_issued_transcript`
  - `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`
  - `distinct_equal_handle_invocations_are_all_resolved`
  - `partitioned_nested_issued_queries_equal_the_full_route_set`
  - `discarded_augmented_source_origins_are_resolved`
- **No Rust edits.** Wave 4 still edits no Rust or test file.
