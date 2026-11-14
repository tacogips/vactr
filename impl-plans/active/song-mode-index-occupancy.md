# Authenticated song Index occupancy implementation plan

**Status**: In Progress
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose

Consume the verified lossless timing recipe to authenticate Slice operands,
derive connected Index applicability, and price configuration openings. This
is a dependency of complete song preparation and playback. Static consumption
is one implementation wave; dynamic canonical realization remains required
for the full feature. Do not silently substitute guessed bounds for unresolved
operands or reject all dynamic arrangements as the final song-mode contract.

## Related plans

- **Previous**: [Index timing operands](song-mode-index-timing-operands.md), accepted ROOT0461.
- **Depends On**: [Route geometry](song-mode-route-geometry.md), [Route preparation](song-mode-route-preparation.md).
- **Next**: Shared canonical dynamic realization, host preparation and transport.
- Exact prior evidence: `tmp/song-mode-riela/ROOT0461-index-timing-scoped-acceptance.json`.
- Matrix: `/tmp/vactr-index-timing-independent-005/final-results.json`, SHA256 `f68869cf25f33ce3aa436d6ecf5c3f3b7f21c3720297972f11999851b3d479c1`.
- Focused nextest: `/tmp/vactr-index-timing-nextest-independent-001/final-results.json`, SHA256 `3ec245d0d69e3f4e261a73f00ae445442166260721ccf77ffd12252a9f3fb610`.
- Prior scope proves capture and shared budgets, not occupancy or full playback.

## Exact Rust manifest

| Module | Path | Deliverable | Status |
|---|---|---|---|
| Routing registry | `src/song/routing.rs` | Register operand consumer | In Progress |
| Operand authority | `src/song/routing/index.rs` (new) | Borrowed authenticated binding and support admission | In Progress |
| Density entry | `src/song/routing/density.rs` | Cohesive walker extraction before growth | In Progress |
| Density consumer | `src/song/routing/density/index.rs` (new) | Walker and configuration opening bounds | In Progress |
| Configuration entry | `src/song/routing/configuration.rs` | Authenticated Slice dispatch | In Progress |
| Component consumer | `src/song/routing/configuration/index.rs` (new) | Connected applicability and replay | In Progress |
| Inherited stages | `src/song/routing/nested.rs` | Retain each output payload and issued frame timings | In Progress |
| Genuine fixtures | `tests/song_index_occupancy.rs` (new) | Candidate/query/resolve/admission witnesses | In Progress |

Eight paths only. `prepare.rs` and `components.rs` remain existing callers.
Any additional Rust path requires a root manifest amendment before modification.
Every touched Rust source must remain below 1000 lines; density starts at 991
and requires extraction. Cohesive splits must fit this manifest or be amended.
No dependency, Git, unrelated editor, or other plan writes by the coding agent.

## Module contracts

The following declarations are opaque consumer contracts. Concrete field and
signature refinements must be grounded in existing clocks, prepared authority
and Failure types and reported before implementation if they alter semantics.

```rust
pub(in crate::song::routing) struct BoundSliceOperands<'a>;
pub(in crate::song::routing) struct StaticIndexSupport;
pub(in crate::song::routing) struct IndexOpeningBound;
pub(in crate::song::routing) struct IndexRealizationRequest;
pub(in crate::song::routing) enum IndexSupportAdmission {
    Static(StaticIndexSupport),
    RequiresRealization(IndexRealizationRequest),
}
```

- Authority begins at the original prepared output payload recipe root, with
  revision, track, complete operand path and producer fragments. Validate Slice
  structure, issuer, issued prefix, index trace, whole and subject handle.
  NodeId alone is insufficient. Virtual Off edges are not operand ordinals.
- Inherited stages retain the current output payload and frame-issued Slice
  timing before switching to the inner subject payload. Scope.payload alone
  cannot authenticate the outer Slice.
- Index structure uses actual index whole and subject sampling anchor. Derive
  maximal connected source applicability in the common root clock, preserving
  complete configuration identity across neighboring occupied cells.
- Subject structure keeps subject-owner support/births: Nil suppresses a note,
  not the persistent instrument/effect configuration.
- Cut values, note values and output onset are not FX configuration identity.
  A convex hull across gaps cannot substitute for a connected component.
