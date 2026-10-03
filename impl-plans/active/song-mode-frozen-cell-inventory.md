# SONG-08C Frozen Cell Inventory Implementation Plan

**Status**: Completed
**Created**: 2026-10-01
**Last Updated**: 2026-10-02
**Design References**: [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application), [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)

## Purpose and scope

Certify copied finite control values and logical analyzer destinations before private DSP staging. Audit every owned graph reference without reading active AtomicCells, querying the whole arrangement, expanding Repeat, or retaining mutable registry/VarSlot aliases. This prerequisite depends on independently verified frozen snapshot contracts, not on completion of SONG-08B geometry.

This plan delivers immutable reference inventories and admission helpers. SONG-09/10 still own physical epoch leases, cell transport, private CellRead/FxCtx views, activation, retirement and acknowledgment. No Ready/playback claim follows from this phase. Neutral Some-stage leases, quad-output gating, native/byte staging and actual host capacities remain required downstream.

## Exact future write manifest

Only the eight Rust paths below plus this plan may be written after explicit root release. Planning does not release implementation. Record fresh immutable hashes before each batch; preserve unrelated changes. No dependencies, Git, indexes, archives or other plan writes.

| Module | Exact path | Baseline lines | Expected final ceiling | Status |
|---|---|---:|---:|---|
| Snapshot owner/export | `src/song/snapshot.rs` | 956 | 900 | Completed |
| Cohesive copied routing DTO split | `src/song/snapshot/routing.rs` (new) | 0 | 250 | Completed |
| Immutable cell DTO/admission | `src/song/snapshot/cells.rs` (new) | 0 | 600 | Completed |
| Capture integration | `src/session/song/inventory.rs` | 845 | 875 | Completed |
| Registry-free copied cell capture | `src/session/song/cells.rs` (new) | 0 | 400 | Completed |
| Child module registration | `src/session/song.rs` | 841 | 850 | Completed |
| Public cell fixtures | `tests/song_frozen_cells.rs` (new) | 0 | 750 | Completed |
| Actual candidate regressions | `tests/song_candidate.rs` | 490 | 650 | Completed |

All touched Rust must stay below 1000 lines including scoped rustfmt. Move the cohesive FrozenInstrument/FrozenPart/FrozenEdit/FrozenRoutingInventory/FrozenSource/FrozenAudioRoute/FrozenTrackStage declarations and their routing-only methods from snapshot into snapshot/routing.rs; preserve existing `snapshot::Type` paths with re-exports. Keep query DTOs/FrozenControl copying with their current budget machinery unless a cohesive move is essential. No extra path is implicit.

## Exact declarations

