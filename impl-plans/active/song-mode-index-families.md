# Connected Index families and affine issuer clocks

**Status**: In Progress
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and dependencies

Extend authenticated Index configuration beyond the accepted direct single-slot
case. Multiple occupied leaves and symbolic copies must contribute their actual
whole footprints to the same source configuration. Positive outer clock maps
must resolve in the issuing clock and return endpoints in the root clock.

Depends on [occupancy](song-mode-index-occupancy.md),
[genuine runtime fixtures](song-mode-index-runtime-fixtures.md) and completed
[matcher reservation](../completed/song-mode-slice-matcher-budget.md).
ROOT0494 accepted the direct checkpoint: native/Clippy and20 actual tests with
fixed903 inputs. This dependency is scoped evidence, not full song completion.

Dynamic callable-result authority, heterogeneous family gap solving, multiple
inherited Index frames, nonlinear clocks, host ownership, transport, Apply/mute,
playback/export remain requirements of the full goal. This wave does not remove
or redefine them. Unsupported combinations retain addressed original authority.

## Exact Rust manifest

| Path | Deliverable |
|---|---|
| `src/song/routing/configuration/index.rs` | Authenticate families and dispatch common-clock connected solver |
| `src/song/routing/configuration/index/cluster.rs` (new) | Charged compressed family arithmetic and exact periodic runs |
| `src/song/routing/nested.rs` | Integrate positive affine issuer traversal with actual retained stage authority |
| `src/song/routing/nested/clock.rs` (new) | Cohesive charged clock helper to keep parent below1000 lines |
| `src/session/song/inventory/index_occupancy.rs` | Genuine multiple-slot/copy/outer-clock preparation/query/resolve witnesses |

Five modules. Every touched Rust file stays below1000 lines. Additional paths
require root amendment before writes. Existing routing/source.rs remains983
lines and is read-only in this wave. No dependency/Git/unrelated editor edits.

## Semantic contracts

- Bind original prepared root, complete producer path and genuine issued timing.
  Match the issued leaf/copy identity against actual recipe structure; accepting
  a numerically equal whole from another producer is insufficient authority.
- Index leaf values and leaf producers do not themselves change source policy
  configuration. Collect only footprints sampling the same authenticated source
  configuration at their whole START. Subject mode preserves its own clock.
- Preserve complete wholes before clipping to the actual output owner. Merge
  overlap and touching boundaries; never bridge a genuine gap by a convex hull.
- Retain weighted slot prefix/width, zero slots and symbolic copy ranges. Never
  allocate or loop once per score cycle, repeat count, period numerator or copy.
- Charge original cumulative work/depth before every scan, comparison, arithmetic
  loop or allocation. No VM callback, RNG replay, default quota or refund.
- Existing density bounds must still cover actual instantaneous ownership,
  uniform openings and finite births. Preparation fixtures must exercise the
  actual bound/admission path, not only private component arithmetic.

### Common-clock family proof

For equal positive width w and Slow s=a/b, an occupied prefix p gives authentic
whole beginning (1-w)c+p+ws*k and length ws, with child-query overlap deciding
which k actually belongs to integer parent c. A repeated slot represents prefix
p+jw for a bounded symbolic j; its range is not expanded.

Where inspected prefix spacing proves each parent cluster connected, let
A=1-w, L=ws and R=max_prefix-min_prefix. Its full cluster endpoints are
A*c+min_prefix+L*floor(c/s) and
A*c+max_prefix+L*ceil((c+1)/s).

Consecutive cluster gaps are A-L-R away from integral child boundaries and A-R
at period seams. L+R>=A proves within-period coverage. R>=A additionally proves
continuous seams. Otherwise maximal complete period runs have endpoints
m*a+min_prefix and (m+1)*a-A+max_prefix. These formulas require checked replay
against actual issued authority and exact prefix spacing; extremes alone do not
prove that intermediate coverage exists.

Owner eligibility is prefix-specific: intersect actual emitting parent ranges
before choosing first/last whole. Partial owner heads/tails cannot reuse a full
cluster formula blindly. Likewise source START eligibility can remove a family;
prove all contributing starts eligible or solve the actual filtered constraint.
An unproved filtered union retains an addressed requirement, never a hull.

