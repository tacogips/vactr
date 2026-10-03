# Shared work through issued query and resolution

**Status**: In Progress (wave 2, session 251; implementation present, combined-tree verification pending)
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
| src/song/snapshot.rs | Thin SongSnapshot/PreparedSong companions | Implemented |
| src/song/snapshot/issued.rs | Shared-work query/freezing engine and legacy wrapper | Implemented |
| src/song/snapshot/issued/shared_work_tests.rs (new) | Genuine owning shared-ledger fixtures | Implemented; final verification pending |
| src/pattern/eval/song_replay.rs | Charged additive replay seeding | Implemented |

Four paths explicitly declared. snapshot.rs currently921 and issued.rs809;
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

**Status**: Implemented; final verification pending
**Parallelizable**: No

- [x] Validate original collector attachment/limits/inherited depth.
- [x] Query and freeze using that actual collector without a new allowance.
- [x] Preserve legacy API behavior and all failure debits.
- [x] Replay seeding authenticates and retains prior genuine executions instead
  of replacing them; charge all validation, scans and appended Rc retention.

### TASK-002: Genuine shared-work evidence

**Status**: In Progress; owning tests implemented, combined-tree gates pending
**Parallelizable**: No; depends on TASK-001

- [x] Real candidate query freezes proofs using supplied collector.
- [x] Subsequent transcript/binding validation charges that same collector.
- [x] Actual observations/executions and original attachment remain present.
- [x] Two genuine queries on the same collector retain all prior executions.
- [x] Foreign existing execution records refuse without synthetic proof repair.
- [x] Foreign attachment fails without callback access.
- [x] Whole transaction exact/one-less/depth failures preserve spent work.
- [ ] Independent focused/regression/native/lint/WASM/format gates pass.

### TASK-003: Playback adoption

**Status**: Not Started
**Parallelizable**: No; depends on issued playback phase

- [ ] Actual Ready realization uses one collector through all proofs and output.
- [ ] No batch field retains mutable work or evaluator ownership.

## Completion criteria

