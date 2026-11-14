# Selected-source use certification implementation plan

**Status**: Completed
**Plan ID**: SONG-07B
**Plan Path**: impl-plans/active/song-mode-source-use-certification.md
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Song routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and current evidence

Complete finite song composition, playback, Apply, mute and export with private
placement/configuration effect ownership. This prerequisite repairs a concrete
admission gap before SONG-08 can be certified. Resource dependency discovery
correctly deduplicates Rc identities, but that does not count independent uses of
one selected source under fast/slow/stack. Unique selected policy roots remain
resource metadata; a separate copied use graph preserves every parent edge.
Root reviewed the actual source query prefix and immutable DTOs. The author and
route consumer agreed on typed source-entry provenance and symbolic support
covers. This source-grounded refinement has not received a new Riela review.
Existing accepted design and the complete song-mode goal remain authoritative.

## Manifest

```json
{
  "planId": "SONG-07B",
  "planPath": "impl-plans/active/song-mode-source-use-certification.md",
  "dependsOn": [
    "SONG-04",
    "SONG-07",
    "SONG-07A"
  ],
  "writePaths": [
    "src/song/source_uses.rs",
    "src/session/song/source_uses.rs",
    "src/session/song/inventory.rs",
    "src/session/song.rs",
    "src/song/snapshot.rs",
    "src/song/source.rs",
    "src/session/song/freeze.rs",
    "tests/song_source_uses.rs",
    "impl-plans/active/song-mode-source-use-certification.md"
  ],
  "sharedPaths": [],
  "sharedPathNotes": []
}
```

## Related plans and dependencies

| Dependency | Required evidence | Status |
|---|---|---|
| SONG-04 | Full typed producer tracing and selected-source semantics | VERIFIED |
| SONG-07 | Private isolated snapshot and bounded query bridge | VERIFIED |
| SONG-07A | Copied selected root/track/full family policies | VERIFIED |
| SONG-08 | Consumes certified use covers before final route admission | IN_PROGRESS |

Previous: [Selected-source inventory](song-mode-source-route-inventory.md).
Next: [Audio contracts](song-mode-audio-contracts.md). SONG-09 stays gated on
independent SONG-08 clearance. No index/archive edits until SONG-16.

## Execution and preservation contract

Use the rust-coding agent and rust-coding-standards skill. An independent
check-and-test-after-modify agent verifies every completed modification batch.
Before edits, read current targets and record immutable design/plan/target SHA-256
baselines and exact intended changes under tmp/song-mode-riela/SONG-07B/.
Recheck hashes immediately before writing and at final join; never overwrite
unexplained drift. Only the eight Rust paths and own plan are authorized.
Any additional path or file split requires a prior exact root amendment.
Every touched Rust source remains below 1000 lines; plan stays below 1000 lines,
eight modules and ten tasks. No dependencies, lockfile edits, Git mutations,
broad formatting, unrelated canvas changes, index updates or archive moves.
Owner alone updates this plan. Poll retained foreground handles to terminal;
timeout is not completion. Coordinate an all-Rust stable window with root before
joined independent verification. Preserve failed evidence and never waive it.
Cargo commands use CARGO_TERM_QUIET=true. Nextest also uses
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
NEXTEST_HIDE_PROGRESS_BAR=1. Keep default test-thread stack size.

## Modules and deliverables

| File | Deliverable | Expected final lines | Status |
|---|---|---:|---|
| src/song/source_uses.rs | Copied use graph/cover types and checked certification | 650–850 | NOT_STARTED |
| src/session/song/source_uses.rs | Borrowed pattern analysis and pure bounded shape preparation | 700–900 | NOT_STARTED |
| src/session/song/inventory.rs | Attach copied use graphs; retain unique policy inventory | 850–950 | NOT_STARTED |
| src/session/song.rs | Register analyzer and provide certified private construction context | 730–800 | NOT_STARTED |
| src/song/snapshot.rs | Copy typed source origin; relocate cohesive DTOs to approved module | 900–970 | NOT_STARTED |
| src/song/source.rs | Preserve typed source-entry producer prefix | 500–600 | NOT_STARTED |
| src/song/mod.rs | Module registration and exports | Below 1000 | NOT_STARTED |
| tests/song_source_uses.rs | Public-contract, alias and admission fixtures | 650–900 | NOT_STARTED |

