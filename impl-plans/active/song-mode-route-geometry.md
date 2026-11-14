# Song source configuration geometry implementation plan

**Status**: In Progress
**Plan ID**: SONG-08D
**Created**: 2026-10-01
**Last Updated**: 2026-10-02
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Purpose and handoff

Complete exact finite source configuration geometry for composed song patterns.
SONG-08B retains admission, density, capacity and detector ownership. This child
owns geometry and its route fixtures after ROOT0179. It consumes the already
implemented preparation/types and source certificates; it does not depend on
full SONG-08B completion. SONG-08B consumes this child's completed geometry.
The full goal still includes actual DSP playback, hosts, transport and export.

## Manifest

```json
{
  "planId": "SONG-08D",
  "planPath": "impl-plans/active/song-mode-route-geometry.md",
  "dependsOn": ["SONG-07I", "SONG-SOURCE-WHOLE", "SONG-08B stable preparation interfaces"],
  "writePaths": [
    "src/song/routing/configuration.rs",
    "src/song/routing/nested.rs",
    "tests/song_source_routes.rs",
    "tests/song_source_routes/grid.rs",
    "src/song/routing/configuration/sampling.rs",
    "src/song/routing/configuration/sampling/reflection.rs",
    "impl-plans/active/song-mode-route-geometry.md"
  ]
}
```

## Related plans and dependencies

- Previous: [Static sampling](song-mode-source-sampling-layout.md).
- Parent and consumer: [Route preparation](song-mode-route-preparation.md).
- Next: parent SONG-08 integration, then actual DSP routing.
- Independent frozen-cell inventory may progress on disjoint paths; coordinate all Rust writers before checks.

| Input | Required evidence | State |
|---|---|---|
| SONG-07I | Immutable static sampling/provenance contracts | Independently verified |
| SONG-08B interfaces | Current prepared cover/route/budget types | Held handoff |
| Basic affine grid routing | ROOT0177, 80 actual focused tests | Verified for stated scope |
| Nested sampled context | TASK020 actual regression | Written, unexecuted |

## Modules and declarations

| Module | Responsibility | Status |
|---|---|---|
| routing/configuration.rs | Exact configuration inversion and bounded recipe dispatch | In Progress |
| routing/nested.rs | Compose inherited source stages with immutable identities | In Progress |
| routing/configuration/sampling.rs | Ordered sampled clocks and joint recipe collection | In Progress |
| routing/configuration/sampling/reflection.rs | Cohesive reflection helpers/tests after prerequisite handoff | NOT_STARTED |
| tests/song_source_routes.rs | Shared actual candidate helpers and non-grid route regressions | In Progress |
| tests/song_source_routes/grid.rs | Grid masks, sampled configurations and nested regression fixtures | In Progress |

Existing central integration signature:

```rust
pub(super) fn map_intrinsic_configuration(
    cover: &FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &[ProducerStep],
    span: TimeSpan,
    anchor: Ratio64,
    source_onset: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure>;
```

Proposed sampled-context boundary; declare any refinement before implementation:

```rust
pub(in crate::song::routing) enum SourceLocatorMapping {
    UnchangedBirth,
    SampledPoint(Ratio64),
    NeedsJointGeometry,
}
pub(in crate::song::routing) fn map_source_locator(
    cover: &FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &[ProducerStep],
    output_birth: Ratio64,
    incoming_sampled: Option<Ratio64>,
    budget: &mut ResolutionBudget,
) -> Result<SourceLocatorMapping, Failure>;
```

A sampled locator is distinct from original handle/birth provenance. It remains
active through inherited affine and static-conditional stages. An unsupported
sampled operation cannot silently reset it to an ordinary birth. Ordinary held
continuations keep existing query-invariant configuration ownership.

Initial fixture split declarations:

```rust
// tests/song_source_routes.rs
#[path = "song_source_routes/grid.rs"]
mod grid;
// tests/song_source_routes/grid.rs
use super::*;
#[test]
fn selected_grid_masks_resolve_maximal_components_across_real_query_partitions();
#[test]
fn grid_interior_point_authenticates_fractional_source_sample_start();
#[test]
fn nested_periodic_sources_use_grid_sample_start_as_distinct_configuration_locator();
```

The child fixture module inherits the existing genuine candidate/query/resolve
helpers. Preserve all assertions and fully qualified fixture accounting. An
explicit #[path = "song_source_routes/grid.rs"] may be required by integration
crate module resolution; declare it before editing the parent test.

## Execution contract

Use specialized Rust author and mandatory checker. Record immutable numbered
intent, exact signatures and current baselines before each change. Only five
Rust paths and this plan may be written; parent plan/index/archive are held.
No touched Rust file may remain at 1000+ lines; use cohesive declared child
modules before growth. Plan stays <=1000 lines, <=8 Rust paths and <=10 tasks.
No dependencies, lockfile changes, Git operations or broad formatting.
All work/allocation admission uses the caller's cumulative quota. Spent sampler
probes are excluded from matcher allowances, as verified by ROOT0177.
Do not turn conservative support hulls into exact component membership.
No VM/query fallback, per-note configuration keys or score/repeat expansion.
Retain actual failure logs and originating handles; poll to terminal without
restarting on observation timeout. Quiet Cargo/nextest follows AGENTS.md.

## Tasks

### TASK-001: Cohesive fixture split and actual nested evidence
**Status**: In Progress
**Parallelizable**: No
- [x] Move three contiguous grid fixtures intact into the declared child module.
- [x] Keep parent and child below1000 lines and preserve the prior size deviation.
- [ ] Inventory the relocated fixtures and execute the new nested case.
- [ ] Preserve actual failure/success, full event diagnostics and terminal handle.

### TASK-002: Sampled context through inherited stages
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No
- [ ] Implement explicit sampled context with original source identity unchanged.
- [ ] Handle affine/static condition stages and multiple inherited frames.
- [ ] Prove cycle2 quarter-cell and point17/8 retain exact full route/handle.
- [ ] Preserve ordinary held conditional, offset, repeated and overwritten routes.