- Support and opening bounds preserve scalar/rest distinction, weighted slots,
  zero-copy slots, symbolic repetition, checked rational affine/reflection maps
  and actual retained producer fragments. No repeat or whole-score expansion.
- Bounds cover instantaneous ownership K, uniform openings B(W), and finite N;
  enforce birth-generation representability, never divide demand by pool slots.
- All consumers charge the existing cumulative remaining-work and depth budget
  before scans/allocations. Shared DAGs must be stack-safe under normal stack.
- Dynamic Function/Native/Late/Thunk/Signal become addressed realization
  requirements tied to original authority. Do not execute VM callbacks here,
  replay RNG per route event, fabricate constant support, or treat an event cap
  as a proof of finite configuration births.

## Dependencies

| Task | Depends On | Parallelizable |
|---|---|---|
| TASK-001 | Accepted capture and root release | No |
| TASK-002 | TASK-001 binding and support | No |
| TASK-003 | TASK-002 consumer implementation | No |
| TASK-004 | Fixed source and independent verification | No |

## Tasks

### TASK-001: Authenticated operands and symbolic support
**Status**: In Progress

- [ ] Extract density walker before growth without changing unrelated behavior.
- [ ] Bind original payload, root, complete operand path and actual query timing.
- [ ] Implement symbolic static support and addressed dynamic requirement.
- [ ] Reject foreign payload/path/timing substitutions under existing budget.

### TASK-002: Configuration and finite bounds consumers
**Status**: Not Started

- [ ] Retain correct output authority through inherited Slice frames.
- [ ] Resolve maximal connected Index applicability and preserve Subject owner.
- [ ] Feed sound K/B/N opening bounds into existing route preparation.
- [ ] Preserve all actual clocks, identity and symbolic repeat semantics.

### TASK-003: Genuine candidate and regression witnesses
**Status**: Not Started

- [ ] Actual scalar/rest-order gap and adjacent occupancy witnesses.
- [ ] Weighted, zero and large symbolic repeat witnesses.
- [ ] Actual affine/Rev and inherited Slice stages.
- [ ] Fractional and reordered query agreement and long source continuations.
- [ ] Subject Nil does not create births or reset private FX configuration.
- [ ] Foreign payload/path/timing rejection and explicit dynamic requirement.
- [ ] Shared quota, normal-stack and prior configuration regressions.

### TASK-004: Independent verification and full-goal handoff
**Status**: Not Started

- [ ] Fixed source hashes and exact changed-file manifest delivered.
- [ ] Mandatory checker verifies native/browser compilation, strict Clippy,
  genuine nonempty suite inventory/run agreement and relevant regressions.
- [ ] Scoped formatting, line limits and diff checks pass.
- [ ] Any nextest invocation is unique and executed at most once.
- [ ] Reconcile geometry/preparation with exact evidence and remaining dynamic,
  host ownership, scheduler, Apply/mute, playback and export requirements.

## Completion criteria

- [ ] All eight declared paths implemented with authenticated actual consumers.
- [ ] Connected support and K/B/N match genuine selected-source behavior.
- [ ] Subject note suppression cannot invent configuration generations.
- [ ] Static support remains symbolic and shared-budget/stack-safe.
- [ ] Dynamic requirements remain explicit without narrowing full song scope.
- [ ] Independent mandatory checks pass and evidence scope is reported exactly.
- [ ] Parent plans receive accepted consumer results and remaining obligations.

## Progress log

### 2026-10-02 — source-grounded consumer audit and release
Read-only Rust audit found density skipping Slice index edges, insufficient for
Index structure. Existing source scope names the inner subject, so inherited
stages must retain output authority separately. Root documented the eight-path
wave before Rust writes. Previous goal turn was a status inspection with no
implementation change; this turn creates the next executable dependency plan.
No occupancy or full playback acceptance is claimed by this release.

### Implementation declaration checkpoint

ROOT0464 baselines were verified before source writes. DensityWalk is moved
unchanged to the owned density/index child before parent growth; only node
visibility changes to pub(super) for its existing entry caller. The static
classification inspects the bound Index subtree, not whole-recipe
requires_realization: a selected SongSource subject is itself represented as
UnresolvedPattern and must not poison otherwise static Index admission.