## Public declarations

The author records exact refined operation/mapping/cover enum signatures in this
own plan before implementing them. These declarations require no VM, registry,
mutable buffer, Value or original Pat alias in public metadata.

```rust
pub struct FrozenSourceUseGraph {
    pub root: u32,
    pub nodes: Vec<FrozenSourceUseNode>,
}
pub struct FrozenSourceUseEdge {
    pub trace: Vec<FrozenUseTraceTerm>,
    pub child: u32,
}
pub struct FrozenSourceOrigin {
    pub original_instrument: FrozenSound,
    pub handle: EventHandle,
    pub entry_trace: Vec<ProducerStep>,
    pub inherited: Vec<FrozenSourceOriginFrame>,
}
pub struct FrozenSourceOriginFrame {
    pub original_instrument: FrozenSound,
    pub handle: EventHandle,
    pub entry_trace: Vec<ProducerStep>,
}
pub struct FrozenUseDiagnostic {
    pub node: u32,
    pub operation: FrozenUseOperation,
    pub reason: FrozenUseReason,
}
pub fn certify_source_uses(
    inventory: &FrozenRoutingInventory,
    pattern: &FrozenPattern,
    window: TimeSpan,
    limits: SongLimits,
) -> Result<FrozenSourceUseCover, Failure>;
```

FrozenPattern adds source_uses: FrozenSourceUseGraph.
FrozenSongEvent adds source_origin: Option<FrozenSourceOrigin>.
Graph operations must explicitly distinguish Empty/Source, forwarding,
parallelism, cycle choice/concatenation, rational rate, cycle reflection,
within-cycle repeats, structural timing/content composition, constant
iteration/shift, slicing and uncertifiable dynamic structure. Do not collapse
structure-generating operators into a forwarding edge.

## Refined operation, mapping and cover declarations

The graph uses a node record rather than a partially enumerated node enum. The
operation identifies every original PatNode category; mapping describes its
certified configuration-support semantics. Edges are explicit even when their
child node is shared. Indices are checked, never pointer or NodeId authority.

```rust
pub struct FrozenSourceUseNode {
    pub operation: FrozenUseOperation,
    pub edges: Vec<FrozenSourceUseEdge>,
    pub mapping: FrozenUseMapping,
}
pub enum FrozenUseOperation {
    Steps, Pure, Sound, Signal, Source, Fast, Slow, Hurry, Rev,
    Every, WhenMod, SometimesBy, DegradeBy, Maybe, Choose, Hold,
    Repeat, Stack, Cat, FastCat, Superimpose, Off, Jux, Iter,
    Chop, Ply, Striate, Slice, Splice, LoopAt, Fit, Chunk,
    Grid, Euclid, Control, ScaleNotes, Chord, Voicing, Arp,
    Segment, Range, MidiNotes,
}
pub enum FrozenUseTraceTerm {
    Exact(ProducerStep),
    Copies { kind: ProducerKind, count: u32 },
}
pub enum FrozenUseMapping {
    Empty,
    Source { policy: u32 },
    Preserve,
    SelectContent { content: u32 },
    Parallel,
    CycleSelect,
    CycleConcat,
    Rate { factor: Ratio64 },
    ReflectCycles,
    Iterate { count: u32 },
    Shift { amount: Ratio64 },
    Subdivide { count: u32 },
    Restructure { timing: u32, content: u32 },
    Slices { starts: Vec<Ratio64>, splice: bool },
    Conditional,
    Uncertifiable(FrozenUseReason),
}
pub enum FrozenUseReason {
    DynamicRate, DynamicCount, DynamicSource, DynamicTiming,
    UncertifiedCallback, UnclosedResource, UnsupportedLiveInput,
}
pub struct FrozenSourceUseCover {
    // Private checked graph/window/bound; read-only getters.
}
impl FrozenSourceUseCover {
    pub fn graph(&self) -> &FrozenSourceUseGraph;
    pub fn window(&self) -> TimeSpan;
    pub fn configuration_bound(&self) -> u64;
}
pub struct FrozenSourceUseIdentity {
    pub edges: Vec<u32>,
    pub copies: Vec<u32>,
    pub policy: u32,
    pub placement: PlacementPath,
    pub revision: PartRevision,
}
pub fn resolve_source_use(
    cover: &FrozenSourceUseCover,
    origin: &FrozenSourceOrigin,
    limits: SongLimits,
) -> Result<FrozenSourceUseIdentity, Failure>;
```