### TASK-003: Complete admitted composed configuration geometry
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No
- [ ] Compose weighted slots and ordered grids without greedy component pinning.
- [ ] Complete retimed Chunk, cycle selection/concatenation, reflection, integer sampling, subdivision, restructuring and slice geometry for admitted static recipes.
- [ ] Keep stochastic conditional identities deterministic for the sealed snapshot.
- [ ] Prove maximal connected components, real gaps, full source scopes and original family policy.
- [ ] Keep work/depth bounded for huge finite durations and symbolic repetitions.
- [ ] Declare any further private types or new module paths before edits.

### TASK-004: Joined verification and parent handoff
**Status**: Not Started
**Depends On**: TASK-003
**Parallelizable**: No
- [ ] Meaningful actual route fixtures pass under full, fractional and reordered queries.
- [ ] Relevant native/browser, source certificate and legacy route regressions pass.
- [ ] Required formatting, clippy and parent integration checks pass or retain concrete unrelated failures.
- [ ] Independent checker validates final unchanged source, exact fixture names and terminal results.
- [ ] Parent accepts geometry interface handoff; this child alone does not claim playback.

## Verification

For the initial test-only split, list song_source_routes tests and run the exact
new nested test, preserving an expected failure rather than suppressing it.
After production changes run the affected geometry/route/source suites and
native/browser compilation, then the parent's required joined checks. Repeat
checks only for new changes, failures or unresolved concerns.

## Completion criteria

- [ ] All four tasks and admitted geometry requirements complete.
- [ ] No unresolved sampled-context or conservative-hull substitute remains.
- [ ] All required evidence matches actual current code and fixture scope.
- [ ] Parent handoff recorded, unrelated work preserved and no active processes.

## Progress log

### 2026-10-01 — child handoff
ROOT0177 verified basic grid routing and probe accounting (80 focused tests).
TASK020 fixture formatting reached1004 lines; its exact bytes and deviation are
retained in SONG-08B/TASK-020-fixture-size-deviation-held-004.json.
ROOT0179 creates this bounded child and requires a cohesive test module split.
No nested regression has executed and no production geometry is released yet.

### 2026-10-01 — TASK-001 split ready
ROOT0180 released only both test files and this plan. The three contiguous grid
fixtures moved with identical bodies into `song_source_routes/grid.rs`, using an
explicit path module and `use super::*`. Exact pre-format body-copy proof is in
SONG-08D/TASK-001-split-body-proof-003.json. Scoped rustfmt completed; no Cargo
ran. The nested fixture remains unexecuted. Both test files and this plan are
held for the targeted mandatory checker; production geometry remains held.

### TASK-002 exact local integration declaration

`configuration.rs` declares `mod sampling;` and re-exports the two declarations
above to sibling `nested.rs`. `configuration/sampling.rs` owns their bodies.
Private `Stage<'a>` adds `sampled_locator: Option<Ratio64>`. Resolution carries
`incoming_sampled: Option<Ratio64>` from the outer stage inward; successful
`SampledPoint(input)` becomes `Some(input - scope.offset)` for the next stage.
The current stage stores its incoming point, or its authentic output birth when
a local sampler activates the context. Reverse component resolution uses this
distinct point as its locator, retaining both original onset fields unchanged.
Rate and Shift map the point in operation order. Static Every/WhenMod preserve
the point; grid quantizes to its exact sample start. Nonunit layouts or other
sampled recipes return NeedsJointGeometry and cannot reset active context.
Ordinary stages without any sampler and without incoming context retain their
existing birth logic. All visits and layout inspections debit the shared quota.
The actual ROOT0181 probe failed Type at output[3,13/4), outer sample3 versus
inherited onset0; original18068 terminal101 is retained. No implementation or
full geometry clearance is inferred from this declaration.

The child helper enum/function use `pub(in crate::song::routing)` so the
configuration facade can re-export them to sibling nested resolution.
Additional actual fixture declaration:
```rust
#[test]
fn nested_grid_locator_persists_through_three_frames_affine_and_offset_scopes();
```
It compares exact cell configurations and complete identities across point and
reordered half-cell queries for three source frames, middle/outer rate mappings
and a shifted Sequence placement. These remain unexecuted until joint holds.

### 2026-10-01 — TASK-002 focused implementation held
ROOT0181 confirmed the genuine nested regression failure: original18068/101,
Type unresolved geometry, output[3,13/4), sampled outerpoint3 and inheritedbirth0.
ROOT0182 released this focused correction. Ordered Rate/Shift/grid sample-start
propagation and inherited Every/WhenMod context are implemented in the child
helper and Stage context; original source/FX onset and handle fields are intact.
The exact regression assertions remain unchanged. A second genuine fixture
adds three frames, middle/outer rate and shifted Sequence cases. No Cargo ran;
all five Rust paths and this plan are held for mandatory joined checks.
Multiple-grid, sampled Chunk/weighted joints and remaining nonlinear recipes
are still TASK-003 work; no full task/phase/playback readiness is claimed.

### TASK-003 composed sampled recipe declarations

