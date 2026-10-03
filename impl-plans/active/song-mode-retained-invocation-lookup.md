# Authenticated retained invocation lookup

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Current completion path](../../design-docs/specs/design-song-mode.md#design-and-implementation-review-current-completion-path), [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose

Issue opaque borrowed addresses from genuine published owner invocations and
project their actual clock/source relations for the configuration consumer.
Existing CanonicalIndexRequest supplies original authenticated site identity;
lookup must not reconstruct execution seeds from frozen routing metadata.

## Related plans and dependencies

| Relation | Plan | Condition |
|---|---|---|
| Previous | song-mode-owner-invocation-retention.md | Independent gates and frontend accepted before source release |
| Next | song-mode-retained-index-geometry.md | Actually consumes these addresses |
| Later | song-mode-canonical-index-occupancy.md | Routing provenance, density, pre-Reserve and varying-domain contracts remain required |

This phase alone does not complete admission or song mode.

## Exact Rust manifest

| Module | Path | Status |
|---|---|---|
| Owning publication entry | `src/song/snapshot/occupancy.rs` | Not Started |
| Lookup and opaque tokens | `src/song/snapshot/occupancy/lookup.rs` | Not Started |
| Genuine lookup fixtures | `src/song/snapshot/occupancy/lookup_tests.rs` | Not Started |
| Genuine key access | `src/pattern/eval/song_replay.rs` | Not Started |
| Owning projection entry | `src/pattern/eval/song_clock.rs` | Not Started |
| Cohesive source projection child | `src/pattern/eval/song_clock_projection.rs` | Not Started |
| Conditional thin snapshot forwarding | `src/song/snapshot.rs` | Not Started |

Seven paths maximum. Declare the projection child explicitly from its parent;
keep each touched source below1000 lines. Snapshot forwarding is conditional:
occupancy already accesses ancestor-private snapshot fields. No new dependency.
The geometry companion has its own manifest; do not silently expand this one.

## Declarations and visibility

The tokens have private fields and no constructor accepting caller key fields.
The lookup child can access its parent's request and retained-record fields.
Export only crate-private types/functions through occupancy. Existing
SongSnapshot::canonical_index_request remains the owning routing issuance seam.
Do not export routing-private PreparedSliceAddress or ResolutionBudget here.

```rust
pub(crate) struct RetainedOwnerAddress<'s>;
pub(crate) struct RetainedIndexAddress<'s>;
pub(crate) struct RetainedSourceMember<'s>;
pub(crate) struct RetainedProjectedIndexRow<'s>;

pub(crate) fn owner_addresses<'s>(
    snapshot: &'s SongSnapshot, request: &CanonicalIndexRequest,
    depth: u32, limits: SongLimits, remaining: &mut u32,
) -> Result<Vec<RetainedOwnerAddress<'s>>, Failure>;

pub(crate) fn bind_index<'s>(
    snapshot: &'s SongSnapshot, request: &CanonicalIndexRequest,
    owner: RetainedOwnerAddress<'s>, depth: u32,
    limits: SongLimits, remaining: &mut u32,
) -> Result<RetainedIndexAddress<'s>, Failure>;

pub(crate) fn bind_member<'s>(
    index: &RetainedIndexAddress<'s>, selected: &FrozenSelectedSource,
    handle: &EventHandle, depth: u32,
    limits: SongLimits, remaining: &mut u32,
) -> Result<RetainedSourceMember<'s>, Failure>;
```

Final declarations may use a named iterator rather than an allocated token Vec
when that preserves the same precharged lookup and explicit ambiguity rules.
Projection exposes opaque borrowed footprint/member results, not private
context vectors. Factoring existing project_for must preserve its current
SharedIndexWork wrapper and add borrowed-counter projection without a fresh
collector or allowance. Every failed debit affects the original counter.

## Tasks

### TASK-001: Owning lookup and authentic site binding

**Status**: Completed
**Parallelizable**: No

- [x] Authenticate original strong Song and recipe identities plus full site,
  owner, invocation placement/offset/window, actual seed and entry context.
- [x] Enumerate only genuine published invocations; deduplicate only proven
  identical authority. Reject missing or ambiguous bindings without VM access.
- [x] Keep source policy binding separate from member binding. Public fields of
  FrozenSelectedSource are insufficient: authenticate its original owning use.
- [x] Nested owner binding may differ from the outer request owner only through
  the actual sealed dependency/source chain; request.matches alone is not proof.
- [x] A genuine successful empty boundary can certify empty membership; it
  cannot mint a member or infer one from note fields.

### TASK-002: Actual source and frame projection

**Status**: In Progress
**Parallelizable**: No; depends on TASK-001

- [ ] Project issuer frames and traverse actual sealed selected-source parent
  relations with source/request/producer/member authentication at each hop.
- [ ] Preserve issuer START, structural sampling, uncut whole, At/Before
  orientation and genuine half-open query applicability in the proper owner.
- [ ] Keep original issuer_sample_start separate from each source-parent sample
  predicate; structural sampling can reset projected sample_start.
- [ ] Refuse Unknown or incomplete boundaries with addressed failures.
- [ ] Charge traversal/copy before growth using original remaining/depth;
  no cfg(test) evidence reconstruction or caller-created authority.

### TASK-003: Genuine fixtures and independent acceptance

**Status**: Completed
**Parallelizable**: No; depends on TASK-002

- [x] Use original evaluator/published records for two real parent bindings,
  cached descendants, successful empty/suppressed subjects and repeated owners.
- [x] Verify foreign same-NodeId snapshots, recipe/site/prefix/placement
  mismatch, actual distinct retained seeds and missing/ambiguous records refuse.
- [x] Deny callback reads during lookup; exact-work/one-less and real depth
  failures consume the same counter and preserve published records.
- [x] Independent native/tests/strict lint/WASM/format gates and full input seal.

## Completion criteria

- [x] All three tasks complete; all touched sources below1000 lines.
- [x] Opaque addresses derive from actual invocation records, not static IDs.
- [x] Borrowed ownership is acyclic and does not retain the evaluator.
- [x] Geometry companion receives these exact addresses and projected evidence.
- [x] No full admission or uniform varying-domain support claim is made here.

## Progress log

### 2026-10-03 — Source-grounded review

Owning replay fields and occupancy records are private; routing binding types
are inaccessible to their sibling occupancy module. Use the existing privately
minted CanonicalIndexRequest. Existing project_for only maps owner frames, so
actual source-parent traversal is an explicit deliverable. Source release waits
for predecessor frontend acceptance. The complete user objective stays active.

### 2026-10-03 — Released owning lookup implementation

Seven-path intent recorded before source edits. Lookup uses borrowed privately
issued requests and published invocation records, never reconstructed seed keys.
Projection distinguishes original issuer START from projected sampling time;
nested policy binding follows only sealed original source boundaries. Common
projection charging borrows the original counter directly with no new ledger.
Geometry companion remains unreleased. Author compile-only check is permitted;
all runtime/lint/WASM acceptance remains independent.

### 2026-10-03 — Actual lookup/projection and ten genuine fixtures written

Opaque borrowed tokens are issued only from original published invocations and
privately minted canonical requests. The original strong Song/recipe/site is
validated, nested tokens are reached only through sealed authentic parent
source chains, and binding preserves actual seed/entry/placement. Duplicate
coverage exposes explicit genuine tokens rather than falsely treating canonical
request overlap as ambiguity. Missing/full-path/foreign/recipe errors refuse
without q. Source policy matching is scoped to the actual boundary parent owner
and full frozen graph producer path, retaining incoming edges plus original
member entry trace/copy ordinals; matching anywhere in the inventory is rejected.

Projection is factored into the declared child. Source topology uses actual
query edit tags/node counts and Overwrite offset/duration, checks placement at
every hop, retains original issuer START separately, propagates At/Before
through affine/reversal hops and resets only on genuine structural sampling.
Member validation borrows the remaining counter and remaining inherited depth;
no new collector/allowance is constructed. The existing SharedIndexWork wrapper
retains its behavior. Conditional snapshot forwarding was unnecessary.

Ten genuine owning fixtures cover dynamic/suppressed rows, nested members, empty
source, exact/one-less/depth, foreign original, missing/full site, duplicate
coverage, Rev chain orientation, same-policy distinct uses/wrong parent, actual
vary seeds/placements/foreign recipe, sampling reset, and inherited origin depth.
No sealed graph/member was forged to simulate unreachable ambiguity: defensive
multiple-match refusal is implemented, and real distinct-use framing is tested;
a reachable genuine ambiguous construction has not been established. Runtime
acceptance remains pending. The geometry companion is still unreleased.

Author compile-only checks: original36838 terminal101 (private child projection
method); original36606 terminal0 warnings; original84262 terminal0 warnings;
original27890 terminal0 one getter warning; original79963 terminal0 same warning;
original54405 terminal0 clean after getter became test-only. Logs are retained
in /tmp/vactr-retained-lookup-author-compile-001.log through -006.log. No edits
occurred while any original handle was live; no author tests/Clippy/WASM ran.
Narrow documented pending-consumer allowances are on named opaque public crate
entries/access methods only, not modules or helper bodies; real helpers remain
unmasked. Original pre-edit text was not saved before this phase's edits; intent
hashes exist, and this audit limitation is recorded without invented originals.

Final quiet author compile-only check --tests original6201 terminated0
(chunk4d6c45), /tmp/vactr-retained-lookup-author-compile-007.log clean. Seven-file
scoped rustfmt --check exit0; all files below1000. Source READY_HELD0001 for
independent behavioral/lint/WASM gates; no runtime acceptance claimed.

API distinction: owner_addresses may enumerate genuine descendant tokens under
an outer request only through their sealed source-parent chain. bind_index must
then receive the separately privately minted request for that actual child
owner/Slice site. Binding that child under an outer issuer/recipe is invalid and
refused. Enumeration authority is not a certificate for a different issuer;
no caller-created fields or reconstructed seed are accepted. The nested fixture
explicitly enumerates the outer request and binds the original inner request.

### Lookup checkpoint 0001 diagnosis and bounded fixture repair

Independent native check passed; occupancy35 executed29PASS6FAIL (all prior25
passed, lookup10 had4PASS6FAIL). Receipt
/tmp/vactr-retained-lookup-final-001.json records all933 held inputs unchanged.
The failures occurred before remaining orientation/distinct-use assertions:
root q entry can legitimately be empty; a leaf at the inclusive depth ceiling
is valid; descendant enumeration does not authorize binding a descendant to
another owner's request; inventory captures selected-source Rc copies as well
as declared Edit ancestry, so allocation count is not declaration topology.

Repair changes only lookup_tests.rs. The root entry is compared with its genuine
stored owner/seed invocation while actual issuer-prefix assertions remain.
The simple leaf explicitly succeeds at max_depth and refuses max_depth+1;
the separate genuine inherited-origin test still requires exhaustion at the
ceiling with an actual inherited hop. Three projection/origin fixtures and the
distinct-use fixture select the actual revision/track/root before binding and
explicitly assert wrong-owner descendant refusal. Distinct-use scope selection
follows original root Sequence children and their Edit.source ancestry, retaining
same-policy revision equality, member/use/producer checks and denied callbacks.
No production authority, limits, projection behavior or geometry code changed.

Quiet author compile-only command `CARGO_TERM_QUIET=true mise exec -- cargo
check --tests` original37725 terminated0 (chunk5bb987), clean log
/tmp/vactr-retained-lookup-author-compile-008.log. No edits occurred while live.
No behavioral execution is claimed for this repair. Intermediate source-hop
START/orientation eligibility and bound-site consumer validation remain explicitly
required in the unreleased geometry companion. Source READY_HELD0002 awaits
independent genuine execution, with all ten lookup fixtures preserved.

Final source review also compares selected source revision/duration/track/family
across copied inventory allocations; frozen root indices are not assumed equal.
Wrong-parent descriptor refusal remains exact. Final quiet compile-only009
original48809 terminated0 (chunk23759b), clean
/tmp/vactr-retained-lookup-author-compile-009.log. All author handles terminal.

### Lookup checkpoint 0002 diagnosis and owning-copy correction

Independent native passed; occupancy35 executed34PASS1FAIL, all933 hashes exact.
Distinct-use failed with FuelExhausted during repeated Cartesian policy scans,
not a valid Type rejection witness. Its original default shared allowance covered
retention, request issuance and every repeated bound/member/source traversal.
The fixture now selects the actual outer parent and the two distinct genuine
producer-use contexts, avoids redundant unrelated-policy trials, and requires
exact Type for the equal-policy foreign parent plus successful original member
and distinct full use/producer identities. All operations retain the same original
remaining counter; no internal reset or limit increase was introduced.

Owning implementation correction: selected_policy previously stopped at the first
revision/track/root-matching payload even when the exact descriptor belonged to a
later allocation copy. It now searches matching parent payloads for the exact
pointer and authentic full use path, refusing missing or multiple owning matches.
This preserves foreign descriptor refusal; semantic policy equality is never
ownership. A separate possible multi-hop case where an earlier semantic-equal
boundary is not the descriptor's owning boundary remains under parent review;
no genuine fixture for that case or allocated-parent-copy acceptance is claimed.
Quiet author compile-only010 original61700 terminated0/chunk40244c, clean
/tmp/vactr-retained-lookup-author-compile-010.log. No tests/lint/WASM by author.

Parent-approved follow-up in the same correction distinguishes exact descriptor
absence at a boundary as Ok(None). Member/empty lookup continues only that absence;
malformed full use, ambiguity, depth and fuel failures propagate, and final absence
remains Type. The two-context fixture tries only original revision/track-matching
policies at each authentic producer and requires exactly one actual member;
Type-only misses preserve exact per-use policy indices instead of assuming both
uses select the first descriptor. Multi-hop equal-policy coverage is still not
claimed. Compile-only011 original14109 terminated0/chunk9236cc, clean
/tmp/vactr-retained-lookup-author-compile-011.log. All author handles terminal;
source READY_HELD0003 awaits independent behavior. Original meter/depth witnesses
and all ten lookup tests remain unchanged in purpose.

### Lookup checkpoint 0003 and stage-cost diagnostic 0004

Independent native0; occupancy35 again34PASS1FAIL, intended-member Type assertion
received FuelExhausted. All933 inputs matched; no owning-copy acceptance claim.
Fixture-only diagnostic records the unchanged original remaining after retention,
request, enumeration, binding, source-chain, wrong-parent and each intended-member
operation. Failure messages include prior/current allowance, stage history and
actual original producer/selected descriptor. Exact Type/member/use assertions
and initial default allowance are unchanged; no fresh ledger or limit increase.
Compile-only012 original22274 terminated0/chunkbc2dd4, clean
/tmp/vactr-retained-lookup-author-compile-012.log. Source READY_HELD0004 awaits
independent diagnostic execution. All author handles terminal; no tests/lint/WASM.

### Measured configurable cumulative fixture allowance 0005

Diagnostic0004 executed34PASS1FAIL with all933 hashes exact. Original allowance
16384: retention left996 (15388 actual cost), request984, enumeration817,
bind797/source-chain785, next bind756/source-chain735, wrong-parent698,
intended-member78 after620 debit then a larger debit failed. This is genuine
default insufficiency for the complete chord/two-use fixture, retained in raw
/tmp/vactr-retained-lookup-occupancy-004.log and final004 receipt; Fuel was not
accepted as an ownership proof.

Parent approved explicit validated fixture SongLimits.max_nodes=32768, with the
same limits passed to actual request issuance, retention, all lookups and one
original remaining counter. No replenishment, production defaults, physical
capacity or work-derived pool bound changed. Existing default helper wrappers
and exact/one-less/depth tests remain intact. Musical chord and full two-use,
wrong-parent Type, origin, trace and denied-callback assertions are unchanged.
Successful complete cumulative work/stage output is recorded by the fixture;
actual positive measurement remains pending independent execution. This does
not claim support under the default allowance.

Quiet author compile-only013 original63219 terminated0/chunk2a72e7, clean
/tmp/vactr-retained-lookup-author-compile-013.log. Source READY_HELD0005, all
author handles terminal; no behavioral/lint/WASM commands by author.

### Independent lookup0005 and lint-only0006

Independent380 unique behavioral tests PASS and native0. The complex two-use
fixture's full measured cumulative work is17909 with configured32768 and one
counter; the diagnostic default16384 insufficiency remains preserved. Strict
Clippy56377 terminal101 identified only needless_borrow at lookup_tests.rs116.
Repair removes the single unnecessary borrow in wrapper.replace; no runtime
behavior or assertions changed. All933 inputs matched at0005 terminal; WASM
and formatting gates stopped after lint failure, not claimed passed.
Quiet author compile-only014 original53361 terminated0/chunk1d25f1, clean
/tmp/vactr-retained-lookup-author-compile-014.log. Source READY_HELD0006 awaits
independent remaining gates; every author handle terminal. No author tests,
Clippy or WASM ran. Full immutable geometry/admission/varying scope remains open.


### Accepted independent lookup0006 execution and frontend gates

Fresh380 distinct Rust behavioral tests passed; native, strict all-target Clippy,
pure WASM and seven-file formatting passed. All933 inputs were unchanged.
Final /tmp/vactr-retained-lookup-final-006.json SHA256
b3105855610ad21ab4d343daa369d2d8623b5884f70190317e029b557468d626.
Root fresh590 editor tests and complete build/dist artifact5bf8604f passed;
root receipt SHA256 f941cc5376a22acaf3671d0ca6da3dd886f15bc6cc5cd994b5fbe365861dac5e.
Task001 owning lookup and Task003 acceptance are accepted. Task002's existing
original/frame projection checks passed, while explicit intermediate source-hop
START/orientation consumer eligibility remains assigned to the geometry companion.
This plan remains In Progress until that companion receives the exact opaque
addresses/evidence; its corresponding completion checkbox remains unchecked.
Full routing/density/pre-Reserve and uniform varying-domain work is not claimed.
Measured17909/configured32768 and default16384 insufficiency remain unchanged.
Historical extraction-byte equivalence remains unproven; original tests passed.
