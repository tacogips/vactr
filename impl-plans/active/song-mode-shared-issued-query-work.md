# Shared work through issued query and resolution

**Status**: Ready (wave 2, session 249; released after SONG-ROUTE8 joins)
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

## Session 249 executable contract (wave 2)

The source of truth is the design section "Wave 2b: shared issued query work"
and decision D8. TASK-003 (playback adoption) is carried out by
SONG-ISSUED-PLAYBACK in wave 3. In this wave, only TASK-001 and TASK-002 are in
scope.

```json
{
  "planId": "SONG-SHARED-WORK",
  "planPath": "impl-plans/active/song-mode-shared-issued-query-work.md",
  "wave": 2,
  "dependsOn": ["SONG-ROUTE8"],
  "writePaths": [
    "src/song/snapshot.rs",
    "src/song/snapshot/issued.rs",
    "src/song/snapshot/issued/shared_work_tests.rs",
    "src/pattern/eval/song_replay.rs",
    "impl-plans/active/song-mode-shared-issued-query-work.md",
    "tmp/song-mode-riela/session249-sharedwork-intent.json",
    "tmp/song-mode-riela/session249-sharedwork-receipt.json",
    "tmp/song-mode-riela/session249-sharedwork-build.log",
    "tmp/song-mode-riela/session249-sharedwork-clippy.log",
    "tmp/song-mode-riela/session249-sharedwork-nextest-focused.log",
    "tmp/song-mode-riela/session249-sharedwork-nextest-full.log",
    "tmp/song-mode-riela/session249-sharedwork-wasm.log",
    "tmp/song-mode-riela/session249-sharedwork-fmt.log"
  ],
  "sharedPaths": []
}
```

### Intent and context

Query, freezing and issued resolution must share one original collector, so
that accumulated executions and spent work survive across phases. Today:

- `SongSnapshot::query_issued` (`snapshot.rs:182`) and
  `PreparedSong::query_issued` (`snapshot.rs:289`) create their own collector
  inside `issued::query_issued` (`snapshot/issued.rs:200`).
- `ReplayView::seed_collection` (`song_replay.rs:245-257`) charges
  `executions.len()` and then replaces `ledger.executions` outright.

### Pinned contract for waves 2a and 3 (do not change)

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

### Non-goals

- Do not edit occupancy.rs, query/issued.rs or the routing files. Their
  `seed_collection` call sites keep the same signature.
- Do not change varying-seed admission.
- Do not change the `seed_collection` signature.
- No fresh-only collector shortcut.
- No new `allow`/`expect` attributes. Leave the existing
  `cfg_attr(not(test), allow(dead_code))` on `query_issued` in place.

### File-level changes