Existing authorized `configuration/sampling.rs` owns these private declarations;
configuration re-exports the routing-visible types/functions to nested.
```rust
pub(in crate::song::routing) struct SampledRecipeInput {
    pub output_birth: Ratio64,
    pub incoming_member: Option<Ratio64>,
    pub source_birth: Ratio64,
    pub source_scope: TimeSpan,
    pub output_owner: TimeSpan,
}
struct SampledStep<'a> {
    node: &'a FrozenSourceUseNode,
    edge: &'a FrozenSourceUseEdge,
    trace: &'a [ProducerStep],
    ordinal: u32,
    output_member: Ratio64,
    input_member: Ratio64,
    slot_cycle: i64,
    output_owner: TimeSpan,
}
pub(in crate::song::routing) struct SampledRecipe<'a> {
    input: SampledRecipeInput,
    steps: Vec<SampledStep<'a>>,
    input_member: Ratio64,
}
fn sampled_steps<'a>(
    cover: &'a FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &'a [ProducerStep],
    point: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<Vec<SampledStep<'a>>>, Failure>;
pub(in crate::song::routing) fn sampled_recipe<'a>(
    cover: &'a FrozenSourceUseCover,
    identity: &FrozenSourceUseIdentity,
    trace: &'a [ProducerStep],
    input: SampledRecipeInput,
    budget: &mut ResolutionBudget,
) -> Result<Option<SampledRecipe<'a>>, Failure>;
pub(in crate::song::routing) fn sampled_configuration(
    recipe: &SampledRecipe<'_>,
    source: TimeSpan,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure>;
fn quantized_point(point: Ratio64, divisions: i64) -> Result<Ratio64, Failure>;
fn sampled_chunk_run(
    divisions: i64,
    transformed: bool,
    member: Ratio64,
    whole_anchor: Ratio64,
) -> Result<TimeSpan, Failure>;
```
Nested gathers every inherited recipe and its complete source scope before
resolving components. Each local constraint selects the containing run at the
propagated authentic membership point, never the earliest run at source birth.
The reverse pass starts from actual source birth and reconstructs emitted whole
anchors; grids replace emitted timing with their stored sample start, while
Chunk reads the child whole anchor fraction. Weighted slots use the authenticated
copy layout and membership cycle, not clipped query spans. Grid quantizer
preimages retain enabled-mask constraints through the existing charged sampler.
All step capacity is admitted before allocation; borrowed trace/node/edge data
remain immutable. Every walk, layout, mask and component scan shares caller fuel.
Unsupported sampled operations remain explicit pending TASK-003 work.
```
#[test]
fn composed_grids_preserve_child_whole_and_maximal_sampled_runs();
#[test]
fn sampled_chunk_uses_held_whole_anchor_instead_of_membership_fraction();
#[test]
fn sampled_weighted_slots_preserve_intrinsic_cycle_and_rest_gaps();
#[test]
fn composed_grid_sequence_offsets_preserve_global_component_identity();
```
Fixtures retain actual full/reordered/fractional/point identity and exact maximal
component assertions; all four were source-derived proposals, not prior passes.
TASK002 evidence: ROOT0196 recorded all28 routes including both new sampled
locator regressions; native/browser passed. No full TASK003 clearance follows.

Exact inherited integration helper in `nested.rs`:
```rust
fn sampled_stage_configuration(
    stages: &[Stage<'_>],
    owner: TimeSpan,
    offset: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<TimeSpan, Failure>;
```
It admits recipe storage, collects every complete stage constraint before any
component clipping, then resolves containing runs from the deepest source out.
Each frame boundary adds the stored output offset; final root offset is applied
once. No-sampler resolution retains its existing joint affine/ordinary paths.
Authenticated emitted whole replay must equal the existing Stage output birth;
this checks provenance separately from sampled membership.

Exact test helper declarations in `tests/song_source_routes/grid.rs`:
```rust
fn composed_grid_song(inner: &str, outer: &str, shifted: bool) -> PreparedSong;
fn assert_sampled_partitions(
    song: &mut PreparedSong,
    plan: &SongRoutePlan,
    rows: &[FrozenSongEvent],
);
```
They retain complete handle/route equality for reordered cell-interior points
and both half-cell queries. Source-derived expected maximal intervals are exact
quarter runs for composed grids, whole cycles for fixed-anchor Chunk, three
quarters for weighted slots, and global offset quarters for shifted Sequence.

Exact repeated layout work helper:
```rust
fn charge_slot(edge: &FrozenSourceUseEdge, budget: &mut ResolutionBudget)
    -> Result<(), Failure>;
```
Each layout API invocation charges twice the layout entries plus actual trace
length before validating/replaying its borrowed copy recipe. Calls for point,
window, unit interval and each outward span/whole anchor are separate debits.
No prior matcher or locator reservation is reused to cover these new scans.

### 2026-10-02 — TASK-003 composed sampled increment held
ROOT0200 released implementation after the brief stable check window. Exact
prior declarations/intents are TASK-003-composed-declaration-intent-001 through
slot-accounting-intent-005. Borrowed ordered recipes and all inherited scopes
are collected before selecting components. Grid sample starts and slot cycles
carry membership separately from source birth; whole timing is reconstructed
outward and checked against authenticated Stage output birth. Fixed-anchor
Chunk uses cycle runs rather than incoming membership fractions. Each resulting
constraint must contain the propagated member; no independent earliest run or
conservative-hull membership substitutes for this test. Repeated layout trace
validation and sampler work are cumulative debits, with no matcher reset.
Four genuine regressions assert exact maximal runs, source/whole distinctions,
rest gaps, complete routes and reordered point/half-cell partitions. They have
not executed; no Cargo ran. All five Rust paths and this plan are held for the
mandatory coordinated checker. Whole-emission equality and asserted source-
derived fixture expectations remain subject to actual runtime evidence.
Other admitted nonlinear mappings, sampled Iter/Rev/Cat and generalized joint
recipes remain unfinished; no complete TASK-003 or phase clearance is claimed.

### 2026-10-02 — missing intersection compile repair
Joined checker original6247/101 stopped at four E0425 sites in sampling.rs; no
behavioral tests executed. ROOT0207 releases only the bounded repair. Exact
private declaration in sampling.rs:
```rust
fn intersect(left: TimeSpan, right: TimeSpan) -> Result<TimeSpan, Failure>;
```
It preserves the existing nested helper's checked nonempty intersection contract
and addressed Type failure. Geometry, fixtures and09PC remain unchanged; author
Cargo remains disabled and all owned paths are resealed after scoped formatting.

### 2026-10-02 — ROOT0217 source-derived fixture correction
Actual joined checker22382/101 ran32 routes:29 passed, three fixture assertions
failed. Both composed-grid fixtures used outer4/8 (alternating pulses), so the
inner quarter contains only one enabled outer phase per cycle. Use full8/8 to
exercise the originally intended two adjacent eighth cells; retain eight rows
and exact quarter component assertions. Chunk exact per-row cycle intervals
passed; contiguous chunks assumed chronological order, contrary to query's
BTreeMap<EventHandle> ordering (producer ordinals precede cycle). Group by
intrinsic output whole cycle instead, asserting exact phase sets, cardinality,
complete route equality within each cycle and distinct adjacent configurations.
Exact private fixture declaration before implementation:
```rust
fn assert_sampled_cycle_groups(
    plan: &vactr::song::routing::SongRoutePlan,
    rows: &[vactr::song::snapshot::FrozenSongEvent],
    cycles: std::ops::Range<i64>, denominator: i64, phases: &[i64],
);
```
All previous full/fractional/reordered identity assertions remain. This is a
fixture-only correction, not an established production geometry defect. Retain
independent003 failed logs; author Cargo stays disabled pending coordinated hold.