### Affine projection

Compose positive Rate/Shift root-to-issuer maps from original trace operations.
Map output owner constraints into the issuer clock. Derive the selected-source
to-issuer relation independently (identity for direct Slice Child0); keep direct
source scope in its original clock. Solve there and project endpoints back exactly. The immutable
timing whole stays in its issuing clock. Output clipped parts or a guessed birth
floor cannot substitute for this map. Preserve all intermediate owner constraints
before choosing a component. Reflection/nonlinear and multiple inherited Index
constraints remain explicit further work if not solved by this wave.

## Tasks

### TASK-001: Source-grounded family and clock declaration
**Status**: Completed
**Parallelizable**: No

- [x] Confirm copy normalization and leaf/copy producer matching against runtime.
- [x] Declare private family/clock interfaces grounded in existing bound types.
- [x] Extract helpers before any touched parent reaches1000 lines.

### TASK-002: Connected common-clock families
**Status**: In Progress
**Parallelizable**: No (depends on TASK-001)

- [x] Multiple occupied leaves and symbolic copy ranges participate in one union.
- [ ] Prove cluster spacing, periodic seam gaps and exact source/owner eligibility.
- [x] Preserve existing direct single-slot and genuine Subject behavior.
- [x] Maintain original cumulative quota and checked rational arithmetic.

### TASK-003: Positive affine issuer traversal
**Status**: In Progress
**Parallelizable**: No (depends on TASK-002)

- [x] Retain authentic outer producer/owner constraints and issued Slice clock.
- [x] Resolve positive outer Rate/Shift and return root-clock components.
- [x] Unsolved combinations keep original addressed authority and diagnostics.

### TASK-004: Genuine fixtures and independent verification
**Status**: In Progress
**Parallelizable**: No (depends on TASK-003)

- [x] Extend existing internal false-structured-source Freeze/capture harness.
- [x] Multiple leaves with Index [{slow 0 2} nil {slow 1 2} nil] exercise
  expected components [0,7/4) and [2,15/4), after actual runtime confirmation.
- [x] Symbolic copies with [{repeat {slow 0 2} 2} nil] exercise expected
  components [0,5/3) and [2,11/3), after actual runtime confirmation.
- [x] Full/fractional/reordered windows retain genuine origins and issued timing;
  compare full resolved identity inside components and distinct gaps across them.
- [x] Outer Fast/Slow, placement offsets, partial owner heads/tails and cumulative
  one-less/depth/large symbolic-count witnesses use the actual preparation path.
- [ ] Held fixed source precedes mandatory checker native/browser/Clippy and exact
  nonempty list/run suites. Full58 wave remains a later coherent acceptance gate.

## Completion criteria

- [x] Multiple same-clock leaves/copies resolve true maximal connected wholes.
- [x] Positive affine outer clocks preserve actual identity and configuration.
- [x] Genuine preparation/query/resolve witnesses verify consumer and bounds.
- [x] Symbolic work stays independent of arrangement duration and copy count.
- [ ] Independent checks pass; source/plan limits and remaining full-goal scope
  are reconciled honestly.

## Progress log

### Root release preparation — 2026-10-02

Read-only specialized audit identified current second-slot/copies rejection and
root-only inherited dispatch. Root corrected initial public fixture suggestions:
transform_instrument creates structured Subject mode, so genuine Index witnesses
must extend internal actual false-structured source capture, not relabel public
Subject tests. Common-clock cluster proof is an implementation increment;
heterogeneous neighbor stepping is not a duration-independent joint gap solver.
No source changes are authorized until a separate hashed root release.

### Concrete family/clock interfaces before implementation

```rust
struct IndexFamily { prefix: Ratio64, width: Ratio64, copies: u32, slow: Ratio64 }
fn family_component(families: &[IndexFamily], emitted_cycle: i64,
    issued: TimeSpan, source: TimeSpan, owner: TimeSpan,
    budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>;
struct IssuerClock { factor: Ratio64, shift: Ratio64, owner: TimeSpan }
fn issuer_clock(stage: &Stage<'_>, cursor: usize, owner: TimeSpan,
    offset: Ratio64, budget: &mut ResolutionBudget)
    -> Result<Option<IssuerClock>, Failure>;
```

