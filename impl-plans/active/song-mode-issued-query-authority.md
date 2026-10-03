# Query-issued song authority

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture), [Production provenance review](../../design-docs/references/song-mode/production-provenance-review-20261003.md)

## Intent and dependencies

Issue authentic invocation authority during original song queries and preserve
it through event expansion, edits and union. Public descriptor APIs remain
available. This supplies internal rows for the following production phases.

| Relation | Companion | Required result |
|---|---|---|
| Previous | [Retained Index geometry](../completed/song-mode-retained-index-geometry.md) | Accepted geometry and retained lookup |
| Next | Frozen issued-event handoff | Snapshot/Ready/scheduler preserve proofs |
| Next | Authenticated production routing | Actual proofs select geometry and members |
| Later | Admission and varying-seed support | Pre-Reserve certification and useful bounds |

Root reviewed the concrete eight-path contract after geometry0008 and fresh
frontend acceptance. Source execution follows the explicit release below.

## Exact proposed Rust manifest

| Module | Path | Status |
|---|---|---|
| Query-local transaction field | `src/pattern/eval.rs` | Not Started |
| Actual issuance | `src/pattern/eval/song_replay.rs` | Not Started |
| Acyclic authority | `src/pattern/eval/song_provenance.rs` (new) | Not Started |
| Canonical propagation | `src/song/query.rs` | Not Started |
| Issued row union | `src/song/query/issued.rs` (new) | Not Started |
| Selected-source binding | `src/song/source/sampling.rs` | Not Started |
| Original source origin | `src/song/source.rs` | Not Started |
| Authenticated boundary member copy | `src/pattern/eval/song_clock.rs` | Not Started |

Eight paths maximum. Keep every touched Rust file below1000 lines. Register
children through replay/query and include genuine fixtures in declared children.
Do not silently change public descriptor fields or add a ninth module.
Snapshot query issuance and frozen inherited-origin propagation move to the following required handoff phase; this
phase returns live issued batches and does not yet supply frozen-ready authority.

## Concrete private authority and transport

Eight-path feasibility is source-grounded: Pattern Event and all public query
context literals remain unchanged. QState receives one private optional transaction
field initialized to None. song_replay registers the new sibling song_provenance
module through eval.rs and delegates substantial issuance work there. song/query.rs
registers query/issued.rs. Existing internal canonical/walk/edit functions return
IssuedQueryRow internally; existing public wrappers erase only the envelope and
return the same SongEvent descriptors. No second realization algorithm is added.

The following are complete data contracts, not public field constructors. All
fields are private to the owning authority module; hooks require genuine scoped
marks produced during the actual owner q. Rc allocation identity authenticates
opaque leaves against completed tables; scalar identity values alone never issue
or authenticate a seal.