### ROOT0226 exact cycle mapping increment declarations
Private helpers in configuration/sampling.rs, before Rust implementation:
```rust
#[derive(Clone, Copy)]
enum CycleMembership {
    Identity,
    Iterate { count: u32, shift: Ratio64 },
    Cat { input_cycle: i64, shift: Ratio64, scale: Ratio64 },
}
fn cycle_membership(
    mapping: &M, arity: usize, ordinal: u32, member: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<(Ratio64, CycleMembership)>, Failure>;
fn cat_birth_context(
    mapping: &M, arity: usize, ordinal: u32, output_birth: Ratio64,
    source_birth: Ratio64, budget: &mut ResolutionBudget,
) -> Result<Option<CycleMembership>, Failure>;
fn replay_cycle_component(
    context: CycleMembership, source: TimeSpan, owner: TimeSpan,
    member: Ratio64, whole_anchor: &mut Ratio64, budget: &mut ResolutionBudget,
) -> Result<TimeSpan, Failure>;
pub(super) fn ordinary_cat_configuration(
    cover: &FrozenSourceUseCover, identity: &FrozenSourceUseIdentity,
    trace: &[ProducerStep], source: TimeSpan, output_birth: Ratio64,
    source_birth: Ratio64, budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure>;
```
SampledStep retains one Copy CycleMembership context per already charged edge.
Iterate uses exact count and cycle phase; Cat/FastCat retain all child arity,
including rests, and authenticate the selected ordinal. Whole replay is separate
from membership; layout inversion follows cycle forward mapping, and layout
outward replay precedes cycle replay. Shared work charges all scans and storage.
Ordinary Cat initially supports one Cat/FastCat with affine/preserving wrappers
and unit layouts: solve exact uncut whole equation for intrinsic input cycle,
then regenerate that cycle's maximal support, never use query clip as key.
Multiple ordinary Cat or nonlinear conditional constraints remain queued, not
claimed solved by this specialized fast path. Rev and other mappings remain
mandatory. Signed cycle proof is helper-level; actual root negative anchors are
filtered by canonical query. Exact fixture declarations:
```rust
fn sampled_iterate_preserves_intrinsic_policy_and_grid_cell_partitions();
fn sampled_cat_and_fastcat_preserve_rest_gaps_and_arity_one_continuity();
fn inherited_cycle_maps_preserve_offset_and_complete_partition_identity();
fn ordinary_cat_births_preserve_intrinsic_rest_gap_components();
fn cycle_map_signed_arithmetic_and_cumulative_quota_are_checked();
```
No author Cargo; hold all geometry paths for joined checker after formatting.

### ROOT0226 atomic production checkpoint (fixtures unfinished)
Declaration014 and source015 preceded the cycle context implementation. Exact
forward sampled membership and separate outward whole replay now support
Iterate and authenticated Cat/FastCat children. Arity includes silent children;
unit arity retains continuous full scope. Ordinary single Cat with affine/unit
wrappers uses an exact source/output whole equation to regenerate intrinsic
cycle support; other ordinary joint nonlinear contexts remain unfinished.
Current Rust is scoped-formatted and conceptually coherent, not compile-proven.
New cycle-map fixtures have NOT yet been written or executed. No author Cargo.
All five Rust paths and this plan are held at the requested atomic boundary so
actual staging tests may run without waiting for remaining geometry scope.

### ROOT0284 exact fixture helper declarations
```rust
fn cycle_source(inner: &str, outer: &str, shifted: bool) -> vactr::song::PreparedSong;
fn assert_cycle_rows(song: &mut vactr::song::PreparedSong, window: vactr::pattern::TimeSpan, expected: &[(i64, i64, i64)], configurations: &[(i64, i64, i64)]);
```
Both helpers are test-only in grid.rs. Expected triples are independent exact
(begin,end,denominator) runtime cycle equations; queried rows are sorted by
whole start, preserving complete opaque handles and resolved route identity
across reordered point/fractional partitions. Five fixture signatures above
remain unchanged. Signed arithmetic is local sanity only, not production-helper proof; canonical
root queries filter negative onset; cumulative fuel covers the full actual
inherited resolution pipeline, not independent quota resets. No host playback
claim or author Cargo. Production remains read-only.

### ROOT0284 fixture-only checkpoint
Five declared cycle-map tests are now written, not executed. Direct Cat/rest
uses natural one-cycle notes; inherited grid-to-cycle uses held source and
exact grid starts. Reverse Cat-to-grid uses natural notes to avoid conflating
negative remapped held onsets with source membership. Signed Euclidean cycle
equations are arithmetic sanity only; production signed-helper proof remains
unproven; the actual two-stage route quota test checks
first sufficient cumulative reservation and one-less failure. Full opaque
handles and complete route identities are compared under reordered point and
fractional partitions. Production unchanged; no author Cargo. ReflectCycles,
multiple ordinary nonlinear constraints and broader geometry remain mandatory.

### Reverse Iterate fixture correction after retry004
Immutable retry004 count failure is retained. Subcase diagnostics distinguish
held grid→Iter (four unchanged exact shifted spans) from held Iter→grid. The
latter's inner finite Part canonicalizes the slow64 whole at each cycle and
filters negative remapped onset at residues1/2/3 before outer sampling. A real
inner-song control explicitly asserts its initial whole[0,64) and three empty
cycles; the held reverse asserts only the initial quarter and empty interior
points. A separate one-cycle analog duration8 candidate now tests all four
valid reverse-order quarters and complete reordered fractional/point routes.
This is a source-grounded fixture correction, not a route production repair.
Signed production-helper proof stays unproven; all broader geometry remains
mandatory. No author Cargo; exact two owned files held for mandatory retry.

### ROOT0294 next signed and reflection declarations (documentation only)

Rust remains held. These private declarations refine TASK003 within the same
five Rust paths; they do not claim current implementation or passed coverage.

