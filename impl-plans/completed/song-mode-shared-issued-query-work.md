# Shared work through issued query and resolution

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-04
**Design Reference**: [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

**Completion**: SONG-SHARED-WORK is reconciled in session 259 from accepted source commit `10c3eab02ab29d25203f7cd5847a2247af404c20`. Final gate and requirement evidence is recorded in `tmp/song-mode-riela/session249-final-receipt.json`. Earlier amendment sections are historical and are superseded by later session amendments.

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
- [x] Independent focused/regression/native/lint/WASM/format gates complete with allowed diagnostics dispositioned; see session 260 progress and clippy evidence.

### TASK-003: Playback adoption

**Status**: Completed
**Parallelizable**: No; depends on issued playback phase

- [x] Actual Ready realization uses one collector through all proofs and output.
- [x] No batch field retains mutable work or evaluator ownership.

## Completion criteria

- [ ] Shared engine and owning evidence accepted under exact source hold.
- [x] Legacy wrapper preserves public behavior and original failures.
- [x] Actual playback adopts companion; unused helpers are insufficient.
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

### 2026-10-04 — Session 261 playback adoption verified

The issued playback consumer adopts shared query/proof work through one
collector. `tmp/song-mode-riela/session249-playback-receipt.json` records the
session-261 gate receipt; `tmp/song-s249/SONG-ISSUED-PLAYBACK/session261-resume/nextest-realize.log`
passes all four realization tests, including shared-collector and atomic
failure behavior. Formal test-integrity, adversarial and integration review
remain downstream.

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
- [x] All seven tests above are present and pass (eight shared-work tests pass).
- [x] Build, full tests and WASM exit 0.
- [x] In this plan's paths, Clippy reports only the two forwarders. Every
  other diagnostic maps to a Route8 D-row or a `RES-` item in the session 260 disposition evidence.
- [x] fmt is clean on the four declared paths and every file is below 1000 lines; the
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

## Session 255 amendment (runs THIRD, after 2c and 2a are joined; serial)

The source of truth is the design section "Session 255 resume amendments
(2026-10-04)": "Implementer authority" and "Remaining waves" > 2b. The scope,
the four writePaths, the two conditional seams, TASK-001 and TASK-002, the
tests and the session 249-254 criteria are unchanged. Only the stop rules, the
diff base (`<2a-join>`, the 2a join commit) and the evidence locations change.

### Intent and context

- `ReplayView::seed_collection` (`src/pattern/eval/song_replay.rs:245`) must
  keep the collector's accumulated executions. It appends view executions
  that are absent by `Rc::ptr_eq`, precharges a checked
  `view.len * (prior.len + 1)` and refuses foreign or unattested prior
  collectors before any mutation. It must never replace or clear executions.
- One caller-owned `SharedIndexWork` must survive across query, freeze and
  issued resolution through the pinned `query_issued_with_work` forwarders.
- `clippy::let_and_return` at `src/song/snapshot/issued.rs:231` is fixed by
  returning the expression directly.
- The partial code from sessions 251-254 is reviewed against "File-level
  changes" above before the gates. It is not presumed accepted.

### Implementer authority (replaces earlier stop clauses)

- Diagnose each failing gate and fix it inside the four writePaths, or the
  two declared seams under their session 252 limits. Rerun the focused
  command until it passes, then run every gate. Record each fix in the
  receipt under `fixes[]` with: the test, the exact message, the root cause,
  the edited paths, and the class.
- Stop only for:
  1. a fix outside the manifest below;
  2. a change to an assertion of a test that exists at `37ea3e8` (for
     example the held-count tests in `snapshot::occupancy` and `song::query`);
  3. a relaxed identity check. Membership is by `Rc::ptr_eq` only; foreign
     and unattested prior collectors are refused before mutation.
- Never replace additive retention with a fresh-only collector. That counts
  as weakening, even when the tests pass.

### Owned paths for session 255

```json
{
  "planId": "SONG-SHARED-WORK",
  "planPath": "impl-plans/active/song-mode-shared-issued-query-work.md",
  "dependsOn": ["SONG-ROUTE8", "SONG-STRUCTURAL-CLOCK", "SONG-ISSUED-RESOLUTION"],
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
  "sharedPaths": [
    "src/song/snapshot/occupancy.rs",
    "src/song/query/issued.rs"
  ]
}
```

- The sharedPaths may change only in the `seed_collection` call statement and
  the collector attachment line directly before it (session 252 rule). The
  receipt records `seams`.
- Never run rustfmt in write mode on `snapshot.rs` or `occupancy.rs`: they
  have unowned children.
- Frozen: every 2a and 2c path, every wave-3 path,
  `src/song/snapshot/occupancy/lookup/authority.rs`, the three unowned paths,
  and `.agents/settings.local.json`.

### Session 255 execution steps (in order)

1. Copy `tmp/song-mode-riela/session249-sharedwork-*` to
   `tmp/song-s249/SONG-SHARED-WORK/attempt-session254/` with sha256 values.
   Scratch logs go under `tmp/song-s249/SONG-SHARED-WORK/session255/`.
2. Rewrite the intent JSON with the design and plan sha256 values and the
   sha256 and full text of each file to edit. Hash-check before every edit.
3. Review the partial code (session 254 bullet "Unreviewed partial code") and
   record the findings.
4. Fix `issued.rs:231` and every finding, under the authority rule.
5. Run verification 1-9 and write the receipt
   `tmp/song-mode-riela/session249-sharedwork-receipt.json` with: the
   review findings, `fixes[]`, `seams`, the clippy disposition, the cohort
   (953), the line counts, the unowned-path result and `evidenceFingerprint`.
   The fingerprint must differ from every hash in
   `tmp/song-s249/SONG-SHARED-WORK/`.
6. Add one progress-log entry.

### Session 255 tests (input -> expected outcome)

The "Tests to add" bullets above, unchanged:

- two queries on one collector -> the first query's executions are still
  present by pointer, and the remaining work is monotonic;
- reseeding the same executions -> no duplicates, and the charge is
  `view.len * (prior.len + 1)`;
- a fresh collector -> the same result and a charge of `view.len`;
- a foreign collector -> refused, with zero callback reads and no mutation;
- an unattached collector with prior executions -> refused, unchanged;
- exact work passes and one less fails, with the debit written back;
- the legacy `query_issued` output equals `query_issued_with_work` on a fresh
  collector.

All 2c and 2a tests stay green.

### Session 255 verification (foreground; record the exit status and full log path)

1. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast -E 'test(/shared_work_tests|song_replay|snapshot::issued|occupancy::/)' > tmp/song-mode-riela/session249-sharedwork-nextest-focused.log 2>&1`
   must exit 0 with a nonzero count.
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker >> tmp/song-mode-riela/session249-sharedwork-nextest-focused.log 2>&1`
   must exit 0.
3. `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-sharedwork-build.log 2>&1`
   must exit 0.
4. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-sharedwork-clippy.log 2>&1`.
   It shows no diagnostic in this plan's paths and no `SW-` row. Only `RES-`
   `dead_code` rows whose sole consumer is wave 3 may remain, each mapped.
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-mode-riela/session249-sharedwork-nextest-full.log 2>&1`
   must exit 0, with every test run and at least 425 distinct tests passed.
6. `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-sharedwork-wasm.log 2>&1`
   must exit 0.
7. `rustfmt --edition 2021 --check src/song/snapshot.rs src/song/snapshot/issued.rs src/song/snapshot/issued/shared_work_tests.rs src/pattern/eval/song_replay.rs > tmp/song-mode-riela/session249-sharedwork-fmt.log 2>&1`.
   It passes when no `Diff in` hunk covers a changed line of these four paths.
   Untouched-child hunks are recorded and not fixed.
8. `wc -l` on the four paths and any edited seam: each is below 1000.
9. `git diff --quiet <2a-join> -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs src/song/snapshot/occupancy/lookup/authority.rs`
   must exit 0. `git ls-files -co --exclude-standard -- '*.rs' Cargo.toml Cargo.lock | wc -l`
   prints 953.

### Session 255 done criteria (mechanically checkable)

- [ ] Every session 252 and 254 done criterion holds, with `<2a-join>` as the
  base.
- [ ] `grep -n "executions = \|executions.clear()" src/pattern/eval/song_replay.rs`
  shows no wholesale replacement of `ledger.executions` inside
  `seed_collection` (line 245 onward). At `c7083fb`, the only match is the
  unrelated `let executions = if let Some(view)` near line 521, outside that
  function.
- [ ] Verification 1-6 and 9 exit 0 (9 prints 953). Verification 4 shows no
  `SW-` row. Verification 7 and 8 meet their rules.
- [ ] `git diff <2a-join> | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
- [ ] The receipt has the review findings, `fixes[]`, `seams` and a new
  fingerprint, and one progress-log entry is added.

## Session 256 amendment (runs THIRD, after 2c and 2a are accepted; serial)

The source of truth is the design section "Session 256 resume amendments
(2026-10-04)". The session 255 amendment stays in force: the owned paths,
the conditional seams, the tests, verification 1-9 and the done criteria. The
base is still `<2a-join>`, the commit that records 2a acceptance. Only these
items change.

- **Allowed failures.** None. 2a ends with full nextest green, so verification
  5 must exit 0 with zero failures. Before any edit, run
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-s249/SONG-SHARED-WORK/session256/baseline-full.log 2>&1`
  on `<2a-join>` and record its failure list in the receipt as
  `baselineFailures`. The expected list is empty. If it is not empty, every
  listed test is outside 2b's paths, so it is stop condition 1: report it with
  its owner and do not start 2b edits.
- **Evidence.** Step 1 copies `tmp/song-mode-riela/session249-sharedwork-*` to
  `tmp/song-s249/SONG-SHARED-WORK/attempt-session255/` with `sha256.txt`.
  Scratch logs go to `tmp/song-s249/SONG-SHARED-WORK/session256/`. The
  fingerprint must differ from every hash under `tmp/song-s249/SONG-SHARED-WORK/`.
- **Unchanged requirements, restated for the implementer:**
  - `ReplayView::seed_collection` (`src/pattern/eval/song_replay.rs`, about
    line 245) keeps accumulated executions additively. It appends only
    executions absent by `Rc::ptr_eq` and precharges the checked
    `view.len * (prior.len + 1)`. It refuses a foreign collector, and an
    unattached collector with prior executions, before any charge or
    mutation. No fresh-only collector, no `clear()`, no wholesale
    `executions =` assignment.
  - `clippy::let_and_return` at `src/song/snapshot/issued.rs:231` is fixed by
    returning the expression directly, with no behavior change and no
    `allow`/`expect`.
  - The 2c tests, the 2a resolver tests and all three
    `routing::source::issued::tests` stay green.

### Session 256 done criteria

- [ ] Every session 255 done criterion holds.
- [ ] `baselineFailures` is recorded and empty.
- [ ] Verification 5 exits 0 with zero failures and a `Summary` line.
- [ ] `tmp/song-s249/SONG-SHARED-WORK/attempt-session255/sha256.txt` exists, and
  the receipt fingerprint is new.

## Session 257 amendment (runs THIRD, after 2c and 2a are accepted; serial)

The source of truth is the design section "Session 257 resume amendments
(2026-10-04)" > "Later waves". The session 255 and 256 amendments stay in
force: the four owned paths including `src/pattern/eval/song_replay.rs`, the
conditional seams, the zero-allowed-failure rule, `baselineFailures`, the
additive charged `Rc::ptr_eq` retention, and the `let_and_return` fix at
`src/song/snapshot/issued.rs:231`. Only the evidence locations change:

- Step 1 copies `tmp/song-mode-riela/session249-sharedwork-*` to
  `tmp/song-s249/SONG-SHARED-WORK/attempt-session256/` with `sha256.txt`. Do
  not create, delete or overwrite earlier attempt directories.
- The baseline run and scratch logs go to
  `tmp/song-s249/SONG-SHARED-WORK/session257/`, for example
  `session257/baseline-full.log`.
- The fingerprint must differ from every hash under
  `tmp/song-s249/SONG-SHARED-WORK/`.

### Session 257 done criteria

- [ ] Every session 256 done criterion holds, with `session257/` in place of
  `session256/`.
- [ ] `tmp/song-s249/SONG-SHARED-WORK/attempt-session256/sha256.txt` exists.

## Session 258 amendment (runs THIRD, after 2a and then 2c are accepted; serial)

The source of truth is the design section "Session 258 resume amendments
(2026-10-04)" > "Serial order and dependency edge" and "Later waves". The
session 255 to 257 amendments stay in force: the four owned paths
(`src/song/snapshot.rs`, `src/song/snapshot/issued.rs`,
`src/song/snapshot/issued/shared_work_tests.rs`,
`src/pattern/eval/song_replay.rs`), the conditional seams
(`src/song/snapshot/occupancy.rs`, `src/song/query/issued.rs`), the tests,
verification 1-9, `baselineFailures`, the zero-allowed-failure rule, the
additive charged `Rc::ptr_eq` retention in `ReplayView::seed_collection` and
the `let_and_return` fix at `src/song/snapshot/issued.rs:231`. Only these
items change:

- **Order and base.** 2b now runs after 2a and then 2c. Every `<2a-join>` in
  this plan means `<2c-accepted>`, the commit that records 2c acceptance on
  `wf/route-authority`. That commit contains 2a's accepted work. The
  `dependsOn` list stays `["SONG-ROUTE8", "SONG-STRUCTURAL-CLOCK", "SONG-ISSUED-RESOLUTION"]`.
- **Baseline.** Before any edit, run
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-s249/SONG-SHARED-WORK/session258/baseline-full.log 2>&1`
  on `<2c-accepted>`. It must exit 0 with a `Summary` line, and the receipt
  records `baselineFailures: []`. A non-empty list is stop condition 1: report
  each test with its owner and do not start 2b edits.
- **No retries.** No nextest `--retries`, and no rerunning the full suite
  until it happens to pass.
- **Reviews.** The reviews and the receipt field `reviewDiffRange` cover
  `git diff 1ac457f <2b-accepted> -- <the four 2b paths and any edited seam>`.
  This range includes the unreviewed partial 2b code already in the tree
  (sessions 251 to 253).
- **Unowned paths.** `git diff --quiet <2c-accepted> -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs`
  exits 0. Never run rustfmt in write mode on `src/song/snapshot.rs` or
  `src/song/snapshot/occupancy.rs`; they have unowned children.
- **Evidence.** Step 1 copies `tmp/song-mode-riela/session249-sharedwork-*`
  to `tmp/song-s249/SONG-SHARED-WORK/attempt-session257/` with `sha256.txt`.
  Never delete or overwrite earlier attempt or session directories. The
  baseline and scratch logs go to `tmp/song-s249/SONG-SHARED-WORK/session258/`.
  The receipt has `session: 258` and `baseCommit`. Its fingerprint differs
  from every hash under `tmp/song-s249/SONG-SHARED-WORK/`.

Tests (input -> expected outcome), restated:

- A collector already holding executions A and B is seeded by a replay view
  holding B (the same `Rc`) and C -> the collector holds A, B and C; B is not
  duplicated; the debit equals the checked `view.len * (prior.len + 1)`
  precharge.
- A foreign collector, or an unattached collector with prior executions ->
  refused before any charge or mutation; `remaining()` and the executions are
  unchanged.
- A fresh collector -> the same result and charge (`view.len`) as before.
- `cargo clippy --all-targets -- -D warnings` -> no `let_and_return`
  diagnostic and no diagnostic in a 2b path.

### Session 258 done criteria

- [ ] Every session 256 done criterion holds, with `session258/` in place of
  `session256/` and `<2c-accepted>` in place of `<2a-join>`.
- [ ] `baseline-full.log` exits 0, and `baselineFailures` is empty.
- [ ] Verification 5 (full, `--no-fail-fast`) exits 0 with a `Summary` line
  and 0 failures.
- [ ] `grep -n "let_and_return" tmp/song-mode-riela/session249-sharedwork-clippy.log`
  prints nothing, and `git diff <2c-accepted> | grep -E '^\+.*#\[(allow|expect)'`
  prints nothing.
- [ ] The receipt records the line range of the `ReplayView::seed_collection`
  body (it starts at about `src/pattern/eval/song_replay.rs:245`).
  `grep -nE 'executions\s*=|\.clear\(\)' src/pattern/eval/song_replay.rs`
  prints no line inside that range. Matches outside it (today line 521
  `let executions = ...` and line 772 `dependencies.clear()`) are not in
  scope.
- [ ] `tmp/song-s249/SONG-SHARED-WORK/attempt-session257/sha256.txt` exists.
  The receipt has `session: 258`, `reviewDiffRange` and a new fingerprint.
- [ ] One progress-log entry is added.

## Session 259 amendment (runs THIRD, on <2c-accepted>; serial)

The source of truth is the design section "Session 259 resume amendments
(2026-10-04)", subsection "Later waves". The session 258 amendment applies
unchanged:

- seeding stays additive, charged and authentic: Rc::ptr_eq retention in
  `src/pattern/eval/song_replay.rs`, with no fresh-only collector, no `clear()`
  and no wholesale `executions` assignment in `seed_collection`;
- `clippy::let_and_return` at `src/song/snapshot/issued.rs:231` is fixed
  without `allow`/`expect`;
- `baselineFailures` is empty;
- full nextest exits 0 with 0 failures.

This amendment changes only these items:

- **Base.** `<2c-accepted>` is the session 259 commit that records
  SONG-STRUCTURAL-CLOCK acceptance.
- **Evidence.** Copy the prior `tmp/song-mode-riela/session249-sharedwork-*`
  files to `tmp/song-s249/SONG-SHARED-WORK/attempt-session258/`, with
  `sha256.txt`. Baseline and scratch logs go to
  `tmp/song-s249/SONG-SHARED-WORK/session259/`, for example
  `session259/baseline-full.log`.
- **Receipt.** It has `session: 259`, `baseCommit: <2c-accepted>`,
  `reviewDiffRange` (`git diff 1ac457f <2b-accepted> -- <2b paths>`) and a new
  fingerprint.
- **Interaction with 2a.** The 2a session 259 change to `bind_issued_member`
  (cross-seal equal-edge agreement) is not a 2b path. If a 2b test fails
  because of it, that is stop condition 1. Do not edit
  `lookup/authority.rs`.

### Session 259 done criteria

- [ ] Every session 258 done criterion holds on `<2c-accepted>`.
- [ ] `tmp/song-s249/SONG-SHARED-WORK/session259/baseline-full.log` shows 0
  failures.
- [ ] `tmp/song-s249/SONG-SHARED-WORK/attempt-session258/sha256.txt` exists.
  The receipt has `session: 259` and a new fingerprint.
- [ ] One progress-log entry is added.

### 2026-10-04 — Session 260 source-matched verification

The runtime-owned accepted dependency set includes SONG-ROUTE8,
SONG-ISSUED-RESOLUTION and SONG-STRUCTURAL-CLOCK. The four declared Rust paths
were already implemented and unchanged from checkpoint `f72a753`; the final
clippy gate exposed the plan-owned `clippy::let_and_return` at
`src/song/snapshot/issued.rs:231`, so the immediate closure result is now
returned directly with no behavior change and no `allow`/`expect` attribute.
The caller-owned shared collector, charged additive `Rc::ptr_eq` retention,
fresh-collector charge and all eight genuine tests remain intact.

Final-source verification (all logs under
`tmp/song-s249/SONG-SHARED-WORK/session260/`):

- Focused nextest: exit 0, 88 passed, 0 failed, including all eight
  `shared_work_tests` (`focused-after-fix.log`).
- Native build: exit 0 (`build-final.log`).
- Full nextest: exit 0, 2,793 passed, 3 skipped, 0 failed
  (`full-nextest.log`).
- WASM build: exit 0 (`wasm.log`).
- Strict all-target clippy: exit 101. The `SW-let_and_return` diagnostic is
  absent; all 66 remaining diagnostics are mapped to Route8 D-rows,
  issued-resolution RES-rows or the two expected SW forwarder rows in
  `clippy-dispositions.json`. They are dead-code/unused-import dispositions
  for the accepted earlier waves whose production callers are wired by the
  downstream playback wave.
- Rustfmt check: exit 1 with hunks only in the unowned child files
  `src/song/snapshot/resources.rs` and
  `src/song/snapshot/reservations_tests.rs`; no hunk names a declared path.
  Both child files remain unchanged. All four declared paths are below 1,000
  lines (`line-counts-final.log`).

`TASK-003` and the completion items for actual playback consumption remain
assigned to SONG-ISSUED-PLAYBACK. Formal review and later workflow finalization
remain downstream.

### 2026-10-04 — Session 260 playback adoption

`SongTransport::realize` passes one collector through issued query, every route
resolution, staged pools, and output. `realize_tests::one_collector_spans_query_and_every_resolution`
passed with exact and one-less work (`tmp/song-s249/SONG-ISSUED-PLAYBACK/session260-resume/nextest-realize-retry1.log`). Playback receipt and serial strict-Clippy handoff:
`tmp/song-mode-riela/session249-playback-receipt.json`.

- [x] Eight shared-work tests pass on final source.
- [x] Native build, full nextest and WASM build pass on final source.
- [x] No shared-work clippy row remains; all other strict-clippy diagnostics
  are recorded with disposition IDs.
- [x] Declared paths are formatted, under the line limit, and unowned child
  paths remain unchanged.
