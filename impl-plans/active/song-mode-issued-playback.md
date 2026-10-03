# Issued song playback through Ready and scheduler

**Status**: Ready (wave 3, session 249; released after the wave-2 join)
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