In `configuration/sampling.rs`, test the real private helpers directly:
```rust
fn signed_cycle_membership_replays_cat_fastcat_and_iterate_under_shared_fuel();
fn reflection_birth_cycle(output_whole: TimeSpan, source_birth: Ratio64, budget: &mut ResolutionBudget) -> Result<i64, Failure>;
fn reflected_component(source: TimeSpan, owner: TimeSpan, cycle: i64, budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>;
pub(super) fn ordinary_reflection_configuration(cover: &FrozenSourceUseCover, identity: &FrozenSourceUseIdentity, trace: &[ProducerStep], source: TimeSpan, output_whole: TimeSpan, source_birth: Ratio64, budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>;
```
The signed test calls actual `cycle_membership`, `cat_birth_context` and
`replay_cycle_component` successively with one ResolutionBudget. Cat cycle-3,
arity2/ordinal1/output member-11/4 maps to input-7/4. Source[-2,-1) replays
configuration[-3,-2), whole start-3. FastCat member-11/8 gives same input and
configuration[-3/2,-1), whole start-3/2. Iterate4 member-3/4 maps to0 using
phase3/4; replay uses the actual iterator helper. Test sufficient and one-less
shared quota, count0/arity1 identity, invalid ordinal and checked overflow.
The old local Euclidean loop remains arithmetic sanity, not this proof.

In `configuration.rs`, replace the existing signature with:
```rust
pub(super) fn map_intrinsic_configuration(cover: &FrozenSourceUseCover, identity: &FrozenSourceUseIdentity, trace: &[ProducerStep], span: TimeSpan, anchor: Ratio64, source_onset: Ratio64, output_whole: Option<TimeSpan>, budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>;
```
`nested.rs` Stage gains `output_whole: Option<TimeSpan>` in its local output
clock. Stage0 starts from immutable `event.whole` translated by the owning
placement offset; do not use event.part/hulls. Affine/preserving operators can
propagate whole endpoints with the same checked local offsets. A sampler
REPLACES the whole and cannot be inverted to the child's held whole. Missing
whole must remain an explicit pending recipe/context case, never fabricate an
end from current query clipping or treat a sampled cell as child whole.

Reflection authenticity: runtime maps [a,b) to [M-b,M-a), M=2c+1. Thus
M=output_whole.end+source_birth authenticates an integral odd mirror; solve
c=(M-1)/2 using checked rational/integer arithmetic, preserving negative c.
Reject inconsistent full whole; original root revision/placement/trace remains
authenticated by the existing matcher. Whole/onset locates the intrinsic
configuration; neither becomes a generation key.

Maximal component algorithm: reflect C intersect each intrinsic cycle, not
the entire uncut C through one mirror. A contiguous C has at most two partial
boundary pieces plus one scalar full-cycle interior envelope. Compute those
three candidates in O(1), charge before storage, intersect owner and merge
only touching pieces with identical admitted source/use/configuration identity.
Return the candidate containing the authenticated birth component; no cycle
scan, duration/repeat expansion, or bridge across fractional rest gaps. Full
C=[0,4) stays[0,4); C=[1/4,1/2) maps[1/2,3/4). C=[1/4,11/4) yields
[0,3/4),[1,2),[9/4,3), with real gaps. A billion-cycle full interior requires
constant work. Zero/reversed support and one-less fuel retain existing codes.

Public fixture declarations in existing `tests/song_source_routes/grid.rs`:
```rust
fn ordinary_rev_preserves_full_policy_and_fractional_rest_components();
fn inherited_sampled_rev_boundary_preserves_authentic_whole_and_membership();
```
First actual candidate: Part drums {fast {s :analog} 4} duration4, transform
`{p -> rev p}`;16 quarter whole rows, full configuration[0,4), complete
opaque handles/routes across reordered fractional/point queries. Fractional
source: Sequence bass duration1/4, drums {fast {s :analog} 4} duration1/4, bass
duration1/2, transform drums Rev. Its sole selected whole/config is[1/2,3/4).
Private reflected-component tests cover the multicycle intervals above and
large interior/quota; no synthetic host-fit claim.

Sampled/inherited reflection declaration refinement:
```rust
#[derive(Clone, Copy)]
enum MembershipSide { At, Before }
#[derive(Clone, Copy)]
struct SourceTimingContext { member: Ratio64, side: MembershipSide, whole: Option<TimeSpan> }
fn reflect_timing_context(context: SourceTimingContext, cycle: i64, budget: &mut ResolutionBudget) -> Result<SourceTimingContext, Failure>;
```
At a grid sample x, Rev point querying actually realizes the FULL child cycle
and then selects reflected wholes containing x. Inverse membership is M-x
with `Before` boundary orientation: a child ending exactly there is eligible;
a child starting there is excluded. This is immutable half-open membership,
not an epsilon float or query-span key. Reflection toggles At/Before; affine
positive maps preserve orientation. Replay BOTH whole endpoints in reverse
order. A grid resets output whole to its own cell but retains distinct child
held identity and membership; it must not overwrite inherited authentic whole.

Genuine next probe: base drums {fast {s :analog} 4} duration4; inner selected
`{p -> rev p}`, outer selected `{p -> euclid p 4 4}`. Output grid starts
0,1/4,1/2,3/4 sample original quarters in reverse order; original source
onsets3/4,1/2,1/4,0 respectively. Test exact boundary and midpoint point
queries against full canonical rows. Reverse operator order Rev over grid
uses actual full cell whole and must preserve reflected selected runs. Include
Sequence offset and held slow64 variant without inventing negative-onset rows.

Current provenance limitation and concrete completion route: root event.whole
is authentic, but each FrozenSourceOrigin/frame carries only pre-outer
`source_part` (possibly clipped), handle onset and entry trace, not pre-outer
whole end. After an outer sampler, this may be insufficient to replay an
inherited Rev. If the genuine probe establishes that gap, the bounded upstream
prerequisite must copy `source_whole: Option<TimeSpan>` from successful
`query_source` event.whole BEFORE outer transforms into SongEventOrigin and
each frozen origin/frame, sharing existing depth/copy budgets. This is an
exact proposed immutable field, not permission to edit upstream paths or a
permanent Unsupported exemption. The own Stage context then borrows that
full span, translates its owning local offset and uses the typed orientation
above. Root must authorize the prerequisite separately after actual evidence.
Rev, SampleCycles, Subdivide, Restructure, Slices, stochastic conditionals and
remaining joint nonlinear combinations remain required before TASK003 clears.

