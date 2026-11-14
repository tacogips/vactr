# Closed graph resource provenance implementation plan

**Plan ID**: SONG-GRAPH-RESOURCE-CLOSURE
**Status**: In Progress
**Design Reference**: [Finite values](../../design-docs/specs/design-song-mode.md#finite-values), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and authority

Pin supported declaration assets during isolated candidate construction and expose an immutable site-to-closed-PCM inventory for later private host materialization. Genuine opaque issued records from phase 1, together with exact installed Arc identity and actual graph sites, establish provenance. Numerical graph references alone cannot establish a captured asset.

ROOT0374 authorizes document-only phase separation. No Rust/Cargo release is implied. Phase 2 consumes independently accepted phase-1 APIs; prior serial source release and fresh baseline/intents are required. This prerequisite does not implement host upload/Ready/Applied, reusable scheduling, remaining geometry or finite playback/export.

## Related plans and dependencies

- [Obsolete freeze helper removal](song-mode-closure-obsolete-helpers.md) owns the
  single freeze.rs cleanup discovered by ROOT0408 strict Clippy. The primary
  eight Rust paths below remain unchanged; no implicit ninth edit is allowed.

- **Previous / depends on**: [Graph resource declaration capture](song-mode-graph-resource-capture.md), phase 1, four Rust paths and opaque original keyword/installed-Arc authority.
- **Host compiler**: [Graph materialization](song-mode-graph-materialization.md), independently accepted real Native environment and retained Box submission.
- **Later consumer**: actual preparation owner and full host adapters; fixed/event PCM union, multiplicity and real physical admission remain mandatory.

| Dependency | Required state |
|---|---|
| Genuine declaration capture | Independently accepted ROOT0385 |
| Candidate assets and source shapes | Existing verified isolated/frozen contracts; same cumulative work counters |
| Host owner | Later consumer, not supplied by closure |

## Exact eight-Rust manifest

| Path | Baseline lines | Deliverable |
|---|---:|---|
| `src/session/song.rs` | 842 | Isolated mode selection, charged declarations and pin before close |
| `src/session/song/inventory.rs` | 847 | Exact used owner/Arc handoff and shared frozen-copy quota |
| `src/song/snapshot.rs` | 871 | Compact resources child registration and public DTO reexports |
| `src/song/snapshot/routing.rs` | 103 | Defaulted resources field on actual routing inventory |
| `src/song/snapshot/resources.rs` | absent | Cohesive immutable resource capture/validation/inspection |
| `tests/song_graph_resource_closure.rs` | absent | Actual candidate and closed loader/provenance/privacy/bounds tests |
| `src/session/song/source_uses.rs` | 957 | Reexport prepared-shape interfaces; cohesive extraction and retained outcome consumer |
| `src/session/song/shape_preparation.rs` | absent | Once-only discovery, closed validation and authentic copied re-keying |

Snapshot is near1000; place definitions/capture in the new child, with only registration and reexports in its facade. All touched Rust stays below1000; any additional helper needs prior manifest amendment. No build/registry/VM/asset-loader/freeze or external literal source mutation in phase 2. Those phase-1 sources remain readonly.

## Modules and exact declarations

### Immutable frozen resource inventory — new resources.rs

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrozenGraphOwner {
    Instrument(InstId),
    Bus(BusId),
    Master(BusId),
}
#[derive(Clone, Debug)]
pub struct FrozenGraphResource {
    owner: FrozenGraphOwner,
    site: GraphResourceSite,
    source: SampleSrc,
}
impl FrozenGraphResource {
    pub fn owner(&self) -> &FrozenGraphOwner;
    pub fn site(&self) -> &GraphResourceSite;
    pub fn source(&self) -> &SampleSrc;
}
#[derive(Clone, Debug, Default)]
pub struct FrozenGraphResources {
    entries: Vec<FrozenGraphResource>,
    consumed_work: u32,
}
impl FrozenGraphResources {
    pub fn entries(&self) -> &[FrozenGraphResource];
    pub const fn consumed_work(&self) -> u32;
    pub(crate) fn pin_selections(
        registry: &InstRegistry,
        instruments: &[InstId],
        remaining: &mut u32,
    ) -> Result<Vec<SongAssetSelector>, Failure>;
    pub(crate) fn capture(
        inventory: &FrozenRoutingInventory,
        registry: &InstRegistry,
        assets: &mut PinnedSongAssets,
        remaining: &mut u32,
    ) -> Result<Self, Failure>;
}
```

Private fields and no public issuer prevent arbitrary FrozenGraphResource construction, rebinding or changing a captured SampleSrc. Default is empty only; a graph with a required fixed reference cannot be certified by substituting empty metadata. capture consumes the actual registry's readonly issued records and verifies exact matching inventory graph Arc, site/effect/parameter, temporary placeholder and closed asset presence. It does not accept a caller-created FrozenGraphResource list as authority. Original bank identity stays issued, never reconstructed from numerical zero or a resource ID. Clone preserves the immutable binding and consumed_work.

For the accepted ClosedSong Convolution IR Bank(keyword), pin the complete bank, then select explicit member index0. Missing or empty bank fails with an addressed candidate diagnostic; dynamic/nonnegative numeric fixed sites and stale/absent graph provenance fail. Missing or negative constant IR remains disabled with no required resource. Preserve phase-1 Legacy keyword behavior and the event-selected UGen exclusions.

pin_selections uses the complete candidate-retained instrument/family set plus retained named/master buses. Establish that set from actual evaluated candidate sound/source dependencies before backend closure; do not silently omit a callable/source family's graph. Final capture crosschecks the actual frozen inventory against the same issued graph authority and rejects any incomplete selection. Resource closure is not a live fallback. The current FrozenRoutingInventory captures used instruments and registry buses; do not infer owners from identical graphs/placeholder values.

### Facade registration and routing field

```rust
// snapshot.rs, new child registered here rather than under private routing:
mod resources;
pub use resources::{FrozenGraphOwner, FrozenGraphResource, FrozenGraphResources};
// Existing FrozenRoutingInventory gains a Default-compatible field:
// pub resources: super::FrozenGraphResources
```

Actual snapshot.rs private routing registration and explicit reexports require this owned facade path. Existing imports remain valid. Literal consumers in session/song/cells.rs and routing/source.rs use Default completion and require no source edits. Revalidate that inventory before release; do not conceal new mandatory fields in another sidecar.

### Isolated candidate mode, budgets and assets — session files

Before package/document forms, explicitly enable ClosedSong on the fresh candidate registry, after prelude construction. No ordinary Evaluator mode change or global ir control row. Inst/bus/master lowering and commit are phase-1 dependencies, not repeated implementations here.

Collect and charge graph/site/keyword/source copies before allocations, then merge bank selections with the existing event asset selections before assets.pin and assets.close. Existing pin closes/deduplicates full banks under real max_resources/max_pcm_bytes/max_banks; selected index0 is retained as the exact SampleSrc. Path/Buffer event support stays unchanged. UGenInput currently lacks a fixed Path/Buffer resource variant: this plan adds no such fixed-input language contract.

Carry declaration-collection and retained discovery work into shape preparation and inventory limits; no max_walk reset. Inventory currently subtracts shapes.consumed_work and uses builder.remaining_work through cell capture. FrozenGraphResources capture consumes the same remaining counter and stores its own actual admitted delta; do not refund shape/cell/declaration work or double-count one admission as a later scan. Every actual repeated scan/copy is charged. Exact/one-less tests must cover cumulative collection plus frozen validation, including repeated declarations and missing/stale sites.

No assets.rs change is authorized. Its existing bounded pin, closed-bank loading, resource_count and pcm_bytes are the real resource/PCM authorities. No backend or live AtomicCells loader survives successful closure. Asset pinning deduplicates captured identities; an ID-based inverse guess cannot substitute for the opaque original declaration.

Evaluator/journal restore namespace slots, not registry graphs. This accepted contract is isolated-candidate disposal: reject every failed form, diagnostics or invalid result and drop the entire candidate/assets/metadata. A successful replacement must use current matching declaration Arc. Existing active evaluator/graph/assets must remain untouched, with genuine tests. No ordinary whole-form registry rollback is claimed.

## Later owner integration handoff

Host materialization must rewrite each fixed site to its admitted private sample ID and validate exact float resource representation. It then uploads captured PCM before dependent graphs using actual ResourceReady/fullkey/Seal/Applied; this prerequisite does not fabricate those receipts.

The later owner must union event-selected assets and fixed IR assets, deduplicate captured PCM identities, price actual frames/bytes/resources/per-slot/native limits, and retain dependency leases through refusal/cancellation. Scalar/analysis bank multiplicity and exact private owner identities remain separate; capture must not alter analyzer ordering/sparse logical IDs or event BANK semantics.

## Tasks

### TASK-001: Candidate selections and pinning

**Status**: Completed
**Parallelizable**: No (depends on accepted capture phase)

- [x] Enable mode only on fresh candidate before declarations; discover and retain actual once-evaluated callback outputs before close and establish complete retained graph/family owner set.
- [x] Charge selections/copies and pin complete banks before closing; member0 is exact authority.
- [x] Preserve event resource routing and all ordinary source-free compatibility; closed validation and shared-cache re-keying never re-execute callbacks.

### TASK-002: Immutable frozen capture and public facade

**Status**: Completed
**Parallelizable**: No (depends on TASK-001)

- [x] Exact Arc/site/effect/placeholder/closed asset validation and opaque private DTOs.
- [x] Complete resources field and clean snapshot.rs public registration/reexports without literal churn.
- [x] Shared cumulative work/depth/resource/PCM admission; actual consumed_work/clone retention.

### TASK-003: Actual closed candidate verification

**Status**: Completed
**Parallelizable**: No (depends on TASK-002)

```rust
fn candidate_fixed_ir_banks_survive_backend_close();
fn equal_placeholders_retain_distinct_issued_bank_and_pcm_authority();
fn header_event_resource_and_fixed_ir_keep_separate_sites();
fn closed_candidate_replacement_and_failure_preserve_active_resources();
fn closed_site_mismatch_and_unresolved_fixed_inputs_are_rejected();
fn resource_work_pcm_limits_are_cumulative_and_exact();
```

- [x] Genuine declaration source with embedded, named bus and master keyword IR resolves captured PCM after actual backend disposal.
- [x] Actual callback-computed keyword from retained string/list yields a supported generated instrument family with fixed IR; PCM remains available after backend disposal, and a once-only callback counter proves no VM/RNG replay.
- [x] Two distinct original banks both lowered to zero retain distinct owner/site/SampleSrc/PCM; naked numerical zero cannot masquerade as either.
- [x] Header event bank versus fixed IR member0 remains distinct; multiple effects/shared DAG/reversed sites retained from phase1.
- [x] Successful replacement, failed declaration/candidate, stale matching id/changed Arc and missing metadata do not reuse provenance; active evaluator/assets unchanged.
- [x] Missing/empty bank, numeric/dynamic fixed IR, invalid graph index/parameter and hostile private issuance mutations reject without live fallback.
- [x] Genuine exact/one-less resource/work/PCM quotas and huge/sparse cases fail before unbounded allocation; full/point/reordered candidate queries retain identical bindings.
- [x] Existing bank/path/Buffer/event resource and ordinary source-free regressions pass.
- [x] Native/browser checks, strict Clippy, scoped format/diff, fresh nonempty qualified inventories and actual unfiltered fixtures independently pass; preserved failure/log/terminal/source-hash evidence.

### TASK-004: Integration handoff

**Status**: In Progress
**Parallelizable**: No (depends on TASK-003)

- [x] Supply accepted immutable provenance and measured closed resource counts to the preparation-owner plan.
- [ ] Parent owner consumes actual closure/materialization/union-admission contracts; genuine Native/Arena remap/adoption proof remains required and unchecked until that implementation.

## Status and completion

| Area | Status | Evidence |
|---|---|---|
| Source contract / facade audit | Declared | ROOT0380 amended eight paths; getter separately prerequisite |
| Candidate mode/pin | Verified | ROOT0420 matrix and ROOT0422 nextest acceptance |
| Immutable inventory/tests | Verified | Actual closed Bank/Path/Buffer and fixed-site PCM tests; frozen115 hashes |
| Consuming owner | Pending | Later integration requirement |

- [ ] All own tasks/criteria independently proven within eight paths and file bounds.
- [ ] Every original pin/candidate/PCM/opaque-authority obligation preserved across the linked phases.
- [ ] Parent preparation consumes exact closed provenance and actual resources; prerequisite alone does not complete playback.

## Progress log

### 2026-10-02: Source audit and reviewed contract

Historical Planning receipts 0003/0005 record IR keyword rejection and namespace-only rollback findings. Receipts 0007/0009 resolve them through candidate disposal and root-accepted narrow ClosedSong IR keyword support. Full-repository inventory literal search found Default-completed constructions; lower_inst accepts exactly id, root, header, Lowering, and the additive helper appends only mode. No general evaluator transaction work or ordinary resource semantic change is included. No Rust, existing plans or Cargo changed.

Implementation proceeds only after a separate source release; independent closure proof precedes the consuming host preparation owner. Arbitrary dynamic fixed resources and fixed Path/Buffer language inputs remain separate scope, while all existing event-selected assets are preserved.

### 2026-10-02: ROOT0374 document-only bounded phase split

The earlier combined contract's public facade required a ninth Rust consumer. Capture now owns four paths in the linked phase-1 plan; this closure phase owns six including snapshot.rs. Source declarations and installed metadata are opaque/private-issued, frozen closure validates them rather than accepting a fabricated DTO list. All original Legacy/ClosedSong semantics, source-free/event compatibility, complete-bank member0, exact Arc authority, isolated failure disposal, cumulative budgets and later real owner/PCM union obligations remain. No Rust/Cargo/other plan/index/archive change; separate source release and accepted capture prerequisite are required.

## ROOT0380 superseding retained-shape contract

The original six-path sequencing is superseded by the eight-path table above. Capture acceptance is independently recorded by ROOT0385; freeze accounting is independently accepted ROOT0400; this document is Ready specification for the separately released source wave, not source authorization or PCM proof. Existing source_uses.rs491–767 preparation is extracted into shape_preparation.rs before growth. The parent reexports PreparedShapes and any compatibility wrapper needed by current crate callers; analysis continues consuming the same final prepared-key map. freeze.rs980 remains read-only.

### Private retained state and exact proposed seams

```rust
pub(super) struct PendingShape {
    callback: Value,
    input: Rc<Pat>,
    outcome: PendingShapeOutcome,
}
pub(super) enum PendingShapeOutcome {
    Pattern(Rc<Pat>),
    Rejected(FrozenUseReason),
}
pub(super) struct PendingShapes {
    entries: Vec<PendingShape>,
    retained_instruments: Vec<InstId>,
}
pub(super) fn discover_shapes(
    evaluator: &mut Evaluator, song: &Song, limits: SongAssetLimits,
    remaining: &mut u32,
) -> Result<PendingShapes, Failure>;
impl PendingShapes {
    pub(super) fn retained_instruments(&self) -> &[InstId];
    pub(super) fn freeze_records(
        self, freeze: &mut super::freeze::Freeze<'_>,
        remaining: &mut u32,
    ) -> Result<FrozenPendingShapes, Failure>;
}
pub(super) struct FrozenPendingShapes {
    entries: Vec<PendingShape>,
}
impl FrozenPendingShapes {
    pub(super) fn admit_closed(
        self, evaluator: &mut Evaluator, assets: &mut PinnedSongAssets,
        limits: SongAssetLimits, remaining: &mut u32,
    ) -> Result<PreparedShapes, Failure>;
}
```

Pending fields and issuers remain private to construction. Retained values, not pointer triples alone, establish the link from discovery to copied keys. Deduplication uses callback plus input identity while retaining both values, including rejected outcomes. Nested callbacks in successful returned graphs are recursively discovered under the same depth/work admission, without expanding score cycles or Repeat counts. Permitted Native/Fn checks, Query effect mode, saved fuel/output handling and diagnostic rejection semantics match the existing ShapePreparation::callback; actual VM fuel consumption is deducted once. Only successfully retained/generated outputs contribute owner discovery. Original Source/buffer identity checks are performed against the authentic pre-copy dependencies.

Route owners are resolved with the actual registry and frozen sound-kit family semantics already used by ClosedShapeCtx, including Value::Inst, Sound::Inst and Builtin families. Each owner scan and descriptor growth is charged. The union contains genuine dependency/family owners plus actual generated owners; it excludes unrelated unused registry graphs. pin_selections validates issued records for that union and retained buses/master before assets.pin/close. Existing syntactic event-resource admissions and no-I/O boundaries remain unchanged: discovering a generated fixed IR bank does not authorize an arbitrary callback event Path or Buffer.

### Borrow and copy ordering

1. discover_shapes uses evaluator.vm_and_ns() only while its mutable VM/immutable namespace pair is held; no backend is loaded through callback execution. Retain values and genuine owner IDs, then release these borrows.
2. Collect issued fixed selections and original event selections using the same remaining counter; pin complete banks once under actual resource/PCM limits, then close/drop the backend.
3. Namespace copying follows existing isolated semantics. A single reachable Freeze with immutable closed-assets borrow copies the Song plus every retained callback, input and successful output through freeze_records. Existing Freeze::value is sufficient; no new Freeze API is presumed. Copy original retained values, then retain the actual copied callback/input/output values in opaque FrozenPendingShapes. Shared Source/buffer copies stay shared through that cache.
4. Explicitly drop the reachable Freeze before admit_closed obtains a fresh vm_and_ns() pair and constructs existing ClosedShapeCtx with mutable closed assets. Admit and inspect the copied actual outcomes without a VM call; derive final shape_key from the retained copied callback/input values and preserve addressed rejected outcomes. There is no function simultaneously taking mutable closed assets and a Freeze borrowing those assets.
5. PreparedShapes flows to inventory. Resource capture follows under the same remaining counter, allowing mutable closed-asset member0 checks. The inventory builder exposes its remaining counter to final cell/resource capture instead of creating a fresh quota.

Every retained vector/key-map allocation and repeated dependency/family walk is preadmitted. Freeze copying uses its actual existing internal guards under a reduced remaining limit, then publishes successful actual work through the separate getter prerequisite. Exact/one-less tests cover cumulative discovery, record storage, actual cached/copy traversal and later validation, not a conservative invented multiplier. Existing namespace/root freeze guarantees are preserved; no mutable assets and immutable Freeze asset borrow overlap. The new inventory entry takes the current remaining counter directly; it must not subtract aggregate PreparedShapes work again after those stages have already debited it. Existing compatibility entrypoints retain their previous local accounting.


### Required genuine discovery evidence

Use a real accepted candidate callback that constructs a sound keyword from captured string/list values, resolves a declared generated family and retains its fixed IR original bank. Verify real emitted route, opaque issued site, explicit member0 and decoded PCM after backend disposal; retain distinct event bank versus fixed bank. A private construction counter at the actual callback admission/VM-call seam proves one invocation per retained key and zero calls during freeze_records/admit_closed/inventory, including nested outputs. No user side-effect native is admitted to create this counter. Prove final shared-copy keys match actual analyzed callbacks, full/point/reordered query bindings, missing/empty/dynamic fixed-bank failures, unchanged arbitrary event-resource rejection, active-state preservation, exact/one-less cumulative work and PCM budgets. Existing source-free/selected-source/loaded-callable fixtures remain unchanged.

### 2026-10-02: ROOT0380 document-only sequencing amendment

Before receipt SONG-GRAPH-RESOURCE-CLOSURE/0001 records both exact document hashes. This amendment closes the late generated-owner discovery seam without static-only fallback or pinning unused registry instruments. All original closure/materialization/preparation-owner obligations remain; TASK004 parent consumption is still a later integration requirement and cannot be marked complete by prerequisite tests. No Rust, Cargo, other plan, index, archive or dependency changes.

### ROOT0380 borrow/accounting clarification

The same-cache copy and final closed validation are distinct operations: FrozenPendingShapes owns copied values after Freeze is dropped, and admit_closed borrows vm/ns and assets only afterwards. Pre-copy Source/buffer authenticity is established during discovery; final copied aliases are retained through the actual shared cache.

Use the separately declared [Freeze work accounting](song-mode-freeze-work-accounting.md) prerequisite. Its actual readonly consumed_work getter replaces the speculative conservative copy-price visitor. Each namespace/reachable Freeze receives the current reduced work limit; existing internal guards admit every traversal and array clone before allocation. After a successful pass, retain the returned values and its actual work, explicitly drop the Freeze borrow, then checked-debit that work once before later admission. No hidden reset/refund or second charge of the same successful pass. Discovery, retained-record storage, final validation and inventory/cell/resource scans independently debit their own actual admissions. Failed attempts dispose the whole candidate and never contribute a successful retained debit.

Repository caller audit found prepare_shapes only at session/song.rs404 plus its definition; no external test calls that function. Existing source_uses private closed_resource_tests directly constructs ShapePreparation, so move/adapt those tests with the cohesive extraction and preserve their real closed-bank/default-family/unpinned expectations. A legacy prepare_shapes wrapper, if retained for local compatibility, must consume already-frozen inputs without copying/rekeying them again; no automatic single-Freeze wrapper may silently invalidate input pointer keys. The new candidate path explicitly uses discovery, same-cache freeze_records, drop, and admit_closed.

### Exact accounting prerequisite (independently verified ROOT0400)

Freeze::work remains private and its readonly crate getter now exposes actual admitted work, independently verified ROOT0400. Dependency-node counts omit prototype code/spans/masks/call/list sites, so no conservative multiplier is claimed. The separate getter plan is Completed; closure is Ready for its own source authorization and remains unimplemented. The closure manifest stays eight paths; freeze.rs is a readonly prerequisite here, never an implicit ninth write. Existing retained PendingShapes -> shared reachable freeze_records -> explicit drop -> admit_closed vm/ns/mutable-assets sequencing remains mandatory, with actual generated families and no VM/RNG replay.

### ROOT0386 prerequisite handoff

Capture is independently accepted ROOT0385 (910 joined distinct+17 selected repeats); this is not closed PCM proof. Freeze accounting was declared separately in the linked plan at this historical entry. No source/Cargo changes; phase2 remained Planning at that point, and parent owner consumption/full song playback remain unchecked.

### ROOT0401: Ready after independent accessor acceptance

Independent ROOT0400 acceptance (SHA659866f5b3bd880416f18bbb28e72c0411707a8d484366835f08c0b0c2b0f3d7) verified67 command gates,1026 exact unit/integration names plus4 privacy executions =1030 distinct across the joined matrix. Matrix `/tmp/vactr-freeze-reuse-independent-004/final-results.json` SHA f34310087d469cdf8de2edadf2d68000cc25ddc8c50e81b2ea40812855c8953e; original27304 terminal0. Supplemental `/tmp/vactr-freeze-reuse-nextest-independent-001/final-results.json` SHA d405431dc16956e4eae985688518ccedd788b3077f93774d603849c4485ccbd2, foreground4f743a terminal0:18 selected PASS,0 selected skips,1934 unselected skips. Exact inventory/run identities and all102 current hashes matched. Joined totals are not child-only counts. Native/wasm/strict Clippy/scoped format/diff and relevant actual regressions passed. Earlier failed attempts and repair receipts remain retained.

Capture ROOT0385 and getter ROOT0400 prerequisites are independently met. The exact eight future Rust paths in the manifest are unchanged; all closure TASK001–004 implementation/verification/parent-consumption checkboxes remain unchecked. Ready does not authorize source by itself or prove PCM. Once-only generated callback discovery with retained values, authentic owner/fixed-bank union, single audited reachable Freeze copying all retained records, explicit drop before vm/ns/mutable-assets closed admission and exact successful-work debit remain required. No VM/RNG replay, static-only fallback, unused-registry pinning, hidden reset/refund/double-charge or ninth implicit path. Full preparation/transport/playback/export remain pending.

### ROOT0403 source batch

Implementation started under exact eight-path authority and immutable 0006 baseline. Preparation is extracted cohesively; callback execution remains discovery-only. Internal `PreparedShapes::dependency_roots(&self, pattern: &Rc<Pat>, remaining: &mut u32, limits: SongAssetLimits) -> Result<Vec<Value>, Failure>` includes reachable retained callback outputs for actual family inventory. `capture_routing_shared(evaluator: &Evaluator, song: &Song, limits: SongAssetLimits, shapes: &PreparedShapes, remaining: &mut u32) -> Result<FrozenRoutingInventory, Failure>` carries already-debited work without a second shape charge. All verification and parent consumption remain pending.

### Actual fixture and accounting declarations

Private test-only `SHAPE_CALLS: Cell<u32>` observes the actual permitted discovery `Vm::call_value` seam; it grants no user effect native. The closed candidate test checks one invocation before any query and retained output family/resources, plus direct malformed registry/Arc/site capture with actual decoded assets. Internal `charge(remaining: &mut u32, count: usize) -> Result<(), Failure>` and `dependencies(roots: &[Value], limits: SongAssetLimits, remaining: &mut u32) -> Result<SongAssetDependencies, Failure>` debit actual scans. Pending selector roots are collected during the charged discovery traversal instead of re-running unmetered extra_roots. Genuine generated keyword evidence selects a keyword from a captured list using a captured string comparison; no unsupported string-to-keyword native is invented. All phase verification is pending.

### ROOT0403 coherent source checkpoint (verification pending)

Written: candidate ClosedSong opt-in; pre-close once-only discovery with retained original Source/buffer identity checks and actual generated owner resolution; complete issued fixed-bank union; namespace and shared reachable Freeze passes using reduced limits and actual consumed_work debited once after explicit drop; copied callback/input re-keying and closed admission without VM replay; prepared-output dependency inventory; shared cell/resource counter and opaque resources facade. Ordinary unclosed callback outcomes remain addressed rejected shapes, preserving ordinary candidate semantics rather than imposing a new whole-document refusal. Eight public closure fixtures and five private fixtures (including the retained prior closed-bank test) are written; one immutable-field privacy doctest is written. No Cargo or behavior passing claim. Parent TASK004 ownership, real remap/Ready/Applied union consumption and full playback/export remain unchecked.

### ROOT0410 actual first-check cleanup

ROOT0408 original78949 terminal101: native/wasm passed, strict Clippy failed with seven unused import/helper/field diagnostics; no behavioral test executed. Raw `/tmp/vactr-closure-pool-independent-001/clippy.log` SHA bd6db655288e1b0c710056b0395e31b9c9c002ae7541fd957351ead82c947a70 retained. Exact repair removes the unused song_asset_dependencies import, cfg(test)-gates existing test-used inventory wrappers and the PreparedShapes legacy local work field. No shared counter or compatibility body changed. Explicit one-path obsolete-helper companion owns freeze deletion; closure manifest remains eight paths. New joined verification remains mandatory; no passing behavior or full playback claim.

### ROOT0414 retained-shape behavior repair

Checker 003 original 15605 terminated 101: public closure 8 and reuse 14 passed; two private retained-shape witnesses failed. Named `every` callbacks remain `Value::VarRef` under the native Fn argument mask, while `shape_key` accepted only Fn/Native and returned before discovery. Extend the existing exact key signature with kind 2 and `Rc::as_ptr(&slot.0)` identity; Freeze already copies slot contents while retaining that slot identity, so final copied input rekeying remains unchanged. Retain all dependency, purity, fuel and closed-admission checks. The direct unknown `s :not-pinned` fixture is rejected by the document checker; use a genuine captured-list computed selector to reach closed asset admission instead. Scoped author diagnostics are authorized under ROOT0414; passing evidence remains pending.

Scoped author run 001, original 31457, terminated 101: original nested anonymous callback witness passed exact two calls/no replay. Computed unknown-bank discovery now reaches actual complete-bank pinning and correctly refuses the whole candidate with HostUnavailable (no catalog). To additionally prove the closed-admission boundary, the same private fixture evaluates the genuine score in a fresh isolated evaluator, discovers the actual callback once, freezes the score and retained outputs with one Freeze, debits measured successful work after dropping its asset borrow, then admits against an actual fixed-only closed bank and captures inventory using the same remaining budget. The existing UnclosedResource mapping assertion remains. This is a deliberately narrower lower-level closed-validator witness; production complete generated-owner discovery is unchanged.

Scoped run 002 (`CARGO_TERM_QUIET=true mise exec -- cargo test --lib retained_shape_tests`), original 57343, terminated 0: both genuine private tests passed (2 distinct, no nextest). Log `tmp/song-mode-riela/SONG-GRAPH-RESOURCE-CLOSURE/retained-shape-author-002.log`. Run 001 original 31457/101 and its full log remain retained. The nested original anonymous inner callback invokes exactly twice, and prepare_song does not replay it. Full-candidate unknown bank now refuses actual missing complete catalog with HostUnavailable. The additional deliberately fixed-only closed-boundary path evaluates actual code, performs genuine discovery once, freezes candidate namespace/song/outcomes with the same Freeze, drops the borrow and debits measured work once, and produces exactly one UnclosedResource rejection plus its original inventory mapping while fixed PCM remains retained. No successful full-candidate claim is made for that adversarial path. Production delta is only VarRef key support; no purity/dependency/fuel/copy guards changed. All source held for mandatory independent joined retry; no phase completion.

### ROOT0416 event-selected BANK ownership refinement

Checker 004 original 13333 terminated 101 after native/wasm/Clippy, closure 8, reuse 14 and session 31 passed. Original candidate tests found an extra prelude sampler default bank and missing pinned bank for a native loaded sample. Issued header metadata remains authentic, but Header(BANK) describes an event-selected default: only an actual resolved bank route matching `(InstId, issued KwId)` requires its closed PCM. All other headers and embedded/bus IR remain fixed. `pin_selections(registry, instruments, event_banks: &BTreeSet<(InstId, KwId)>, remaining)` and `capture(inventory, registry, assets, event_banks, remaining)` share this private proof set. `include_inst_resource(owner: InstId, record: &DeclaredGraphResource, event_banks: &BTreeSet<(InstId, KwId)>, remaining: &mut u32) -> Result<bool, Failure>` charges classification; full installed Arc, site and placeholder validation precedes filtering. No bank name, numeric placeholder or catalog existence establishes use.

Rename pending selector roots to dependency roots, retaining every actual admitted callback Pattern after charging storage. Original Song, retained outputs and selectors enter the same bounded dependency/family scans before closure. Both discovery and final owner collection include actual Bank/Path/Buffer routes; normalize event paths and retain original buffer identity/pinning. Newly allocated roots, route copies and bank-use insertions are charged before growth. Original candidate count/PCM/native-loader assertions remain read-only. New private default/override BANK plus fixed-IR tests and genuine output-root witnesses remain within the existing authorized closure paths. Independent verification pending.

Initial scoped ROOT0416 run001 original3715 terminated0: unchanged original candidate15 and existing closure8 all pass (23 distinct). Log event-bank-author-001.log. New fixture declarations: retained_generated_bank_is_an_actual_dependency_root evaluates a genuine callback, verifies output Rc identity and actual event-bank dependency; selected_default_and_overridden_bank_keep_fixed_ir constructs a real sampler header plus fixed IR, derives proof from actual registry routes, checks shared pin/capture classification and stale/site rejection even with dormant header; computed_generated_event_sources_close_bank_path_and_buffer proves real closed generated event source union for all three kinds. Fixtures remain in authorized existing paths and preserve earlier assertions.

Author run002 original8131/101 exposed runtime kit refusal for `:event-bank`; catalog membership is not sound-kit authority. Run003 original4314/101 exposed literal Path type mismatch. Preserve full query evidence using actual default-known `:bd` with explicit complete local catalog, and `s {sample ./c.wav}` for genuine Path. Native `sample` returns Sound::Sample, including the original loaded-file witness; it is not Buffer. Test-only `fresh_evaluator(&DecodedSongAssetFactory, file: &str) -> (SongAssetPreparation, Evaluator)` reuses identical isolated setup. The retained-root private witness additionally binds an actual issued ready SampleBuf to a fresh ordinary slot, evaluates a real callback, checks source identity and actual pin/copy/freezing/closed admission with no replay. This lower-level ready-buffer input witness does not claim forbidden language capture/render support in whole candidate evaluation. No counters/count assertions weakened.

### ROOT0417 cohesive retained test child

The separate [retained shape fixture plan](song-mode-retained-shape-fixtures.md) owns `src/session/song/shape_preparation/retained_shape_tests.rs`; closure retains its original eight Rust paths and parent registration. Extract the complete test module body with preserved qualified names, production prefix, closed_resource_tests and assertions. The formatted intermediate exceeded 1000; no Cargo ran on it. Actual native-ready Buffer callback/copy proof and explicit public Bank/Path checks continue before scoped verification.

ROOT0416/0417 scoped author repair evidence: run001 original3715/0 preserved original candidate15 and closure8, including exact3 resources/48 bytes, lazy1 and native backend-disposal sample loading. Genuine expanded retained module extracted into companion child (parent758/child255); production plus closed_resource_tests prefix unchanged. Run005 original24856/0 retained3 proves original nested2/no replay, actual generated Bank/Path/ready Buffer root identity and successful private copied closed Buffer, and actual unknown-bank full-candidate/closed validator refusal. Run006 terminal0 resources3 proves actual default/override BANK use with fixed IR retained, no stale/site bypass. Run008 original39322/0 publicclosure9 proves computed real known-kit Bank and explicit Path with exact variants, original8 unchanged; run009 terminal0 closed-resource1 preserves original closed loader boundary. These are31 distinct scoped tests (15 candidate,9 closure,3 retained,3 resources,1 closed-resource); no nextest. Failed runs002/003/004/007 remain immutable with full logs: genuine kit mismatch, literal-path fixture type mismatch, private constructor and detached path identity respectively, corrected without weakening assertions. Every process is terminal, all source held for mandatory independent joined retry. TASK004 parent preparation consumption and full playback remain incomplete.

### 2026-10-02 — Independent closure and pool acceptance

ROOT0420 accepts the joined83 successful commands: 1111 distinct unit/integration
names plus5 privacy doctests. Original53105 exited1 at the harness overlap guard;
all71 Cargo commands passed, seven repeated executions are retained and counted
once. Remaining-only original86720 exited0 for12 gates without repeating those
suites. ROOT0422 accepts fresh exact21 nextest selected/PASS names, selected
skips0, original83017 exit0. All115 source/document hashes matched through checks.
Tasks001–003 are verified, including unchanged candidate15, closure9 and actual
Bank/Path/ready Buffer witnesses. Preparation receives this closed resource
contract; its consuming remap/adoption and full Ready proof remain unchecked.
TASK004 and overall plan remain In Progress until that integration is real.
