# Authenticated issued route resolution

**Status**: Planning
**Created**: 2026-10-03
**Design Reference**: [Production provenance review](../../design-docs/references/song-mode/production-provenance-review-20261003.md), [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose and dependencies

Resolve real production events using original invocation/member evidence,
attested copied topology and retained connected geometry. Public DTOs cannot
issue authority. Preserve public resolve_route compatibility.

- **Previous**: [Query-issued authority](song-mode-issued-query-authority.md)
  and [frozen issued events](song-mode-frozen-issued-events.md).
- **Depends On**: [Immutable route authority](song-mode-immutable-route-authority.md).
- **Next**: [Ready and scheduler consumption](song-mode-issued-playback.md).
- **Required later**: Pre-Reserve certification and useful varying-seed domains.

No Rust edits are released by this candidate plan. Complete the evidence adapter
contract below in the preceding manifests or a bounded declared companion before
marking Ready. Do not silently add a ninth path to this resolver phase.

## Evidence adapter prerequisite

The accepted transcript exposes a charged borrowed actual invocation and exact
retained-execution membership. Route8 supplies bind_issued_owner using an
attested prepared site plus complete issuer/prefix/window selectors; it selects
a retained request privately and returns an address using the fresh invocation
clock. No new song_provenance path or invented IssuedInvocationRef is required.

```rust
impl IssuedQueryTranscript {
    pub(crate) fn invocation<'s>(
        &'s self, seal: &Rc<InvocationSeal>,
        work: &SharedIndexWork, depth: u32,
    ) -> Result<&'s OwnerInvocation, Failure>;
}
pub(crate) fn bind_issued_owner<'a>(
    site: PreparedSiteRef<'a>, issuer: NodeId,
    prefix: &[FrozenUseTraceTerm], owner_window: TimeSpan,
    transcript: &'a IssuedQueryTranscript, seal: &'a Rc<InvocationSeal>,
    work: &SharedIndexWork, depth: u32,
) -> Result<RetainedOwnerAddress<'a>, Failure>;
```

The transcript signature is present; the route adapter is currently being
implemented in the declared preceding phase. Final Ready signatures must be
checked against its accepted source. Member lookup must validate the genuine
selected boundary/member relationship and attested copied policy binding.
Authenticate raw execution membership in retained evidence, then use the new
transcript's actual rebound invocation clock and projected observations. Never
substitute old retained clocks or require Rc equality of fresh and old invocation
objects. Caller IDs and selectors cannot construct proof addresses.

## Proposed resolver manifest

| Module | Path | Status |
|---|---|---|
| Source resolution compatibility/extraction | `src/song/routing/source.rs` | Not Started |
| Issued source resolution and private fixtures | `src/song/routing/source/issued.rs` (new) | Not Started |
| Nested resolution compatibility/extraction | `src/song/routing/nested.rs` | Not Started |
| Issued nested resolution and private fixtures | `src/song/routing/nested/issued.rs` (new) | Not Started |
| Authenticated configuration dispatch | `src/song/routing/configuration.rs` | Not Started |
| Canonical geometry consumer | `src/song/routing/configuration/index/canonical.rs` | Not Started |
| Prepared owner entry | `src/song/routing/prepared.rs` | Not Started |
| Issued evidence binding | `src/song/snapshot/occupancy/lookup/authority.rs` | Not Started |

Eight paths. source.rs983 requires cohesive extraction before
growth; nested.rs is currently833 after the accepted meter extraction. Private owning fixtures live in declared children. Keep every touched
Rust file below1000 lines; extraction behavior/evidence must be reviewed.

## Proposed entry

```rust
impl PreparedRoutes {
    pub(crate) fn resolve_issued_event(
        &self, batch: &FrozenIssuedBatch, event_index: usize,
        work: &SharedIndexWork, depth: u32,
    ) -> Result<SongResolvedRoute, Failure>;
}
```

The entry validates every genuine contributing invocation and inherited source
binding before returning a route. Site/policy operands follow trusted copy
bindings to canonical view allocations. Keep existing structural source-use,
instrument and effect policy checks. Inherit the original remaining work/depth.

## Tasks

### TASK-001: Evidence prerequisites and cohesive extraction

**Status**: Not Started
**Parallelizable**: No

- [ ] Finalize actual borrowed invocation/member/address adapter signatures.
- [ ] Confirm complete dependency publication and route-view ownership.
- [ ] Extract source/nested logic coherently without changing public resolution.
- [ ] Preserve full original recipe, source-use/copy and policy attestation.

### TASK-002: Actual authenticated geometry resolution

**Status**: Not Started
**Parallelizable**: No; depends on TASK-001

- [ ] Resolve every contributing proof through genuine immutable evidence.
- [ ] Replace nested static-only exits at current nested.rs659–677 so composed
  and dynamic issued evidence reaches the real geometry consumer.
- [ ] Handle NeedsJointGeometry before its existing early locator refusal.
- [ ] Use canonical_prepared_index_geometry/canonical_index_configuration.
- [ ] Preserve addressed source START eligibility, full configuration identity,
  genuine connected unions/gaps and final owner clipping.
- [ ] Reject conflicting authenticated routes before musical-event coalescing.

### TASK-003: Genuine consumer evidence and acceptance

**Status**: Not Started
**Parallelizable**: No; depends on TASK-002

- [ ] Rebound cached nested sources resolve with callback reads disabled.
- [ ] Equal-handle distinct invocations all authenticate before coalescing.
- [ ] Swapped genuine policy/member or foreign transcript refuses.
- [ ] Real gaps, continuations and query partitions preserve route components.
- [ ] Exact/one-less cumulative work/depth failures publish no route/partial result.
- [ ] Independent scoped/native/lint/WASM/format checks pass held inputs.

## Completion criteria

- [ ] All tasks and actual owning geometry consumer fixtures pass.
- [ ] Public resolver compatibility and original work/depth remain intact.
- [ ] Ready/scheduler actually adopt this resolver in the required next phase.
- [ ] Pre-Reserve admission and useful varying-seed support remain full-goal work.

## Progress log

### 2026-10-03 — Current production exits and adapter requirement reviewed

Reviewer identifies genuine source/nested geometry dispatch paths and their
near-limit parent files. Root reads the actual nested static-only early exit.
The draft records a concrete resolver manifest and preceding evidence adapter
requirement. No Rust changes, test run or resolver completion is claimed.

### 2026-10-03 — Align draft with accepted transcript and released Route8

Removed the stale proposed transcript signature requiring a new proof type and
SongSnapshot-dependent request argument. The existing actual invocation API and
Route8 attested-site adapter supply the intended seam. Resolver receives the same
SharedIndexWork used by query/freezing; projection adapters may bridge local
remaining only with actual failure debits written back before other shared work.
No source release: final source-level contracts, geometry dispatch and owning
fixtures still depend on accepted Route8 implementation. Full resolver scope
includes every inherited contribution and full connected configuration before
clipping, not merely the event's first seal or retained cached clock.

### 2026-10-03 — Read-only consumer audit identifies concrete issued seams

The eight paths remain sufficient. Each issued event exposes all invocation seals
and source contributions; each contribution carries its own augmented origin,
leaves and authentic member-slot copies. Enumerate and authenticate every seal
and genuine transcript member, then bind every copied member slot to its actual
member. Do not resolve only the surviving public descriptor or first branch.
Discarded augmented origins can carry necessary inherited Slice timing.

Declare an opaque issued-index operand in lookup/authority.rs containing the
privately selected request, fresh address and prepared site/policy binding. The
canonical issued companion must validate_prepared_binding and canonical view
policy descriptors, replacing only the issued path's snapshot payload-pointer
check. Preserve source eligibility/original whole START, full configuration-group
identity and connected union of uncut projected wholes before owner clipping.

Current nested.rs293 refuses NeedsJointGeometry before geometry dispatch;
index_stage_configuration reconstructs static clocks and can reject genuine
composed runtime projections. The issued child must carry actual attested
invocation/member evidence through those stages, preserving offsets, sampling
orientation and source-owner bases. Do not infer affine clocks as a substitute.
Limits come from the original collector; public resolve_route's fresh budget and
first matching branch remain compatibility behavior, not the issued algorithm.

Owning fixtures must include callback-denied cached nested replay, discarded
augmented-origin contributors, distinct equal-handle invocations, source START,
real gaps and partitions, foreign/swapped member/policy refusal, and cumulative
exact/one-less work/depth failures without partial route publication.