Resolution validates typed source-entry edges and original placement/revision
against the addressed source policy. It deliberately excludes source note
occurrence and tone. Source-use identity is one component of08's configuration
key; symbolic mapped interval/configuration components are supplied by the graph,
source topology and exact timing map, not by assigning a graph to each event.

`certify_source_uses` validates the graph and computes a checked conservative
configuration reservation using symbolic source placements and operation-specific
mapping components; it retains the graph/window recipe for08 rather than expanding
intervals or repeats. The bound includes duplicated incoming edges and every
possible changed configuration component. Unknown dynamic structure gives a
specific diagnostic; it never yields a successful finite guessed bound.

All origin prefix and opaque-handle vector copying is charged before allocation
against the same existing shared query/DTO budget. Graph analysis and certification
use one checked work allowance, depth policy and identity/cycle cache; incoming
edge multiplicity is counted independently of unique-node memoization. Callback
shape preparation borrows only the certified private frozen candidate context,
rejects output/mutation/capabilities, and validates closed returned dependencies.

## Scope and acceptance contract

- Preserve duplicate parent edges with cached unique child nodes. No occurrence
  or placement expansion, complete-score queries, callback music realization or
  pointer/hash authority. Symbolic Part Sequence/Repeat and use mappings remain
  bounded with checked scalar multiplicity and integer/rational arithmetic.
- Preserve exact typed source-entry prefix before source_trace framing. Copy it
  and the certified original handle through ordinary transforms and public query
  DTOs. Charge every trace/handle/cover collection against the existing shared
  work budget before allocation; honor caller depth and normal stack limits.
- Generation keys stop at use/placement/configuration interval. Notes and chord
  tones in the same admitted configuration share branch voices. Independent
  transformed uses and nonadjacent changed scopes retain distinct ownership
  through tail retirement. Compact NodeId hashes never certify equality.
- Constant fast/slow/hurry, rev, stack, cat/choose/fastcat, constant hold/repeat,
  iteration/shift, filtering, controls, subdivisions and structure-giving
  operators retain their actual timing/content semantics. Covers must account
  for rate/reflection boundary splits and overlapping old private tails.
- Ordinary eager pure function construction is supported by analyzing its actual
  returned Pat tree. Distinguish control/filter callbacks from structure/source
  producers. Lazy pure transform shape preparation must use certified frozen
  construction restrictions and bounded work without querying events or allowing
  mutation, output or external capability access. Returned shape must retain
  closed asset/family certification; no post-freeze unseen mutable resource.
- Truly unbounded or uncertifiable time-dependent structure receives a specific
  copied diagnostic and explicit preactivation failure. Never guess a multiplier,
  use deduplicated resource count as admission, or blanket-reject ordinary
  function composition. Failures leave active snapshot and leases intact.
- Route consumer SONG-08 must validate actual event-to-cover mapping with typed
  provenance, without reparsing flattened producer ordinal words. The author
  obtains route-consumer agreement on final signatures before source edits.

