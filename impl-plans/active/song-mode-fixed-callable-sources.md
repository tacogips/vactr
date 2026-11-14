# Retained fixed callable source results

**Status**: In Progress
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Editing contract](../../design-docs/specs/design-song-mode.md#editing-contract), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and dependencies

Enable the genuine public `slice {beat -> p} ...` path when the copied callable
provably returns the same captured Pattern. The current public `slice p` path is
Subject mode; an unstructured function wrapper selects Index mode at runtime but
source-use preparation currently classifies every function as dynamic.

Depends on [Index occupancy](song-mode-index-occupancy.md),
[family/affine consumers](song-mode-index-families.md) and the existing shared
Freeze/retained shape discovery. ROOT0504 accepted a focused family checkpoint
with native/browser/Clippy and31 tests; broader criteria remain unfinished.
This source wave is sequential with the current family fixture work because
both touch routing/nested.rs. No Rust release is authorized by this document.

This is a fixed-result proof increment. Genuinely time-dependent callable results
still require original canonical-query result records and sealed topology,
configuration-opening and physical-overlap ceilings. They remain part of the
full song goal. Do not declare final support by rejecting all other callbacks.

## Proposed exact Rust manifest

| Path | Deliverable |
|---|---|
| `src/session/song/shape_preparation.rs` | Own and discover retained copied callable results |
| `src/session/song/shape_preparation/callable_results.rs` (new) | Bounded bytecode proof and closure/result identity records |
| `src/session/song/source_uses/layout.rs` | Preserve the proved result through actual dynamic-expansion producer edges |
| `src/session/song/source_uses/timing.rs` | Capture the same immutable result operand |
| `src/session/song/source_uses/timing/capture.rs` | Actual callable-result dependency and operand traversal |
| `src/song/routing/nested.rs` | Admit certified identity-clock wrapper between Slice and selected source |
| `tests/song_callable_sources.rs` (new) | Actual public isolated candidate/query/prepare/resolve witnesses |

Seven modules. Every touched Rust file remains below1000 lines; shape preparation
currently758, timing715, capture625, nested880. Recheck current baselines after
family work. Additional source paths require a root amendment before writing.
No dependency, Git or unrelated editor edits are implied.

## Source-grounded recognition contract

FnProto/Closure expose code, captures, constants, arity, constructor and memo.
Compiler lambda lowering appends Op::Ret. An immutable captured outer parameter
loads with Op::LoadCapture, whereas mutable reads add Deref. transform_instrument
passes an actual Value::Pattern to its callback. Thus a nested beat-ignoring
lambda can have LoadCapture followed by Ret; this expectation must be verified
through a genuinely compiled public fixture, not assumed as executed evidence.

Recognize only the actual shared-Freeze copied Value::Fn with:

- Fixed arity1, no keyword/default parameters, valid local/capture indices.
- No constructor and no mutable memo cell.
- Exactly LoadCapture(index)/Ret or LoadConst(index)/Ret.
- The referenced copied value is directly Value::Pattern.
- A validated supported forcing mask. Reject Forward and any binding behaviour
  whose safety for the actual Ratio argument has not been established.
- No argument reads, calls, global/slot dereferences, mutation or branches.

LoadCapture/LoadConst followed by Force and Ret is an optional explicit refinement
only if the copied loaded value is already a Pattern and Force is provably a no-op.
Never force a thunk, read a live variable or call at beat0 to guess constancy.
Unknown well-formed bodies retain their genuine dynamic requirement. Malformed
indices and actual work/depth failures retain deterministic diagnostics.

## Authority and lifecycle

The proposed private proof entry is:

```rust
fn fixed_pattern_result(
    callable: &Value,
    depth: u32,
    remaining: &mut u32,
    limits: SongAssetLimits,
) -> Result<Option<Rc<Pat>>, Failure>;
```

Retain the actual copied Closure and exact copied Pattern result together. A
dedicated function-of-time result record is keyed by that copied closure identity;
it is not the existing callback-plus-input shape key. Keep strong ownership so
pointer reuse cannot substitute an unrelated callable. FnProto alone, Pat.id,
pre-freeze pointers and results from another input/snapshot are insufficient.

PendingShapes already copies callback/input/outcome through the shared Freeze and
reconstructs keys afterwards. Determine the precise insertion and discovery order
before source release: proved results must enter closed source/resource dependency
discovery and actual immutable timing capture under the same copied authority.
Do not run a second independent Freeze or fabricate an output Pattern record.

The runtime producer chain is DynamicExpansion(0) then Child(0). Preserve its
complete actual prefix and original wrapper structure flag. A returned structured
Pattern does not make the original function wrapper structured. Source-use and
timing graphs must agree on the result edge and its identity clock.

Nested routing may traverse only the authenticated certified preserving wrapper
chain to the original selected-source policy. Do not weaken direct-source checks
to accept arbitrary dynamic mappings, discard trace words or manufacture timings.
Recognition does not itself establish configuration geometry or K/B/N bounds.
The existing consumers must run against the actual captured result topology.

All metadata inspection, depth checks, map storage and alias traversals consume
the original cumulative counter before work/allocation. No callback/RNG replay
per route event, default-quota reset or arrangement/copy expansion is permitted.

## Tasks

### TASK-001: Resolve copied-result lifecycle
**Status**: In Progress
**Parallelizable**: No

- [x] Ground nested public lambda lowering/captured value and mask in compiler sources.
- [ ] Confirm those expectations through the genuine compiled fixture in TASK-003.
- [x] Ground retained-record/dependency discovery insertion order in shared Freeze.
- [x] Confirm seven-module manifest and helper/lookup interfaces are sufficient.
- [x] Root accepts Ready status; separate hashed source release controls Rust writes.

### TASK-002: Retained proof and immutable graph capture
**Status**: In Progress
**Parallelizable**: No (depends on TASK-001)

- [ ] Bounded recognizer and actual copied-closure/result ownership implemented.
- [ ] Closed dependency/source discovery includes the proved Pattern.
- [ ] Source-use and timing operands preserve complete runtime wrapper provenance.
- [ ] Foreign closure/result, unsupported body, stale key and alias/depth checks.

### TASK-003: Genuine public consumer and independent checks
**Status**: In Progress
**Parallelizable**: No (depends on TASK-002)

- [ ] Public isolated candidate with transform-instrument and slice {beat -> p}
  issues real Index timing and resolves full configuration identities.
- [ ] Full/fractional/reordered queries agree; original Part stays unchanged.
- [ ] Same proto with different captured Patterns cannot substitute authority.
- [ ] Time-dependent, mutation/global-read and other unproved bodies remain
  addressed dynamic requirements without calling them to guess a fixed result.
- [ ] Genuine cumulative quota/depth and closed-resource/admission checks pass.
- [ ] Mandatory checker uses fixed source, native/browser/Clippy, exact nonempty
  list/run suites and affected regressions. Full song matrix remains required.

## Completion criteria

- [ ] Genuine public fixed callable composition reaches authenticated routing.
- [ ] Actual copied closure/result, complete traces and resources stay bound.
- [ ] No VM replay, guessed constancy or static-only full-goal claim.
- [ ] Independent checks pass; remaining canonical dynamic authority, geometry,
  host/transport/Apply/mute/playback/export obligations remain explicit.

## Progress log

### Read-only source audit — 2026-10-02

Specialized audit inspected compiler names/lambda lowering, actual transform
argument passing, Freeze closure/value caches and PendingShapes key reconstruction.
Root verified actual Op names, Value::Fn copying and pointer-key cache categories.
This is source evidence and a proposed lifecycle, not a compiled fixture result.
The read-only audit below resolves the lifecycle insertion contract. Actual compiled
closure inspection and root source release remain pending. No Rust or Cargo has
been authorized by this plan.

### Copied-result lifecycle preflight — 2026-10-02

The specialized read-only audit and root source inspection establish this order:
discover pending records, discover/pin/close assets, copy the Song and pending
records with the existing reachable Freeze, admit closed copied results, then
capture inventory/source-use/timing graphs (session/song.rs:278,465–479).
There is no additional Freeze pass for callable results.

Asset dependency discovery already walks closure captures/constants. Shape
callback discovery does not walk Fn values. Therefore discovery must provisionally
recognize the exact getter and descend only into its referenced Pattern, retaining
nested callback outcomes. That pre-freeze recognition creates no routing authority.
The copied closure is recognized again during closed admission, and the copied
closure plus exact copied Pattern is retained together. Count pending callable
records in the existing record storage reservation before shared copying.

The lookup interface must preserve the actual cumulative work counter:

```rust
fn fixed_pattern(
    &self,
    callable: &Value,
    remaining: &mut u32,
) -> Result<Option<&Rc<Pat>>, Failure>;
```

Recognition receives the current ancestry depth. Dependency traversal includes
proved results and their nested retained callbacks. Timing capture schedules the
proved Pattern in its existing postorder dependency tasks before fill/assembly;
changing only fill would leave an uncaptured dependency. Both timing and layout
use DynamicExpansion(0), Child(0), retaining the original wrapper structure flag.
For a Pure unit slot, runtime squeeze maps the unit cycle identically; Steps slots
retain their existing width/prefix map. No blanket identity assumption applies
to arbitrary placements.

Root inspected types/masks.rs:349–359: a normally inferred unused beat parameter
has Value mask because its use links are empty. The earlier possible Undetermined
expectation is withdrawn. Actual compiled fixture inspection must still confirm
code, arity, capture and mask. VM call.rs:139–179 shows direct Ratio arguments
are unchanged by Value/Fn/Late/Undetermined masks; Forward remains excluded.

The seven-module manifest covers fixed callables in actual Pure/Steps source
operands. General PParam::Fn Sound-source classification lives in source_uses.rs
and requires a separately amended wave. Time-dependent returned Patterns remain
a full goal requirement with canonical original-query authority.

TASK-001 lifecycle and helper-manifest review are source-grounded. The compiled
public getter witness, source baseline release and implementation remain pending.

### Ready declaration — 2026-10-02

ROOT0508 accepted the unchanged production family checkpoint: native/wasm/strict
Clippy and34 actual disjoint PASS tests. This clears sequential nested.rs writes;
it does not prove the public fixed-callable path. Specialized author and auditor
agree that the seven paths are sufficient for this increment. Root accepts the
lifecycle and manifests as Ready. A separate release records exact source hashes.

The charged retained lookup includes actual depth/limits:

```rust
fn fixed_pattern<'a>(
    &'a self,
    callable: &Value,
    depth: u32,
    remaining: &mut u32,
    limits: SongAssetLimits,
) -> Result<Option<&'a Rc<Pat>>, Failure>;

struct PendingFixedResult {
    callable: Rc<Closure>,
    result: Rc<Pat>,
}
struct FixedPatternResult {
    callable: Rc<Closure>,
    result: Rc<Pat>,
}
struct FixedCallableResults {
    entries: BTreeMap<usize, FixedPatternResult>,
}
```

All fields remain private. Recognized malformed load indices return Type failure.
Copied admission re-recognizes the same Closure and checks exact result Rc identity
before closed dependency acceptance. Discovery records remain provisional. Every
map admission, record copy, lookup and traversal uses the existing remaining work.

The genuine public fixture is a named outer transform `indexed p` calling
`slice {beat -> p} 2 [0 nil]`, applied to an actual analog Part before play-song.
Compiler matchc lambda expr lowering and names LoadCapture establish expected
LoadCapture/Ret; masks infer unused beat as Value. Compiled fixture inspection
must assert the real arity/code/mask/captured selected source. Expected full query
[0,4) has four half-cycle occupied wholes and exact issued timings; fractional
and reordered queries must agree with complete configuration identities.

The routing tail helper follows the remaining authenticated identity edges from
Slice subject to exact Source policy. Only Pure/Preserve unit-geometry wrappers
with full DynamicExpansion(0), Child(0) trace may extend the former direct-source
case. No rate, nonunit layout, extra trace, wrong source or arbitrary dynamic edge
may pass. Issued timing/handle binding, local clock and budgets stay mandatory.

Independent runtime verification is pending. General dynamic returned Patterns,
PParam::Fn support, geometry proofs, host/transport/playback/export remain required.

### Actual implementation private helper refinement

PendingFixedResults owns charged Vec<PendingFixedResult>; it provides discover,
freeze_records and admit_closed methods following the existing PendingShapes
state transition. Each record stores a strong actual Closure and exact Pattern.
FixedCallableResults owns private BTreeMap entries and provides charged current
depth/limits lookup. The parent only exposes PreparedShapes::fixed_pattern to
actual layout/timing consumers. Discovery returns a recognized result for
nested callback traversal without executing the getter; copies are admitted
through the same Freeze. Admission re-recognizes copied closure, compares result
Rc and actual closed dependency acceptance before map issuance. Unknown bodies
produce no fixed record. Recognized bad load indices retain Type diagnostics.
This refines storage helpers within the declared new child, no added path.

### Actual ancestry and graph scheduling refinement

PendingFixedResult additionally stores discovery `depth: u32`; aliases retain
the deepest charged discovery ancestry. Copy preserves it, copied recognition
and ClosedShapeCtx reuse it, and dependency walking reduces max_walk_depth by
that actual ancestry before traversing the result. No independent depth0 reset.
Layout and timing Pure wrappers retain full DynamicExpansion0/Child0 edges.
TimingCapture::fixed_value(node,value,depth)->Result<bool,Failure> assembles
only a previously scheduled proved result. Atomic value jobs schedule that
actual borrowed result before Finish with the same ancestry; literal lists remain
atomic values, not guessed runtime pattern expansion. Routing tail validation
inspects authentic remaining edges, exact trace and only Pure/Preserve unit
geometry before exact policy. General fixed getters used as dynamic index operands
may still require later static-summary support beyond this source-only increment.

### Fixture declarations — actual compiler and shared Freeze

New private `limits() -> SongAssetLimits` and `copied(code: &str) ->
(PreparedShapes, Rc<Song>)` compile/evaluate a real Song, discover once, freeze
namespace/Song/records with the same reachable Freeze, debit its measured work
after drop, then perform actual closed admission. `getter(part: &Part) ->
(&Rc<Closure>, &Rc<Pat>)` inspects the actual copied transform Slice/Pure
getter and its copied Source. Genuine identity, malformed negative bytecode,
exact inspection work/one-less and caller-depth tests use the existing child.
The public candidate helper and full/fractional/reordered Index route fixtures
use the normal isolated candidate pipeline. Resource fixture retains an actual
fixed IR bank. No manually manufactured successful shape or timing is used.

### Source-ready checkpoint — ROOT0509

Implemented actual copied fixed-result inspection, ancestry-retaining pending
records, same-Freeze closure/Pattern pairs, closed admission and charged strong
lookup. Source-use and postorder timing graphs retain DynamicExpansion(0)/
Child(0) and the original false structured wrapper. The existing authenticated
Index route dispatcher now checks a unit Pure/Preserve tail to its exact Source
policy. No original capture authority is regenerated.

Written public fixtures: `public_fixed_getter_issues_index_timings_and_reordered_full_routes`,
`fixed_getter_does_not_mutate_the_original_selected_part`,
`fixed_getter_keeps_actual_closed_instrument_ir_and_query_authority`,
`public_fixed_callable_resolution_has_exact_shared_work_and_depth`.
Written private callable-result fixtures: compiled/copy identity and exact
inspection work/depth; one nested callback discovery before closed copy;
unsupported time-dependent/global-load bodies with no discovery calls;
malformed direct-load index deterministic Type failure. The actual copied
compiled getter assertions require arity1, unused Value mask, LoadCapture/Ret,
and pointer equality to the same frozen selected Source. Equal copied code and
captures in an unissued foreign closure must not gain authority.

Scoped formatting and its check succeeded. No Cargo or nextest ran in this
author wave. These are written executable obligations, not passing results.
All behavior/check criteria remain unchecked pending independent verification.
The public resource test uses actual fixed convolution IR; the nested callback
fixture separately uses genuine discovery/shared Freeze/closed admission.
Unknown dynamic bodies, canonical retained realization, broader inherited/
nonlinear geometry, host preparation, transport and full playback remain
unfinished. Parent host documentation changed serially under root ROOT0510/
ROOT0511; rebuild the fresh cohort with those root-owned current hashes.

### ROOT0514 narrow sequence-fixture correction

Joined ROOT0513 original93627 terminated101; native/browser/strict Clippy
passed and root audited257 unique passing names plus one failure. The compiled
two-getter setup failed before its intended identity checks. Actual `song`
requires a Part (natives/song.rs183/411), while `sequence` constructs that Part
from a list (line237). Replaced only private `song [x y]` and public
`song [base selected]` with the actual `song {sequence [...]}` form. Added
actual form error diagnostics; every existing identity, route and exact
quota/depth assertion is preserved. No production source changed.

Failed raw results: `/tmp/vactr-callable-reservations-independent-001/final-results.json`,
SHA6293dbf241d8a2a5cf263567b22b52ccde23b859aaca4888b7ed40c5a8dcf9a4.
Later gates did not execute; no full-wave acceptance is inferred. Scoped
formatting/check passed; no author Cargo/nextest. Mandatory retry remains.

### ROOT0518 immutable Part-clone fixture correction

Joined retry002 original69728 terminated101. The helper reached the genuine
compiled getter assertions, then incorrectly required the selected Source Part
and edit source Part to have the same Rc address. edit.rs38/58 deliberately
construct two immutable clones of the same original Part. Freeze maps these
distinct original allocations separately (freeze.rs652). The corrected fixture
checks exact revision/duration/seed/tracks/node count/depth, selection track and
selector, and pointer equality of each actual copied captured Pattern payload.
It retains every later exact strong closure/result Rc check, same-proto/different
captures, foreign closure refusal, work/depth and no-rejection assertion.

Only the private test helper and this progress record changed. Production code
and all other fixtures remain identical. Scoped formatting/check succeeded; no
author Cargo or nextest. Failed raw results remain at
`/tmp/vactr-callable-reservations-independent-002/final-results.json`,
SHAd5f31ba3de259ca9b349d300a0cf20d2526889a2cef0c9b6a74b01d5871fa295.
No full-wave acceptance is inferred from earlier passing gates.
