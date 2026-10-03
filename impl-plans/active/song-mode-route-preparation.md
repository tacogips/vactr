# Symbolic route preparation implementation plan

**Status**: In Progress
**Plan ID**: SONG-08B
**Plan Path**: impl-plans/active/song-mode-route-preparation.md
**Created**: 2026-10-01
**Last Updated**: 2026-10-02
**Design Reference**: [Song routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

**Timing consumer handoff**: ROOT0461 capture verified; exact source/evidence hashes in `tmp/song-mode-riela/ROOT0465-index-timing-consumer-handoff.json`; [Index occupancy](song-mode-index-occupancy.md) released ROOT0464. Connected support, dynamic K/B/N, host readiness and full playback remain unfinished.

## Intent and current evidence

Complete bounded route preparation for finite song playback, Apply and export.
SONG-08 has actual carrier/receipt tests and direct route fixtures, but remains
incomplete for selected-source use covers and partial overwrite policy intervals.
Basic grid routing is verified by ROOT0177. Exact remaining geometry and its
fixtures are delegated to SONG-08D under ROOT0179; full playback remains pending.

## Manifest

```json
{
  "planId": "SONG-08B",
  "planPath": "impl-plans/active/song-mode-route-preparation.md",
  "dependsOn": [
    "SONG-07A",
    "SONG-07B",
    "SONG-07E",
    "SONG-07F",
    "SONG-07G",
    "SONG-07H",
    "SONG-08A"
  ],
  "writePaths": [
    "src/song/routing.rs",
    "src/song/routing/prepare.rs",
    "src/song/routing/source.rs",
    "src/song/routing/density.rs",
    "src/song/routing/components.rs",
    "impl-plans/active/song-mode-route-preparation.md"
  ],
  "geometryChild": "SONG-08D",
  "sharedPaths": [
    "src/song/routing.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/routing.rs",
      "intendedEdit": "Serial ownership from08 to08B for preparation declarations; ROOT0203 yields routing.rs to09PC carrier work, then returns for final admission. No concurrent parent writes."
    }
  ],
  "retiredReadOnlyPaths": [
    {
      "path": "tests/song_audio_wire.rs",
      "sha256": "0535dcef09bb1542b5fc775509a5ffb8ac4b15da16795739dd3105268a5c7a1b",
      "ownership": "Parent SONG-08/root held carrier dependency. Route relocation and staged carrier fixtures finished; no further08B writes."
    },
    {
      "path": "tests/song_route_preparation.rs",
      "sha256": "cb090323d2d0ba1352a057ae91ed8521cbe83c2b3d7c9b64945a5c0b185d3751",
      "ownership": "Parent SONG-08/root held preparation dependency. Existing22 preparation fixtures remain in joined regression gates; no further08B writes."
    }
  ]
}
```

## Related plans and dependencies

| Dependency | Required output | Status |
|---|---|---|
| SONG-07A | Copied root/track/full family/source policies | VERIFIED |
| SONG-07B | Typed independent use covers and nested source provenance | VERIFIED |
| SONG-07E | Exact normalized slot/copy geometry shared by support and configuration inversion | VERIFIED |
| SONG-07F | Structurally validated ordinary source-free Capture/Edit payloads | VERIFIED |
| SONG-07G | Immutable measured certification work including all successful admissions | VERIFIED |
| SONG-07H | Exact static Every/WhenMod/Chunk integer predicates and membership helpers | VERIFIED |
| SONG-08A | Actual one-message host acknowledgment polling | VERIFIED |
| SONG-08 carrier work | Existing POD/receipt contracts, actual partial regression evidence | PARTIAL |

Previous: [Source-use certification](song-mode-source-use-certification.md).
Parent: [Audio contracts](song-mode-audio-contracts.md) remains incomplete until
this child independently clears. The child consumes current08 contracts; it does
not depend on full08 completion, avoiding a dependency cycle. Next:09PC/09P
implement stable prerequisites; full09 acceptance still requires joined08.

## Execution and preservation contract

Use rust-coding agent/standards and independent check-and-test-after-modify agent.
Freshly record immutable accepted design/plan/target SHA-256 before and after
edits under tmp/song-mode-riela/SONG-08B/. Recheck baselines immediately before
writing; stop and reconcile unexplained drift without stale restoration.
Only five Rust paths and own plan remain owned here. Geometry paths are owned
by [SONG-08D](song-mode-route-geometry.md); this parent consumes its completed
interface handoff and retains density/capacity obligations. Extra paths require root
amendment before edits. Each touched Rust file stays below1000 lines, plan below
1000 lines/eight modules/ten tasks. Only owner updates own progress. Parent08
owner serially records shared ownership handoff and adds08B prerequisite; no
concurrent parent Rust writes during08B execution. Preserve failed logs and
all unrelated canvas/editor/session changes. No dependencies, lockfiles, Git,
broad formatting, shared indexes or archive moves. Poll every retained foreground
handle to terminal; observation timeout never triggers restart. Coordinate all
Rust writers before final joined author/independent gates. Quiet Cargo and nextest
settings follow AGENTS.md; default test-thread stack remains unchanged.

## Modules and public declaration contract

| Path | Deliverable | Estimated lines | Status |
|---|---|---:|---|
| src/song/routing.rs | POD/shared capacity/receipt types, route DTOs and prepare reexport | ~380 | In Progress |
| src/song/routing/prepare.rs | Strict compile/admission and symbolic placement/use/region covers | ~800 | In Progress |
| src/song/routing/source.rs | Certified selected-source admission, mapped support and typed event-to-route resolution | ~650 | In Progress |
| src/song/routing/density.rs | Cohesive private checked uniform source configuration density and three relocated fixtures | ~390 | In Progress |
| src/song/routing/configuration.rs | Authenticated intrinsic source scope and affine/weighted mapping orchestration | ~515–850 | In Progress |
| src/song/routing/nested.rs | Active symbolic nested cover discovery/admission, borrowed inherited-frame authentication and inside-out local-offset geometry | ~450–550 | In Progress |
| src/song/routing/components.rs | Borrowed bounded periodic/Chunk/Iterate maximal-component solvers and relocated three unit fixtures | ~300–550 | In Progress |
| tests/song_source_routes.rs | Actual aliases, nested origins, mapped points, closed banks, tails and causal detector fixtures | ~500 | In Progress |

```rust
pub fn prepare_routes(
    snapshot: &SongSnapshot,
    caps: &CapabilitySet,
    available: &SongHostCapacities,
) -> Result<SongRoutePlan, Failure>;
```

Keep existing SongRoutePlan/SongBranchRoute/SongTrackRoute public declarations,
full-width identities and carriers. Record precise additive source-use/region
placement fields in this own plan before implementing them. Route plan retains
immutable topology and certified graph/cover recipes; candidate IDs are not
actual installed host graph/cell/resource IDs. Preparing is not Ready/Applied.

## Acceptance contract

- Strict branch -> distinct track stage -> master topology. Same-name bus supplies
  a track chain; otherwise explicitly allocate neutral intermediate stage.
  Explicit selected bus/FX names resolve exactly. No direct-master fallback.
- Every live branch generation owns private Room/bus state and stereo four-second
  delay even without explicit FX. Include optional chain, actual quarter-slot
  Room partition, delay samples/padding, stage/master memory and graph/cell/PCM/
  sample/ack leases in measured capacity. Shared configured voice pool is validated
  against tier and per-voice memory, never leased twice across Apply.
- Actual remaining capacities account current/staged/retiring resource ownership.
  Checked arithmetic and admission precede activation; reject excess with responsible
  track/family/placement. No unreported graph/orbit storage or callback allocation.
- Consume07B duplicate use edges, exact rational timing/structural maps, typed
  nested source provenance and bounded symbolic placement covers. Unique dependency
  roots do not certify use counts. No whole-score query or repeat expansion.
- Separate configuration intervals and independent use scopes, excluding note/tone
  from branch keys. Notes in one placement/configuration share branch voices.
  Nonadjacent configurations cannot borrow an old draining effect state.
- Resolve FX using authoritative SONG-04 cutoff/onset/source-revision semantics.
  Earlier FX can apply to generated Replace/Overwrite payloads; same-family
  overwrite must not be advertised neutral when query policy inherits plate.
  Later overrides win only as existing policy specifies.
- Partial overwrite retains exact included/excluded onset-support region recipe.
  If actual configurations change across the region, reserve both separated source
  configurations and inserted interval with checked symbolic counts and real
  positive support lengths. Zero-length sides do not create phantom intervals.
  Preserve onset-based retained notes/releases crossing region boundaries; no
  destructive waveform splice or premature voice cutoff.
- Identical adjacent retention is optional. Direct repeats remain symbolic and
  tail overlap bounded; transformed timing never uses unscaled source duration
  as a false minimum. Uncertifiable dynamic source structure fails explicitly.
- Typed event-to-admitted-use mapping remains lossless without parsing flattened
  producer words or trusting hashes/pointer identity. Preserve native/byte carrier
  identities, zero callback allocation and acknowledgment backpressure.

## Certified source and detector split contract

The source.rs helper consumes certified selected-use covers and typed nested origins
to admit mapped configuration support and resolve actual realized events to routes.
Configuration ownership includes the independent source-use identity chain and exact
placement revision; final note handles do not allocate separate effect branches.
The owner records exact additive route-cover references, configurations-per-placement
and resolver signatures in this plan before implementing them.

Sidechain bindings retain explicit target stage (branch ID, track name or master).
Admission includes branch-to-track-to-master audio dependencies and detector
track-to-target dependencies. Valid kick-track input to a bass branch is supported;
own-track private feedback and mutual private-branch cycles reject with addressed
diagnostics. Track-stage detectors consume complete pre-track-effect snapshots.
Reserve causal ordering/workspace for DSP consumption without selecting raw empty
track buffers or substituting callback-partition-dependent prior-block values.

The six-path manifest is a cohesive split within the eight-module limit. Existing
carrier/direct fixtures remain preserved. Root0061/0062 record the amendment under
confirmed author Rust/own-plan hold. No parent08 or future09 source write is released.

## Tasks

### TASK-001: Serial split and exact interface
**Status**: In Progress
**Parallelizable**: No
- [ ] Record fresh hashes and parent08 shared-source handoff.
- [ ] Move cohesive route helper and fixtures without semantic changes.
- [ ] Record exact new cover/region fields and07B consumer agreement.

### TASK-002: Complete policy and capacity admission
**Status**: NOT_STARTED
**Depends On**: TASK-001
**Parallelizable**: No
- [ ] Implement selected-source cover admission and typed route resolution.
- [ ] Correct generated-payload policy inheritance and partial region intervals.
- [ ] Preserve strict topology, private state and shared voice-pool bounds.

### TASK-003: Author behavioral evidence and seal
**Status**: NOT_STARTED
**Depends On**: TASK-002
**Parallelizable**: No
- [ ] Actual direct/family/bus/FX, neutral sibling private memory and Apply capacities.
- [ ] Shared-source fast/slow/stack, nested sources and nonadjacent same-template uses.
- [ ] Same-family overwrite inherited FX and distinct-family interrupted FX tails.
- [ ] Onset/release region boundaries, zero sides, fractional covers and partition invariance.
- [ ] Billion direct repeats without expansion; checked overflow and work/depth rejection.
- [ ] Real stage memory, each insufficient capacity and current/retiring reservations.
- [ ] Native/browser tiers, legacy carrier/receipt regressions and zero callback allocations.
- [ ] All author gates/logs/counts/current hashes sealed and foreground handles terminal.

### TASK-004: Independent verification and parent join
**Status**: NOT_STARTED
**Depends On**: TASK-003
**Parallelizable**: No
- [ ] Stable all-Rust window and independently passing required gates.
- [ ] Root inspects hashes, exact counts/commands/logs and terminal evidence.
- [ ] Own plan Completed; parent08 alone reconciles own acceptance and full seal.

## Verification gates

- CARGO_TERM_QUIET=true mise run check
- CARGO_TERM_QUIET=true mise exec -- cargo clippy --all-targets --no-default-features --features host-native -- -D warnings
- CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm
- Scoped rustfmt and git diff --check, exact line/hash manifest
- Nonzero song_route_preparation, song_source_routes, song_audio_wire, song_source_uses, candidate,
  source-policy, snapshot/freeze, authoritative song_edits and legacy regression suites
- Quiet nextest repeats for route preparation and wire/receipt fixtures
- No empty filtered suite accepted; retain full logs and default stack evidence

## Completion criteria

- [ ] Every module and acceptance contract complete with actual07B integration.
- [ ] Author and independent gates pass on unchanged source seal.
- [ ] Exact counts/logs/terminal handles recorded, no unresolved findings.
- [ ] Parent shared-source handoff reconciled and unrelated work preserved.
- [ ] Own status updated; indexes/archives deferred, full song goal still active.

## Progress log

### Session: 2026-10-01 — bounded route split authorization
Root accepted the exact four-source split requested by08 author before size limits.
Existing partial08 evidence includes23 direct/carrier/receipt fixtures; full08 and
selected-source admission are not claimed complete. Source review also requires
inherited generated-payload FX and partial overwrite interval correction. No Rust
edits or checks were executed by this plan creation.

### Session: 2026-10-01 — serial helper/test split
Immutable TASK-001 intent001 records the exact four Rust baselines and both own plan hashes. Direct preparation math/RouteBuilder moved to routing/prepare.rs; route fixture helper/cases moved to song_route_preparation.rs. POD/shared capacity/receipt contracts and all carrier/receipt fixtures remain in their original paths. Partial overwrite placement adds an exact typed Region{part,span,inside} recipe; separated source intervals are conservatively counted, not per-note graphs. Generated-payload source policy inheritance and selected-source use certification remain pending. First region fixtures retained public-arity/exact-ratio corrections; author-routes-tests005 was blocked solely by transient07B FrozenSound import and is not a successful gate. No final child or parent seal.

### Detector binding declaration and handoff
`SongSidechainRoute {template:BusId,unit:u32,source_track:KwId,source_destination:BusId}` records candidate graph/unit and exact logical track stage. `SongRoutePlan.sidechains` retains these copied bindings. Selected compressor selectors resolve exact declared root tracks; malformed/self/outside-topology selectors fail. Detector inputs are linked-channel pre-track-effect sums, including that snapshot's live/retiring generations; they do not alter branch->track->master audio sums.09/10 must privately remap both compiled template IDs and reserved selector controls to epoch-owned stage identities, with correct staged causality and no cross-epoch detector leakage or silentzero fallback.

### Session: 2026-10-01 — direct support, inherited policies and detectors
Helper/test split compiles. Author-policy-tests003 passed18 retained carrier/receipt cases plus9 real route cases, terminal41289 exit0. Generated Replace/Overwrite payloads now collect earlier source-policy variants by bounded symbolic ancestry; Sequence uses exact touched spans and Repeat visits child support once, without repetition expansion. Partial overwrite retains typed Region included/excluded recipes; positive before/inside/after lengths bound tails, and only the target track receives conservative separated source intervals. Tests cover unmatched-family interruption, same-family inherited FX, zero sides/full span and fractional original release continuations preserving handle/whole/route.

Author-detector-tests001 passed18+11 cases, terminal48013 exit0. Valid declared-track detector metadata works on native/browser capability tiers, while an outside-root-track detector fails. Real insufficient arena diagnostics include bounded track/family/scope/typed placement. Source sizes at this batch were routing382/prepare693/route-test359/wire-test769. No full gates or final seal;07B actual independent cover/provenance certification still pending.

The new actual PCM/default-cell fixture is not yet green: retained attempts corrected public path syntax and an unmatched fixture constructor edit; the plain-var body correctly compiled to constant (zero cells). A declared header default now targets real tweak-backed default cells. The last probe15872 was blocked by concurrently incomplete07B VmQuery call_value argument integration, already reported to its owner. Production and fixture failures remain retained; no success claim or acceptance checkbox is made for that case. Source-use final integration and all author processes must be terminal before independent handoff.

### Session: 2026-10-01 — actual PCM and default-cell accounting verified
Author-capacity-tests006 completed at retained63652 exit0:18 carrier/receipt plus12 direct route fixtures passed. The real closed path sample charges one resource and16 PCM bytes; a declared instrument header default contributes actual candidate cell storage. One-less resource, PCM and cell capacities reject before activation with addressed route diagnostics. This passing batch supersedes the previous pending fixture statement without deleting failed probe history. No author processes remain live. Actual07B selected-source certification is still in progress; no source seal, full author gates, acceptance completion or Ready/Applied claim is made.

### ROOT0062 additive source route and detector declarations

`SongBranchRoute` adds `source_cover: Option<u32>` and
`configurations_per_placement: u64`. Occurrences retain owning finite placement
multiplicity; configurations count potential generations inside that placement,
never note/chord counts. `SongRoutePlan.source_covers` stores
`SongSourceRouteCover { scope_part: usize, track: KwId, cover: FrozenSourceUseCover }`.
`SongDetectorTarget::{Branch(SongBranchId), Track(KwId), Master}` identifies each
actual template instance target in `SongSidechainRoute.target`. Detector admission
checks audio branch→track→master plus detector track→target edges for causal
cycles, while valid cross-track detectors remain supported.

`resolve_route(plan, event, limits) -> Result<SongResolvedRoute, Failure>` returns
`SongResolvedRoute { branch: SongBranchId, placement: PlacementPath, sources:
Vec<SongSourceConfiguration>, configuration: TimeSpan }`; each source configuration
contains its typed `FrozenSourceUseIdentity` and original finite configuration
support. Exact scope placement/use/copy/source-policy/component identity excludes
source occurrence and tone. All notes of one admitted configuration share DSP.
Copied current sound determines actual audio template, typed original source
members determine inherited policies. Certified outer and inherited origins are
matched without parsing producer ordinal encodings, reevaluating callbacks or
materializing the score. Final event routes use authoritative query provenance.

Selected configuration overlap is bounded using checked rational static policy
boundaries and frozen timing recipes, not unscaled source durations. Complete
source covers retain the proof for long symbolic arrangements; live reservations
count finite overlapping configuration components and independent static uses.
Point/support checks, malformed graph diagnostics and all work/depth arithmetic
remain explicit. preparing resources does not install them or issue Ready.

### TASK-006 historical density evidence

The earlier slope/burst model and three fixtures were moved under ROOT0075/0076.
Its exact original declarations,7032/0 focused proof and retained concurrent
failures remain in immutable TASK006/008 intents/logs; TASK017/018 supersede
that ambiguous lifecycle model without dropping those fixtures. Detector proof
63699/0 remains unchanged. Full08B acceptance was never claimed.

### Root amendment: bounded density split

With the author held at TASK-007-layout-hold-001, ROOT0075/0076 authorize a seventh
Rust path, `src/song/routing/density.rs`, for relocation of the existing private
uniform density analyzer and its three fixtures. This leaves source.rs room for
authenticated configuration resolution while keeping every touched file below1000
lines. No new dependency, public test path, parent08 edit or additional runtime
provenance path is authorized. Actual Rust writes remain held until explicit root
release after SONG-07E independent clearance. The full acceptance contract and
original source/layout/context requirements remain unchanged.

### TASK-008 resumed consumer declarations
- SONG-07E independently cleared in ROOT0081: 287 distinct fixtures and26 nextest repeats, all gates0. Its source helpers are consumed read-only.
- `SongSourceRouteCover.live_generation_bound: u64` is the uniform tail-window lifecycle bound for one finite owner. It is separate from `cover.configuration_bound`, which authenticates source placements and does not automatically count output reentry.
- `routing/density.rs` relocates the private `SourceDensity`, exact checked uniform analyzer and three unit fixtures; sibling preparation calls `density::source_density`. No semantic change in this split.
- Prototype selected-source preparation compiles at45075 terminal0 but remains incomplete for current/original sound-policy union and authenticated event component resolution. It neither installs host resources nor issues Ready.

### Root amendment: intrinsic configuration helper

ROOT0092/0093 record the author-held seven-source/plan baseline and release an
eighth Rust module, `src/song/routing/configuration.rs`. Relocate the cohesive
source_scope and affine configuration mapping there, then complete nonlinear
component regeneration and nested policy geometry with shared caller work/depth
accounting. Source.rs retains route/root authentication, shared typed matcher
reservation and detector validation. No note/tone identity or clipped query hull
may substitute for an intrinsic configuration key. Original onset may locate a
proven uncut topology birth; long releases retain that birth configuration.

Actual affine fast/slow fixture passed1/1 (15232 terminal0); direct route suite
passed8/8 (11370 terminal0), including corrected sibling and exact logical depth
checks. These focused results do not clear the child. Nonlinear weighted rests,
A→B→A policy births, full/fractional/reordered continuations, nested origin frames,
closed resource families and full author/independent checks remain required.
All eight touched Rust modules stay below1000 lines. No further Rust path,
dependency, runtime provenance edit, parent completion or SONG-09 release follows
from this amendment. Any additional interface gap requires actual counterexample
evidence and a fresh prior amendment.

### TASK-011: weighted intrinsic birth and verified source-free prerequisite

SONG-07F independently cleared 237 distinct fixtures plus 38 nextest repeats,
terminal process 47368 exit 0 (ROOT0108). Its completed plan and held sources
are prerequisite evidence only. ROOT0109 releases this child's exact eight paths.
The unchanged public plain Iterate long-source route fixture passed 1/1 in
`author-actual-iterator-route-002.log`; this does not prove weighted or nested
configuration ownership. TASK-011 records exact checked residue inversion for
a root Iterate and one normalized weighted slot, charging shared caller work
per residue/candidate without scanning score duration or symbolic repeats.
Original/current onsets locate an intrinsic policy/slot interval and never
become note/tone configuration keys. Candidate exclusion uses inverse source
query coordinates. Actual cycle 4, fractional policy start, reordered queries
and nested policies remain acceptance work; no full readiness is claimed.

TASK-011 concrete progress: five actual weighted route fixtures passed in
`author-weighted-component-002.log`, process 23018 terminal exit 0. Prior first
slot expectations were provisional birth-locator evidence and are superseded
by maximal connected policy components: cycle4 long source [13/4,19/4),
fractional source-policy start9/4 [31/16,9/4), and ordinary source onset6's two
distinct output handles both [23/4,25/4). Direct and count0 Iterate also recover
intrinsic slotcycle6 from projected whole onset3/2. Reordered fractional/full
queries agree. Exact inverse candidates charge shared work, independent of song
duration/repeat/Iterate count; width controls the checked candidate interval.
Positive weighted rest in every full output cycle bounds connected support
regeneration to six slots in three neighboring cycles. No note/tone or clipped
query part is a generation key.

At the nested preparation preflight, certify_source_uses returned
no consumed-work scalar. A proposed read-only cover accounting getter would
permit cumulative preparation admission of nested pre-certified payload covers;
runtime must reuse those covers and authenticate every inherited origin under
its existing caller budget. No upstream API/source edit is made here. Nonlinear
operators beyond the implemented recipes, nested FX policy geometry, closed
family/provenance negatives and full author/independent verification remain
required; this is not child or parent completion.

### Session: 2026-10-01 — nested module manifest trade

SONG-07G subsequently added immutable admitted-work accounting and passed43
focused fixtures (48261 terminal0); its final author/independent gates remain
pending. Nested preparation now follows active admitted source dependencies,
preserves the existing1,000,000 preparation ceiling, rejects exhausted work/depth
before certificate invocation, and uses owning payload local windows. Actual
obsolete selected DynamicRate track replacement first failed (51313 terminal101)
and then passed after wholly excluded payloads were skipped before certification
(51769 terminal0,20 preparation plus12 source-route fixtures). These focused
results do not clear runtime inherited-frame routing.

The author held all eight Rust paths and own plan in
TASK-012-nested-module-trade-hold-001.json, with configuration877 lines and
preparation904. ROOT/0118 records prior exact manifest intent. Root serially
retires tests/song_audio_wire.rs from08B writes and returns it to parent08/root
as an unchanged924-line read-only dependency at its sealed0535dcef hash. Adding
src/song/routing/nested.rs keeps eight future Rust write paths. Earlier wire
relocation/staged-carrier evidence remains historical; no source is restored.

The cohesive nested module owns active symbolic cover discovery, cumulative
preparation guards and their fixtures, then borrowed inherited-frame resolution
with inside-out local-offset policy geometry. Every inherited frame authenticates
its full root revision, track, source family, placement and producer context;
prepared certificates are reused under one shared runtime work/depth budget.
No runtime evaluator/certifier call, quota reset, per-note configuration key or
query-hull support substitution is permitted. configuration.rs retains intrinsic
scope and affine/nonlinear component solvers. Author records exact private
declarations and fresh numbered intents before moving code. Root verifies the
amended plan before releasing the new path; all touched Rust remains below1000.

### TASK-013: nested prepared certificates and borrowed runtime stages

ROOT0118/0119 retired the completed wire fixture from future child writes and
authorized cohesive routing/nested.rs within eight future Rust paths. The old
wire hash0535dcef...c7a1b and all carrier evidence remain preserved for parent08.
Actual source cover preparation uses one1,000,000-work counter across main and
nested certification, deducting SONG07G's measured successful consumed_work.
Discovery starts admitted active selected payloads, follows audible typed graph
edges and selected roots symbolically, excludes fully replaced/overwritten old
track payloads, and certifies owning payload local windows. Zero work/depth fail
explicitly before certification. The first real obsolete selected DynamicRate
regression failed51313 (11pass/1fail); that log is retained. Correcting earlier
main payload admission then passed20preparation+12source-route tests, original
51769 terminal0 in author-nested-preparation-002.log.

Native nested runtime check87453 terminal0. Actual nested public fixture74395
terminal0 (1/1, author-nested-runtime-fixtures-001.log) proves inverse fast/slow
and shifted Sequence local-offset geometry, full/fractional query equality,
foreign inherited opaque-handle/member rejection and shared tiny-work failure.
All frames are borrowed; matchers reserve from one caller budget and carry
ancestor graph/Part depths; inside-out mapping uses separate source/input and
parent-output offsets. Runtime never evaluates or re-certifies payloads.

This is focused progress only. Exact scoped FX policy/cutoff verification,
remaining nonlinear recipes, nested weighted policy/reentry and closed-bank
fixtures, full child author matrix and independent verification remain pending.
SONG07G itself is focused-ready pending its final author/independent matrix;
no full child/parent Ready or SONG09 release is claimed. All sources and own
plan are held after this receipt for the coordinated SONG07G verification window.

### TASK-015: resumed FX verification and static conditional handoff

ROOT0123 and ROOT0134 released the current eight-path author scope after
independent SONG-07G and SONG-07H clearance. Their immutable proof is
`/tmp/vactr-song07g-independent-001/final-results.json` and
`/tmp/vactr-song07h-independent-001/final-results.json`; supporting sources remain
read-only. Earlier focused-only/held notes above are historical, superseded by
these verified handoffs. SONG-07H records307 distinct plus49 nextest repeats.

Original77653 terminal0 proves the four-stage complete identity/reordered
partitions/shared depth fixture. Uniform later FX override admission now requires
one template to cover every potential immediate original member and emitted
member; partial original-family coverage retains inherited variants. Actual
uniform spring-only branch/exact-capacity and complete-bank partial-family tests
pass. Frozen FX replay walks authenticated source roots inside-out, reproduces
original-member selection and Transform revision cutoffs, and compares complete
public family/template routes under shared work/depth. Unfiltered regression
58709 terminal0 passed22 preparation plus14 source-route fixtures.

TASK-015 adds actual ordinary/nested route tampering against another admitted
template, correct-template wrong-family, missing/extra routes, retimed source FX
and reordered fractional positive equality. Original94104 terminal0 passed1
new fixture. Initial typed branch-ID fixture compile6118/101, selector-list
setup failure, and copied Sequence tuple compile49840/101 logs remain preserved.

The coordinated SONG-07H window is finished; author files are resumed. Remaining
nonlinear geometry and actual transformed-current/original-member coverage,
strict author gates and complete independent review remain pending. No Ready,
host installation, full08B or parent08/09 clearance is claimed.

### Serialized component module trade: 2026-10-01

All eight author Rust paths and own plan were held in
TASK-015-components-split-hold-007.json. ROOT0136 records prior exact hashes.
configuration.rs is806 lines; remaining mappings need more room. Retire
tests/song_route_preparation.rs at870 lines as a parent/root-held read-only
dependency, preserving its22 fixtures in joined gates. Author cannot edit it
after this trade. Add src/song/routing/components.rs, retaining eight future
Rust write paths and own plan. Move periodic_component, chunk_component,
iterator_component and their three existing unit fixtures cohesively; retain
intrinsic source scope and affine/weighted orchestration in configuration.rs.
Register private module/helper visibility through the already-owned routing.rs.

Record fresh private declarations and numbered intents before the move. No
algorithm, shared work/depth limit, caller-height semantics or fixture assertion
changes belong to extraction. Every touched Rust file stays below1000 lines.
Root verifies this amended plan and unchanged held Rust before releasing the
new path. Retired wire/preparation tests and all completed upstream metadata
remain held; no full child/parent clearance or SONG09 release follows this split.

### TASK-016 private recipe declarations and current clock contract

The bounded components extraction moves unchanged periodic/Chunk/Iterate
arithmetic and all three iterator fixtures; configuration retains orchestration.
Its proof57538 terminal0 passed3 iterator units plus22 preparation and21
source-route fixtures. First58839/101 dangling-doc-comment extraction failure
is retained and corrected, without algorithm/limit/assertion changes.

Current additive private declarations:

- `components::ClockedPredicate { condition: FrozenStaticCondition,
  transformed: bool, factor: Ratio64, shift: Ratio64 }`: node clock is
  `factor * root_time + shift`; strictly positive affine factors.
- `configuration::AffineSourceRecipe { factor: Ratio64, shift: Ratio64,
  predicates: Vec<ClockedPredicate> }`: owning payload output clock maps to
  selected-root input clock by the final affine relation.
- `affine_source_recipe(cover, identity, trace, &mut ResolutionBudget)
  -> Result<Option<AffineSourceRecipe>, Failure>`: authenticated typed edge
  paths, exact positive Rate/Shift, preserving/content/unit-layout edges and
  static Every/WhenMod constraints; other recipes remain unfinished.
- `joint_periodic_component(predicates, source, owner, anchor,
  &mut ResolutionBudget) -> Result<TimeSpan, Failure>`: shared charged run
  endpoint jumps; initial intersection before any horizon arithmetic; an
  integer multiple of each exact rational root period provides a conservative
  checked/capped horizon. No cycle/residue array or score/repeat expansion.
- `nested::joint_stage_configuration(stages, owner, offset, anchor, budget)
  -> Result<Option<TimeSpan>, Failure>`: project every full immutable cover
  window and source_scope into one root output clock, collecting ALL inherited
  frame predicates before choosing any birth component. Explicit capture/
  sequence offsets are subtracted in each next local owning payload clock.

The anchor is an authenticated birth locator only. Returned configuration keys
contain the uncut intrinsic component and existing full source identities,
never note/tone/onset or clipped event.part/union hull. Source identity vectors
and original_configuration remain unchanged. Matcher and all solver stages
share caller work/depth; no runtime evaluator, query or certificate fallback.

Actual one-graph nested Every3/Every2 held-source failure is retained in
`author-nested-periodic-001.log`; repaired52212 terminal0 passes1 fixture.
Billion-period nested arithmetic passes explicit synthetic capacity fixture
budget1024/tiny1; this is not actual-host-fit evidence. Maximum integer periods
reproduce false early LCM overflow89256/101 and repaired26641/0 passes1 fixture.
Unfiltered7669 terminal0 passed22 preparation plus24 source-route fixtures.

Actual two selected-Part stages reproduced the separate greedy inherited-frame
failure in `author-cross-part-periodic-001.log`. Current15792 terminal0 passes
crossPart0031/1 and outer-affine0031/1 (genuine fast/slow only). Retained22692
fixture Default-limits compile error, invalid `late` fixture and21498 nonexistent
checked_neg compile error are corrected; no new public Shift native is claimed.

Shifted Sequence/Repeat, affine between frames, weighted/multiple-slot and
remaining certified mappings, strict full author gates and independent complete
review remain pending. No full08B or parent08/09 clearance is claimed.

### TASK-016 offset evidence and sampling handoff

Historical TASK016 receipts preserve88005/101,52246/101 and geometry-only
90354/0 under synthetic1m slots/1e12frames, plus62244/0 (6private,22prep,24routes).
This was never actual host-fit evidence. Verified07I supplied scalar Euclid
metadata; exact08B phase/maximal geometry is the next integration below.

## Post-07I density and lifecycle refinement

SONG-07I independent14741 terminal0 cleared373 distinct fixtures and60 repeats.
ROOT0157 accepted metadata only. This amendment retains TASK-002 as the
capacity-admission work unit and TASK-003/004 as author/independent evidence.
All eight Rust paths remain held until ROOT0159 release. No new Riela review
or full routing/playback clearance is claimed.

The exact eight-source future manifest above is unchanged (ROOT0158/0159).
Retired wire/preparation tests and upstream sampling files stay read-only.

### Semantic declaration

Count connected configuration components, including full typed placement/use
identity; do not count notes, tones or individual event onsets. A component
contains its voices. Different original/transformed edges or finite placements
remain distinct unless immutable configuration continuity is explicitly proved.

For any intrinsic output window W, retain three independent conservative proofs:

- K: maximum instantaneous applicable configurations, ignoring retiring tails.
- B(W): configuration starts inside W, with explicit half-open endpoint rules.
- N: finite total connected components across the admitted owning scope.

Components intersecting a positive-length W are bounded by K+B(W), capped by N.
A point window uses K, not the affine burst intercept. A generation live at t
through tail T intersects the closed lookback [t-T,t]; account exact endpoint
conventions conservatively. Tail0 uses K. This counts old held voices/state:
changed configurations stop receiving onsets and retire at the bounded deadline
(design Routing and tails), regardless of nominal held-note whole duration.
Actual acknowledgment-delayed lease reuse remains a09/10 obligation; never
assume a DSP deadline alone has acknowledged physical resource retirement.

### Proposed exact private declarations (no implementation bodies)

In density.rs, replace the ambiguous SourceDensity meaning with:

```rust
#[derive(Clone, Copy)]
pub(super) struct UniformBirthBound {
    slope: Ratio64,
    burst: u64,
}
#[derive(Clone, Copy)]
pub(super) struct ConfigurationBound {
    instantaneous: u64,
    births: UniformBirthBound,
    finite_total: u64,
}
#[derive(Clone, Copy)]
struct DensitySelection<'a> {
    track: KwId,
    original_members: &'a [FrozenSound],
}
struct DensityWalk<'a, 'b> {
    inventory: &'a FrozenRoutingInventory,
    remaining: &'b mut u32,
    max_depth: u32,
}
pub(super) fn source_density(
    inventory: &FrozenRoutingInventory,
    pattern: &FrozenPattern,
    owner: TimeSpan,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<ConfigurationBound, Failure>;
impl ConfigurationBound {
    pub(super) fn components_intersecting(
        self, length: Ratio64,
    ) -> Result<u64, Failure>;
    pub(super) fn live_generations(
        self, tail_cycles: Ratio64,
    ) -> Result<u64, Failure>;
}
fn conditional_partition(
    original: ConfigurationBound,
    transformed: ConfigurationBound,
    condition: FrozenStaticCondition,
    owner: TimeSpan,
    remaining: &mut u32,
) -> Result<ConfigurationBound, Failure>;
fn finite_repeat(
    child: ConfigurationBound,
    child_duration: Ratio64,
    count: u32,
    owner: TimeSpan,
    remaining: &mut u32,
) -> Result<ConfigurationBound, Failure>;
```

The existing DensityWalk::node/part methods gain owning intrinsic support and
selected original-member context. Exactly which member spans survive a partial
Transform must be derived from frozen topology and original-policy provenance,
not emitted instrument alone. No public certificate change or runtime query.

In prepare.rs, introduce a cohesive private lifecycle entrypoint:

```rust
fn lifecycle_generations(
    bound: ConfigurationBound,
    tail_cycles: Ratio64,
) -> Result<u32, Failure>;
```

All count/rational operations checked. Enter/charge uses the same preparation
remaining counter; no fresh1m budget. Before Vec/path/member copies, charge
output and traversal storage. Structural depth includes real Part/graph ancestry
without treating Region bookkeeping as a new Part level.

### Sound conditional composition

Every and WhenMod select one child by the SAME local query-cycle clock. For a
nonconstant predicate, Kout<=max(K0,K1). Mask openings activate at most Ke
child configurations at a boundary, hence:

Bout(W)<=B0(W)+B1(W)+K0*open0(W)+K1*open1(W).

For Every n>1, each role opens once per n cycles; a safe uniform bound is
open_e(L)<=ceil(L/n)+1, additionally capped by exact finite owner run counts.
WhenMod 0<threshold<modulus has the same two-opening structure. Constant
Every/WhenMod predicates admit ONLY their reachable child, without doubling or
cycle_burst. Positive Rate composes the child clock and scales start frequency;
Shift changes phase but not a uniform rate. Genuine identity callbacks still
have distinct original/transformed use keys. No template-name coalescing.

Chunk is NOT generically this exclusive partition: it filters each child's
emitted anchor.frac; retiming may overlap roles or leave gaps. Preserve sumK and
a conservative cycle fragmentation bound until exact copied clocks/anchor
geometry proves a complementary mask. The n-1-per-period maximal-run formula
is valid only for an authenticated fixed-anchor recipe, not arbitrary notes.
Remaining nonlinear mappings retain proved safe bounds and full accepted scope;
this proposal does not waive their geometry or manufacture a smaller resource
count. Refinement is not complete until every admitted operator has a bound.

### Transform and finite placement proof

A full family Transform replaces matching original member streams. Remove
those original configurations from the residual contribution; do not add
original*3+constant1 unconditionally. A partial selector retains UNMATCHED
original members and transformed SELECTED outputs. The selected stream can
contain multiple original members despite one emitted instrument, and can add
generated events with no source origin; include all reachable configurations.
No guessing selector completeness from current sound or family cardinality.

Overwrite differs: original before/after intervals and replacement inside can
create A-to-B-to-A births. Keep exact zero-side and fractional supports; a coarse
three-way split remains only where its bounded proof is required. Deletion may
leave configurations applicable without notes and cannot prove a smaller route.

For Repeat child duration D>0/count N, sum contributions over intersected shifted
child windows, retaining copy scope identity. At most min(N,ceil(L/D)+1) tiles
intersect a positive window; a point intersects at most one. Exact first/last
partial tiles plus a scalar count of full middle tiles avoid expansion. Total
components<=N*Cchild(D). Count Repeat once in the same lifecycle expression,
not both an unbounded mean-density envelope and an extra N multiplier. Adjacent
state retention requires explicit continuity proof; never merge nonadjacent use
scopes. Sequence uses the same shifted-support arithmetic.

### Integration points and removal conditions

1. RouteBuilder::payload currently calls source_density(inventory,payload), then
   C(owner.duration) and C(tail+2). Pass LOCAL owner [0,duration), real remaining
   work and family context. Store finite total and the ONE proved live bound.
2. Existing configurations_per_placement/occurrences remain symbolic identity
   metadata, not an additional multiplier on an already repeated live bound.
3. prepare_routes currently multiplies cover.live_generation_bound by global
   overlap=floor(tail/minDuration)+2. Remove it ONLY for routes whose complete
   outer placement and source configuration lifecycle was included in the new
   bound. For ordinary/direct branches, derive their own finite placement bound;
   do not delete placement overlap without replacement proof.
4. Keep required Room/private delay/optional FX frames per reserved generation;
   fixed shared voice pool is not duplicated. Keep addressed owner failures.
5. Shared matcher/certificate authentication and event-route identities do not
   change. No per-event recertification, query, VM callback or notes-as-branches.

### Existing evidence and honest numeric scope

The private fixture uses base duration4, shifted Sequence duration11/2 and
Repeat2 duration11. Query0..6 does not reduce the admitted score extent.
Synthetic offsets0041/1 proves geometry only; earlier1024-slot failures are
retained. For G total generations/two neutral tracks/no samples:

- template_slots=G; bus_slots=G+3; ack_slots=2G+5.
- At48k, branch stereo delay=384008*G frames.
- Room partition approximately28472 floats per branch/stage.
- bus_frames approximately412480*G+85416.

Thus G1025 entails approximately422.88m float frames (~1.69GB), not host fit.
Exact actual frozen-node totals were not dumped during the held read-only audit.
The minimal direct graph calculation illustrates inflation: constant source
(0,1), Every2 sums to(0,2), cycle_burst3 gives(2,4), fast2 gives(4,4);
Transform adds old*3+1 ->(4,8), outerEvery3 ->(40,80); C(2)*2 reserves320
drums generations at tail0. This is an illustrative exact algebra for that
graph, NOT an asserted dump of the repeated fixture's complete copied graph.

### Required genuine fixture assertions

Add new density private tests within density.rs or private components.rs test
space; source_routes currently896 lines, so prior path trade is required before
any touched file would reach1000. Keep all existing eight paths and fixtures.

- Exact bound declarations tested at point/half-open/fractional boundaries.
- Every/WhenMod degenerates, long held notes, distinct use roles and same-family
  tones, full/partial selectors, generated no-origin outputs.
- Tail0 and positive tails; actual A-to-B-to-A draining overlap, finite Repeat
  count0/1/2/billion, Sequence offsets, before/after overwrite continuity.
- Nonlinear/Chunk retains conservative soundness and quotas; no sparse-note
  heuristic. Full queried handle/configuration vectors unchanged through
  fractional/reordered partitions.
- Same actual candidate and allocations copied from real configured host, with
  current and retiring leases subtracted via after_reservations. Assert every
  required field, generation count and arena bytes fits those MEASURED remaining
  values; no raised synthetic constants or host capability waiver.
- One-below-real-required slots/frames rejects with track/family/scope.
- Tail-positive may legitimately exceed native default6 buses; configure and
  measure a bounded arena in09/10 rather than bypass distinct stages.08B cannot
  claim real installation/lease reuse before those host phases.
- Shared tiny work/depth and overflowing durations/counts fail explicitly before
  allocations. Retain all failures and original terminals; final all-writer
  author/independent gates follow approved source release.

### TASK-017 checked bound algebra implementation declarations

Historical declaration deviation is retained in TASK017001-009; exact methods:

```rust
impl Default for UniformBirthBound { fn default() -> Self; }
impl UniformBirthBound {
    fn at(self, length: Ratio64) -> Result<u64, Failure>;
    fn add_finite(self, other: Self, finite_total: u64) -> Result<Self, Failure>;
    fn scale(self, count: u64) -> Result<Self, Failure>;
}
impl ConfigurationBound {
    fn empty() -> Self;
    fn constant(count: u64) -> Self;
    fn add(self, other: Self) -> Result<Self, Failure>;
    fn scale(self, count: u64) -> Result<Self, Failure>;
    fn fragmented(self, owner: TimeSpan, source_width: Ratio64)
        -> Result<Self, Failure>;
}
fn density_charge(remaining: &mut u32, work: u32) -> Result<(), Failure>;
fn lifecycle_generations(bound: ConfigurationBound, tail_cycles: Ratio64)
    -> Result<u32, Failure>;
```

TASK017 helper history and declaration deviations remain immutable in intents001-009.
ROOT0162 helper proof: original79410 exit0; native/browser and six unfiltered
fixtures passed. ROOT0164 native50001/101 was corrected by import-onlyROOT0165.
ROOT0166/0167 focused integrated retry33066/101 passed native/browser, six density,
eleven components and22 preparation fixtures; routes23/24 exposed reciprocal
slope representation overflow. No full08B or installation clearance is claimed.

### TASK-018 shared bound traversal declaration

```rust
struct PayloadOwner { part: usize, track: KwId, policy_root: Option<usize>, depth: u32 }
fn density_clip(window: TimeSpan, duration: Ratio64) -> Result<Option<TimeSpan>, Failure>;
impl DensityWalk<'_, '_> {
    fn enter(&mut self, depth: u32) -> Result<(), Failure>;
    fn node(&mut self, pattern: &FrozenPattern, index: u32, owner: TimeSpan,
        original_members: Option<&[FrozenSound]>, depth: u32)
        -> Result<ConfigurationBound, Failure>;
    fn part(&mut self, index: usize, selected: DensitySelection<'_>,
        owner: TimeSpan, depth: u32) -> Result<ConfigurationBound, Failure>;
    fn payload(&mut self, payload: &FrozenPattern, selected: DensitySelection<'_>,
        owner: TimeSpan, depth: u32) -> Result<ConfigurationBound, Failure>;
}
```

RouteBuilder::payload owner tuple becomes PayloadOwner, retaining actual walk
Part depth (Region path bookkeeping does not contribute). Its remaining semantic
height is checked256-depth-1 before source_density; graph and selected Part
recursion consume that same allowance. The density entrypoint borrows the real
preparation remaining work after certificate costs and passes it through every
mapped-edge helper/grid mask. Original member selection is derived from genuine
payload.sources and source policy families, narrowed through Part edits; emitted
instruments alone never establish complete Transform selection.

Static grid handling charges enabled-mask work before allocations; zero grid
has no components, full grid avoids artificial gaps but retains child changes,
and sparse enabled runs require cycle fragmentation with seam adjacency.
Unsupported nonlinear maximal geometry remains pending; this bound increment
does not authenticate a phase from an envelope or complete the route resolver.

Additional exact integration declarations:

```rust
fn outer_placement_overlap(inventory: &FrozenRoutingInventory,
    path: &[SongRoutePlacement], track: KwId, tail_cycles: Ratio64,
    remaining: &mut u32) -> Result<u64, Failure>;
fn member_work(members: &[FrozenSound], candidates: &[FrozenSound])
    -> Result<u32, Failure>;
```

`PayloadOwner.depth` supplies checked256-depth-1 to BOTH initial certificate
SongLimits.max_depth and density entrypoint before allocations. Outer symbolic
Repeat uses finite min(count,ceil(tail/childDuration)+1), not an additional
source-density repeat factor; targeted Region reentry remains conservatively
charged for positive tails. Tail0 external factors are one. Source member work
charges checked comparison products and path bytes before filtering/cloning;
Transform pattern explicitly ignores unrelated cutoff with `..`. Sparse grid
run count increases births/finite components only, retaining child point K.

`nested::prepare_nested_covers(inventory: &FrozenRoutingInventory,
active: &[SongSourceRouteCover], remaining: &mut u32)
-> Result<Vec<SongNestedSourceRouteCover>, Failure>` replaces its prior-work
value input with a borrowed remaining counter, committing measured remaining
work back after success. Main certification, density, nested certificates and
outer-placement lifecycle arithmetic therefore spend one cumulative quota.

TASK018 intents006-010 preserve current/original-family generated-output,
byte-aware member work, exact logical payload depth, grid pointK versus births,
and cumulative nested remaining-work refinements implemented above.

Lifecycle relocation and its earlier size deviation are recorded in ROOT0163
and TASK018 intents008-010. Exact helpers remain declared above; components
preserves BeyondCapability diagnostics and checked shared quota arithmetic.

`DensityWalk::carries_source(&mut self, pattern: &FrozenPattern, index: u32,
depth: u32) -> Result<bool, Failure>` shares work/ancestry while checking typed
reachable Source nodes; only validated ordinary DynamicRate/Count/Timing
subtrees may use finite family ownership. No resource/live/callback capability
is relaxed. `fn charge_members(remaining: &mut u32, members: &[FrozenSound],
candidates: &[FrozenSound]) -> Result<(), Failure>` admits lengths before
weight scanning, then byte-aware comparison/output costs before allocation.

### TASK-018 actual admission fixtures

```rust
#[cfg(test)]
fn selected_bound(song: &PreparedSong, remaining: &mut u32, max_depth: u32)
    -> Result<density::ConfigurationBound, Failure>;
```

Actual candidate fixtures cover generated-current-family selection, grid point
occupancy versus run openings, logical depth and cumulative work. Repeat3 with
tail exactly one child duration reserves two half-open logical placements;
physical unacknowledged retirement leases remain separately charged by09/10.
ROOT0164 native50001 failed E0433 after lifecycle relocation: missing FailCode
import in components.rs. ROOT0165 permits the import repair; no behavioural
tests ran. Author hold resumes after correction; no installation claim.

Configured Engine/AtomicCells/SpscRing dimensions will bound the repeated
two-stage candidate in the next fixture. Aggregate fit is partial evidence,
not09/10 private installation, per-slot extents or retiring-lease clearance.

### ROOT0168 finite-owner rate representation repair

```rust
impl UniformBirthBound {
    fn add_finite(self, other: Self, finite_total: u64) -> Result<Self, Failure>;
}
fn scalar_ceiling(numerator: i128, denominator: i128) -> Result<u64, Failure>;
fn period_ceiling(length: Ratio64, period: i64) -> Result<u64, Failure>;
fn ceiling(value: Ratio64) -> Result<u64, Failure>;
pub(super) fn density_charge(remaining: &mut u32, work: u32) -> Result<(), Failure>;
// components.rs relocated helpers, same cost/error contract:
fn member_work(members: &[FrozenSound], candidates: &[FrozenSound]) -> Result<u32, Failure>;
pub(super) fn charge_members(remaining: &mut u32, members: &[FrozenSound],
    candidates: &[FrozenSound]) -> Result<(), Failure>;
```

Compute checked integer bursts/counts independently before rational slope addition.
Only an unrepresentable slope sum falls back to slope0/burstN: all starts in any
window are bounded by admitted finite component totalN. Conditional finiteN is
childN0+childN1+(K0+K1)*(ceil(ownerLength/period)+2), accounting both mask boundary
families and boundary periods. K remains max, not additive for complementary masks.
Birth evaluation and period ceiling use checked i128 products of Ratio64 scalar
numerators/denominators; no unrepresentable intermediate Ratio64 is materialized.
Retain existing overflow regression unchanged; add genuine quarter-duration owner
with a full-width period, and keep all quotas and other overflows explicit.

ROOT0168 finite-rate repair accepted focused64 distinct under ROOT0170;
all12 gates0/original44969 terminal0. Full08B is still incomplete.

### TASK-019 sampled configuration geometry (ROOT0171)

```rust
fn sample_grid_component(recipe: FrozenStaticSampling, source: TimeSpan,
    owner: TimeSpan, output_anchor: Ratio64, budget: &mut ResolutionBudget)
    -> Result<Option<TimeSpan>, Failure>;
fn grid_phase_ceiling(time: Ratio64, divisions: i64) -> Result<i128, Failure>;
fn grid_phase_time(phase: i128, divisions: i64) -> Result<Ratio64, Failure>;
impl ResolutionBudget {
    fn with_remaining<T>(&mut self,
        f: impl FnOnce(&mut u32) -> Result<T, Failure>) -> Result<T, Failure>;
}
```

Mask allocation/work borrow the actual remaining counter, even on failure.
Enabled sample START must belong to full child configuration; clipped output
parts or conservative hulls are never exact membership. Use scalar phase bounds
ceil(n*a)..ceil(n*b), authenticated whole-anchor phase, and maximal enabled run;
full mask has no cycle enumeration. Sparse connected runs join cyclic seams only
within the same admitted source/use/configuration. Clip final run to owner.
Actual fixtures cover masks, source quarter-scope interior point and partitions,
outer fast/slow. Shift has no public native: label its private arithmetic proof
honestly. Nested conditional/grid and weighted joint grids remain full scope.
ROOT0170 accepted focused integrated64 distinct/all12gates, original44969/0;
that evidence does not clear full08B. New author Cargo remains paused.

TASK019 intents001-007 preserve basic grid implementation and independent
whole-cell run oracle. ROOT0177 accepted accounting-corrected80fixtures,
18gates/original2370 exit0; broader sampled composition remains incomplete.

### TASK-019 matcher grid reservation and probe separation

```rust
pub(super) fn reserve_grid_search(recipe: FrozenStaticSampling,
    budget: &mut ResolutionBudget) -> Result<u32, Failure>;
fn search_allowance(cover: &FrozenSourceUseCover, payload: &FrozenPattern,
    position: SearchPosition, budget: &mut ResolutionBudget, probe_work: &mut u32)
    -> Result<(), Failure>;
```

Existing enabled_phases spends probeM on the actual borrowed quota before mask
allocation. Independently reserve matcher2M+5p: direct membershipM+p plus edge
envelopeM+4p (point envelopesM+p). Totalcaller debit3M+5p, plus old structural
reservation; checked probe accumulator is subtracted only from RETURNED matcher
allowance, never refunded to caller. Failed borrowing retains consumed work.
ROOT0174 original2009/0 passed18gates/80fixtures, but reviewer found probeM was
included in returned matcher allowance. That green evidence remains historical;
ROOT0175 corrects probe/admission separation. Existing public26 fixtures stay
unchanged. Real reserve_source_search and core matcher proof uses genuine issued
origin, exact cumulative debit=probe+allowance and one-less total failure.
Nested sampled-context geometry remains pending; no author Cargo is allowed.

ROOT0175 repair written: separate spent probes and matcher reservation; genuine
core matcher/exact caller total/one-less fixtures added. All8Rust+plan HELD,
no author Cargo; nested context remains pending.

### TASK-020 actual inherited conditional/grid regression

```rust
#[test]
fn nested_periodic_sources_use_grid_sample_start_as_distinct_configuration_locator();
```

Genuine slow64 duration4 -> Every2 identity Part -> Euclid1/4 Part:
fullquery four quarter-cell rows, each configuration equals its whole;
cycle2 [2,9/4), interior17/8 point and reordered fractional queries retain
complete handle/route. Actual result awaits checker; no weakened expectation.
Root0178 permits fixture/plan only; production and authorCargo remain held.