## Tasks

### TASK-001: Exact declarations and source baseline
**Status**: Completed
**Parallelizable**: No
**Deliverables**: Own plan signatures, immutable intents and consumer agreement.
- [x] Freshly inspect accepted design and all actual operator semantics.
- [x] Record complete graph/cover/mapping declarations and exact identities.
- [x] Confirm all paths, default-stack bounds and aggregate admission counters.

### TASK-002: Copied graph and provenance implementation
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No within this plan
**Deliverables**: Seven owned non-test modules and author checks.
- [x] Implement operator-specific metadata and bounded shape certification.
- [x] Preserve duplicate uses and lossless typed origin in query DTOs.
- [x] Keep private snapshot, asset closure and allocation/work invariants.

### TASK-003: Behavioral verification and immutable seal
**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No
**Deliverables**: Focused fixtures, complete logs, terminal handles and source seal.
- [x] Shared-source fast/slow stack and nested aliases have distinct use paths.
- [x] Nonadjacent same-template source placements and symbolic repeats bounded.
- [x] Eager nested pure functions and lazy control/filter transformations work.
- [x] Uncertifiable source-producing callback fails before activation explicitly.
- [x] Multiple notes/chord tones share configuration; typed event-cover match exact.
- [x] Alias DAG memoization, cycle/overflow/collection pre-admission covered.
- [x] Normal-stack depth200 success, depth300 typed rejection and caller depth honored.
- [x] Source-origin copies charge shared query budget before allocation.
- [x] Full author seal matches current hashes and no process remains running.

### TASK-004: Independent join and progress
**Status**: Completed
**Depends On**: TASK-003
**Parallelizable**: No
**Deliverables**: Independent results and own plan completion record.
- [x] Independent checker reruns every required gate on stable joined sources.
- [x] Exact source hashes/counts/logs and terminal status inspected by root.
- [x] Mark only this plan Completed; preserve unrelated work and defer indexes.

## Verification gates

All selected suites execute nonzero fixtures. Keep full logs and exact commands.

- CARGO_TERM_QUIET=true mise run check
- CARGO_TERM_QUIET=true mise exec -- cargo clippy --all-targets --no-default-features --features host-native -- -D warnings
- CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm
- Scoped rustfmt check for exactly owned Rust sources and git diff --check
- Focused song_source_uses, song_candidate, song_assets, source-policy, private
  freeze/snapshot and affected legacy session/query/provenance regression suites
- Quiet nextest repeat for song_source_uses with exact selected fixture count
- Privacy compile-fail and callback-free metadata proof where applicable

## Completion criteria

- [x] All module and acceptance contracts implemented with exact consumer agreement.
- [x] Required author and independent gates pass on sealed current sources.
- [x] No running foreground handles; all counts and failures retained accurately.
- [x] Typed mapping and reservations cover accepted composition without guessed bounds.
- [x] Own status/progress updated; no full-song completion claimed at this boundary.

## Progress log

### Session: 2026-10-01 — source-grounded plan creation
Root and both Rust owners inspected actual source identity and route contracts.
Unique dependency discovery cannot certify independent use multiplicity. This
plan authorizes the isolated metadata/provenance repair; SONG-08 direct routes
may proceed independently, while final source routing and SONG-09 stay gated.
No Rust edits or tests were performed by this document creation.

### Session: 2026-10-01 — exact declaration preparation
Required Rust agent/standards/security and current source semantics were read.
TASK-001 is active, with complete operation/mapping/cover declarations recorded
under immutable doc-only intent0001 before source implementation. The08 consumer
confirmed duplicate-edge cover admission and branch identity excluding note/tone.
mod.rs ownership is released at SHA585a4ae7c133f77fe853c8483801ccba3d9ff93c611143133e787bdf51e9fef7;
07B holds it for serial registration. No Rust source changes yet. Final refined
signatures are being supplied to08 before source edits; author and independent
verification remain pending.

