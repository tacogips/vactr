# Static selected-source sampling layout implementation plan

**Status**: Completed
**Plan ID**: SONG-07I
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Proposed language surface](../../design-docs/specs/design-song-mode.md#proposed-language-surface), [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), and [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and evidence

The accepted design includes Euclidean patterns and existing operators applied
to selected symbolic streams. The copied source graph currently represents both
unstructured Sound sampling and Euclid as SampleCycles. Sound samples at an
integer cycle; Euclid samples enabled fractional grid starts. The graph omits
Euclid pulses, divisions and rotation. A single realized slot cannot establish
its neighboring enabled slots or a maximal configuration component.

ROOT0140 records inspected source hashes and this source-derived gap. The
candidate pair euclid p 1 4 versus euclid p 4 4 still requires actual fixtures.
This plan refines the accepted design; it is not a new Riela review. It prepares
metadata. Complete consumer geometry and actual playback remain required.

## Related plans and dependencies

- **Previous / Depends On**: [Static conditionals](song-mode-source-conditionals.md), independently completed SONG-07H.
- **Consumer**: [Route preparation](song-mode-route-preparation.md), SONG-08B in progress.
- **Following**: Parent SONG-08 joined gates, then SONG-09 DSP integration.

| Dependency | Required result | Status |
|---|---|---|
| SONG-07H | Numeric conversion, structural validation, shared accounting | Completed; sources held |
| SONG-08B | Agreed scalar grid contract and held verification window | Agreed; exact eight-path hold verified |

## Exact proposed manifest

```json
{
  "planId": "SONG-07I",
  "planPath": "impl-plans/active/song-mode-source-sampling-layout.md",
  "writePaths": [
    "src/song/source_uses.rs",
    "src/song/source_uses/sampling.rs",
    "src/song/source.rs",
    "src/song/source_uses/source_free.rs",
    "src/session/song/source_uses.rs",
    "tests/song_source_sampling.rs",
    "tests/song_source_uses.rs",
    "impl-plans/active/song-mode-source-sampling-layout.md"
  ],
  "sharedPaths": [],
  "ownershipNotes": "rust_coding receives seven Rust paths and own plan only after explicit serialized root release. All upstream sources remain held now. Routing consumers remain solely SONG-08B-owned. Final author and independent gates require all Rust writers held."
}
```

Planning does not release Rust writes. Before release, root verifies exact held
baselines and the accepted scalar contract. Numbered immutable SHA intents
precede every batch under tmp/song-mode-riela/SONG-07I/. Preserve failed logs and
original process handles through authoritative terminal polling. No additional
paths, dependencies, lockfiles, Git, indexes, archives or broad formatting.
Every touched Rust file stays below1000 lines; additional splits require prior
manifest amendment. Delegate cohesive sampler helpers to the new module before
source_uses.rs approaches that limit.

## Public declaration contract

Reexport the following bounded scalar enum from source_uses and add its separate
mapping variant. Existing SampleCycles retains integer-sampled Sound semantics.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenStaticSampling {
    Euclid { pulses: i64, divisions: i64, rotation: i64 },
}
```

```rust
// Additive variant in the existing FrozenUseMapping enum:
SampleGrid { content: u32, sampling: FrozenStaticSampling },
```

The sampling module provides the following checked internal declarations:

```rust
pub(crate) fn validate(node: &FrozenSourceUseNode) -> Result<(), Failure>;
pub(crate) fn enabled_phases(
    recipe: FrozenStaticSampling, remaining: &mut u32,
) -> Result<Vec<u32>, Failure>;
pub(crate) fn support_envelope(
    recipe: FrozenStaticSampling, window: TimeSpan, remaining: &mut u32,
) -> Result<Option<TimeSpan>, Failure>;
pub(crate) fn contains_sample(
    recipe: FrozenStaticSampling, window: TimeSpan,
    sampled_at: Ratio64, remaining: &mut u32,
) -> Result<bool, Failure>;
```

The owned source.rs edge mapper receives the existing shared work counter;
its three current callers are all in owned source_uses.rs:

```rust
pub(crate) fn mapped_edge_support(
    node: &FrozenSourceUseNode, edge: &FrozenSourceUseEdge,
    window: TimeSpan, actual: Option<&[ProducerStep]>,
    remaining: &mut u32,
) -> Result<Option<TimeSpan>, Failure>;
```

Enabled phase generation consumes the caller's existing remaining-work counter
and admits bounded work/storage before allocating or invoking the existing pure
runtime Euclid algorithm. No callback, source query or certifier execution is
introduced into runtime routing. A support envelope is a conservative hull of
sample starts, not exact connected support. Precise membership additionally
requires an enabled phase whose output cell intersects the original window.
Zero enabled phases yield None. Dispatch this through owned mapped_edge_support
before its existing envelope/slot path; preserve mapped_support's TimeSpan
signature and do not fabricate a point for silence. Certification and matching
must skip an empty grid consistently under the same shared budget.
The unbudgeted mapped_support path must explicitly refuse SampleGrid until its
08B consumer uses a bounded grid recipe, rather than silently treating it as
identity. Metadata completion does not claim arbitrary composed grid phase
authentication. Any exact matcher addition requires a prior declaration, and
must recover the correct grid clock through typed mappings.

## Module status

| Path | Deliverable | Status |
|---|---|---|
| src/song/source_uses.rs | Additive variant/reexport; validation, certification and use-search hooks | Completed; independently verified metadata |
| src/song/source_uses/sampling.rs | Scalar recipe, checked validation, bounded enabled phases and exact support | Completed; independently verified metadata |
| src/song/source.rs | Distinguish grid sample-start support from integer sampling | Completed; independently verified metadata |
| src/song/source_uses/source_free.rs | Structural checks and ordinary dynamic geometry compatibility | Completed; independently verified metadata |
| src/session/song/source_uses.rs | Exact static numeric metadata and addressed dynamic capabilities | Completed; independently verified metadata |
| tests/song_source_sampling.rs | Genuine candidates, malformed DTOs and shared limits | Completed; independently verified metadata |
| tests/song_source_uses.rs | Existing selected-use regressions | Completed; independently verified metadata |

## Behavioral contract

- Copy raw signed runtime integers with the existing int_of(Const) semantics;
  do not replace them with host floating-point conversion or clamp pulses and
  rotation. Preserve negative pulses, i64::MIN absolute handling, and rotation
  direction exactly as the existing runtime algorithm specifies.
- Divisions outside the runtime-supported positive range up to4096 produce an
  explicit addressed diagnostic matching `euclid_steps`:
  `euclid steps must be between 1 and 4096`. Static constants that fail int_of
  produce the runtime Type diagnostic `expected an integer`, rather than a
  false dynamic capability. Dynamic pulse/division parameters remain
  DynamicCount; dynamic rotation remains DynamicTiming for selected sources.
  Ordinary source-free geometry retains SONG-07F compatibility. Unknown
  callbacks, unclosed resources and live input retain their strict rejection.
- Validate Euclid operation, content index and exact existing Child0 entry edge.
  Validate typed DTOs even through ordinary paths. Do not manufacture a
  GeneratedBranch prefix in the source-entry edge: the runtime adds its timing
  producer after sampling, while source entry was recorded earlier.
- Enabled phases match the actual runtime mask, including zero/full pulses,
  complement, rotation and numeric conversion. Work/storage admission precedes
  mask allocation; copied phases and Euclidean arithmetic consume shared work.
  No per-score-cycle or repeat expansion.
- A grid cell samples its start; a point inside it cannot be treated as its
  sampling instant. End-exclusive support and owner clipping remain exact.
  Keep unstructured Sound's integer-cycle sampling separate.
- Conservative envelopes must not authenticate a disabled phase or a phase
  outside the original output window. Membership checks use the grid's input
  clock; do not compare an affine-retimed leaf origin directly to an unrelated
  grid phase. Any additional signature or metadata required for that typed
  mapping must be declared before implementation, within the manifest.
- Preserve final-copy accounting, immutable certification getters and all
  caller-height semantics. No local quota reset or callback reevaluation.
- Consumer density and maximal components belong to08B and require explicit
  integration after metadata clearance. A scalar DTO pass cannot clear routing,
  host installation, transport, export or browser playback.

## Tasks

### TASK-001: Held declaration and metadata implementation
**Status**: Completed
**Parallelizable**: No
- [x] Root verifies held hashes, exact manifest and consumer agreement.
- [x] Record exact helper declarations and immutable intents before edits.
- [x] Implement scalar copying and structural validation within file limits.

### TASK-002: Genuine sampling and compatibility evidence
**Status**: Completed
**Parallelizable**: No (depends on TASK-001)
- [x] Real candidate pair distinguishes 1/4 and4/4 masks.
- [x] Compare enabled phases with existing runtime across rotations/complements.
- [x] Cover static numeric boundaries, dynamic selected rejection and ordinary allowance.
- [x] Malformed operation/content/edge DTOs reject; shared tiny work/depth rejects.
- [x] Reordered fractional/cell-boundary support fixtures prove end exclusivity.

### TASK-003: Author matrix and immutable seal
**Status**: Completed
**Parallelizable**: No (depends on TASK-002)
- [x] Hold all Rust writers and inventory public/private selected tests freshly.
- [x] Quiet native/browser compilation, exact-path format check and focused clippy pass.
- [x] Run unfiltered sampling/conditionals/source-free/budget/uses/layout/provenance/trace and affected private suites, with nonzero inventories.
- [x] Preserve exact commands, exits, complete logs, hashes and original handles.

### TASK-004: Independent verification and consumer handoff
**Status**: Completed
**Parallelizable**: No (depends on TASK-003)
- [x] Independent checker executes the held sealed matrix without edits.
- [x] Root reconciles exact before/after source hashes and real fixture scope.
- [x] Mark only this metadata plan completed;08B must integrate grid geometry.
- [x] Archive/index reconciliation remains SONG-16-owned.

## Completion criteria

- [x] Seven-path implementation and declared signatures match current source.
- [x] Actual runtime mask, sampling support, malformed DTO and quota evidence passes.
- [x] Ordinary and existing selected-source compatibility is verified in focused003 and the complete held author/independent regressions.
- [x] Both held author and independent matrices pass with exact hashes.
- [x] Consumer handoff records unresolved08B geometry explicitly.

## Progress log

### Session: 2026-10-01
**Tasks Completed**: Source audit and proposed seven-path decomposition.
**Tasks In Progress**: TASK-001 prerequisite declaration only.
**Notes**: ROOT0140 and read-only author research establish the omitted Euclid
sampling layout. Cross-Part affine routing repair remains08B-owned. No upstream
Rust writes or Cargo release, runtime reproduction, new Riela acceptance or
complete routing/playback claim accompanies this plan.

### Serialized author window: 2026-10-01
ROOT0144 verifies TASK-016-sampling-window-020-held.json against all eight08B
Rust paths and own plan, unchanged shared07H baselines, and absent new sampling
paths. Original90354/0 proves the shifted/repeated arithmetic fixture;
62244/0 passes6 component,22 preparation and24 source-route fixtures. The
repeated case needs a very large synthetic capacity budget; actual host fit and
admission efficiency remain unproved. No full08B clearance follows.

The consumer accepts distinct scalar grid metadata. ROOT0145 must verify this
Ready plan and release the seven-path specialized author window explicitly.
All08B and other upstream Rust stay held. Existing source_.core/builder sizes
require cohesive helpers to remain below1000 lines. Final author/independent
matrices still require exact current held seals and fresh test inventories.

### Author declaration batch: 2026-10-01
ROOT0145 releases the seven-path implementation. The private helper `fn admitted_mask_cost(recipe: FrozenStaticSampling, remaining: &mut u32) -> Result<(), Failure>` charges checked conservative logical allocation/work requests before runtime mask construction. Scalar group lengths/counts price initial cells/descriptors, pairing copies and possible relocation, split-off descriptors, flattening, rotation, scans and phase writes, plus fixed setup and dry-run iterations. This is not an RSS guarantee. Valid divisions4096 may exhaust the unchanged caller quota. Support envelopes end at the final sampled cell end to include its start under end-exclusive interval semantics; exact enabled-start membership is checked separately. No score-cycle enumeration. Immutable baseline:0001-declaration-before.json.

### Implementation readiness: 2026-10-01
Scalar grid copying/validation and shared-budget edge dispatch are written. Six public genuine candidate tests and four private mask/support/budget tests are prepared, including an extreme single-cycle boundary that must not compute unused neighboring cycles. Focused execution remains pending a root-coordinated held window. No sampling geometry consumer or full routing claim. Records0002/0003 retain before/after bytes.

### ROOT0148 scoped refinement: 2026-10-01
Initial focused001 session34945 terminated101: native/browser checks passed, two public fixture keyword errors (`rot:` versus actual `rotation:`). All24 inputs stayed unchanged and logs/post hashes are retained. The resolver will call the declared `contains_sample` only for a direct SampleGrid→Source child with a realized point `source_part`, using this node current output window in its known grid clock. No handle onset substitution or arbitrary composed-grid authentication. Shared-base sparse/full actual candidates will exercise an authentic disabled-phase negative. Genuine callback pulse/rotation ordinary Capture positives and paired selected failures retain source-free semantics. Prior intent0004 precedes edits.

### ROOT0150 final acceptance declaration: 2026-10-01
Focused002 original40398/0 passes native/browser plus56 public and4 private with all24 bytes unchanged (0005 terminal evidence). The private builder helper `fn static_sampling(k: &PParam, n: &PParam, r: &PParam) -> Result<M, Failure>` preserves exact int_of and diagnoses any known invalid divisions even amid dynamic fields. Direct grid→Source resolution requires a point source_part before enabled-phase authentication. Nonpoint/tampered/outside-window negatives, real source-gap/fractional ordering and checked-mask quota boundary fixtures finish this metadata scope. No arbitrary composed authentication or08B admission claim. Intent0006 precedes the batch.

### Final refinement readiness: 2026-10-01
ROOT0150 numeric helper extraction, strict direct-source point check and acceptance fixtures are written. Eleven public sampling tests and seven private sampler/builder tests are prepared (execution pending). Static invalid n is diagnosed even amid dynamics. Genuine source-gap reordered points, exact quota/one-less, clone/cumulative accounting, actual nonpoint/outside-window rejection and malformed layout are covered. Core976/builder957 stay below1000. Existing initial failures are preserved. No Rust or plan writes after this readiness seal while awaiting coordinated focused/full author matrix.

### ROOT0152 full-matrix readiness: 2026-10-01
Focused003 original52216/0 passes five gates: native, host-wasm,60 unfiltered public tests (budget5/conditionals6/free12/layout9/sampling11/uses17), builder2 and sampler5 private tests. All24 cohort hashes and five logs are retained in0007-focused003-terminal.json. TASK001/002 are complete; TASK003 full13-gate matrix and TASK004 independent verification remain pending. Fixture struct-update cleanup and exact-seven-path formatting follow prior0009 intent. Cleared metadata is not08B geometry or playback; arbitrary composed grid authentication remains a consumer obligation. No full matrix or independent clearance has yet occurred.

### Independent completion and consumer handoff: 2026-10-01
**Tasks Completed**: TASK-001 through TASK-004; SONG-07I metadata only.
ROOT0157 accepts independent original14741 terminal0 with evidence
`/tmp/vactr-song07i-independent-001/final-results.json`, SHA256
`2f0bbc3e78f619fa7f5da93ece8a95f53b7265d82631a20de610266b5e82741e`.
Author original48414/0 passed13 gates; supplement original25211/0 passed
provenance/trace/contracts with fresh inventories. Author seal0011 has SHA256
`5935872ca447cd67cdfda5104be99f8db5912272d3700e795d6871690989b848`.
Root matched all373 binary-qualified distinct fixtures (222 public,52 song
library,64 pattern library,33 supplemental,2 doctests),60 separate nextest
repetitions, all24 pre/post/current verification input hashes and34 independent
log hashes. Root subsequently authorized consumer-plan advancement under
ROOT0158/0159; the completed phase source proofs remain unchanged.
Initial focused34945/101 keyword failure and pre-gate argument-path exit1 remain
retained; corrected focused40398/0 and52216/0 prove the repaired acceptance.

The completed deliverable is bounded immutable Euclid sampling metadata,
structural validation, checked shared-budget support and direct-source point
membership. Conservative envelopes alone do not prove discrete membership.
Arbitrary composed grid-clock authentication, maximal components and density
must be integrated by SONG-08B; routing, host admission and playback remain
pending in the full song-mode goal. Runtime-valid expensive masks can still
honestly exhaust the unchanged caller work quota. Archive/index reconciliation
is explicitly handed to SONG-16; this plan stays in active and no archive/index
operation was performed. Doc-only prior/post evidence:0012.
