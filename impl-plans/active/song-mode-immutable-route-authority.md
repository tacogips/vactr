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
// Session 250 amendment: the eight-parameter form failed
// clippy::too_many_arguments. Caller selection inputs are grouped into a
// borrowed selector; authentication semantics are unchanged.
pub(crate) struct IssuedOwnerSelector<'a, 'p> {
    pub(crate) site: PreparedSiteRef<'a>,
    pub(crate) issuer: NodeId,
    pub(crate) prefix: &'p [FrozenUseTraceTerm],
    pub(crate) owner_window: TimeSpan,
}
pub(crate) fn bind_issued_owner<'a>(
    selector: &IssuedOwnerSelector<'a, '_>,
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

**Status**: Completed
**Parallelizable**: No

- [x] Finalize owning-view issuance, original record/recipe validation and
  compatibility for current snapshot-based genuine fixtures.
- [x] Extract authentication helpers before lookup grows to1000 lines.
- [x] Preserve original source/use, seed, placement and member distinctions.
- [x] Prove the issued view remains usable after evaluator ownership is dropped.

### TASK-002: Attested topology copy and prepared owner

**Status**: Completed
**Parallelizable**: No; depends on TASK-001

- [x] Mint opaque site/policy bindings during the actual trusted inventory copy.
- [x] Preserve public prepare_routes compatibility and keep the new owner private.
- [x] Charge copying, bindings and record retention under inherited original work.
- [x] Reject independently cloned, mutated, foreign or swapped policy plans.

### TASK-003: Genuine consumer fixtures and acceptance

**Status**: In Progress
**Parallelizable**: No; depends on TASK-002

- [x] Genuine copied-policy lookup and source-member geometry work through
  issued bindings without callback execution.
- [x] Actual retained seed/member/copy distinctions survive trusted copying.
- [x] Exact/one-less work/depth and foreign/swap failures publish no owner.
- [x] Independent tests/lint/WASM/format pass against held source inputs; the
  wave-1 Clippy diagnostics are limited to their declared dispositions.
- [ ] Concrete following consumer/admission manifests use this owner; storing
  an unused view or forwarding to scalar-only routing is not completion.

## Completion criteria

- [ ] All tasks and genuine copy-authority fixtures pass.
- [x] No evaluator/VM retention and no forgery through public mutable descriptors.
- [x] Original work/depth and public preparation compatibility are preserved.
- [x] Actual host admission, scheduler geometry and varying-seed adoption remain
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

### Session 249 implementation and verification

TASK-001 and TASK-002 are complete. The nine owning-fixture tests are present
and passed within the focused suite. The all-target Clippy gate remains
unaccepted: `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
exited 101, and reports one `clippy::too_many_arguments` diagnostic for the
plan-pinned eight-argument `bind_issued_owner` signature. Every other diagnostic
is `dead_code` or `unused_imports` on the wave-1 disposition table. That
signature remains as designed, with no lint suppression attribute added; this
specific acceptance mismatch is recorded for integration review. TASK-003 and
the overall plan remain In Progress pending that disposition and downstream
consumer work.

Foreground gates (full logs under `tmp/song-mode-riela/`):

- `CARGO_TERM_QUIET=true cargo build` — exit 0;
  `session249-route8-build.log`.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` — exit 101;
  `session249-route8-clippy.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/prepared::tests|occupancy::|routing::nested::preparation::meter_tests/)'` — exit 0, 88 passed, 0 failed;
  `session249-route8-nextest-focused.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` — exit 0, 2767 passed, 3 skipped;
  `session249-route8-nextest-full.log`.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit 0;
  `session249-route8-wasm.log`.
- `rustfmt --edition 2021 --check src/song/snapshot/occupancy.rs src/song/snapshot/occupancy/route_view.rs src/song/snapshot/occupancy/lookup.rs src/song/snapshot/occupancy/lookup/authority.rs src/song/routing.rs src/song/routing/prepare.rs src/song/routing/prepare/builder.rs src/song/routing/prepared.rs` — exit 0, no diffs;
  `session249-route8-fmt.log`.
- `git ls-files -co --exclude-standard -z -- '*.rs' Cargo.toml Cargo.lock | xargs -0 shasum -a 256` — exit 0, 947 cohort inputs: baseline 944 plus exactly `route_view.rs`, `lookup/authority.rs`, and `prepared.rs`; the only changed existing hashes are the five Route8 paths and pre-existing drift in `src/sched/runtime/song/clock_tests.rs`, `src/song/snapshot/reservations_tests.rs`, and `src/song/snapshot/resources.rs`; `session249-route8-cohort.sha`.
- `wc -l` on the eight authorized Rust paths — exit 0; counts are 586, 586, 841, 318, 648, 573, 605, and 645 (all below 1000).
- Added-attribute scan (`git diff -- <eight paths> | grep -E '^\+.*#\[(allow|expect)'`) — no matches; `/tmp/session249-route8-attrs.log`.
- Agent verification `CARGO_TERM_QUIET=true cargo check` — exit 0;
  `/tmp/song-route8-compile-check/final2-cargo-check.log`.

