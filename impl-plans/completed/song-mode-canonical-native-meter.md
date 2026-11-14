# Canonical VM and native query shared metering

**Status**: Completed
**Updated**: 2026-10-03
**Design Reference**: [Collector review and cumulative accounting](../../design-docs/specs/design-song-mode.md#canonical-collector-review-owner-selection-and-clocks)

## Purpose and dependency

Close the actual nested fuel-reset gap in the occupancy foundation before
accepting its cumulative work invariant. Direct callback debit alone misses
fresh VmQuery adapters created by song natives. Preserve the public VmQuery
layout and QueryVm interface. No change to ordinary unscoped VM semantics.

- **Depends On**: [Canonical occupancy](../active/song-mode-canonical-index-occupancy.md) held direct foundation and independent checkpoint.
- **Next**: [Ordinary canonical replay](../active/song-mode-canonical-query-replay.md), then immutable consumers and uniform bounds.
- **Acceptance owner**: [Reconciliation](../active/song-mode-reconciliation.md).

Source implementation waits until all current checker processes are terminal
and the root releases this manifest. Ready is not a source-edit release.

## Actual source seams

- Vm::tick debits every instruction and NativeCx::tick already delegates to it.
- Vm::run resets top-level fuel; suppress this reset only inside the private
  original canonical work scope.
- VmQuery::guarded currently supplies fresh fuel and restores it. Nested
  adapters must preserve consumed fuel when the canonical scope is active.
- song natives construct adapters for pure thunks, transforms, sound-kit and
  instrument resolution. The common guard covers their VM calls.
- Native part-events also creates a separate song query. Its QState operations,
  inherited depth and output collection must use the same incoming ledger.
- The existing MeteredSongQuery aggregate post-call debit must be removed when
  tick itself debits the ledger, to prevent double charging.

## Exact seven-path Rust manifest

| Path | Deliverable |
| --- | --- |
| `src/vm/vm.rs` | Private optional canonical ledger scope, checked installer/restorer, actual tick debit and scoped run-reset guard. |
| `src/vm/query_vm.rs` | Common guarded inheritance without nested fuel refund; MeteredSongQuery scope installation and no aggregate double debit. |
| `src/pattern/eval.rs` | Thin dependent-meter QState entry; extract existing observation methods before growth from 975 lines. |
| `src/pattern/eval/song_observation.rs` | Extracted QState methods and shared dependent work/depth support, separating metering from observation selection. |
| `src/song/query.rs` | Private query_part_metered entry inheriting actual limits/work/depth; public wrapper unchanged. |
| `src/vm/natives/song.rs` | Native part-events bridge and charged structural/output growth; other adapters inherit the common guard. |
| `src/song/snapshot/occupancy.rs` | Genuine isolated candidate/native nested query and boundary fixtures. |

Every touched Rust file remains below 1000 lines. No new dependencies, public
VM hooks, callback dictionaries, query score flattening or host edits here.
If source review proves an additional path necessary, revise the manifest
before editing it; do not quietly expand this phase.

## Private interface deliverables

- Vm::with_song_work(work, callback): install the original shared ledger under
  a checked private scope and restore prior scope/fuel/depth on every Result.
- Vm::song_work(): borrowed/strong original work access for internal natives.
- Vm::song_query_depth(): checked inherited pattern plus actual VM frame depth.
- query_part_metered(part, span, context, work, depth): dependent query using
  original limits and counter rather than default independent admission.

Actual tick charges occur before instruction/native iteration execution. Nested
guarded calls must neither receive fresh fuel nor refund consumed fuel. Existing
native loops already ticking remain charged once. Argument, structural queue,
five-field part-events row and result-list copies debit work before growth.
Retained callback journal copies remain separately charged for actual storage.

Dependent computational queries share metering but must not inherit the outer
occupancy target filter: a callback querying a Part must obtain its genuine
Part events. Their internal Index rows do not become unrelated output occupancy
observations. Keep owner observation selection distinct from shared work access.

## Tasks

### TASK-001: Scoped VM debit and guard inheritance

**Status**: Completed
**Parallelizable**: No

- [x] Seal verified foundation input hashes and this seven-path intent.
- [x] Install private original work scope and debit actual tick before execution.
- [x] Preserve consumed fuel through nested VmQuery and scoped top-level run.
- [x] Remove aggregate post-debit; prove no nested refund or double charging.
- [x] Restore outer fuel/depth/scope on successful and failed callback paths.

### TASK-002: Dependent native query and allocations

**Status**: Completed
**Parallelizable**: No; depends on TASK-001.

- [x] Extract parent QState helpers and keep all touched files below1000 lines.
- [x] Share original limits/work/inherited depth through native part-events.
- [x] Separate dependent metering from output-target observation filtering.
- [x] Charge structural queues and actual output row/list copies before growth.
- [x] Preserve all ordinary native and public query behavior outside the scope.

### TASK-003: Genuine nested coverage and independent gates

**Status**: Completed
**Parallelizable**: No; depends on TASK-002.

- [x] Original candidate callback actually invokes part-events with nested calls.
- [x] Measured sufficient/one-less cumulative work and inherited depth are real.
- [x] Native structural-thunk evaluation uses the same scope and counter.
- [x] Native iteration fuel is charged once, and failure restores outer state.
- [x] Existing six canonical fixtures and affected ordinary VM/query tests pass.
- [x] Independent native, strict all-target lint, WASM and scoped format pass against exact held source.

## Completion boundary

This phase proves transitive cumulative metering, not ordinary query replay,
immutable admission or uniform varying-seed authority. Only after actual nested
fixtures and independent gates may the foundation's shared-meter criterion be
checked. The full song-mode goal and reconciliation plan remain active.

## Progress log

### 2026-10-03 — Author source review identifies an executable bridge

The specialized Rust author traces instruction/native iteration debit to
Vm::tick and the native-created adapters/query_part seam. The root converts
that read-only proposal into this bounded manifest while the prior foundation
is under independent affected-suite verification. No Rust edit is released yet.

### 2026-10-03 — Released implementation intent

Foundation receipt `/tmp/vactr-canonical-foundation-final-003.json` proves six genuine fixtures, 188 affected library tests and 71 public tests, with native/strict lint/WASM/format success. This bridge now installs the original shared ledger at actual VM tick, preserves nested consumption, and meters computational Part queries without occupancy selection. No Cargo is run by the author. Baselines:

- `src/vm/vm.rs`: 769 lines, SHA256 `569d3bb3b3826b4c524199c1728d4d0ffa4edc110f2968fc189ffb16fdfa16a3`.
- `src/vm/query_vm.rs`: 213 lines, SHA256 `443886d7186917c7350da2ef4701b270a197404b7f1960abae9356f67c214d84`.
- `src/pattern/eval.rs`: 975 lines, SHA256 `c3b14eaa98744e9173ef3a9f16c3bf3c5858bfd71197f0d30dfbe410d8581d6c`.
- `src/pattern/eval/song_observation.rs`: 141 lines, SHA256 `99f86e2ff1a327db5cfcc3fb8d9ed4a7beda0342f4208f0053ba0c994c98b572`.
- `src/song/query.rs`: 675 lines, SHA256 `ff2e831bd5a578ac8011a59d8240e4d55a4c2944da39c99b8e2e3ef67bf8ae5b`.
- `src/vm/natives/song.rs`: 448 lines, SHA256 `0e0f8361bcde878303d165d6cbe99829d14e50d33a9681c1c7496d5afb1295e4`.
- `src/song/snapshot/occupancy.rs`: 620 lines, SHA256 `e6ab839212212038d3c3d67596e0159c3619f1df49ebe6842d4f82ce41add521`.

### 2026-10-03 — Coherent seven-path source hold, independent checks pending

Actual tick debits the original ledger before each instruction/native iteration.
MeteredSongQuery no longer post-debits aggregate fuel. Scoped nested VmQuery
preserves consumed fuel; ordinary guards retain their old independent policy.
The VM restores original fuel/depth/scope on every Result. Dependent Part query
uses original limits/work and inherited pattern-plus-live-frame depth. A scoped
frame floor prevents ancestor VM frames being counted twice after that query
inherits them. QState helpers are extracted into the existing observation child;
computational queries meter normally without output-owner selection/recording.
Native structural queues, reconstructed containers, Part constructors, result
union storage and exact event dictionaries debit before growth.

Four genuine added fixtures in `song::snapshot::occupancy::tests`:

- `native_part_events_preserves_dependent_values_and_shared_exact_work`
- `nested_native_query_inherits_original_depth_without_refund`
- `actual_vm_ticks_debit_once_and_restore_failed_scope`
- `native_structural_thunk_retains_same_meter_and_dependent_result`

They retain actual compiled original candidate/VM authority. Dependent note61
must reach the native dictionary and outer cut0 despite a different occupancy
target. Exact/one-less shared work, minimum/one-less inherited depth, failure
restoration/reuse and actual tick debit equality are asserted. Structural-thunk
coverage constructs a real Part from a lazy dictionary member under Query mode.
These fixtures are written, not yet compiled or passed. Existing six remain.
Scoped rustfmt and its check both exit0; every seven-path Rust file is below1000.
No Cargo/dependency changes or ordinary query replay/consumer/uniform admission
claim. Source and this plan are held for the independent joined checker.

### 2026-10-03 — Joined independent acceptance and archive

Actual joined checkpoint executes307 unique passing tests:10 occupancy,13
original component fixtures,189 affected library tests and95 public tests.
Native, strict all-target Clippy, pure WASM and twelve-file formatting/line
checks exit0. All923 held inputs are exact. Evidence:
/tmp/vactr-canonical-meter-final-001.json, SHA256
4447b1a96ee59709084cfe647cba0ff8116a4147c911d5a312aad93d29ef8b63.
All checker processes are terminal. The completed bounded phase is archived;
ordinary replay, immutable admission and uniform varying-seed authority remain
required in the active song-mode plans. No full song-mode completion is claimed.