### Session: 2026-10-01 — nested typed origin refinement
08 explicitly accepted the complete refined mapping/cover interface before Rust
writes. Root source review identified that query_source overwrites an already
present inner selected-source origin after nested_part. Retain the immutable
inner origin privately instead, and copy a flat outer-to-inner inherited frame
vector into the DTO. This keeps all typed source-entry edges without recursive
public Rc/Box aliases or flattened-word parsing. Each handle/prefix/frame is
pre-admitted against shared work and caller depth; iterative copying avoids
heavy recursive stack frames. Add nested selected-source inherited-FX roundtrip
and exact event-to-use chain fixtures. The exact eight source paths suffice.
Intent0002 recorded no source mutations; this declaration amendment is doc-only
intent0003.

### Session: 2026-10-01 — compile-preserving graph/provenance batches
Intents0004/0005/0006 implemented public metadata, serial module registration,
shared-budget iterative nested-origin copying, cohesive DTO relocation and the
initial source-use graph attachment. Native check-first12504 and check-third8820
are terminal0; missing relocated FrozenSound name in second63433 terminal101 is
retained and corrected. The generic operation arm is explicitly unfinished;
no certification success or full gate completion is claimed.
Actual step.rs layout requires symbolic typed Copies trace terms for
NestedStep/GeneratedBranch repetitions. Generic resource-children ordering is
being replaced by per-operation semantics. Control/Chord source-entry prefixes
use Child0/1 before final TimingSource/ContentSource merge; source-use matching
must not invent merged roles. New SelectContent distinguishes control-only
sampling from actual structure replacement. Intent0007 records these refinements
before their Rust changes. Complete analyzer, callback certification, reservation
math, mapping fixtures and independent verification remain pending.

### Session: 2026-10-01 — original source sound identity
Root and08B consumer require copied FrozenSound original_instrument on every
outer/inherited origin frame. Current event sound can be transformed independently
of certified original source family; source::original_sound governs selection/FX.
Immutable scalar/buffer-ID/path copying is charged with handle/prefix/frame work
before allocation. No mutable Sound/SampleBuf alias enters public metadata.
Add changed-current-sound and multi-member selected-family mapping evidence.
This additive own declaration change is recorded by doc-only intent0009 before
Rust changes within the existing authorized source_uses copy_origin batch.

### Author refinement: source-local realized support

Every `FrozenSourceOrigin` and inherited `FrozenSourceOriginFrame` additionally exposes `source_part: TimeSpan`, copied from the successful source realization before outer transformations. This is immutable realized support, including retained releases and points; it is not a generation key or a substitute for opaque placement identity. Resolution intersects this support with the operation-mapped cover and validates source topology. Copied support fields spend the shared whole-query DTO budget. The SONG-08 consumer accepted this additive contract; no new module is required. `SampleCycles { content: u32 }` explicitly identifies sound sampling's content edge and conservative source-cycle support.

### Author refinement: borrowed inherited-frame resolution

`pub fn resolve_source_use_frame(cover: &FrozenSourceUseCover, origin: &FrozenSourceOriginFrame, limits: SongLimits) -> Result<FrozenSourceUseIdentity, Failure>` uses the same admitted typed trace/support matcher as outer-origin resolution. A borrowed internal view shares the implementation without cloning an origin/handle or creating a synthetic outer frame. The existing opaque handle authenticates its source-root placement; full exact revision/track/family matching and mapped source support are required. No expanded placement table or mutable policy alias is introduced.

### Session: 2026-10-01 — partial public boundary verification