- [ ] Shared engine and owning evidence accepted under exact source hold.
- [x] Legacy wrapper preserves public behavior and original failures.
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
  "dependsOn": ["SONG-ROUTE8", "SONG-ISSUED-RESOLUTION"],
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
  may report, in this plan's four paths, only `dead_code` on the two new
  `query_issued_with_work` forwarders, whose consumer is wave 3. Diagnostics
  in other paths are allowed only if they map to a Route8 disposition row
  D01-D32 (`song-mode-immutable-route-authority.md`) or to a concurrent
  SONG-ISSUED-RESOLUTION item. Record every diagnostic as
  `{file, line, message, rowId}`, with `rowId` set to `SW-<item>`, a D-row or
  `RES-<item>`. Do not edit any file outside this plan's writePaths to clear a
  diagnostic.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/shared_work_tests|song_replay|snapshot::issued|occupancy::/)' > tmp/song-mode-riela/session249-sharedwork-nextest-focused.log 2>&1`
  must exit 0 with a nonzero count.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session249-sharedwork-nextest-full.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-sharedwork-wasm.log 2>&1` must exit 0.
- `rustfmt --edition 2021 --check src/song/snapshot.rs src/song/snapshot/issued.rs src/song/snapshot/issued/shared_work_tests.rs src/pattern/eval/song_replay.rs > tmp/song-mode-riela/session249-sharedwork-fmt.log 2>&1`
  passes when no `Diff in` line names these four paths. `snapshot.rs` pulls in
  the unowned child modules `resources.rs` and `reservations_tests.rs`. Record
  any `--check` hunk reported for them as an untouched-child hunk and do not fix
  it. Never run rustfmt in write mode on `snapshot.rs`, because that would
  rewrite those children. A changed cohort hash on either file is not excused;
  it fails the join audit (design: Unowned working-tree paths).
- `wc -l` on the four Rust paths: each must be below 1000.

### 2026-10-03 — Session 251 shared-work implementation

Implemented both pinned `query_issued_with_work` forwarders and a caller-owned
query/freeze engine. The legacy wrapper constructs one collector and writes its
actual remaining work back after either result. The engine binds an unattached
collector to the original Song, refuses a foreign attachment before query
callbacks, and drops its collector borrow before running the query.

`ReplayView::seed_collection` now checks the view and collector attachments by
pointer identity, refuses unattached collectors with prior executions before
charging, rejects prior execution records for another Song without replacing
them, and precharges checked `view.len * (prior.len + 1)` work before additive
pointer-identity retention. The fresh collector charge remains `view.len`.
Eight genuine fixture tests are present, including the seven required cases and
an attached collector carrying a foreign execution record. The TASK-003 playback
consumer remains assigned to SONG-ISSUED-PLAYBACK.

An initial focused run exposed and failed one descriptor-comparison test
because its fixtures had separate global revision identities; that test was
corrected to compare both APIs on one snapshot. A subsequent focused run passed
85/85 before the final foreign-record assertion was added
(`tmp/song-s249/session251-sharedwork-nextest-focused-attempt2.log`, summary at
line 347; the same archived log later includes a compile-failed rerun). Neither
run is final-source evidence. Current complete logs report:

- Build, focused nextest, full nextest, and wasm build exit 101 because
  unowned issued-resolution files still have compile errors: private
  `CanonicalIndexRequest` and `ResolutionBudget` access, an absent `topology`
  method, a missing `issued_leaves` field, and an unresolved
  `FrozenIssuedSongEvent` import. Current nextest attempts stop before tests
  start. Logs: `tmp/song-mode-riela/session249-sharedwork-build.log`,
  `tmp/song-mode-riela/session249-sharedwork-nextest-focused.log`,
  `tmp/song-mode-riela/session249-sharedwork-nextest-full.log`, and
  `tmp/song-mode-riela/session249-sharedwork-wasm.log`.
- Clippy exits 101 on the same compile errors and reports the unowned
  `nested/issued.rs:499` `collapsible_if` lint and the existing Route8
  `prepare_routes_issued` import warning. The receipt maps these to
  `RES-TASK-002` and `D07`. Log:
  `tmp/song-mode-riela/session249-sharedwork-clippy.log`.
- The exact rustfmt check exits 1 with hunks only in unowned child files
  `src/song/snapshot/resources.rs` and
  `src/song/snapshot/reservations_tests.rs`. Neither file was edited or
  formatted. Log: `tmp/song-mode-riela/session249-sharedwork-fmt.log`.
- `git diff --quiet -- src/song/snapshot/resources.rs
  src/song/snapshot/reservations_tests.rs` exited 0. Current line counts are
  921, 809, 382 and 871 for the four plan Rust paths.
- The Rust source manifest before and after the final gate attempts is
  byte-identical (SHA-256 of each manifest:
  `d90f010b5906465ffbdabc4684ef13035dcaef555c700d01065ccbaacf9e7cf1`).

No repair was made outside this plan's write paths. TASK-001 and the behavioral
TASK-002 cases are implemented; final combined-tree verification remains open
until the unowned compile blockers are resolved. Formal review and playback
adoption remain downstream workflow work.

### Shared-branch protocol

Same as SONG-ISSUED-RESOLUTION: an intent JSON before the first edit, a hash
check before every edit, no edits to other workers' files, no crate-wide
format, no stash/checkout/reset, and updates to this plan's log only.

### Done criteria

- [x] Both pinned signatures exist.
- [x] `seed_collection` is additive, and the fresh-path charge is unchanged.
- [ ] All seven tests above are present and pass.
- [ ] Build, full tests and WASM exit 0.
- [ ] In this plan's paths, Clippy reports only the two forwarders. Every
  other diagnostic maps to a Route8 D-row or a `RES-` item in the receipt.
- [x] fmt is clean on touched paths and every file is below 1000 lines; the
  only `--check` hunks are in the two untouched child modules documented above.
- [x] No new `allow`/`expect` attributes.
- [x] The progress-log entry is recorded.

## Session 251 amendment (wave 2b, serial)

Source of truth: design section "Session 251 resume amendments". This plan
starts only after SONG-ISSUED-RESOLUTION (2a) is joined and committed. It runs
alone (concurrency 1). The compile blockers that session 250 recorded came
from concurrent issued-resolution files. They are resolved by 2a before this
sub-wave runs.

- Fix `clippy::let_and_return` at `src/song/snapshot/issued.rs:231` by
  returning the expression directly. Do not use `allow`/`expect`. Do not
  change behavior.
- `seed_collection` keeps its charged, additive `Rc::ptr_eq` retention. A
  fresh-only collector, or a seeding path that clears or replaces prior
  executions, fails review.
- The eight tests in `shared_work_tests.rs` (the seven required ones plus the
  attached-collector foreign-execution test) must all pass on the final
  source. The foreign-execution test must be part of the final focused run.
- Clippy: after this sub-wave, no `SW-` row may remain. Map other diagnostics
  to a Route8 D-row, `RES-<item>`, or `ST-<item>` for committed 2c paths.
- Before rerunning the gates, the orchestrator copies the existing
  `tmp/song-mode-riela/session249-sharedwork-*` files to
  `tmp/song-s249/SONG-SHARED-WORK/attempt-session250/` and records their
  sha256 values. Rerun every verification command above in the foreground.
  Record each exit status and full log path.

### Session 251 test cases (input -> expected outcome)

- Two `query_issued_with_work` calls on one collector -> every first-query
  execution is still present by `Rc::ptr_eq`, and remaining work never
  increases.
- `seed_collection` on a collector that already holds the view's executions
  -> no duplicates, and the charge equals `view.len * (prior.len + 1)`.
- `seed_collection` on a fresh collector -> the result equals the view, and
  the charge equals `view.len`.
- A foreign-attached collector, or an unattached collector with prior
  executions -> `Err` before the precharge. `executions` (by pointer) and
  `original` are unchanged, and there are 0 callback reads.
- An attached collector that carries a foreign execution record -> `Err`, and
  the prior entries are not replaced.
- The legacy `query_issued` equals `query_issued_with_work` on a fresh
  collector (same descriptors and seal count). Exact work is `Ok`, one less is
  `Err`, and the spent work is written back.

### Session 251 done criteria (mechanically checkable)

- [ ] Strict clippy shows no `let_and_return` (or any other non-dead-code
  lint) in the four paths. `git diff 1ac457f -- <four paths> | grep -E '^\+.*#\[(allow|expect)'`
  prints nothing.
- [ ] `grep -n "executions.clear\|executions = " src/pattern/eval/song_replay.rs`
  shows no replacement of prior executions in `seed_collection`.
- [ ] The focused filter exits 0 and runs all eight `shared_work_tests`. Full
  nextest and WASM exit 0.
- [ ] `git diff --quiet -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs`
  exits 0. Each of the four paths is < 1000 lines, and `snapshot.rs` is at
  most 935 lines.
- [ ] The receipt and a progress-log entry are written with the exit codes and
  log paths.

## Session 252 amendment (wave 2b, serial)

Source: the design section "Session 252 resume amendments", Downstream seams
(operator authorization 2). This plan starts only after 2a is joined and
committed. TASK-001 to TASK-002 and the session 251 criteria are unchanged.
The session-251 partial code already in `song_replay.rs` and `issued.rs` at
`980083a` is unreviewed. Review it against the "Wave 2b" design contract before
the gates run; it is not presumed accepted.

### Declared seams (conditional sharedPaths)

`seed_collection` has three production call sites. A 2b test exercises each
one:

- `src/song/snapshot/occupancy.rs:194` (retention; the collector is attached
  at `:191` before seeding);
- `src/song/snapshot/occupancy.rs:390` (fresh collector, never attached);
- `src/song/query/issued.rs:191` (`query_part_issued`; seeds before
  `IssuedQueryTransaction::begin`).

The signature does not change, so none of them is expected to need an edit.
Both files are declared up front only so that a concrete defect does not end
the run:

- **`src/song/snapshot/occupancy.rs`** (a Route8 path; owned by 2a in session
  252). Edit only the `seed_collection` call statement and the collector
  attachment directly before it. Edit them only if a `shared_work_tests` or
  `snapshot::occupancy` test proves that the call order is refused by the
  attachment or prior-execution checks. Do not touch the 2a `site_alias`
  logic, `same_execution` or `shares_execution`.
- **`src/song/query/issued.rs`**. Same rule: only the call at `:191` and its
  attachment.

If neither file needs a change, record `"seams": {"occupancy.rs": "declared,
unedited", "query/issued.rs": "declared, unedited"}` in the receipt. If either
is edited, add it to the `rustfmt --check` and `wc -l` commands. Never run
rustfmt in write mode on `occupancy.rs`, because its child modules are not
owned by this plan.

### Interaction with the 2a site aliases

A 2a site alias holds the same `OwnerInvocation` allocations as its primary,
but `ReplayView` executions are collector executions, not records. Additive
`Rc::ptr_eq` seeding therefore never sees alias records and needs no
alias-specific code. Do not add any.

### Session 252 done criteria

- [ ] `clippy::let_and_return` at `src/song/snapshot/issued.rs:231` is gone
  without `allow`/`expect`, and no `SW-` row remains.
- [ ] The receipt has the `seams` entry. Any edit to a declared seam is
  limited to the call statement and the attachment line
  (`git diff 980083a -- src/song/query/issued.rs` is empty or touches only
  those lines).
- [ ] 2a's `distinct_sites_sharing_one_execution_are_retained_separately` and
  `distinct_equal_handle_invocations_are_all_resolved` still pass in the full
  run.

## Session 254 amendment (runs THIRD, after 2c and 2a are accepted; serial)

The source of truth is the design section "Session 254 resume amendments
(2026-10-04)", subsection "Carried forward unchanged". The scope, the four
writePaths, the two conditional seams, the tests and the done criteria of
this plan are unchanged. Only the following changes.

- **Order and base.** `dependsOn` is now SONG-ROUTE8, SONG-STRUCTURAL-CLOCK and
  SONG-ISSUED-RESOLUTION. Every `git diff` base in this plan's checks becomes
  the 2a join commit (written `<2a-join>`), replacing `980083a` and
  `6543273`. The cohort is 953 and stays 953.
- **Unreviewed partial code.** Before the gates, review
  `git diff 37ea3e8 a8b8ed2 -- src/song/snapshot.rs src/song/snapshot/issued.rs src/song/snapshot/issued/shared_work_tests.rs src/pattern/eval/song_replay.rs`
  against Wave 2b. Record the findings in the receipt. The review checks:
  - `seed_collection` is additive by `Rc::ptr_eq`, never clears prior
    entries, precharges a checked `view.len * (prior.len + 1)`, and refuses
    foreign or unattached-with-prior collectors before any mutation or
    precharge;
  - there is no fresh-only collector anywhere on the query path;
  - the fresh-collector result and charge (`view.len`) are unchanged.
- **Carried repair.** Fix `clippy::let_and_return` at
  `src/song/snapshot/issued.rs:231` by returning the expression directly, with
  no `allow`/`expect` and no behavior change.
- **Interaction with the 2a session 254 change.** The 2a fallback scan in
  `bind_issued_owner_if_matching` reads retained records only. It does not
  touch `ReplayView` or the collector's executions. This plan does not depend
  on it and must not edit `lookup/authority.rs`.
- **Full suite.** Every full nextest run adds `--no-fail-fast` and must exit
  0, with no allowed failures, every test run and at least 425 distinct tests
  passed. The 2a tests for session 254 (including the possibly renamed
  `discarded_augmented` test) and the 2c `domains` test must pass in that run.
- **Evidence.** Copy `tmp/song-mode-riela/session249-sharedwork-*` to
  `tmp/song-s249/SONG-SHARED-WORK/attempt-session253/` with sha256 values
  before rerunning. The receipt fingerprint must differ from every earlier
  sharedwork receipt hash under `tmp/song-s249/SONG-SHARED-WORK/`. Strict
  Clippy must show no `SW-` row afterward. Only `RES-` `dead_code` rows whose
  sole consumer is wave 3 may remain.
- **Unowned paths.**
  `git diff --quiet <2a-join> -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs`
  must exit 0. `snapshot.rs` has those children, so it is never run through
  rustfmt in write mode.

### Session 254 done criteria

- [ ] The receipt records the partial-code review findings and the `seams`
  entries.
- [ ] `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` shows
  no diagnostic in this plan's paths and no `SW-` row.
- [ ] Full nextest with `--no-fail-fast` exits 0.
- [ ] One progress-log entry is added.
