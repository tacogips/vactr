# Static source conditional metadata implementation plan

**Status**: Completed
**Plan ID**: SONG-07H
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance) and [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent and source evidence

The prepared source graph records Every, WhenMod and Chunk as a generic
Conditional. It omits their period, threshold and division count. Runtime
time.rs uses those values to select transformed cycles or fractional anchor
regions. Producer traces identify the selected child, not the missing predicate.
Every1 and Every2 can produce identical cycle-zero traces but different adjacent
policy components. Routing cannot reconstruct this information from clipped rows.

This supporting refinement follows the accepted design and inspected sources;
it is not a new Riela review. Copy static timing metadata without executing
callbacks during resolution. Full nested geometry, stochastic transforms,
production playback, Apply/mute, transport, export and editor remain required.

## Related plans and dependencies

- **Previous / Depends On**: [Certification accounting](song-mode-source-certification-budget.md), SONG-07G independently completed.
- **Consumer**: [Route preparation](song-mode-route-preparation.md), SONG-08B in progress.
- **Following**: Parent SONG-08 join and SONG-09 DSP playback.

| Dependency | Required result | Status |
|---|---|---|
| SONG-07G | Immutable cumulative admitted-work accounting | Completed |
| SONG-08B | Consumer agrees copied predicate contract and held gate window | Agreed; held focused window passed |

## Exact manifest

```json
{
  "planId": "SONG-07H",
  "planPath": "impl-plans/active/song-mode-source-conditionals.md",
  "writePaths": [
    "src/song/source_uses.rs",
    "src/song/source_uses/conditional.rs",
    "src/song/source_uses/source_free.rs",
    "src/session/song/source_uses.rs",
    "tests/song_source_conditionals.rs",
    "tests/song_source_uses.rs",
    "impl-plans/active/song-mode-source-conditionals.md"
  ],
  "sharedPaths": [],
  "ownershipNotes": "rust_coding owns six Rust paths and own plan only after explicit root release. SONG-08B owns disjoint routing files. All other upstream sources remain held. Final author and independent gates require all Rust writers held."
}
```

Fresh numbered immutable SHA intents precede every batch under
tmp/song-mode-riela/SONG-07H/. Preserve failed logs and original process handles
through terminal observation. Observation timeouts never authorize restarts.
No additional files, dependency/lock changes, broad formatting, Git, indexes or
archive work. Keep every touched Rust file below1000 lines; prior amendment is
required for additional splits. Root0126 records prior plan/source intent.

## Public declaration contract

Add the following enum, reexported from source_uses, and an additive
FrozenUseMapping::ConditionalStatic(FrozenStaticCondition) variant:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenStaticCondition {
    Every { period: i64 },
    WhenMod { modulus: i64, threshold: i64 },
    Chunk { divisions: i64 },
}
```

Preserve legacy Conditional for separately tracked stochastic recipes. Record
exact checked helper signatures before implementing them. Helpers belong in the
new cohesive conditional module; source_uses.rs is already919 lines.

## Module status

| Path | Deliverable | Status |
|---|---|---|
| src/song/source_uses.rs | Additive variant/reexport and certification validation hooks | COMPLETED |
| src/song/source_uses/conditional.rs | Copied enum and checked predicate/structure helpers | COMPLETED |
| src/song/source_uses/source_free.rs | Validate static DTOs while preserving ordinary dynamic geometry | COMPLETED |
| src/session/song/source_uses.rs | Copy exact static parameters into prepared recipes | COMPLETED |
| tests/song_source_conditionals.rs | Genuine recipes, numeric conversion and malformed DTO fixtures | COMPLETED |
| tests/song_source_uses.rs | Existing selected-use compatibility fixtures | COMPLETED |

## Behavioral contract

- Match eval_param_int/int_of exactly: Int/Int64, integral Ratio, and integral
  Float/Float64 with absolute value below9e15. Do not floor, cast to u32, clamp
  to4096 or normalize negative values. Nonconstant or invalid static values get
  an addressed DynamicTiming classification; ordinary source-free geometry
  remains permitted under its existing validator.
- Every transforms only when period is positive and the cycle residue is zero.
  WhenMod uses a positive modulus and residue greater than or equal to threshold;
  negative and large thresholds remain meaningful.
- Chunk with nonpositive divisions is identity. Positive Chunk uses the selected
  fractional anchor interval and its complement; it is not a cycle-wide choice.
  Event anchors may locate membership but never become configuration keys.
- Preserve original child0 and transformed DynamicExpansion1/Child1 traces,
  checked callback shapes and closed-resource admission. Copy no live callback,
  mutable parameter, guessed predicate or clipped event span.
- Validate operation/recipe compatibility and required edge structure on every
  reachable static DTO, including ordinary payloads. Existing shared work/depth
  accounting, complete graph/family copy admission and failure discipline remain.
- This phase supplies metadata, not full mapped geometry. SONG-08B must consume
  it and prove maximal components, partition invariance and capacity separately.
  SometimesBy and other nonlinear/stochastic support remain in the full goal.

## Tasks

### TASK-001: Declare and copy static recipes
**Status**: Completed
**Parallelizable**: No

- [x] Fresh source/plan hashes and exact helper declarations recorded.
- [x] Additive enum/variant and static integer copying implemented.
- [x] Operation/edge validation and ordinary dynamic compatibility preserved.

### TASK-002: Behavioral evidence and author seal
**Status**: Completed
**Parallelizable**: No, depends on TASK-001

- [x] Genuine Every2/0/negative, WhenMod thresholds and Chunk2/0 recipes proved.
- [x] Integral Ratio/float accepted; fractional/nonfinite/dynamic values classified precisely.
- [x] Original/transformed producer traces and malformed DTO rejection proved.
- [x] Fresh complete inventories and joined source-budget/free/uses/layout/candidate/assets/routes regressions pass.
- [x] Native/wasm, strict scoped Clippy, scoped formatting/diff and separate nextest repeats pass on held sources.
- [x] Exact terminal handles, logs/counts and unchanged hashes sealed.

### TASK-003: Independent verification and consumer handoff
**Status**: Completed
**Parallelizable**: No, depends on TASK-002

- [x] Independent adequately joined matrix passes with all Rust writers held.
- [x] Root verifies current hashes, exact inventories/logs/exits and full scope.
- [x] Owner marks own plan complete and hands immutable metadata to SONG-08B.

## Completion criteria

- [x] All three tasks independently proved; no source semantics or quotas weakened.
- [x] Missing static predicate information preserved exactly without runtime evaluation.
- [x] Consumer geometry and full song playback scope remain separately tracked.

## Progress log

### Session: 2026-10-01

Root created this bounded plan from independently inspected builder/runtime
metadata loss and the specialized Rust author's exact additive proposal.
No Rust edits are released by plan creation. Root must reconcile the consumer
handoff before release; all later gate windows hold every Rust writer.

### Author declarations: 2026-10-01

ROOT0127 releases the exact six Rust paths. Add the declared copied enum and
mapping variant, with public `anchor_slice(self, cycle: i64) -> Result<Option<TimeSpan>, Failure>`
and `selects(self, cycle: i64, anchor: Ratio64) -> Result<bool, Failure>` in the
cohesive conditional helper. The slice is normalized fractional anchor membership:
full [0,1) for a true Every/WhenMod predicate, none for false/nonpositive,
exact [r/n,(r+1)/n) for positive Chunk. Internal `validate(node: &FrozenSourceUseNode)`
checks matching operation and exact two original/transformed trace edges with
empty layouts. Builder `static_condition(node: &PatNode) -> FrozenUseMapping`
uses existing `int_of` only on constant values and returns addressed DynamicTiming
otherwise. No extra admission cost or altered work accounting; copied metadata
has fixed scalar size. The routing consumer was sent these signatures; full
geometry remains its separate task. Immutable 0001 records fresh baselines.

### Focused fixture entrypoint correction: 2026-10-01

Original focused process 9993 exited 101; retained public001 log records two new
fixture failures and four passes (budget five passed before Cargo stopped).
Both failures used an ordinary payload directly at the public selected-use root.
The existing source-free allowance instead applies when Source -> Part traverses
an ordinary Capture via payload_bound. Correct fixtures use that genuine nested
entrypoint. Ordinary lazy literal parameters remain DynamicTiming; hostile public
DTO validation explicitly replaces only the genuine ordinary node mapping with a
static recipe copied from an actual selected constant fixture, preserving its
prepared edges. No production entrypoint, builder, limits or accounting changed.
Immutable 0005 before/after records cover this fixture-only repair.

### Corrected focused evidence and author readiness: 2026-10-01

Original process 59184 exited 0 for corrected focused002 under ROOT0130. Actual
unfiltered public runs passed 49 fixtures: conditional6, budget5, free12,
uses17 and layout9. Private exact integer conversion fixtures passed 2/2.
All 21 joined input hashes were unchanged before/after. Retained public001
failure remains historical; no production relaxation was made. Static predicates
copy exactly the runtime integer conversion, including i64 bounds and nonfinite
refusal; malformed selected and nested ordinary public DTOs reject. Consumer
agreed the additive recipe and slice helper signatures. Final author and mandatory
independent matrices remain pending; full routing/playback remains separate.
Artifact-only final_gates_001.py and inventory_001.py are prepared but cannot run
until fresh root authorization. Private conditional fixtures are in
session::song::source_uses::conditional_tests, included by the library song::
filter; fresh qualified inventory will confirm this and deduplicate counts.

### Independent metadata completion: 2026-10-01

TASK-001 through TASK-003 and all module deliverables are complete for SONG-07H
metadata only. Corrected focused002 passed 51 distinct fixtures (49 public plus
2 private), original process 59184 terminal 0. The earlier focused001 process
9993 terminal 101 and its two fixture-entrypoint failures remain preserved;
only fixtures changed for that correction, with no production relaxation.

Final author evidence: `tmp/song-mode-riela/SONG-07H/0007-final-author-readiness-seal.json`
(SHA-256 `e48eed6e8dc767b2adc9c2c6ab5089b91dc48d5f7063d3dfec9c9fe48eb1d23d`).
All 12 gates exited 0; original process 54683 terminal 0. Fresh qualified
inventories and unfiltered runs reconcile to 307 distinct fixtures: public 201
across 13 binaries, song library 42 including the two private conversion tests,
and pattern library 64, with no overlap. Nextest passed 49 separate repeats.
All 21 joined inputs remained unchanged and complete log hashes are sealed.

Independent evidence: `/tmp/vactr-song07h-independent-001/final-results.json`
(SHA-256 `e7e22c7849e30b0c1837c836fbd53f0c88485fbab1bb499367098975d04e50f2`).
Original process 80690 terminal 0; nine execution commands and fifteen inventory
commands all exited 0. Independent results confirm 307 distinct fixtures and
49 repeats, exact 21 input hashes and log receipts. ROOT0133 accepts only this
metadata phase. Immutable 0008 before/after records complete this documentation
update; the six Rust paths remain unchanged and held. No archive, index or Git
mutation was made.

The agreed static enum, exact integer copying, anchor slices and operation/edge
validation are handed to SONG-08B. Full conditional geometry, nonlinear and
stochastic support (including legacy SometimesBy), parent SONG-08 readiness,
DSP playback, Apply/mute and export remain separately pending in the full goal.