Private BoundSliceOperands retains the original output payload, output Part
revision and track, full root-to-issuer producer match, the actual Slice node,
its Index child, and borrowed issued timing. Binding starts from the immutable
prepared output root and compares complete trace fragments, including symbolic
Copies ranges. No NodeId-only lookup or graph-edge ordinal substitution.
Consumers continue caller work/depth; addressed unresolved results apply only
to their actual bound operand. No compile or behavior evidence yet.

### Output authority construction refinement

`output_operand_owner(&FrozenRoutingInventory, scope_part: usize, track: KwId)
-> Result<OutputOperandOwner<'_>, Failure>` privately derives payload and output
revision from the actual topology part and scope_payload lookup. It does not
borrow the selected subject revision or permit a caller-asserted owner tuple.
`bind_slice_operands(owner, &FrozenSliceTiming, &EventHandle, depth: u32,
&mut ResolutionBudget) -> Result<BoundSliceOperands<'_>, Failure>` compares
the issued original subject handle against the previously authenticated frame
handle; selected policy track and outer output track remain distinct authorities.
The actual root/path traversal is stack-based and charges each requested entry
and producer fragment inspection. First-structure checks use original subject
and index child flags, with no NodeId-only binding. Support admission and
consumer hookup remain in progress, not a verification/readiness claim.

### Concrete binding/support increment (not source ready)

Density traversal has been extracted before growth. The operand consumer now
contains original-root trace matching, private topology-derived output owner
construction and first-structure/issued-index-path checks. Owner construction
accepts the existing ResolutionBudget and precharges the Capture entry lookup.
A dynamic leaf whose evaluated Pattern adds uncaptured producer descendants
produces a borrowed unresolved request retaining owner scope, original payload,
recipe, issued timing, complete traces and subject handle; it is not a static
path-validation success. Request descriptors alone are not canonical authority.

Postorder Index-only support admission is being implemented without VM calls.
The preliminary summary retains empty/full distinction, symbolic copies and
checked rate/opening arithmetic. Static Pattern-valued parameters and unsupported
operations currently remain unresolved; this is an implementation gap, not final
rejection of those arrangements. The shared-alias caller-height proof, exact
whole replay, maximal-component solver, inherited Stage hookup, density K/B/N
consumption and genuine executable fixtures remain unfinished. No source-ready
seal or checker execution is authorized by this checkpoint. All original task
and completion checkboxes remain unchecked; no Cargo has run.

### Prepared/event authority and authentic whole applicability refinement

ROOT0472 confirms Index applicability is the connected union of authentic whole
footprints for one complete immutable source/use/configuration identity, clipped
only to the common-root output owner. Weighted Slow footprints [0,1) and
[1/2,3/2) therefore unite to [0,3/2), even though their emitted parts have a gap.
Different identities remain separate; no hull may bridge a genuine whole gap.

Private `PreparedSliceAddress<'a>` retains the topology-derived output owner,
original recipe/root, Slice and Index roots, and complete borrowed symbolic
operand prefix. `prepare_slice_operands(owner, issuer, prefix, depth, budget)`
resolves that prefix against actual recipe child role traces under the caller's
remaining work/depth; virtual source-use nodes cannot substitute ordinal paths.
`BoundSliceOperands` strengthens this prepared address with actual issued timing
and subject identity. `IndexRealizationRequest` retains the prepared address and
optional genuine issuance; density requests do not manufacture timing.

`output_operand_owner<'a>` explicitly ties its returned lifetime to the inventory.
Static reachability scans revalidate every incoming alias depth, without resetting
the shared budget. Whole replay, maximal components and sound long-whole K/B/N
remain implementation obligations, with genuine weighted Slow union evidence.

### Bounded weighted Slow replay helper

Owned `configuration/index.rs` declares
`weighted_slow_whole_cycle(prefix: Ratio64, width: Ratio64, slow: Ratio64,
whole: TimeSpan, budget: &mut ResolutionBudget) -> Result<Option<i64>, Failure>`.
For positive width <=1 and integral positive Slow, q=whole.begin-prefix,
L=width*slow, d=1-width. The exact original-cycle condition is
c in [q,q+L) and q-d*c in L*Z, with whole length exactly L. Checked rational
denominator clearing and Euclidean congruence solve the finite progression
without enumeration. The width=1 case checks q/L integral. Each Euclidean
iteration debits the caller budget. This proves replay only; it cannot alone
authenticate owner membership, join different identities, or admit K/B/N.

