# Bounded complete-song resource reservations

**Status**: In Progress
**Design Reference**: [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics), [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and dependencies

The consuming host owner must reserve its complete lease projection, including
instrument/bus/sample/control/analyzer leases. PreparedSong::reserve currently
limits all leases to SampleStore MAX_RESOURCES256, even though actual all-kind
stager lease capacity is separately configured. Preserve sample-slot admission
while allowing complete reservation under explicit owned resource/work bounds.

Depends on the existing privately owned PreparedSong preparation state machine.
Next: [consuming host preparation](song-mode-host-preparation.md). This prerequisite
is independent of fixed-callable source writes; its two Rust paths do not overlap
that wave. Independent checking joins both frozen source cohorts before execution.
Host adoption, geometry, finite transport, Apply/mute and export remain required.

## Exact Rust manifest

| Path | Deliverable |
|---|---|
| `src/song/snapshot.rs` | New charged bounded reservation API; register test child |
| `src/song/snapshot/reservations_tests.rs` (new) | Genuine PreparedSong resource/state/work fixtures |

Two modules. Snapshot currently873 lines; every touched Rust stays below1000.
Additional files require root amendment. No dependencies, Git or editor edits.

## Declared API and contract

```rust
impl PreparedSong {
    pub(crate) fn reserve_bounded(
        &mut self,
        resources: Vec<SongResourceLease>,
        max_resources: u32,
        remaining: &mut u32,
    ) -> Result<(), Failure>;
}
```

The original reserve keeps its compatibility behavior and sample-derived256 limit.
The new consuming owner uses reserve_bounded with its explicit resource maximum
and original cumulative counter. Do not introduce a production default-quota reset.

Charge one state/entry check. Require Preparing and uninitialized reservations.
Validate actual length against max_resources, then admit checked C(n)-1 work before
any scan: C(n)=1+2n+n(n-1)/2 covers both scalar fields and every possible numeric-ID
comparison. Pairwise borrowed comparisons need no auxiliary map or cloning. Work
that cannot fit remaining/u32 yields FuelExhausted, without refund or mutation.
Count/rational overflow diagnostics remain precise. No arrangement-duration loop.

Reject duplicate numeric resource IDs even when generations differ. Preserve
existing generation values and acknowledgement semantics. Move the original input
vector only after all validation; failure leaves preparation state, reservations,
initialization and original snapshot identity unchanged. Empty reservation is still
explicit and once-only. Do not count resources as samples or claim DSP adoption.

Register the cfg test child from the snapshot root using its exact new path.
The existing test-only candidate helper may become pub(super) so the sibling child
can use the genuine prepare_song(candidate()) fixture. No production helper or
synthetic prepared authority is needed.

## Test-only helper declarations

```rust
fn leases() -> Vec<SongResourceLease>;
fn prepared() -> PreparedSong;
```

The child uses the existing isolated-evaluation candidate, not fabricated ownership.

## Tasks

### TASK-001: Source-grounded declaration
**Status**: Completed
**Parallelizable**: Yes

- [x] Verify distinct SampleStore256 and configured all-kind stager lease capacities.
- [x] Identify actual PreparedSong reservation bottleneck and preserve old API.
- [x] Declare explicit cumulative work/resource API and two-path genuine fixture seam.

### TASK-002: Charged complete reservation
**Status**: In Progress
**Parallelizable**: Yes (independent of fixed-callable seven-path wave)

- [ ] New API validates count/state/once-only and numeric-ID uniqueness.
- [ ] Explicit checked work admitted before comparisons; no mutation on failure.
- [ ] Original vector/generations retained; legacy behavior and ACKs preserved.

### TASK-003: Genuine tests and mandatory independent checking
**Status**: In Progress
**Parallelizable**: No (depends on TASK-002 and joined source hold)

- [ ]257 unique leases admitted then genuinely acknowledged Ready by existing API.
- [ ] Exact resource/work limits and one-short refusal; C(257)=33411.
- [ ] Duplicate IDs with same/different generations and unchanged failure state.
- [ ] Empty one-shot, successful retry after invalid input, Ready/Applied/Failed refusal.
- [ ] Generation preservation and mismatched acknowledgement rejection.
- [ ] Fresh held sources, native/browser/strictClippy and exact nonempty list/run tests.

## Completion criteria

- [ ] New owner can reserve complete kind-erased leases under explicit bounded demand.
- [ ] Sample storage bounds remain actual downstream host admission constraints.
- [ ] Independent verification proves state/identity/work and existing regressions.
- [ ] No host adoption or full-song completion claim from reservation-only fixtures.

## Progress log

### Source audit and root release preparation — 2026-10-02

Specialized audit and root inspection distinguish engine sample storage from
stager leases. Snapshot.rs345 imposes the actual all-kind256 bottleneck; that is
not a universal DSP numeric-ID restriction. The new bounded API is a prerequisite
for the generic consuming owner. Root accepts Ready and releases exact baselines
separately. No tests have executed for this new API.

### ROOT0512 source-ready hold — 2026-10-02

Implemented reserve_bounded under the declared original counter and explicit
resource limit. Entry work is charged before state checks; checked scan work is
admitted before pairwise validation. Legacy reserve and acknowledgement bodies
remain unchanged. Six genuine isolated-candidate fixtures cover257/exact33411,
limits, shared work, duplicate generations, retry, allocation identity, empty
one-shot and Ready/Applied/Failed refusal. Snapshot920 and child186 lines.
Scoped rustfmt passed; no Cargo/nextest or behavioral checking executed by author.
Immutable receipts: SONG-BOUNDED-RESERVATIONS/0001-before-source.json,
0002-before-fixtures.json and0003-source-ready-held.json. Both Rust paths and this
plan are HELD for mandatory independent joined verification; criteria remain
unverified until actual checking. Host adoption and full Song completion pending.
