# Song Slice Timing Authority Implementation Plan

**Status**: In Progress
**Plan ID**: SONG-SLICE-AUTHORITY
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Source Author**: rust_coding under ROOT0423

## Design references

- [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance): canonical realization, exact producer identity, whole spans before caller clipping.
- [Editing contract](../../design-docs/specs/design-song-mode.md#editing-contract): immutable selected streams and retained sibling identities.
- [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails): configuration ownership and bounded generations.
- [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics): shared fuel, depth and explicit failures.
- Source evidence: immutable HOST-PREPARATION/0019 and0020. Their cycle/cut-count density speculation is withdrawn; dynamic first-structure timing remains a genuine descriptor gap.

## Scope and dependencies

Capture actual Slice/Splice Subject versus Index mode and query-issued index timing in selected-source provenance. Preserve every inherited frame, authenticate issuing source and exact producer, and expose read-only borrowed timing to future consumers. Original source_whole stays separate.

This metadata prerequisite does not compute maximal configuration components, solve arbitrary dynamic index timing, change density/admission, or enable playback. It neither narrows existing admitted mappings to static-only nor treats a cell witness as a topology component. Source implementation requires a separate root release and specialized author/checker procedure.

| Dependency | State | Obligation |
|---|---|---|
| Source-whole authority | Existing source | Preserve issued_handle checks and exact None/whole behavior |
| Slice route consumer | Future separate wave | Joint constraints across inherited stages; exact membership and maximality |
| Dynamic timing bounds | Future separate prerequisite | Seal shared query/work ceiling; prove K/B/N and finite generation admission |
| Witness aggregation | Future consumer/query wave if needed | Existing insert_union retains first origin; cannot use hull as exact support |

The separate [Slice schema fixture compatibility](song-mode-slice-schema-fixture.md)
plan owns the compile-required hostile public Slices constructor in
tests/song_source_free.rs222. Root's fresh ROOT0407 search found this external
enum literal; existing inventory/density/nested matches use `..`. The primary
eight Rust paths remain unchanged. Both source scopes require prior releases and
a joined check; no implicit ninth test edit is permitted.

No existing plan, Rust source, index, dependency or archive is authorized by this document task.

## Exact future Rust manifest

| Path | Observed lines | Deliverable | Status |
|---|---:|---|---|
| src/pattern/combinators/region.rs |498|Issue timing before first-structure replacement|SOURCE_WRITTEN|
| src/song/source.rs |891|Origin storage, defaults, inherited freeze delegation|SOURCE_WRITTEN|
| src/song/source/slices.rs |absent|Cohesive issuance, validation and copy-cost helpers|SOURCE_WRITTEN|
| src/song/source_uses.rs |940|Additive Slice mode, borrowed matcher validation|SOURCE_WRITTEN|
| src/song/source_uses/origin.rs |159|Private frozen authority, getters, all inherited views|SOURCE_WRITTEN|
| src/session/song/source_uses.rs |957|Delegate Slice mode/cut capture before growth|SOURCE_WRITTEN|
| src/session/song/source_uses/slices.rs |absent|Cohesive Slice mapping capture|SOURCE_WRITTEN|
| tests/song_slice_timing.rs |absent|Genuine query-issued timing and shared quota evidence|SOURCE_WRITTEN|

Exactly eight future Rust paths. No implicit query.rs, snapshot.rs, routing or pattern Event change. Every touched Rust file must remain below1000 lines. Move the existing cut extraction into the new session helper before adding mode capture. Source parent delegates substantial timing logic/copy costs to its child. If an actual missing caller requires another path, hold and amend/split before writing it.

## Metadata types and exact interfaces

Signatures below are declarations, not implementation bodies. Imports use actual Pat/NodeId/Event/ProducerStep/EventHandle/TimeSpan/QState/Ratio64/Failure types.

### src/song/source_uses.rs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenSliceStructure {
    Subject { issuer: crate::reader::span::NodeId },
    Index { issuer: crate::reader::span::NodeId },
}
```

Extend the existing mapping variant, preserving starts/splice and all old enum cases:

```rust
Slices {
    starts: Vec<Ratio64>,
    splice: bool,
    structure: FrozenSliceStructure,
},
```

Issuer NodeId is a recipe locator, never sole authority. Bind it to the authenticated cover/root/revision/track plus complete source use path and exact Slice producer prefix. No fingerprint substitute.

Add a borrowed timing slice to existing OriginView; validate through existing resolve_origin before using a timing witness. Do not modify public resolve_source_use signatures or reset remaining work. Ordinary empty timing slices preserve old behavior. Extend copy-cost admission for the added mapping scalars before cover clone.

### src/song/source/slices.rs

```rust
#[derive(Clone, Debug)]
pub(in crate::song) struct SongSliceTiming {
    issuer: NodeId,
    issuer_trace: Vec<ProducerStep>,
    index_trace: Vec<ProducerStep>,
    index_whole: TimeSpan,
    sample_start: Ratio64,
    subject_handle: EventHandle,
}
pub(crate) fn issue_slice_timing(
    event: &mut Event,
    index_event: &Event,
    slice: &Pat,
    state: &mut QState<'_, '_>,
) -> Result<(), Failure>;
pub(in crate::song) fn slice_origin_clone_work(
    origin: &SongEventOrigin,
) -> Result<u32, Failure>;
pub(in crate::song) fn slice_timing_work(
    timing: &SongSliceTiming,
) -> Result<u32, Failure>;
pub(in crate::song) fn admit_slice_timings(
    timings: &[SongSliceTiming],
    subject: &EventHandle,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<(), Failure>;
pub(in crate::song) fn copy_admitted_slice_timings(
    timings: &[SongSliceTiming],
    admission_depth: u32,
) -> Vec<FrozenSliceTiming>;
pub(in crate::song) fn validate_frozen_slice_timings(
    timings: &[FrozenSliceTiming],
    subject: &EventHandle,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<(), Failure>;
```

The issuance helper is the only constructor. Live fields are readable only within song modules; region calls the helper through crate-visible source::slices. No public setter or forgeable constructor. Getter implementation and frozen type validation belong to the origin module; child helpers receive borrowed authority, not a mutable per-event authority sidecar.

### src/song/source.rs

Register `pub(crate) mod slices;`. Add private authority storage to SongEventOrigin:

```rust
pub(crate) slice_timings: Vec<slices::SongSliceTiming>,
```

query_source creates a new frame with empty local timings and retains the prior origin in inherited. A Slice over that selected subject adds its timing to the current frame, leaving inherited timing and source_whole unchanged. Update all actual literals here; repository search found SongEventOrigin literals only in this owned source. Existing iterative copy_origin retains its signature and delegates timing admission/copy for every frame before allocating result collections.

### src/song/source_uses/origin.rs

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenSliceTiming {
    issuer: NodeId,
    issuer_trace: Vec<ProducerStep>,
    index_trace: Vec<ProducerStep>,
    index_whole: TimeSpan,
    sample_start: Ratio64,
    subject_handle: EventHandle,
}
impl FrozenSliceTiming {
    pub(in crate::song) fn from_issued(
        issuer: NodeId,
        issuer_trace: Vec<ProducerStep>,
        index_trace: Vec<ProducerStep>,
        index_whole: TimeSpan,
        sample_start: Ratio64,
        subject_handle: EventHandle,
    ) -> Self;
    pub const fn issuer(&self) -> NodeId;
    pub fn issuer_trace(&self) -> &[ProducerStep];
    pub fn index_trace(&self) -> &[ProducerStep];
    pub const fn index_whole(&self) -> TimeSpan;
    pub const fn sample_start(&self) -> Ratio64;
    pub fn subject_handle(&self) -> &EventHandle;
}
impl FrozenSourceOrigin {
    pub fn slice_timings(&self) -> &[FrozenSliceTiming];
}
impl FrozenSourceOriginFrame {
    pub fn slice_timings(&self) -> &[FrozenSliceTiming];
}
```

Add private `slice_timings: Vec<FrozenSliceTiming>` to both frozen origin types. Constructors/copy helpers require ancestor visibility `pub(in crate::song)` where siblings call them; do not widen private public fields. Reexport FrozenSliceTiming/FrozenSliceStructure from source_uses as needed without unowned modules. Add compile-fail coverage for public timing field mutation/construction.

### src/session/song/source_uses/slices.rs

```rust
impl Builder<'_> {
    pub(super) fn slice_mapping(
        &mut self,
        slice: &Pat,
        subject: &Pat,
        index: &Pat,
        cuts: &crate::pattern::pat::SliceCuts,
    ) -> Result<FrozenUseMapping, Failure>;
}
```

Use actual `!subject.structured && index.structured`, not result.structured. Preserve Equal count bounds, int_of/numeric conversion, Manual points and splice behavior. Dynamic cuts retain their current addressed classification. Dynamic index timing is not rejected by this child merely because no static timing recipe exists. Store structure/issuer even for static valid cuts, without querying index or replaying RNG.

## Authority and preservation invariants

1. region query_slice calls issuance after successful sample_child, before merge_producers and assigning index whole/part. Only the Index branch issues timing. Subject branch preserves source timing and carries no invented cut-number producer.
2. Index whole must be present for an accepted sliced row; query_slice already faults absent whole in apply_slice. Preserve that diagnostic behavior. sample_start equals actual index_event.anchor, hence whole.begin; never derive it from requested span or the sampled subject's original onset.
3. Charge full current producer prefix, index producer trace, bound subject handle placement/occurrence words and scalar copies before allocation/Rc cloning. Rc::make_mut cloning must precharge the complete current origin frame, including existing timing vectors, source traces/handles and Sound/route-family/path bytes through slice_origin_clone_work; inherited Rc pointers retain their identity. Missing traced authority in song context fails explicitly. Legacy rows without song_source retain existing Slice behavior and do not gain song metadata.
4. Validate original issued handle before clone/use; subject_handle in each timing must equal that frame's authenticated full issued_handle. Public handle swap must fail before allocating frozen metadata. Same onset with different duration/producer/revision is not equivalent.
5. Captured spans stay in the issuing Slice's local clock. Later rate/rev/weighted/Sequence wrappers retain metadata unchanged. Future consumer maps the issuer path using authentic stage offsets and whole/member separation; it must not compare local timing directly against final shifted whole.
6. Every inherited frame carries its own ordered timing list. Do not flatten only the newest timing or reuse the outer handle for inner subject validation. Preserve source_whole=None exactly; a timing witness does not supply a missing original whole.
7. Admission charges collection lengths, each handle copy/equality operand, every trace step and all rational scalars. No quota refund/reset. Check aggregate chain/timing depth before allocating all frozen vectors. A timing helper does not add a fictitious Part level; real nested issuance/trace depth remains bounded.
8. Query paths retain event.song_source through clone/time transforms. This child therefore needs no Event field or query.rs mutation to issue and freeze metadata. Existing insert_union keeps the first origin for same-handle rows; this child promises authority of retained frames, not lossless disjoint timing aggregation. Consumer must prove retained authority sufficient or receive a separate bounded query aggregation amendment before using merged hulls.
9. Witness issuer/path matching against a certified Slice Index mapping is required for routing. Metadata structural validation can check issued source equality, trace/spans, ordering and work now; it must not claim that source handle alone authenticates arbitrary foreign issuer node/root. Prepared cover/root/path binding is a mandatory consumer obligation.

## Tasks

| Task | Status | Depends on | Parallelizable |
|---|---|---|---|
| TASK-001: Immutable Slice mode and authority types | In Progress | Source release | No |
| TASK-002: Actual issuance and all-frame frozen preservation | In Progress | TASK-001 | No |
| TASK-003: Genuine identity/quota verification and handoff | In Progress | TASK-002 | No |

### TASK-001

- [x] Move cut extraction into new session child before parent growth.
- [x] Copy exact Subject/Index mode and issuer; retain all legacy/cut semantics.
- [x] Add private live/frozen fields, borrowed getters/views and full checked copy costs.
- [x] Preserve public old handle/matcher signatures and sourcewhole authority.

### TASK-002

- [x] Issue actual index whole/sample-start/producer and bound source authority before replacement.
- [x] Preserve ordered timing on every inherited frame under rate/rev/weighted/offset transformations.
- [x] Reject source-handle/issued-authority mismatch before copy/use; retain actual legacy no-origin behavior.
- [x] Charge validation/cloning cumulatively before all allocation with true caller depth.

### TASK-003: exact genuine fixture declarations

```rust
#[test] fn structured_slice_retains_subject_timing_without_index_authority();
#[test] fn first_structure_slice_issues_real_index_whole_and_sample_start();
#[test] fn nested_rate_reflection_weighted_indices_preserve_all_timing_frames();
#[test] fn dynamic_index_issues_actual_timing_without_rng_replay();
#[test] fn timing_authority_rejects_handle_swaps_before_copy();
#[test] fn slice_timing_copy_and_validation_use_shared_exact_quota();
```

- [x] Build genuine capture_part/SongSource/query_part origins; use manually unstructured public Pat when needed. Normal transform_instrument constructs structured=true, so do not pretend ordinary `slice p` reaches first-structure mode.
- [x] Index Steps[0,Nil,1], Equal2: actual non-Nil cells [0,1/3),[2/3,1), sample starts0,2/3. Assert original subject whole distinct from index whole; exact producer paths, not emitted vector indexes.
- [x] Genuine nested selected stages with actual fast/slow/rev/weighted index patterns. Full/fractional/reordered queries compare retained local authority and complete handles; no local duplicate equations as production proof.
- [x] Actual frozen dynamic index parameter/callback supported by current query is evaluated only through canonical realization. Compare issued timing with actual emitted index whole; no handcrafted successful provenance or hidden RNG reevaluation. Missing exact dynamic fixture setup is a test-authoring issue, not permission to omit it.
- [x] Genuine same-onset/different-duration subject handles and foreign track/revision substitutions fail; missing original whole remains None, absent index whole preserves actual Slice failure.
- [x] Exact admission/one-less, cumulative two-frame copy and caller-depth boundaries; all error paths debit caller work. Public frozen mutation compile-fail cases.
- [x] Mandatory independent native/browser checks, strict Clippy, fresh inventories/unfiltered new+affected suites, scoped format/diff and nextest when released. Cargo quiet and prescribed nextest environment.

## Explicit consumer and dynamic-admission handoff

Future route consumer authenticates mode/issuer/path against prepared source covers, distinguishes index sample-start from membership/output/source birth, maps all inherited constraints into one root clock before clipping, and regenerates maximal connected configuration support. Issued whole/sample-start alone cannot authenticate neighboring index occupancy or continuity. No conservative hull is exact membership; no note/tone/index-cut value configuration key.

Future dynamic admission seals an actual shared query/work ceiling and proves prepared K/B/N, finite total openings and generation representability against host capacity. Caller-variable max_events and successful cell issuance alone do not establish that bound. Static recipes may provide tighter bounds; dynamic support needs bounded control-side proof/discovery and truthful addressed fuel/overflow/capacity failure. No whole-score/Repeat expansion, blanket static-only exemption, arbitrary success, or RNG/VM query in per-event route resolution.

No full geometry, preparation owner, host readiness or song playback completion follows from this child. Neutral/other plans remain unchanged.

## Completion criteria

- [x] All eight declared sources implemented below1000 lines with independent gates passing.
- [x] Every issued timing is immutable, genuine, bounded and retained on all inherited frames.
- [x] Subject/sourcewhole authority remains separate and handle swaps are addressed.
- [x] Dynamic-index issuance demonstrated without pretending maximality/admission solved.
- [x] Exact pending route/aggregation/bound contracts reported and full goal remains active.

## Progress log

### 2026-10-02 — Document-only Ready handoff

Source seams inspected, exact eight-path manifest/signatures declared. All live/frozen origin literals found in owned source/origin files; no mandatory Event/query literal changes. First-structure issuance attaches metadata to retained song_source, avoiding a ninth query.rs path. Existing union loss is explicitly outside this metadata promise and remains a consumer/aggregation dependency. No Rust or Cargo work started. Immutable receipt0021 records absence and observations;0022 records this new document and held source comparison.

### 2026-10-02 — ROOT0407 external schema caller audit

Fresh enum-constructor search identified a required source-free hostile fixture
update outside the eight primary paths. Declared a separate one-path companion
before implementation. Original five malformed cases and Type diagnostics remain
mandatory; neither timing metadata nor this compatibility change establishes
component connectivity, dynamic bounds, host readiness or playback. Source
ownership of the session builder is still held by the active closure wave, so
timing implementation remains unstarted pending its serialized release.

### ROOT0423 actual source implementation

Implement immutable timing and genuine issuance in the exact eight paths plus the separate one-path schema child. Internal source/slices refinement approved before code: `admit_slice_timings(timings: &[SongSliceTiming], subject: &EventHandle, remaining: &mut u32, max_depth: u32) -> Result<(), Failure>` validates and charges without allocating; `copy_admitted_slice_timings(timings: &[SongSliceTiming]) -> Vec<FrozenSliceTiming>` is used only after all frames have been admitted. Replace the unused standalone freeze helper declaration with those actual two passes. This preserves whole-chain preallocation admission and prevents double debits. Query issuance is guarded by the real shared QState budget; no invented Part level or fresh nested query budget. Strong Index/dynamic/quota tests can reside in the owned source child for access to crate-private authentic copy and frozen VM adapter, while new public tests cover genuine exposed candidate flows/privacy. No additional source path or public API is introduced for test access. Source work started; no verification or routing/playback completion claimed.

### Aggregate authority depth refinement and actual fixture declarations

Before allocation, copy_origin accumulates all inherited frames and timing entries, then requires each actual index producer depth to fit the remaining caller depth. The copied private timing admission_depth retains this aggregate bound; frozen validation checks it and the actual copied trace, without resetting shared work. Empty timing chains preserve old admission.

Private query fixture helpers in source/slices.rs: `selected(part: Rc<Part>, structured: bool) -> Rc<Pat>`, `sliced(subject: Rc<Pat>, splice: bool) -> Rc<Pat>`, `real_origins(pattern: Rc<Pat>, window: TimeSpan) -> Vec<Rc<SongEventOrigin>>`. They use actual SongSource/query_part issuance, not fabricated origins. Tests cover Index/Subject, nested inheritance, clipping/points/reordered calls, exact copy quota/depth and legacy no-origin behavior. Session helper tests use real evaluated dynamic index and copied graph. Public integration fixtures use actual isolated candidate preparation. No Cargo while mutable.

### Fixture completion declarations

`FrozenSliceTiming::from_issued(..., admission_depth: u32)` additionally copies the private admitted whole-chain depth; `copy_admitted_slice_timings(timings, admission_depth)` receives it only after complete preadmission. `producer_depth(timing) -> Result<u32, Failure>` bounds the real copied producer path. No external depth setter exists.

Private tests also query real weighted Hold indices through nonunit Fast/Early transforms and compare actual emitted whole with the issued local witness. Genuine no-whole uses a public signal Pat declared structured as the structure-giving Control value over ordinary Sound, preserving actual continuous events. A structured signal index produces actual NoWhole. Dynamic test uses a genuine evaluated/frozen function list and a CountingVm delegating every call to actual VmQuery; no simulated callback result. Private mutable None is labeled hostile preservation only. Additional helpers remain local and use actual public pattern construction/query. Public candidate fixtures exercise both copied mode descriptors and structured selected Subject timing.

### Query-side preallocation depth refinement

The actual issuer checks the complete inherited chain plus existing/new timing count and the real producer path against QState's caller SongLimits before producer/handle/vector allocations. The bounded no-allocation chain scan is charged to that same QState. Frozen copies retain aggregate admitted depth; copy validation uses one shared remaining counter, no reset/refund. Foreign-track handles used by negative fixtures are themselves issued by real query_part. No candidate or timing results are fabricated.

### Source-ready checkpoint under ROOT0423

All eight primary Rust deliverables and the explicit schema companion are written and scoped formatted. Query issuance occurs before Index first-structure whole replacement, authenticated live subject equality is admitted before allocation, and all inherited timing vectors are copied only after complete aggregate admission. Private frozen depth retains the combined chain/timing/producer bound. The shared QState issuer/copy/matcher counters are not refunded or reset.

Written genuine fixtures cover both Slice/Splice Index issuance, prepared Subject and ordinary Index descriptors, inherited/rate/reflection/weighted/offset timing, reordered/point queries, actual frozen dynamic functions delegated to VmQuery, same-revision/same-onset/different-duration and foreign-track/revision substitutions, real continuous no-whole events and absent Index whole failure, and exact single/inherited copy quotas/depth. The internal manually cleared whole remains explicitly a hostile structural preservation case, not a genuine issuer. Public timing fields remain private with compile-fail declarations.

No Cargo, test, Clippy, native or browser verification has run in this author wave. Written fixture inventories are declarations only. All behavioral/independent criteria remain unchecked until the mandatory checker runs. Slice component connectivity, dynamic-density proof, host readiness and full playback remain separate pending obligations.

### ROOT0427 private visibility repair

Independent checker original49613 reached terminal101: native and host-wasm checks passed, strict Clippy rejected private_interfaces because the live timing field was crate-visible while its sealed element type is song-visible. No behavioral tests ran. Narrowed only the field to pub(in crate::song), matching all actual field consumers; region continues calling the crate-visible issuer helper. No function/test change, lint allowance or authority widening. Failed001 evidence is retained. Rust remains held, awaiting fresh checker release.

### ROOT0429 actual public Index diagnostic

Original56167 terminal101 had native/wasm/strict Clippy passing, Subject public test passing and ordinary Index descriptor assertion failing; private suites remained unrun. The authorized exact single diagnostic command (`CARGO_TERM_QUIET=true mise exec -- cargo test --test song_slice_timing prepared_unstructured_ordinary_slice_copies_index_mode -- --exact --nocapture`) reached foreground terminal101, chunk83de34. The Debug graph proves actual Capture root operation Slice, but mapping Uncertifiable(DynamicCount); its index Steps is Uncertifiable(DynamicSource). Native cuts uses param_of without forcing lazy arguments, so this raw brace fixture does not provide a static copied cut parameter. The helper correctly retains dynamic-count classification. Only the failed assertion gained graph Debug; all original mode/row expectations and production bytes remain unchanged. Proposed named-function fixture repair awaits root coordination; no broad tests or nextest ran.

### ROOT0430 actual static public Index repair

Changed only the ordinary Index test's candidate script to a genuine named `fn sliced p:` whose eager body supplies constant2 and static [0,nil,1]. Captured Index mapping, exactly two emitted rows and absent selected origin assertions remain unchanged. Exact quiet author test reached foreground terminal0, chunk72db95: 1 passed, 0 failed, 1 filtered. Log0016-static-index-test.log is retained with the earlier failures and graph diagnostic. No production/other-fixture change, broad tests or nextest. This one-test success does not verify the private suites or clear metadata/geometry/playback. Source remains held for mandatory independent retry.

### ROOT0435 accepted independent verification; ROOT0439 plan reconciliation

The final held Slice source passed the independent matrix recorded in
`tmp/song-mode-riela/ROOT0432-slice-matrix-acceptance.json`: all 48 gates,
270 distinct unit/integration tests and four privacy cases. Native and browser
compilation, strict Clippy, scoped formatting, line limits and diff checks passed.
The whole source-free suite retained all 12 tests, including the original five
malformed metadata cases and Type diagnostics. All 888 frozen inputs remained
unchanged through the checks. Earlier failed attempts remain in the progress log.

The separate nextest run passed all 11 selected Slice tests, with no selected
skips or failures; its original foreground process terminated with exit zero.
ROOT0435 joins that evidence with the matrix acceptance. The nextest selection
is a subset of the matrix tests, not 11 additional distinct tests.

The scoped implementation and verification criteria above are now checked.
This plan remains In Progress solely pending serial archive/index reconciliation;
its source work and independent verification are finished. This evidence proves
immutable genuine Slice timing and schema compatibility. Index occupancy capture,
maximal connected support, dynamic admission bounds, host readiness, automatic
part progression, playback and export remain unfinished under the active full
song-mode goal. ROOT0438 is implementing the separate index timing operand plan.
