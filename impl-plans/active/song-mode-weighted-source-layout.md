# Copied weighted source layout implementation plan

**Status**: Completed
**Plan ID**: SONG-07E
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance) and [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and current evidence

Finite Parts composed through Steps, Hold, lists and repeated steps require exact
source-use geometry to own private configuration state. The runtime layout counts
scalar and rest slots, static Hold weights and repeated copies before normalizing
prefixes and widths. The copied source graph currently flattens lists, removes
non-source slots and discards Hold weights. Source-edge ordinals authenticate uses,
but cannot distinguish the geometry of `[source nil]` and `[source nil nil]`.

Independent source review and the SONG-08B author confirmed this concrete gap.
Copy bounded rational layout metadata from actual frozen inputs; do not infer it
from note onset/tone, clipped query spans or selected-edge counts. This refinement
is source-grounded and has not received a separate Riela review. The accepted
design and full playback/Apply/mute/export goal remain authoritative.

## Manifest

```json
{
  "planId": "SONG-07E",
  "planPath": "impl-plans/active/song-mode-weighted-source-layout.md",
  "dependsOn": ["SONG-07B", "SONG-07C", "SONG-07D"],
  "writePaths": [
    "src/song/source_uses.rs",
    "src/song/source_uses/layout.rs",
    "src/song/source.rs",
    "src/session/song/source_uses.rs",
    "src/session/song/source_uses/layout.rs",
    "tests/song_source_uses.rs",
    "tests/song_source_layout.rs",
    "impl-plans/active/song-mode-weighted-source-layout.md"
  ],
  "sharedPaths": [],
  "ownershipNotes": "Serial continuation of cleared source-use paths; SONG-08B only reads this metadata and owns its separate routing files."
}
```

## Related plans and dependencies

- **Previous**: [Source-use certification](song-mode-source-use-certification.md), independently cleared.
- **Depends On**: Cleared callable and loader semantics; no changes to either are authorized.
- **Next**: [Source route preparation](song-mode-route-preparation.md), then full SONG-08 join and SONG-09 DSP.

| Dependency | Evidence | Status |
|---|---|---|
| SONG-07B | Independent graph/provenance/resource certification | VERIFIED |
| SONG-07C | Callable type and dependency proof | VERIFIED |
| SONG-07D | Actual loader publication | VERIFIED |

## Execution and preservation

Use required rust-coding and independent check-and-test-after-modify agents.
Root releases this exact manifest before edits. Fresh immutable target/design/plan
hashes and intentions precede every batch under `tmp/song-mode-riela/SONG-07E/`.
Reconcile drift by fresh reading; never restore old source copies. Only seven Rust
paths and own plan are authorized. Every touched Rust file stays below 1000 lines;
cohesive core and builder layout helpers provide room. Any additional source or
runtime provenance interface needs an exact prior root amendment or separate plan.

SONG-08B source/component implementation waits for this handoff. Its causal tests
and density work can use disjoint paths only after explicit coordination. Final
author/independent gates require all Rust writers held on exact seals. No broad
formatting, dependencies, lockfiles, Git, index or archive changes. Preserve unrelated
editor/canvas/session work. Cargo and nextest use repository quiet settings; test
stack settings remain unchanged. Retain failed evidence and poll handles to terminal.

## Module and declaration contract

| Module | Deliverable | Status |
|---|---|---|
| src/song/source_uses.rs | Additive typed per-edge layout DTO and bounded certificate integration | Independently verified |
| src/song/source_uses/layout.rs | Checked layout validation and exact affine slot support helpers | Independently verified |
| src/song/source.rs | Shared support mapping consumes copied layout consistently | Independently verified |
| src/session/song/source_uses.rs | Wire actual copied layout; relocate cohesive Steps/list extraction | Independently verified |
| src/session/song/source_uses/layout.rs | Copy normalized offsets/widths including silent slots and Hold/repeat | Independently verified |
| tests/song_source_uses.rs | Preserve and update existing graph/provenance/certification fixtures | Independently verified |
| tests/song_source_layout.rs | Actual weighted/nested/repeated source geometry and partition fixtures | Independently verified |

The author records exact declarations before implementation. Proposed additive
shape (names and placement must be reconciled with actual consumers):

```rust
pub struct FrozenUseSlotLayout {
    pub prefix: Ratio64,
    pub width: Ratio64,
    pub copy_trace_term: Option<u32>,
}
```

Each selected edge retains ordered normalized transforms along its flattened
nested slot path. A repeated-copy transform refers to its existing checked Copies
trace term; copies remain symbolic. Include all non-source slot weights when
computing prefixes/totals. Copy positive static Hold weights exactly. Keep actual
runtime count clamping, empty layouts and zero repeated-copy behavior. Unknown
dynamic timing gets the existing addressed typed reason; no guessed successful
layout. No user VM, mutable source alias or eager score materialization enters DTOs.

Certification and resolution must use the same layout support arithmetic and
validate indices, positivity, rational overflow and copy ordinals before allocation.
Admit all copied nested transforms under work/depth limits, retaining cache-height
proofs and default-stack behavior. Existing full revision/family/closed-resource
authentication remains intact. Weighted layout is metadata, not a replacement
for source-use identity proof or SONG-08B's conservative density admission.

## Tasks

### TASK-001: Exact interface and baseline
**Status**: Completed
**Parallelizable**: No

- [x] Inspect runtime Steps/Hold/repeat semantics and record fresh source hashes.
- [x] Agree exact copied declarations and inverse-support helpers with SONG-08B.
- [x] Record every trace-index adjustment needed by nested symbolic copies.
- [x] Identify any further intrinsic component-context gap explicitly before edits.

### TASK-002: Implement bounded copied geometry
**Status**: Completed
**Parallelizable**: No
**Depends On**: TASK-001

- [x] Copy silent/scalar weights, exact Hold weights, nested layout and symbolic repeats.
- [x] Preserve structural traces and source-policy identities; no note keys or repeat expansion.
- [x] Integrate shared certified/resolved layout support and checked validation.
- [x] Split existing near-limit files cohesively within the exact manifest.

### TASK-003: Author evidence and independent clearance
**Status**: Completed
**Parallelizable**: No
**Depends On**: TASK-002

- [x] Actual source plus one/two rests produce distinct correct rational widths.
- [x] Nonuniform Hold, nested lists, zero/count-clamped Repeat and fractional slots pass.
- [x] Nested symbolic copies preserve independent trace/copy identity and slot order.
- [x] Fast/slow/shift/reflection composition and points/continuations use exact layout.
- [x] Split/reordered query windows cannot manufacture different intrinsic layout components.
- [x] Malformed layouts, dynamic timing, overflow and work/depth bounds reject honestly.
- [x] Existing source-use/default-stack/candidate/callable/loader regressions pass unchanged.
- [x] Native, host-wasm, strict Clippy, scoped fmt/diff and quiet nextest gates pass.
- [x] All final commands/counts/logs/terminal handles and exact hashes sealed.
- [x] Independent stable-source review verifies actual interfaces and behavior.

## Completion criteria

- [x] Copied weighted layouts match actual frozen runtime semantics without expansion.
- [x] Certification and resolver support consume the same validated geometry.
- [x] Seven module deliverables and exact author/independent gates complete.
- [x] SONG-08B receives a documented usable geometry interface; further runtime-context
      requirements, if any, are tracked explicitly rather than asserted supported.
- [x] Full song mode remains active until routing/DSP/hosts/transport/export/UI complete.

## Progress log

### Session: 2026-10-01 — source-grounded weighted layout gap

Runtime `pattern/step.rs79–101/159–196` includes rest/scalar weight, Hold and copies.
Copied builder `session/song/source_uses.rs243–286/296–373` drops those quantities.
The reviewer and routing author independently confirmed missing exact slot geometry.
ROOT0067/0068 record this supporting plan and fresh baselines. No Rust edits or tests
were performed by plan creation; no new Riela acceptance is claimed.


### Author declarations: agreed exact weighted edge geometry

SONG-08B consumer agrees additive `FrozenSourceUseEdge.layout:
Vec<FrozenUseSlotLayout>` ordered outer-to-inner. Each normalized prefix identifies
first copy, width identifies one copy; optional copy_trace_term addresses the same
edge.trace Copies index, whose actual authenticated ordinal selects the slot.
Public helpers in source_uses::layout are:
`slot_interval(edge: &FrozenSourceUseEdge, cycle: i64, actual_trace: &[ProducerStep]) -> Result<TimeSpan, Failure>`,
`map_slot_support(edge: &FrozenSourceUseEdge, window: TimeSpan, actual_trace: Option<&[ProducerStep]>) -> Result<Option<TimeSpan>, Failure>`, and
`map_slot_configuration(edge: &FrozenSourceUseEdge, cycle: i64, source: TimeSpan, actual_trace: &[ProducerStep]) -> Result<TimeSpan, Failure>`.
The last maps authenticated full source configuration support back to output using
exact affine slot geometry, never clipped event note identity. Cycle is explicit
context, not guessed from note onset. Certification uses symbolic first/last-copy
support; resolution uses exact authenticated copy terms. Nested trace insertion
shifts every existing copy index before prefixing new terms; scalar/rest slots
contribute weights but not source edges. Positive Hold weights are exact; runtime
Repeat clamps static integer counts to0..4096 without expanding copies. Pure lists
remain atomic under actual runtime semantics. Shared mapped support handles the
same arithmetic for certification and resolution. Any concrete missing intrinsic
cycle provenance discovered through actual fixtures must be reported before an
additional path/interface amendment. No such extra path is authorized now.

Consumer agreement received directly from rust_filters before source edits.
Immutable0001 records this declaration refinement and current plan baseline.


### Session: 2026-10-01 — copied geometry implementation and focused readiness

Immutable0002/0003 records seven authorized Rust baselines and changes. Shared
source::mapped_edge_support composes existing operation maps with copied normalized
slot geometry identically for certification and resolution. Copy validation,
trace-index shifts, collection/transform precharge and cover-clone costs include
all nested layouts. The core helper uses bounded affine arithmetic and symbolic
copy envelopes; no repeat expansion or score query enters metadata extraction.
Explicit-cycle map_slot_configuration returns exact uncut slot geometry for08B;
cycle context is supplied, never derived from clipped note/source support.

Focused public tests now eight/zero-filtered passed (88613 terminal0,
SONG-07E/tests-fifth.log). Earlier public six plus seventeen cleared source-use
regressions passed together (71315 terminal0, tests-fourth.log). Three private
layout fixtures passed (99067 terminal0, layout-private-first.log): intrinsic
PatNode::Repeat clamp negative/zero/two/5000→4096 with one symbolic edge and actual
query_traced output exact slot matching, static rational overflow, and nested-list
200 normal-stack success/300 DepthExceeded plus tiny-work rejection. No larger
thread stack or lower depth limit was used. Source-use public default-stack tests
remain intact. All main/new files stay below1000 lines.

Retained failures are honest: tests-first/nested-first guessed two transforms for
DSL repeat, but actual repeat native constructs an eager list (natives/list.rs217),
so nested geometry has three transforms. The5000 eager list consumed bounded
analysis work rather than expanding a symbolic PatNode::Repeat. Fixture correction
uses real eager repeat4 separately from explicit intrinsic Repeat tests; no budget
was raised to waive that eager-list rejection. tests-second used first edge for
both distinct actual nested paths; now selects the authenticated resolved edge,
and helpers validate Exact terms as well as Copies. depth-first failed solely on
concurrent unowned routing predicate typo; owner repaired it, depth-second passed.

Known separate runtime provenance limitation is tracked: non-injective Iter cycle
mapping plus long-release continuation union may lose disconnected outer context.
Checker/root received exact source evidence; this phase does not alter unowned time
or query runtime files, reject accepted Iter, or claim supplied-cycle geometry can
recover context from clipped spans. Further bounded context handoff is required
for that consumer case. This does not replace the copied weighted Steps contract.

All own Rust and own plan are held after this readiness entry. Final native/wasm,
strict all-target Clippy, regression/nextest/scopedfmt/diff gates await root's stable
all-author window and exact seal. Independent clearance remains mandatory; phase
status is In Progress and full routing/playback/Apply/mute/export remains active.


### Session: 2026-10-01 — reviewer multi-cycle helper correction

ROOT0074 superseded the unstarted final matrix for a scoped helper correction.
Before envelope/empty-layout early returns, map_slot_support now validates all
supplied typed Exact/Copies trace terms and length using the same composed helper
as single-cycle support. Conservative multi-cycle envelope semantics are unchanged.
Public negative fixture verifies wrong Exact ordinal, wrong copy kind, out-of-range
copy ordinal and missing term, including unweighted edges. Immutable0006 records
helper/test/plan baseline and corrected bytes. Public layout nine plus existing
source-use seventeen passed together (80051 terminal0, tests-sixth.log); no tests
filtered, no changed stack/limits. Final matrix remains suspended until root
reauthorizes the exact refreshed held source set. No other Rust paths changed.


### Session: 2026-10-01 — retained first final matrix and fixture lint correction

ROOT0077 author runner77285 reached terminal101: native and host-wasm passed,
strict all-target Clippy rejected two redundant references in the nested public
fixture (edge was already borrowed). The first matrix stopped before test gates;
all001 logs/ledger remain retained and no pass is claimed for unexecuted gates.
ROOT0078 authorizes removing only those two redundant borrows under immutable0008.
No production source change or lint allowance was used. Updated readiness seal
precedes a separately authorized002 full matrix. All actual six held08B Rust and
its current root-amended plan are captured; future density.rs absence is recorded
rather than guessing a nonexistent components file or pretending it was checked.


### Session: 2026-10-01 — independently verified weighted layout completion

ROOT0081 freshly accepted immutable independent evidence
`/tmp/vactr-song07e-independent-001/final-results.json`, status
`independently_cleared_song07e_only`:
all twelve gates exit0,287 distinct fixtures plus26 separately counted nextest
repeats, with exact source/plan hashes matching the held finalauthor seal.
Author runner64253 terminal0; finalauthor evidence is immutable
`tmp/song-mode-riela/SONG-07E/0010-final-author-seal.json`, gate ledger
`author-final-results-002.json`, and exact nonempty names/counts inventory
`author-final-test-inventory-002.json`. Firstmatrix77285Clippy101 and all other
failure history remain retained. No allowances, raised stacks or lowered limits.

Documentation-only completion is recorded under0011 before/after intent. Seven
cleared Rust paths remain held and unchanged; no index/archive/Git/otherplan edits.
Weighted Steps/Hold/list/symbolic Repeat geometry and supplied-cycle affine helpers
are delivered to08B. The separately recorded non-injective Iter continuation
configuration-context caveat remains08B-tracked, not claimed solved by geometry or
used to reject accepted operators. Fullrouting/playback/Apply/mute/export still
continues under remaining active plans. Completion is strictly SONG-07E scope.