Prepared-only `admit_prepared_index_support(&PreparedSliceAddress, depth,
&mut ResolutionBudget)` traverses exactly the original Index subtree and returns
requests with no issuance. The event-bound admission delegates to the same
implementation after genuine trace checks, retaining its actual timing. Each
incoming alias path consumes work/depth; allocations remain precharged. Subject
handle comparison charges both actual handle lengths before equality inspection.

The weighted Slow helper now supports positive rational Slow using runtime
child query overlap, superseding the integer-only interval: c lies strictly
between q-width and q+L. Integer ranks are floor(q-width)+1 through ceil(q+L)-1.
The same congruence gives exact integer child cycle k; replay uses that k rather
than floor(c/slow). Final issued part/query and owner eligibility remain separate.
This includes the genuine fractional crossing n=3/2,w=1/2,c=1,k=1,T=5/4.

### Intrinsic alias caller-depth accounting

Private `operand_heights(recipe, budget) -> Result<Vec<u32>, Failure>` prices
and computes structural postorder heights, including child, leaf Pattern, nested
AtomicList and all parameter/slot weight/count references. Small explicit-stack
`operand_leaf_height` and `operand_parameter_height` helpers charge every visited
value/reference and stack request before allocation. This does not classify the
whole recipe as dynamic or execute callbacks. Each reachable alias checks
caller depth plus its intrinsic height before memoized inspection is skipped;
height never uses global peak or another incoming path's caller depth.

Replay denominator clearing uses budgeted checked gcd/LCM, superseding the
product-of-denominators implementation. The modular inverse and target are
reduced before a bounded binary modular product, charging every iteration;
modular addition avoids intermediate overflow. Private arithmetic witnesses
cover fractional crossing, negative cycles, width 1/10^15 and exact work
one-less refusal. These are arithmetic witnesses, not genuine route admission.

Genuine public fixture `weighted_slow_index_unites_authentic_wholes_without_emitting_in_part_gap`
uses an actual selected Part and eager named Slice transform. It asserts runtime
whole [0,1), [1/2,3/2) versus clipped emitted parts [0,1/2), [1,3/2),
requires authentic origin, and requires both resolved configurations [0,3/2).
An actual fractional gap query emits no notes, while reordered partial queries
must resolve the same full component. This fixture is written before consumers
are complete and is not a passing/readiness claim.

### Inherited stage authority retention

`Stage` retains `output_owner: OutputOperandOwner<'a>` and
`slice_timings: &'a [FrozenSliceTiming]` from the actual current output frame
before the inner selected payload switch. The next output owner scope/track
comes from the authenticated source_scope index and original source handle.
Private `OutputOperandOwner::payload()` exposes only the borrowed original
payload for exact pointer consistency checking inside routing. No subject
revision or caller-supplied tuple substitutes for output topology membership.

### Sound ownership opening bound refinement

`IndexNodeSupport` separately retains maximum authentic whole length H and
uniform index-whole starts E(W)=ceil(event_slope*W)+event_burst. It never
substitutes emitted-part width or max slot multiplicity for these fields.
At a point, each applicable footprint started in the preceding H interval, so
K <= E(H) times sampled-subject instantaneous ownership. Finite N is bounded
by E(output-owner width+H) times that ownership. Component openings in any W
are bounded by E(W) times the ownership; joining only reduces these counts.
All clocks/copies/rates use checked arithmetic and cumulative charged metadata
inspection. Existing caller occurrence scaling is retained once. This is a
conservative bound, separate from exact connected-union identity resolution.

### Actual density consumer entry

Existing `DensityWalk::node` remains the facade. Private `node_path` retains the
complete actual source-edge symbolic producer prefix (including empty virtual
edges); extensions are precharged before storage and restored after traversal.
`index_opening` locates the original payload by exact pointer in the actual
topology Capture/edit ownership, precharging each scan. It constructs the
prepared Slice address and consumes static subtree support before any query.
The ResolutionBudget receives only the existing remaining counter and publishes
its actual post-work counter even on failure. Static start bounds price K/B/N
with the same-clock whole horizon; unresolved requests preserve original
prepared authority and never manufacture an issued timing. Subject mode
continues the existing bound unchanged. Exact configuration continuity remains
a separate consumer proof; event counts are conservative opening upper bounds.