```rust
pub(crate) struct ExecutionSeal {
    original: Weak<Song>,
    owner: CanonicalOwnerFrame,
    entry: ProducerTrace,
    seed: u64,
    admitted_depth: u32,
    entry_depth: u32,
}
pub(crate) struct InvocationSeal {
    execution: Rc<ExecutionSeal>,
    original: Weak<Song>,
    owner: CanonicalOwnerFrame,
    entry: ProducerTrace,
    seed: u64,
}
pub(crate) struct SourceBindingSeal {
    original: Weak<Song>,
    child: Rc<InvocationSeal>,
    issued_handle: EventHandle,
    source_entry: ProducerTrace,
}
pub(crate) struct IssuedSourceLeaves {
    invocations: Rc<[Rc<InvocationSeal>]>,
    bindings: Rc<[Rc<SourceBindingSeal>]>,
}
pub(crate) struct CompletedIssuedInvocation {
    seal: Rc<InvocationSeal>,
    execution: Rc<OwnerCycleExecution>,
    entry_clock: CanonicalClockProjection,
    child_seals: Vec<Rc<InvocationSeal>>,
    source_binding_seals: Vec<Rc<SourceBindingSeal>>,
    actual_entry_depth: u32,
}
pub(crate) struct IssuedOwnerMark {
    scope: Rc<IssuedOwnerScope>,
    entry_clock: CanonicalClockProjection,
    invocation_start: usize,
    binding_start: usize,
    source_completion_start: usize,
    entry_depth: u32,
}
pub(crate) struct PendingSourceBinding {
    seal: Rc<SourceBindingSeal>,
    boundary: Rc<SelectedSourceBoundary>,
    parent_scope: Rc<IssuedOwnerScope>,
}
pub(crate) struct CompletedSourceBoundary {
    boundary: Rc<SelectedSourceBoundary>,
    parent_scope: Rc<IssuedOwnerScope>,
    bindings: Vec<Rc<SourceBindingSeal>>,
    actual_depth: u32,
}
pub(crate) struct RetainedIssuedChild {
    invocation: Rc<InvocationSeal>,
    execution: Rc<OwnerCycleExecution>,
    entry_clock: CanonicalClockProjection,
    source_bindings: Vec<CompletedSourceBinding>,
    source_completions: Vec<CompletedSourceBoundary>,
}
pub(crate) struct CompletedSourceBinding {
    seal: Rc<SourceBindingSeal>,
    boundary: Rc<SelectedSourceBoundary>,
    member: Rc<SongEventOrigin>,
    parent_scope: Rc<IssuedOwnerScope>,
}
pub(crate) struct IssuedOwnerScope {
    original: Weak<Song>,
    owner: CanonicalOwnerFrame,
    entry: ProducerTrace,
    seed: u64,
}
pub(crate) struct IssuedQueryTransaction {
    original: Rc<Song>,
    work: SharedIndexWork,
    invocations: Vec<CompletedIssuedInvocation>,
    bindings: Vec<CompletedSourceBinding>,
    pending_bindings: Vec<PendingSourceBinding>,
    source_completions: Vec<CompletedSourceBoundary>,
    active_scopes: Vec<Rc<IssuedOwnerScope>>,
    inherited_depth: u32,
    completed_scopes: Vec<(Rc<IssuedOwnerScope>, Rc<InvocationSeal>)>,
    failed: Option<Failure>,
}
pub(crate) type SharedIssuedTranscript = Rc<RefCell<IssuedQueryTransaction>>;
pub(crate) struct IssuedQueryTranscript {
    original: Rc<Song>,
    inherited_depth: u32,
    source_completions: Vec<CompletedSourceBoundary>,
    invocations: Vec<CompletedIssuedInvocation>,
    bindings: Vec<CompletedSourceBinding>,
    completed_scopes: Vec<(Rc<IssuedOwnerScope>, Rc<InvocationSeal>)>,
}
pub(crate) struct IssuedQueryRow {
    row: SongEvent,
    contributors: Rc<[Rc<InvocationSeal>]>,
}
pub(crate) struct IssuedQueryBatch {
    rows: Vec<IssuedQueryRow>,
    transcript: IssuedQueryTranscript,
}
```

IssuedOwnerScope is a privately created actual scope leaf, not a caller-supplied
owner certificate. It lets a source binding refer to its still-active parent q
without owning a transaction/record. Successful finish resolves that exact scope
pointer to the fresh completed invocation; publication rejects unresolved scopes.
ExecutionSeal is created once at actual successful raw q retention and stored on
OwnerCycleExecution.
OwnerCycleExecution gains private execution_seal: Rc<ExecutionSeal> and
issued_children: Vec<RetainedIssuedChild>. The existing genuine execution mark
supplies the original owner/entry/seed and admitted depth even when no issuance
transaction is installed; no caller facts create this seal. IssuedOwnerMark is
move-only (no Clone), captured at real entry and consumed exactly once by finish
or abort. Its transaction indices delimit actual descendants and completions;
finish accepts only already completed children from that suffix. RetainedIssuedChild
stores no transaction or own/ancestor record. Child execution references follow
actual q completion order strictly, establishing the acyclic descendant DAG.
Replay reissues each sealed child and its completed source boundaries through
one old-to-fresh seal/scope map before copying all inherited origin bags. InvocationSeal is fresh for every actual q-entry/rebound
invocation, including successful empty q. SourceBindingSeal names one exact
returned original member of one actual completed boundary. All copied vector data
and leaf allocations are charged before creation under the same original ledger.

Ownership is acyclic: transcript -> completed records -> raw execution/clock/
original members -> immutable leaf bags -> invocation/execution/binding leaves.
Leaves never own executions, clocks, replay views, transactions or member origins.
Raw executions may retain only already-completed descendant templates; never
its own or ancestor invocation record. A template stores genuine child execution,
actual key/clock and source-binding facts from the completed descendant, with
parent scope references retained as leaves. Replay substitutes a genuinely current
parent scope for that exact sealed template relationship, not a reconstructed
seed/path match. Original raw execution membership and new query transcript
membership are independently authenticated.

### Issuance and publication signatures

Names can adapt to local Rust types without changing the contracts. Opaque mark
fields remain private; callers cannot mint marks from handles/timing/scalars.
The owner hooks are called only by pattern_rows around actual q/replay, after the
original canonical traversal installs the genuine owner context.