### Session 250 design amendment (no Rust edits)

The operator chose to amend the signature instead of suppressing the lint.
`bind_issued_owner` now takes a borrowed `IssuedOwnerSelector` and has five
parameters, with unchanged semantics. See "Session 250 amendment" below. The
session-249 entry above attributed three changed cohort hashes to
"pre-existing drift". That attribution is withdrawn: those paths are unowned,
and the run must not have changed them. The fresh gates use
`session250-route8-*` evidence. TASK-003 and the plan stay In Progress.

### Session 250 implementation and final-source gates — 2026-10-03

Applied the selector repair specified above. `IssuedOwnerSelector` groups the
site, issuer, prefix and owner window; `bind_issued_owner` now has five
parameters. Its body retains the same authentication, candidate selection,
charging and return order after selector-field substitution. The two bridge
fixture calls pass the selector with assertions and loop structure unchanged.
`prepared.rs` imports `RouteAuthorityView` through the occupancy re-export, so
D02 is absent. A first scoped format check found only the import's ordering;
the import was reordered and the final scoped check passes. The earlier logs
are preserved under
`tmp/riela-fanout/BB4EA297-A8CA-4E97-9E5F-8D3741E5B6C7/step6-prior-gates/pre-import-order-fix/`.

Final-source command results:

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session250-route8-build.log 2>&1` — exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session250-route8-clippy.log 2>&1` — exit 101; only mapped `dead_code`/`unused_imports` rows D01 and D03-D32; no `too_many_arguments`. Wave 1 permits these disposition rows; exit 0 is required from wave 3.
- `CARGO_TERM_QUIET=true cargo clippy --lib --profile test -- -D warnings > tmp/song-mode-riela/session250-route8-clippy-libtest.log 2>&1` — exit 101; D01 only.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/prepared::tests|occupancy::|routing::nested::preparation::meter_tests/)' > tmp/song-mode-riela/session250-route8-nextest-focused.log 2>&1` — exit 0; 88 passed.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker >> tmp/song-mode-riela/session250-route8-nextest-focused.log 2>&1` — exit 0; 105 passed.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session250-route8-nextest-full.log 2>&1` — exit 0; 2,767 passed, 3 skipped.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session250-route8-wasm.log 2>&1` — exit 0.
- `rustfmt --edition 2021 --check <the eight Rust writePaths> > tmp/song-mode-riela/session250-route8-fmt.log 2>&1` — exit 0, no diffs.
- `git ls-files -co --exclude-standard -z -- '*.rs' Cargo.toml Cargo.lock | xargs -0 shasum -a 256 > tmp/song-mode-riela/session250-route8-cohort.sha` — exit 0; 947 inputs, exactly three additions, five existing Route8 hash changes, no unowned changes.
- The three unowned paths matched `HEAD` before the gates (diff exit 0); all eight Rust files are below 1,000 lines; both allow/expect scans found no matches.
- Receipt: `tmp/song-mode-riela/session250-route8-receipt.json`; `shasum -a 256` exit 0, fingerprint `cc5c9c3457f181d9d89ad99edcdf8346d009d648aa655c9a9cb6b2cc052263ce`, different from the rejected fingerprint. The lookup/authority and prepared.rs hashes also differ from session 249.

The test-integrity, adversarial and integration reviews remain pending their
downstream workflow steps. No review acceptance is claimed here. Production
consumer/admission work remains assigned to later plans, so this plan remains
In Progress.

### Session 255 status note (2026-10-04, plan author; no Rust edits)

SONG-ROUTE8 was accepted at `1ac457f` by the session-250 reviews (dispatch
manifest `acceptedDependencies`). It is not redispatched in session 255. Its
eight paths are edited only as declared writePaths or sharedPaths of later
plans:

- `lookup/authority.rs` and `prepared.rs` (2a, wave 3);
- `occupancy.rs`, `lookup.rs` and `route_view.rs` (2a, 2b seams);
- `routing.rs` (wave 3 re-export seam).

The remaining TASK-003 consumer wiring for `LookupAuthority::Issued`,
`with_work`, `bind_issued_owner` and `PreparedRoutes` is completed by 2a and
wave 3. The proof is strict Clippy exiting 0 from wave 3, with no D-row left.
SONG-16 sets this plan's status to Completed only after its final gates pass.
The held exact-947 Route8 cohort stays documented in
`tmp/song-mode-riela/session250-route8-cohort.sha`. The current cohort is 953.

## Session 249 executable contract (wave 1)