The uniform opening burst includes the owner-clipped head term
E(H)*K_subject in addition to event_burst*K_subject. Whole starts can precede
the output owner while their footprints create multiple physical ownership
heads at owner.begin. This bound applies independently to arbitrary windows;
K+B masking is not a proof of standalone B. The finite N cap is retained.

### Subject sampled-start lookback correction

`DensityWalk::index_support` authenticates and inspects H before traversing the
subject. Index subject density receives [output_owner.begin-H, output_owner.end)
in the Slice clock, then the existing checked subject transformations and Part
clipping apply. It must not return K_subject=0 merely because a valid sampled
source configuration precedes the output owner while its authentic Index whole
continues into it. `index_opening(subject, support, owner)` applies K/B/N to the
original output interval, not the expanded subject lookup interval.

### First exact issued component consumer

`configuration::index_configuration(&BoundSliceOperands, source: TimeSpan,
owner: TimeSpan, depth, budget) -> Result<TimeSpan, Failure>` replays actual
weighted scalar Slow wholes, validates source eligibility at the issued START
and computes a maximal connected footprint. Uniform scalar Slow uses exact
quantized eligible child cycles. A weighted slot with fully eligible footprints
has parent-cycle union endpoints p+d*c+L*floor(c/n),
p+d*c+L*ceil((c+1)/n). Connected periodic blocks have period a=n.num() and
span [a*j+p,a*j+p+a-1+w); separated parent unions use exact eligible k ranks.
A genuine issued whole covering the complete owner-clipped block proves that
block without scanning cycles. Partial source filtering across multiple parent
unions requires an addressed joint constraint, not a hull or clipping shortcut.

`index_stage_configuration` consumes the retained output owner/timing for an
authenticated direct selected-source Slice frame. More composed Stage cases
remain explicitly unresolved until their joint clock and identity proof is
implemented; no full geometry completion or final static-only endstate follows.
Existing sampled-grid and non-Slice route consumers are retained.

### Issuing-owner parent cycle refinement

Replay now also accepts `issuing_owner: Option<TimeSpan>` and intersects the
solution progression with c in (owner.begin-prefix-width, owner.end-prefix).
The global earliest congruence solution is not an owner eligibility proof.
Connected periodic unions first intersect their parent-cycle block with these
actual issuing-parent ranks, then derive authentic first/last whole boundaries.
For width=1, all admitted parent groups touch; source-eligible k ranks and
owner-eligible parent ranks jointly bound the exact continuous whole union.
Source START membership remains a separate authenticated clock constraint.

`index_stage_configuration` is wired before existing sampled/periodic geometry.
It follows actual authenticated cover edges and builds their symbolic prefix
under the same budget, matching the complete issued prefix before binding.
Direct single-frame Slice→Source dispatch consumes `index_configuration`;
composed frame clocks not yet jointly solved retain the actual prepared/issued
authority in a realization requirement. Existing no-timing ordinary/Subject
paths continue to use the original geometry. This is an implemented direct
consumer milestone, not completion of all inherited/composed criteria.

### First coherently wired direct checkpoint — verification pending

The direct Slice Index→selected Source path now uses retained output/frame
authority, authentic symbolic path binding, issued whole replay constrained by
actual output-owner parent cycles, source START eligibility and maximal
weighted scalar Slow components. Density consumes the prepared Index recipe
before query, with authentic H, expanded sampled-start reach and conservative
K/uniform B/finite N including owner heads. Ordinary and Subject paths retain
existing ownership semantics. No TASK/behavior criterion is marked complete.

Written fixtures: public
`weighted_slow_index_unites_authentic_wholes_without_emitting_in_part_gap`;
private configuration/index replay tests for fractional/negative/large
denominator arithmetic, exact one-less work, and owner-eligible later cycle.
All reordered public query windows assert one actual row before resolution.
These fixtures have not run. Root will perform the eight-command first
checkpoint on held source; no author Cargo or nextest process exists.

Known unfinished combinations: multiple occupied Index leaves, symbolic
multiple copies in exact configuration geometry, composed outer clock maps,
multiple inherited Index frames, nested/static Pattern-valued parameters and
partial source-start filtering across connected parent-cycle blocks. They
retain authenticated unresolved requirements rather than guessed support.
Broader static support, identity-substitution/depth/quota fixtures, Subject
ownership proofs and genuine sampled-start/owner-head witnesses remain due.
No canonical dynamic realization, complete geometry, playback or Ready claim.