TASK-001 is completed with consumer-approved declarations and immutable baselines. TASK-002/003 remain active; this is not a final author seal. Actual public candidates, copied metadata certification, query DTOs and typed origin resolution pass in public-tests-twelfth.log (71195 terminal0, 11/11) and public-tests-thirteenth.log (51029 terminal0, 12/12, one explicitly filtered production eager-depth fixture). Accepted profiles include static rates/reflection, shared stack/cat/choose/fastcat semantics, repeated steps, Hold/Repeat, Grid, Euclid, controls, slices/splice, fit/loop, prepared Superimpose/Off/Chunk/Every, and the actual sound wrapper. Invalid initial Every order was corrected to subject-first; invalid direct Sound segment was corrected to signal-segment gain composition. Historical failing logs are retained.

Nested public origins resolve outer and inherited frames, with full PartRevision inventory deduplication and shared generation identities across notes. A billion-count symbolic source repeated into a one-cycle fast2 window reserves exactly two touched placements without expansion. Authentic source support outside the mapped cover is rejected; points and continuations use copied source-local support rather than original onset alone. Cache keys include original node identity, exact mapped window, track (Part policies), and depth; duplicate incoming edges add cached bounds independently. Copied-recipe default-stack depth200 succeeds/depth300 explicitly fails, and a compact twelve-level diamond reserves4096 within max_nodes500; invalid indices and cycles reject (76681 terminal0, 1/1).

The intact production eager nested-function depth fixture currently fails before traversal with an AnyNotNarrowed candidate diagnostic; coordinated SONG-07C owns the disjoint checker correction. The failed direct depth commands/logs are retained. Remaining work includes production eager depth/diamond success, complete original-family/closed-callback and nonadjacent-scope cases, stricter/overflow/admission regressions and full author/independent gates. Support helpers and provenance copying were relocated cohesively to existing source.rs to retain every touched module below1000. A partial unchanged-source verification window can now be offered; no full07B clearance is claimed.

### Author refinement: actual closed callback resource classification

Private `prepare_shapes` borrows `&mut PinnedSongAssets` synchronously after close; this capability only performs existing closed `SampleLoader::load` lookups and never reopens preparation or disk. Returned sound keywords resolve through the certified frozen sound kit and candidate `InstResolver`, including complete family lists and instrument IDs. Every resulting sample path/bank/buffer route must exist in the closed inventory. Buffer/source pointer closure remains exact; all family/work collections are admitted before allocation. Unknown late families yield `UnclosedResource`; raw syntactic keyword membership alone is insufficient. No public alias or new asset-module method is added.


### Root amendment: bounded snapshot pattern copying

The author explicitly holds all07B Rust and plan writes and reports no live Cargo
process. Root trades src/song/mod.rs for src/session/song/freeze.rs in FUTURE writes,
keeping exactly eight Rust modules. Prior legitimate module exports remain intact;
mod.rs receives no further writes. The earlier module table records historical
completed exports and does not grant future edit permission. ROOT0053 records the
fresh exact prior hashes and source manifest trade. No new dependencies or source
files are authorized.

Intact production eager200 construction now passes all four form checks/evaluations
and asset dependency discovery, then SIGABRTs at namespace freeze on the default
thread stack. Retained depth-tests-fifth.log9794 terminal101 is authoritative stage
evidence; this is not a type waiver or cover/query failure. Independent read-only
inspection confirms recursive pattern->value->cached->pat_inner->pat_unary->pattern
uses several large Rust frames per layer before the advertised256-depth guard.

Implement bounded explicit copying for the unary pattern spine, or an equivalently
bounded pattern visit/finish work stack, preserving every node's enter/depth/work
accounting, complete Rc cache and visiting/cycle invariants, pre-allocation admission,
PParam payload copying, node id/span/structured and reverse reconstruction/cache
publication. Keep exhaustive complex/vector behavior and exact semantic depth.
Do not widen thread stack, lower accepted depths, flatten away meaningful operators,
waive cycles, bypass memoization or replace actual candidate tests with copied DTOs.
Record any exact declaration before code. Native and wasm paths must both use the
correction. Every touched Rust source stays below1000 lines; any split needs prior
exact amendment.

