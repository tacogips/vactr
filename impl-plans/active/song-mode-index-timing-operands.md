# Song Index Timing Operand Capture

**Plan ID**: SONG-INDEX-TIMING-OPERANDS
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Source Status**: Verified under ROOT0461; ROOT0465 consumer handoff complete; serial archival pending
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics), [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Purpose and evidence

Preserve actual timing operands before the selected-source graph erases scalar
and rest leaves. The existing graph describes selected-source paths and audio
families. It cannot distinguish Steps [0,nil] from [nil,0] reliably: atomic_step
maps both leaves to Empty, and weighted_steps stores slots only on edges to
nested patterns. Pure(0) and Pure(nil) likewise lose their leaf distinction.
Audio family counts cannot recover integer index occupancy.

The accepted Slice prerequisite retains actual emitted index whole, sample
start and producer authority. It does not certify neighboring occupancy or
finite total configuration openings. This separate capture wave retains the
operand recipe needed by those subsequent consumers.

ROOT0429/0430 also demonstrated an actual Slice whose raw lazy CUTS argument is
classified DynamicCount despite a literal-looking source expression. Capture
must describe the actual frozen operand; syntax alone is not a constant proof.

## Related plans and dependencies

| Dependency | State | Requirement |
|---|---|---|
| [Slice timing authority](song-mode-slice-timing-authority.md) | Scoped verification accepted ROOT0435 | Source ownership transfers only after its independent acceptance |
| [Slice schema fixture](song-mode-slice-schema-fixture.md) | Same joined acceptance scope | Preserve existing malformed cases |
| [Route geometry](song-mode-route-geometry.md) | In Progress | Future consumer authenticates operands and maximal connected support |
| [Host preparation](song-mode-host-preparation.md) | Planning | Future admission consumes real finite birth and resource proofs |

No density, components, preparation, scheduler, transport or export source is
owned by this capture wave. Dynamic operands remain represented; their eventual
admission still needs bounded canonical realization and a uniform ceiling.

## Exact source manifest

| Path | Current observed lines | Deliverable |
|---|---:|---|
| src/song/source_uses.rs | 958 | Register child and add opaque recipe reference/getter on FrozenPattern |
| src/song/source_uses/timing.rs | absent | Immutable recipe, operand/slot descriptors and checked validation |
| src/session/song/source_uses.rs | 587 | Register capture child and keep parent dispatch small |
| src/session/song/source_uses/timing.rs | absent | Capture actual frozen patterns and retained callback shapes |
| src/session/song/inventory.rs | 867 | Issue recipe at actual FrozenPattern construction under shared budget |
| src/song/routing/source.rs | 727 | Explicit compatibility for two private matcher fixture literals |
| src/session/song/source_uses/timing/capture.rs | absent | Explicit charged postorder dependency traversal and cohesive Pat assembly |
| tests/song_index_timing.rs | absent | Genuine capture/query, producer and quota evidence |

Exactly eight Rust paths after ROOT0455. Search all constructors again before source release.
No implicit ninth fixture, source-free schema change or dependency update.
Every touched file remains below 1000 lines; substantial code belongs in the
two new children. Fixture-only matcher DTOs use None and never claim issuance.

## Types and interfaces

Declarations below are interfaces, not implementations. Private recipe fields
and constructors stay within the owning song/session capture boundary.

```rust
pub struct FrozenIndexTiming {
    root: u32,
    nodes: Vec<FrozenIndexTimingNode>,
    requires_realization: bool,
}
pub struct FrozenIndexTimingNode {
    issuer: NodeId,
    operation: FrozenUseOperation,
    structured: bool,
    leaf: Option<FrozenIndexLeaf>,
    leaf_trace: Vec<FrozenUseTraceTerm>,
    parameters: Vec<FrozenIndexParameter>,
    slots: Vec<FrozenIndexSlot>,
    children: Vec<FrozenIndexChild>,
}
pub enum FrozenIndexNumber {
    Int(i32),
    Int64(i64),
    Ratio(Ratio64),
    FloatBits(u32),
    Float64Bits(u64),
}
pub enum FrozenIndexLeaf {
    Rest,
    Scalar { number: Option<FrozenIndexNumber> },
    AtomicList { elements: Vec<FrozenIndexLeaf> },
    Pattern { child: u32 },
    Continuous,
    Dynamic(FrozenIndexDynamicKind),
}
pub enum FrozenIndexDynamicKind {
    Function,
    Native,
    Late,
    Thunk,
    Signal,
    UnresolvedPattern,
}
pub enum FrozenIndexParameter {
    Number(FrozenIndexNumber),
    Literal(FrozenIndexLeaf),
    Pattern { child: u32 },
    Dynamic(FrozenIndexDynamicKind),
}
pub struct FrozenIndexSlot {
    ordinal: u32,
    geometry: Option<FrozenUseSlotLayout>,
    copies: Option<u32>,
    weight: FrozenIndexParameter,
    count: Option<FrozenIndexParameter>,
    child: u32,
}
pub struct FrozenIndexChild {
    role: u32,
    child: u32,
    trace: Vec<FrozenUseTraceTerm>,
}
```

Expose borrowed read-only getters for node/slot/child fields. Do not expose
mutable vectors, raw Value, VarSlotRef, Fn captures, SampleBuf or active evaluator
references. Numeric float bits preserve the actual f32 value without float
equality serving as identity. Nonnumeric scalar is not a valid Slice-index claim.

```rust
impl FrozenPattern {
    pub fn index_timing(&self) -> Option<&FrozenIndexTiming>;
}
impl FrozenIndexTiming {
    pub fn root(&self) -> u32;
    pub fn nodes(&self) -> &[FrozenIndexTimingNode];
    pub fn requires_realization(&self) -> bool;
    pub(crate) fn retained_copy_work(&self) -> Result<u32, Failure>;
}
pub(super) fn capture_index_timing(
    pattern: &Rc<Pat>,
    shapes: &PreparedShapes,
    remaining: &mut u32,
    limits: SongAssetLimits,
) -> Result<Rc<FrozenIndexTiming>, Failure>;
```

FrozenPattern receives a crate-private Option<Rc<FrozenIndexTiming>>. The opaque
public recipe type prevents private-interface warnings without opening its
construction or mutation. Cache clones share immutable recipe ownership; charge
actual pointer/scalar retention instead of a fictional deep recipe clone.

## Capture and authority contract

1. Capture the actual already-frozen Pat supplied to inventory. Reuse the same
   remaining_work and walk-depth limits; no fresh quota per operand or callback.
2. Retain every scalar/rest Steps slot, not merely source-bearing edges. Static
   prefixes/widths and symbolic copy counts match runtime weighted layout.
   Dynamic weight/count keeps its raw classification and no guessed geometry.
3. Pure lists remain atomic. Steps nested lists subdivide. Preserve recursive
   atomic classification and true caller depth before allocating their vectors.
4. Preserve actual structured flags and static timing parameters through the
   current operations, including rates, Rev, Cats, repeats, Euclid and Slice.
   Symbolic repeat copies remain symbolic; no score or repeated-slot expansion.
5. Runtime Steps paths use NestedStep(ordinal), GeneratedBranch(copy) where
   applicable, then Child(0) for squeezed pattern children. DynamicExpansion(0)
   belongs only to actual dynamic resolution. Store real relative terms and
   child roles; do not synthesize producer paths from emitted vector positions.
6. Consume a callback result only when PreparedShapes already owns that actual
   retained result. Missing shapes, Fn, Native, Late, Thunk and Signal preserve
   explicit unresolved descriptors. Capture executes no VM/RNG and does not
   reject an otherwise supported dynamic pattern solely for being dynamic.
7. Initial issuance charges nodes, leaves, parameters, slots, traces, collection
   extents and symbolic scalars before allocation. Validate node references,
   cycles, normalized ranges, copy terms and checked arithmetic before use.
   Failed admission never restores or resets prior consumed work.
8. NodeId locates a recipe; it does not authenticate an arbitrary foreign owner.
   A future consumer must bind the opaque issued operand to its original
   prepared payload/root, revision/track, complete operand path and actual
   query-issued Slice prefix. Public copied DTOs cannot manufacture issuance.

## Tasks

| Task | Status | Depends on | Parallelizable |
|---|---|---|---|
| TASK-001: Opaque recipe and exact caller audit | Completed | Slice acceptance and root source release | No |
| TASK-002: Lossless actual operand capture | Completed | TASK-001 | No |
| TASK-003: Genuine fixtures and independent verification | Completed | TASK-002 held | No |
| TASK-004: Explicit geometry/dynamic-bound handoff | Completed | TASK-003 | No |

### TASK-001

- [x] Search every FrozenPattern constructor, clone and copy-work caller.
- [x] Implement immutable recipe storage and borrowed getters without a public
  constructor/mutator or mutable VM aliases.
- [x] Preserve the two matcher fixtures' actual old behavior using None.
- [x] Keep all touched sources below1000 lines and source ownership bounded.

### TASK-002

- [x] Preserve distinct [0,nil] and [nil,0], longer scalar/rest lists and Pure
  atomic lists, with actual operation and structured mode.
- [x] Preserve every weighted slot, static normalized geometry, symbolic copies
  and unresolved raw parameters with runtime producer terms.
- [x] Retain actual prepared callback output; never execute a new callback during
  recipe capture or infer an unseen output from a similar prototype.
- [x] Charge shared issuance/validation and retained Rc-copy costs before copies.

### TASK-003

- [x] Actual candidate/query fixtures distinguish scalar/rest and atomic versus
  subdivided operands and compare actual whole/producer paths with the recipe.
- [x] Genuine Hold/Repeat/weighted slots preserve symbolic large copies without
  output expansion; static rates/Rev/Euclid keep exact local timing descriptors.
- [x] Real retained transform callback and unresolved Fn/Late/Thunk/Signal cases
  preserve classification and demonstrate no capture-time VM/RNG replay.
- [x] Exact/one-less cumulative quotas, nested caller-depth, cached Rc sharing,
  invalid private references and public privacy tests cover actual code paths.
- [x] Mandatory independent native/browser, strict Clippy, nonempty whole
  inventories and actual runs, scoped format/line/diff and selected nextest pass.

### TASK-004

- [x] Report lossless static recipe and unresolved dynamic realization contract
  to geometry and preparation plans with exact source/evidence hashes.
- [x] Leave finite birth bounds, maximal connected components, host readiness,
  scheduling, Apply/mute and export explicitly unfinished where not implemented.

## Completion criteria

- [x] All eight paths and compile-required callers implemented within ownership.
- [x] Actual scalar/rest/atomic/static-slot distinctions retained immutably.
- [x] Actual local producer fragments, symbolic repetition and shared budget/depth preserved.
- [x] Dynamic operands represented without fabricated normalization or admission.
- [x] Independent verification passes on fixed hashes with actual name agreement.
- [x] Future consumer obligations reported without claiming full Song Mode ready.

## Progress log

### 2026-10-02 — Ready capture handoff, source unstarted

Read-only audit established actual leaf/slot erasure and the seven-path manifest.
Root confirmed the three real FrozenPattern construction sites and actual f32
representation. PreparedShapes proves retained transform outputs only, not a
universal bound on time-dependent callbacks. ROOT0434 records prior document
intent. No Rust, Cargo, density proof, host preparation or playback is authorized
by this plan alone; source work remains serialized behind the Slice check.

### Concrete capture declarations before source

Fresh constructor audit confirms inventory plus exactly two matcher literals. `FrozenIndexTiming::issue(root: u32, nodes: Vec<FrozenIndexTimingNode>, requires_realization: bool, remaining: &mut u32, max_depth: u32) -> Result<Self, Failure>` is crate-private, validates the actual postorder reference DAG and structural depth under the same remaining counter before opaque Rc issuance. Descriptor fields are crate-private for the sibling session issuer and public through read-only getters; no external constructor/mutable accessor. `retained_copy_work` charges one actual Rc pointer retain, not a fictitious deep clone.

Actual Value also has Float64, so lossless numeric capture adds `FrozenIndexNumber::Float64Bits(u64)` alongside existing f32 bits; this avoids silently classifying a numeric operand as nonnumeric. Raw invalid/nonfinite numeric bits are retained without invented valid geometry. Session `TimingCapture` carries shared remaining, limits, postorder nodes and pointer-keyed (node,height) cache; cache hits validate depth+height. `pat`, `value_node`, `atomic_leaf`, `parameter`, `steps`, and `child` are private cohesive methods. Their collection bounds and cache requests are charged before allocation; no VM/RNG invocation occurs.

Pure atomic-list contained patterns are retained as operand references with empty relative trace, not claimed as runtime children. Actual Steps list recursion gets NestedStep edges; symbolic repeated slots get GeneratedBranch terms and squeeze Child0, including the additional unwrap Child0 on Hold/Repeat. Static geometry remains None when any sibling weight/count is unresolved or invalid. The runtime clamps intrinsic Repeat to0..4096; preserve raw count separately from copies. Future consumers must still authenticate payload ownership, trace and realization bounds.

### Approved static-literal refinement and producer declaration

Root approved Float64Bits and `FrozenIndexParameter::Literal(FrozenIndexLeaf)`: static Nil, nonnumeric scalars and atomic lists must not be mislabeled dynamic. Parameter Literal and each slot weight/count are validated with the same shared work/depth, including embedded Pattern references. Node additionally retains `leaf_trace: Vec<FrozenUseTraceTerm>` through a read-only getter: an actual top-level dynamic leaf uses DynamicExpansion0, while a continuous Signal node and contents of an atomic list do not invent an evaluation edge. These are operand-local producer fragments, not absolute identity. Every producer-edge search is charged per visited edge.

Routine fixture declarations: private `limits() -> SongAssetLimits`, `capture(pattern: &Rc<Pat>, remaining: &mut u32, depth: u32) -> Result<Rc<FrozenIndexTiming>, Failure>`, and `actual_query(pattern: &Pat, window: TimeSpan) -> QueryResult` use real patterns and VmQuery. Private tests cover scalar/rest order, atomic/subdivided list, intrinsic Hold/Repeat with genuine trace paths and huge symbolic counts, actual static operations/raw parameters, unresolved variants, exact/one-less/shared quotas and cached-height behavior. Public candidate tests cover authentic prepared callback retention and immutable Rc cache sharing through repeated Parts. Validation hostile tests use only private issued-type constructors, never claim fabricated successful provenance.

### Source checkpoint before mandatory verification

All seven declared Rust paths are written. Inventory constructs the immutable
operand recipe and precharges pointer retention for both initial cache storage
and cached payload copies. Direct private native-output setup is labeled as
capture coverage; the public Every fixture instead uses the production retained
transform preparation and a genuinely returned Slice pattern. Public candidate
fixtures cover scalar/rest order, that retained output, repeated-Part inventory
Rc sharing, and unresolved function/thunk operands. Actual relative ProducerStep
comparison is in private VmQuery fixtures; public whole comparisons do not infer
producer ordinals from onset. Two privacy doctests and private hostile-reference,
literal-depth and exact-budget fixtures are written.

No Cargo command has run in this source wave. Compile, behavior, quota and
compatibility results await mandatory independent verification on held hashes.
All task/verification checkboxes remain unproven until actual evidence. The
recipe is an operand description, not finite density, maximal configuration,
host Ready, scheduling, Apply, mute, playback or export admission.

### Independent verification 001 — scoped Copy fixture repair

Original checker handle 65171 terminated 101 (chunk bd7e5b). Native and
wasm checks passed; strict Clippy stopped at clone_on_copy in the public
fixture before behavioral execution. Logs remain in
`/tmp/vactr-index-timing-independent-001/`; final-results SHA
`8db69afe9f258fbca6b30787068b311a8af1f84e454f8559dce4e69a0bbcf8f5`.
ROOT0445 authorizes only dereferencing the already Copy slot layout instead
of cloning it. Assertions and production sources are unchanged. No author
Cargo command ran; mandatory retry remains pending.

### Independent verification 002 — dynamic classification diagnostic

Original checker 22038 terminated 101 (chunk 1b7eeb). Native, wasm and
strict Clippy passed. Public inventory ran four tests: three passed and the
function/thunk case failed at requires_realization before later assertions.
Raw evidence remains in `/tmp/vactr-index-timing-independent-002/`. ROOT0447
authorizes only adding list, expected kind and complete recipe Debug to that
assertion message. Classification and query-row assertions remain unchanged;
production and later private/regression evidence are not claimed. No Cargo
ran in this diagnostic author wave; exact diagnostic checker is pending.

### Actual lowering diagnostic — genuine lazy Thunk setup

Diagnostic foreground 753d74 terminated 101. The Function case completed its
classification and two-row query assertions; only the named eager-body Thunk
case failed. Its actual recipe contained Int0/Rest/Int1 because eager evaluation
had already consumed the closures. Diagnostic final-results SHA
`e4ba425ec2947f257cbdb28cc8da9036d79eb0487ebab0b23213121e7ae28579`
and raw log SHA `e7927cfbc779849bde1c3cf6fd60d3f2d136acd8b9d88d140f89affd1e48a04b`
remain preserved. ROOT0449 changes only the Thunk setup to a direct lazy native
Slice expression, consistent with actual retained closure lowering. Function
setup, requires_realization, exact Dynamic(Thunk) and two-row assertions remain
unchanged. No production classification changes or author Cargo; verification
of the corrected genuine setup remains pending.

### Lazy operand diagnostic — retained leaf location

Original diagnostic 36560 terminated 101 (chunk 675580). The corrected lazy
setup passed requires_realization but failed the following exact dynamic-leaf
assertion before querying rows. ROOT0451 adds only list, expected kind and
full recipe Debug to that second assertion, to establish the actual lowered
kind/location before proposing another repair. No expectation, production,
classification or row assertion changed. No author Cargo; diagnostic pending.

### Actual Late lowering and genuine retained Thunk declaration

Diagnostic 76746 terminated 101 (chunk a52755), showing direct lazy Slice
blocks lowered to Dynamic(Late), including its raw cuts parameter. ROOT0453
authorizes the truthful public function/lazy-block fixture to expect Late,
retaining realization, dynamic-leaf, query-row and Debug assertions. Separate
private `actual_vm_thunk_survives_freeze_capture_and_query()` obtains a genuine
unevaluated Value::Thunk from Sess::new evaluation of `at 4 {0}` and its released
OneShot effect. It closes actual empty assets, freezes that VM-created value,
captures a Steps operand from the frozen closure without evaluating it, and
queries the frozen pattern using actual VmQuery, asserting its scalar output.
This is direct private retained-value coverage, not full candidate Thunk issuance.
No fabricated closure, provenance, prepared shape or successful query is used.

### ROOT0455 — normal-stack postorder repair declaration

Original checker 78253 terminated 101 (chunk 7ed13a): native/wasm/Clippy
passed and actual index4, Slice2, whole5, provenance4 and contracts8 passed.
The unchanged source-use 200/300-spine regression then aborted with stack
overflow. Actual Pat capture retained the large exhaustive fill frame through
every descent. No stack size, depth limit or fixture expectation is changed.
The eighth cohesive capture child is declared before writes.

`pub(super) fn capture_pat(capture: &mut TimingCapture<'_>, pat: &Pat,
depth: u32, step_wrapper: bool) -> Result<u32, Failure>` uses private borrowed
postorder tasks for Pat, parameter, atomic/Steps values and Finish. Task requests
and additional inspections are charged before allocation under the same counter.
It preserves semantic pointer+wrapper cache keys, caller depths and cycles;
Finish assembles the existing descriptors after dependency patterns are cached.
Existing helper declarations, scalar/rest semantics, symbolic counts, producer
traces and callback lookup remain unchanged. This extra traversal adds honest
work, not a reset, refund, VM replay or score-cycle expansion. Raw failure remains
in `/tmp/vactr-index-timing-independent-003/`; mandatory retry is pending.

### Explicit-stack source checkpoint

The cohesive child now gathers borrowed Pat, parameter, atomic-value and Steps
value dependencies through precharged task requests. Its actual signature is
`capture_pat<'p, 'c: 'p>(&mut TimingCapture<'c>, &'p Pat, u32, bool)
-> Result<u32, Failure>`; retained shape references outlive each task borrow.
Each incoming reference checks caller depth and cached intrinsic subtree height.
Each uncached node resets its local height measurement before its descendants
and restores the enclosing peak only after recording its own height.
Finish rejects any unexpected uncached pattern reference instead of descending
recursively, using a private assembly-phase flag. The original small atomic-list
and weighted-value assembly helpers retain their existing actual depth checks;
they no longer carry the exhaustive Pat dispatcher through recursive values.
No scalar/rest/trace/slot operation or existing test assertion was changed.

Parent and capture child are below1000 lines after scoped formatting. No Cargo
ran in this repair wave. The 200 admitted/query/route, 300 DepthExceeded and
shared-diamond regression remain mandatory and unchanged; source readiness does
not establish their success until the independent retry runs.

### Independent verification 004 — actual native fixture registration

Original checker 21276 terminated 101 (chunk c4a12f). Native, wasm and
strict Clippy passed. Prior eighteen whole suites passed, including unchanged
deep200/300/shared-diamond source-use17 and new core validation2. Session
suite ran39 with38 passing, including genuine VM Thunk and timing quota/copy
fixtures; only direct retained native callback failed UndefinedName because
its fresh core namespace had not registered domain natives. ROOT0457 changes
only that fixture to register_domain on its core Prelude before Namespace
construction. Actual rev NativeId, VM call, output and all assertions remain
unchanged. Raw failure is preserved in
`/tmp/vactr-index-timing-independent-004/session-song-tests.log`.
No production or author Cargo changes; mandatory independent retry pending.

### ROOT0461 — independently accepted timing capture

Root reparsed the actual 22 whole-suite inventory/run pairs: 263 distinct unit
and integration tests plus six privacy cases, all 54 gates successful. This
includes the unchanged 200/300-depth/shared-diamond regression, all four public
fixtures, six private capture fixtures and two private validation fixtures. The
normal-stack traversal repair passed; no limits or original assertions weakened.
All 893 frozen inputs and raw-log hashes matched before, after and at acceptance.
The separate nextest run passed the exact twelve new tests with zero selected
skips or failures; those tests are a subset of the 263, not additional cases.
Original nextest 31972 reached terminal0, chunk9ece3d. Failed attempts remain
recorded above; ROOT0459 and ROOT0461 contain the exact source/evidence hashes.

TASK001–003 are complete within timing capture scope. TASK004 remains In Progress
until the exact handoff is recorded in geometry and preparation consumer plans.
The owning prepared payload/root and complete path still need authentication
when consuming these descriptive fragments. Maximal temporal support, occupancy
and finite birth/generation bounds, host preparation, transport/playback,
Apply/mute and export remain unfinished under the active full song-mode goal.

### 2026-10-02 — ROOT0465 accepted consumer handoff
All accepted capture source hashes rechecked against ROOT0461. Geometry and
preparation plans now consume the exact ROOT0465 handoff and ROOT0464 occupancy
wave. TASK-004 is complete; plan stays In Progress solely pending serial archive
and reference reconciliation. This completion applies to timing metadata only;
occupancy, dynamic bounds, host readiness and full playback remain unverified.
