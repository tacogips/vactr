# Issued song playback through Ready and scheduler

**Status**: Planning
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