```rust
// QState thin storage in eval.rs; implementations in song_provenance.rs.
fn install_issued_query(&mut self, tx: SharedIssuedTranscript);
fn issued_query(&self) -> Option<&SharedIssuedTranscript>;
fn begin_issued_owner(&mut self) -> Result<Option<IssuedOwnerMark>, Failure>;
fn finish_issued_owner(
    &mut self, mark: IssuedOwnerMark,
    execution: &Rc<OwnerCycleExecution>,
) -> Result<Rc<InvocationSeal>, Failure>;
fn abort_issued_owner(&mut self, mark: IssuedOwnerMark, failure: &Failure);

// Real owning transaction; shared work.original must be this exact original.
fn begin_issued_query(
    original: Rc<Song>, work: SharedIndexWork, depth: u32,
) -> Result<SharedIssuedTranscript, Failure>;
fn publish_issued_query(
    tx: SharedIssuedTranscript, rows: Vec<IssuedQueryRow>,
) -> Result<IssuedQueryBatch, Failure>;

// One original query algorithm with private issued companions.
fn query_part_issued(
    original: &Rc<Song>, span: TimeSpan, cx: &mut SongQueryCtx<'_>,
    work: SharedIndexWork, depth: u32, replay: Option<Rc<ReplayView>>,
) -> Result<IssuedQueryBatch, Failure>;
fn nested_part_issued(
    part: &Part, span: TimeSpan, track: KwId, state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure>;
// REQUIRED frozen-handoff companion owning seam (not implemented in this phase):
// query_issued_rows(snapshot, span, limits, remaining, depth) -> IssuedQueryBatch.
```

The transaction field persists across all root-cycle QStates of one issued query;
nested selected sources use that same state and transaction. query_part_issued
passes (work, depth) into the existing computational QState constructor; nested
calls retain that inherited floor. The required downstream query_issued_rows adapter accepts incoming depth rather
than resetting it; only the actual snapshot root caller may explicitly pass zero.
begin validates the same original ledger, admitted limits and inherited depth;
publication rechecks that attachment without granting fresh work/depth. Computational
observation filtering cannot suppress issuance. execution_mark/owner retention
must support issuance independently of records_owner: actual instruction/query
work remains charged even with no selected observer rows. An issued query starts
with exact original attachment in SharedIndexWork. No implicit new budget is
created per q, source, root cycle or publication. Independent public query_part
keeps its previous cycle budget behavior when no issued transaction is installed.
Native part-events descriptor queries remain public DTO computations: their output
is not automatically route authority. The enclosing actual callback q/execution
is still sealed; no claim is made that every native-created descriptor carries
an independent issued row. Such DTOs cannot be substituted at the private route
entry. Shared native/VM metering from the accepted meter bridge stays intact.

With an existing required ReplayView, a missing genuine execution still fails
before q; issuance never authorizes immutable callback-on-miss. Without a view,
a candidate-time actual q may create a fresh execution, but that fact alone is
not a future varying-seed domain certificate or routing admission. The completed
transcript distinguishes records actually present in the retained view from
successful fresh candidate-time records through exact execution membership,
not copied scalar fields. That handoff/classification belongs to the next phase.

### Actual source boundary/member transport

SongEventOrigin already has private fields. Add a crate-private optional
Rc<IssuedSourceLeaves> there, preserving public construction/descriptor contracts
and the original source whole/handle/timing/inherited chain. Pattern Event remains
unchanged. Raw events carry only origin leaf bags. Normal direct output receives
its own invocation leaf in IssuedQueryRow after actual q finish and tone expansion.
Each child contributor also survives the actual selected-source origin chain.

```rust
// Source/sampling calls with the actual with_source_boundary token and issued row.
fn bind_issued_source_member(
    &mut self, boundary: &Rc<SelectedSourceBoundary>,
    child: &IssuedQueryRow, member: SongEventOrigin,
) -> Result<Rc<SongEventOrigin>, Failure>;
fn finish_issued_source_boundary(
    &mut self, boundary: &Rc<SelectedSourceBoundary>,
    returned: &[Event],
) -> Result<(), Failure>;
fn original_issued_leaves(origin: &SongEventOrigin)
    -> Option<&Rc<IssuedSourceLeaves>>;
```

The member is the actual unshared newly constructed original origin from query_source,
with genuine handle, issued_handle, source whole/part, source entry and inherited
origin. Binding checks its exact child row/contributors and actual boundary
selection before publishing a completed binding. complete_source_boundary's
actual returned members must be sealed first; failure leaves only provisional
records and fails the whole issued transaction. An empty successful source can
publish a completed empty boundary observation, but never SourceBindingSeal or
an invented member. CompletedSourceBoundary retains the actual boundary and parent scope even when
bindings is empty. Its boundary must have the genuinely sealed returned empty
member list: the exact selected policy/use, finite domain, query piece and producer
remain in that original boundary and its entry clock. Absence of a member or leaf
alone never certifies empty completion. SourceBindingSeal remains member-specific.
The owning hook consumes that unshared origin, creates PendingSourceBinding,
installs its leaf bag before allocating the final Rc, and returns that exact Rc;
finish receives the actual returned Events after complete_source_boundary seals
them, authenticates those final origins against the pending seals, then moves them
into completed records. Publication rejects any pending binding or unsealed source
completion, including a source query that faults before returning its empty list.

