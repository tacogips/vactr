# Issued song playback through Ready and scheduler

**Status**: Ready (session 260 resume amendment: owner-duration fix and export profile authorized; see "Session 260 amendment")
**Created**: 2026-10-03
**Design Reference**: [Production provenance review](../../design-docs/references/song-mode/production-provenance-review-20261003.md)

## Purpose and dependencies

Make production scheduling consume original query and source-member proofs
through the authenticated geometry resolver. Preserve public descriptor APIs.

- **Previous**: [Frozen issued events](song-mode-frozen-issued-events.md).
- **Depends On**: [Immutable route authority](song-mode-immutable-route-authority.md)
  and [authenticated issued route resolution](song-mode-issued-route-resolution.md),
  whose evidence adapter and signatures must be finalized before this plan is Ready.
- **Depends On**: [Shared issued query work](song-mode-shared-issued-query-work.md).
- **Next**: Required pre-Reserve retention/admission and useful varying-seed
  certification; storing a prepared owner does not prove either.

No source edits are released by this candidate plan. A companion that forwards
to scalar-only resolve_route does not satisfy its purpose.

## Proposed manifest

| Module | Path | Status |
|---|---|---|
| Ready authority and query handoff | `src/host/caps/song/preparation.rs` | Not Started |
| Scheduler issued-envelope consumption | `src/sched/song.rs` | Not Started |
| Transactional physical projection | `src/sched/song/pools.rs` | Not Started |
| Prepared owner authenticated resolver seam | `src/song/routing/prepared.rs` | Not Started |
| Public transport/output tests | `tests/song_issued_transport.rs` (new) | Not Started |

Five candidate paths. Private owning fixtures belong inside declared modules;
external tests use public preparation/playback output. Keep touched files below1000.
Any required additional module must be declared before source release.

## Required interface contract

Ready owns PreparedRoutes and exposes its plan only through a shared reference.
Preserve public routes() and descriptor query(). Add private issued query and
resolution companions with complete typed signatures after prerequisites exist.
Resolution receives the original FrozenIssuedBatch plus an event index and
inherited work/depth. It authenticates the Ready original Song, complete query
transcript, every contributing invocation and actual selected member/site binding.
Do not replace this contract with equality of copied fields.

## Tasks

### TASK-001: Ready handoff

**Status**: Not Started
**Parallelizable**: No

- [ ] Finalize exact prerequisite resolver manifest and callable signatures.
- [ ] Retain issued PreparedRoutes without evaluator/VM ownership.
- [ ] Provide issued query/resolution while preserving public descriptor wrappers.
- [ ] Inherit actual remaining work and depth across query, freeze and resolution.

### TASK-002: Production scheduler consumption

**Status**: Not Started
**Parallelizable**: No; depends on TASK-001

- [ ] Query issued batches and sort whole envelopes by descriptor onset/handle.
- [ ] Replace unconditional handle-only dedup_by at the actual scheduler seam.
- [ ] Replace current per-event max_nodes/count allowances with one cumulative
  caller budget spanning query, freeze, all proof resolution and publication.
- [ ] Resolve all contributing proofs before coalescing duplicate musical events.
- [ ] Coalesce only consistent descriptors and authenticated equivalent routes;
  conflicting evidence fails without partial scheduled publication.
- [ ] Keep batch transcript alive through resolution; encode borrowed descriptors.
- [ ] Replace scalar route resolution with the real authenticated geometry consumer.

### TASK-003: Owning playback evidence

**Status**: Not Started
**Parallelizable**: No; depends on TASK-002

- [ ] Actual PreparedSong→Ready→scheduler→retained geometry fixture passes.
- [ ] Disable callback execution after retention and prove resolution performs no read.
- [ ] Equal-handle distinct proofs and conflicting routes exercise actual coalescing.
- [ ] Native/Arena output and exact route/onset/partition equivalence pass.
- [ ] Foreign/member/swap and cumulative exact/one-less failures preserve old playback.
- [ ] Independent tests/native/lint/WASM and frontend evidence pass held inputs.

## Completion criteria

- [ ] All tasks and actual production consumer fixtures pass.
- [ ] Public API compatibility and original budget/depth are preserved.
- [ ] No scalar-only routing fallback or unused proof owner counts as integration.
- [ ] Separate pre-Reserve admission and varying-seed requirements are fulfilled
  before declaring the full song-mode goal complete.

## Progress log

### 2026-10-03 — Actual scheduler seams identified

Reviewer identifies sched/song.rs442–467 querying, handle-only deduplication and
scalar routing as actual production proof-loss seams. Dependencies include a real
authenticated geometry resolver, not just frozen envelopes or PreparedRoutes.
Scope/signatures need author confirmation; no Rust edits or runtime evidence.

### 2026-10-03 — Budget reset confirmed at actual realization seam

Current realize queries with a full internal budget, then derives a fresh
max_nodes/count routing allowance and calls scalar resolve_route separately for
each event. Issued realization must pass one remaining counter through actual
query/freeze/resolution and debit every contribution before musical coalescing.
Neither averaging allowance nor public query's hidden fresh budget preserves the
original cumulative work contract. Validate the complete realization boundary
with exact/one-less work and no partial queue/pool publication on failure.
No source release; prerequisite live identity checkpoint is under repair.

### 2026-10-03 — Current scheduler publication and file headroom audit

Current Ready query forwards to PreparedSong's descriptor-only query; private
PreparedSong/SongSnapshot issued query companions already exist. Scheduler
realize currently deduplicates handles before proof resolution, divides a fresh
max_nodes allowance by descriptor count, selects scalar resolve_route, then
mutates pools per event. The issued replacement must resolve all complete
envelopes using the shared query/freeze/resolution ledger before any musical
coalescing or pool/queue publication. Validate all authenticated candidate routes
and stage allocation/output before committing the batch; an error in a later
event must not leave partial pool assignments or scheduled publication.

preparation.rs is903 lines and sched/song.rs614 at this audit. Keep every touched
file below1000; if cohesive extraction is required, declare its child in this
plan before release rather than silently adding an undeclared path. Actual admission
before Reserve and useful varying/first executions remain mandatory following
work; replacing descriptor query alone does not establish them. No source edits
or current Route8 scope expansion are released by this audit.

### 2026-10-03 — Declare transactional pool source before playback release

Actual PoolBook owns private Slot records and mutable pending rebound queues;
there is no clone/staging API available to sched/song.rs. Current realization
assigns pools then encodes/pushes each event, so a later error can leave earlier
physical projection changes. Add pools.rs explicitly as the fifth candidate
path for a bounded staging/commit API. Charge projection state copying before
allocation, preserve acknowledged/projected generations and pending receipts,
and publish the staged pool plus queued commands only after complete success.
No changed pool is installed on route, encoding, work or capacity failure.
Existing activation/acknowledgement behavior must remain compatible.
This is a future Planning scope declaration; current Route8 remains exact8.

## Session 249 executable contract (wave 3)

The source of truth is the design sections "Wave 3: issued playback" and
"Requirement-level end-to-end evidence", decisions D9-D12. This plan also
carries out TASK-003 of `song-mode-shared-issued-query-work.md`: Ready
realization uses one collector. A sixth path,
`src/host/caps/song/preparation/issued.rs`, is pre-declared for one use only:
if `preparation.rs` (903 lines) would reach 1000 lines, move the new issued
Ready/preparation helpers into it.

```json
{
  "planId": "SONG-ISSUED-PLAYBACK",
  "planPath": "impl-plans/active/song-mode-issued-playback.md",
  "wave": 3,
  "dependsOn": ["SONG-ROUTE8", "SONG-ISSUED-RESOLUTION", "SONG-SHARED-WORK", "SONG-STRUCTURAL-CLOCK"],
  "writePaths": [
    "src/host/caps/song/preparation.rs",
    "src/host/caps/song/preparation/issued.rs",
    "src/sched/song.rs",
    "src/sched/song/pools.rs",
    "src/song/routing/prepared.rs",
    "tests/song_issued_transport.rs",
    "impl-plans/active/song-mode-issued-playback.md",
    "tmp/song-mode-riela/session249-playback-intent.json",
    "tmp/song-mode-riela/session249-playback-receipt.json",
    "tmp/song-mode-riela/session249-playback-build.log",
    "tmp/song-mode-riela/session249-playback-clippy.log",
    "tmp/song-mode-riela/session249-playback-nextest-focused.log",
    "tmp/song-mode-riela/session249-playback-nextest-full.log",
    "tmp/song-mode-riela/session249-playback-wasm.log",
    "tmp/song-mode-riela/session249-playback-fmt.log"
  ],
  "sharedPaths": ["impl-plans/active/song-mode-shared-issued-query-work.md"],
  "sharedPathNotes": [
    {"path": "impl-plans/active/song-mode-shared-issued-query-work.md", "intendedEdit": "Tick the TASK-003 checkboxes and add one progress-log line pointing to this plan's evidence. Change nothing else."}
  ]
}
```