The source of truth is
[the production integration contract](../../design-docs/specs/design-song-mode.md#production-integration-contract-route-authority-to-playback-2026-10-03),
Wave 1 items 1-5. This section supersedes the earlier "Not Started" markers in
the manifest table. The 37ea3e8 checkpoint already implements most of the
deliverables.

```json
{
  "planId": "SONG-ROUTE8",
  "planPath": "impl-plans/active/song-mode-immutable-route-authority.md",
  "wave": 1,
  "dependsOn": [],
  "writePaths": [
    "src/song/snapshot/occupancy.rs",
    "src/song/snapshot/occupancy/route_view.rs",
    "src/song/snapshot/occupancy/lookup.rs",
    "src/song/snapshot/occupancy/lookup/authority.rs",
    "src/song/routing.rs",
    "src/song/routing/prepare.rs",
    "src/song/routing/prepare/builder.rs",
    "src/song/routing/prepared.rs",
    "impl-plans/active/song-mode-immutable-route-authority.md",
    "tmp/song-mode-riela/session250-route8-intent.json",
    "tmp/song-mode-riela/session250-route8-receipt.json",
    "tmp/song-mode-riela/session250-route8-cohort.sha",
    "tmp/song-mode-riela/session250-route8-build.log",
    "tmp/song-mode-riela/session250-route8-clippy.log",
    "tmp/song-mode-riela/session250-route8-clippy-libtest.log",
    "tmp/song-mode-riela/session250-route8-nextest-focused.log",
    "tmp/song-mode-riela/session250-route8-nextest-full.log",
    "tmp/song-mode-riela/session250-route8-wasm.log",
    "tmp/song-mode-riela/session250-route8-fmt.log"
  ],
  "sharedPaths": []
}
```

Session 250 replaced the `session249-route8-*` evidence paths with
`session250-route8-*`. The session-249 files are read-only historical evidence
of the failed Clippy gate. Workers must not overwrite them. The Rust writePaths
are unchanged.

### Intent and context

The user wants static song code to produce a complete song that ends on its own.
Production routing must use authenticated immutable authority, not copied public
descriptors. Checkpoint 37ea3e8 already contains the following:

- `RouteAuthorityView` (`route_view.rs:29`);
- `TrustedRouteCopy` (`route_view.rs:37`);
- `publish_route_authority` (`route_view.rs:265`);
- `SongSnapshot::issue_route_authority` (`route_view.rs:386`);
- `PreparedRoutes` and `prepare_routes_issued` (`prepared.rs:13`, `prepared.rs:33`);
- `with_work` and `bind_issued_owner` (`lookup/authority.rs:194`,
  `lookup/authority.rs:211`).

Production does not call any of them yet. The author compile log
`tmp/song-mode-riela/vactr-route-authority-author-compile-008.log` has 32
dead-code warning lines.

This wave finishes the slice:

- legacy-compatible preparation;
- input-size precharges on the issued path;
- a pre-Reserve retention entry for later waves;
- genuine owning fixtures;
- the exact 947-input cohort.

### Non-goals

- Do not edit any file outside writePaths. This includes nested.rs,
  nested/preparation.rs, snapshot.rs, routing/index.rs, the
  host/scheduler/replay/clock files, editor/, and the unowned paths
  `src/song/snapshot/resources.rs`, `src/song/snapshot/reservations_tests.rs`
  and `src/sched/runtime/song/clock_tests.rs`. Never format, stage or restore
  these paths. Run rustfmt in write mode only on the eight Rust writePaths.
- Do not wire production callers. That happens in wave 3.
- Do not write an issued resolver. That happens in wave 2.
- Do not add varying-seed admission guards.
- Do not add `allow` or `expect` lint attributes.
- Do not change the public signature of `prepare_routes`.
- Do not change any existing test's assertions.

### File-level changes

1. **`src/song/routing/prepare.rs`**
   - Add a private enum `PreparationLedger { Legacy, Issued }`. It is visible
     only in `prepare.rs` and `prepared.rs` (`pub(super)`).
   - Thread it into `prepare_routes_metered` as an extra parameter.
   - `Legacy`, used by `prepare_routes`, restores held0003 semantics exactly:
     - It calls `nested::prepare_nested_covers(inventory,
       &builder.source_covers, remaining)`, the legacy wrapper (see
       `nested/preparation.rs:3`).
     - It builds the plan topology with `inventory.clone()` instead of
       `copy_inventory`. The original text is in the intent receipt,
       `prepare.rs` original line 239.
     - It applies no new precharges.
   - `Issued` keeps `nested::prepare_nested_covers_metered` and
     `route_view::copy_inventory`, and adds the precharges listed in item 2.
   - This removes the dead-code warnings at `nested.rs:99` and
     `nested/preparation.rs:3`.
2. **Issued-only precharges**, in `prepare.rs` and `prepare/builder.rs`. Debit
   the counter before each operation, using checked arithmetic. A failure keeps
   the debit. Add a one-line comment documenting each bound:
   - track/template setup: `root.tracks.len() * (inventory.buses.len() + 1)`;
   - branch reservation loop: `branches.len() + 1`;
   - master lookup: `buses.len() + 1`;
   - `sidechain_routes` plus `source::validate_detectors`: tracks plus
     branches plus sidechain controls, as an upper bound on its scans;
   - chain-frame aggregation: tracks plus branches plus 1;
   - instrument graph compilation: before each `Template::from_inst` call in
     the instrument loop, charge that instrument's `nodes.len() + edges.len() +
     node_params.len() + params.len() + 1` (checked), as the bound on
     compilation traversal and diagnostics;
   - bus default cell scan: per bus, `chain.len()` plus the sum of each unit's
     `params.len()`, plus 1;
   - bus chain compilation: per branch, the selected bus `chain.len()` plus the
     sum of its units' `params.len()`, plus 1, charged before
     `BusTemplate::from_def`. `chain_frames` is a free function, so charge at
     the call site in the branch loop.

   These charges apply only to `PreparationLedger::Issued`. `Legacy` stays
   held0003-identical.

   The design's remaining precharge operations are
   already implemented at 37ea3e8 and must be kept, not duplicated: the
   topology copy (`copy_inventory`, `route_view.rs`), the publication Capture
   search (`publish_route_authority`), the site/policy binding scans
   (`route_view.rs` trusted copy and binding methods), and the transcript
   invocation find (`bind_issued_owner`, `lookup/authority.rs`). The worker
   must confirm each still charges before the operation and record that
   confirmation in the receipt.

   Imitate the existing charge-before-call style in `builder.rs:15`
   (`RouteBuilder::charge`) and `occupancy.rs:340` (`debit`).
3. **`src/song/snapshot/occupancy.rs`**
   - Add `pub(crate) fn canonical_song_requests(snapshot: &SongSnapshot,
     limits: SongLimits, remaining: &mut u32, depth: u32) ->
     Result<Vec<CanonicalIndexRequest>, Failure>`.
   - For every Part scope and track whose Capture/Edit payload has
     `index_timing()`, walk the recipe nodes from `root()`. Extend the prefix
     with each `edge.trace()`. Collect every node whose `operation()` is
     `FrozenUseOperation::Slice`, as `(issuer, prefix)`.
   - Mint each request with `SongSnapshot::canonical_index_request` (existing,
     `routing/index.rs:886`), using the window `[0, part duration)`.
   - Imitate the walk in
     `occupancy/geometry_tests/domains.rs:actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct`.
   - Charge one unit per visited node and per prefix term before visiting it.
4. **`src/song/snapshot/occupancy/route_view.rs`**
   - Add the pre-Reserve entry. It must be a descendant of the snapshot module
     to reach the private `PreparedSong.snapshot`.

     ```rust
     impl PreparedSong {
         pub(crate) fn issue_retained_route_authority(
             &mut self, limits: SongLimits, remaining: &mut u32, depth: u32,
         ) -> Result<Rc<RouteAuthorityView>, Failure>;
     }
     ```

   - The order is: `canonical_song_requests`, then `retain_index_occupancy`,
     then `issue_route_authority`, all on the same `remaining`.
   - A `CanonicalIndexReadiness::RequiresUniformBound` result is not a refusal.
     It is also not evidence of varying-seed support.
   - A song with no Slice sites gets an empty request list and still issues a
     view.
   - Add the `#[cfg(test)] capture_test_route_authority` helper with the
     signature already specified in this plan's progress log. It evaluates a
     real candidate, issues the view through this entry, queries an issued
     batch, and drops the `PreparedSong` before returning.
5. **`src/song/routing/prepared.rs`**
   - Keep the existing `prepare_routes_issued` and `PreparedRoutes` API.
   - Pass `PreparationLedger::Issued`.
   - Add a `#[cfg(test)] mod tests` block holding the bulk owning fixtures
     listed below.
   - The file must stay below 1000 lines. If the fixtures would push it over,
     stop and record a plan amendment. Do not create an undeclared file.
6. **`src/song/routing.rs`, `lookup.rs`, `lookup/authority.rs`,
   `prepare/builder.rs`**: edit only what items 1-5 require (re-exports,
   visibility). Delete any Route8 item that has no consumer in item 4 or in
   the waves 2-3 table below.

### Dead-code disposition after wave 1

The non-test library may still warn only for the items in this table. Each
item's planned consumer is listed. Any other warning fails this wave.

Session 250 replaced the earlier three-row summary with this table. The table
lists every `dead_code`/`unused_imports` diagnostic in
`tmp/song-mode-riela/session249-route8-clippy.log` (31 lib diagnostics at
`a91d94b`), plus the row for the session-250 selector (D32).

Match each diagnostic by file and item name, not by line number, because the
repair may shift lines. Every item stays in wave 1, and no wave-1 deletion is
needed. The "Caller written in" column names the wave that adds the caller
code. Wave-2 callers (`resolve_issued_event` and its children) are themselves
unreachable from non-test code until wave-3 playback calls them. So every row
stays allowed through the wave-2 join, and all rows become live in wave 3.

| ID | Diagnostic (file:line at a91d94b) | Item | Consumer chain | Caller written in |
|---|---|---|---|---|
| D01 | `src/song/routing.rs:623` unused imports | re-exports `PreparedPolicyRef`, `PreparedRoutes`, `prepare_routes_issued` | wave-2 resolver children (`routing/source/issued.rs`, `routing/nested/issued.rs`) and wave-3 `host/caps/song/preparation.rs` import them through `routing` | wave 2 / 3 |
| D02 | `src/song/snapshot/occupancy.rs:6` unused import | re-export `RouteAuthorityView` | the session-250 repair (item 3) changes the `prepared.rs` import to go through this re-export, so D02 should disappear in wave 1 | wave 1 (session 250) |
| D03 | `src/song/routing/prepared.rs:14` struct never constructed | `PreparedRoutes` | `prepare_routes_issued` <- wave-3 `SongHostPreparation::prepare` | wave 3 |
| D04 | `prepared.rs:20` struct never constructed | `PreparedSiteRef` | `PreparedRoutes::site` <- wave-2 `resolve_issued_event` | wave 2 |
| D05 | `prepared.rs:26` struct never constructed | `PreparedPolicyRef` | `PreparedRoutes::policy` <- wave-2 resolver | wave 2 |
| D06 | `prepared.rs:31` function never used | private `invalid` | `prepare_routes_issued` settings guard (`:44`) and `PreparedRoutes::site`/`policy` refusals (`:86`, `:89`, `:112`, `:122`) | wave 2 / 3 |
| D07 | `prepared.rs:34` function never used | `prepare_routes_issued` | wave-3 `SongHostPreparation::prepare` | wave 3 |
| D08 | `prepared.rs:66` methods never used | `PreparedRoutes::plan`, `site`, `policy` | wave-2 resolver (`site`, `policy`); wave-3 Ready `routes()` (`plan`) | wave 2 / 3 |
| D09 | `prepared.rs:137` methods never used | `PreparedSiteRef::authenticates_request`, `authority`, `scope`, `track`, `bind_original` | `bind_issued_owner` (`authority.rs:244` calls `authenticates_request`; it uses `authority`, `scope`, `track`, `bind_original`) <- wave-2 resolver | wave 2 |
| D10 | `prepared.rs:173` method never used | `PreparedPolicyRef::bind_original` | wave-2 issued companion in `configuration/index/canonical.rs` | wave 2 |
| D11 | `src/song/snapshot/occupancy.rs:27` function never used | `canonical_song_requests` | `issue_retained_route_authority` <- wave-3 preparation | wave 3 |
| D12 | `src/song/snapshot/occupancy/route_view.rs:12` variants never constructed | `PayloadSlot::Capture`, `PayloadSlot::Edit` | constructed during `publish_route_authority` and the trusted copy <- `issue_route_authority` <- wave 3 | wave 3 |
| D13 | `route_view.rs:18` fields never read | `ViewPolicy.source`, `ViewPolicy.seal` | trusted policy copy and `TrustedPolicyCopy::resolve`/`matches` <- `prepare_routes_issued` / wave-2 policy binding | wave 2 / 3 |
| D14 | `route_view.rs:22` fields never read | `ViewSite.slot`, `track`, `seal`, `policies` | `TrustedSiteCopy::matches`/`authenticates_record`/`resolve` and the trusted copy | wave 2 / 3 |
| D15 | `route_view.rs:29` field never read | `PublishedRetainedIndex.site` | `TrustedSiteCopy::authenticates_record` (`route_view.rs:258`, `Rc::ptr_eq(&record.site, &site.seal)`) <- `PreparedSiteRef::authenticates_request` <- `bind_issued_owner` | wave 2 |
| D16 | `route_view.rs:35` fields never read | `RouteAuthorityView.resource_count`, `pcm_bytes`, `sites` | accessors (D22) and the trusted copy <- `prepare_routes_issued` | wave 3 |
| D17 | `route_view.rs:40` struct never constructed | `TrustedRouteCopy` | `RouteAuthorityView::trusted_copy` <- `prepare_routes_issued` | wave 3 |
| D18 | `route_view.rs:46` methods never used | `TrustedRouteCopy::inventory`, `sites`, `policies` | `prepare_routes_issued` and `PreparedRoutes::site`/`policy` | wave 2 / 3 |
| D19 | `route_view.rs:56` struct never constructed | `TrustedSiteCopy` | trusted copy <- `prepare_routes_issued` | wave 3 |
| D20 | `route_view.rs:61` struct never constructed | `TrustedPolicyCopy` | trusted copy <- `prepare_routes_issued` | wave 3 |
| D21 | `route_view.rs:66` function never used | `slot_payload` | site/policy `bind_original` and `resolve` paths <- wave-2 resolver | wave 2 |
| D22 | `route_view.rs:189` methods never used | `RouteAuthorityView::resource_count`, `pcm_bytes`, `trusted_copy` | `prepare_routes_issued` host demand and plan copy | wave 3 |
| D23 | `route_view.rs:240` methods never used | `TrustedSiteCopy::authenticates_record`, `resolve` | `PreparedSiteRef::authenticates_request`/`bind_original` (D09) | wave 2 |
| D24 | `route_view.rs:293` function never used | `publish_route_authority` | `SongSnapshot::issue_route_authority` (D25) | wave 3 |
| D25 | `route_view.rs:415` method never used | `SongSnapshot::issue_route_authority` | `PreparedSong::issue_retained_route_authority` (D28) | wave 3 |
| D26 | `route_view.rs:426` method never used | `TrustedSiteCopy::matches` | `PreparedRoutes::site` <- wave-2 resolver | wave 2 |
| D27 | `route_view.rs:438` methods never used | `TrustedPolicyCopy::resolve`, `matches` | `PreparedRoutes::policy` and `PreparedPolicyRef::bind_original` <- wave-2 resolver | wave 2 |
| D28 | `route_view.rs:486` method never used | `PreparedSong::issue_retained_route_authority` | wave-3 `SongHostPreparation::prepare` | wave 3 |
| D29 | `src/song/snapshot/occupancy/lookup.rs:13` variant never constructed | `LookupAuthority::Issued` | `bind_issued_owner` <- wave-2 resolver | wave 2 |
| D30 | `src/song/snapshot/occupancy/lookup/authority.rs:194` function never used | `with_work` | `bind_issued_owner` and `resolve_issued_event` | wave 2 |
| D31 | `authority.rs:211` function never used | `bind_issued_owner` | wave-2 `resolve_issued_event` step 3 | wave 2 |
| D32 | `authority.rs` struct never constructed or fields never read (new in session 250) | `IssuedOwnerSelector` | wave-2 `resolve_issued_event` step 3 | wave 2 |

Gate rules:

- The receipt lists every lib and lib-test Clippy diagnostic as
  `{file, line, message, rowId}`.
- A diagnostic that maps to no row fails the gate. This includes
  `too_many_arguments` and any non-dead-code lint.
- To clear an unmapped diagnostic, do not delete, add or rewire any item.
  Stop and record a plan amendment instead.
- Rows may remain through the wave-2 join, where they are recorded in the
  resolution and shared-work receipts. Every row must be clear by wave 3, when
  strict Clippy must exit 0.

### Key pitfalls

- Do not route the legacy wrapper through the issued precharges or through
  `copy_inventory`. Legacy output and refusal must be identical for every
  existing test.
- Never compare allocation addresses or scalar fields to grant authority.
  Authority comes only from seal/token membership, which already exists.
- On failure, nothing is published: no partial view, no partial
  `PreparedRoutes`, and no retained records beyond those
  `retain_index_occupancy` itself commits transactionally. Debited work stays
  debited.
- `prepare_routes_issued` must reject settings that differ from the original
  Song. This check already exists (`prepared.rs:42`). Keep it ahead of any copy.
- Do not hold a `RefCell` borrow of `SharedIndexWork` across
  `transcript.authentic` or `transcript.invocation` (see `with_work`).

### Tests to add (in `prepared.rs` `mod tests`, using genuine candidates)

- A real Slice song goes through retention and is issued. The `PreparedSong` is
  then dropped, and copied site/policy lookup succeeds through
  `PreparedRoutes::site`/`policy`/`bind_original` with zero callback reads.
- The same song goes through `issue_retained_route_authority`. The resulting
  retained-record count equals the number of Slice sites found, and every
  record's window is `[0, duration)`.
- A song with no Slice sites gets a view with no records.
  `prepare_routes_issued` succeeds, and `plan().branches` equals the branches
  of legacy `prepare_routes`.
- Legacy `prepare_routes` against the issued plan, for the same snapshot, gives
  equal branches, tracks, `source_covers`, `nested_source_covers` and topology
  descriptors.
- Settings with a mutated tempo, tail or seed are refused before the copy is
  made.
- A foreign view's site or policy ref, a cloned-and-mutated `SongRoutePlan`, or
  two swapped equal-looking policies are all refused.
- Run view issuance, the trusted copy and `prepare_routes_issued` with exact
  work and with one less. Exact work succeeds with `remaining == 0`. One less
  fails, publishes no owner, and leaves the debit consumed. The fixture song
  must declare at least one instrument graph and one bus chain used through
  `instrument-fx`, so the new graph-compilation precharges are inside the
  measured exact amount.
- Depth at `max_depth - 1` is accepted where it should be. Depth at
  `max_depth` is refused.
- `bind_issued_owner` through the `with_work` bridge: a successful bind and a
  foreign-transcript failure both leave the collector debited by the actual
  spent amount. A prior debit is preserved.

### Edit protocol (all waves use the same protocol)

- Before the first edit, write `session249-route8-intent.json`. It contains:
  the accepted design sha256, this plan's sha256, each writePath's sha256, and
  the full original text of each writePath.
- Before every edit, re-read the target fresh and compare its sha256 with the
  last recorded value. If they differ, stop and reconcile from the current
  file. Never restore a stale whole-file copy.
- After the work is done, record post-edit hashes in the receipt.
- Never run `cargo fmt` on the whole crate, `git stash`, `git checkout` or
  `git reset`.
- Update only this plan's status, checkboxes and progress log.

### Verification (run in the foreground; record the exit status and full log path)

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-route8-build.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-route8-clippy.log 2>&1`
  is expected to exit non-zero in this wave. Every diagnostic must be
  `dead_code` or `unused_imports` on an item in the disposition table. Copy the
  exact list into the receipt.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/prepared::tests|occupancy::|routing::nested::preparation::meter_tests/)' > tmp/song-mode-riela/session249-route8-nextest-focused.log 2>&1`
  must exit 0 with a nonzero test count.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session249-route8-nextest-full.log 2>&1`
  must exit 0. At least the held0003 425 tests must pass, plus the new ones.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-route8-wasm.log 2>&1`
  must exit 0.
- `rustfmt --edition 2021 --check src/song/snapshot/occupancy.rs src/song/snapshot/occupancy/route_view.rs src/song/snapshot/occupancy/lookup.rs src/song/snapshot/occupancy/lookup/authority.rs src/song/routing.rs src/song/routing/prepare.rs src/song/routing/prepare/builder.rs src/song/routing/prepared.rs > tmp/song-mode-riela/session249-route8-fmt.log 2>&1`
  passes when the log has no `Diff in` line naming any of these eight paths.
- Cohort: `git ls-files -co --exclude-standard -z -- '*.rs' Cargo.toml Cargo.lock | xargs -0 shasum -a 256 > tmp/song-mode-riela/session249-route8-cohort.sha`.
  - It must contain exactly 947 lines.
  - Compared with `route-preparation-meter-full-cohort-held-0003.json`, it adds
    exactly route_view.rs, lookup/authority.rs and prepared.rs.
  - Hash changes may appear only on the five existing Route8 paths. A changed
    hash on any other path fails the gate and is listed under
    `unownedChanges`. Superseded for the rerun by the session-250 amendment
    below.
- `wc -l` on all eight paths: each must be below 1000.

### Done criteria (mechanically checkable)

- [x] Build, focused tests, full tests and WASM all exit 0.
- [ ] Clippy diagnostics are a subset of the disposition table.
- [x] The fmt log has no `Diff in` for any of the eight paths.
- [x] The cohort has 947 lines with exactly the three additions.
- [x] Every new test listed above exists and passes.
- [x] No `allow`/`expect` lint attributes were added. Check with
  `git diff -- <eight paths> | grep -E '^\+.*#\[(allow|expect)'`, which must
  print nothing.
- [x] Status stays In Progress, with a progress-log entry giving commands, exit
  codes and log paths. It becomes Completed only after wave 3 removes the last
  disposition-table warning (TASK-003 consumer criterion).

## Session 250 amendment: wave-1 Clippy repair and fresh evidence

This amendment resumes the blocked session-249 run on top of WIP commit
`a91d94b`. The session-249 gates passed except one: strict Clippy reported
`clippy::too_many_arguments` at `lookup/authority.rs:211`, because this plan
pinned an eight-parameter `bind_issued_owner` and forbade `allow`/`expect`.
That pin is now replaced by the selector signature in "Proposed issuing and
lookup signatures". The design rule is in
[Wave 1 item 4](../../design-docs/specs/design-song-mode.md#wave-1-completing-immutable-route-authority).
The Rust writePaths, tasks and the 947-input cohort target are unchanged.

### Repair scope

Before the first edit, write `session250-route8-intent.json` under the edit
protocol above. It records the design sha256, the plan sha256, and the sha256
and full original text of each of the eight writePaths as they are at
`a91d94b`.

1. **`src/song/snapshot/occupancy/lookup/authority.rs`**
   - Add `IssuedOwnerSelector<'a, 'p>` next to `bind_issued_owner`, with the
     four `pub(crate)` fields shown in the signature block. Derives are
     optional. The selector carries no seal and grants no authority:
     `PreparedSiteRef` can only be obtained from `PreparedRoutes::site`.
   - Change `bind_issued_owner` to the five-parameter selector form. The body
     reads `selector.site`, `selector.issuer`, `selector.prefix` and
     `selector.owner_window` where it previously read the four parameters.
     Nothing else in the body changes: no statement is reordered, and no
     check, charge or error is added, removed or changed.
2. **`src/song/routing/prepared.rs`**
   - Update only the two `bind_issued_owner` call sites in
     `retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits`
     to build a selector. Keep every assertion and the loop structure as they
     are.
3. **`src/song/routing/prepared.rs` import (D02).** `unused_imports` depends
   on whether any path names the re-export, not on reachability. So the
   `occupancy.rs:6` re-export stays unused forever unless something names it.
   Import `RouteAuthorityView` in the non-test part of `prepared.rs` as
   `crate::song::snapshot::occupancy::RouteAuthorityView`, instead of through
   `occupancy::route_view`. Keep the other `route_view` imports
   (`TrustedRouteCopy`, `TrustedSiteCopy`, `TrustedPolicyCopy`) unchanged.
   This is an import-path change only.
4. No other path changes unless the compiler requires a visibility or import
   adjustment inside the eight writePaths. Record any such adjustment in the
   receipt.
5. **D01 import-path rule for later waves.** The `src/song/routing.rs:623`
   re-exports (`prepare_routes_issued`, `PreparedPolicyRef`, `PreparedRoutes`)
   are consumed only when later code names them as `crate::song::routing::X`.
   Waves 2 and 3 must import them through that path, never through
   `routing::prepared`. SONG-ISSUED-RESOLUTION and SONG-ISSUED-PLAYBACK pin
   this rule.

### Equivalence obligations for adversarial review

The reviewer compares `a91d94b` with the repaired source and confirms each item
below:

- Retained-request selection still uses the attested site seal, plus the exact
  scope, track, issuer, prefix and window-coverage predicates, in the same
  order.
- `transcript.authentic` runs before `transcript.invocation`. The measured
  authentication delta is still precharged once before the invocation find.
- Exact retained-execution membership is checked, along with owner, placement,
  seed, entry, admitted depth, original recipe and complete coverage.
- The returned address uses the fresh issued invocation clock, never a
  retained clock.
- The `with_work` bridge holds no `RefCell` borrow across transcript calls. The
  debit on success and on failure is identical to `a91d94b`. The existing
  bridge fixture shows this: its assertions are unchanged and it still passes.
- No `allow` or `expect` attribute is added anywhere in the eight paths.

### Unowned paths precondition

`src/song/snapshot/resources.rs`, `src/song/snapshot/reservations_tests.rs`
and `src/sched/runtime/song/clock_tests.rs` must equal `HEAD` before the gates
run. Under the SM3 default
(`design-docs/user-qa/pending-song-mode-questions.md`), the orchestrator saves
the diff to `tmp/song-mode-riela/session250-unowned-rewrite.patch` and
restores exactly those three paths. Workers never do this. The receipt records
the exit status of `git diff --quiet -- <the three paths>`. The session-249
"pre-existing drift" exception is withdrawn.

### Fresh gates (foreground; record the exit status and full log path)

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session250-route8-build.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session250-route8-clippy.log 2>&1`.
  The log must contain no `too_many_arguments` diagnostic and no lint other
  than `dead_code`/`unused_imports` mapped to a row D01-D32. Under the accepted
  design, exit 0 is required from wave 3 onward, not in wave 1.
- `CARGO_TERM_QUIET=true cargo clippy --lib --profile test -- -D warnings > tmp/song-mode-riela/session250-route8-clippy-libtest.log 2>&1`.
  This prints the cfg(test) lib diagnostics on their own, because cargo
  deduplicates them in the all-targets log (session 249 printed only "3
  previous errors"). Every diagnostic must map to a row D01-D32. Record both
  logs in the receipt as `{file, line, message, rowId}` entries.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/prepared::tests|occupancy::|routing::nested::preparation::meter_tests/)' > tmp/song-mode-riela/session250-route8-nextest-focused.log 2>&1`
  must exit 0 with a nonzero count. Then append
  `cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker`
  (same env, `>>` to the same log), which must also exit 0.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session250-route8-nextest-full.log 2>&1`
  must exit 0, with at least the session-249 count of 2767 passing.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session250-route8-wasm.log 2>&1`
  must exit 0.
- `rustfmt --edition 2021 --check <the eight Rust writePaths> > tmp/song-mode-riela/session250-route8-fmt.log 2>&1`
  must report no `Diff in` naming any of the eight paths.
- `git ls-files -co --exclude-standard -z -- '*.rs' Cargo.toml Cargo.lock | xargs -0 shasum -a 256 > tmp/song-mode-riela/session250-route8-cohort.sha`.
  It must have exactly 947 lines. Compared with held0003, it adds exactly
  route_view.rs, lookup/authority.rs and prepared.rs. Changed hashes appear
  only on the five existing Route8 paths, and `unownedChanges` is empty.
- `wc -l` on the eight paths: each must be below 1000.
- `git diff a91d94b -- <eight paths> | grep -E '^\+.*#\[(allow|expect)'` and
  `git diff -- <eight paths> | grep -E '^\+.*#\[(allow|expect)'` must both
  print nothing.
- **Freshness.** `session250-route8-receipt.json` records the sha256 of every
  gate log, of the cohort file and of each of the eight paths. The sha256 of
  the receipt file is the evidence fingerprint. It must not equal
  `92d679ea9b4ca29f3f96a7594724ae8ca1a7f0ff2b2b73834930287638c5f61e`. The
  `lookup/authority.rs` and `prepared.rs` hashes must differ from the
  session-249 receipt values (`9e3b4601...` and `d530fe89...`).

### Session 250 done criteria

- [x] `bind_issued_owner` takes the selector form (five parameters), and the
  plan, design and SONG-ISSUED-RESOLUTION plan agree on it.
- [x] Every fresh gate met its wave-1 disposition: behavioral/build/WASM/format/cohort gates exit 0; Clippy logs contain only mapped rows (command exit 101 under `-D warnings`). The receipt records exit status and log path.
- [x] Every Clippy diagnostic in both clippy logs maps to a row D01-D32. No
  item was deleted or added except `IssuedOwnerSelector`.
- [x] The three unowned paths equal `HEAD`, and `unownedChanges` is empty.
- [ ] Test-integrity, adversarial and integration review accept Route8 before
  any wave-2 worker starts.