On cached outer replay, instantiate every sealed child template with a fresh
InvocationSeal and rebound actual clock/boundary/member relationship. Rebuild
all nested inherited source-origin leaf bags using the exact old-seal -> new-seal
association established from actual templates. Copy each ordinary origin field,
whole, timing and producer unchanged. No stale child invocation leaves may escape
from the raw cached events. Do not infer leaves from final handles or seeds.
copy_execution performs this even when observe=false; observation/journal
copying is not the issuance switch. Work/depth is charged before all origin/bag/
record copying. Pure VM depth continues to require admitted-depth conservative
checks, not an incomplete observed peak.

### Canonical row propagation and compatibility

query/issued.rs owns envelope construction, original-depth copying and union.
Use actual primary invocation leaf plus the authentic event's inherited source
leaf bag when expanding each chord tone. Clip/shift changes the descriptor only;
all contributing leaf identities survive. Sequence/Repeat preserve original
placement/seed scope. Delete removes the row and its envelope. Overwrite and
Replace add genuinely newly queried rows. Transform retains actual inner-member
binding and the fresh outer invocation, while sibling rows keep their own proofs.

```rust
fn issued_expansion(
    rows: Vec<SongEvent>, owner: Option<&Rc<InvocationSeal>>,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure>;
fn insert_issued_union(
    rows: &mut BTreeMap<EventHandle, IssuedQueryRow>, row: IssuedQueryRow,
    state: &mut QState<'_, '_>,
) -> Result<(), Failure>;
fn into_public_rows(rows: Vec<IssuedQueryRow>) -> Vec<SongEvent>;
```

Union retains all distinct genuine invocation contributions for a continuation,
not only the first row's metadata. Deduplicate solely identical Rc leaf authority,
with charged comparisons/growth. Descriptor handle/part union semantics stay
unchanged. No new proof allocation is part of public origin PartialEq/descriptor
comparison: current SongEventOrigin derives Clone/Debug, frozen public origins
retain their existing PartialEq. Issued identity/authentication uses dedicated
Rc-pointer membership methods, not a new descriptor equality derive. A fixture
must compare descriptors exactly while separately checking different real
contributor sets.

### Publication, failure and frozen handoff

After all root QStates detach/drop their transaction handles, publication consumes
the sole shared transaction with Rc::try_unwrap; an unexpected live alias is a
hard refusal, never an aliased mutable published table. publish_issued_query
succeeds only after no active scopes, no pending bindings, no fault, actual
complete source boundaries, exact original attachment and every emitted leaf
belonging to completed transcript records. Failure drops provisional rows/tables,
retains all consumed original work and restores QState depth/clock/source scope.
Previously published snapshot retention/replay is unchanged. The actual issued root writes consumed remaining back on success or error;
the snapshot owning adapter is required in the following frozen handoff phase. Record result
storage/copy costs and publication scans are charged through the same ledger.

This phase intentionally does not add leaf fields to FrozenSourceOrigin or
FrozenSourceOriginFrame (source_uses/origin.rs is outside the eight paths).
Existing copy_origin descriptor constructors compile unchanged; the public
wrapper can erase live authority only when returning public descriptors. An
issued batch must remain live and separate until the next frozen handoff phase
copies the descriptor and proof together. Next required paths include snapshot.rs
query/copy, source.rs copy_origin, source_uses/origin.rs inherited constructors,
Ready preparation and scheduler. They must preserve opaque transcript/leaf
membership independently of public PartialEq; no drop-and-reconstruct from handle
is permitted. Immutable route-view pairing and actual production consumers remain
separate mandatory followups. No unused live batch is full integration.

### Genuine owning fixture matrix

Fixtures fit declared song_provenance and query/issued children; use existing
isolated candidate/freeze, nested public code and genuine replay. No ninth file:

1. Original direct chord q yields one shared raw ExecutionSeal and separate
   tone rows carrying the same actual primary InvocationSeal. Public descriptors
   equal the unchanged ordinary query output, not pointer equality.
2. A required cached raw hit shares ExecutionSeal, issues a fresh InvocationSeal,
   keeps original key/seed/placement and records actual rebound entry relation.
   Deny cut reads after retention; missing/foreign view refuses before q.
3. Genuine cached outer hit returns nested selected children with fresh invocation
   leaves at every inherited origin, actual member/boundary/use/copy association
   and unchanged complete descriptors; no stale template leaves remain.
4. Empty selected-source query seals empty entry context, produces no member
   binding; suppressed/deleted notes do not fabricate output contributors.
5. Fractional offset/reordered root queries plus chord continuations union every
   genuine contributing invocation leaf; an equal handle cannot erase authority.
