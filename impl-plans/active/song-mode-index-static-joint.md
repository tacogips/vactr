# Static joint Index components

**Status**: Planning
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)

## Purpose and dependencies

Complete heterogeneous static Index connected components after authentic operand
binding. Preserve immutable Parts, original source identity, symbolic copies and
the original cumulative work limit. This plan is a production prerequisite for
remaining geometry, not completion of dynamic realization or host playback.

- Previous: [Index occupancy](song-mode-index-occupancy.md),
  [Index families](song-mode-index-families.md),
  [Weighted source layout](song-mode-weighted-source-layout.md).
- Next: retained canonical dynamic Index consumer, with a proved uniform birth
  ceiling and original immutable evaluator authority. That remains required.
- Source author has not started. No Rust or Cargo authorization accompanies this
  document. Ready requires review of TASK-002's exact bounded elimination method.

## Actual source evidence

- `pattern/combinators/region.rs:351`: when subject is unstructured and Index is
  structured, query the Index, sample the subject at the Index whole start, issue
  timing, then copy Index whole/part. Subject-mode public `slice p` tests cannot
  prove Index geometry.
- `routing/index.rs:362,596`: static admission preserves occupied/rest nodes and
  heterogeneous weighted support summaries. RequiresRealization is a borrowed,
  addressed request; it is not an implemented canonical consumer.
- `configuration/index.rs:153,321`: actual trace/copy binding and weighted Slow
  emission exist; current multi-family resolution delegates to cluster, then
  falls back only for one family/one copy.
- `configuration/index/cluster.rs:119`: common width/common Slow are required;
  source eligibility is currently checked against a complete cluster, so a
  source-trimmed subset of heterogeneous wholes is not solved.
- `density/index.rs:342`: unresolved recipes immediately fail admission; static
  support bounds exist. Do not replace configuration birth bounds with emitted
  note counts or a query event cap.
- `nested.rs:679` and `nested/clock.rs:15`: authenticated positive affine issuer
  clocks already carry separate owner and canonical birth-owner windows. Keep
  these callers read-only in this increment; do not infer the issuer clock from
  the final event whole.
- `session/song/inventory/index_occupancy.rs:43,209,278`: genuine false-structured
  subject helpers run original discovery/Freeze/capture/admission/query/resolve.
  New fixtures extend this seam, not fabricated FrozenSliceTiming DTOs.

## Exact future Rust manifest

| Path | Current lines | Deliverable |
|---|---:|---|
| `src/song/routing/index.rs` | 835 | Shared private authenticated static family program and support projection |
| `src/song/routing/configuration/index.rs` | 655 | Joint dispatch; preserve actual issued identity/emitter checks |
| `src/song/routing/configuration/index/cluster.rs` | 339 | Retain proved common-clock fast path, compare with general solver |
| `src/song/routing.rs` | 641 | cfg(test)-only ordinary re-export of authenticated compiler observation |
| `src/song/routing/index/static_families.rs` | new | Shared authenticated family compiler and composed-rate producer program |
| `src/song/routing/density/index.rs` | 516 | Consume shared static support, retain conservative birth proof |
| `src/session/song/inventory/index_occupancy.rs` | 665 | Register cohesive child and expose existing private fixture helpers to child |
| `src/session/song/inventory/index_occupancy/static_joint.rs` | new | Genuine heterogeneous, trimmed, quota and partition fixtures |

Eight paths only; no configuration facade, nested, query VM, preparation or
capture changes. Before substantive growth, move general solver code into the
declared children. Every touched Rust file must remain below 1000 lines.
Baseline hashes and new-file absence: `SONG-INDEX-STATIC-JOINT/0001-document-intent.json`
under `tmp/song-mode-riela/`.

## Private declarations

Declarations below specify interfaces, not implementation bodies. Existing
`PreparedSliceAddress`, `BoundSliceOperands`, `IndexNodeSupport`, `TimeSpan`,
`Ratio64` and `ResolutionBudget` remain the authority and arithmetic types.