### ROOT0296 reflection input refinement before Rust
```rust
pub(super) struct IntrinsicConfigurationInput { pub span: TimeSpan, pub anchor: Ratio64, pub source_onset: Ratio64, pub output_whole: Option<TimeSpan> }
pub(super) fn map_intrinsic_configuration(cover: &FrozenSourceUseCover, identity: &FrozenSourceUseIdentity, trace: &[ProducerStep], input: IntrinsicConfigurationInput, budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>;
fn reflected_component_boundaries_and_fuel_are_checked();
```
Typed grouping supersedes the earlier eight-argument proposed mapper and
preserves all clocks explicitly without a lint exemption. Root-stage whole is
translated from event.whole; deeper output whole stays None until an authentic
inverse or copied provenance exists. This increment supports one Rev with
affine/preserving and unit-layout wrappers, not arbitrary joint nonlinear
reflection. Boundary solver chooses the regenerated connected component that
intersects the authenticated mirror cycle; source/owner endpoints remain
half-open. The sampled probe retains an actual expected positive result so
missing context becomes a recorded failure, not a silently passing limitation.

### ROOT0296 ordinary reflection source checkpoint
Typed intrinsic input and authentic root-stage output whole are wired. The
ordinary one-Rev path reconstructs its mirror from full emitted whole end and
authenticated source birth, regenerates constant-work boundary/interior
components, replays affine wrappers and preserves full-policy shared state.
Private tests now CALL actual signed cycle_membership/cat_birth_context/
replay_cycle_component with one budget, including one-less rejection. Private
reflection tests check fractional multicycle gaps, negative mirrors, billion
interior constant work and arithmetic errors. Public ordinary tests assert
16 reflected quarters share[0,4), and fractional source support mirrors to
[1/2,3/4), with full route/handle partition equality. None is executed yet.
The genuine outer-grid/inherited-Rev probe asserts16 actual output quarters,
reversed authenticated original quarter onsets and positive complete routing
at exact boundaries. The current sampled locator still lacks ReflectCycles;
this probe is deliberately retained to expose actual missing context/behavior
without inventing inherited whole ends or blessing an Unsupported fallback.
All touched files below1000; sampling994 leaves only6 lines: any larger
next increment needs a prior cohesive split proposal. No author Cargo.


### Authentic sampled reflection consumer amendment — document only

This declaration supersedes earlier Ratio64-only sampled membership and deeper
output_whole=None proposals for the next source wave. Existing historical source
and failed test receipts remain intact; no Rust is changed by this amendment.
Actual007 source-route38/39 passed; the positive inherited sampled Rev fixture
failed `sampled context requires joint mapping geometry`. Preserve that positive
fixture and its exact reversed source quarter expectations without ignore/waiver.
Evidence: SONG-09/0031-source-whole-authority-preflight.md and actual007 raw log.

Source prerequisite: independently clear source-whole authority before this
consumer's serial source release. Its live/frozen privately issued handle+whole
pair and borrowed matcher validation reject public handle swaps. Metadata alone
does not implement reflection. source-whole owns nested.rs/components.rs until
held independent clearance; their observations here are not stable write baselines.

#### Cohesive sixth-path split before growth

At current sampling994, relocate existing reflection_birth_cycle,
reflected_component and ordinary_reflection_configuration (current643–824), plus
only reflected_component_boundaries_and_fuel_are_checked (current936–993), into
`configuration/sampling/reflection.rs`. Keep actual signed CycleMembership tests
and their existing helpers in sampling.rs; no algorithm, quota or assertion changes
in the extraction. Project sampling~757 and reflection~244 before new consumers.

Declare `mod reflection` in sampling.rs and preserve the configuration caller by
`pub(super) use reflection::ordinary_reflection_configuration`. Child function
must be `pub(in crate::song::routing::configuration)` so that facade does not widen
its original sampling pub(super) visibility. New typed membership shared with
nested uses `pub(in crate::song::routing)`. Child `use super::*` accesses checked
budget, layout and recipe types; no unrelated root module edits.

Relocated exact signatures:
```rust
fn reflection_birth_cycle(output_whole: TimeSpan, source_birth: Ratio64, budget: &mut ResolutionBudget) -> Result<i64, Failure>;
fn reflected_component(source: TimeSpan, owner: TimeSpan, cycle: i64, budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>;
pub(in crate::song::routing::configuration) fn ordinary_reflection_configuration(cover: &FrozenSourceUseCover, identity: &FrozenSourceUseIdentity, trace: &[ProducerStep], source: TimeSpan, output_whole: TimeSpan, source_birth: Ratio64, budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>;
fn reflected_component_boundaries_and_fuel_are_checked();
```

#### Typed clocks and authenticated input