6. :same/:vary Repeat and two actual same-policy source uses retain distinct
   placements/seeds/entry/boundary/member proofs. Foreign or swapped leafs must
   fail owning transcript authentication despite scalar-equal descriptors.
7. Genuine Delete/Overwrite/Transform and sibling outputs preserve exact notes,
   tone handles, spans and correct contributor sets; computational observer
   selection does not disable actual production row issuance.
8. Whole issued operation exact work succeeds, one-less/depth fails with consumed
   counter and no published partial transcript, preserving prior replay/view;
   actual callback fault restores all scopes. Test acyclic leaf ownership with
   Weak lifecycle evidence after dropping transcript/rows, not fabricated seals.

## Remaining implementation boundary

The eight-path unit is executable and supplies the live issued-query seam.
It is not production routing/host admission. Allocation/counter hooks and every
source-origin clone must be inspected during implementation; any need to edit
pattern/query.rs, song_observation.rs or source_uses/origin.rs must be declared in
a separate bounded companion before edits, not silently added as a ninth path.
Current headroom: eval933 (thin field/init/register), replay719 (thin scope/copy
hooks), song/query761 (one walk with issued envelope), source807 (private shallow
bag and propagation helpers), sampling346; clock hooks stay thin. New children own bulk
logic/tests so all touched Rust stays below1000.

## Tasks

### TASK-001: Genuine opaque issuance

**Status**: Completed
**Parallelizable**: No

- [x] Source-grounded eight-path signatures and live/frozen boundaries finalized for parent review; no source release.
- [x] Issue capsules at successful original execution/replay boundaries.
- [x] Authenticate actual seed, owner placement and full producer entry.
- [x] Genuine replay/foreign/missing-record fixtures prove acyclic authority.

### TASK-002: Row and selected-source propagation

**Status**: Completed
**Parallelizable**: No; depends on TASK-001

- [x] Preserve authority through canonical walking, edits, clipping and chords.
- [x] Preserve nested source member bindings through live inherited origins;
  identify every frozen copy seam required by the following handoff phase.
- [x] Union distinct authentic contributing proofs; deduplicate only identical
  issued authority, never scalar-equal or handle-equal invocations.
- [x] Preserve public descriptor behavior and original counter/depth.

### TASK-003: Owning seam and acceptance

**Status**: In Progress
**Parallelizable**: No; depends on TASK-002

- [x] Provide actual live issued rows and transcript for the required snapshot
  adapter in the following frozen handoff phase.
- [x] Genuine nested source/bank, chord continuation, fractional/reordered
  query and distinct seed/placement fixtures establish propagation.
- [x] Foreign/swapped/missing and exact/one-less work/depth failures publish
  no partial rows and preserve prior retained evidence.
- [x] Independent tests/lint/WASM/format pass against held inputs.

## Completion criteria

- [ ] All tasks and genuine query authority fixtures pass.
- [x] Public compatibility and original metering are preserved.
- [ ] Following envelope/routing phases have concrete manifests and consume
  these proofs; an unused capsule is not production integration.
- [ ] Admission and useful varying-seed support remain mandatory full-goal work.

## Progress log

### 2026-10-03 — Source-grounded draft

Reviewer confirms invocation finishes before expansion and handle-only union
loses contributing authority. Proposed eight-path phase uses shared private
leaf capsules and issued rows. Geometry correction remains active; no Rust
edits or execution are released by this draft.

### 2026-10-03 — Replay authority refined by source review

Reviewer identifies fresh invocation rebinding in copy_execution. Split the
original capsule proposal into shared ExecutionSeal and fresh InvocationSeal,
with a completed query-owned transcript. Nested source certificates bind actual
boundary/member facts to child invocation leaves; frozen inherited frames retain
those certificates. Proposed issued query companions preserve public wrappers.
The eight-path scope still requires author validation and finalized signatures;
observation selection cannot substitute for query issuance. No source release.

### 2026-10-03 — Missing nested transport corrected in draft scope

Further source review disproves the original eight-path feasibility claim:
QState has no transcript field, and raw events cannot transport strong completed
child records safely. Replace source_uses/origin.rs with eval.rs in this phase.
Install a single shared query-local issuance transaction in root-cycle states;
nested recursion registers completed records there and returns leaf seals only.
Frozen propagation is mandatory in the next phase, including origin.rs and
source.rs's actual copy_origin constructors. The revised eight-path scope still
needs author validation and complete signatures before Ready/source release.


### 2026-10-03 — Concrete source-verified issuance specification

Read-only review confirms sourceorigin owns private fields, while Pattern Event
needs no new field. QState field can carry the shared transaction across original
root cycles and selected nested traversal; row envelopes retain direct output
seals and origin leaf bags retain selected contributors. Actual owner q completes
before expansion, requiring explicit successful finish return. Cached child
instantiation must run independently of observer filtering and rebind inherited
bags. No public VmQuery layout change or raw Event modification is required.
Complete private shapes/signatures and eight owning fixture scenarios replace
placeholder unit fields. Frozen proof handoff, permitted varying domains and
actual route-view integration remain explicitly next required phases. No Rust
source edits, compile/test commands or release resulted from this plan review.