Steps capture normalizes width per copy, keeps copies symbolic, and uses actual
NestedStep/GeneratedBranch/Child trace terms. Runtime removes a Repeat step
wrapper before squeeze. Family replay must therefore unwrap that descriptor once
and match its complete producer chain, including the actual copy ordinal; its
count must not multiply the clock again. Clustering proves spacing between every
compressed occupied range, not only extremes. Prefix-specific owner intersection
uses integer copy ranks at the two boundary parent cycles before endpoint
selection, with explicit boundary connectivity checks. Selected source eligibility
is checked at whole START in the direct subject/Slice clock. Positive outer maps
project common-root output owner into issuer clock and component endpoints back;
the independently selected source scope is not blindly reinterpreted as root time.
All scans/storage/arithmetic use the original cumulative work/depth budget.

### Boundary query refinement

Parent part intersection alone is insufficient at partial owners. For each
eligible prefix/copy, intersect [c+p,c+p+w) with issuing owner [U,V), then map
that positive piece through squeeze to child [c+(u-c-p)/w,c+(v-c-p)/w).
Eligible child whole ranks are floor(inner.begin/s) through ceil(inner.end/s)-1.
Only boundary copies can be partial; a compressed run has first/last boundary
copies and a full interior range, so no per-copy expansion is necessary. Merge
only actually intersecting/touching resulting whole ranges. The adversarial
w1/4, s2, prefixes0 and1/2, owner[2/5,3/5) must begin at1/2, not2/5.
Whole START0 can belong to source[0,1/4) while whole[0,1/2) extends beyond it;
source bounds test sample membership, never crop that whole.

### Canonical capture and fixture refinement before source

IssuerClock additionally retains `birth_owner: TimeSpan`, mapping the original
SourceUseCover capture window through the same positive affine issuer prefix.
This differs from the narrower configuration owner. Song canonical query first
queries whole cycles and rejects whole anchors outside capture duration before
clipping parts. Solve START membership against the intersection of direct source
configuration and birth_owner; retain actual source identity and never crop whole
to that membership interval. Fractional configuration owner still filters actual
prefix child queries independently. Complete parent copy footprints have minimum
length `w*s*ceil(1/s)`; since s*ceil(1/s)>=1, adjacent complete copies connect
even for fractional Slow. Partial boundary copies still require actual merge.

Fixture helpers: `prepared_index(index: Rc<Pat>, rate: Ratio64) -> PreparedSong`
uses the same actual evaluator/assets/shared Freeze/capture; `slow_index(value:
i32, slow: Ratio64) -> Rc<Pat>` and `family_rows(index: Rc<Pat>, rate: Ratio64,
expected: &[TimeSpan])` preserve genuine origins, local issued clock and full
resolved-route identity across canonical/reordered nonempty queries. Private
cluster tests exercise partial owner/source membership and compressed large
counts with exact original-counter/one-less refusal. No callback or fabricated
origin is introduced.

### Rational period and placement declarations

For Slow numerator one a periodic group contains one parent; within-group
parent connectivity is vacuous. Retain actual copy spacing/boundary merge and
seam gap proof, but do not impose the multi-parent L+R>=1-w test.
`prepared_at(index: Rc<Pat>, rate: Ratio64, offset: Ratio64) -> PreparedSong`
wraps the actual capture in an actual empty-prefix Part sequence; the existing
prepared_index wrapper supplies zero offset. A finite source eligibility test
proves START membership without clipping whole to source end. These tests are
consumer witnesses or explicitly private arithmetic, never fabricated issuance.

### Coherent family/affine source checkpoint — pending mandatory checker

Five Rust paths are SOURCE_READY_HELD. Scoped rustfmt and check exit0; no author
Cargo or nextest. The temporary nested module-registration ordering error was
corrected before format succeeded; no Rust compilation or behavior execution
has occurred. No task/behavior acceptance is claimed.