Use immutable source_whole() only after that same outer/frame origin has passed
its full private issued identity matcher. No copied scalar from another frame and
no duration inferred from source_part, caller part/hull or final sampler cell.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::song::routing) enum MembershipSide { At, Before }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::song::routing) struct SourceMembership {
    pub point: Ratio64,
    pub side: MembershipSide,
}
#[derive(Clone, Copy)]
pub(in crate::song::routing) struct SourceTimingContext {
    pub incoming_member: Option<SourceMembership>,
    pub output_whole: Option<TimeSpan>,
    pub source_whole: Option<TimeSpan>,
}
pub(in crate::song::routing) enum SourceLocatorMapping {
    UnchangedBirth,
    SampledPoint(SourceMembership),
    NeedsJointGeometry,
}
pub(in crate::song::routing) fn map_source_locator(
    cover: &FrozenSourceUseCover, identity: &FrozenSourceUseIdentity,
    trace: &[ProducerStep], output_birth: Ratio64,
    timing: SourceTimingContext, budget: &mut ResolutionBudget,
) -> Result<SourceLocatorMapping, Failure>;
```

Actual SampledRecipeInput retains output_birth/source_birth/source_scope/
output_owner, changes incoming_member to Option<SourceMembership>, and adds
output_whole/source_whole Option<TimeSpan>. Actual SampledStep retains its borrowed
node/edge/trace, ordinal, slot cycle, CycleMembership and owner; output_member and
input_member become SourceMembership. Add CycleMembership::Reflect { cycle: i64 }.
No separate per-event mutable history, approximate epsilon or opaque ID lookup.

Stage retains input_root/policy_depth/handle/cover/identity/trace/instrument/scope/
output_onset/output_offset/input_onset. Refine sampled_locator to
Option<SourceMembership> and add source_whole: Option<TimeSpan>, retaining its
existing output_whole: Option<TimeSpan>. Root-stage output whole is event.whole
translated by root placement offset, as already implemented. Stage i's source_whole
is its own authenticated origin getter in the selected-root clock. Stage i+1 output
whole is stage i source_whole translated by stage i scope.offset into that deeper
payload's local output clock. Translate BOTH endpoints and membership point; preserve
side. Do not mistakenly subtract the following stage offset or final root offset.

IntrinsicConfigurationInput preserves existing span/anchor/source_onset/output_whole
and adds source_whole Option<TimeSpan> if its ordinary dispatch needs it; declare
that exact call-site refinement before implementation within configuration/nested.

#### Checked reflected membership and whole replay

```rust
fn membership_contains(span: TimeSpan, member: SourceMembership) -> bool;
fn membership_cycle(member: SourceMembership) -> Result<i64, Failure>;
fn quantized_membership(member: SourceMembership, divisions: i64) -> Result<SourceMembership, Failure>;
fn inverse_slot_membership(edge: &FrozenSourceUseEdge, cycle: i64, trace: &[ProducerStep], member: SourceMembership, budget: &mut ResolutionBudget) -> Result<SourceMembership, Failure>;
pub(in crate::song::routing::configuration::sampling) fn reflect_membership(member: SourceMembership, cycle: i64, budget: &mut ResolutionBudget) -> Result<SourceMembership, Failure>;
pub(in crate::song::routing::configuration::sampling) fn reflect_whole(whole: TimeSpan, cycle: i64, budget: &mut ResolutionBudget) -> Result<TimeSpan, Failure>;
```

For positive span[a,b), At means a<=point<b; Before means a<point<=b.
For actual zero-width support, retain explicit exact point membership. Reflected
At(x) maps to Before(M-x), and Before(x) maps to At(M-x), M=2c+1 checked.
A Before point on integral boundaryk uses cyclek-1; otherwise floor(point).
Positive affine inverse transforms point and both endpoints, preserving side.
Weighted slot inverse uses actual normalized intrinsic slot/copy layout and typed
trace; Before at slot.end selects that slot, while At there excludes it. Do not
route Before through the current At-only map_slot_support(point) end exclusion.

A grid selecting a Before membership at a cell boundary chooses the preceding
quantized timing cell; the enabled mask still authenticates that exact cell.
Its actual child sampleSTART resets membership to At. This is genuine sampling,
not carrying a reflected boundary orientation into a new point sample. Replay
replaces output whole by the emitted grid cell but keeps the separately issued
source whole. All existing grid/Iter/Cat/static conditions retain exact masks,
whole anchors and original source birth semantics.

For sampled Rev, collect ordered inverse mapping context using the actual query
piece membership cycle, including At/Before. Replay the privately authenticated
inner whole outward through each operation: Rev maps[a,b) to[M-b,M-a), retaining
both endpoints; grid creates its own actual cell whole. Where actual output and
source wholes coexist, verify both mirror equations M=output.begin+source.end and
M=output.end+source.begin. Never reconstruct source whole from a note's clippedpart.
Ordinary no-sampler paths retain existing authenticated original-birth continuity;
do not switch every continuation to caller point or fabricate new generations.

#### Maximality and shared work

Collect all inherited stage predicates, grid enabled masks, slot/cycle contexts and
full source bounds before selecting a component. At the authenticated oriented
member, intersect exact constraints; use the existing constant-work reflected
partial-boundaries/full-interior recipe, not a whole-score cycle scan. Merge only
touching supports with the same complete admitted source/use/placement identity;
re-evaluate orientation/copy context across boundaries. Missing mandatory whole is
an addressed retained error, not metadata guessed from a cell or permanent waiver.

Charge actual steps/contexts/endpoints, checked mirror/cycle arithmetic and any
candidate storage before Vec growth, using the same caller ResolutionBudget through
all inherited stages, inverse and replay. No local budget reset, additional default
fuel, query/certifier/VM call, duration or finite-repeat expansion. Existing direct
signed helper fuel/negative-cycle/overflow fixtures remain actual production proof.

#### Required actual source wave fixtures

```rust
fn nonaligned_sampled_rev_uses_authentic_pre_grid_whole();
fn sampled_rev_long_release_preserves_halfopen_membership_and_ownership();
fn sampled_rev_sequence_offsets_and_crossings_preserve_full_routes();
fn reflected_membership_boundaries_and_shared_work_are_exact();
```

- Existing aligned fast4 source→selected Rev→selected Euclid4/4 positive probe:
  sixteen actual quarter cells, source onsets3/4,1/2,1/4,0 in each first-cycle
  reflection, exact grid boundary/midpoint point queries, full handle and complete
  route equality under full/fractional/disjoint/reordered partitions.
- Genuine fast3 source/Rev/grid4: cell[0,1/4) samples authentic pre-grid whole
  [0,1/3), original whole[2/3,1); assert the unequal ends and correct reverse
  membership, not coincident quarter reconstruction. Compare source getter/handle
  and final resolved maximal policy for all actual cells and partitions.
- Long whole slow2 with finite duration4: source whole[0,2), cycle1 reflected
  whole[1,3) sampled to quarter timing, including exact reflected onset/end,
  crossing full source cycle boundaries and releases beyond owner when admitted.
  Assert real canonical rows/finite negative-onset filtering; do not shrink valid
  expected counts blindly or demand excluded negative-onset rows.
- Sequence prefix/fractional source policy and rest gaps, outer affine and reversed
  grid order; exact full routes/issued handles, intrinsic local offset translation
  and unchanged original configuration ownership. At/Before source and cell endpoint
  checks CALL real reflection helpers with shared just-enough/one-less budgets.
- Preserve prior source-route/sampling/matcher/ordinaryRev/actual signed production
  helpers and all failed raw receipts, especially actual007 sampled Rev failure.

#### Consumer completion and handoff

TASK003 remains In Progress. The source-whole dependency and six-path consumer must
both independently clear before sampled reflection criteria can pass. Finish the
cohesive split, actual borrowed provenance clocks, oriented joint inverse/replay,
positive probe and new nonaligned/held/point/offset fixtures, then mandatory stable
joined native/browser/strict Clippy/inventories/unfiltered tests/nextest/format/hash
checks. No Rust/Cargo authorization exists in this document amendment.

ReflectCycles combinations, SampleCycles, Subdivide, Restructure, Slices, stochastic
conditions and remaining joint nonlinear mappings remain mandatory beyond this wave;
no full geometry/08B/09/host/playback/export claim follows a focused sampledRev pass.

### Consumer implementation — ROOT0337

0040 preserves fresh six-path source baselines before cohesive reflection extraction. Authentic issued whole metadata is independently verified; typed oriented sampled membership and each actual inherited clock are next. No author Cargo or full geometry claim.

0041 source declaration refinement: existing private `cycle_membership(..., member: SourceMembership, ...)` and `replay_cycle_component(..., member: SourceMembership, ...)` carry orientation; old signed helper fixtures/ordinary Cat wrap `At`. Existing IntrinsicConfigurationInput stays unchanged because ordinary dispatch consumes its corrected own-clock output_whole; sampled input adds separately authenticated source_whole. No quota reset, extra path or unmetered reflected scans.

0042 private declarations: `replay_iterate_membership(source: TimeSpan, count: u32, owner: TimeSpan, member: SourceMembership, budget: &mut ResolutionBudget) -> Result<TimeSpan, Failure>` and `replay_periodic_membership(condition: C, transformed: bool, span: TimeSpan, owner: TimeSpan, member: SourceMembership, budget: &mut ResolutionBudget) -> Result<Option<TimeSpan>, Failure>` preserve ordinary At replay and use exact Before membership without epsilon. `reflected_component_for_member(source: TimeSpan, owner: TimeSpan, cycle: i64, member: SourceMembership, budget: &mut ResolutionBudget) -> Result<TimeSpan, Failure>` selects the actual reflected component. Precharge eight logical step fields before Vec admission and four units per authentic whole endpoint transform, with the same caller budget; Iterate enumerates at most five candidates. Genuine boundary/fast3/slow2/offset fixtures remain pending executed verification.

0043 ROOT0337 coherent sampled reflection consumer is written and scoped-formatted. Six Rust files remain below1000: configuration793, nested747, sampling907, reflection678, parent source routes855, grid782. Authentic own-frame whole endpoints and previous-scope offsets now travel with exact At/Before membership; reflection maps both endpoints, weighted slots preserve orientation and sampler quantization returns enabled cellSTART membership. Shared caller budget admits all queued step fields and authentic endpoint transformations before work. Three new public route fixtures cover fast3/grid4, slow2 release crossings, prefix/fractional offsets and affine/weighted wrappers; three new private boundary fixtures cover negative integral Before cycles, slot-end quantization, endpoint quota and periodic/Iter replay. The original aligned sampledRev positive and all existing route expectations remain. No Cargo was executed by the author. These source/fixture changes are HELD pending mandatory checker; no passing or full geometry/playback claim.

0045 ROOT0345: mandatory checker original51477 terminal101 (poll e4b509) found native E0382 at nested421 before any behavioral tests. Raw evidence /tmp/vactr-sampled-reflection-independent-001/final-results.json SHA e83c39c0450c800545df5b6d162268d4214a900da114ee56faa644cbc249d28f. The current scope was moved into Stage before whole endpoint translation. Use the already copied `next_offset` for both endpoint subtractions; the exact current-stage clock is unchanged, with no scope clone or assertion changes. Scoped format and fresh held receipt follow; mandatory verification remains pending. No Cargo by author.

0047 ROOT0348: checker retry002 original3318 terminal101 passed native/wasm checks but Clippy test compilation found missing test-only SongLimits and FailCode imports (14 errors), before behavioral tests. Evidence /tmp/vactr-sampled-reflection-independent-002/clippy.log retained. Add actual crate imports only in reflection private test module; all assertions and production behavior unchanged. Scoped format and new held receipt; no author Cargo and no acceptance claim.

0049 ROOT0351: checker003 original24826 terminal101 (poll1c48c5): native/wasm/strict Clippy0, both privacy commands0, all42 public routes passed; private reflection3 passed and1 failed. Full evidence /tmp/vactr-sampled-reflection-independent-003/final-results.json SHA55daebeb664818bf1a6c512301206d265af19a92ee8dca03062261d1d7826bc2. Private owner[-4,4), source[0,1), Iterate2 includes c=-1 shift1/2 giving[-1/2,0) and c=0 giving[0,1); maximal joined support is[-1/2,1). Correct only that expected interval, explicitly verify Before1/At(-1/2) membership and At1 exclusion. Production unchanged; negative owner and original quota/periodic/At1 rejection retained. All42 genuine public expectations unchanged. Remaining gates and repaired private fixture await mandatory checker; no full geometry claim or author Cargo.

Focused sampled reflection consumer independently accepted in ROOT0355: 833 distinct binary-qualified fixtures and46 nextest executions, original processes terminal and exact current source hashes verified. Evidence combines retained003 native/wasm/privacy/public42 successes with repaired private tests and all remaining fresh regressions. This proves the bounded ROOT0337 source-whole/oriented sampledRev wave only. Broader geometry remains In Progress; remaining transform combinations, stochastic mappings and full08/09/host/playback/export integration remain mandatory. No geometry Rust changed for this documentation entry.

### 2026-10-02 — verified timing capture consumer handoff
ROOT0461 verifies opaque timing capture and stack-safe shared budgets: 54 gates,
269 distinct tests, with 12 focused nextest passes already included in that count.
Exact accepted eight-path source hashes and evidence are retained in
`tmp/song-mode-riela/ROOT0465-index-timing-consumer-handoff.json`.
[Authenticated Index occupancy](song-mode-index-occupancy.md), released ROOT0464,
now owns output-payload binding, connected Index support and finite opening bounds.
Subject Nil suppresses notes without creating instrument/effect configuration births.
Dynamic realization, host readiness, scheduler, Apply/mute and export remain required.