### 2026-10-03 — Inherited depth and completion records finalized

DOCS ONLY: all issued root/owning entry signatures now accept genuine inherited
depth. Move-only owner marks, sealed descendant templates, pending member bindings
and completed source boundaries have concrete private shapes. Successful empty
boundaries retain their actual sealed policy/use/domain/query rather than infer
empty authority from missing output. Shared raw execution seals are issued at
actual retention, while cached calls create fresh invocation seals and rebind every
contributor bag. Publication consumes the uniquely owned finished transaction;
no new ledger, scalar authority constructor or public Event field is introduced.
Eight Rust paths remain sufficient; implementation/source release still requires
root review. Frozen issued-event and issued-playback companions remain mandatory.

### Authenticated clock member-copy seam (eighth path)

song_clock.rs insert_source and replace_source currently clone each sealed
returned member Rc when rebuilding a descendant boundary. Those clones must use
the SAME issuance rebinder as raw Event/inherited origin copies. Otherwise events
would have fresh invocation bags while their authenticated boundary still stores
old bags. The owning clock code emits exact old/new boundary pairs; caller scalar
fields never manufacture those pairs. Substantial rebinding/memoization lives in
already declared song_provenance.rs; clock.rs keeps thin charged hooks.

```rust
// Private owning clock token, constructed only after genuine insert/replace.
pub(crate) struct ReboundSourcePair {
    original: Rc<SelectedSourceBoundary>,
    current: Rc<SelectedSourceBoundary>,
}
// Private provenance mapper, created only from completed descendant templates.
pub(crate) struct IssuedOriginRebinder {
    invocations: Vec<(Rc<InvocationSeal>, Rc<InvocationSeal>)>,
    bindings: Vec<(Rc<SourceBindingSeal>, Rc<SourceBindingSeal>)>,
    scopes: Vec<(Rc<IssuedOwnerScope>, Rc<IssuedOwnerScope>)>,
    origins: Vec<(Rc<SongEventOrigin>, Rc<SongEventOrigin>)>,
    boundaries: Vec<ReboundSourcePair>,
}
fn rebind_observation_issued(
    &self, original: &CanonicalClockProjection,
    observation: &CanonicalClockProjection, owner: &CanonicalOwnerFrame,
    depth: u32, work: &mut CanonicalIndexCollector,
    mapper: &mut IssuedOriginRebinder,
) -> Result<CanonicalClockProjection, Failure>;
fn copy_issued_origin(
    &mut self, original: &Rc<SongEventOrigin>, depth: u32,
    work: &mut CanonicalIndexCollector,
) -> Result<Rc<SongEventOrigin>, Failure>;
```

Existing descriptor-only rebind wrappers retain their behavior. Issued mode maps
every actual returned member and every inherited origin through one charged
pointer-keyed memo; each old origin gets the identical new Rc in returned members
and replayed Events. Mapper entries are authentic old/new completed seal pairs,
not a callback dictionary or handle-to-proof inference. Before cloning variable
fields, allocating bags, traversing inherited origins or growing memo/boundary
tables, debit original work and inherited depth; no reset at a source hop.
Unchanged descriptor fields stay unchanged. Unknown clocks remain barriers.

### Genuine owning fixture construction without snapshot private access

query/issued.rs tests construct their own isolated Evaluator using Prelude::core,
DecodedSongAssetFactory::begin source_loader and RecordingSink, as existing
song_clock::tests::Fixture does. They evaluate valid public DSL, require all actual
forms successful, close assets, and Freeze::value the original evaluated Song.
They capture the same original routing via session::song::capture_original_test_routing.
No access to sibling-private snapshot.evaluator is needed and no second issuer's
Song is substituted. The same evaluator supplies vm_and_ns, MeteredSongQuery and
SongQueryCtx; CanonicalIndexCollector retains that exact original, prepares genuine
owner dependencies, and actual observe_part produces the ReplayView. Issued queries
then call query_part_issued with this evaluator/original/work and incoming depth.
Denial of genuine callback reads is installed only after actual retention.
Tests compare full public descriptors/handles and separately authenticated seals.
The snapshot wrapper remains mandatory next, not an unused fake entry in this unit.

### 2026-10-03 — Material clock-copy path included within eight