### Intent and context

This wave makes production consume issued routes, which is the user-visible
integration. Today the code works like this:

- `SongHostPreparation::prepare` (`preparation.rs:255-269`) charges a fixed
  1,000,000 and calls the legacy `prepare_routes`.
- `SongHostPreparation.routes` and `SongReadyBundle.routes` hold a
  `SongRoutePlan` (`preparation.rs:154`, `preparation.rs:164`). `routes()` and
  `query()` are public (`preparation.rs:859`, `preparation.rs:896`).
- `SongTransport::realize` (`sched/song.rs:437-503`):
  - queries public descriptors and sorts them;
  - runs `dedup_by(handle)`;
  - computes `allowance = max_nodes / count`;
  - calls the scalar `routing::resolve_route`;
  - mutates `self.pools` per event (`PoolBook::assign`, `pools.rs:27`);
  - pushes `self.pending`.

Wave 1 provides `PreparedSong::issue_retained_route_authority` and
`prepare_routes_issued`. Wave 2 provides
`PreparedRoutes::resolve_issued_event` and
`PreparedSong::query_issued_with_work`.

### Non-goals

- Keep the public `routes()`/`query()` signatures, the activation and
  acknowledgement protocol, and `encode::encode` (`src/sched/song/encode.rs`
  is not a writePath).
- No runtime first-time seed execution (design user-QA SM2).
- No changes to `SongLimits::default()`.
- No edits to the routing resolver, snapshot, replay, clock or the editor.
  Never edit, format or stage the unowned paths `src/song/snapshot/resources.rs`,
  `src/song/snapshot/reservations_tests.rs` or
  `src/sched/runtime/song/clock_tests.rs` (design: Unowned working-tree paths).
- No `allow`/`expect` attributes.

### File-level changes

1. **`preparation.rs` `prepare`**
   - Replace the fixed charge and `prepare_routes` with these steps:
     1. Set a local `u32` counter to
        `min(cleanup.remaining, 1_000_000)`, converted with a checked cast.
     2. Call `prepared.issue_retained_route_authority(limits.song, &mut local,
        0)`.
     3. Call `prepare_routes_issued(view, prepared.snapshot().settings(),
        caps, &report.available, limits.song with max_nodes = initial local,
        &mut local, 0)`. The settings must be the original Song's settings.
     4. Write `initial - local` back as a debit to `cleanup.remaining`, on both
        success and error.
   - All of this happens before `resources::build`, so before any Reserve or
     upload.
   - `resources::build`, `pools::finish` and `demand` receive
     `prepared_routes.plan()`.
2. **`preparation.rs`, ownership**
   - `SongHostPreparation.routes` becomes `Option<PreparedRoutes>` and
     `SongReadyBundle.routes` becomes `PreparedRoutes`. That includes the
     cancel/retire path at line ~845.
   - Public `routes()` returns `self.routes.plan()`.
   - Add the crate-private companions
     `query_issued_with_work(&mut self, span, work, depth)` and
     `resolve_issued(&self, batch, idx, work, depth)`, which forward to wave-2
     APIs.
3. **`pools.rs`**
   - Add `PoolBook::staged(&self, work: &SharedIndexWork) ->
     Result<PoolBook, Failure>`. It charges `slots.len()` plus the sum of the
     `expected` lengths plus 1 before cloning.
   - Keep `assign` unchanged so it works on the staged book.
   - No other API change.
4. **`sched/song.rs` `realize`**
   1. Create one collector:
      `CanonicalIndexCollector::new(self.limits.max_nodes, self.limits)`.
   2. Get the batch from `ready.query_issued_with_work(span, &work, 0)`, then
      run `check_events(batch.events().len())`.
   3. Sort the event indices by descriptor onset (`whole` or `part` begin),
      then by handle.
   4. Resolve every event with `ready.resolve_issued`. This happens before any
      coalescing.
   5. Coalesce equal handles only when their descriptors are equal and their
      routes are equal. Otherwise fail.
   6. Create `let mut staged = self.pools.staged(&work)?` and a local
      `Vec<Box<SongCommand>>`.
   7. Every queried envelope has been resolved by step 4. Now apply the
      existing onset window filter to each surviving event, before staged
      assignment: compute `onset = whole.unwrap_or(part).begin` and skip the
      event (not an error) when `onset < self.cursor || onset >= end ||
      onset >= self.duration`. For each event that passes, preserve the
      existing per-event body exactly:
      - look up the branch in `ready.routes().branches` by `route.branch` and
        fail with the same "logical song route missing" error if absent;
      - compute `component_end = route.configuration.end.min(self.duration)`;
      - call `staged.assign(route, self.map.at(onset)?,
        self.map.at(component_end)?, self.map.deadline(component_end)?)`;
      - call `encode::encode(ready, event.descriptor(), ...)` with the
        existing arguments;
      - push into the local vec in the order RebindBranch (if any), Release
        (if any), Event.
   8. Only after the loop succeeds: set `self.pools = staged`, extend
      `self.pending` and set `self.cursor = end`.
   - Remove `dedup_by`, the allowance and the scalar `resolve_route` from
     production.
5. **`prepared.rs`**: only the borrow and lifetime adjustments that Ready needs.
   Do not change the resolver semantics. If an item in `prepared.rs` is still
   reported as dead after wiring, it is in this plan's writePaths. Wire it
   through its planned consumer (Route8 D-row table) or record a blocker. Never
   delete it silently.
6. **`tests/song_issued_transport.rs` (new)**: public end-to-end tests, listed
   below.

### Code to imitate

- Public preparation and playback drivers: `tests/song_end_to_end.rs`
  (`candidate`, `play` and `backend_parity`), and its `EDITED` program at
  line 801 for Part edits.
- Export: `tests/song_export.rs`, using
  `vactr::song::export::{export_song, SongExportOptions}`.
- Program syntax: `examples/song-mode.vact`.
- Declared signatures: `design-docs/specs/design-song-mode.md` line 50
  ("Proposed declarations: ..."). Working syntax: `tests/song_checker.rs:110`,
  `tests/song_source_routes.rs:160` and
  `src/song/snapshot/occupancy/lookup_tests.rs:662`.
- Slice syntax: `tests/song_source_routes.rs` and
  `src/song/snapshot/occupancy/geometry_tests/domains.rs`
  (`repeat {slice {beat -> nil} 1 [cut]} 2`).

### Key pitfalls

- Never resolve after coalescing, and never deduplicate by handle alone.
- Do not drop the onset window filter; an envelope outside [cursor, end) or
  at/after duration is resolved but not encoded.
- Never mutate `self.pools`, `self.pending` or `self.cursor` before full
  success.
- Never fall back to scalar `resolve_route` when issued resolution fails.
- Keep the batch, and so its transcript, alive while resolving.
- Do not hold a collector `borrow_mut` across calls.
- A refusal from `issue_retained_route_authority` must happen before
  `resources::build`. Check that no Reserve or upload command was queued.
- A `RequiresUniformBound` readiness is not a refusal.
- If the end-to-end song is refused for lack of work under
  `SongLimits::default()`, record the measured work and follow user-QA SM1
  option (a). That means raising only the CLI/host preparation song allowance,
  which is outside this plan's paths, so stop and report it. Do not change
  defaults silently.

### Tests to add

`tests/song_issued_transport.rs` uses public APIs only. Use one program that
contains:

- two reusable Part generator functions, one calling the other;
- `sequence` with `{part-repeat p 2 seed-mode: :same}` and
  `{part-repeat p 2 seed-mode: :vary}` (keyword argument) over a
  random-dependent Part;
- `part-events` with `delete-event` of one chord tone;
- `overwrite-region`;
- `{transform-instrument base :drums :analog {p -> lpf p 900}}` (Part argument
  first, then track keyword, then sound selector, then `fn ctl -> ctl`);
- `{instrument-fx part :track :sound :bus-name}` on one instrument, with a
  declared `bus :name:` chain;
- one Slice-sourced track, combined with `seed-mode: :vary` in the same song
  as in `src/song/snapshot/occupancy/lookup_tests.rs:662`;