### First checkpoint native compile failure and narrow repair

ROOT0479 original checker handle81855 terminated101 before Clippy/tests.
The compiler reported E0308 at density/index.rs171: trace length usize passed
to density_charge requiring u32. Raw native log
`/tmp/vactr-index-occupancy-checkpoint-001/native.log`, SHA256
`7eee0c8dbd7da2b07aa5dad69f8dc8c0ee492e27ec1591d1fa55370d874b4a5f`.
All900 cohort inputs remained unchanged during the failed check.

The authorized repair converts the actual length with checked u32::try_from
and capacity_overflow, preserving the preallocation shared-counter debit.
No unwrap, counter reset, semantic change or author Cargo/nextest. All task
criteria remain pending; this failed compile provides no behavior evidence.

### Checkpoint002 native pass and strict Clippy failure

Original57676 terminated101 (poll efde47). Native cargo check passed0;
strict all-target Clippy found only two collapsible_if conditions in operand
issuer traversal. No tests ran, and all900 cohort hashes remained unchanged.
Raw `/tmp/vactr-index-occupancy-checkpoint-002/clippy.log`, SHA256
`0726b3e8c891bcc6b54fd8e8f9689d3857676d26b23981e6731e508f64e54d83`.

Both ambiguity checks now append found.replace(index).is_some() after the
issuer/cursor/operation guards. Short-circuit mutation and duplicate detection
are preserved without lint suppression. No broader source changes, author
Cargo/nextest or behavioral passing claim. Remaining criteria stay unchecked.

### Checkpoint003 real query Arity and grouped slot repair

Original19388 terminated101 (poll fac001). Native check and strict all-target
Clippy passed. The sole public fixture failed in actual song.query before
geometry: `slow` got1arguments at source span15..39 beat0. Private replay
and density suites did not run; all900 cohort hashes remained unchanged.
Raw `/tmp/vactr-index-occupancy-checkpoint-003/occupancy-tests.log`, SHA256
`e8d9add31d7ddafddfbfb99ec73b105be7f499c66511b3ff41de6e963aeeff98`.

Reader list parsing uses Mode::List items (line.rs684/700), while braces parse
one contained expression into Block (line.rs660–680). The successful existing
weighted fixture uses `[{hold p 3} nil]`. The intended first slot is therefore
`[{slow 0 2} nil]`, grouping the call rather than four independent list items.
Only this fixture source syntax changes; every whole/part/gap/nonempty/routing
assertion and all production hashes remain preserved. No author Cargo/nextest
or claimed behavioral pass. Broader criteria remain incomplete.

### Checkpoint004 actual mode diagnosis, not an arithmetic oracle revision

Original4052 terminated101 (poll c2247b). Native and strict all-target Clippy
passed; the public query returned row1 whole [1,2), while the retained Index
oracle expects [1/2,3/2). No resolver, private replay or density assertions ran.
All900 cohort hashes remained unchanged. Raw occupancy-tests.log under
`/tmp/vactr-index-occupancy-checkpoint-004/`, SHA256
`4dc6d4175559f0be4832b4a572ba07a6126c7d9a88b659662b96491feb82fd6d`.

Source inspection establishes that transform_instrument constructs its selected
SongSource with structured=true (edit.rs39). query_slice chooses Index mode only
when the subject is unstructured and the index structured (region.rs362).
The original `slice p` fixture therefore exercises Subject mode, which preserves
subject whole, rather than the weighted Index clock. Snapshot assembly copies
that runtime whole directly; there is no normalization explaining it otherwise.
A diagnostic assertion now requires genuine issued Slice timing before any
Index geometry claim and prints actual rows and captured routing. Every earlier
whole, part, gap, nonempty and connected-configuration assertion remains intact.

An actual public function wrapper can create unstructured Pure and return the
authentic selected pattern during query. However current source-use capture
classifies Fn as DynamicSource, and Pure callable results are not prepared by
the retained transform discovery. This is an explicit public static-certification
capability gap; no callback output or origin is manufactured to bypass it.
Genuine false-structured selected-source consumer fixtures require actual session
capture and query, not arithmetic-only data; an additional session fixture seam
needs prior manifest authorization. No production change or author Cargo/nextest.
All task completion criteria remain pending.