Read-only source review confirmed two real sealed-member clone sites in clock.rs.
Replace snapshot/occupancy.rs in this phase with that owning clock path; root
accepts this scope if the genuine direct owning fixtures above remain feasible.
Snapshot issued query adapter moves to REQUIRED frozen handoff, preserving full
feature scope. Member construction consumes an unshared actual origin and returns
its final leaf-bearing Rc; no mutation of an already shared completed member.
No separate precursor or ninth Rust path is needed. This remains a Ready-candidate
for root review, with no Rust implementation/source release from the draft.

### 2026-10-03 — Source implementation started

Root releases the exact revised eight-path manifest after geometry0008 accepted
397 Rust/590 editor tests and all gates. Original text baseline is saved in
query-authority-source-intent-0001.json. Implement shared transaction and real
owner hooks before envelope/source/replay transport; all acceptance remains pending.

### 2026-10-03 — Implemented eight-path source READY_HELD0001

Actual QState transaction installation now runs across observer root cycles and
issued root cycles. Initial raw retention seals genuine completed descendant
templates; replay issues fresh invocation leaves and uses one charged origin AND
leaf-bag memo for cached Events, inherited origins and reconstructed boundary
returned members. Completed records borrow through authenticated actual
OwnerInvocation/member interfaces, preserving original raw execution and rebound
clock/Index observations. Immutable published transcript owns no mutable ledger.

Internal row envelopes preserve every genuine primary and inherited invocation
and source binding bag through edits/union. Source binding consumes the actual
unshared origin, validates the issued child descriptor/scope/source entry, installs
the bag before final Rc creation, and finalizes only after actual returned
membership sealing. Empty completion stores the actual boundary and parent scope;
no fabricated member. All failure work remains consumed and prior views remain
separate. Public descriptor queries use the original algorithm and ignore
envelopes when issuance is disabled.

Eight genuine owning fixture bodies compile under actual isolated evaluation and
Freeze. They cover direct chord; shared raw/fresh invocation; nested cached
source/member replay; successful empty source; real fractional continuation
union; same/vary/use/foreign identity; deletion/overwrite; and whole exact/one-less,
depth, real callback fault and leaf drop. Missing and foreign ReplayViews refuse
before q. Fixtures explicitly configure validated max_nodes upper limit1,000,000,
shared actual-work allowance, never a default-admission or physical geometry
proof. Whole-operation exact/one-less uses actual measured consumption.

Source-grounded distinction: Slice adds timing metadata with Rc::make_mut AFTER
the original selected boundary seals its raw member. The augmented Event origin
can be a different Rc from that sealed member. Both must share the same authentic
fresh leaf/binding bag. Rebinder memoizes bags as well as origins; identical old
origin inputs map to identical new member Rc. Fixtures compare full descriptors
and separately assert member Rc identity in sealed boundary/fresh child clock,
plus augmented bag pointer identity.

Author compile-only001/002/003 passed;004 exited101 on two fixture VarSlotRef
equality assumptions. Repair uses actual slot identity for late/cells, without
namespace changes.005/006/007/008 passed, latest008 log empty. All original handles
are terminal. Full original source intent text baseline is retained; intermediate
pre004 fixture text was not separately saved, so no intermediate byte-equivalence
audit is claimed. Detailed handles/log hashes and all eight fixture names are in
query-authority-source-held-0001.json.

Scoped eight-file rustfmt/check passed; every touched Rust file is below1000.
Full cohort938 contains exactly six changed existing paths plus two declared
children compared with accepted geometry936; every other input is unchanged.
NO author behavioral tests, Clippy or WASM were run. Independent verification
and task acceptance criteria remain pending. Rust is held for checker; root
explicit EXECUTE is required. Snapshot/frozen/route/admission/vary phases remain
mandatory; no full song-mode completion claim.

### Repair checkpoint 0002: nested clock topology and memo reuse

Independent focused0001 test compilation succeeded; eight genuine fixtures ran, seven passed
and cached_outer failed at the immediate child clock member assertion. All938
inputs matched, and every checker handle terminated. The fixture had assumed
all contributor bindings were immediate clock sources; genuine inner/base
contributors can instead name a boundary in the authenticated parent chain.
The corrected witness traverses that original chain and requires both the exact
sealed boundary Rc and its exact returned member Rc, retaining full descriptor,
shared fresh bag, member authentication and denied-read assertions.

Production correction: insert_source/replace_source now consult the same charged
old-boundary memo before reconstructing descendant boundaries. The mapper is
scoped to one authentic original-to-current execution binding; direct old/next
replacement remains first. Repeated mapping of a completed child clock and its
actual OwnerInvocation must reuse the boundary used by transcript bindings.
The exact boundary/member pointer witness distinguishes this correction from
merely permitting traversal of parent clocks. No callbacks or certificates are
fabricated; independent verification of this repair remains pending.

