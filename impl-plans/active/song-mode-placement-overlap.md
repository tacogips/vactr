# Half-open ordinary placement overlap implementation plan

**Plan ID**: SONG-PLACEMENT-OVERLAP
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)

## Purpose and related plans

Ordinary placement admission currently uses floor(tail/minimum duration)+2,
reserving two physical generations at zero tail. Source-cover placement admission
already uses ceil(tail/duration)+1 for half-open logical intervals. Preserve the
full million-repeat arrangement and both retained instrument families while making
ordinary admission use the same authentic logical overlap rule. Actual retiring
physical leases and concurrent preparations remain charged separately.

- **Depends on**: [Route preparation](song-mode-route-preparation.md).
- **Consumer**: [Host preparation](song-mode-host-preparation.md), [Transport core](song-mode-transport-core.md).
- **Evidence**: ROOT0556 and original joined013 host preparation failure log.

## Exact Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/song/routing/prepare.rs` | Correct checked ordinary logical overlap bound | In Progress |
| `tests/song_route_preparation.rs` | Actual zero/exact/fractional-tail repeated route admission witnesses | In Progress |

Two Rust files plus this plan. Other source and plans remain held. Prepare is984
lines and tests870 at release; every touched file stays below1000. Additional
paths or a required split need an explicit Root amendment before writing.

## Existing public declaration contract

```rust
pub fn prepare_routes(snapshot: &SongSnapshot, caps: &CapabilitySet,
    available: &SongHostCapacities) -> Result<SongRoutePlan, Failure>;
```

Use actual existing signature/imports in source; no new API, dependency or VM work.

## Required behavior

- Ordinary logical overlap is ceil(nonnegative tail/minimum positive placement
  duration)+1; no positive duration still uses the existing bounded empty rule.
- Compute with exact Ratio64 and checked arithmetic; do not use floating time.
- Reserved generations remain min(logical occurrences, proven overlap), with
  authentic full route identity and every retained family intact.
- Zero tail has one live generation per ordinary logical family; exact and
  fractional tails match half-open source-cover overlap conventions.
- Preserve real measured capacity reports, resource ownership, concurrent old
  preparation/retirement accounting and explicit insufficient-capacity errors.
- Keep the million-repeat owner fixture unchanged. No musical count reduction,
  fabricated capacity, weakening effect/purity checks or eager repeat expansion.
- Future transport still must submit authenticated same-frame Release/Rebind/Event
  and establish normal safe returns; scoped admission alone proves no playback.

## Tasks

### TASK-001: Ground admission rule
**Status**: In Progress
**Parallelizable**: Yes; disjoint from held inference and owner repair paths.
- [ ] Confirm actual ordinary and source-cover interval conventions.
- [ ] Record immutable exact source/plan baseline.

### TASK-002: Correct bound and genuine regressions
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this manifest.
- [ ] Correct checked ordinary bound without altering other route authority.
- [ ] Genuine repeated route cases cover zero, integral and fractional tail.
- [ ] Exact measured boundary accepts and one-below rejects honestly.

### TASK-003: Mandatory independent verification
**Status**: In Progress
**Depends On**: TASK-002 and complete owner repair hold.
**Parallelizable**: No; source held through original checker terminal.
- [ ] Native/wasm/strictClippy and all route/owner regression suites pass.
- [ ] Exact inventories, logs, hashes and original process evidence retained.
- [ ] Scoped formatting and touched sources below1000.

## Completion criteria

- [ ] Correct ordinary bound with full repeat/family authority.
- [ ] Actual Native/Arena million-repeat owner and route tests pass.
- [ ] No unexpected source changes; all mandatory checker evidence reviewed.
- [ ] Full scheduling, Apply/mute, browser playback and export remain required.

## Progress log

### 2026-10-02 — ROOT0557 actual ordinary bound seam

Read-only author and Root audits identified inconsistent ordinary versus source-
cover zero-tail bounds. Joined013 already passed521 earlier tests and six owner
witnesses; four owner failures remain. This bounded source release preserves the
original repeat case and measured capacity. No behavior acceptance yet.

### Exact fixture declarations before source

Private helper `measured_placement_capacity(bus_slots: usize) -> SongHostCapacities`
constructs real NativeArc Engine storage and real SPSC command/ACK producers,
configures actual staging and observes RequestCapacity through Engine.process.
Genuine frozen custom tone retains both keyword and installed instrument family
branches. Repeated duration1 at frozen default120BPM/4beats is2 seconds.
Tail seconds0/1/2/3/4 require overlap1/2/2/3/3, preserving million occurrences
and min(O,bound). An actual5-slot BusGraph (legacy master consumes1) reports4
free slots, while actual4-slot construction reports3; the unchanged two-family
zero-tail arrangement needs4 and must accept/refuse respectively. No metadata
capacity fabrication. Million fixture unchanged means musical input/count and
complete retained family authority remain, not stale one-family expectations.

### ROOT0557 source-ready checkpoint

Implemented exact checked ceil(tail/duration)+1 with existing min(O,bound),
empty-duration rule and independent physical retirement accounting preserved.
Actual source-cover formula independently confirms the half-open convention.
Written genuine fixtures:
- `ordinary_repeat_half_open_tail_overlap_preserves_every_family_and_occurrence`
- `million_repeat_zero_tail_fits_actual_measured_boundary_and_refuses_one_below`

The latter observes actual Engine RequestCapacity receipts from real NativeArc
BusGraphs with5/4 total slots and4/3 free slots. Both retained families and all
million occurrences remain; no resource count or musical input was reduced.
Scoped rustfmt and scoped rustfmt --check both exited0. Sources are989 and965
lines, below1000. No Cargo/nextest commands executed in the author wave; written
fixtures and behavior require Root's mandatory independent checker. Plan remains
In Progress and acceptance criteria remain unchecked.

### ROOT0560 genuine fixture compile repair

Joined016 native and wasm checks passed, but strict all-target Clippy stopped
before any tests: fixture enqueue unwrap required Debug on NativeRecord. Raw
`/tmp/vactr-callable-reservations-independent-016/clippy.log` SHA
`01ff2ddf233eb3ce8496f3f55354ed747cbbae02703cdb6b12d57961b1d44a8c`.
Replaced only that unwrap with an explicit successful enqueue assertion using
is_ok; actual RequestCapacity, Engine.process and exact report assertions remain.
Production prepare.rs remains unchanged. No behavior pass inferred.

### ROOT0562 original billion-repeat exact-bound repair

Joined017 native/wasm/strictClippy passed;400 tests passed with one stale
billion-repeat overlap expectation failing (route23 passed/1 failed). New tail
and actual measured-capacity fixtures passed. Root independently audited931
inputs and raw logs; `/tmp/vactr-callable-reservations-independent-017/` retains
the evidence. The original billion input and occurrence/topology assertions stay
unchanged. Exact overlap is ceil(4seconds/2seconds)+1=3; an expired oldest
generation at the half-open boundary is excluded. Updated exact maxlive3 and
strengthened exact per-branch reserved_generations3. Production remains unchanged.
Scoped formatting/check passed; no author Cargo or nextest. Full joined acceptance
still requires mandatory independent retry.