```rust
// routing/index.rs: fields private, routing ancestor visibility throughout.
pub(in crate::song::routing) struct StaticIndexFamilies;
pub(in crate::song::routing) struct StaticIndexFamily {
    prefix: Ratio64,
    width: Ratio64,
    copies: u32,
    slow: Ratio64,
}
pub(in crate::song::routing) fn compile_static_families(
    prepared: &PreparedSliceAddress<'_>, depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<Option<StaticIndexFamilies>, Failure>;
impl StaticIndexFamilies {
    pub(in crate::song::routing) fn support(&self) -> IndexNodeSupport;
    pub(in crate::song::routing) fn families(&self) -> &[StaticIndexFamily];
}
// configuration/index/joint.rs; privately mounted by configuration/index.rs.
pub(super) fn component(
    bound: &BoundSliceOperands<'_>, families: &StaticIndexFamilies,
    source: TimeSpan, owner: TimeSpan, depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<TimeSpan, Failure>;
// joint/lattice.rs; mounted only by joint.rs.
pub(super) struct FamilyConstraints;
pub(super) fn compile(
    families: &StaticIndexFamilies, source: TimeSpan, owner: TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<FamilyConstraints, Failure>;
pub(super) fn first_gap(
    constraints: &FamilyConstraints, begin: i128, end: i128,
    reverse: bool, depth: u32, budget: &mut ResolutionBudget,
) -> Result<Option<i128>, Failure>;
```

`None` from compile denotes an addressed non-family recipe, not silent empty
music. Existing static admission for Rev/other operations must remain intact;
they are not relabeled solved by this family compiler. Dynamic fields retain
their exact existing request and original operand identity.

## Exact mathematical consumer

For family prefix p, width w>0, copies m, Slow s>0, use integer variables parent
cycle c, symbolic copy j (0<=j<m), and child cycle k. Define slot start b=c+p+j*w,
whole start x=(1-w)*c+p+j*w+w*s*k and length L=w*s. Preserve actual normalized
weights; unwrap Repeat/Hold exactly where Steps already owns the squeeze.

An emitted whole is eligible exactly when all of these hold:

- b<owner.end and b+w>owner.begin (slot really queried);
- x<min(b+w,owner.end) and x+L>max(b,owner.begin) (child whole overlaps the
  squeezed slot query; both strict inequalities preserve half-open endpoints);
- source.begin<=x<source.end (sample START membership in the held source);
- authentic issuer trace/copy/whole resolves this actual family and integer k.

Source end never truncates x+L. Union eligible complete wholes first, merge
touching intervals, select the connected component containing the authenticated
issued whole membership, then clip to configuration owner. Full source handle,
policy and placement identity remain untouched; leaf Index values are not new
configuration identities. Return no convex hull across an uncovered gap.

### Non-expanding decision approach requiring proof review

Scale all rational coefficients/endpoints to a checked integer lattice using
LCM. Each family above is a fixed-dimensional integer constraint set; copy count
is a bound on j, never an allocation size or iteration count. A lattice segment
[z,z+1) is covered iff some family has eligible (c,j,k) with x<=z<x+L.

Use bounded integer constraint elimination to decide whether any uncovered z
exists in a requested integer range. Negate the union's existential coverage
formula; eliminate c/j/k using floor bounds and exact congruences. Preserve
disjunctions and strict inequalities. GCD/Euclidean reductions compress residue
classes; no scanning parent cycles, copies, note events or finite placements.
Then find nearest uncovered segments left/right of an authenticated interior
segment through range bisection using this existential-gap decision. At most
the checked lattice endpoint bit width determines the bisection count, not
score length. Clip endpoints back with checked rational arithmetic.

This is a concrete decision reduction, **not yet a proved implementation**.
TASK-002 must specify the finite elimination representation and its exact
residue/storage bound before Ready. In particular, enumerating an LCM-sized
period or repeatedly asking adjacent-event neighbors is not an acceptable
substitute. If exact residue elimination cannot fit the shared limit, return a
genuine limit failure before allocation; do not misreport static operands as
dynamic or weaken family/whole eligibility. Limits are tied to original recipe
and canonical owner, never caller partition size.

## Work, bounds and authentication

- Charge scans, actual full trace/handle equality, copied scalar words, DAG
  heights, family records, constraint rows, Euclidean reductions, branches,
  bisection decisions and temporary storage before the operation/allocation.
- Reuse the incoming ResolutionBudget and depth. No helper resets max_nodes,
  no cached uncharged proof and no new default SongLimits.
- Check every LCM, rational/integer conversion and intermediate coefficient;
  overflow fails explicitly. Huge symbolic counts stay scalar.
