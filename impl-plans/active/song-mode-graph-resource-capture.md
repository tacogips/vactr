# Graph resource declaration capture implementation plan

**Plan ID**: SONG-GRAPH-RESOURCE-CAPTURE
**Status**: Completed
**Design Reference**: [Finite values](../../design-docs/specs/design-song-mode.md#finite-values), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and authority

Capture genuine declaration resource inputs before graph lowering erases keywords. Retain opaque issued site records with the exact installed graph Arc for the later isolated-candidate closure phase. Numerical resource controls are never declaration provenance.

ROOT0374 authorized the document split. ROOT0375 subsequently released exactly the four Rust paths below for implementation. The author records fresh exact baselines and immutable intents and holds all sources before the mandatory checker; author Cargo remains prohibited. Existing index/archive/dependencies and unrelated plans stay unchanged.

## Related plans and dependencies

- **Previous**: [Graph materialization](song-mode-graph-materialization.md), independently verified actual host compiler/ownership seam; consuming owner remains pending.
- **Next / consumer**: [Closed graph resources](song-mode-graph-resource-closure.md), phase 2, pins complete named banks and validates frozen owner/site/PCM bindings.
- **Source reference**: historical closure receipts0003/0005 (keyword rejection and namespace-only rollback),0007/0009 (isolated disposal and reviewed ClosedSong contract), ROOT0374 bounded split.

Phase 1 supplies genuine capture and installed-Arc metadata. It does not close assets, implement registry transaction rollback, publish Ready, or complete finite playback/export.

## Exact four-Rust manifest

| Path | Baseline lines | Deliverable |
|---|---:|---|
| `src/dsp/build.rs` | 743 | Capture mode, opaque sites/inputs and additive lowering wrappers |
| `src/ns/insts.rs` | 835 | All resource headers and exact installed-Arc declaration records |
| `src/vm/natives/dsp.rs` | 523 | Actual bus/master lowering mode and record handoff |
| `tests/song_graph_resource_capture.rs` | absent | Genuine declaration, DAG/reversal/replacement/privacy witnesses |

All touched Rust must remain below1000. Existing lower_inst/lower_bus wrappers and Lowering literals keep their signatures. No graph.rs, evaluator.rs, journal.rs, snapshot facade or asset-loader changes in this phase. If build/registry growth needs a cohesive helper, obtain a prior manifest amendment; there is no implicit child module.

## Modules and exact declarations

### Lowering types and opaque issuer — build.rs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphResourceCaptureMode { Legacy, ClosedSong }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphResourceSite {
    Header { parameter: CtlId },
    EmbeddedEffect { node: u16, parameter: CtlId },
    BusEffect { effect: u16, parameter: CtlId },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphResourceInput {
    Bank(KwId),
    UnresolvedNumeric,
    UnresolvedDynamic,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredGraphResource {
    site: GraphResourceSite,
    input: GraphResourceInput,
}
impl DeclaredGraphResource {
    pub fn site(&self) -> &GraphResourceSite;
    pub fn input(&self) -> &GraphResourceInput;
}
// Existing Extras gains a Default-compatible field:
// pub resources: Vec<DeclaredGraphResource>
pub fn lower_inst_with_resources(
    id: InstId, root: &Rc<UGenNode>, header: &[(CtlId, Ctl)],
    lowering: Lowering<'_>, mode: GraphResourceCaptureMode,
) -> Result<(InstDef, Extras), LowerError>;
pub fn lower_bus_with_resources(
    id: BusId, root: &Rc<UGenNode>, lowering: Lowering<'_>,
    mode: GraphResourceCaptureMode,
) -> Result<(BusDef, Extras), LowerError>;
```

Record fields are private with borrowed inspection. There is no public constructor or public mutation API for a site/input pairing. Issuance stays inside genuine lowering/header realization; any crate-private issuing routine must be declared before its implementation. Cloning a genuine immutable record preserves its content, never permits rebinding or inventing an IR bank from zero. Later registry and frozen closure additionally validate exact graph authority.

Existing lower_inst/lower_bus call Legacy mode. Only a fresh candidate registry, explicitly enabled by phase 2 before package/document forms, selects ClosedSong. Ordinary evaluators and existing prelude definitions retain Legacy behavior. Extras has only a definition and Default construction in build.rs; adding its defaulted resource field does not require external literal edits.

Only ClosedSong Convolution `ir` accepts UGenInput::Keyword(bank), records its actual effect site and substitutes a canonical temporary resource placeholder. No global ir control row is introduced. Existing IR keyword rejection in Legacy is retained. Negative constant IR is disabled and creates no resource binding. Nonnegative numeric or dynamic fixed IR is retained as unresolved classification and must fail phase-2 closed admission; never become a bank binding. Existing missing/default negative IR is valid.

Header resource parameters retain every original keyword before default_ctl conversion. The existing InstEntry.resource last-keyword route remains unchanged. sample-play bank/n, wavetable table and granular source UGen arguments remain event-selected and skipped by graph lowering; they are not fixed graph-site resources. UGenInput has no fixed Path/Buffer resource variant, so this phase adds no such language semantics; existing event Path/Buffer behavior is preserved.

Embedded effects record the actual final lowered node index, including shared Rc DAG reuse. Bus chains are traversed backwards then reversed; reverse the site ordinal with the chain. Retain actual effect-local parameter IDs rather than inferring IDs from numerical values. Header/MAX_PARAMS and NODE_CAP bounds remain authoritative; no callback or VM fallback is added.

### Exact installed graph authority — insts.rs

```rust
#[derive(Clone, Debug)]
pub struct InstResourceDeclaration {
    graph: Arc<InstDef>,
    resources: Vec<DeclaredGraphResource>,
}
impl InstResourceDeclaration {
    pub fn graph(&self) -> &Arc<InstDef>;
    pub fn resources(&self) -> &[DeclaredGraphResource];
}
#[derive(Clone, Debug)]
pub struct BusResourceDeclaration {
    graph: Arc<BusDef>,
    resources: Vec<DeclaredGraphResource>,
}
impl BusResourceDeclaration {
    pub fn graph(&self) -> &Arc<BusDef>;
    pub fn resources(&self) -> &[DeclaredGraphResource];
}
impl InstRegistry {
    pub fn enable_closed_song_resources(&mut self);
    pub fn resource_capture_mode(&self) -> GraphResourceCaptureMode;
    pub fn inst_resources(&self, id: InstId) -> Option<&InstResourceDeclaration>;
    pub fn bus_resources(&self, id: BusId) -> Option<&BusResourceDeclaration>;
    pub(crate) fn install_with_resources(
        &mut self, entry: InstEntry, id: InstId,
        resources: Vec<DeclaredGraphResource>,
    );
    pub(crate) fn install_bus_with_resources(
        &mut self, name: Option<KwId>, def: BusDef,
        signals: Vec<SignalInput>, resources: Vec<DeclaredGraphResource>,
    ) -> GraphHandle;
}
```

All metadata fields are private. Public lookup/inspection cannot issue declarations or replace graph authority. Genuine realization alone calls crate-private commit APIs. Keyed records retain the exact installed Arc; lookup must verify current entry Arc identity, not only id, numerical parameters or structural equality. Existing install/install_bus invalidate any previous records. Successful replacement commits graph and records together; old graphs cannot inherit prior bindings.

InstEntry/BusEntry fields and external literal consumers stay unchanged. InstRegistry's sole definition/new constructor are in the owned file. Capturing all headers does not alter the old single-resource event route.

Snapshot/journal currently restore namespace slots, not registry graphs. No ordinary rollback claim is made. Phase 2 uses a fresh isolated evaluator, rejects any failed form and disposes the whole candidate. Acceptance separately proves an independently existing active evaluator/graph is untouched.

### Bus/master real caller — vm/natives/dsp.rs

Existing realization at507–519 must select the registry mode before creating the mutable alloc-cell closure, call lower_bus_with_resources, and transfer issued Extras resources with signals into the genuine commit method. Existing diagnostic staging and failed-inst behavior remain unchanged. Named bus and master share the exact metadata handoff; no side channel or collector outside the manifest is allowed.

## Tasks

### TASK-001: Genuine lowering and opaque records

**Status**: Completed
**Parallelizable**: No

- [x] Implement exact types, readonly getters and Default-compatible Extras field.
- [x] Preserve Legacy wrappers/signatures and ClosedSong-only IR keyword extension.
- [x] Capture headers/embedded/DAG/bus reversal indexes with original keyword authority.
- [x] Preserve negative disabled sentinel, unresolved numeric/dynamic classifications, and event bank/table/granular semantics.

### TASK-002: Registry and actual declaration caller

**Status**: Completed
**Parallelizable**: No (depends on TASK-001)

- [x] Private issued graph records and exact current Arc lookup; stale replacement invalidation.
- [x] Real inst/bus/master realization commits records with the actual installed graph.
- [x] Mode selection only through isolated-candidate opt-in; no ordinary rollback change.

### TASK-003: Real declaration and privacy verification

**Status**: Completed
**Parallelizable**: No (depends on TASK-002)

```rust
fn closed_capture_preserves_original_ir_banks_at_actual_sites();
fn embedded_shared_dag_and_reversed_bus_chain_keep_site_indices();
fn replacement_and_legacy_install_invalidate_old_arc_metadata();
fn legacy_ir_keywords_and_event_resource_arguments_keep_old_semantics();
fn unresolved_fixed_inputs_never_issue_bank_authority();
fn resource_header_bound_and_sparse_dynamic_input_are_admitted_honestly();
```

- [x] Actual Evaluator declarations in explicit ClosedSong mode capture two distinct banks despite identical placeholders, header plus fixed IR, shared DAG and reversed multiple effects, both bus and master.
- [x] Genuine successful replacement and failed realization preserve exact prior graph/records according to actual behavior; no namespace-only rollback assumption.
- [x] Wrong/current Arc and stale declarations cannot pass readonly lookup; external literal construction/private-field mutation compile-fail proof is meaningful.
- [x] Boundary node/header/site admission, huge/sparse rejected inputs and actual graph caps fail without unbounded metadata allocation.
- [x] Native/browser checks, strict Clippy, scoped format/diff, fresh nonempty qualified public/private inventories and actual unfiltered regressions independently pass; preserve failures and original handles.
- [x] Phase-2 handoff exposes genuine opaque records, not closed PCM or fabricated Ready.

## Status and completion

| Module | Status | Evidence |
|---|---|---|
| Lowering capture | Verified | ROOT0385 real declarations, Legacy and opaque issuance |
| Registry/actual caller | Verified | ROOT0385 exact Arc replacement/DAG/site tests |
| Real fixtures/privacy | Verified | Six public tests; joined privacy and regression execution ROOT0385 |

- [x] All phase-1 tasks independently verified within four paths and file bounds.
- [x] Every original capture obligation retained; closed asset/candidate ownership obligations delegated explicitly to phase 2.
- [x] Root accepts source hold and exact evidence before consumer implementation.

## Progress log

### 2026-10-02: ROOT0374 document-only split

The previous eight-path closure combined nine actual facade consumers. Phase 1 owns four capture paths; phase 2 owns six candidate/DTO paths including snapshot.rs registration/reexports. Before intent is SONG-GRAPH-MATERIALIZATION/0010. Original source-erasure, Legacy compatibility, exact Arc authority, isolated candidate failure, complete bank member0, cumulative quota and later union/host requirements remain in the linked plans. No Rust/Cargo/index/archive change.

### ROOT0375 initial source batch declarations
`pub(crate) fn header_resource(parameter: CtlId, input: GraphResourceInput) -> DeclaredGraphResource` issues header records only at genuine Resource-domain default keyword capture. Internal `fixed_resource_input(kind: EffectKind, parameter: &str, input: &UGenInput, mode: GraphResourceCaptureMode) -> Option<(GraphResourceInput, Option<f32>)>` classifies fixed Convolution IR and optional ClosedSong keyword placeholder. `push_resource(extras: &mut Extras, site: GraphResourceSite, input: GraphResourceInput, span: Span) -> Result<(), LowerError>` admits bounded metadata before Vec growth, capped by NODE_CAP plus instrument MAX_PARAMS. Negative finite constants remain disabled. Captured pending embedded inputs finalize at actual pushed node; backwards bus sites reverse with chain. Before intent SONG-GRAPH-RESOURCE-CAPTURE/0001, no author Cargo.

0002 source/fixture batch: combined header plus effect metadata is admitted against NODE_CAP + MAX_PARAMS before append allocation. Genuine public tests use actual explicit ClosedSong Evaluator plus real UGen ASTs passed through the public lowerers (never fabricate opaque records), real shared Rc DAG, bus chain reversal, current registry replacement/legacy invalidation, unchanged disabled/event source semantics and bounds. Private issuance has compile-fail doc coverage; no candidate closed assets or broader rollback claim.

### 2026-10-02: Source-ready held checkpoint (0004)

ROOT0375 implementation is written under immutable before intents0001/0002. Actual source lengths are build901, registry952, VM DSP524, public capture fixtures402; all are below1000. Scoped rustfmt check of these exact four paths returned0. Six public declarations cover original headers and IR banks, actual DAG/reversed effect sites, replacement/current Arc invalidation, Legacy and prelude event resources, unresolved inputs, and header/sparse-input bounds. Two compile-fail doctests retain private issuance. No Cargo or behavioral test has run in this author wave; task verification and acceptance checkboxes remain unchecked. Sources and this own plan are held for the root-coordinated joined mandatory checker alongside the private-delay author. Closed PCM, phase-2 candidate closure and full playback/export remain pending.

### ROOT0377: First joined compile failure and scoped fixture repair

Original checker60286 terminated101 (poll ceef24); native and host-wasm checks passed, but test compilation failed E0618 at the shared-DAG fixture because its local EmbeddedEffect `node` binding shadowed the genuine AST helper `node`. No behavioral tests ran. Immutable intent0005 renames only that binding to `node_index` and its assertion reference; all six tests and assertions remain. Production capture sources are unchanged. Failure evidence is retained at `/tmp/vactr-delay-capture-independent-001/clippy.log` and `final-results.json` (SHA9f328ce0598673b90f8ef603afeac2f44656aa79a0f24bbe6fe38be72c201a4c); renewed independent verification remains pending, with no author Cargo.

### ROOT0386: Independently accepted capture prerequisite

ROOT0385 inspected all50 command gates,906 exact Cargo test names plus4 executed privacy doctests =910 distinct; the joined total includes private-delay and legacy regressions and is not a count of capture-only fixtures. Original2957 terminal0/poll429178; matrix final SHA bab20d86e601908c53cb16fcff9bda6c292c48157893109461bf3a1afe402394. Supplemental originalc3278d terminal0,17 selected actual nextest PASS and1929 unselected SKIP; final SHA fa369ad54b74e5ea61bf9e7f2a0e9c0a2f677b0f4e296aa157ee612fdef80575. All94 cohort hashes matched. Native/browser/strict Clippy/scoped fmt/diff and exact inventories passed. Six capture fixtures and private issuance proof fulfilled this phase; earlier60286/76862 failures remain retained.

**Status**: Completed for declaration capture only.
- [ ] Phase2 consumes these exact records and closes actual fixed PCM under shared work/resource limits.
- [ ] Host preparation owner unions/uploads/remaps real assets; full playback/export remains unfinished.

No Rust/Cargo/index/archive changes in this completion entry; sources remain held.
