# Retained canonical Index occupancy

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Remaining occupancy implementation direction](../../design-docs/specs/design-song-mode.md#remaining-occupancy-implementation-direction), [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)

## Purpose

Supply original retained canonical authority for unresolved static and dynamic
Index geometry. Keep accepted symbolic family paths and arrangement Repeat.
This is a bounded control-thread implementation, not a general lattice solver
and not an eager materialized arrangement. Original supported geometry and
dynamic source requirements remain required.

The first executable unit creates metered observations and replay authority.
Admission/geometry must subsequently consume that authority before this plan
can complete. The uniform `:vary` ceiling is an explicit unresolved integration
gate; one observed seed cannot certify every varying placement.

## Actual seams and blockers

- `pattern/combinators/region.rs:365` obtains actual Index rows before subject
  sampling. Observation must occur here, including rows later suppressed by
  empty subjects/deletion. `FrozenSongEvent` alone loses these footprints.
- `song/query.rs:31` creates QState per canonical root cycle. `pattern_rows:314`
  queries a complete local cycle, filters capture onsets, then clips parts.
  Preserve canonical issuer eligibility separately from caller clipping.
- `query.rs:255` normalizes Same repeat seed ordinal to zero and retains Vary
  ordinal. `scoped_walk:295` installs/restores the true derived seed.
- `pattern/eval.rs:232` stores SongLimits but leaves legacy QUERY_BUDGET active.
  Its spend counter is not currently the incoming construction counter.
- `vm/query_vm.rs:38` grants/restores fresh fuel for each guarded VM operation.
  A callback-call count is not its actual instruction work.
- `snapshot.rs:245` privately borrows the original isolated evaluator/Song.
  Existing public query separately initializes copy remaining; it is not a
  cumulative canonical-retention API and repeated queries replay callbacks.
- `routing/index.rs` binds original output payload/root/revision/track/complete
  path. Requests must be minted through this authority, never NodeId alone.
- `host/caps/song/preparation.rs:265` currently calls immutable prepare_routes
  before Reserve. A later consuming bridge must obtain retained authority before
  this call and before any resource reservation.

## First-unit exact Rust manifest

| Path | Current lines | Deliverable |
|---|---:|---|
| `src/pattern/eval.rs` | 902 | Thin optional song observer/shared work hooks; ordinary queries unchanged |
| `src/pattern/eval/song_observation.rs` | new | Opaque observation collector and original cumulative ledger |
| `src/pattern/combinators/region.rs` | 504 | Pre-subject actual Index observation and matched replay lookup |
| `src/vm/query_vm.rs` | 134 | Optional actual-fuel metering and original callable-result retention |
| `src/song/query.rs` | 606 | Canonical request execution with real owner/path/seed frame and shared work |
| `src/song/snapshot.rs` | 946 | Thin consuming authority method and original store ownership |
| `src/song/snapshot/occupancy.rs` | new | Opaque requests/retained records, exact unions and genuine private fixtures |
| `src/song/routing/index.rs` | 877 | Crate-only minting of requests from authenticated prepared operands |

Eight Rust modules only. All substantive new code lives in declared children;
parents must remain below1000 lines. Root released TASK-001 on 2026-10-03 after
the terminal legacy compatibility verification; TASK-002/003 remain unreleased.
Consumer hookup is not smuggled into this manifest. It needs a separately
reviewed bounded manifest covering immutable routing/density/configuration and
host preparation; this plan remains incomplete until those callers are real.

## Private declarations

All constructors remain private. These are declarations, not implementation code.

```rust
// pattern/eval/song_observation.rs
pub(crate) struct CanonicalIndexWork;
pub(crate) struct CanonicalIndexCollector;
pub(crate) struct CanonicalIndexObservation;
// Ownership: one control-thread Rc<RefCell<CanonicalIndexCollector>> is shared
// by QState and its VmQuery adapter. One original counter is charged by both.

// pattern/eval.rs; thin forwarding into ordinary child
pub(crate) fn observe_song_index(
    &mut self, issuer: &Pat, index: &Event,
) -> Result<(), Failure>;

// routing/index.rs; requires original snapshot/topology and complete address
impl SongSnapshot {
    pub(crate) fn canonical_index_request(
        &self, scope: usize, track: KwId, issuer: NodeId,
        path: &[FrozenUseTraceTerm], window: TimeSpan, depth: u32,
        limits: SongLimits, remaining: &mut u32,
    ) -> Result<CanonicalIndexRequest, Failure>;
}

// song/snapshot/occupancy.rs
pub(crate) struct CanonicalIndexRequest;
pub(crate) struct RetainedCanonicalIndex;
pub(crate) enum CanonicalIndexReadiness {
    OriginalOwnerComplete,
    RequiresUniformBound,
}
pub(crate) fn retain_index_occupancy(
    snapshot: &mut SongSnapshot, requests: Vec<CanonicalIndexRequest>,
    limits: SongLimits, remaining: &mut u32,
) -> Result<CanonicalIndexReadiness, Failure>;
```

The crate-only minting wrapper builds/validates its private PreparedSliceAddress
inside routing/index using the supplied original remaining counter; it does not
expose ResolutionBudget or private operand fields to snapshot consumers.
Request minting copies the actual prepared output owner identity, original
payload/recipe root and complete symbolic path under the existing budget. The
snapshot validates those against its own inventory before any VM work. Record
identity includes original owner revision/track, semantic seed path, canonical
issuer window, full actual producer path and source frames. NodeId may index a
lookup but never replaces complete equality. Dynamic returned Patterns are
retained from actual VM results with their strong original closure/result
identity, not cloned into forged authority. No public constructor or evaluator
accessor is added.

## Execution and accounting

1. Keep statically proved symbolic paths; request only unresolved original
   output sites. Do not query all registry instruments or flatten arrangement.
2. Execute canonical local owner windows using the original isolated evaluator,
   immutable input view, real seed and inherited depth. Charge each iteration,
   query operation, VM instruction debit, argument/result/trace copy, stored
   observation and union operation before allocation or execution.
3. VmQuery saves/restores its outer fuel state but debits actual consumed fuel
   to the same ledger. Fresh independent callback quotas must not refund shared
   work. Nested queries keep the same collector and original remaining depth.
4. Observe each actual Index event before subject sampling. Preserve whole,
   part, full producer, issuer, actual value/fault and owner/window eligibility.
   Invalid nonnumeric cuts remain actual Slice failures; no conversion to rests.
5. Retain callback results and canonical observations once. Later repeated or
   reordered queries use the same owner/seed/window record, not fresh VM/RNG
   calls. Refuse before growth if original work/cache policy is exhausted.
6. Attach successful opaque records to their original snapshot only. Failed
   construction publishes no partial authority. The store survives preparation
   and playback; snapshot disposal releases it on the control thread.

This plan does not assert that current per-cycle QUERY_BUDGET or per-call fuel
already supplies shared accounting. TASK-001 implements the missing bridge.

## Geometry and immutable consumer contract

Use original emitted whole footprints, not clipped parts or note presence.
After validating source configuration eligibility at the authentic sample START,
group footprints only by complete immutable source/use/configuration identity.
Merge overlapping/touching wholes, never fill a genuine gap. Clip the resulting
component to the actual output owner in the same clock. Keep inherited frame
clocks and issuer/subject clocks separate. Subject-mode note suppression must
not create configuration births.

Immutable prepare_routes must receive an opaque completed certificate through
the original SongSnapshot, and preserve it in SongRoutePlan. Density and route
resolution must look up the same complete address; an unrelated root with equal
NodeId cannot match. Geometry cannot trigger a fresh VM call. A consumer bridge
must charge copied certificate ownership and lookup/scans under its existing
ResolutionBudget. The existing public prepare_routes signature and original
Ready/lease cleanup remain intact.

These consumers are mandatory next code, not present APIs or proof supplied by
observation alone. Their exact module split must be declared before source
release; prepare.rs989 and density.rs991 need cohesive children before growth.

## Finite bounds and repeat gate

For a completely realized single original owner, derive K from simultaneous
configuration intervals, N from exact distinct component births, and arbitrary
window B from those birth positions. The conservative B(W)<=N is valid for that
finite owner; it is a configuration count, not emitted-note count. Include
owner-clipped head births and half-open tails.

`:same` may reuse an original child record only when its complete semantic seed
path and local canonical owner/window are identical. Preserve different physical
placements and compose the existing occurrence bound once. Nested inner Vary
records are part of that same reproducible development.

`:vary` changes the derived seed. One child observation is not uniform authority.
A seed-independent existing structural bound may be accepted only when actually
proved for the complete requested operand and enforced during every later
realization. Arbitrary pure Fn/Thunk results currently have no such tight bound.
The real missing gate is an enforceable uniform configuration-opening ceiling
for these results, including dynamic whole reach and subject eligibility.

Do not silently use max_cached_events as opening count, guess from note caps,
scan every arrangement repetition, or equate a fuel limit with a proved tight
birth envelope. If the original shared work/storage cannot admit a requested
realization, return an addressed real limit failure before excessive work. This
is not a permanent final exclusion of Vary or dynamic operands; completing their
uniform consumer is required for full scope.

## Tasks

### TASK-001: Metered original canonical records

**Status**: In Progress
**Parallelizable**: No

- [ ] Implement the eight-path collector/fuel/query/snapshot authority unit.
- [ ] Actual pre-filter Index rows survive empty subjects and deletion; full
  original trace/window/value/faults remain authenticated.
- [ ] Actual callback executes once; repeated/fractional/reordered requests
  reuse its original result and preserve full identities.
- [ ] Exact shared work and inherited depth success/one-less refusal, including
  VM instruction debit and preallocation; legacy query behavior unchanged.
- [ ] Genuine private fixtures use original isolated Candidate/Freeze/evaluator,
  not fabricated observations, timings or expected callback results.
- [ ] Native/WASM/strict lint/scoped format pass via independent checker.

### TASK-002: Immutable admission and geometry consumer

**Status**: Planning
**Parallelizable**: No (depends on TASK-001 and prior bounded manifest)

- [ ] Declare exact <=8-path consumer source scope, including necessary parent
  extraction before1000 lines and original host pre-Reserve handoff.
- [ ] Immutable route admission/resolution consume the same retained authority.
- [ ] Genuine heterogeneous, trimmed, inherited/static/dynamic whole unions and
  exact K/B/N agree with independently observed actual configurations.
- [ ] Subject gaps do not invent births; symbolic fast paths remain unchanged.

### TASK-003: Same/Vary uniform authority and full acceptance

**Status**: Planning
**Parallelizable**: No (depends on actual uniform bound)

- [ ] Prove/enforce the addressed seed-independent ceiling or faithful uniform
  alternative; one observed seed is never generalized to Vary.
- [ ] Genuine Same/nested-Vary/outer-Vary seed and repeated-placement fixtures;
  no arrangement score flattening, uncharged replay or ordinal substitution.
- [ ] Huge arrangement repeat retains symbolic admission; original capacity,
  query, callback and storage limits fail truthfully when genuinely exceeded.
- [ ] Independent complete route/query/playback and Native/Arena/browser gates.

## Progress log

### 2026-10-03 — First coherent eight-path foundation source hold

Written optional QState observer, shared work collector, private metered VM
adapter, actual pre-subject Index hook, canonical owner traversal, snapshot
store and authenticated request minting. The explicit request window is in the
original root clock; original traversal derives local owner windows, offsets,
placement paths and seeds. It traverses symbolic topology under charged work
but skips unrelated payload execution. Selected-source dependencies within the
requested payload continue under that state. No arrangement score is flattened.

The original Song and copied recipe Rc, topology scope/revision/track/root and
complete symbolic operand path authenticate a request. Execution sharing uses
the complete owner/window/depth identity rather than an individual callback or
Slice number. All original owner's Slice observations survive in the record;
addressed replay filters their actual issuer/prefix. The ordered callable,
argument and result journal retains real strong immutable values. Actual VM
fuel consumption is debited together with query work and retained allocations.
Event clone pricing includes owned BTree entries, occurrence and producer
vectors; immutable Value/source-origin Rc payloads are retained without deep
payload allocation. New cache authority is published only after a whole batch
succeeds. Cached depth reuse conservatively preserves its admitted caller bound.

Replay selects original canonical owner windows and returns uncut issuer-clock
events; it does not invent transformed root-clock event geometry or connected
source configuration identities. Uniform readiness stays RequiresUniformBound.
Ordinary public snapshot.query intentionally remains unchanged and executes
callbacks anew. The subsequent canonical-query-replay plan must integrate raw
owner output reuse before immutable admission/geometry can consume it.

Written genuine isolated candidate fixtures in snapshot::occupancy::tests:
original_dynamic_index_executes_once_and_replays_fractional_reordered_windows;
pre_subject_observation_survives_genuine_empty_subject;
pre_subject_index_footprint_survives_actual_handle_deletion;
canonical_cumulative_work_exact_one_less_and_inherited_depth_are_real_limits;
addressed_owner_preserves_nested_offset_seed_and_excludes_unrelated_callbacks;
request_from_independent_original_snapshot_cannot_remint_authority.
The unrelated callback is actual division-by-zero; its ordinary full query is
asserted to fail before addressed observation. These are written assertions,
not passing results until independent execution.

Concrete unfinished metering boundary: native part-events creates an ordinary
VmQuery (vm/natives/song.rs); nested guarded calls can restore separate fuel
inside a callback. The current private adapter meters direct instruction work,
but native-created nested adapters still need an inherited bridge before full
TASK-001 shared-metering acceptance. Exact source-eligible connected unions,
immutable consumers and uniform Same/Vary authority remain mandatory later work.
No blanket child-module lint allowance remains; only named pending entrypoints
and retained journal fields have documented dead-code allowances.

All eight Rust files are below1000 and scoped rustfmt/check succeeded. No Cargo,
dependencies, public VmQuery fields or QueryVm hooks changed. Source is held for
the first independent native/WASM/lint/fixture checkpoint; TASK-001 remains In
Progress and no full song-mode completion is claimed.

### 2026-10-03 — TASK-001 authorized source intent

Implement the declared eight-path foundation using the original snapshot's
isolated VM and immutable Song. A shared control-thread ledger charges query
work, actual VM instructions and retained copies. The optional observer records
Index events before sampling even when no subject note survives, together with
the original owner/placement/seed/window and complete issuer/producer traces.
Successful records and strong actual callback results remain private to their
snapshot; repeated/fractional replay reads stored records. No public VmQuery
layout or ordinary query behavior changes. Requests are minted through existing
authenticated routing operands. Actual consumer and uniform-Vary admission
remain subsequent tasks; a retained observation is not a readiness certificate.
Genuine private candidate fixtures and cumulative work/depth boundaries must be
written before the source hold. No author Cargo or dependency changes.

### 2026-10-03 — Source-grounded bounded implementation declaration

Read actual original query, Slice hook, QState, VmQuery, snapshot and host seams.
First eight-path capture/authority task is executable without a general solver.
Consumer and Vary uniform gates remain explicitly unfinished. No Rust/Cargo
changes; all accepted static compiler/production files remain held. Root owns
the design document and receives the proposed authority/accounting amendment
separately. This plan does not certify dynamic occupancy or host admission.