- Geometry consumes original bound timing; admission consumes the same compiled
  static families/support. Existing conservative IndexNodeSupport event-start
  envelope remains an upper bound for configuration openings, not an equality
  inferred from emitted notes. Do not multiply a configuration bound twice.
- A dynamic recipe must eventually retain original canonical realization and
  uniform birth authority under shared work. This plan neither executes VM
  callbacks nor treats unresolved dynamic support as final feature exclusion.

## Tasks

### TASK-001: Shared static program and exact emission constraints

**Status**: In Progress
**Parallelizable**: No

Release scope: implement the authenticated family compiler and integrate its
static support/replay into admission and configuration. Preserve the existing
cluster fast path and addressed unresolved joint result. Implement composed
positive rate recognition with genuine internal fixtures and inherited budget
accounting. This task does not require TASK-002's unproved gap solver and does
not declare heterogeneous components or dynamic realization complete. Keep
all edits within the eight declared paths; unused solver children need not be
created. The overall plan remains Planning until TASK-002's proof gate closes.

- [ ] Compile heterogeneous scalar/rest Steps and normalized symbolic copies;
  nested positive rates compose algebraically without reinterpreting Repeat.
- [ ] Bind actual issued leaf/copy/whole with complete trace/subject authority.
- [ ] Prove the inequalities above against actual query Steps/Slow semantics.
- [ ] Preserve existing non-family and dynamic requests without music narrowing.

### TASK-002: Prove and implement bounded gap elimination

**Status**: Planning
**Parallelizable**: No (depends on TASK-001)

- [ ] Before Ready, declare exact integer elimination records, congruence
  operations and charged worst-case row/residue/storage bound.
- [ ] Prove existential uncovered-segment decisions, touching merge, nearest-gap
  bisection, negative cycles and partial owner boundaries.
- [ ] Implement joint components and compare existing cluster fast path with
  general results; no family/copy/period/score expansion.
- [ ] Source-START filtering precedes union; whole end remains authentic.

### TASK-003: Genuine fixtures and independent verification

**Status**: Planning
**Parallelizable**: No (depends on TASK-002)

- [ ] Internal genuine Index candidate with widths 1/3 and 2/3, different Slows,
  rest gaps, touching wholes, fractional owner head/tail and source trimming.
- [ ] Symbolic repeated copies with first/last partially queried copy; exact
  zero-copy silence and huge-count bounded-work success or addressed limit.
- [ ] Positive outer Fast/Slow/Shift plus Sequence offsets preserve issuer-clock
  components, complete route equality and original source handles.
- [ ] Full/fractional/point/reordered partitions produce identical whole,
  configuration, controls and complete identities; continuation is not onset.
- [ ] Exact sufficient work and one-less fail against the original cumulative
  budget; repeat count/caller partition cannot create a fresh quota.
- [ ] Independent native/WASM/strict lint/scoped format and genuine fixture
  execution; no success based on expected equations alone.

Expected components must be derived independently from actual runtime-emitted
whole STARTs over small finite canonical fixtures, not copied from the new
solver. Existing family_rows assumes four members/component and must not be
reused blindly for heterogeneous row counts. New child helpers are
`joint_rows(index, rate, offset, owner, expected_components)` and
`assert_joint_partitions(prepared, windows, expected_routes)` using actual
PreparedSong query/prepare/resolve and complete equality.

## Progress log

### 2026-10-03 — Source-grounded document preflight

Created Planning plan and exact eight-path future manifest. Existing source
unchanged. Common-width/common-Slow and source-trimmed multi-family restrictions
are actual geometry gaps; no empirical new failure is claimed. Shared static
constraint reduction is declared, while its bounded elimination implementation
is explicitly the Ready gate. Dynamic retained canonical consumer remains a
separate mandatory next task. No Rust authored, no Cargo run, no completion
criterion checked.


### 2026-10-03 — TASK-001 source declaration

Root authorizes TASK-001 only. Replace unused future lattice child with ordinary
routing/index/static_families.rs under mod static_families; manifest remains8.
General joint child remains uncreated. StaticIndexFamily retains normalized
geometry, copies, effective positive Slow and complete original producer terms.
StaticIndexFamilies::matched(bound,depth,budget) returns actual family index and
symbolic copy only after complete trace validation; configuration separately
replays the original weighted whole equation and source START membership.
compile_static_families(prepared,depth,budget) charges original slot/edge/leaf
scans, depth and copied terms before allocation. Normalize nested positive
Slow/Fast/Hurry by multiplying Slow/inverse rates along actual original child
edges; unwrap the single Steps-owned Repeat/Hold once, never expand copies.
Admission uses compiled support with the existing conservative postorder bound;
configuration/cluster consume the same compiler records, including one-family
fallback. No general gap solver or dynamic consumer is implemented here.


