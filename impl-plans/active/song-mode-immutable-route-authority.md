# Immutable route authority and attested topology copies

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture), [Route-view ownership gap](../../design-docs/references/song-mode/production-provenance-review-20261003.md#immutable-route-view-ownership-gap)

## Purpose and dependencies

Provide privately issued immutable route authority without retaining the
snapshot evaluator. Preserve original site/policy attestation across the trusted
topology copy used by SongRoutePlan. Public copied descriptors are not proof.

| Relation | Companion | Required output |
|---|---|---|
| Previous | Retained Index geometry | Accepted original geometry and lookup |
| Related | Query-issued song authority | Genuine query execution/invocation seals |
| Previous | Frozen issued-event handoff | Accepted owning envelopes retain complete query proofs |
| Depends On | Retained execution membership | Genuine shared raw execution check accepted; consumer adoption pending |
| Depends On | [Route preparation metering](song-mode-route-preparation-meter.md) | Actual nested depth and spent work on certification failure |
| Next | Production route consumers | Actual copied-site/member/geometry validation |
| Later | Admission/varying seeds | Pre-Reserve bounds and permitted execution domains |

Ownership, compatibility wrappers and signatures are reviewed. Source release
is limited to the exact eight paths after accepted meter0003 gates. A stored unused view is not
production integration; consuming admission and scheduler phases remain required.

## Exact proposed Rust manifest

| Module | Path | Status |
|---|---|---|
| Bounded retained publication | `src/song/snapshot/occupancy.rs` | Not Started |
| Immutable owning view | `src/song/snapshot/occupancy/route_view.rs` (new) | Not Started |
| View-based lookup seam | `src/song/snapshot/occupancy/lookup.rs` | Not Started |
| Cohesive authentication extraction | `src/song/snapshot/occupancy/lookup/authority.rs` (new) | Not Started |
| Private routing registration | `src/song/routing.rs` | Not Started |
| Attested preparation companion | `src/song/routing/prepare.rs` | Not Started |
| Caller-metered route traversal/certification | `src/song/routing/prepare/builder.rs` | Not Started |
| Private prepared owner/fixtures | `src/song/routing/prepared.rs` (new) | Not Started |

Eight paths maximum. lookup.rs is near1000 lines: extract authentication helpers
cohesively before growth. Keep snapshot forwarding thin and every touched file
below1000. Define the thin impl SongSnapshot issuer in route_view.rs, which
can access ancestor-private fields. Include genuine owning fixtures in the
declared prepared child.

## Candidate ownership declarations

These are proposed private contracts, not present APIs. All fields below are
private. `Rc` shares the immutable owned allocations; no public `Rc` getter or
mutable plan/view access is exposed. The view is not `Clone` by value.

```rust
enum PayloadSlot {
    Capture { part: usize, track_slot: usize },
    Edit { part: usize },
}
struct SiteSeal;
struct PolicySeal;
struct ViewSite {
    slot: PayloadSlot,
    seal: Rc<SiteSeal>,
    policies: Vec<ViewPolicy>,
}
struct ViewPolicy {
    source_slot: usize,
    seal: Rc<PolicySeal>,
}
struct PublishedRetainedIndex {
    request: CanonicalIndexRequest,
    site: Rc<SiteSeal>,
    invocations: Vec<Rc<OwnerInvocation>>,
}
pub(crate) struct RouteAuthorityView {
    original: Rc<crate::song::Song>,
    inventory: Rc<FrozenRoutingInventory>,
    resource_count: usize,
    pcm_bytes: u64,
    sites: Vec<ViewSite>,
    records: Vec<PublishedRetainedIndex>,
}
struct PreparedSiteBinding {
    copied_slot: PayloadSlot,
    view_site: usize,
    seal: Rc<SiteSeal>,
}
struct PreparedPolicyBinding {
    copied_site: usize,
    copied_source_slot: usize,
    view_policy: usize,
    seal: Rc<PolicySeal>,
}
pub(crate) struct PreparedRoutes {
    plan: SongRoutePlan,
    authority: Rc<RouteAuthorityView>,
    sites: Vec<PreparedSiteBinding>,
    policies: Vec<PreparedPolicyBinding>,
}
pub(crate) struct PreparedSiteRef<'a> {
    owner: &'a PreparedRoutes,
    binding: &'a PreparedSiteBinding,
}
pub(crate) struct PreparedPolicyRef<'a> {
    owner: &'a PreparedRoutes,
    binding: &'a PreparedPolicyBinding,
}
```

Cross-module access uses crate-visible borrowed methods, not public fields.
Re-export the view from occupancy and the prepared references from routing
inside the declared parent paths. Only the issuer/copy implementation allocates
seals. The routing owner asks the view to issue its topology copy and stores
the returned opaque binding set; it never accesses or constructs seal fields.

`PublishedRetainedIndex` copies the private request's complete fields and its
prefix, retains the original recipe `Rc`, and shares completed invocation `Rc`s.
Current lookup reads `request` and `invocations`; it does not need the evaluator,
callback journal, or a clone of raw observations. Invocation records already
own their actual seed, entry, observations and clock. Never replace them with
reconstructed owner keys. Query-issued rebound invocations remain the separate
issued-transcript dependency; this view does not manufacture missing executions.

`SiteSeal` and `PolicySeal` are private allocated issuance identities, not
numeric IDs or descriptor fingerprints. They certify a trusted copy relation,
not the validity of arbitrary public descriptor values. No seals contain
references to records, clocks, evaluator or their owning view.

## Issuance and trusted rebinding contract

The following order is mandatory; a bare `snapshot.routing.clone()` is not an
authority issuer.

1. While the original snapshot is borrowed, authenticate each retained request
   against that snapshot's actual payload and original recipe `Rc`; authenticate
   every retained invocation's original Song membership and actual depth.
2. Enumerate actual original Capture track slots and Edit payloads. Associate
   each authenticated request with its exact original payload allocation.
   Enumerate each payload's actual selected-source descriptor allocations.
3. Copy every inventory field into the view through the trusted copier. Preserve
   the original immutable graph/recipe sharing and all source-family order and
   source-use topology. Allocate seals for the actual original-to-view slot pairs
   during that traversal; never infer pairs by searching equal-looking values.
4. Publish requests with their original recipe references and the issued site
   seal. Their scope/track/root still describe the original Song; the seal attests
   their validated transfer to the new stable payload allocation. No old raw
   payload/descriptor pointer is saved or dereferenced after publication.
5. When preparing the owned plan, perform its topology copy from the issued
   view through the same bounded trusted-copy operation. Mint prepared bindings
   for exact view-to-plan slot pairs, sharing those seals. Reject missing,
   duplicate or ambiguous pairs before publishing `PreparedRoutes`.

The complete copy includes instruments/parameters/defaults, buses, every Part
variant and edit, tracks, source metadata, cells, resource sites, families,
routes, selected policies, source-use graph and timing recipe attachments.
Charge vector/string/scalar copies, visited nodes, seal allocations and `Rc`
retention according to their actual sizes. Graph/recipe/immutable invocation
sharing must not silently become deep copying. No allocation addresses are
used as public source/configuration identity.

## Proposed issuing and lookup signatures

All functions inherit caller work and depth; none initialize a default quota.
The snapshot forwarder is thin. Definitions requiring private snapshot access
belong in its occupancy descendants. Path visibility is crate-only as needed.

```rust
impl SongSnapshot {
    pub(crate) fn issue_route_authority(
        &self, limits: SongLimits, remaining: &mut u32, depth: u32,
    ) -> Result<Rc<RouteAuthorityView>, Failure>;
}
pub(crate) fn publish_route_authority(
    snapshot: &SongSnapshot,
    limits: SongLimits, remaining: &mut u32, depth: u32,
) -> Result<Rc<RouteAuthorityView>, Failure>;
pub(crate) fn prepare_routes_issued(
    authority: Rc<RouteAuthorityView>,
    settings: SongSettings, caps: &CapabilitySet,
    available: &SongHostCapacities,
    limits: SongLimits, remaining: &mut u32, depth: u32,
) -> Result<PreparedRoutes, Failure>;
impl PreparedRoutes {
    pub(crate) fn plan(&self) -> &SongRoutePlan;
    pub(crate) fn site(
        &self, scope: usize, track: KwId,
        limits: SongLimits, remaining: &mut u32, depth: u32,
    ) -> Result<PreparedSiteRef<'_>, Failure>;
    pub(crate) fn policy<'a>(
        &'a self, site: PreparedSiteRef<'a>, source_slot: usize,
        limits: SongLimits, remaining: &mut u32, depth: u32,
    ) -> Result<PreparedPolicyRef<'a>, Failure>;
}
pub(crate) struct BoundOriginalSite<'a> {
    view: &'a RouteAuthorityView,
    site: &'a ViewSite,
}
impl PreparedSiteRef<'_> {
    pub(crate) fn bind_original(
        &self, limits: SongLimits, remaining: &mut u32, depth: u32,
    ) -> Result<BoundOriginalSite<'_>, Failure>;
}
impl BoundOriginalSite<'_> {
    pub(crate) fn payload(&self) -> &FrozenPattern;
}
pub(crate) fn owner_addresses_issued<'a>(
    site: PreparedSiteRef<'a>, issuer: NodeId,
    prefix: &[FrozenUseTraceTerm], owner_window: TimeSpan,
    limits: SongLimits, remaining: &mut u32, depth: u32,
) -> Result<Vec<RetainedOwnerAddress<'a>>, Failure>;
pub(crate) fn bind_issued_owner<'a>(
    site: PreparedSiteRef<'a>, issuer: NodeId,
    prefix: &[FrozenUseTraceTerm], owner_window: TimeSpan,
    transcript: &'a IssuedQueryTranscript, seal: &'a Rc<InvocationSeal>,
    work: &SharedIndexWork, depth: u32,
) -> Result<RetainedOwnerAddress<'a>, Failure>;
impl RetainedIndexAddress<'_> {
    pub(crate) fn validate_prepared_binding(
        &self, site: PreparedSiteRef<'_>, issuer: NodeId,
        prefix: &[FrozenUseTraceTerm],
        limits: SongLimits, remaining: &mut u32, depth: u32,
    ) -> Result<(), Failure>;
}
```

`settings` must match the original Song settings retained by the view; an
arbitrary tempo/tail argument cannot alter the attested preparation domain.
Existing `prepare_routes(snapshot, caps, available)` remains compatible. Its
public plan does not acquire opaque authority merely because it was returned
from this API. The issued companion must meter traversal from `remaining`,
including RouteBuilder work, rather than invoke the existing default-million
wrapper and reset its allowance.

`owner_addresses_issued` selects only retained request records attached to the
issued site's seal, then checks original recipe, complete issuer/prefix, window
coverage and actual seed-bearing invocations. The numeric scope/track lookup
above merely borrows an already issued binding; it cannot mint authority from
caller-provided FrozenPattern or FrozenSelectedSource values.

Lookup follows the binding to the view's stable payload/selected-policy
allocation. `selected_policy` receives the canonical view descriptor reached
through `PreparedPolicyRef`; its existing exact descriptor and policy-path
checks operate on this view. A caller cannot pass a copied plan descriptor
directly and gain proof by scalar equality. Member binding still uses genuine
retained source boundaries and complete source/use/copy identity.

The distinct bind_issued_owner entry selects a request privately from the
view's retained records using the attested site and complete issuer/prefix/window.
It authenticates that stored request's original recipe/site attachment, obtains
transcript.invocation(seal) and scans only that site's retained records. Caller
issuer/prefix/window are selection inputs and cannot mint request authority.
No SongSnapshot-dependent request constructor is needed after evaluator drop. Charge each candidate and full key; require exact retained
execution membership via the provenance API. Then validate actual owner, seed,
entry, admitted depth, original recipe and complete requested coverage before
constructing the address with the fresh issued invocation. Retained invocation
supplies execution/coverage authority; its old clock must not supply geometry.
Both view and transcript outlive the resulting borrowed address.

This entry receives the same SharedIndexWork as query/freezing/resolution.
Bridge legacy ProjectionBudget operations serially, debit actual consumed work
on success and failure, and release mutable collector borrows before transcript
authentication. Retained-only compatibility APIs keep their existing remaining
counter signatures. Do not initialize a new default quota in either adapter.

The SharedIndexWork bridge reads the original limits and current remaining under
a short immutable collector borrow, drops that borrow, then runs the legacy
ProjectionBudget operation against a local remaining counter. On either result,
charge the checked initial-minus-final debit back to the same collector before
returning. Transcript authentication occurs outside this operation or after
writeback; it cannot debit the collector between the initial snapshot and local
writeback. No RefCell borrow survives a call that authenticates a transcript or
otherwise charges SharedIndexWork. Precharge any additional invocation lookup
scan not already covered by its authenticating API within a declared Route8
adapter; do not add a provenance path merely to move that charge. Owning fixtures
must exercise success and failure through this bridge and preserve prior debits.

## Snapshot compatibility and size gate

Keep existing `owner_addresses(snapshot, request, ...)`, `bind_index` and
snapshot-fixture call signatures. Internally use a borrowed authority backend:

```rust
enum AuthorityBorrow<'a> {
    Snapshot(&'a SongSnapshot),
    Issued(&'a RouteAuthorityView),
}
```

Retained addresses borrow that backend rather than keeping only a snapshot
reference. Existing snapshot lookup follows original pointers unchanged; issued
lookup follows issued site bindings. Extract payload/request authentication,
authority backend access and selected-policy authentication into the declared
`lookup/authority.rs` before growing the current lookup parent (observed958).
This does not require changing clock/projection/recipe modules or observer
collector fields. New private fixtures stay in `prepared.rs` and use original
public candidate/query construction. Frozen extraction leaves snapshot905;
define its thin issuing impl in the declared route_view descendant, with no
snapshot.rs edit. Stop for manifest revision if extraction still cannot keep
the lookup parent below1000; no implicit ninth module.

## Publication and fixture obligations

- Fully authenticate and charge copying/record/binding retention before
  publishing a view or prepared owner. Failure discards partial collections,
  preserves prior snapshot evidence and retains consumed caller work.
- Construct a genuine candidate, retain actual Index invocations, issue and
  prepare its view, then drop the candidate/prepared snapshot and evaluator.
  Resolve copied site/policy/member geometry through the surviving owner with
  no callback execution. Snapshot-dependent query realization is not claimed.
- Preserve current original-snapshot lookup fixtures unchanged in behavior.
- Independently cloned/mutated public topology never produces a binding.
  Foreign-owner site/policy references and swapped equal-looking policies
  refuse while returning/retaining their original opaque owners as applicable.
- Verify exact and one-less work/depth at view issuance and trusted plan copy;
  every failure publishes no partial authority. Keep actual source START,
  original whole, seed, member and copy distinctions in geometry assertions.
- Following issued-event/routing phases must consume these references in
  actual `resolve_issued_route`; this phase supplies no uniform varying-seed
  admission proof or host readiness by storing an unused owner.

## Tasks

### TASK-001: Immutable view and lookup ownership

**Status**: Not Started
**Parallelizable**: No

- [ ] Finalize owning-view issuance, original record/recipe validation and
  compatibility for current snapshot-based genuine fixtures.
- [ ] Extract authentication helpers before lookup grows to1000 lines.
- [ ] Preserve original source/use, seed, placement and member distinctions.
- [ ] Prove the issued view remains usable after evaluator ownership is dropped.

### TASK-002: Attested topology copy and prepared owner

**Status**: Not Started
**Parallelizable**: No; depends on TASK-001

- [ ] Mint opaque site/policy bindings during the actual trusted inventory copy.
- [ ] Preserve public prepare_routes compatibility and keep the new owner private.
- [ ] Charge copying, bindings and record retention under inherited original work.
- [ ] Reject independently cloned, mutated, foreign or swapped policy plans.

### TASK-003: Genuine consumer fixtures and acceptance

**Status**: Not Started
**Parallelizable**: No; depends on TASK-002

- [ ] Genuine copied-policy lookup and source-member geometry work through
  issued bindings without callback execution.
- [ ] Actual retained seed/member/copy distinctions survive trusted copying.
- [ ] Exact/one-less work/depth and foreign/swap failures publish no owner.
- [ ] Independent tests/lint/WASM/format pass against held source inputs.
- [ ] Concrete following consumer/admission manifests use this owner; storing
  an unused view or forwarding to scalar-only routing is not completion.

## Completion criteria

- [ ] All tasks and genuine copy-authority fixtures pass.
- [ ] No evaluator/VM retention and no forgery through public mutable descriptors.
- [ ] Original work/depth and public preparation compatibility are preserved.
- [ ] Actual host admission, scheduler geometry and varying-seed adoption remain
  explicit required work until independently verified.

## Progress log

### 2026-10-03 — Source-grounded ownership draft

prepare_routes clones public topology, but retained lookup authenticates original
payload and selected descriptor allocations. Reviewer proposes a private owner
and attested trusted-copy bindings rather than scalar reconstruction or retaining
the snapshot evaluator. Exact eight-path proposal includes lookup extraction.
This is Planning only; geometry and query authority work precede source release.

### 2026-10-03 — Concrete publication and copy-binding review

Replaced placeholder ownership with privately owned frozen inventory, issued
site/policy seals, copied retained requests and shared actual invocation records.
Publication validates requests against original snapshot allocations BEFORE
copying; newly cloned inventory pointers are not asserted to be original pointers.
Defined inherited work/depth entries, issued backend compatibility and trusted
view-to-plan bindings. The eighth path remains authentication extraction, with
lookup observed958 and snapshot942. No Rust edits, tests or Cargo were performed.
Planning remains: author must confirm extraction/copy plumbing fits this exact
manifest and following actual issued-event/route consumers remain mandatory.

### 2026-10-03 — Builder ownership and retained membership prerequisite reviewed

Read-only source review finds prepare/builder.rs hardcodes1M work/256 depth in
walk24–34 and cover230–265. Editing prepare.rs alone cannot satisfy inherited
remaining/depth. Replace the proposed snapshot.rs edit with builder.rs in this
exact8 manifest; the new route_view descendant can define impl SongSnapshot
without editing the parent. Preserve existing default wrappers while issued
preparation meters the actual builder and cover traversal from caller work.

Live transcript now returns actual rebound OwnerInvocation and source member
references. The earlier missing fresh-clock/observations access is resolved.
Exact retained execution membership remains inaccessible outside eval; a narrow
[retained membership prerequisite](song-mode-retained-execution-membership.md)
in provenance must authenticate original/seal/retained invocation
using its genuine completed execution pointer without exposing records or seal
constructors. Declare this as a separate bounded prerequisite before edits,
not a ninth path hidden in this route bridge. This plan stays Planning.

Issued lookup borrows genuine fresh invocation/member evidence, authenticates
retained execution membership and trusted prepared site binding, and projects
that actual invocation's rebound observations/clocks. Do not substitute original
raw observations or compare fresh invocation pointers to retained invocation
pointers. Retained coverage and full source/use groups/START eligibility remain
required. Later permitted first-execution/varying domains remain mandatory and
must not be permanently restricted to exact retained-execution cache hits.

### 2026-10-03 — Frozen predecessor accepted; explicit issued adapter required

Frozen0002 passes413 Rust cases and590 frontend tests plus native, lint, WASM,
format and build, all940 inputs unchanged after terminal. Membership2 now
implements the narrow caller-metered retained raw execution check; its genuine
fixtures and acceptance are pending. This plan stays Planning.

The proposed retained-address enumeration does not yet show a typed entry for
the actual query transcript/seal. Finalize that adapter with the author and
read-only reviewer within the declared eight paths. It must authenticate the
genuine completed raw execution, then use the issued invocation's actual
placement/seed/entry/depth/clock/observations and trusted site/policy binding.
An unused membership helper or geometry from the old retained invocation does
not satisfy this bridge. Do not add an implicit ninth provenance edit here.

### 2026-10-03 — Explicit fresh-invocation adapter reviewed

Independent read-only review confirms bind_issued_owner fits the declared
lookup/authority child. Existing lookup_owner/seed/entry/clock/depth and
observations getters suffice; no replay/provenance module must be added to
route8. Replace the address's snapshot-only reference with the borrowed
Snapshot/Issued backend while keeping request and fresh invocation borrows.
Retained-only enumeration remains a separate companion. Exact raw execution
membership does not authorize substituting original retained clocks for the
actual rebound query clocks. Author signature/fixture feasibility is pending;
this plan stays Planning while membership2 is implemented.

### 2026-10-03 — Preparation cost audit before Route8 release

The five-path preparation prerequisite is compiled and held for independent
verification; Route8 remains Planning. The current preparation entry and builder
reset work/depth to fixed defaults. The issued entry must retain the original
SongLimits, direct caller remaining counter and actual inherited depth; legacy
wrappers supply their existing defaults. Builder certification must call the
metered adapter without charging consumed_work again. Density must receive the
same counter directly and a checked remaining relative depth; nested preparation
must use its metered adapter. Existing overlap work already debits the counter.

The source audit also identified track/template setup, branch-demand aggregation,
sidechain scans and final topology copying that need charges before allocation.
Calls to existing source::validate_detectors and graph compilation helpers must
be preceded by documented checked input-size upper bounds covering searches,
edge/node storage, traversal and diagnostic work. These call sites belong to the
eight declared paths, so no ninth source path is required. If exact operation
metering cannot be covered there, declare a separate prerequisite before edits.

These CPU-work bounds do not establish physical resource capacity or default
admission for varying seeds; those remain separate full-goal requirements.

### 2026-10-03 — Resource ownership and post-snapshot request selection

Actual prepare.rs reads resource_count and pcm_bytes for host demand. The view
must copy those authentic snapshot totals during issuance, without retaining
resources or evaluator solely to recover them. Song settings come from original
Song. The issued adapter signature above now selects a privately retained request
from site/issuer/prefix/window; earlier draft request-argument recommendations
are superseded. CanonicalIndexRequest::mint currently requires SongSnapshot, so
requiring consumers to mint one after snapshot drop would defeat the owning view.
Selection must preserve ambiguity/failure checks, full key charging and actual
fresh invocation clock. All changes remain within the proposed eight paths.

The current transcript invocation API charges authentic's scan and then performs
a separate find without exposing transcript length. Within Route8, authenticate
once under SharedIndexWork and measure that charged delta, which is the actual
transcript length plus one. Precharge this delta as an upper bound for the find
before invoking invocation; its own authentication still charges normally.
Document this deliberate conservative charge and include it in exact/one-less
fixtures. Do not infer transcript length from retained-view record count.

### 2026-10-03 — Author confirms executable eight-path ownership contract

Author reviewed the refined draft and confirms no ninth path is needed. The
lookup backend provides borrowed original Song, inventory and retained-record
iteration for Snapshot and Issued variants. Extract authentication/site/source
policy helpers before growing the current958-line lookup parent. PreparedRoutes
exposes only a borrowed public plan; private site/policy bindings reach canonical
view descriptors. The issued address borrows its request from the view and its
fresh actual invocation clock from the transcript, which both outlive it.
Actual seed/entry/depth/recipe and complete coverage remain mandatory checks.

The declared preparation core can preserve settings and authenticated resource
totals while using the original policy/counter/depth across builder, density,
direct certification and nested preparation. Density's relative depth is checked
once. Real input-size precharges cover detector, compilation and topology copy
helpers in existing declared call sites. Mark Ready and release source only
after the preparation prerequisite's independent gates are accepted.

The evaluator-drop owning fixture helper belongs in route_view.rs, a snapshot
descendant with legitimate private access; the routing test child cannot reach
PreparedSong's private snapshot/evaluator fields. Proposed test-only signature:

```rust
#[cfg(test)]
pub(crate) fn capture_test_route_authority(
    code: &str, limits: SongLimits, remaining: &mut u32, depth: u32,
) -> Result<(Rc<RouteAuthorityView>, FrozenIssuedBatch), Failure>;
```

This helper evaluates a real candidate, mints genuine requests, retains actual
invocations, queries issued events and issues the owning view using the original
counter; it drops PreparedSong/evaluator before returning. Bulk assertions live
in prepared.rs. A public legacy comparison plan may be captured while snapshot
exists, solely as separate descriptor comparison evidence; it cannot supply
issued authority or hide the cumulative issued work cost. Genuine foreign,
mutated/copy, seed/entry, exact-budget and failure fixtures remain required.

### 2026-10-03 — Ready after full preparation prerequisite acceptance

Meter0003 focused eight and broad425 passed; native, strict all-target lint,
fresh pureWASM and scoped5 formatting passed. Original broad62099 terminated
066282/0; all author/checker handles terminal. Broad receipt
SHA176932ec20d76cc149c8c722be765a74adfb764f0879b57781e02335e534ef76.
Root fresh frontend590/78 and build passed against identical top/deps/dist
155098702-byte WASM SHA0175699ff49e9fd5cb48f88caaf266368a3358f93c0fa9b8a62b8326a725b8b8.
Root receipt SHA886e97e0f8235d78f303e641065033b0bfc7140f1ad2d15ab544db3ab68b932c.
All944 source inputs unchanged after final root terminal handles. Exact8 below
may now execute; retain original texts and extraction checkpoints before edits.
No downstream resolver/scheduler/admission/structural path is released implicitly.

### Implementation intent and lookup extraction checkpoint

Source release follows accepted425 Rust tests/590 frontend and build, all944
exact. Exact8 implementation now started. Lookup payload/request/site and policy
authentication helpers moved cohesively into declared authority child before
semantics. Only parent-subtree method visibility and module import changed;
full extracted texts saved in immutable-route-authority-lookup-extraction-only-0001.json.
Fresh issued clock, original counter and trusted copy bindings remain required.
No production routing acceptance is inferred from this extraction.
