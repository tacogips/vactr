# Production song routing provenance review

**Date**: 2026-10-03
**Status**: Source review; production implementation remains required
**Design**: [Song mode](../../specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Current source evidence

| Boundary | Current behavior | Required change |
|---|---|---|
| `src/song/query.rs`, `pattern_rows` | Finishes the owner invocation before event expansion | Issue row authority while the genuine owner, seed and producer entry are available |
| `src/song/query.rs`, `insert_union` | Unions parts by handle and retains the first row's metadata | Preserve the authentic contributing invocation evidence; a handle alone cannot substitute for it |
| `src/song/snapshot.rs`, `query` | Copies public event descriptors, source origins and controls | Provide a private issued envelope without breaking the public descriptor API |
| `src/host/caps/song/preparation.rs`, Ready query | Returns public descriptors | Carry private issued authority to the scheduler |
| `src/sched/song.rs`, `realize` | Routes copied descriptors using the immutable route plan | Consume the issued envelope and authenticate its routing lookup |
| `src/host/caps/song/preparation.rs`, prepare | Calls route preparation without occupancy retention | Retain and certify under the original work counter before Reserve |

These are inspected source behaviors, not newly verified runtime outcomes.
The last accepted checkpoint remains retained lookup0006; geometry changes are
under implementation and have no fresh behavioral acceptance yet.

## Authority contract

Keep `FrozenSongEvent` public and unchanged. Use a crate-private issued event
envelope for the production route path. Its proof must be minted during the
actual query; copying NodeId, handle, note, owner fields or a reconstructed seed
does not issue authority.

Authentication must bind the original immutable Song and genuine retained
invocation, including actual seed, full producer entry, owner placement and
window. Use the existing owning lookup and genuine source-member binding.
Nested selected-source rows must retain their actual source use/copy/member
relations; a top-level owner proof alone cannot authenticate an inner Slice.

Preserve contributing authority through chord expansion, edits, clipping and
union. A temporary map keyed only by EventHandle loses evidence when several
genuine invocations contribute the same continuation. Do not repair that loss
by guessing an invocation from the final descriptor.

The retained ownership graph must remain acyclic. A proof retained by raw output
events or source-boundary members must not own an execution or clock that owns
those events. An opaque privately issued value proof or an acyclic external
table can carry authority, provided immutable lookup authenticates it against
the exact retained execution or a separately sealed permitted domain.

Source review refines this into two opaque leaves: a shared execution seal for
the original raw execution, and a fresh invocation seal for each genuine or
rebound invocation. An immutable transcript issued by the query owns completed
invocation records and their actual clock/member bindings. Authenticate raw
execution membership against the original snapshot and fresh invocation
membership against that transcript. Rows and origins own seals only, preserving
the acyclic ownership graph. Computational query observation filtering must not
silently disable issuance for production rows.

## Immutable route-view ownership gap

SongRoutePlan exposes a copied FrozenRoutingInventory; prepare_routes clones
the snapshot inventory. Current retained lookup instead borrows SongSnapshot,
checks the exact prepared payload allocation and authenticates the exact selected
descriptor allocation in its original parent. A copied public route topology is
not automatically that authority. Keeping the entire snapshot in a route plan
would also retain the evaluator and VM.

Specify a privately snapshot-issued immutable route view, or a private owner
pairing the unchanged public route plan with such a view. It must preserve the
original Song/recipe/source-policy attestation and authenticate any copied
topology used by consumers. Mutable public descriptors or scalar-equal copies
cannot substitute for issuance. Actual route and geometry consumers must use
this view; merely storing it is insufficient. Its exact bounded companion scope
is under source review and is not released for implementation yet.

## Implementation and verification obligations

Declare bounded companion plans before edits. The actual end-to-end bridge must
consume canonical geometry in production routing; an unused envelope or cloned
retained view is not integration. Existing large routing files require narrow
children or a cohesive precursor split before growth reaches1000 lines.

Genuine tests must exercise callback denial after retention, nested sampled
sources, repeated continuations and chord union, distinct source uses/placements,
actual varying seeds, foreign original Songs and one-less work/depth failures.
Failure must preserve published retention and prior active playback.

Admission and varying-seed support remain distinct required companions. The
immutable route consumer cannot invoke the VM on a missing proof, and a fixed
initial replay view cannot silently acquire an unseen seed's execution. Useful
finite return-shape bounds and transactional first-execution issuance must be
specified and connected before full song mode can be declared complete.