1. **`song_replay.rs` `seed_collection`**
   - First check that the ledger is attached to this view's original Song. If
     `ledger.original` is `Some` and is a different allocation from
     `self.original` (not `Rc::ptr_eq`), fail with a foreign-collector error. If
     it is `None` and `ledger.executions` is empty (a fresh collector), allow it
     and leave it for the caller to attach. If it is `None` and
     `ledger.executions` is non-empty, fail: those prior records have no
     attested original, so they are not authentic (the design's "prior record
     that is not authentic fails without changing the ledger").
   - Every one of these checks runs before the precharge and before any
     mutation, so a refusal from them takes no precharge and leaves
     `ledger.executions` and `ledger.original` unchanged.
   - Precharge the checked `view.len * (prior.len + 1)` before any mutation.
     Once taken, only this debit stays consumed if a later step fails.
   - For each view execution, scan prior entries by `Rc::ptr_eq`. Append it
     only if it is absent.
   - Never clear or replace prior entries.
   - A fresh collector must give the same result and the same charge
     (`view.len`) as today.
   - The file is 828 lines. Keep it below 1000.
2. **`snapshot/issued.rs`**
   - Refactor the engine so it takes `&SharedIndexWork` and the inherited
     depth.
   - The legacy remaining-taking `query_issued` creates one collector, calls
     the engine, and writes back the actual debit on both success and error.
     Imitate `lookup/authority.rs:with_work` for the before/after debit.
   - An unattached collector is bound to the original Song once. A collector
     already attached to another Song fails before any callback or query runs.
   - Drop collector borrows before query callbacks.
3. **`snapshot.rs`**: add the two thin `query_issued_with_work` forwarders. The
   file is 905 lines and may grow by at most 30.
4. **`snapshot/issued/shared_work_tests.rs` (new)**: add the fixtures. Register
   it with `#[cfg(test)] mod shared_work_tests;` in `issued.rs`.

### Key pitfalls

- Charging the fresh-collector path differently from today would break
  held-count tests in occupancy and query.
- Do not deduplicate by key equality. Pointer identity is the only membership
  test.
- On error, the legacy wrapper must still write back the spent work.
- Do not keep a `borrow_mut` alive across `query_rows` or callbacks; doing so
  causes a `RefCell` panic.

### Tests to add (in `shared_work_tests.rs`, with genuine candidates)

- Two genuine `query_issued_with_work` calls on one collector: every execution
  from the first query is still present, by pointer, after the second, and the
  remaining work decreases monotonically.
- `seed_collection` on a collector that already holds the same executions: no
  duplicates are appended, and the charge equals `view.len * (prior.len + 1)`.
- `seed_collection` on a fresh collector: the result equals a clone of the
  view, and the charge equals `view.len`, as before.
- A collector attached to a foreign Song: the call fails, the callback-read
  counter stays at 0, and the remaining work is unchanged apart from any
  precharge.
- An unattached collector that already holds prior executions is refused by
  `seed_collection`; its `executions` (by pointer) and `original` are unchanged
  afterwards.
- The whole transaction with exact work succeeds. With one less, it fails, and
  the legacy wrapper writes back the spent work.
- The legacy `query_issued` output equals the output of `query_issued_with_work`
  on a fresh collector: same descriptors, same seal count.

### Verification (run in the foreground; record the exit status and full log path)

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-sharedwork-build.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-sharedwork-clippy.log 2>&1`
  may report only `dead_code` on the two new forwarders, whose consumer is
  wave 3. Record them.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/shared_work_tests|song_replay|snapshot::issued|occupancy::/)' > tmp/song-mode-riela/session249-sharedwork-nextest-focused.log 2>&1`
  must exit 0 with a nonzero count.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session249-sharedwork-nextest-full.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-sharedwork-wasm.log 2>&1` must exit 0.
- `rustfmt --edition 2021 --check src/song/snapshot.rs src/song/snapshot/issued.rs src/song/snapshot/issued/shared_work_tests.rs src/pattern/eval/song_replay.rs > tmp/song-mode-riela/session249-sharedwork-fmt.log 2>&1`
  passes when no `Diff in` line names these four paths. `snapshot.rs` pulls in
  `resources.rs` and `reservations_tests.rs`. Hunks in those two files are
  pre-existing drift: record them and do not fix them.
- `wc -l` on the four Rust paths: each must be below 1000.

### Shared-branch protocol

Same as SONG-ISSUED-RESOLUTION: an intent JSON before the first edit, a hash
check before every edit, no edits to other workers' files, no crate-wide
format, no stash/checkout/reset, and updates to this plan's log only.

### Done criteria

- [ ] Both pinned signatures exist.
- [ ] `seed_collection` is additive, and the fresh-path charge is unchanged.
- [ ] All seven tests above are present and pass.
- [ ] Build, full tests and WASM exit 0.
- [ ] Clippy diagnostics are limited to the two forwarders.
- [ ] fmt is clean on touched paths and every file is below 1000 lines.
- [ ] No new `allow`/`expect` attributes.
- [ ] The progress-log entry is recorded.