Types live in snapshot/cells.rs and are re-exported through snapshot. Fields of certified inventories remain private; public accessors return immutable slices or copied scalars.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrozenCellOwner {
    Instrument(crate::value::intern::KwId),
    Bus(crate::dsp::graph::BusId),
    Master(crate::dsp::graph::BusId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenCellSite {
    Header { parameter: crate::sched::slots::CtlId },
    NodeParameter { node: u16, parameter: crate::sched::slots::CtlId },
    EmbeddedEffect { node: u16, parameter: crate::sched::slots::CtlId },
    BusEffect { effect: u16, parameter: crate::sched::slots::CtlId },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrozenCellValue {
    pub cell: crate::dsp::cells::CellId,
    pub value: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenCellReference {
    pub owner: FrozenCellOwner,
    pub site: FrozenCellSite,
    pub cell: crate::dsp::cells::CellId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenAnalysisRange {
    pub owner: FrozenCellOwner,
    pub effect: u16,
    pub writer_order: u32,
    pub kind: crate::dsp::graph::AnalyzerKind,
    pub logical_start: u32,
    pub width: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrozenAnalysisBank {
    pub owner: FrozenCellOwner,
    pub slots: u32,
}
#[derive(Clone, Debug, Default)]
pub struct FrozenCellInventory {
    values: Vec<FrozenCellValue>,
    references: Vec<FrozenCellReference>,
    analysis: Vec<FrozenAnalysisRange>,
    analysis_banks: Vec<FrozenAnalysisBank>,
    consumed_work: u32,
    logical_analysis_slots: u64,
}
impl FrozenCellInventory {
    pub(crate) fn capture(
        routing: &FrozenRoutingInventory, remaining: &mut u32,
    ) -> Result<Self, crate::vm::fail::Failure>;
    pub fn values(&self) -> &[FrozenCellValue];
    pub fn references(&self) -> &[FrozenCellReference];
    pub fn analysis_ranges(&self) -> &[FrozenAnalysisRange];
    pub fn analysis_banks(&self) -> &[FrozenAnalysisBank];
    pub const fn consumed_work(&self) -> u32;
    pub fn logical_control_slots(&self) -> usize;
    pub const fn logical_analysis_slots(&self) -> u64;
    pub fn value(&self, cell: crate::dsp::cells::CellId) -> Option<f32>;
    pub fn validate_graphs(
        &self, routing: &FrozenRoutingInventory,
        remaining: &mut u32,
    ) -> Result<(), crate::vm::fail::Failure>;
    pub fn validate_event_controls(
        &self, owner: FrozenCellOwner,
        controls: &[(crate::sched::slots::CtlId, crate::host::wire::Ctl)],
        remaining: &mut u32,
    ) -> Result<(), crate::vm::fail::Failure>;
}
// Additive field, existing Default remains valid only for empty graphs.
pub struct FrozenRoutingInventory {
    // Existing fields remain byte-for-byte equivalent declarations.
    pub cells: FrozenCellInventory,
}
// Owned session/song/cells.rs, callable only by trusted inventory capture.
pub(super) fn capture_frozen_cells(
    routing: &crate::song::snapshot::FrozenRoutingInventory,
    remaining: &mut u32,
) -> Result<crate::song::snapshot::FrozenCellInventory, crate::vm::fail::Failure>;
```

The declaration excerpt for FrozenRoutingInventory is additive, not permission to remove existing fields. The constructor for certified inventory is crate-only; callers cannot forge arbitrary values through public constructors. `Default` permits hostile/empty DTO construction for tests, but validation must reject nonempty graphs with missing coverage. `validate_graphs` verifies equality/coverage against graph contents, not just that some inventory exists.

## Reference and value contracts

- Inspect InstDef.params, InstDef.node_params and every UGenSpec::Effect embedded EffectSpec.params. Template compilation produces header Ctl::Cell, node Src::Cell and effect Ctl::Cell from these source sites; cover each source site without depending on compiled private arrays.
- Inspect every named BusDef and master EffectSpec.params, including neutral empty graphs. Shared Arc identity may cache graph inspection while retaining each owner/site binding.
- Copy scalar values from candidate-copied FrozenInstrument.defaults, never active AtomicCells. Those defaults originate from isolated InstEntry.default_cell_value evaluated header sites. A referenced cell missing from this closed value set fails with addressed BeyondCapability; do not invent zero or evaluate a signal. Equal cell/value aliases are allowed; conflicting scalar aliases, nonfinite values and ambiguous owner graphs fail.
- Existing snapshot capture rejects InstEntry/BusEntry signals. This phase preserves that rejection; it cannot support an unresolved node/bus cell merely because the graph is otherwise finite. Every collected reference must have one certified scalar. A graph that requires a new legitimate scalar source needs prior root contract amendment, not a live table fallback.
- Analyzer outputs are a separate logical namespace from CellId reads. Resolve static analyzer `id` exactly as FxUnit.configure/resolve: schema default, ordered explicit overrides (last matching parameter wins), and copied scalar value for Ctl::Cell. Apply ParamDef.clamp to the finite f32 value (current id range0..65535), then analyzer::process uses max(0) and truncating nonnegative float-to-index conversion. Finite fractional, negative or large raw values retain these representable static semantics; e.g.1.75 becomes1, -2 becomes0 and70000 becomes65535. Checked u32 address+kind width must be representable; use effects::analyzer::cells(kind), not inferred sample data. Existing finite-frozen-input policy still rejects nonfinite or unresolved cell sources, rather than substituting schema NaN default for an untrusted live input. Constant captured ids have equal initial/current smoothing targets; any admitted later value changes require separate bounded address policy downstream.
- Preserve all analyzer writer descriptors in actual node/chain execution order, including identical or partially overlapping same-owner ranges and default id0 writers. Later writers overwrite only the entries they actually write; their state/layout semantics remain unchanged. writer_order records the relative analyzer site order in its owner graph, effect identifies the original node/chain index, and shared graph aliases retain each owner binding without inventing extra same-site writers. Do not reject overlaps, merge writers, split overlapping ranges into independent physical destinations, or promise deterministic ordering between different voice executions beyond the existing engine schedule. Distinct epochs require private storage downstream.
- Event controls from public FrozenSongEvent are resolved immutable FrozenControl scalars/lists and contain no CellId variant. Do not scan the full score for future events. The explicit host-materialized Ctl audit admits only certified owner references, so late/dynamic unknown Ctl::Cell is rejected before queueing; numeric frozen controls need no cell lease. Do not convert an unknown cell to zero. Analyzer `id` event overrides need explicit static inventory equality or refusal before activation; they cannot redirect writes to another bank.
- Inventory retains local candidate cell addresses and owner bindings, not physical host IDs, SnapshotEpoch truncations or leases. Preserve graph identity/defaults/source revisions and all existing frozen query behavior.

## Bounds and privacy

Capture uses the existing Inventory.remaining_work by mutable borrow: one admitted audit budget across all graphs, defaults, aliases, sites, sorting/cache work and output ranges. Charge collection admission before reserve/clone/insert, use checked sizes and arithmetic, and record exact successful consumption. Never reset per graph or family. No recursive pattern/Part walk or arrangement sampling is needed for this graph inventory.

Public validation methods debit their caller's shared remaining quota; clone retains consumed_work and immutable scalar data. Alias reuse must not skip reference bindings or range validation. This phase records logical counts; actual control/analysis slot capacities and overlapping branch-generation storage remain SONG-09/10 admission inputs. No promise that one logical range uses one physical bank across concurrent branches.

Control/analysis capacity handoff: current SongHostCapacities exposes only cell_slots; preparation counts registered defaults. New logical_control_slots counts distinct certified CellId values, and logical_analysis_slots is the checked sum of owner-local bank high-water sizes: each FrozenAnalysisBank.slots=max(logical_start+width) over that owner's writers, or0 for no writers. Same-owner overlap is counted once in storage high-water, writer records remain separate and ordered, and sparse holes remain reserved so existing addresses directly index the bank. This is not a sum of writer widths or a claim of physical host fit. SONG-08/09/10 must separately account actual control and analysis storage per concurrently admitted branch/track/master generation, declare their capacity source, and reserve both before Ready. This plan does not change routing.rs, prepare.rs, carrier capacities or overlap accounting. Downstream passes an owner/epoch-qualified mutable analysis slice of at least bank.slots to FxCtx; logical id0 remains slice index0. No id rebase or sparse compaction is assumed: a different remap must preserve every overlap, address and writer order and receive a prior contract amendment. Until that later contract is implemented, new inventory success certifies finite reference closure only.

## Literal and consumer audit

Fresh implementation-time search must enumerate all struct literals before edits. Current FrozenInstrument construction is in session/song/inventory.rs. FrozenRoutingInventory defaults/builders are in inventory and public tests; active routing/source.rs literal uses `..Default::default()`, so additive defaulted `cells` requires no active08B write. Verify all other literals afresh, stop for prior amendment if any omitted explicit literal needs changes.

Snapshot split re-exports preserve routing.rs, prepare/source/nested/configuration/density/components imports, source-use tests, freeze facade and `SongSnapshot::routing()` callers. No mutation of those active08B paths is authorized. Candidate ownership transfer, set_routing and prepare_song can remain unchanged because cells are carried within FrozenRoutingInventory. session/song.rs adds only child module registration. Complete capture must fill cells before returning success from inventory; failures discard the isolated candidate.

## Tasks

| Task | Depends on | Parallelizable | Status |
|---|---|---|---|
| TASK-001 Declarations/literal audit and root approval | frozen contracts | No | Completed |
| TASK-002 Cohesive snapshot DTO split and immutable types | 001 | No | Completed |
| TASK-003 Complete graph references and copied scalar closure | 002 | No | Completed |
| TASK-004 Analyzer ranges and event-materialization audit | 003 | No | Completed |
| TASK-005 Capture integration under shared work | 004 | No | Completed |
| TASK-006 Genuine public/private candidate and hostile DTO fixtures | 005 | No | Completed |
| TASK-007 Stable author gates and source seal | 006 | No | Completed |
| TASK-008 Independent checker and DSP handoff | 007 | No | Completed |

## Acceptance criteria

- [x] Root approves exact DTO/error/range-collision declarations and manifest before Rust writes (ROOT0183, ordered overlap policy retained).
- [x] Snapshot split preserves public type paths and all legacy contracts; every touched Rust stays below1000 lines.
- [x] Genuine candidate header default/tweak values are copied, graph/header/node/effect/bus/master references are complete and immutable.
- [x] Missing/ambiguous/nonfinite scalar sources reject; native AtomicCells and subsequent active default changes cannot affect copied values.
- [x] Embedded and bus/master analyzer ranges match kind widths and schemas; unresolvable references, address overflow and unadmitted dynamic redirects reject with owner/site diagnostics; finite legacy id normalization and ordered overlapping writes remain valid.
- [x] Genuine source declarations prove valid analyzer/default behavior; crate-local hostile fixtures cover otherwise unavailable cell graph forms without adding forging APIs.
- [x] Default empty inventory succeeds only with empty graphs; forged nonempty inventory/defaults, duplicate owners and missing coverage fail.
- [x] Event Ctl::Cell helper accepts certified ownership and rejects foreign cells/redirected analysis IDs; no eager full-score scan or Repeat expansion.
- [x] Wide/shared graph low-budget tests prove preallocation admission, exact cumulative consumption, one-less failure and clone retention.
- [x] Independent native and host-wasm check, strict scoped/all-target Clippy as coordinated, nonempty public/private candidate+cell inventories, regression and privacy doctests, nextest repeats, scopedfmt/diff all pass with exact current source hashes.
- [x] Mandatory independent checker reports actual terminal exits/counts/log hashes; author gates alone do not complete phase.
- [x] SONG-09 receives immutable local reference/range inventory with full epoch physical remap/overlay/retirement obligations explicitly pending.

## Verification and handoff

Use CARGO_TERM_QUIET=true for every Cargo command; prescribed quiet nextest settings. Fresh binary-qualified inventories prevent double counts. Tests must include actual declared candidate graphs plus missing node/effect/bus cells, same-ID conflicting defaults, shared graph aliases, analysis read/write namespace separation, cross-owner identical analysis address, ordered internal/default-id0 overlaps, partial overlaps and high-water versus width-sum accounting, width overflow, analyzer schema defaults, duplicate explicit override precedence, fractional/negative/clamped ids and cell-backed id, later active/native writes, event materialization and failed candidate preservation.

Coordinate all-author holds before full compilation/lint/test matrices. Retain failures and original terminal handles; never restart due timeout observation. Mandatory checker runs automatically after Rust modifications, via root coordination. No source execution has been authorized by this Planning document.

## Dependencies and related plans

| Dependency / consumer | Status / relationship |
|---|---|
| SONG-06/07 private snapshot/evaluation and frozen defaults | Independently verified baseline |
| SONG-08B geometry | Independent work; active paths remain untouched |
| SONG-09 DSP routing/staging | Future consumer; physical ownership/read/output isolation still required |
| SONG-10 host transport | Future full-epoch cell carrier and actual capacity consumer |

Related: `song-mode-candidate-evaluation.md`, `song-mode-route-preparation.md`, `song-mode-dsp-routing.md`. Preserve full song-mode goal; do not infer playback/Ready from this metadata prerequisite.

## Progress log

### 2026-10-01 — Planning only

Read current copied graph/default, candidate ownership, cell read/write and analyzer contracts. Verified new SONG-08C artifact directory and plan path absent before creation. Recorded immutable0001 baselines; proposed eight-path split/capture/test manifest. No Rust, Cargo, active08B, existing plans, index, archive or Git changes. Root must review before implementation release.

### 2026-10-01 — Root review: preserve analyzer semantics

Document-only0004 revision removes proposed overlap rejection. Copied writer descriptors preserve original node/chain order and same-owner aliases/overlaps. Owner-local bank high-water storage preserves addresses and holes; downstream uses private owner/epoch slices. Static id capture follows schema clamp then nonnegative truncation, preserving finite fractional/negative/out-of-range raw inputs. Corrected dangling related-plan reference. No implementation release, Rust or Cargo.

### 2026-10-01 — Implementation release ROOT0183

TASK001 declarations and literal audit recorded in0005. Active routing literal uses Default; no unowned explicit literal requires modification. TASK002-005 in progress; Cargo held until both authors ready. Per-event CtlId names are header/global control names; analyzer parameters are effect-local indices and must not be interpreted as global event ids. Frozen analyzer cell targets cannot be redirected by event headers; downstream graph updates must recertify inventory.

### 2026-10-01 — Writer-order refinement before code0007

Template.build compiles nodes in build_helpers::topo_order, not raw declaration order. The owned cells helper derives the same fixed-capacity Kahn order, ascending initial zero-indegree queue and raw-edge insertion order, charging all admitted edge/node scans before work. Retain raw effect index separately; writer_order is actual relative execution order. Private fixtures move cohesively into owned session/song/cells.rs to keep formatted DTO implementation bounded.

```rust
pub(crate) fn graph_execution_order(
    graph: &crate::dsp::graph::InstDef, remaining: &mut u32,
) -> Result<[u16; crate::dsp::graph::NODE_CAP], crate::vm::fail::Failure>;
```

### 2026-10-01 — TASK001-006 written, held for joined verification

0005-0007 immutable pre/post artifacts retain source and declaration changes. Snapshot routing DTOs moved coherently with existing type re-exports; actual scoped-format sizes: snapshot871, routing103, snapshot/cells563, inventory847, session/cells327, session/song842, public cells285, candidate511. Snapshot predicted ceiling adjusted900 to retain existing query/control DTO logic. All touched Rust remain below1000.

Capture fills defaulted routing.cells under Inventory.remaining_work. Analyzer order follows actual compile topology queue order; static id clamp/truncation and writer overlaps/holes retained. Ten public cell fixtures, seven private `session::song::cells::tests` fixtures and one added genuine candidate fixture are present. Actual behavioral/Cargo/native/wasm/lint gates have NOT run; source is implemented but unverified. Reverse-index private fixture compares copied writer order with real RawGraph/Template compilation. No active source writes or Cargo handles remain. Root will coordinate joined08D/source hold and mandatory checker. Physical capacities, per-epoch banks/transport and actual DSP playback remain pending.

### 2026-10-01 — Scoped analyzer accumulation repair

Checker30501 terminated101 on Clippy too_many_arguments after focused114 distinct fixtures passed. Immutable0011 records the prior hashes. Private `AnalyzerAccumulation { writer_order: u32, highwater: u32 }` groups per-owner writer/range state; `analyzer` accepts one mutable state instead of two mutable scalar arguments. Each instrument/bus starts at zero; ordering, checked increment, highwater maximum and work admission remain unchanged. No lint suppression or Cargo execution; affected tests require rerun on the held source.

### 2026-10-02 — Independent completion accepted in ROOT0200

All eight implementation tasks and fulfilled metadata criteria are complete. Root accepted `/tmp/vactr-song08cd-gates-002/final-results.json` (SHA256 `8d37469abf047e8d9f0e3d4c47bc4682b8a833881a5e69258a1fb484ead867e5`), original session76287 terminal0, chunk0b30eb. Strict all-target Clippy, scoped formatting and diff checks passed. The post-repair affected cell/candidate suite passed32 tests; earlier focused behavioral evidence contains114 distinct tests. Snapshot privacy passed1 and nextest repeated53 tests across three binaries with zero skipped. These overlapping suites are separate evidence, not an additive distinct-test total. All31 joined current/pre/post hashes and continuation logs were verified. Native and host-wasm production checks passed in the initial matrix; subsequent fixture corrections and the portable analyzer-state refactor were verified through strict Clippy and source review as recorded in ROOT0200.

Retained constructor failures36131/101 and77005/101 and Clippy30501/101 remain historical evidence; numbered0009-0011 repairs resolved them without adding Default traits or lint allowances. Actual copied defaults, analyzer execution order, owner/site binding, finite ID normalization, shared quotas and immutable inventory behavior are verified. Event materialization audits header cell ownership; effect-local analyzer address changes require graph recertification and downstream admission, not reinterpretation of global event control IDs.

Immutable0012 records the document-only completion intent and unchanged eight Rust hashes. The plan remains in active until final integration archival. SONG-09P/09/10 retain physical control/analysis banks, full-epoch transport and reservation, private read/write views, installation, activation and retirement obligations. This completion certifies frozen logical inventory only; full routing geometry, actual DSP playback and export remain pending. No Rust or Cargo changes accompanied this completion.