Written production paths match original complete slot/copy traces, merge actual
compressed parent footprints with prefix-specific part eligibility, and project
positive affine issuer clocks. Canonical capture whole-anchor membership and
fractional configuration owner remain separate. Single-parent trimmed groups
need no multi-parent adjacency premise. Source bounds constrain START and never
crop whole ends. Actual source identity remains the original selected scope.

Eight genuine inventory tests (two retained plus six new) exercise multiple
leaves, Repeat copies, rational Slow copies, outer Fast/Slow, real sequence
placement, exact local issued timing, full resolved-route equality, genuine
period gaps and nonvacuous partial queries. Actual prepared branches expose
configuration/occurrence/live/reserved bounds; assertions require the finite
bound cover realized components and occurrence multiplication exactly once for
the single placement. Three new private cluster tests are arithmetic evidence
targets: prefix-specific partial owner, START-vs-whole-end, and a million symbolic
copies with measured exact/one-less constant work. They are not route quota or
actual issued-source evidence by themselves. Existing replay tests are retained.

Pending: mandatory compiler/Clippy/runtime results; genuine route-level shared
quota/depth and actual fractional owner/canonical onset witnesses; filtered
multi-family source eligibility beyond the proven complete-start range. Such
unproved unions retain the original addressed requirement. Positive Shift
implementation is written but genuine Shift fixture still pending. The existing
weighted-Slow partial-query fixtures and new affine witnesses must pass before
any related checkbox is completed. Heterogeneous/nonlinear/multiple inherited
frames, callable-result authority and retained canonical dynamic realization,
full matrix, host/transport/playback/export remain required.

### Corrected silent-zero source audit — ROOT0500

ROOT0499's inferred zero-Hold concern was superseded by ROOT0500 after actual
source inspection: timing capture filters Hold weights strictly positive, and
runtime step layout rejects weight<=0 with Type. Hold0 therefore cannot be a
silent successful fixture; no runtime/capture semantics or family-width bypass
was added. The genuine sanctioned zero is Repeat0, whose actual copied recipe
retains copiesSome0 and whose runtime emits no body row. A ninth inventory test
places Repeat0 before the two occupied Slow leaves, shifting their actual trace
ordinals while preserving normalized weights, strict eight total rows, connected
components [0,7/4),[2,15/4), full authentic timing/route equality and nonvacuous
partial queries through the existing genuine helper. Behavior remains unrun
until mandatory checker; all earlier assertions and production bytes unchanged.

### Family001 actual compile failure — ROOT0502

Original checker31742 terminal101, chunk320817: native and host-wasm pass;
Clippy lib-test compilation reports E0308 at fixture147 because Value::Int stores
i32 while slow_index declared i64. No behavior tests ran. The fixture parameter
now declares i32 directly, with all literal callers unchanged and no truncation,
cast or production mutation. Inspected the remaining new Value::Int fixture
construction: rate parameters use Value::Ratio and counts use inferable Int
literals. Scoped format/check pass; mandatory retry remains required.

### Focused family002 acceptance and remaining genuine witness declarations

ROOT0504 independently verifies original78641 terminal0/chunk7c0f4c,
15 gates/native/wasm/strict Clippy,31 unique tests,906 unchanged inputs. Raw
final-results /tmp/vactr-index-families-checkpoint-002/final-results.json
SHA4a72cf38aa3d54793a550c1ed62db389f775d3a34dc0f5089c31d8042071018d.
This proves only the focused source/fixtures, not all wider geometry/song scope.

Additional private fixture setup before source:
`prepared_context(index: Rc<Pat>, rate: Ratio64, offset: Ratio64,
shift: Option<Ratio64>, duration: Ratio64) -> PreparedSong` extends actual
prepared_at. Shift uses actual evaluator-defined identity function and Off,
once-only discover_shapes, original namespace slots/Song/retained records in
one Freeze, measured successful work/debit after drop, actual admit_closed and
capture_routing_prepared. No synthetic prepared output/cache key. Fractional
canonical duration5/4 with multiple_index must produce three rows; a second
weighted Slow single-slot case retains whole[1/2,3/2) across clipped part[1,5/4),
proving original whole is not cropped at capture end. Both resolve actual routes.
Shift+1/4 keeps local issued timing and exact root translation, and distinguishes
authentic direct versus shifted issuer/use identities. Binary search smallest
positive valid resolve_route max_nodes and max_depth under unchanged defaults;
every success equals default full route, unexpected failure codes fail the test.
One-less boundaries must be genuine FuelExhausted/DepthExceeded.