### ROOT0486 genuine runtime companion and truthful public Subject regression

[Genuine Index runtime fixtures](song-mode-index-runtime-fixtures.md) owns a
separate two-path test companion: test-only inventory registration and its new
child. No original seven production occupancy implementations changed in this
checkpoint. The original DSL fixture is now explicitly Subject mode, with
source-derived whole/part oracle and no issued Slice timing. A second real
Subject fixture checks that Nil skips notes while configuration remains [0,4).
This is not Index evidence or an arbitrary oracle relaxation: the runtime branch
condition previously diagnosed selects a different clock and is now asserted.

The companion supplies actual internal Index coverage through fresh immutable
SongSource -> shared Freeze -> capture_routing_prepared -> isolated candidate ->
snapshot.query -> prepare_routes/resolve_route. Strict weighted Slow whole/gap/
connected-support and exact nonempty reordered windows remain unchanged there.
All origins/timing come from real runtime issuance and existing copying. No
public structure semantics, visibility or production source API is changed.

Written tests and scoped formatting are a pending-checker checkpoint, not proof
that all geometry or density criteria pass. Public function wrappers still need
retained callable-result authority and remain a full-goal followup; dynamic
canonical realization, broad composed/inherited support and full playback remain
unfinished. No author Cargo or nextest was executed.

### Checkpoint005 authenticated Subject clock traversal repair

Original24971 terminated101 (poll a0b40a); native and strict Clippy passed,
but both public Subject regressions failed resolving nested mapping geometry.
Private Index/replay/density suites did not run; all902 inputs remained exact.
Raw `/tmp/vactr-index-occupancy-checkpoint-005/occupancy-tests.log`, SHA256
`070e364b5ecd9308fc3bd90d897a87da932c1ae0076c72c039cbd53450e236e0`.

Intrinsic and affine source traversal rejected every Slices mapping before
building the otherwise authenticated subject chain. Actual query_slice Subject
mode preserves event whole/part and only skips a note when its sampled index is
Nil. mapped_support already leaves this clock's window unchanged; origin matching
already selects subject edge0 and authenticates its source trace. The narrow
repair admits only FrozenSliceStructure::Subject on ordinal0 in these two walks.
All trace consumption, owner/layout mapping and shared quota checks remain.
Index mode and edge1 are not preserving mappings and remain excluded here.

Public and genuine private fixture assertions are unchanged. This repair remains
pending independent verification, and composed nonlinear Subject support is not
claimed complete. No author Cargo/nextest or other production path changed.

### Checkpoint006 actual consumer reached missing matcher reservation

Original20882 terminated101 (poll7b3417), with all902 cohort hashes unchanged.
Native/strict Clippy and16 actual tests passed: public Subject2, replay3, density6,
prior inventory5. Both genuine internal Index tests issued timing and passed
strict query geometry, then failed resolve_route with FuelExhausted from Slice
timing validation. Raw session-tests.log under checkpoint006, SHA256
`03bea03dd161ab8b134d94d4ac8b9e7021478c0c63b82cc15d49664d4b5765f3`.

[Slice matcher reservation](../completed/song-mode-slice-matcher-budget.md) owns the separate
one-file accounting repair. It charges actual timing probe and reserves the later
matcher validation under original caller work/depth, without changing fixture
limits. All original occupancy and runtime assertions stay fixed. The repair
and its genuine quota regression are written and held for independent retry;
Index resolved-route identity and the broader wave are still unverified.

### ROOT0494 direct consumer and timing-budget checkpoint accepted

Original008 session49564 terminal0; all12 gates and20 unique tests passed.
Actual runtime-issued Index wholes/parts/gap and reordered full route identity
now pass. Public Subject/Nil, private replay, density, no-timing and genuine
exact/one-less/twice-shared/depth reservation all pass independently.
All903 source inputs stayed fixed. Evidence is checkpoint008 final-results.json
SHA256 f17254ee393ea1e1edf4265461876419f8958381bd270327b61fa2310b138cc7.
This accepts the direct checkpoint, not broad occupancy tasks or full58 matrix.
Multiple leaves/copies, outer clocks, inherited constraints and dynamic callable
authority remain required alongside host/transport/playback/export.