Additional source-grounded correction in the same eight-path scope: raw selected
members seal before Slice augments timing metadata using Rc::make_mut. Leaf bags
alone do not retain every contributing augmented descriptor after equal-handle
union. IssuedQueryRow now retains actual (leaf bag Rc, augmented origin Rc) pairs
for every outer/inherited source frame at expansion. Union deduplicates only
identical pointer pairs, precharges scans/storage, and preserves all actual
origin records. Private original pair lists authenticate the envelope alongside
its contributor seals. The owning fractional continuation witness requires
distinct genuine query origins for the same handle, augmented timing beyond the
sealed raw member, and real freeze-copyable metadata on every retained pair.
Frozen handoff must copy these actual augmented descriptors independently of
raw member membership; it cannot reconstruct them from bags or handles.

The genuine same/vary fixture also replaces a row contribution with a distinct
actual pair from another issued row while preserving its private original pair
list, and requires Type refusal. This tests new pair authentication independently
of existing swapped invocation checks. Descriptor freeze comparisons use a
separate fixture comparison allowance and do not establish cumulative production
frozen-handoff acceptance. Author compile009 (36454/f26761) and010
(76391/7f3f12) terminated0 with empty logs; final fixture compile is recorded
in the refreshed held0002 receipt. No author behavioral commands were run.

### Repair checkpoint 0003: actual post-seal frame correspondence

Focused0002 executed eight genuine tests: seven passed, including the corrected
clock/boundary identity witness; fractional contribution metadata failed its
new per-frame strict timing-count assertion. Every938 input matched and all
checker handles terminated. Slice appends only to the immediate origin, while
expansion deliberately retains every inherited frame. Therefore not every
frame is required to gain a timing entry at that append.

The owning witness now matches the actual sealed member by full handle, issued
handle, source entry and shared authentic bag; all such contributor bindings
must reference that same member Rc. Real freeze copies compare original timing
prefixes exactly. The test additionally requires at least one contributed origin
that would be discarded by equal-handle union AND contains genuine post-seal
timing absent from its raw member. Full actual handle/entry/timing diagnostics
remain on missing correspondence or missing witness. Descriptor, distinct-origin,
callback-denial, swapped-pair, work and depth assertions remain unchanged.
No production Rust change is made at this checkpoint.

### Repair checkpoint 0004: genuine first-structure timing issuance

Focused0003 again executed eight tests, seven passed. The fractional witness
reached its required discarded-post-seal assertion but diagnostics showed all
eight authentic origin pairs had empty timing vectors. Full938 inputs matched
and all checker handles terminated. This is a fixture mode error, not an
observed metadata-copy defect: query_slice issues timing only when subject is
unstructured and Index is structured. The original subject p was structured.

The genuine public fixture now wraps the same selected original source in the
unstructured subject callback {beat -> p}, preserving the structured cut Index
and slow factor2/fractional arrangement. This exercises real first-structure
timing issuance. Full ordinary equality, authentic raw-member correspondence,
all continuation contributors, discarded appended metadata, pointer pair
authentication and callback-denial assertions remain unchanged. No production
Rust changes or fabricated timing records are introduced. Runtime verification
is pending the refreshed held0004 source.

### Broad0004: issued invocation compatibility companion

Native check passed; existing occupancy suite ran52 with51PASS/1FAIL, then
downstream scopes stopped. Original cached-child test still compared boundary
and member allocation identity across captures; genuine issuance intentionally
rebinds these while sharing raw execution. Separate two-path
[specific compatibility companion](../completed/song-mode-issued-invocation-compatibility.md)
is declared before changing that original test. Query8 remains focused accepted;
broad/native/lint/WASM/frontend closure awaits this companion's independent
gates. No old pointers are returned and issuance remains enabled.

### Accepted joined005 source gates, required consumers remain

Fresh405 distinct Rust cases, native/strict all-targets/WASM/union9 format and
root590 frontend/build pass. All938 sources and WASM target/deps/dist match.
Checker final SHA1bcf4b424f6b99344e883c047570585eb92c4d67d3c84f242d02cda3861145e0;
root receipt SHA7766920ea2a56f58dd7c49eea4329fdb1ea3e2debed3d3b20094b5caaca9f614.
TASK001/002 source and genuine query fixture criteria are verified. TASK003's
bank/control extension is authored in the required frozen companion, not yet
independently run. Overall remains In Progress: frozen envelopes and actual
routing consumption, admission and useful varying support are unfinished.

### Frozen002 authentic bank/control extension accepted

Following frozen handoff passes actual direct chord/sample-bank/control, nested
and postseal fractional fixtures, in accepted413 Rust gates plus590 frontend
and build. All940 unchanged; broad final SHA
cbd6667ec5647d4a39fad8f6b416edb176818ff3aa111acc8cd650d7b9388ede.
Propagation fixture criterion is now verified, including real bank/control
capture. Overall stays In Progress: actual production routing/scheduler and
pre-Reserve/varying consumers remain mandatory and unimplemented.
