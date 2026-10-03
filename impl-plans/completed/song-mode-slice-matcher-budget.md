# Slice matcher work reservation implementation plan

**Status**: Completed
**Design Reference**: [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics), [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose

Reserve all work that the source matcher actually performs, including genuine
issued Slice timing validation. Checkpoint006 passed native/Clippy and sixteen
tests, but both real Index fixtures failed in resolve_route with Slice timing
work exhausted. Preparation and actual query issuance had already succeeded.
The matcher allowance excludes timing work now required by resolve_origin.
Do not raise default limits or suppress validation to conceal this omission.

## Related plans

- **Depends On**: [Index occupancy](../active/song-mode-index-occupancy.md), [Genuine runtime fixtures](../active/song-mode-index-runtime-fixtures.md).
- **Next**: Remaining connected Index geometry, dynamic authority, host and transport.
- Evidence: `/tmp/vactr-index-occupancy-checkpoint-006/final-results.json`.

## Exact Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/song/routing/source.rs` | Complete cumulative source-search reservation and inline reservation regressions | In Progress |

One module. Source starts at 729 lines and must remain below 1000. Existing
occupancy and runtime fixture plans keep their own manifests. Any helper in
another Rust file requires a root amendment before modification.

## Contract

Retain the current function interface:

```rust
pub(super) fn reserve_source_search(
    cover: &FrozenSourceUseCover,
    payload: &FrozenPattern,
    origin: OriginView<'_>,
    base_depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<SongLimits, Failure>;
```

- OriginView authority_work currently prices only source-whole authentication.
  resolve_origin additionally validates all issued Slice timings before search.
- Debit timing work from the original caller's cumulative remaining counter.
  Incoming remaining depth is authoritative; do not create a default quota.
- Every probe/scan/validation is charged before work or allocation. If preflight
  performs real validation and the matcher later validates again, charge both
  actual passes and distinguish probe cost from the reserved matcher allowance.
- Preserve complete source/issued handle, prefix, parts, timing, and depth checks.
  Timing validation cannot be omitted just because capture previously succeeded.
- No event expansion, VM evaluation, RNG replay, fixture weakening or widened
  public origin/timing constructors.
- Exact work exhaustion and one-less refusal remain deterministic. Existing
  no-timing source reservations must retain their checked behavior.

## Tasks

### TASK-001: Reservation accounting
**Status**: Completed
**Parallelizable**: No

- [x] Derive actual timing validation cost using existing sanctioned interfaces.
- [x] Reserve it under original work/depth and separate actual probe costs.
- [x] Preserve failure debit and existing source-search work accounting.

### TASK-002: Genuine and existing quota witnesses
**Status**: Completed
**Parallelizable**: No (depends on TASK-001)

- [x] Existing shared source-search quota regression remains valid.
- [x] Genuine runtime Index routes pass without increasing fixture limits.
- [x] Add exact/one-less cumulative reservation test with actual issued authority.
- [x] Depth rejection and no-timing regression remain covered.

### TASK-003: Independent verification
**Status**: Completed
**Parallelizable**: No (depends on TASK-002)

- [x] Held source and plan hashes supplied before checker release.
- [x] Native/Clippy and fresh source-search/runtime inventory/run pairs pass.
- [x] Full actual pre/post/current source cohort remains fixed during checks.
- [x] Evidence reconciled without claiming full song completion.

## Completion criteria

- [x] Complete timing and source work reserved through original budget.
- [x] Actual Index configuration consumer is reached by genuine runtime fixtures.
- [x] Existing quota plus exact/one-less witnesses pass independently.
- [x] Touched source remains below 1000 lines.

## Progress log

### Session: 2026-10-02 root release

Root reviewed routing/source.rs reserve_source_search, source_uses/origin.rs
OriginView::authority_work and source_uses.rs resolve_origin. Only source-whole
work is reserved, but matcher now debits validate_frozen_slice_timings first.
This is an actual production reservation gap. Original006 process terminated101;
all902 checked inputs stayed unchanged. No quota increase is authorized.

### Concrete same-file regression declaration before source

```rust
#[test]
fn genuine_index_timing_reserves_probe_and_matcher_with_exact_quota_and_depth();
```

Use actual capture_part/SongSource/Slice/query_part/copy_origin to issue immutable
Index timing. A same-file symbolic source recipe follows its actual subject
Child0 trace, matching the existing matcher budget regression's scope. No issued
handle, timing or origin constructor is used. The companion separately tests the
full genuine inventory pipeline. Measure outer reservation including real timing
probe and reserved matcher pass; exact allowance succeeds, one-less refuses,
repeated calls share one counter, and caller-relative depth rejects before search.

### Implemented reservation checkpoint, verification pending

reserve_source_search computes caller-relative remaining depth before the timing
probe, runs existing validate_frozen_slice_timings against the original mutable
remaining counter, and measures its successful actual debit. A separate identical
debit is reserved alongside source authority before the search allowance. The
returned matcher allowance excludes the already-spent probe, includes its own
validation reservation once, and preserves existing search probe subtraction.
There is no fresh quota, refund, default-limit increase or skipped validation.

The same-file genuine runtime test queries an actual selected Index Slice and
uses copy_origin to obtain all issued authority. It asserts timing issuer, whole
and subject handle; a matcher-only symbolic recipe follows actual subject trace.
This is not a claimed full inventory capture: companion tests supply that scope.
Assertions cover exact cumulative allowance, one-less refusal, two reservations
on one counter, successful matcher identity and caller-relative depth rejection.
The existing no-timing regression remains unchanged. Source is984 lines after
scoped rustfmt; any additional growth must preserve the hard limit or split first.
No author Cargo/nextest ran. All new behavioral assertions await the checker.

### Checkpoint007 native pass and test descriptor compile repair

Original94510 terminated101 (poll d3d6e6); native passed, strict all-target
Clippy could not compile the new test: FrozenPatternSource is nonexistent.
The actual copied descriptor is FrozenSelectedSource with root_part, track,
and family; its owner revision is authenticated through routing inventory.
All903 cohort inputs stayed unchanged and no behavioral tests ran.
ROOT0492 authorizes only the new fixture type replacement and removal of the
unsupported revision literal field. Production prefix and all runtime issuance,
work/depth/quota assertions remain unchanged. No Cargo/nextest or passing claim.

### Independent focused acceptance — ROOT0494

Original008 session49564 terminated0 (fc2ed0). Native and strict all-target
Clippy plus five disjoint suites passed20 actual tests with exact list/run names.
All903 inputs stayed unchanged. Source matcher tests prove genuine issued timing,
exact/one-less and twice-shared cumulative work, actual caller depth refusal and
unchanged no-timing reservation. Both full genuine Index routes now pass.
Evidence: /tmp/vactr-index-occupancy-checkpoint-008/final-results.json, SHA256
 f17254ee393ea1e1edf4265461876419f8958381bd270327b61fa2310b138cc7.
This closes this one-module accounting plan; broad occupancy and full song remain
unfinished. Full58 matrix acceptance is not claimed.