Retain actual candidate200 success and300 typed DepthExceeded, actual post-freeze
named callback/notes and shared-DAG/private-resource evidence. Fresh callback-copy
identity invalidates old Closure metadata conservatively; do not blindly re-certify
or copy stale stamps. If later checking requires certificates after copying, establish
that actual failure and obtain an exact scoped metadata handoff before extra edits.
Remove temporary production/fixture stage diagnostics before final author seal;
failed stage logs remain. Complete full07B/07C author seals and independent gates
under all-Rust hold before08B integration; this amendment is not phase clearance.


### Session: 2026-10-01 — production depth and closure author acceptance

ROOT0053 traded mod.rs out of future writes for existing freeze.rs before edits,
retaining eight owned Rust modules. Module exports remain unchanged. The intact
ordinary named-function depth fixture passed type checking after SONG-07C; stage
runs on the normal test stack then identified namespace payload freezing as the
SIGABRT location. All failed logs remain. Explicit unary-spine copying preserves
original Rc identity, per-node enter/depth/work, pre-admitted pending storage,
cache/cycle guards, side parameters, id/span/structured and reverse publication.
Production stage diagnostics were removed after localization.

Cached logical heights are retained for immutable Value aggregates, Part/Song,
closures, patterns, prototypes and completed private global slots. Reuse checks
the caller's remaining depth and propagates its peak into enclosing payloads;
each fresh copy resets/restores its own peak. Currently visited global slots can
close valid recursive function cycles, without certifying a finished cache yet.
spine-tests-fifth.log81279 terminal0 proves four default-stack fixtures: shallow
cached200 plus100 parents rejects, shallow List carrying100 pattern levels plus
160 list parents rejects, cached Part/prototype depths reject, valid aliases share
copies, error cleanup restores depth/visiting, and bounded storage admission.
spine-tests-sixth.log13936 terminal0 repeats four fixtures after slot propagation.

public-tests-nineteenth.log67384 terminal0 executes16/16 unfiltered fixtures,
including actual candidate200 success/300 DepthExceeded, post-freeze query and
resolution, compact shared diamonds, exact billion-repeat short-window bounds,
all accepted static operator profiles, nested inherited source frames, authentic
out-of-cover rejection, nonadjacent plate/spring/plate identities, same-placement
note sharing, strict caller limits, malformed indices/cycles and checked overflow.
closed-candidate-tests-second.log9952 terminal0 adds actual lazy source production
with a complete pinned bd bank and preactivation failure of an unpinned bank.
The language has no string-to-keyword constructor; the private generated-keyword
fixture uses the actual frozen kit/InstResolver and closed loader, not an invented
public conversion. provenance-tests-first.log82067 terminal0 proves original member
copy remains separate from changed current sound and a multi-member family.

TASK-002 author implementation is complete; TASK-003 full gates/seal and TASK-004
independent verification remain pending. Historical partial verification245 passing
fixtures and eager-depth failure remain accurate, with no full07B clearance or
production playback claim. Strict first-Clippy failure64638 is retained; own field
initialization/test-order findings corrected and the disjoint07C ordering finding
sent to its owner. Every owned Rust source remains below1000 lines. Final joined
gates await the disjoint07C stable write hold.


### Session: 2026-10-01 — independent completion

ROOT0058 accepted independent joined SONG-07B/07C verification at
/tmp/vactr-song07bc-independent-final-001/final-results.json:574 distinct fixtures,
60 nextest repeats, every gate exit0, retained70199 terminal0. All current and
historical sealed Rust hashes and both plan hashes matched fresh root inspection.
This independently supersedes the retained partial failures without deleting them.
TASK-003/004 and this prerequisite are Completed. Cleared07B Rust remains held;
this update changes only the own plan under immutable0029 before/after intent.
No archive/index/Git mutation and no whole-song playback completion claim.