### Genuine remaining witness source checkpoint

Three further actual candidate/query/prepare/resolve tests are written: genuine
retained Off identity callback with positive Shift and partial issuer queries;
fractional canonical capture5/4 plus uncropped whole[1/2,3/2); smallest successful
full-route work/depth with one-less exact failure. Shared Freeze namespace,
original Song and retained outputs use the sanctioned discover/freeze/drop/debit/
admit seam; actual prepared pattern count1 and no rejected callback are asserted.
No production Rust changed from focused31 acceptance. All previous assertions
remain, with helper defaults preserving previous cases. The inventory suite now
has12 genuine tests. Scoped formatting/check exit0, no author Cargo/nextest.
The three new tests await mandatory execution; quota/depth/Shift/boundary
criteria are not marked passed until actual raw evidence. Wider filtered source
union/heterogeneous/nonlinear/inherited/dynamic/callable and full-host/playback
criteria remain unchanged and incomplete.

### Family003 actual remaining fixture failure — ROOT0506

Original73987 terminal101/chunk4e258e. Native/wasm/strict Clippy and32 actual
unique tests passed, including the genuine full resolve_route smallest work/depth
witness. Raw session-tests.log reports Shift rejected prepared pattern count0
versus required1; fractional capture has the correct exact three onset values
but producer traversal order [0,3/4,1/2] differs from the unstated sorted order.
Fractional fixture now compares the exact sorted multiset, keeping exact3,
provenance, component, uncropped whole and full-route assertions unchanged.

Source inspection: shape_key accepts the actual Fn value; callback discovery
checks permitted native dependencies, actual vm.call_value result, then actual
selected-source/Buffer identity and closed admitted family resources. Count0
can mean a retained rejected descriptor; the prior assertion omitted its reason.
The unchanged count1/noRejected requirements now report the actual rejected map
and copied namespace callback, so mandatory diagnostic execution can distinguish
UncertifiedCallback from UnclosedResource. No synthetic prepared output or
production change is made. Static inspection alone does not establish which
rejection path ran; Shift is still unresolved pending that actual evidence.

### Source-grounded callback body correction before diagnostic execution

Root confirmed compiler.rs469–477 lowers a bare parameter statement p as
LoadLocal followed by Call0 (maybe-do statement), not returning the original
Pattern value. It therefore cannot be the intended identity transform in this
fixture. Existing genuine tests/song_checker.rs269 defines an eager identity
using first[p]. The actual evaluator definition now uses that same first[p]
body, retaining actual discovery/copy/admission and strict one-pattern/noRejected
requirements with diagnostic details. No callback is executed outside sanctioned
discovery and normal runtime query; no production behavior or synthetic metadata
is changed. Runtime success remains pending mandatory verification.

### Focused genuine witness acceptance — ROOT0508

Root independently accepted checker004 terminal0, all15 commands and34 unique
actual PASS names against disjoint list/run inventories, all907 pre/post/current
inputs and15 raw log hashes. Acceptance receipt
`tmp/song-mode-riela/ROOT0508-family-genuine-witness-focused-acceptance.json`
SHA300013cc3265c4c1a4becef102230623377bf577ca127a4675600cea5387a1d3.
This supersedes pending status of the three genuine remaining witnesses: real
Off shared discovery/Freeze/admission has one accepted identity callback and
zero rejected; Shift/full-source identity/reordered partial routes pass; exact
fractional capture multiset and original uncropped whole pass; genuine full-route
smallest work/depth and one-less precise failures pass. Earlier family00231
counts overlap this34 and are not added to fabricate65 distinct tests.

Overall plan remains In Progress. The exact source/owner eligibility criterion
stays unchecked for source-filtered multi-family unions beyond the complete
START range proof. The full58 integration matrix remains pending, as do
heterogeneous/nonlinear/multiple inherited/callable/dynamic canonical authority
and actual host/transport/Apply/mute/playback/export. No archive/index or source
changes; this record is focused verification and next-wave handoff only.