- `tail-seconds: 2`.

Tests on that program:

- `prepare` + Ready + `SongTransport` driven to completion with no cycles
  argument: the state reaches `Ended`, and the total frames equal the
  arrangement frames plus the tail frames exactly.
- Descriptor query: the deleted tone is absent and its siblings are present.
  The overwritten region holds the replacement onsets.
- The `:same` repeats are equal in notes and controls. The `:vary` repeats
  differ in at least one random decision.
- `ready.routes()`: only the selected instrument branch carries the effect
  template, and the sibling branch does not.
- `export_song` writes a finalized WAV. The frame count and metadata match, and
  the result is bit-exact on a repeat export.
- The existing `tests/song_end_to_end.rs` and `tests/song_export.rs` pass
  unchanged.

Private fixtures in a `#[cfg(test)] mod` inside `sched/song.rs` (crate-private
access):

- Make the last event's route fail through a forged foreign batch, and make a
  capacity failure. In both cases the pools, pending queue and cursor are
  byte-equal to their state before `realize`.
- Callback reads stay at 0 during `realize` resolution after retention.
- Equal-handle events with conflicting routes fail.
- An event whose whole begins before the window (or at/after duration) is
  resolved but not re-emitted; the local command vector contains only
  in-window events.
- Exact and one-less collector work for one realization window: exact
  succeeds; one less fails with no publication.

Private fixture in `preparation.rs`'s existing test module:

- A foreign or missing authority refuses before any Reserve.

### Verification (run in the foreground; record the exit status and full log path)

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-playback-build.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-playback-clippy.log 2>&1`
  must exit 0. This is the first strict gate, so every Route8 and wave-2
  dead-code item must be consumed by now. Import `PreparedRoutes` and
  `prepare_routes_issued` as `crate::song::routing::X`, which consumes the
  rest of Route8 row D01. If the log still shows a diagnostic in a path outside
  this plan's writePaths (for example `src/song/routing.rs` or a wave-2 path),
  do not edit that path. Record it in the receipt as
  `{file, line, message, rowId}` and stop with a blocker. The root reviewer
  then repairs it serially at the wave-3 join (dispatch `joinProtocol`),
  limited to removing or re-pathing the named unused import or item, and reruns
  the gate.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_issued_transport --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker --test song_export > tmp/song-mode-riela/session249-playback-nextest-focused.log 2>&1`
  must exit 0 with a nonzero count for every binary.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session249-playback-nextest-full.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-playback-wasm.log 2>&1` must exit 0.
- `rustfmt --edition 2021 --check <touched Rust writePaths> > tmp/song-mode-riela/session249-playback-fmt.log 2>&1`
  passes when no `Diff in` line names a touched path.
- `wc -l` on every touched Rust path: each must be below 1000.
- `grep -n "dedup_by\|resolve_route(" src/sched/song.rs` must print nothing.

### Shared-branch protocol

Same as wave 2: an intent JSON with original texts and hashes, a hash check
before every edit, no crate-wide format, no stash/checkout/reset, and updates
to this plan's log plus the one shared TASK-003 note.

### Done criteria

- [ ] Strict clippy exits 0.
- [ ] Every test above is present and passes.
- [ ] The scheduler has no scalar fallback (grep is empty).
- [ ] Ready owns `PreparedRoutes`, and the public `routes()`/`query()` are
  unchanged in signature.
- [ ] The pre-Reserve refusal fixture passes.
- [ ] Build, full tests and WASM exit 0, and fmt is clean.
- [ ] Every file is below 1000 lines.
- [ ] The progress log is updated.

## Session 251 amendment (wave 3)

- The fifth Rust path is `src/sched/song/pools.rs` (`PoolBook`/`Slot`), as
  declared in the manifest. `src/host/caps/song/preparation/pools.rs` is not a
  playback writePath and must not be edited.
- This wave starts only after 2a, 2b and 2c are joined and committed. It runs
  alone (concurrency 1).
- Strict `cargo clippy --all-targets -- -D warnings` must exit 0. Any `SW-`,
  `ST-` or `RES-` row still present at entry is a blocker for its owning
  sub-wave. This plan does not fix it.
- The Index/Slice-sourced track in `tests/song_issued_transport.rs` uses a
  statically admissible Index list (user-QA SM4 default (a)).

## Session 252 amendment (wave 3)

Source: the design section "Session 252 resume amendments" (Downstream seams,
operator authorization 2). The writePaths, tasks and acceptance criteria are
unchanged. One seam is declared up front, and it may be edited only under its
stated condition. `PreparedSong::issue_retained_route_authority`
(`route_view.rs:486`) is already `pub(crate)`, so no seam is declared on
`route_view.rs`.

- **`src/song/routing.rs`** (a Route8 path). The Route8 disposition table
  leaves an `unused_imports` diagnostic on the `PreparedRoutes` and
  `prepare_routes_issued` re-exports (`routing.rs:623`). This plan wires their
  consumers, which is expected to clear the diagnostic. Edit `routing.rs` only
  if strict Clippy still reports `unused_imports` there after `preparation.rs`
  and `sched/song.rs` name these items as `crate::song::routing::X`. The only
  allowed edit is to the `routing.rs:623` re-export line. The receipt then
  records the before and after diagnostics. Otherwise record
  `"routing.rs": "declared, unedited"`.

The seam allows no semantic change. SONG-ROUTE8 tests (`snapshot::occupancy`
and `routing::prepared`) must pass unchanged. The 2a site aliases need no
playback change, because only the site-scoped selector inside
`resolve_issued_event` enumerates them.

Done criteria added:

- [ ] The receipt has a `seams` entry for `routing.rs`.
  `git diff <2c-join-commit> -- src/song/routing.rs` is empty or limited to
  the one re-export line.
- [ ] Strict all-target Clippy exits 0 (unchanged criterion).

## Session 254 amendment (runs FOURTH, after 2c, 2a and 2b are accepted)

The source of truth is the design section "Session 254 resume amendments
(2026-10-04)", subsection "Carried forward unchanged", together with the
existing Wave 3 sections. Scope, writePaths, sharedPaths, tests and done
criteria are unchanged. Only these facts change:

- **Order and base.** The serial order is now 2c, 2a, 2b, then this plan.
  `dependsOn` is unchanged. Every `<2c-join-commit>` base in the session 252
  section becomes the 2b join commit (`<2b-join>`), which is the last commit
  before this plan starts.
- **Cohort.** The cohort is 953 at entry. It becomes 954 with
  `tests/song_issued_transport.rs`, or 955 if
  `src/host/caps/song/preparation/issued.rs` is created.
- **Full suite.** Every full nextest run adds `--no-fail-fast`. It must exit 0
  with every test run and at least 425 distinct tests passed. Strict
  all-target Clippy must exit 0. No `SW-`, `ST-` or `RES-` row may remain.
- **Pool atomicity (unchanged, restated because it is adversarially
  reviewed).**
  - `src/sched/song/pools.rs` stages projections and encoded commands in a
    charged staging copy.
  - Commit swaps the stage in and pushes the commands only after the whole
    batch succeeds.
  - On any route, encoding, work or capacity failure, the old pools, queue,
    cursor and pending receipts stay byte-identical.
  - The failure-path test must assert that unchanged state explicitly.
- **Issued routes in production.** `grep -n "dedup_by\|resolve_route(" src/sched/song.rs`
  prints nothing. The scheduler resolves every envelope and contributor
  through `PreparedRoutes::resolve_issued_event` before coalescing.
- **Evidence.** Copy `tmp/song-mode-riela/session249-playback-*` to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session253/` with sha256 values
  before rerunning. The new receipt fingerprint must differ from every earlier
  playback receipt hash.
- **Unowned paths.**
  `git diff --quiet <2b-join> -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs`
  must exit 0.

### Session 254 done criteria

- [ ] Every session 249-252 done criterion of this plan holds, with
  `<2b-join>` as the diff base.
- [ ] Full nextest with `--no-fail-fast` and strict Clippy both exit 0.
- [ ] One progress-log entry is added.

## Session 255 amendment (runs FOURTH, after 2c, 2a and 2b are joined; serial)

The source of truth is the design section "Session 255 resume amendments
(2026-10-04)": "Implementer authority" and "Remaining waves" > Wave 3, plus
the existing "Wave 3: issued playback" and "Requirement-level end-to-end
evidence" sections. The file-level changes, tests and done criteria from
session 249-254 are unchanged. This amendment changes only the stop rules,
declares one more seam, and sets the evidence locations. The diff base is
`<2b-join>`.

