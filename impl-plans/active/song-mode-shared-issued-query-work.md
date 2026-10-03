# Shared work through issued query and resolution

**Status**: Planning
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose and dependencies

Keep one original collector across query, descriptor freezing and issued route
resolution. Current query_issued creates a collector, writes scalar remaining
back, and returns a batch without the collector. Reconstructing a collector
loses its accumulated execution/observation state and cannot prove same-ledger
integration. Immutable batch/view should not retain mutable work merely to fix
that handoff. Caller retains the collector and passes it through each phase.

- **Previous**: [Immutable route authority](song-mode-immutable-route-authority.md).
- **Related**: [Issued resolution](song-mode-issued-route-resolution.md).
- **Next**: [Issued playback](song-mode-issued-playback.md).

No Rust source released by this Planning draft. Validate signatures and privacy
against accepted Route8 source before Ready.

## Exact candidate manifest

| Path | Deliverable | Status |
|---|---|---|
| src/song/snapshot.rs | Thin SongSnapshot/PreparedSong companions | Not Started |
| src/song/snapshot/issued.rs | Shared-work query/freezing engine and legacy wrapper | Not Started |
| src/song/snapshot/issued/shared_work_tests.rs (new) | Genuine owning shared-ledger fixtures | Not Started |
| src/pattern/eval/song_replay.rs | Charged additive replay seeding | Not Started |

Four paths explicitly declared. snapshot.rs currently905 and issued.rs777;
private fixture child avoids growth past1000. No current Route8 path is added implicitly; the replay path is explicitly declared
for accumulated execution preservation. No dependencies required.

## Required interfaces

```rust
impl SongSnapshot {
    pub(crate) fn query_issued_with_work(
        &mut self, span: TimeSpan, work: &SharedIndexWork, depth: u32,
    ) -> Result<FrozenIssuedBatch, Failure>;
}
impl PreparedSong {
    pub(crate) fn query_issued_with_work(
        &mut self, span: TimeSpan, work: &SharedIndexWork, depth: u32,
    ) -> Result<FrozenIssuedBatch, Failure>;
}
```

Use collector's original limits and remaining; authenticate original Song
attachment. A new unattached collector may bind the actual original once; an
already attached foreign collector must fail before callback/query execution.
Do not reset accumulated observations, executions or remaining. Drop collector
borrows before query callbacks/authentication. The legacy remaining-taking API
creates one collector, calls this companion and writes back on success/error.
Original constructor debit remains charged exactly once for that transaction.

## Tasks

### TASK-001: Caller-owned engine and compatibility

**Status**: Not Started
**Parallelizable**: No

- [ ] Validate original collector attachment/limits/inherited depth.
- [ ] Query and freeze using that actual collector without a new allowance.
- [ ] Preserve legacy API behavior and all failure debits.
- [ ] Replay seeding authenticates and retains prior genuine executions instead
  of replacing them; charge all validation, scans and appended Rc retention.

### TASK-002: Genuine shared-work evidence

**Status**: Not Started
**Parallelizable**: No; depends on TASK-001

- [ ] Real candidate query freezes proofs using supplied collector.
- [ ] Subsequent transcript/binding validation charges that same collector.
- [ ] Actual observations/executions and original attachment remain present.
- [ ] Two genuine queries on the same collector retain all prior executions.
- [ ] Foreign existing execution records refuse without synthetic proof repair.
- [ ] Foreign attachment fails without callback access.
- [ ] Whole transaction exact/one-less/depth failures preserve spent work.
- [ ] Independent focused/regression/native/lint/WASM/format gates pass.

### TASK-003: Playback adoption

**Status**: Not Started
**Parallelizable**: No; depends on issued playback phase

- [ ] Actual Ready realization uses one collector through all proofs and output.
- [ ] No batch field retains mutable work or evaluator ownership.

## Completion criteria

- [ ] Shared engine and owning evidence accepted under exact source hold.
- [ ] Legacy wrapper preserves public behavior and original failures.
- [ ] Actual playback adopts companion; unused helpers are insufficient.
- [ ] Varying domains/pre-Reserve admission remain full-goal requirements.

## Progress log

### 2026-10-03 — Actual ledger handoff gap independently confirmed

Read-only source review confirms issued query drops its original collector after
returning immutable batch. This bounded prerequisite is separate from active
Route8, preserving its eight-path scope. No source implementation or runtime
evidence claimed.

### 2026-10-03 — Explicit fourth path for accumulated execution preservation

Read-only review confirms same-ledger query/freezing/transcript authentication
can be proved locally without accepted Route8. It also identifies actual
ReplayView::seed_collection replacing ledger.executions with its own clone.
Declare song_replay.rs explicitly as fourth path before source release; retain
the accumulated-state requirement rather than narrowing the API to fresh-only
collectors. Authenticate prior original membership, precharge real scans/copies,
and merge authentic raw execution allocations without scalar-equality authority
or destructive reset. Original fresh-collector wrapper remains compatible.
The replay parent is828 lines; keep all touched sources below1000. Preserve
existing seed-domain admission/refusal; additive seeding does not establish
useful varying/first executions or permission to execute unadmitted callbacks.
No fourth-path Rust execution is authorized by this Planning declaration.