### TASK-001 genuine fixture declarations

static_joint child extends actual prepared_index/capture/Freeze/admit/query
helpers, without new provenance constructors. A cfg(test)-only routing/index
inspect_static_family_program(inventory,scope,track,depth,budget) binds the real
root Slice recipe through output_operand_owner/prepare_slice_operands and returns
copied (prefix,width,copies,slow) observations. This enables heterogeneous and
million-copy recognition/quota assertions without querying expanded million
events or widening production visibility. Actual composed-rate rows separately
resolve routes and preserve full issued timing/handles under reordered windows;
compare direct Slow2 geometry/payload, not forged equal producer identities.
Tests include heterogeneous temporary joint refusal, Rest/Repeat0, full route
work/depth exact/one-less and actual million-copy program work independent of
copy count. Native compiler checkpoint63191/101 was a missing capacity_overflow
call in error conversion; no tests executed. Fix only that call before fixtures.


### TASK-001 test visibility amendment

Root approves replacing unused general joint.rs with routing.rs solely for a
cfg(test) ordinary re-export. Session fixture cannot access private routing/index
or ResolutionBudget directly. Observation helper accepts original inventory,
scope/track, caller depth and original SongLimits, binds the real copied recipe,
charges one budget throughout, then returns family descriptors and actual spent
work. No production visibility changes or reset within an operation. Complete
route quota/depth tests continue to call actual resolve_route.


### TASK-001 source-ready checkpoint

Shared static compiler is wired into admission (therefore density) and actual
configuration replay. Complete composed producer terms, symbolic count and
normalized rates are retained; the existing cluster and one-family solvers
consume the same program. The cfg(test) observation is isolated from production
visibility. Five genuine static_joint fixtures cover nested Slow/Fast/Hurry,
heterogeneous recognized weights with temporary addressed joint refusal,
Rest/Repeat0, exact full-route cumulative work/depth, and actual captured million
copies without querying expanded events. Existing assertions remain intact.
Scoped formatting passed and every touched Rust file is below1000 lines. No
Cargo execution by author; fixture success and compiler acceptance are pending.
TASK-001 remains In Progress until independent verification; TASK-002/003 and
full dynamic/inherited geometry remain unfinished. Root review now prefers a
bounded shared canonical realization fallback for unresolved static/dynamic
cases, with authentic uniform opening bounds including vary seeds. The general
Presburger algorithm above is an approach proposal, not a required feature
contract; this source increment does not implement that fallback.


### TASK-001 bounded pattern Repeat fixture correction

Independent original74362/0 verified7 regression suites98PASS. Full private
Index original41446/101 verified16PASS/1FAIL; new fixtures4PASS/1FAIL. The failed
large-request expectation incorrectly assumed pattern Repeat's requested million
count survives normalization. Runtime step.rs90 clamps count to4096; captured
source_uses/timing.rs250 applies the same original limit. No compiler defect or
production change. Rename the fixture to
large_requested_pattern_repeat_preserves_existing_limit_without_expansion and
assert exact4096 copies with width1/4097. Retain actual copied-rate2, identical
compiler work versus10 copies, exact/one-less original budget and measured
physical admission refusal. Arrangement Part-repeat is a distinct symbolic
contract and remains unchanged. Earlier million-copy statements describe the
requested input only, not admitted/captured cardinality; no million-event or
unbounded pattern-repeat success is claimed. Source correction awaits independent
private rerun; prior98 regression passes apply to unchanged production.


### TASK-001 independent evidence and canonical direction

Corrected private whole17/17 passed including all5 new fixtures; native check0.
Prior unchanged-production7 regression suites98 passed. StrictalltargetsClippy
then found only existing session/song/transport applied-catalog type complexity;
root assigned that isolated repair to its owner. Final strict acceptance remains
pending that terminal, so TASK-001 is not marked Completed here. Remaining
canonical work is declared in song-mode-canonical-index-occupancy.md: executable
metered original pre-filter collector, mandatory immutable consumer handoff and
explicit uniform-Vary gate. General Presburger elimination is not required by
the original feature contract. Source held unchanged; documentation only.