### Intent and context

- The production scheduler (`SongTransport::realize` in `src/sched/song.rs`,
  614 lines) must stop using `dedup_by`, the per-event allowance and the
  scalar `resolve_route`. It consumes the issued batch and resolves every
  envelope and contributor through `PreparedRoutes::resolve_issued_event`
  before coalescing.
- `src/sched/song/pools.rs` (151 lines, the plan's fifth path) gets a charged
  staged copy (`PoolBook::staged`). All assignments and commands go to the
  stage. Commit (`self.pools = staged`, extend `pending`, set `cursor`)
  happens only after the whole batch succeeds. Any failure leaves the old
  pools, queue, cursor and pending receipts byte-identical.
- `preparation.rs` (903 lines) retains authority and prepares issued routes
  before `resources::build`. That means before any Reserve or upload.
- `tests/song_issued_transport.rs` is the requirement-level end-to-end proof
  of the user goal.

### Implementer authority (replaces earlier stop clauses)

- Diagnose each failing gate and fix it inside the manifest below. Rerun the
  focused command until it passes, then run every gate. Record each fix in the
  receipt under `fixes[]` with: the test, the exact message, the root cause,
  the edited paths, and the class.
- Stop only for:
  1. a fix outside the manifest. This covers the SM1 case (the end-to-end
     song needs a larger host song allowance outside these paths) and a
     strict-Clippy diagnostic in a path outside the manifest, which the root
     reviewer repairs at the join;
  2. a change to an assertion of a test that exists at `37ea3e8`. This
     includes `tests/song_end_to_end.rs`, `tests/song_export.rs` and the
     existing `preparation.rs` and `sched/song.rs` tests;
  3. relaxing an identity check, adding a scalar or handle-only fallback, or
     a non-atomic pool commit.
- **Earlier clause that becomes "diagnose and fix":** "If an item in
  `prepared.rs` is still reported as dead after wiring ... record a blocker".
  Wire the item through its planned consumer in this plan's paths. Delete it
  only if the Route8 D-row table names no consumer, and record that in
  `fixes[]`.
- Own tests (`tests/song_issued_transport.rs` and the new private fixtures)
  may be corrected toward the design. Each required assertion in "Tests to
  add" stays.

### Owned paths for session 255

```json
{
  "planId": "SONG-ISSUED-PLAYBACK",
  "planPath": "impl-plans/active/song-mode-issued-playback.md",
  "dependsOn": ["SONG-ROUTE8", "SONG-STRUCTURAL-CLOCK", "SONG-ISSUED-RESOLUTION", "SONG-SHARED-WORK"],
  "writePaths": [
    "src/host/caps/song/preparation.rs",
    "src/host/caps/song/preparation/issued.rs",
    "src/sched/song.rs",
    "src/sched/song/pools.rs",
    "src/sched/song/realize_tests.rs",
    "src/song/routing/prepared.rs",
    "tests/song_issued_transport.rs",
    "impl-plans/active/song-mode-issued-playback.md",
    "tmp/song-mode-riela/session249-playback-intent.json",
    "tmp/song-mode-riela/session249-playback-receipt.json",
    "tmp/song-mode-riela/session249-playback-build.log",
    "tmp/song-mode-riela/session249-playback-clippy.log",
    "tmp/song-mode-riela/session249-playback-nextest-focused.log",
    "tmp/song-mode-riela/session249-playback-nextest-full.log",
    "tmp/song-mode-riela/session249-playback-wasm.log",
    "tmp/song-mode-riela/session249-playback-fmt.log"
  ],
  "sharedPaths": [
    "src/song/routing.rs",
    "impl-plans/active/song-mode-shared-issued-query-work.md"
  ],
  "sharedPathNotes": [
    {"path": "src/song/routing.rs", "intendedEdit": "Only the PreparedRoutes/prepare_routes_issued re-export line (about line 623), and only if strict Clippy still reports unused_imports after wiring (session 252 rule)."},
    {"path": "impl-plans/active/song-mode-shared-issued-query-work.md", "intendedEdit": "Tick TASK-003 and add one progress-log line pointing to this plan's evidence."}
  ]
}
```

- **Conditional new files.** Each is created only when needed, and each adds
  one to the cohort, which the receipt records:
  - `src/host/caps/song/preparation/issued.rs`, only if `preparation.rs`
    would reach 1000 lines;
  - `src/sched/song/realize_tests.rs` (new in session 255), only if the
    private `realize` fixtures would push `sched/song.rs` to 1000 lines. It
    is registered as `#[cfg(test)] mod realize_tests;` in `sched/song.rs`, and
    the fixtures move there unchanged.
- `src/host/caps/song/preparation/pools.rs` and `src/sched/song/encode.rs`
  are not writePaths.
- Frozen: every 2c, 2a and 2b path, the three unowned paths, editor and
  canvas files, and `.agents/settings.local.json`.

### Key pitfalls (restated for the adversarial review)

- Stage before mutating. `PoolBook::staged` charges `slots.len()` plus the sum
  of the `expected` lengths plus 1, before cloning. `assign` runs only on the
  stage.
- Resolve every envelope before coalescing. Coalesce equal handles only when
  their descriptors and authenticated routes are equal; otherwise fail.
- Keep the onset window filter. An envelope outside `[cursor, end)`, or at or
  after the duration, is resolved but not encoded.
- Never fall back to `resolve_route`, and never `dedup_by` handle.
- Write the actual preparation debit back to `cleanup.remaining` on both
  success and failure.
- A static Song pre-retains every `:vary`/`:same` placement's first
  execution. Runtime replay therefore never needs an unseen seed (SM2 stays
  out of scope).

### Session 255 execution steps (in order)

1. Copy `tmp/song-mode-riela/session249-playback-*` to
   `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session254/` with sha256
   values. Scratch logs go under `tmp/song-s249/SONG-ISSUED-PLAYBACK/session255/`.
2. Write the intent JSON with the design and plan sha256 values and the
   sha256 and full text of each file to edit. Hash-check before every edit.
3. Implement file-level changes 1-6 from the session 249 contract, in order:
   preparation, ownership, pools, `realize`, `prepared.rs`, then the
   end-to-end tests. Run `CARGO_TERM_QUIET=true cargo build` after each.
4. Run verification 1-9, fixing failures under the authority rule.
5. Write the receipt `tmp/song-mode-riela/session249-playback-receipt.json`
   with:
   - `fixes[]`;
   - `seams` (`routing.rs`, `preparation/issued.rs`, `realize_tests.rs`, each
     as edited/created or "declared, unedited/uncreated");
   - the measured end-to-end work;
   - the cohort;
   - the line counts;
   - the unowned-path result;
   - `evidenceFingerprint`. It must differ from every hash in
     `tmp/song-s249/SONG-ISSUED-PLAYBACK/`.
6. Tick TASK-003 in the shared-work plan, and add one progress-log entry
   here.

### Session 255 tests (input -> expected outcome)

These are the session 249 "Tests to add", unchanged:

- The static program (reusable Part functions with nested reuse, `sequence`,
  `part-repeat` with `:same` and `:vary`, `delete-event`, `overwrite-region`,
  `transform-instrument` with `lpf`, `instrument-fx`, a Slice-sourced track
  with a static Index list, `tail-seconds: 2`) driven through public `prepare`,
  Ready and `SongTransport` with no cycle argument -> it ends on its own, and
  the frames equal the arrangement frames plus the tail frames exactly.
- Deleted tone -> absent, with its siblings present. Overwritten region ->
  holds the replacement onsets.
- `:same` repeats -> equal. `:vary` repeats -> differ in at least one random
  decision, while authenticating distinct executions.
- Effect -> only on the selected instrument branch.
- `export_song` -> a finalized WAV whose frames and metadata match, and the
  output is bit-exact on a repeat export.
- Forged foreign batch, or a capacity failure on the last event -> pools,
  pending queue and cursor are byte-equal to their state before `realize`.
- Equal-handle events with conflicting routes -> fail.
- Exact work for one window succeeds. One less fails with no publication.
- A foreign or missing authority -> refused before any Reserve.
- `tests/song_end_to_end.rs` and `tests/song_export.rs` -> unchanged and
  passing.

### Session 255 verification (foreground; record the exit status and full log path)

1. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast --test song_issued_transport --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker --test song_export > tmp/song-mode-riela/session249-playback-nextest-focused.log 2>&1`
   must exit 0, with a nonzero count for every binary.
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib -E 'test(/sched::song|caps::song::preparation|routing::prepared/)' >> tmp/song-mode-riela/session249-playback-nextest-focused.log 2>&1`
   must exit 0.
3. `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-playback-build.log 2>&1`
   must exit 0.
4. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-playback-clippy.log 2>&1`
   must exit 0.
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-mode-riela/session249-playback-nextest-full.log 2>&1`
   must exit 0, with every test run and at least 425 distinct tests passed.
6. `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-playback-wasm.log 2>&1`
   must exit 0.
7. `rustfmt --edition 2021 --check src/host/caps/song/preparation.rs src/sched/song.rs src/sched/song/pools.rs src/song/routing/prepared.rs tests/song_issued_transport.rs <each edited or created conditional path> > tmp/song-mode-riela/session249-playback-fmt.log 2>&1`.
   It passes when no `Diff in` hunk covers a changed line of an owned path.
8. `wc -l` on every touched Rust path: each is below 1000.
9. Checks:
   - `grep -n "dedup_by\|resolve_route(" src/sched/song.rs` prints nothing;
   - `git diff --quiet <2b-join> -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs src/sched/song/encode.rs src/host/caps/song/preparation/pools.rs`
     exits 0;
   - `git ls-files -co --exclude-standard -- '*.rs' Cargo.toml Cargo.lock | wc -l`
     prints 954, plus one for each conditional file created.

### Session 255 done criteria (mechanically checkable)

- [ ] Every session 249-254 done criterion of this plan holds, with
  `<2b-join>` as the base.
- [ ] `grep -n "fn staged" src/sched/song/pools.rs` matches, and the
  failure-path test asserts unchanged pools, pending and cursor.
- [ ] Verification 1-6 exit 0. Verification 7 and 8 meet their rules.
  Verification 9 prints nothing, exits 0 and gives the expected count.
- [ ] `git diff <2b-join> | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
- [ ] The receipt has `fixes[]`, `seams`, the measured work and a new
  fingerprint. TASK-003 is ticked in the shared-work plan, and one
  progress-log entry is added.

## Session 256 amendment (runs FOURTH, after 2c, 2a and 2b are accepted; serial)

The source of truth is the design section "Session 256 resume amendments
(2026-10-04)". The session 255 amendment stays in force: the owned paths
including `src/sched/song/pools.rs`, the conditional seams, the tests,
verification 1-9 and the done criteria. The base is still `<2b-join>`, the
commit that records 2b acceptance. Only these items change.

- **Allowed failures.** None. Before any edit, run
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-s249/SONG-ISSUED-PLAYBACK/session256/baseline-full.log 2>&1`
  on `<2b-join>` and record `baselineFailures` in the receipt. The expected
  list is empty. A non-empty list is stop condition 1: report each test with
  its owner. Verification 4 (strict clippy) and verification 5 (full nextest)
  must both exit 0.
- **Evidence.** Step 1 copies `tmp/song-mode-riela/session249-playback-*` to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session255/` with `sha256.txt`.
  Scratch logs go to `tmp/song-s249/SONG-ISSUED-PLAYBACK/session256/`. The
  fingerprint must differ from every hash under
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/`.
- **Unchanged requirements, restated for the implementer:**
  - `SongTransport::realize` in `src/sched/song.rs` resolves every issued
    event through `PreparedRoutes::resolve_issued_event`, with no `dedup_by`
    and no scalar `resolve_route(`.
  - `src/sched/song/pools.rs` stages projections and commands
    (`PoolBook::staged`). Pools, the pending queue, the cursor and pending
    receipts are committed only after the whole batch succeeds. The failure
    test asserts they are byte-equal to their state before `realize`.

### Session 256 done criteria

- [ ] Every session 255 done criterion holds.
- [ ] `baselineFailures` is recorded and empty.
- [ ] Verification 4 and 5 exit 0. Verification 5 has zero failures and a
  `Summary` line.
- [ ] `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session255/sha256.txt` exists,
  and the receipt fingerprint is new.

## Session 257 amendment (runs FOURTH, after 2c, 2a and 2b are accepted; serial)

The source of truth is the design section "Session 257 resume amendments
(2026-10-04)" > "Later waves". The session 255 and 256 amendments stay in
force: the owned paths including the fifth path `src/sched/song/pools.rs`, the
conditional seams, the zero-allowed-failure rule, `baselineFailures`, strict
Clippy exit 0, the issued-route scheduler and the staged atomic pool commit.
Only the evidence locations change:

- Step 1 copies `tmp/song-mode-riela/session249-playback-*` to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session256/` with `sha256.txt`.
  Do not create, delete or overwrite earlier attempt directories.
- The baseline run and scratch logs go to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/session257/`.
- The fingerprint must differ from every hash under
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/`.
- `tests/song_export.rs` stays in the focused gate. Its 2c harness edit is not
  a wave 3 path, and wave 3 must not edit it.

### Session 257 done criteria

- [ ] Every session 256 done criterion holds, with `session257/` in place of
  `session256/`.
- [ ] `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session256/sha256.txt` exists.

## Session 258 amendment (runs FOURTH, after 2a, 2c and 2b are accepted; serial)

The source of truth is the design section "Session 258 resume amendments
(2026-10-04)" > "Serial order and dependency edge" and "Later waves". The
session 255 to 257 amendments stay in force:

- the owned paths, including the fifth path `src/sched/song/pools.rs`;
- the conditional seams (`src/song/routing.rs` re-export,
  `src/host/caps/song/preparation/issued.rs`, `src/sched/song/realize_tests.rs`);
- the tests and verification 1-9;
- `baselineFailures` and the zero-allowed-failure rule;
- strict Clippy exit 0;
- the issued-route scheduler and the staged atomic pool commit.

Only these items change:

- **Base.** `<2b-join>` means `<2b-accepted>`, the commit that records 2b
  acceptance. That commit contains the accepted 2a, 2c and 2b work. The
  `dependsOn` list is unchanged.
- **Baseline.** Before any edit, run
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-s249/SONG-ISSUED-PLAYBACK/session258/baseline-full.log 2>&1`
  on `<2b-accepted>`. It must exit 0, and the receipt records
  `baselineFailures: []`. A non-empty list is stop condition 1.
- **No retries.** No nextest `--retries`, and no rerunning the full suite
  until it happens to pass.
- **Strict Clippy.** `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
  exits 0. The remaining Route8 dead-code rows (`LookupAuthority::Issued`,
  `with_work`, `bind_issued_owner`, `PreparedRoutes`) and the `RES-` rows are
  cleared by production use from `src/host/caps/song/preparation.rs` and
  `src/sched/song.rs`, never by `allow`/`expect` or by deleting the items. A
  residual diagnostic outside the playback paths is a blocker that the root
  reviewer repairs serially at the join; the worker never repairs it.
- **Reviews.** The reviews and the receipt field `reviewDiffRange` cover
  `git diff 1ac457f <wave3-accepted> -- <playback writePaths and edited seams>`.
- **Evidence.** Step 1 copies `tmp/song-mode-riela/session249-playback-*` to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session257/` with `sha256.txt`.
  Never delete or overwrite earlier directories. Scratch logs go to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/session258/`. The receipt has
  `session: 258` and `baseCommit`. Its fingerprint differs from every hash
  under `tmp/song-s249/SONG-ISSUED-PLAYBACK/`.

Tests (input -> expected outcome), restated:

- The `tests/song_issued_transport.rs` program (nested reusable Part
  functions, `sequence` with `part-repeat :same` and `:vary`, `delete-event`,
  `overwrite-region`, `transform-instrument` `lpf`, `instrument-fx`, a
  Slice-sourced track, `tail-seconds 2`) -> the transport reaches `Ended`
  with exact frames, and the WAV export is bit-exact on a repeat.
- An injected failure in the middle of a `realize` batch -> pools, the pending
  queue, the cursor and pending receipts are equal to their state before
  `realize`.
- `grep -n "dedup_by\|resolve_route(" src/sched/song.rs` -> prints nothing.

### Session 258 done criteria

- [ ] Every session 256 done criterion holds, with `session258/` in place of
  `session256/` and `<2b-accepted>` in place of `<2b-join>`.
- [ ] `baseline-full.log` exits 0, and `baselineFailures` is empty.
- [ ] Strict Clippy (verification 4) exits 0. Full nextest (verification 5)
  exits 0 with a `Summary` line and 0 failures.
- [ ] `git diff <2b-accepted> | grep -E '^\+.*#\[(allow|expect)'` prints
  nothing.
- [ ] `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session257/sha256.txt`
  exists. The receipt has `session: 258`, `reviewDiffRange` and a new
  fingerprint.
- [ ] One progress-log entry is added.

## Session 259 amendment (runs FOURTH, on <2b-accepted>; serial)

The source of truth is the design section "Session 259 resume amendments
(2026-10-04)", subsection "Later waves". The session 258 amendment applies
unchanged:

- `src/sched/song.rs` consumes issued routes through
  `PreparedRoutes::resolve_issued_event`. It contains no `dedup_by` and no
  `resolve_route(`.
- `src/sched/song/pools.rs` stages projections and commands. It commits only
  after the whole batch succeeds. The failure test asserts that the old pools,
  queue, cursor and pending receipts are unchanged.
- Strict Clippy exits 0 through production use of the Route8 dead-code items,
  never through `allow`/`expect`.
- Full nextest exits 0 with 0 failures.

This amendment changes only the following:

- **Base.** `<2b-accepted>` is the session 259 commit that records
  SONG-SHARED-WORK acceptance.
- **Evidence.** Copy the prior `tmp/song-mode-riela/session249-playback-*`
  files to `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session258/` with
  `sha256.txt`. Baseline and scratch logs go to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/session259/`.
- **Receipt.** It records `session: 259`, `baseCommit: <2b-accepted>`,
  `reviewDiffRange` (`git diff 1ac457f <wave3-accepted> -- <playback paths>`)
  and a new fingerprint.
- **Issued resolution contract.** The scheduler calls `resolve_issued_event`
  as-is. It must not re-implement or bypass the 2a owner-revision,
  cross-seal or owner-local-window rules. A failure that needs a 2a path is
  stop condition 1.

### Session 259 done criteria

- [ ] Every session 258 done criterion holds on `<2b-accepted>`.
- [ ] `tmp/song-s249/SONG-ISSUED-PLAYBACK/session259/baseline-full.log` shows
  0 failures.
- [ ] `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session258/sha256.txt`
  exists. The receipt has `session: 259` and a new fingerprint.
- [ ] One progress-log entry is added.

### Session 259 implementation attempt — blocked at export capacity

- Implemented metered issued authority and `PreparedRoutes` ownership in
  `preparation.rs`; actual local work debit is written back before resource
  construction on success and error.
- Implemented Ready's shared-work query/resolution forwarders, issued batch
  resolution before equal-handle reconciliation in `sched/song.rs`, and charged
  pool staging before local command publication in `pools.rs`.
- Added `tests/song_issued_transport.rs` with the required edits, `:same` and
  `:vary` seed-dependent output checks, Slice source, instrument filter/effect,
  and two-second tail. The pre-edit full baseline passed (2,793 passed,
  0 failed, 3 skipped).
- The current integration test reaches export, but `export_song` refuses the
  required program with `insufficient measured song capacity: bus_slots`.
  Attempts with fewer routed branches and a stateless named bus still refuse.
  Resolving this through export host capacity requires `src/song/export.rs`,
  which is outside the declared playback write paths. Stop until that write
  path is explicitly assigned or the accepted test contract is revised.
- Full current-source nextest, strict Clippy, WASM, formatter, private scheduler
  atomicity fixtures, and final receipt remain incomplete. Completion criteria
  remain unchecked. Evidence: `tmp/song-s249/SONG-ISSUED-PLAYBACK/session259/`.

### Session 260 implementation attempt — capacity resolved, resolver dependency blocked

- Addressed the integration review finding in `tests/song_issued_transport.rs`.
  The fixture now measures route capacity from `SongRoutePlan.required.bus_slots`
  and independently verifies `tracks + 1 + sum(reserved_generations)`. Final
  measured values are 2 tracks + 1 master + 28 reserved generations = 31 bus
  slots, within the exporter's fixed 32 slots. The tested program retains nested
  reusable Part functions, `:same` and `:vary` repeats, delete/overwrite edits,
  `lpf`, instrument FX, a Slice-sourced track and `tail-seconds 2`.
- The focused fixture reaches production export after its capacity, event-edit,
  and repeat-signature assertions pass. Export then fails with `foreign source
  child scope` at beat 4, raised from `src/pattern/eval/song_provenance.rs:608`.
  That provenance path is outside `SONG-ISSUED-PLAYBACK` writePaths, so the
  playback worker cannot repair the resolver dependency here.
- Focused current-source evidence is in
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/session260/nextest-final-focused.log`.
  Earlier attempts and their measured outcomes remain in the same session
  directory; no prior evidence was overwritten.
- Strict Clippy, current-source full nextest, WASM build, formatting gate,
  private preparation/atomicity fixtures and final acceptance receipt remain
  incomplete because the required export fails in the predecessor-owned source
  provenance path. Assigned completion criteria remain unchecked.
- Resume when the source-provenance owner repairs the admitted child-scope
  failure or an approved plan explicitly assigns its path; then rerun the
  current-source focused export, full nextest, strict Clippy, WASM and fmt gates.
- A fixture-only unique Slice track experiment was also measured: it raised
  legacy bus demand to 32 and issued export then refused `bus_slots`. The final
  fixture uses the same `hats` track name, requires 31 slots, passes issued
  capacity admission, and remains blocked at source-provenance child validation.
- Final restored-source verification: `cargo check --test song_issued_transport`
  exits 0 (`session260/final-source-check.log`); focused nextest runs one test
  and fails it at export (`session260/final-source-nextest.log`), recording
  31 required bus slots and `foreign source child scope` at beat 4. The scoped
  `rustfmt --check` for `tests/song_issued_transport.rs` exits 0
  (`session260/fmt-check-final.log`). Full nextest, strict Clippy, WASM,
  build, and private atomicity/preparation fixtures were not run on this source.
- Authoritative own-run final-source logs with explicit exit markers:
  `session260/verified-final-cargo-check.log` (0),
  `session260/final-reverted-nextest.log` (100; 1 run, 0 passed, 1 failed),
  and `session260/final-source-fmt.log` (0). Fixture source SHA256:
  `bd5108d08788780552f8cf6933697c27325447ffdb085d660f9d44ae6e376d4a`.

## Session 260 amendment (runs FIRST, on 10c3eab; serial)

The source of truth is the design section "Session 260 resume amendments
(2026-10-04)" in `design-docs/specs/design-song-mode.md`. The manifest entry
is `resumeSession260` in `impl-plans/active/song-s249-dispatch.json`. Earlier
amendments still apply where this one does not override them.

### Intent and context

The scheduler and Ready work from earlier sessions is already in `10c3eab`:

- `SongHostPreparation::prepare` (`src/host/caps/song/preparation.rs:256-287`)
  issues retained authority on a local counter. It writes the debit back
  before `routes_result?`.
- `SongTransport::realize` (`src/sched/song.rs:438-503`) uses one collector,
  resolves every event, then reconciles equal handles. It stages pools through
  `PoolBook::staged` and commits pools, pending and cursor only at the end.

Two defects block acceptance:

1. Export fails with `foreign source child scope` at beat 4. The cause is
   `source_owns_owner` in `src/pattern/eval/song_clock.rs:192-257`: it compares
   the edited Part's duration (4) with an overwrite-region owner frame's
   duration (1).
2. Export builds a generic 32-bus-slot engine instead of the song profile.

Session 259 integration review also found these checks missing:

- atomicity, authority, Ended and bit-exact tests;
- the gates;
- TASK-003 evidence;
- evidence copies;
- measured capacity in the receipt.

This amendment closes all of them. Do not rewrite code that already meets the
contract.

### Non-goals

- Do not change `SONG_BUS_SLOTS`, `SONG_TEMPLATE_SLOTS`, the generic
  `EngineConfig` bus-slot default, or the two-generation tail reservation
  (`src/song/routing/prepare.rs:189-211`).
- Do not shrink the `tests/song_issued_transport.rs` program further. Do not
  remove any requirement element or tail.
- Do not fix the out-of-scope resolution defect `required issued execution is
  not retained at site`, unless a required test reaches it.
- Do not touch these files:
  - `src/song/snapshot/resources.rs`;
  - `src/song/snapshot/reservations_tests.rs`;
  - `src/sched/runtime/song/clock_tests.rs`;
  - `src/pattern/eval/song_clock/dispatch.rs`;
  - `src/pattern/eval/song_clock/structural_tests.rs`;
  - `src/pattern/combinators/structure.rs`;
  - `.agents/settings.local.json`;
  - anything under `editor/`.
- Do not create `src/host/caps/song/preparation/issued.rs` unless
  `preparation.rs` (941 lines) would reach 1000 lines.
- Never run `cargo fmt` or rustfmt in write mode. Never run `git stash`,
  `git checkout`, `git restore` or `git reset`.

### Owned paths for session 260

```json
{
  "planId": "SONG-ISSUED-PLAYBACK",
  "planPath": "impl-plans/active/song-mode-issued-playback.md",
  "dependsOn": ["SONG-ROUTE8", "SONG-ISSUED-RESOLUTION", "SONG-SHARED-WORK", "SONG-STRUCTURAL-CLOCK"],
  "writePaths": [
    "src/host/caps/song/preparation.rs",
    "src/host/caps/song/preparation/issued.rs",
    "src/sched/song.rs",
    "src/sched/song/pools.rs",
    "src/sched/song/realize_tests.rs",
    "src/song/routing/prepared.rs",
    "tests/song_issued_transport.rs",
    "src/pattern/eval/song_clock.rs",
    "src/song/export.rs",
    "impl-plans/active/song-mode-issued-playback.md",
    "tmp/song-mode-riela/session249-playback-intent.json",
    "tmp/song-mode-riela/session249-playback-receipt.json",
    "tmp/song-mode-riela/session249-playback-build.log",
    "tmp/song-mode-riela/session249-playback-clippy.log",
    "tmp/song-mode-riela/session249-playback-nextest-focused.log",
    "tmp/song-mode-riela/session249-playback-nextest-full.log",
    "tmp/song-mode-riela/session249-playback-wasm.log",
    "tmp/song-mode-riela/session249-playback-fmt.log"
  ],
  "sharedPaths": [
    "impl-plans/active/song-mode-shared-issued-query-work.md",
    "src/song/routing.rs",
    "src/pattern/eval/song_provenance.rs",
    "src/song/snapshot.rs",
    "src/song/snapshot/issued.rs",
    "src/host/caps/song/preparation/pools.rs",
    "src/host/song_profile.rs",
    "src/song/routing/nested/issued/members.rs",
    "src/song/snapshot/occupancy/lookup/authority.rs"
  ]
}
```

Every sharedPath except the shared-work plan file is conditional. Edit one only
for a concrete defect that a required test or strict Clippy reproduces. Record
each such edit in `fixes[]` with the test or diagnostic and its exact message.
The manifest `sharedPathNotes` give each path's limits. If a fix needs any
other path, stop with stop condition 1 and report the owner and the failing
test.

### TASK-101: Overwrite-region owner duration (song_clock.rs)

**Status**: Not Started. **Parallelizable**: No (serial worker).

- **Target.** Only the body of `source_owns_owner`
  (`src/pattern/eval/song_clock.rs:192-257`). Its signature does not change.
- **Change.**
  - Add a local `payload_duration: Option<Ratio64>`, initially `None`.
  - In the `PartNode::Edit` arm, split
    `PartEdit::OverwriteRegion { track: selected, region, pattern }` out of the
    or-pattern. When `*selected == track`, set `payload = Some(pattern)` and
    `payload_duration = Some(region.duration()?)`.
    `region.duration()` is `TimeSpan::duration` (`src/pattern/query.rs:108`)
    and returns a `Result`.
  - `ReplaceTrack` and `TransformInstrument` keep the shared arm and set only
    `payload`.
  - The final check becomes
    `payload_duration.unwrap_or(part.duration()) == owner.duration`.
  - Revision, root, track, recursion, `work.charge(1)` and the depth check stay
    the same.
- **Pitfalls.**
  - Do not compare `owner.offset` or add any new check. The diagnosis fix is
    exactly this predicate.
  - Make sure `Ratio64` is in scope; use the path already used in this file.
  - Do not touch `SelectedSourceBoundary::validate_issued_child` (`:79-96`)
    or `song_provenance.rs`.
- **Tests.**
  - `tests/song_issued_transport.rs` -> passes past beat 4 with no
    `foreign source child scope`.
  - `cargo nextest run --lib song_clock` -> all pass. These are the
    structural clock tests.
- **Done.**
  - `git diff 10c3eab -- src/pattern/eval/song_clock.rs` shows hunks only
    inside `source_owns_owner`.
  - `wc -l` is below 1000.

### TASK-102: Export engine profile (export.rs)

**Status**: Not Started. **Depends on**: none.

- **Target.** `src/song/export.rs` `render` (lines 188-196 at `10c3eab`).
- **Change.**
  - Replace `EngineConfig::new(...)` plus `config.bus_slots = 32` with
    `crate::host::song_profile::song_engine_config(options.sample_rate as f32,
    MAX_BLOCK, caps, StoreKind::NativeArc, 2)`.
  - Map its `ConfigError` to a `Failure`, for example with the module's
    `fail(...)` helper and a message such as `export song engine profile:
    {error:?}`.
  - `song_engine_config` takes `CapabilitySet` by value, and `caps` is used
    again for `SongPreparationLimits.capabilities`. Pass a clone, or rebuild
    with `CapabilitySet::native()`. Do not change what preparation receives.
  - Remove the now-unused `EngineConfig` import. Keep everything else.
- **Imitate.** `src/host/song_profile.rs:song_engine_config` as native
  playback uses it: `grep -rn "song_engine_config(" src` shows the call
  sites.
- **Pitfalls.**
  - Do not edit `song_profile.rs`.
  - Do not change `MAX_BLOCK`, the frame arithmetic, the tail, the warnings or
    the second tail generation.
- **Tests.** `tests/song_export.rs`, `tests/song_end_to_end.rs`,
  `tests/song_issued_transport.rs` and `tests/song_cli.rs` all pass
  unchanged, except for the TASK-104 assertions.

### TASK-103: Private scheduler and Ready checks (realize_tests.rs)

**Status**: Not Started. **Depends on**: TASK-101 (needs a working issued
path).

- **Declaration.** In `src/sched/song.rs`, add
  `#[cfg(all(test, feature = "host-native", not(target_arch = "wasm32")))] mod realize_tests;`
  next to the existing `mod pools;` declarations. The file is
  `src/sched/song/realize_tests.rs`. It is a child of `sched::song`, so it can
  reach `SongTransport` private fields and `realize`.
- **Fixture helper** (private to the test file):
  - Evaluate a program with `crate::session::song::evaluate_song_candidate` and
    `prepare_song`, imitating `tests/song_issued_transport.rs:candidate`.
  - Prepare to Ready with `NativeAudioHost::headless_with_config(
    song_engine_config(...), 4096)`. Run `SongHostPreparation::begin`, then a
    pump loop, imitating `src/song/export.rs:render` and `pump_owner`.
    `pump_owner` is private there, so reimplement it as a small local helper.
  - Use the small program from `tests/song_transport_core.rs:499`:
    `inst tone ...; let a {part [tone: ...n 60...] duration: 1}; let b {... n 67 ...};
    song {sequence [{part-repeat a 2} b]} tail-seconds: 0 > play-song`.
    Give each Ready a distinct `SnapshotEpoch`.
- **PoolBook test seams** (`src/sched/song/pools.rs`; `#[cfg(test)]` only, no
  lint attributes):
  - `fn projection(&self) -> Vec<(Option<SongResolvedRoute>, u32, u32, u64, Vec<SongBranchRebound>)>`
    returns, per slot: configuration, projected, acknowledged, deadline and
    expected. `SongPhysicalBranch` has no `PartialEq`, so do not derive
    `PartialEq` on `Slot`.
  - `fn inject_assign_fault(&mut self, fail_at_call: Option<usize>)` is backed
    by `#[cfg(test)]` fields (`fault: Option<usize>`, `assign_calls: usize`).
    `staged()` copies them. At the top of `assign`, if the fault equals the
    current call count, return `Failure::new(FailCode::BeyondCapability,
    "injected staging fault")`. Otherwise increment the counter.
  - Make both `pub(super)`.
  - Why a fault point: the only real in-batch staging failure is receipt
    storage (`pools.rs:121`). It is bounded by `SongLimits.max_cached_events`,
    which also caps the batch event count (`src/song/limits.rs:83-91`), so a
    real trigger is fixture-fragile. The design allows this fallback. Record
    the choice in the receipt.
- **Tests (input -> expected outcome).**
  - `realize_failure_after_staging_preserves_pools_queue_cursor_and_receipts`:
    1. Build Ready, then `SongTransport::new(ready, activation,
       SongLimits::default())`.
    2. Inject a fault at call 1. That is the second `assign`, so the first
       assignment has already mutated the stage.
    3. Snapshot `pools.projection()`, `pending.len()` and `cursor`.
    4. Call `realize(duration)` -> `Err` with the injected message. All three
       snapshots are equal to their earlier values.
    5. Clear the fault and call `realize(duration)` again -> `Ok`, and
       `pending.len()` > 0.
    6. Do the same on a second Ready that never failed. Its `pending.len()`
       and the `(projected, acknowledged, deadline, expected.len())`
       projection equal the recovered transport's.
  - `realize_work_failure_preserves_state`: a transport with
    `SongLimits { max_nodes: <small value that passes SongLimits::validate>, ..default }`
    -> `realize(duration)` returns `Err`, and pools projection, pending and
    cursor are unchanged. If `SongTransport::new` refuses the small value,
    use the smallest value that `new` accepts and `realize` still refuses.
    Record that value in the receipt.
  - `foreign_issued_batch_is_refused_by_ready`: Ready A and Ready B come from
    the same program with different epochs. Build a batch with
    `B.query_issued_with_work(span, &work_b, 0)`.
    - `A.resolve_issued(&batch_b, 0, &work_a, 0)` -> `Err`.
    - `B.resolve_issued(&batch_b, 0, &work_b, 0)` -> `Ok` (control).
  - `one_collector_spans_query_and_every_resolution` (TASK-003 evidence):
    1. On a fresh Ready, create one `CanonicalIndexCollector::new(budget,
       limits)`. Call `query_issued_with_work`, then `resolve_issued` for every
       event index. `remaining()` strictly decreases after the query and does
       not increase across the resolutions. The spent amount is
       `W = budget - remaining`.
    2. On a second fresh Ready, the same sequence with budget exactly `W` ->
       `Ok`.
    3. On a third, budget `W - 1` -> some call returns `Err`.
- **Pitfalls.**
  - Do not call `advance` with a real host for these checks. Call `realize`
    directly so a failure does not go through `fail_and_retire`.
  - Keep the file below 1000 lines (target < 400).
  - No `#[allow]` or `#[expect]`.
  - The module must not compile into the WASM build, which the `cfg` above
    guarantees.

### TASK-104: Requirement transport assertions (tests/song_issued_transport.rs)

**Status**: Not Started. **Depends on**: TASK-101, TASK-102.

- **Capacity.** Replace the bound `measured_bus_slots <= 32` with
  `measured_bus_slots <= u32::try_from(vactr::host::song_profile::SONG_BUS_SLOTS - 1)`
  (50, the song-profile free slots). Keep the existing equality assertion with
  `route_plan.required.bus_slots`. Extend the `capacity evidence:` eprintln
  line with `available=50`.
- **Ended / frames.**
  - Keep every existing assertion.
  - Add: `first.total_frames == SongLimits::default().frames_at(duration *
    seconds_per_cycle + tail_seconds, 8000)`. Compute this from
    `candidate().snapshot()` (`duration()`, `settings().seconds_per_cycle()`,
    `settings().tail_seconds`). This is the export formula
    (`src/song/export.rs:180-186`); it is not the sum of the separately
    rounded parts.
- **Bit-exact.** Keep the byte-equality assertion. Add
  `std::fs::metadata(first_path).len() == 44 + first.total_frames * 4`.
- **Pre-Reserve refusal** (new test
  `preparation_refuses_before_upload_when_route_work_is_exhausted`):
  - Build `SongHostPreparation::begin(candidate(), limits)`, where
    `limits.max_work` is small enough that the issued route stage cannot finish
    but `begin` succeeds.
  - Pump with a headless native host using the song profile.
  - Expected:
    - preparation ends in `SongPreparationProgress::Failed` (or `begin` or
      `submit` returns the failure);
    - progress never reports `Uploading`, `AwaitingReady` or `Ready`;
    - the failure code is `FuelExhausted` or the work failure that the route
      stage raises.
  - Pick the `max_work` value empirically. Record it and the observed message
    in the receipt.
  - The design's authority table puts this in `preparation/issued.rs`. It is
    placed here because the public API is enough, and no preparation code
    change is required.
- **Pitfalls.**
  - Do not weaken or delete any existing assertion.
  - The file stays below 1000 lines.
  - Use the same `TempDir` helper.

### TASK-105: Evidence, receipt and gates

**Status**: Not Started. **Depends on**: TASK-101 to TASK-104.

1. **Before editing.**
   - Copy the current `tmp/song-mode-riela/session249-playback-build.log` and
     `session249-playback-receipt.json` to
     `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session259/`. Write
     `sha256.txt` with `shasum -a 256` for each copy.
   - Write the intent JSON (`tmp/song-mode-riela/session249-playback-intent.json`).
     It records the original text and sha256 of every writePath you will edit.
   - Run the baseline full nextest into
     `tmp/song-s249/SONG-ISSUED-PLAYBACK/session260-resume/baseline-full.log`.
     Expected: only `song_issued_transport` fails, with
     `foreign source child scope`. Record the result as `baselineFailures`.
2. **Every edit.** Re-read the file and compare it with the intent hash. If
   the hash has drifted, stop and report.
3. **Gates.** Run them in the foreground. Each must exit 0. Use the exact
   commands in the manifest `verification` list for SONG-ISSUED-PLAYBACK.
   - Build, strict all-target clippy, focused nextest (including `song_cli`),
     `--lib song_clock` and `--lib sched::song::realize_tests` must all pass.
   - Full nextest `--no-fail-fast`, with no `--retries`, must show a `Summary`
     line with 0 failures.
   - The WASM build must pass.
   - rustfmt `--check` on touched files reports no `Diff in` for a touched
     file. A hunk in an untouched child module is recorded, not fixed.
   - `wc -l`: every touched file is below 1000.
   - `grep -n "dedup_by\|resolve_route(" src/sched/song.rs` prints nothing.
   - `git diff 10c3eab | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
   - `git diff --quiet 10c3eab -- <unowned and frozen paths>` exits 0.
4. **Receipt** (`tmp/song-mode-riela/session249-playback-receipt.json`):
   - `session: 260`, `baseCommit: 10c3eab02ab29d25203f7cd5847a2247af404c20`;
   - `reviewDiffRange: "git diff 1ac457f <playback-accepted> -- <playback paths>"`;
   - `baselineFailures`;
   - `fixes[]`: TASK-101 and TASK-102 as `production-defect`, plus any
     conditional sharedPath edit;
   - `capacity.busSlots`: `{required, measuredFrom:
     "SongRoutePlan.required.bus_slots == tracks+1+sum(reserved_generations)",
     available: 50, genericReplaced: 31, engineProfile: "song_engine_config"}`,
     with `required` taken from the test's `capacity evidence:` line in the
     focused log;
   - `sm1`: `{refused: false, routeStageAllowance: 1000000}`. If export
     refuses for work, record the message and stop (SM1 option (a) needs
     `src/cli/render.rs` or `export.rs` limits, which is a SONG-16 conditional
     seam);
   - `attemptSession258`: `{files: [], reason: "no session249-playback-* files existed before session 259"}`;
   - `atomicityTrigger: "cfg(test) injected staging fault"`;
   - `task003Evidence`: the test name, plus the call chain
     `src/sched/song.rs:443-458 -> src/host/caps/song/preparation.rs:923 ->
     src/song/snapshot.rs:306 -> :191`;
   - each gate's command, exit status and log path;
   - a fingerprint that differs from every earlier receipt.
5. **Plan files.**
   - Tick TASK-003 in
     `impl-plans/active/song-mode-shared-issued-query-work.md` and add one
     progress-log line there pointing to this receipt.
   - Add one progress-log entry here.

### Session 260 done criteria

- [ ] `tests/song_issued_transport.rs` passes. No `foreign source child scope`
  appears in the focused log.
- [ ] `git diff 10c3eab -- src/pattern/eval/song_clock.rs` changes only
  `source_owns_owner`, and `--lib song_clock` passes.
- [ ] `grep -n "bus_slots = 32" src/song/export.rs` prints nothing, and
  `grep -n "song_engine_config" src/song/export.rs` prints one call.
- [ ] The four `realize_tests` tests and the new transport test pass.
- [ ] Strict clippy, full nextest (0 failures), WASM and scoped fmt `--check`
  all exit 0, with logs at the manifest paths.
- [ ] There are no new `allow`/`expect` lines, and the unowned and frozen
  paths are unchanged against `10c3eab`.
- [ ] `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session259/sha256.txt`
  exists. The receipt has the fields above.
- [ ] TASK-003 is ticked in the shared-work plan. One progress-log entry is
  added here.
