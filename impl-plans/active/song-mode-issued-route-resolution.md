# Authenticated issued route resolution

**Status**: In Progress (session 258: runs FIRST on base e71d726 and depends only on SONG-ROUTE8; TASK-008 to TASK-012 unchanged; see "Session 258 amendment")
**Created**: 2026-10-03
**Design Reference**: [Production provenance review](../../design-docs/references/song-mode/production-provenance-review-20261003.md), [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture), [Production integration contract](../../design-docs/specs/design-song-mode.md#production-integration-contract-route-authority-to-playback-2026-10-03)

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
// Session 250: selector form pinned by the Route8 plan amendment.
pub(crate) struct IssuedOwnerSelector<'a, 'p> {
    pub(crate) site: PreparedSiteRef<'a>,
    pub(crate) issuer: NodeId,
    pub(crate) prefix: &'p [FrozenUseTraceTerm],
    pub(crate) owner_window: TimeSpan,
}
pub(crate) fn bind_issued_owner<'a>(
    selector: &IssuedOwnerSelector<'a, '_>,
    transcript: &'a IssuedQueryTranscript, seal: &'a Rc<InvocationSeal>,
    work: &SharedIndexWork, depth: u32,
) -> Result<RetainedOwnerAddress<'a>, Failure>;
```

This resolver builds one `IssuedOwnerSelector` per seal from the attested
prepared site and the stage's complete issuer, prefix and owner window. It
must not add parameters back to `bind_issued_owner` or bypass the selector.
New functions in this plan's paths also stay at seven parameters or fewer,
because no `allow`/`expect` is permitted.

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

## Session 249 executable contract (wave 2)

The source of truth is the design section "Wave 2a: issued route resolution"
and decision D7. This plan runs in parallel with SONG-SHARED-WORK and
SONG-STRUCTURAL-CLOCK. Their write paths do not overlap with this plan's.

```json
{
  "planId": "SONG-ISSUED-RESOLUTION",
  "planPath": "impl-plans/active/song-mode-issued-route-resolution.md",
  "wave": 2,
  "dependsOn": ["SONG-ROUTE8"],
  "writePaths": [
    "src/song/routing/source.rs",
    "src/song/routing/source/issued.rs",
    "src/song/routing/nested.rs",
    "src/song/routing/nested/issued.rs",
    "src/song/routing/configuration.rs",
    "src/song/routing/configuration/index/canonical.rs",
    "src/song/routing/prepared.rs",
    "src/song/snapshot/occupancy/lookup/authority.rs",
    "src/song/routing/density/index.rs",
    "impl-plans/active/song-mode-issued-route-resolution.md",
    "tmp/song-mode-riela/session249-resolution-intent.json",
    "tmp/song-mode-riela/session249-resolution-receipt.json",
    "tmp/song-mode-riela/session249-resolution-build.log",
    "tmp/song-mode-riela/session249-resolution-clippy.log",
    "tmp/song-mode-riela/session249-resolution-nextest-focused.log",
    "tmp/song-mode-riela/session249-resolution-nextest-full.log",
    "tmp/song-mode-riela/session249-resolution-wasm.log",
    "tmp/song-mode-riela/session249-resolution-fmt.log"
  ],
  "sharedPaths": []
}
```

### Intent and context

Production must resolve each issued event's route from authenticated evidence
before any coalescing. The pieces already available after wave 1 are:

- `FrozenIssuedBatch`, with `events()`, `transcript()` and per-event
  `descriptor()`, `invocations()` and `source_contributions()`
  (`src/song/snapshot/issued.rs:25-57`);
- `bind_issued_owner` (`lookup/authority.rs`);
- `PreparedRoutes` and its site/policy refs (`prepared.rs`);
- the canonical consumers `canonical_prepared_index_geometry` and
  `canonical_index_configuration`
  (`configuration/index/canonical.rs:70`, `configuration/index/canonical.rs:176`).

The legacy `resolve_route` (`source.rs:292`) and its barriers stay as they are:
`NeedsJointGeometry` at `nested.rs:293` and the `index_stage_configuration`
static clocks at `nested.rs:490`.

### Non-goals

- Do not change any scheduler, Ready or preparation code. That is wave 3.
- Do not add varying-seed first-execution support.
- Do not touch Chunk; its barrier stays.
- Do not edit snapshot.rs, issued.rs or song_replay.rs. They belong to
  SONG-SHARED-WORK.
- Do not edit `song_clock*` or the combinators. They belong to
  SONG-STRUCTURAL-CLOCK.
- Do not add `allow` or `expect` attributes. Leave the existing
  `cfg_attr(not(test), allow(dead_code))` on the canonical consumers in place;
  wave 3 makes them reachable.

### Pinned contract for wave 3

This signature is used by playback. Do not change it.

```rust
impl PreparedRoutes {
    pub(crate) fn resolve_issued_event(
        &self, batch: &FrozenIssuedBatch, event_index: usize,
        work: &SharedIndexWork, depth: u32,
    ) -> Result<SongResolvedRoute, Failure>;
}
```

### File-level changes

1. **`lookup/authority.rs`**
   - Add the opaque `pub(crate) struct IssuedIndexOperand<'a>`. Its fields are
     private: the selected request reference, the `RetainedOwnerAddress<'a>`
     returned by `bind_issued_owner`, and the `PreparedSiteRef<'a>` and
     optional `PreparedPolicyRef<'a>`.
   - It is constructed only inside this module, from `bind_issued_owner`
     results.
   - The file must stay below 1000 lines.
2. **`configuration/index/canonical.rs`**
   - Add an issued companion that accepts `&IssuedIndexOperand`.
   - It calls `validate_prepared_binding` and resolves canonical view policy
     descriptors through `PreparedPolicyRef::bind_original`. This replaces only
     the snapshot payload-pointer check.
   - Beyond that, it reuses the existing interval-union logic, so connected
     uncut wholes are unioned before owner clipping.
3. **`configuration.rs`**: dispatch an issued Slice stage to that companion.
   Legacy dispatch stays unchanged.
4. **`source/issued.rs` and `nested/issued.rs` (new)**
   - These hold the issued source and nested stage walks.
   - They carry the actual attested invocation/member evidence, using the fresh
     rebound invocation clock from the transcript, never a retained clock.
   - They route `SourceLocatorMapping::NeedsJointGeometry` into the canonical
     companion instead of refusing.
   - They use the original source START for eligibility and full
     configuration-group identity.
   - `source.rs` and `nested.rs` gain only `mod issued;` and the minimal
     `pub(super)` visibility that the children need.
   - Budget: `source.rs` is 983 lines and may grow by at most 10. If more is
     needed, stop and record a manifest amendment.
5. **`prepared.rs`**: implement `resolve_issued_event` in this order.
   1. Check `Rc::ptr_eq` between the batch transcript's original Song and the
      view's `original()`.
   2. Bound-check `event_index`.
   3. For every seal in `invocations()`, build an `IssuedOwnerSelector`, call
      `bind_issued_owner(&selector, transcript, seal, work, depth)`, and then
      run the issued stage walk.
   4. For every `source_contributions()` entry, authenticate its own augmented
      origin and each member slot against genuine transcript members and the
      attested policy binding. Discarded augmented origins are included.
   5. Compare all candidate routes. If any two differ, fail with no partial
      result.
   6. Return the single agreed route.

   The work comes from the caller's `SharedIndexWork` through `with_work`
   only. Never create a fresh budget.

### Code to imitate

- Stage walking: `nested.rs` `Stage` construction (around lines 280-300).
- Budget bridging: `lookup/authority.rs:with_work`.
- Interval union: `canonical.rs` component insertion (lines 50-66).

### Key pitfalls

- Do not resolve only `descriptor()` or the first seal or branch.
- Do not compare a fresh invocation `Rc` with a retained invocation `Rc`.
- Do not substitute retained clocks or infer affine clocks.
- Clip to the owner only after the connected union.
- An event with no Index contribution must still resolve, through structural
  checks with the attested site/policy binding.
- A missing retained record is refused without VM access.
- No `RefCell` borrow may be held across transcript calls.
- The public `resolve_route` output must stay identical for every existing
  test.

### Tests to add (in `source/issued.rs` and `nested/issued.rs` `#[cfg(test)]` modules, using genuine candidates through `capture_test_route_authority`)

- A cached, rebound nested Slice source with callback reads denied resolves to
  the same route as the legacy route on that topology.
- An event whose discarded augmented origin carries the Slice timing: removing
  that contribution's authentication causes refusal, and the full
  authentication succeeds.
- Two distinct invocations with an equal handle both authenticate before the
  result is returned. A conflicting route between them fails with no result.
- A swapped genuine policy or member, or a transcript from another batch, is
  refused.
- Real gaps and continuations: the union of a partitioned query's routes
  equals the route of the single query.
- A NeedsJointGeometry case resolves on the issued path, and the same case
  still refuses through public `resolve_route`.
- Cumulative work, exact and one less: exact succeeds. One less fails with no
  route, and the collector debit is kept.

### Verification (run in the foreground; record the exit status and full log path)

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-resolution-build.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-resolution-clippy.log 2>&1`
  may report only `dead_code`/`unused_imports` diagnostics of two kinds:
  - a diagnostic that maps to a Route8 disposition row D01-D32 (see
    `song-mode-immutable-route-authority.md`, "Dead-code disposition after
    wave 1"), matched by file and item;
  - a new item added by this plan whose only caller is wave-3 playback
    (`resolve_issued_event` and the items reachable only from it).

  List each diagnostic in the receipt as `{file, line, message, rowId}`, with
  `rowId` either a D-row or `RES-<item>`. Diagnostics from the concurrent
  SONG-SHARED-WORK forwarders may also appear; label them `SW-<item>`. No other
  lint is allowed.

  Import rule (D01): name `PreparedPolicyRef`, `PreparedRoutes` and
  `PreparedSiteRef` as `crate::song::routing::X` in every new or changed
  file. Never use `routing::prepared::X` or `super::prepared::X` outside
  `prepared.rs`. In particular, the `IssuedIndexOperand` field types in
  `lookup/authority.rs` and the issued companion in `canonical.rs` must name
  `crate::song::routing::PreparedPolicyRef`. That consumes the
  `PreparedPolicyRef` part of D01, which wave 3 cannot reach (`routing.rs` is
  not a playback writePath).
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/routing::(source|nested)::issued|prepared::tests/)' > tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
  must exit 0 with a nonzero count.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
  must exit 0. This proves legacy compatibility.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/song-mode-riela/session249-resolution-nextest-full.log 2>&1` must exit 0.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-resolution-wasm.log 2>&1` must exit 0.
- `rustfmt --edition 2021 --check <the eight Rust writePaths, plus src/song/routing/density/index.rs if edited> > tmp/song-mode-riela/session249-resolution-fmt.log 2>&1`
  passes when no `Diff in` line names a touched path.
- `wc -l` on every writePath: each must be below 1000.

### Shared-branch protocol

- Before the first edit, write the intent JSON: the design and plan hashes plus
  the original text and sha256 of each writePath.
- Re-read each target and check its hash before every edit. On drift, stop and
  reconcile.
- If a compile error is caused by another wave-2 worker's file, do not edit it.
  Record it, then re-run.
- No `cargo fmt` on the whole crate. No `git stash`, `git checkout` or
  `git reset`.
- Update only this plan's progress log.

### Done criteria

- [ ] `resolve_issued_event` exists with the pinned signature.
- [ ] All seven test bullets above are present and pass.
- [ ] The legacy focused binaries pass unchanged.
- [ ] Build, full tests and WASM exit 0.
- [ ] Every Clippy diagnostic maps to a Route8 D-row, `RES-<item>` or
  `SW-<item>` in the receipt. The `PreparedPolicyRef` part of D01 no longer
  appears.
- [ ] fmt is clean on touched paths and every file is below 1000 lines.
- [ ] No new `allow`/`expect` attributes.
- [ ] The progress-log entry has commands, exits and log paths.

### 2026-10-04 — Session 250 issued resolver implementation attempt

Implemented the caller-work issued event resolver, source contribution/member
authentication, fresh transcript invocation binding, issued canonical Index
geometry dispatch, and owning tests in the declared Rust paths. The pinned
`PreparedRoutes::resolve_issued_event` seam is present. Corrected fixture
syntax for Euclid arity and the single `play-song` entrypoint. The source
remains incomplete: the required `NeedsJointGeometry` fixture fails before
`resolve_issued_event` during issued preflight. The diagnostic originates in
`src/song/routing/density/index.rs` (`DensityIndex::index_opening`, reached from
the issued preparation builder), which is outside this plan's writePaths.
The distinct equal-handle fixture also reaches `Index address contradicts
first-structure rule`; both issues need scoped follow-up, with the preflight
repair requiring ownership/plan amendment before this plan can pass.

Verification, all in the foreground:

- `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-resolution-build.log 2>&1` — exit 0 on current tree.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-resolution-clippy.log 2>&1` — exit 101; includes an unclassified `clippy::let_and_return` in concurrent `src/song/snapshot/issued.rs:231`, outside this plan's writePaths, in addition to wave disposition warnings.
- Focused issued/prepared nextest command — exit 100; 10 run, 5 passed, 5 failed; complete log `tmp/song-mode-riela/session249-resolution-nextest-focused.log`.
- Corrected-fixture rerun `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/routing::nested::issued::tests::(issued_joint_geometry_resolves_where_legacy_keeps_its_barrier|distinct_equal_handle_invocations_are_all_resolved)/)' >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1` — exit 100; 2 run, 0 passed, 2 failed. The first fails in out-of-plan issued preflight with `kind=Late, issuance=None`; the second fails on the source-address first-structure check.
- Scoped `rustfmt --edition 2021 --check` over the eight Rust writePaths — exit 0; complete log `tmp/song-mode-riela/session249-resolution-fmt.log`.
- `wc -l` over the eight Rust writePaths — exit 0; all paths are below 1000 lines (source.rs 991; nested/issued.rs 916; remaining files 834 or fewer).

Legacy focused binaries, full nextest, WASM, and a Clippy receipt mapping were
not completed because required issued behavior remains blocked and strict
Clippy has a concurrent out-of-scope lint. Keep all completion criteria
unchecked. Resume after the owner of density preflight either adds the exact
admission seam to this plan's authorized paths or implements it in a declared
dependency, and the concurrent `song/snapshot/issued.rs` Clippy diagnostic is
repaired by its owning plan; then correct the equal-handle fixture/address
contract and rerun the required gates on the stable combined tree.

## Session 251 amendment (wave 2a, serial)

Source of truth: design section "Session 251 resume amendments" and user-QA
SM4. This plan runs alone (concurrency 1). SONG-SHARED-WORK and
SONG-STRUCTURAL-CLOCK do not start until this sub-wave is joined and
committed.

### Ninth writePath (operator-authorized)

`src/song/routing/density/index.rs` is added to writePaths for the issued
preparation preflight seam. `DensityWalk` is shared by the legacy and issued
ledgers, and it has no access to retained authority. Therefore:

- Edit it only if a fixture with a statically admissible Index operand is
  refused by the density preflight. Record the failing test and message in the
  receipt before editing.
- Never turn `IndexSupportAdmission::RequiresRealization` into a bound.
  Dynamic Index operands (for example `[cut nil]` with a late-bound function
  variable, `kind=Late`) keep refusing on both ledgers (SM4 default (a)).
- Legacy `prepare_routes` plans stay byte-identical for every existing
  fixture. The `density_tests` module, the legacy focused binaries and full
  nextest are the witnesses.
- If no edit is needed, leave the file unedited and record
  `densityIndex: "authorized, unedited"` in the receipt.
- The file is 516 lines. Keep it below 1000.
- `density.rs`, `prepare.rs` and `prepare/builder.rs` are not writePaths.
  If the seam would need them, stop and record a blocker that cites SM4
  option (b).

### Fixture repairs in `nested/issued.rs`

- The session-250 nested fixtures used `[cut nil]` Index lists, so they failed
  in the preparation preflight. Replace each Index list with a statically
  admissible literal list (for example `[0 nil]` or `[0 1]`). Keep the subject
  lambda `{beat -> p}` and the reusable Part functions, so callback-denied
  replay still has callbacks to deny. Each test still asserts its named
  contract from "Tests to add".
- Distinct equal handles: `slice p 2 [...]` breaks the first-structure rule
  (the subject `p` is structured), which produces `Index address contradicts
  first-structure rule`. Repair the fixture so both `stack` branches and
  `inner` use `slice {beat -> p} 2 [<static Index>]`. Do not edit
  `src/song/routing/index.rs` or relax `slice_index_root`. The test must find
  an event with at least two distinct seals (not `Rc::ptr_eq`). Both seals must
  authenticate before the route is returned. A conflicting-route variant must
  fail with no result.
- NeedsJointGeometry: the legacy check must match the barrier refusal itself.
  The `resolve_route` error message must contain `sampled context requires
  joint mapping geometry`. A bare `is_err()` is not enough. Legacy
  `prepare_routes` on the same program must succeed, so that the refusal comes
  from the barrier and not from preparation. `resolve_issued_event` returns a
  route for the same event.
- `nested/issued.rs` is 916 lines and must stay below 1000. `source.rs` is 991
  lines and may reach 993 at most (983 + 10). Any further growth goes into
  `source/issued.rs`.

### Clippy disposition in a serial sub-wave

Map every diagnostic in `session249-resolution-clippy.log` to
`{file, line, message, rowId}`. Use a Route8 D-row, `RES-<item>` for this
plan's items whose only caller is wave 3, `SW-<item>` for committed 2b paths
(for example `SW-let_and_return` at `src/song/snapshot/issued.rs:231`), or
`ST-<item>` for committed 2c paths. Only `SW-` and `ST-` rows may be lints
other than `dead_code`/`unused_imports`. Do not fix them in this sub-wave.
Any other lint in this plan's nine paths fails the gate. If the
`nested/issued.rs:499` `collapsible_if` reported in session 250 is still
present, fix it without `allow`/`expect`, because the file is in this plan's
paths.

### Evidence

Before rerunning the gates, the orchestrator copies the existing
`tmp/song-mode-riela/session249-resolution-*` files to
`tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session250/` and records their
sha256 values. The gate logs then rewrite the plan-declared paths. Run every
verification command above in the foreground, in order. Record the exit
status and the full log path. Add `src/song/routing/density/index.rs` to the
fmt and `wc -l` commands only if it was edited.

### Session 251 execution steps (in order)

1. Write `tmp/song-mode-riela/session249-resolution-intent.json`: design and
   plan sha256, plus the sha256 and full text of each of the nine Rust
   writePaths. Re-read and hash-check each file before every edit. If a hash
   differs from the intent, stop and reconcile from the current file.
2. Reproduce first. Run the two session-250 failing tests by name and save the
   output in the focused log, before editing anything.
3. Repair the fixtures in `nested/issued.rs` (static Index lists, the
   first-structure rule, the barrier-message assertion). Do not change the
   resolver logic just to make a fixture pass.
4. Rerun the focused filter. If a static-Index fixture is still refused by
   `DensityWalk::index_support` or `index_opening`, apply the
   density/index.rs rules above. If it is refused anywhere outside the nine
   paths (for example `nested.rs:547/555`, which is a writePath and may be
   edited; or `configuration/index.rs:178` or `index.rs`, which may not), and
   the refusal is in an unowned file, stop and record a blocker that cites
   SM4 (b) and the exact message.
5. Fix the remaining lints in this plan's paths without `allow`/`expect`.
6. Run every verification command in the foreground, in order. Write the
   receipt, including the Clippy disposition table and `densityIndex`.
7. Add a progress-log entry to this plan only.

### Session 251 test cases (input -> expected outcome)

- `euclid {slice {beat -> p} 2 [<static Index>]} 1 2` inside a
  `transform-instrument` Part -> legacy `prepare_routes` is `Ok`. Legacy
  `resolve_route` on the event is `Err`, and the message contains
  `sampled context requires joint mapping geometry`. `resolve_issued_event`
  on the same event is `Ok`.
- `stack` of two `slice {beat -> p} 2 [<static Index>]` branches under nested
  `transform-instrument` -> some event has at least 2 non-`Rc::ptr_eq` seals.
  `resolve_issued_event` is `Ok`. Every seal goes through `bind_issued_owner`,
  which the test checks through the collector debit: it grows by at least one
  authentication charge per seal.
- The same program with one contributor's route made to conflict (or with a
  swapped genuine member) -> `Err` and no route.
- The cached nested, discarded-origin and partition fixtures with static
  Index lists -> pass, with the callback-read counter at 0 during resolution.
- The exact-work run is `Ok`. One unit less is `Err`, with no route, and the
  debit is kept.

### Session 251 done criteria (mechanically checkable)

- [ ] `jq` shows `src/song/routing/density/index.rs` in this plan's manifest
  writePaths (already done at plan time).
- [ ] `grep -c "\[cut nil\]" src/song/routing/nested/issued.rs` prints 0.
- [ ] `grep -n "sampled context requires joint mapping geometry" src/song/routing/nested/issued.rs`
  matches inside the NeedsJointGeometry test.
- [ ] `git diff 1ac457f -- src/song/routing/index.rs src/song/routing/density.rs src/song/routing/prepare.rs src/song/routing/prepare/builder.rs src/song/routing/configuration/index.rs`
  is empty.
- [ ] The focused filter exits 0 with every listed resolver test passing. The
  legacy focused binaries, full nextest and WASM exit 0.
- [ ] Every Clippy diagnostic has a row (D-row, `RES-`, `SW-` or `ST-`). No
  non-dead-code lint is left in the nine paths.
- [ ] `rustfmt --check` shows no `Diff in` for touched paths. `wc -l`:
  `source.rs` <= 993 and every path < 1000.
- [ ] `git diff 1ac457f -- <nine paths> | grep -E '^\+.*#\[(allow|expect)'`
  prints nothing.


### Session: 2026-10-04 session 251 implementation attempt

**Status**: Blocked pending an authorized ownership amendment.

**Tasks completed**: Repaired the nested issued fixtures to use statically admissible Index lists, made both equal-handle branches Slice sites, and changed the resolver to pass each contextual seal through the selector binder rather than filtering seals using duplicated scalar owner fields. Temporary diagnostics were removed from the source after investigation. `grep -c "\[cut nil\]" src/song/routing/nested/issued.rs` returned 0. `src/song/routing/density/index.rs` remained authorized and unedited.

**Verification**: Focused reruns did not pass. The latest isolated equal-handle test failed (exit 100; 1 run, 0 passed, 1 failed) in `tmp/song-mode-riela/session249-resolution-nextest-focused.log`: prepared scope 4 retained only issuer `NodeId(3516997820)` with child-1 prefix, while the original frozen pattern contains another Slice issuer `NodeId(3172401064)` with child-0 prefix. The selector cannot authenticate the missing record. A prior same-snapshot NeedsJointGeometry witness run in the same append-only log failed with `original source membership missing` instead of the required legacy barrier text. The full build, clippy disposition receipt, legacy focused binaries, full nextest, WASM and scoped fmt/line-count gates were not rerun after these edits.

**Ownership blocker**: The observed missing request points to request de-duplication in `src/song/snapshot/occupancy.rs` (`CanonicalIndexRequest::same_execution` does not distinguish issuer/prefix for otherwise shared recipe/scope/window). That file is not in this plan's nine concrete writePaths, the fanout `writePaths`, or the current accepted amendment. Repairing it and adding a regression test requires an explicit plan/manifest ownership amendment. No edit was made to that file, and no selector or route-index security checks were relaxed. Resume after the accepted plan and dispatch manifest add that exact path and test location to owned writePaths, then preserve every distinct issuer/prefix request and rerun the focused requirements before the remaining gates.

**Progress**: Assigned implementation remains incomplete. Do not mark TASK-002, TASK-003, or the session-251 done criteria complete. Evidence and per-edit snapshots are under `tmp/song-s249/SONG-ISSUED-RESOLUTION/session251/`.

## Session 252 amendment (wave 2a, serial)

Source of truth: design section "Session 252 resume amendments (2026-10-04)"
in `design-docs/specs/design-song-mode.md`. This amendment adds TASK-004 and
four owned paths. Everything from the session 249, 250 and 251 sections still
applies unless this section says otherwise. This plan still runs alone.

### Intent and context

Session 251 stopped on a real retention defect, not on a fixture problem.
`CanonicalIndexRequest::same_execution` (`src/song/snapshot/occupancy.rs:143`)
compares eight fields (original, recipe, scope, track, revision, root, window,
depth). It does not compare `issuer` or `prefix`. `retain_index_occupancy`
(`occupancy.rs:213-229`) uses it to coalesce requests. When two Slice sites sit
under the same root and window, the second site therefore gets no record. The
issued selector in `bind_issued_owner` (`lookup/authority.rs:296-310`) filters
records by exact `issuer` and `prefix`, so it cannot find the second site.
`distinct_equal_handle_invocations_are_all_resolved` fails for that reason: in
scope 4 only one of two issuers was retained.

### New owned paths (operator authorizations 1 and 2)

| Path | Kind | Allowed edit |
|---|---|---|
| `src/song/snapshot/occupancy.rs` | writePath (auth. 1) | Split identity in `same_execution`; site-alias retention; `site_alias` field |
| `src/song/snapshot/occupancy/tests.rs` | writePath (auth. 1) | Append exactly one new `#[test]`; no existing line changed |
| `src/song/snapshot/occupancy/lookup.rs` | sharedPath (auth. 2) | `AuthorityRecord` gains `site_alias`; alias skip in `owner_addresses` and the configuration-row loop; nothing else |
| `src/song/snapshot/occupancy/route_view.rs` | sharedPath (auth. 2) | `PublishedRetainedIndex` gains `site_alias`, copied at publication; nothing else |

`lookup/authority.rs` and `prepared.rs` are already writePaths of this plan.
No other Route8 path may be edited. The cohort stays at 952 because no file is
created.

### TASK-004: Retained site identity and site aliases

**Status**: Completed (implementation; pending independent review)
**Parallelizable**: No (it must land before the TASK-002/TASK-003 reruns)
**Deliverables**: the four paths above

Changes, file by file:

1. **`occupancy.rs`**
   - Keep the current eight-field comparison as a new private method,
     `fn shares_execution(&self, other: &Self) -> bool`. Its body is exactly
     today's `same_execution` body.
   - Change `same_execution` to `shares_execution(other) && self.issuer ==
     other.issuer && self.prefix == other.prefix`. That is exact `Vec`
     equality. Do not use `trace_matches`, because it is a prefix match meant
     for rows.
   - `RetainedCanonicalIndex` gains the private field `site_alias: bool`. Every
     existing construction sets it to `false`.
   - Rewrite the scan inside the `for request in requests` loop of
     `retain_index_occupancy`:
     - For each record in `snapshot.occupancy.iter().chain(&pending)`, charge
       `request.prefix.len() as u64 + 1`, exactly as today.
     - If `record.request.same_execution(&request)`: this is a full site match.
       Coalesce as today: run the cached-depth refusal, then `continue`, and
       stop scanning.
     - Otherwise, if `record.request.shares_execution(&request) &&
       !record.site_alias`, remember the first such record as `primary`. Keep
       scanning, because a later alias may be a full site match.
     - After the scan, if there was no full site match but there is a
       `primary`:
       - run the same cached-depth refusal against `primary`;
       - charge `primary.observations.len() + primary.calls.len() +
         primary.invocations.len() + 1` as `u64`, with checked addition;
       - run the same `limits.check_events(occupancy.len() + pending.len() + 1)`
         check;
       - push a new `RetainedCanonicalIndex` with `request` = the incoming
         request (moved), cloned `observations` and `calls`, `invocations`
         cloned as `Rc` clones (the same allocations), the primary's
         `peak_depth`, `vm_instructions` and `admitted_depth`, and
         `site_alias: true`;
       - `continue`. Do not call `observe_part`, `prepare_owner_dependencies` or
         any VM or evaluator method.
     - Otherwise, execute as today.
   - Borrowing: `primary` borrows from `pending`, and the push mutates
     `pending`. Copy the needed values first, or record the primary's index in
     the chained iteration, and then push. Do not hold a borrow across the push.
   - `replay_site` keeps calling `same_execution`. It now refuses a request from
     another site with the existing `foreign canonical replay address` error.
     This is intended.
2. **`route_view.rs`**
   - `PublishedRetainedIndex` gains `pub(super) site_alias: bool`.
   - At publication (around line 386), copy `record.site_alias`. Make no other
     change. The alias still goes through `authenticate_request`, the site
     match and the invocation checks, exactly like any other record.
3. **`lookup.rs`**
   - `AuthorityRecord` gains `site_alias: bool`, filled from both iterator
     arms.
   - In `owner_addresses` (`lookup.rs:95`), after the existing per-record
     charge, `continue` when `record.site_alias`. This loop iterates
     `snapshot.occupancy` directly, so read the field there.
   - In the configuration-row loop (`lookup.rs:408`), after the existing
     per-record authentication block, `continue` when `record.site_alias`.
     Skip both its rows and its `insert_coverage`.
   - Do not touch the site-scoped checks at `lookup.rs:206`, `:293` or `:387`.
4. **`lookup/authority.rs`**: no change for TASK-004. The selector at `:296` is
   site-scoped and must find aliases. Do not add an alias skip there.

Code to imitate:

- Site traversal for a fixture with several sites:
  `src/song/snapshot/occupancy/geometry_tests/domains.rs:actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct`
  (lines 143-191).
- Callback journal assertion: `src/song/snapshot/occupancy/tests.rs:cuts`.
- Batch atomicity: `occupancy.rs:296-303`. Publish only after every request
  succeeds.

Key pitfalls (do not):

- Do not add `issuer`/`prefix` to `same_execution` and stop there. A second
  execution creates new `OwnerInvocation` allocations that no issued seal
  refers to, and it duplicates rows in the configuration loop.
- Do not skip aliases in `bind_issued_owner` or in publication. The alias
  exists for the site-scoped selector.
- Do not change the per-record charge or the break-on-full-match behavior.
  `assert_clock_retention_transaction` asserts `remaining == 0` for exact work.
- Do not edit `src/song/snapshot/occupancy/geometry_tests*`,
  `lookup_tests.rs`, `replay_tests.rs`, `invocation_tests.rs`, or any
  existing test in `tests.rs`.
- Do not run rustfmt in write mode on `occupancy.rs` or `lookup.rs`. Both
  have child modules that are not owned here. Hand-format the new lines in the
  surrounding style, then use `--check` only.

Test to append to `src/song/snapshot/occupancy/tests.rs` (one `#[test]`, named
`distinct_sites_sharing_one_execution_are_retained_separately`):

- `from_code` with
  `fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tstack [{slice {beat -> p} 2 [cut nil]} {slice {beat -> p} 2 [cut nil]}]\n`
  plus the `base`/`selected`/`song` lines of `prepared_with_deletion(_, false)`.
  Collect the Slice sites with the `domains.rs` traversal. Mint one request per
  site over `TimeSpan::cycle(0)`. Then retain both in one batch:
  - there are exactly 2 records, with distinct `(issuer, prefix)`;
  - exactly one record has `site_alias`;
  - the alias's `invocations` equal the primary's element by element under
    `Rc::ptr_eq`;
  - `cuts(alias).len() == cuts(primary).len()`;
  - `primary.vm_instructions > 0`.
- Count with `owner_addresses` for each site's request. The address count
  equals the primary record's `invocations.len()`, with no doubling.
- Re-mint both site requests and retain them again in one batch. The
  `occupancy.len()` stays 2.
- `primary.replay_site(&alias_request, ...)` fails with a message containing
  `foreign canonical replay address`.

The Index list may have to be static (`[0 nil]`) if the preparation preflight
refuses `[cut nil]` for two sites. In that case, drop the `cuts` check and keep
every other assertion.

### Joint-geometry fixture: conditional Euclid substitution

`issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` uses
`euclid {slice ...} 1 2`. At `980083a`, Euclid still has a structural clock
barrier (`src/pattern/eval/song_clock/dispatch.rs:13`), and SONG-STRUCTURAL-CLOCK
(2c) removes it later. Session 251 saw this test fail with
`original source membership missing` (`lookup.rs:792`, `bind_member`).

After TASK-004, rerun the test by name.

- If it passes, keep the Euclid fixture.
- If it still fails with `original source membership missing`, confirm the
  cause. Record in the receipt whether the Euclid node goes through
  `with_clock_unknown`, using a fixture-local check or existing test helpers;
  do not leave diagnostics in the source. If that is confirmed, replace only
  the fixture's sampling combinator. Use an instrumented combinator from the
  `false` arm of `with_clock_dispatch` that still yields
  `SourceLocatorMapping::NeedsJointGeometry` on the legacy path, for example
  `segment` or `grid` over the Slice. Keep every assertion: legacy
  `prepare_routes` is `Ok`, the legacy error contains
  `sampled context requires joint mapping geometry`, `resolve_issued_event` is
  `Ok`, and the handle is the same. Record `jointGeometryFixture:
  "euclid-deferred-to-2c"` and the chosen combinator in the receipt.
  SONG-STRUCTURAL-CLOCK then adds the Euclid issued witness.
- If no instrumented combinator yields NeedsJointGeometry, stop and record a
  blocker with the exact messages. Do not change resolver logic or relax any
  barrier.

### Session 252 execution steps (in order)

1. Write `tmp/song-mode-riela/session249-resolution-intent.json` again. It
   holds the design sha256, the plan sha256, and the sha256 and full text of
   all thirteen Rust paths: the nine from session 251 plus the four above.
   Re-read and hash-check each file before every edit.
2. Reproduce before editing. Run the named-tests command below with its output
   redirected to `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/reproduce.log`
   instead of the focused log. Record the exit status, whatever it is. A
   nonzero exit is expected, because the new test does not exist yet.
3. Implement TASK-004 and append the regression test.
4. Run the regression test, then the two named resolver tests. Apply the
   joint-geometry rule above if needed.
5. Finish whatever TASK-002/TASK-003 items the reruns still show failing,
   inside the thirteen paths only.
6. Run every verification command in order, in the foreground. Write the
   receipt, including:
   - the Clippy disposition table (rows D-, `RES-`, `SW-`, `ST-`);
   - `densityIndex`;
   - `jointGeometryFixture`;
   - `evidenceFingerprint`: the receipt sha256, which must differ from
     `04db7146...`, `4945fec7...`, `15c388bd...` and `0493996e...`;
   - the `git diff --quiet` result for the three unowned paths.
7. Add one progress-log entry to this plan. Edit no other plan.

### Session 252 verification (foreground; record the exit status and full log path)

- Named tests. This is the first gate, and it starts the focused log:
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib distinct_equal_handle_invocations_are_all_resolved issued_joint_geometry_resolves_where_legacy_keeps_its_barrier actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct distinct_sites_sharing_one_execution_are_retained_separately > tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`.
  It must exit 0 with 4 tests passed. Every later focused command appends
  with `>>`, in the order the manifest gives.
- Occupancy and Route8 regression:
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/snapshot::occupancy|routing::prepared/)' >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`.
  It must exit 0, with the same test names as before plus the one new test.
- Every session 249 verification command above (build, Clippy, the
  resolver-focused filter, the legacy binaries, the full suite and WASM), to
  the same log paths.
- Format:
  `rustfmt --edition 2021 --check src/song/routing/source.rs src/song/routing/source/issued.rs src/song/routing/nested.rs src/song/routing/nested/issued.rs src/song/routing/configuration.rs src/song/routing/configuration/index/canonical.rs src/song/routing/prepared.rs src/song/snapshot/occupancy/lookup/authority.rs src/song/snapshot/occupancy.rs src/song/snapshot/occupancy/tests.rs src/song/snapshot/occupancy/lookup.rs src/song/snapshot/occupancy/route_view.rs > tmp/song-mode-riela/session249-resolution-fmt.log 2>&1`.
  It passes when no `Diff in` line names one of these paths. Add
  `src/song/routing/density/index.rs` if it was edited. `Diff in` hunks for
  unowned child modules are recorded and not fixed.
- `wc -l` on the thirteen paths. Each must be below 1000, and `source.rs` must
  be at most 993.
- `git diff --quiet 980083a -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs`
  must exit 0.

### Session 252 done criteria (mechanically checkable)

- [ ] `grep -n "fn shares_execution" src/song/snapshot/occupancy.rs` matches.
  `same_execution` compares `issuer` and `prefix`.
- [ ] `grep -c "site_alias" src/song/snapshot/occupancy/lookup.rs` is at least
  3, and `grep -c "site_alias" src/song/snapshot/occupancy/route_view.rs` is at
  least 2.
- [ ] `git diff 980083a -- src/song/snapshot/occupancy/tests.rs` contains only
  added lines (`grep -c '^-[^-]'` prints 0).
- [ ] `git diff 980083a --name-only -- src/song/snapshot/occupancy/` lists no
  path other than `tests.rs`, `lookup.rs`, `route_view.rs` and
  `lookup/authority.rs`.
- [ ] The four named tests pass. The occupancy and Route8 filter, the
  resolver-focused filter, the legacy binaries, the full suite, build and WASM
  all exit 0.
- [ ] Every Clippy diagnostic has a row. There is no non-dead-code lint in the
  thirteen paths, and no new `allow`/`expect`
  (`git diff 980083a -- <thirteen paths> | grep -E '^\+.*#\[(allow|expect)'`
  prints nothing).
- [ ] The receipt has `jointGeometryFixture`, `densityIndex` and a fingerprint
  that differs from the four prior values.
- [ ] The progress-log entry lists every command, exit status and log path.

### Session 252 — Step 6 implementation handoff

**Status**: Incomplete; TASK-004 is implemented, TASK-002/TASK-003 remain blocked by issued source membership resolution.

**TASK-004 completed**: `CanonicalIndexRequest` now separates shared execution identity from exact issuer/prefix site identity. Retention preserves later sites as charged aliases that reuse the primary record's observations, calls and `Rc`-identical invocations. Site-scoped lookup retains aliases while owner-address and configuration enumeration skip duplicate alias work. Added the single occupancy regression `distinct_sites_sharing_one_execution_are_retained_separately`; it passes. No existing test assertion changed. `src/song/routing/density/index.rs` remains `authorized, unedited`.

**Resolver state**: The pinned `PreparedRoutes::resolve_issued_event` path and its owning tests are present, but final-source tests still fail with `original source membership missing` for `distinct_equal_handle_invocations_are_all_resolved`, `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`, and `cached_nested_slice_events_resolve_from_issued_transcript`. Do not weaken member/source-boundary checks: diagnostics showed three authenticated candidate seals, with one exact retained Index row but no matching member in its retained boundary; the other two had no exact row. The required legacy NeedsJointGeometry barrier message was therefore not reached. A second resolver-suite run on a moving exploratory tree also found `discarded_augmented_source_origins_are_resolved` failing on a late Index preflight and `partitioned_nested_issued_queries_equal_the_full_route_set` failing on source membership; see its log.

**Verification (foreground; current source unless noted)**:

- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib distinct_sites_sharing_one_execution_are_retained_separately distinct_equal_handle_invocations_are_all_resolved issued_joint_geometry_resolves_where_legacy_keeps_its_barrier cached_nested_slice_events_resolve_from_issued_transcript` — final-source exit 100; 4 run, 1 passed, 3 failed; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/final-focused-current.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/routing::(source|nested)::issued|prepared::tests/)'` — exploratory moving-tree run exit 100; 11 run, 6 passed, 5 failed; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/attempt22-issued-resolver-suite.log`.
- `CARGO_TERM_QUIET=true cargo build` — final-source exit 0; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/final-current-build.log`.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` — final-source exit 101; warning-as-error diagnostics include only dead-code/unused-imports dispositions plus the concurrent `clippy::let_and_return` at `src/song/snapshot/issued.rs:231`; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/final-current-clippy.log`.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets` — exit 0 with warnings; complete disposition inventory in `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/final-clippy-warnings.log`.
- Recursive `rustfmt --edition 2021 --check` on the plan Rust paths — exit 1 only for unchanged `src/song/snapshot/occupancy/lookup/authority.rs`; exact changed-file check with `--config skip_children=true` — exit 0; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/final-fmt-current.log` and `final-fmt-recursive-current.log`.
- `wc -l` — exit 0; `source.rs` is 991 lines and every listed resolver/occupancy path is below 1000; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/final-line-counts.log`.
- `git diff --quiet 980083a -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs` — exit 0; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/verification/final-unowned-paths.log`.
- Clippy warning dispositions are recorded per diagnostic in `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/clippy-dispositions.json`; final append-only receipt and evidence fingerprint are in `tmp/song-s249/SONG-ISSUED-RESOLUTION/session252/session252-final-receipt.json` (`3c6c504cf4e66accaf04cfedecab68895eb0e40dc149be59dbd60b203f277db6`).

Legacy focused binaries, full nextest, WASM, the occupancy/Route8 aggregate, and the complete Session 252 lint-disposition receipt remain unverified because required issued routing behavior is failing. Do not mark TASK-001/TASK-002/TASK-003 or overall completion criteria done. Continue with the ownership-safe diagnosis of how the exact invocation is paired with its genuine source member/boundary, then rerun the named tests before the remaining gates. Formal test-integrity/adversarial review and later workflow finalization remain downstream.

## Session 253 amendment (wave 2a, serial)

Source of truth: the design section "Session 253 resume amendments
(2026-10-04)" in `design-docs/specs/design-song-mode.md`, plus the operator
diagnosis in `tmp/song-mode-riela/session252-root-cause-diagnosis.md`. This
amendment adds TASK-005 to TASK-007 and one new file. Every rule from the
session 249 to 252 sections still applies unless this section overrides it.
This plan still runs alone (concurrency 1). TASK-004 stays as implemented at
`6543273`; do not redo it.

### Intent and context

Five resolver tests fail at `6543273`:

- Four fail with `original source membership missing` (`lookup.rs:799`):
  - `cached_nested_slice_events_resolve_from_issued_transcript`
  - `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`
  - `distinct_equal_handle_invocations_are_all_resolved`
  - `partitioned_nested_issued_queries_equal_the_full_route_set`
- `discarded_augmented_source_origins_are_resolved` fails at preparation with
  `Index requires shared canonical realization` (`src/song/routing/index.rs:80`).
  Its fixture still uses the dynamic Index list `[cut]`.

Root cause of the first four failures:

- `canonical_index_configuration_issued`
  (`src/song/routing/configuration/index/canonical.rs:417-428`) calls the
  legacy `bind_member` with the consuming stage owner's own address.
- `bind_member` walks the source boundaries of that address's invocation.
- Source boundaries are attached only to the invocation clocks of source-side
  owners (`src/pattern/eval/song_clock.rs:426` `insert_source`, `:838`
  `with_source_boundary`).
- So the outermost owner's chain is always empty, and the call can never
  succeed.

Membership must instead be read from the fresh invocation clocks of all of
the event's seals.

Session 251 also left two test-integrity defects, which are repaired here:

- the weakened joint-geometry test;
- the `continue` that replaced the hard failure
  `issued Index event has no canonical component`.

### Non-goals

- No change to the behavior of the legacy `bind_member`,
  `canonical_index_configuration`, `resolve_route`, `prepare_routes` or
  `RetainedIndexAddress::source_boundaries`. These legacy fixtures keep
  passing unchanged:
  - `geometry_tests.rs:313-379`;
  - `geometry_tests/domains.rs:597-616`;
  - `lookup_tests.rs:205-251`.
- None of these are relaxed:
  - the check at `canonical.rs:251` (`issued selected source classification
    is required`);
  - `slice_index_root` in `src/song/routing/index.rs`;
  - any NeedsJointGeometry barrier on the legacy path.
- No dynamic Index preparation admission (user-QA SM4 default (a)).
- None of these files are edited:
  - `src/pattern/eval/song_clock.rs`, `song_clock_projection.rs`,
    `song_provenance.rs`, `song_replay.rs`;
  - any 2b, 2c or wave-3 path;
  - the three unowned paths.
- No new `allow`/`expect` attribute, no new public API, and no new test helper
  file.

### Owned paths for session 253

| Path | Kind | Allowed edit |
|---|---|---|
| `src/song/routing/nested/issued/members.rs` | writePath (new) | Stage owner frame, parent-use policy and member-binding call (TASK-006) |
| `src/song/routing/nested/issued.rs` | writePath | `mod members;`, owner-frame filter, parent-use policy, hard failure, fixture restorations (TASK-006, TASK-007) |
| `src/song/snapshot/occupancy/lookup/authority.rs` | writePath | `IssuedMemberQuery`, `bind_issued_member`, `selected_policy_in` (TASK-005) |
| `src/song/routing/configuration/index/canonical.rs` | writePath | Remove the `bind_member` call from `canonical_index_configuration_issued` only (TASK-006) |
| `src/song/routing/configuration.rs` | writePath | Only if `issued_index_configuration` cannot already accept `Option<&FrozenSelectedSource>`; at most 7 parameters |
| `src/song/routing/density/index.rs` | writePath | Session 251 rule unchanged: edit only if the static discarded-origin fixture is still refused |
| `src/song/snapshot/occupancy/lookup.rs` | sharedPath | Session 252 alias skip (done). In addition, the body of `RetainedIndexAddress::selected_policy` moves to `authority.rs`, and the method becomes a one-call delegation. Nothing else changes. |

These paths remain writePaths or sharedPaths from earlier sessions, but no
edit is planned for them in session 253:

- `occupancy.rs`, `occupancy/tests.rs` and `route_view.rs`;
- `prepared.rs`;
- `source.rs`, `source/issued.rs` and `nested.rs`.

Edit one of them only to fix a compile error that this session's change
causes, and record that edit in the receipt. The cohort goes from 952 to 953
because of the one new file.

### TASK-005: Issued member binder (`lookup/authority.rs`, `lookup.rs`)

**Status**: Incomplete; binder implemented, required resolver tests still fail
**Parallelizable**: No (TASK-006 calls it)

1. **Move `selected_policy` into `authority.rs`.** `selected_policy` is a private
   method on `RetainedIndexAddress` (`lookup.rs:519-581`). It reads only
   `self.owner.authority`.
   - Move its body unchanged into `authority.rs` as:
     `pub(in crate::song::snapshot::occupancy) fn selected_policy_in(authority: LookupAuthority<'_>, selected: &FrozenSelectedSource, boundary: &SourceBoundaryRef<'_>, depth: u32, budget: &mut ProjectionBudget<'_>) -> Result<Option<Vec<u32>>, Failure>`.
   - The `lookup.rs` method keeps its signature, and its body becomes a single
     call:
     `authority::selected_policy_in(self.owner.authority, selected, boundary, depth, budget)`.
   - `LookupAuthority` is `Copy` (`lookup.rs:10`), and `policy_path` already
     lives in `authority.rs`.
   - Behavior, charges and error strings stay byte-identical. The error
     strings are `ambiguous exact selected policy parent use` and
     `source boundary producer does not authenticate original use`.
   - `lookup.rs` gets shorter.
2. **Add the query type and the binder in `authority.rs`.** The contract is
   pinned and has 5 parameters:

   ```rust
   pub(crate) struct IssuedMemberQuery<'a, 'p> {
       pub(crate) site: crate::song::routing::PreparedSiteRef<'a>,
       pub(crate) owner: &'p CanonicalOwnerFrame,
       pub(crate) selected: &'p crate::song::snapshot::FrozenSelectedSource,
       pub(crate) handle: &'p crate::song::EventHandle,
   }
   pub(crate) fn bind_issued_member<'a>(
       query: &IssuedMemberQuery<'a, '_>,
       transcript: &'a IssuedQueryTranscript,
       seals: &'a [Rc<InvocationSeal>],
       work: &SharedIndexWork,
       depth: u32,
   ) -> Result<RetainedSourceMember<'a>, Failure>
   ```

   Behavior, in order:
   - Set `authority = LookupAuthority::Issued(query.site.authority())`.
   - Look up `part = authority.inventory().parts.get(query.selected.root_part)`.
     If it is absent, fail with `member source root`, exactly as
     `bind_member` does (`lookup.rs:775-781`).
   - For each seal in `seals`, in order:
     - Call `actual = transcript.invocation(seal, work, depth)?`. This uses the
       fresh clock and already authenticates the seal.
     - Inside `with_work(work, |limits, remaining| ...)`, with one
       `ProjectionBudget`, call
       `actual.lookup_clock().retained_sources(actual.lookup_owner(), depth, &mut budget)?`.
       This is the same call `bind_issued_owner_if_matching` makes at
       `authority.rs:366-368`.
   - For each boundary, apply these four checks in order. Skip the boundary
     at the first check that fails:
     1. `same_intrinsic_owner(boundary.policy_parent()?, query.owner)`;
     2. `boundary.policy_matches(part.revision, part.duration, query.selected, &mut budget)?`;
     3. `selected_policy_in(authority, query.selected, &boundary, depth, &mut budget)?` is `Some(edges)`;
     4. `boundary.member(query.handle, depth, &mut budget)?` is `Some(origin)`.
   - Result rule:
     - Keep the first match as `(origin, edges)`.
     - Accept a later match only if `std::ptr::eq(origin, kept_origin)` and
       `edges == kept_edges`. Otherwise fail with
       `ambiguous original member binding`.
     - If no seal yields a match, fail with
       `original source membership missing`.
   - Return `RetainedSourceMember { origin, edges }`. Its private fields are
     visible here because `authority.rs` is a child of `lookup.rs`.
   - Never rebase, clip or copy the origin. The original source START is
     whatever the retained origin holds.
3. Do not touch `bind_member`, `bind_issued_owner`,
   `bind_issued_owner_if_matching` or `bind_issued_index`.

Code to imitate:

- `lookup.rs:bind_member` (lines 765-800) for the loop and the error strings.
- `authority.rs:bind_issued_owner_if_matching` (lines 268-376) for the
  `with_work` work bridging and the fresh `transcript.invocation` call.

### TASK-006: Stage call site, owner filter, parent-use policy, hard failure

**Status**: Incomplete; stage wiring implemented, required owner binding still fails
**Parallelizable**: No (depends on TASK-005)
**Deliverables**: `src/song/routing/nested/issued/members.rs` (new),
`src/song/routing/nested/issued.rs`,
`src/song/routing/configuration/index/canonical.rs`

1. **`nested/issued.rs`.** Add `mod members;` at item level, not inside
   `mod tests`. `members.rs` is a child module, so it can reach `Stage`,
   `IssuedResolution`, `WorkBridge` and `sync_local_work` through `super::`.
2. **`members.rs`** holds the four `pub(super)` helpers below. The signatures
   are indicative; keep each one below 8 parameters.
   - `fn owner_matches(frame: &CanonicalOwnerFrame, stage: &Stage<'_>) -> bool`
     is true if and only if all four hold:
     - `frame.root == stage.output_owner.payload().id`;
     - `frame.track == stage.handle.track()`;
     - `frame.revision == stage.handle.revision()`;
     - `frame.placement == *stage.handle.placement()`.

     This is exactly the predicate that session 251 removed. See the first
     hunk of `git diff 1ac457f 6543273 -- src/song/routing/nested/issued.rs`.
   - `fn stage_owner_frame(...)` walks `context.seals` in order.
     - For each seal it calls `transcript.invocation(seal, bridge.work, depth)`
       and charges it through the bridge. Use the same
       before/after/`sync_local_work` pattern as `nested/issued.rs`
       lines 551-559.
     - It returns the `lookup_owner()` of the first seal for which
       `owner_matches` is true.
     - If no seal matches, it fails with the existing string
       `issued Index timing has no matching fresh invocation`.
   - `fn parent_use_policy(...) -> Result<Option<PreparedPolicyRef<'_>>, Failure>`:
     - It returns `None` when `stage_index == 0`.
     - Otherwise it uses the enclosing stage, `stages[stage_index - 1]`.
       Index 0 is the outermost stage: in `inherited_policy_route`,
       `stages.get(index + 1)` is the inner one.
     - It builds the enclosing stage's site with
       `prepared.site(output_scopes[stage_index - 1], enclosing.handle.track(), ...)`.
     - It returns
       `Some(prepared.policy(enclosing_site, enclosing.identity.policy as usize, ...))`.
   - `fn bind_timing_member(...)` builds
     `IssuedMemberQuery { site, owner: stage_owner_frame, selected, handle: bound.timing.subject_handle() }`.
     - `selected` is the stage consuming policy,
       `prepared.policy(site, stage.identity.policy as usize, ...)`, bound
       with `bind_original`.
     - It calls
       `bind_issued_member(&query, transcript, context.seals, bridge.work, depth)`
       inside the same work-bridge pattern as lines 568-583.
     - The returned member is dropped after success, because it serves only
       as an authentication gate. Do not thread it into `configuration.rs`
       unless a failing test proves that the geometry consumer needs it.
3. **Changes in `issued_index_stage_configuration`.** Inside the
   `for (timing_index, timing)` body, after `bound`, `site` and `selector` are
   built and before `let mut timing_result = None;`:
   - Call `stage_owner_frame`, then `bind_timing_member`. An error from either
     one fails the event.
   - Compute `parent_use = parent_use_policy(...)` once per timing.
4. **Changes in the seal loop:**
   - Restore the owner-frame filter. Keep the fresh invocation result
     (`let actual = transcript.invocation(...)`), and `continue` when
     `!members::owner_matches(actual.lookup_owner(), stage)`.
   - Pass `parent_use` to `bind_issued_index` instead of `Some(policy)` of the
     consuming policy.
   - The consuming-policy lookup at lines 560-567 moves into
     `bind_timing_member`. Remove it from the loop if the loop no longer uses
     it.
   - Keep `let Some(operand) = operand else { continue; };`. It covers an owner
     seal that has no retained request at this site.
   - `selected` becomes
     `operand.policy().map(|p| p.bind_original(...)).transpose()?`, which is
     an `Option<&FrozenSelectedSource>`, and is passed straight to
     `issued_index_configuration`.
   - Drop the `issued index policy binding missing` error, because `None` is
     now legal for the outermost stage.
   - Restore the hard failure. If `issued_index_configuration(...)?` returns
     `None`, the event fails with
     `issued Index event has no canonical component`, as at `1ac457f`. Remove
     the `else { continue; }`.
   - The conflict error and the `no matching fresh invocation` error stay
     unchanged.
5. **`canonical.rs`.** In `canonical_index_configuration_issued`:
   - Delete the
     `if let Some(selected) = selected { ... bind_member(...) ... }` block
     (lines 416-428).
   - Remove the now-unused `limits` binding if clippy flags it.
   - Leave `validate_issued_binding`, `empty_source` and
     `row_source_window` unchanged.
   - Keep the `bind_member` import, which is still used at line 323.
6. **Line limits.** `nested/issued.rs` must end at no more than 949 lines, so
   that the 2c Euclid witness (at most 50 lines) still fits below 1000. Move
   helper bodies into `members.rs` rather than growing `nested/issued.rs`.

### TASK-007: Fixture integrity restorations (`nested/issued.rs` tests)

**Status**: Incomplete; fixtures restored as specified, required outcomes remain unproven
**Parallelizable**: No (verify together with TASK-006)

Diff the tests module against `1ac457f` and `6543273`, both before and after
the edits.

- **`issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`.** Restore
  the `1ac457f` shape: use `capture(program)`, `prepare` and `attached_work`,
  and take the first event (`batch.events().first()`). Assert all four:
  1. the legacy
     `prepare_routes(song.snapshot(), &CapabilitySet::native(), &capacities())`
     is `Ok` (keep the legacy plan it returns);
  2. `resolve_route(&legacy_plan, event.descriptor(), limits())` is `Err`, and
     its message contains `sampled context requires joint mapping geometry`;
  3. `routes.resolve_issued_event(&batch, 0, &work, 0)` is `Ok`;
  4. same handle: the handle checked on the legacy path equals the handle of
     the issued event at index 0.

  The program is the `1ac457f` program with at most two substitutions:
  - a static Index list (`[0 nil]`), with the `fn cut` line dropped;
  - only under the session 252 conditional Euclid rule: `euclid` replaced by
    `segment` or `grid` over the same Slice.

  Record `jointGeometryFixture`: `"euclid"`, or `"euclid-deferred-to-2c"` plus
  the combinator used. Remove the nested-slice program and the "some event
  hits the barrier" loop. If no allowed program reaches the legacy barrier
  while the issued path resolves, stop and report the exact messages.
- **`discarded_augmented_source_origins_are_resolved`.** Change `[cut]` to
  `[0]` and drop the `fn cut` line. Change nothing else.
  - If preparation still refuses, apply the session 251 density seam rule.
  - If `found_discarded_augmented` then fails, stop and report. Do not weaken
    the test.
- **`distinct_equal_handle_invocations_are_all_resolved`.** Restore
  `slice p 2` in `inner` and in both `outer` stack arms; session 251 had
  changed them to `slice {beat -> p} 2`. Keep `[0 nil]` and the added
  seal-debit assertion. If the restored fixture has no event with two or more
  distinct seals (the `ok_or_else` after the `find`), stop and report.
- **`cached_nested_slice_events_resolve_from_issued_transcript`** and
  **`partitioned_nested_issued_queries_equal_the_full_route_set`.** Keep their
  `[cut nil]` to `[0 nil]` changes, which are authorized SM4 substitutions.
  Make no other change.
- No other test line changes. No assertion is deleted or loosened.

### Invariants

- Legacy APIs and their outputs are unchanged. Legacy fixture tests pass with
  zero edits.
- Only allocation identity grants membership: `std::ptr::eq` on the origin,
  and `Rc` seal authentication. Scalar equality never grants it.
- Every contributor authenticates before `timing_result` coalescing and before
  the `result` intersection.
- Work comes only from the caller's `SharedIndexWork`, through the bridge.
  There is no fresh budget, and no `RefCell` borrow is held across transcript
  calls.
- Every touched Rust file stays below 1000 lines. `nested/issued.rs` ends at
  no more than 949 lines, and `source.rs` at no more than 993.
- The three unowned paths equal `6543273`. `.agents/settings.local.json` is
  not touched.

### Key pitfalls (do not)

- Do not call or modify the legacy `bind_member` from the issued path.
- Do not walk only the owner seal's clock. The boundaries live on source-side
  seals; per the diagnosis, seal 0 has none and seals 1 and 2 hold them.
- Do not count one boundary seen in two seals as ambiguous. Compare origins by
  `std::ptr::eq` and edges by value.
- Do not pass the stage consuming policy to `bind_issued_index` any more. That
  argument is now the address parent-use policy.
- Do not keep any `continue` for a `None` canonical component on an
  owner-matching bound seal.
- Do not skip the owner-frame filter on the grounds that `bind_issued_index`
  already filters. It does not: its `None` is scoped to the site, not to the
  seal.
- Do not run rustfmt in write mode on `lookup.rs` or `occupancy.rs`, whose
  children are unowned. Running it on `nested/issued.rs` also formats the
  owned `members.rs` and is allowed. Prefer `--check` plus hand formatting.
- Do not fix `clippy::let_and_return` at `src/song/snapshot/issued.rs:231`.
  It is `SW-let_and_return` and belongs to 2b.

### Tests (input -> expected outcome)

- `cached_nested_slice_events_resolve_from_issued_transcript` -> passes, with
  no `original source membership missing`.
- `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` (restored)
  -> the legacy plan is `Ok`, the legacy error contains the barrier text, the
  issued path is `Ok`, and the handle is the same.
- `distinct_equal_handle_invocations_are_all_resolved` (with `slice p 2`) ->
  passes, with at least two distinct seals on the chosen event.
- `partitioned_nested_issued_queries_equal_the_full_route_set` -> passes.
- `discarded_augmented_source_origins_are_resolved` (static list) -> passes,
  and `found_discarded_augmented` is true.
- `actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct` and
  `distinct_sites_sharing_one_execution_are_retained_separately` -> still
  pass.
- Legacy fixtures in `snapshot::occupancy` (geometry_tests, domains,
  lookup_tests) -> pass unchanged.
- Legacy binaries `song_route_preparation`, `song_source_routes`,
  `song_end_to_end` and `song_checker` -> pass unchanged.

No new test is added. The five resolver tests are the owning evidence.

### Session 253 execution steps (in order)

1. Copy the current `tmp/song-mode-riela/session249-resolution-*` files to
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session252/`, and record
   their sha256 values.
2. Rewrite `tmp/song-mode-riela/session249-resolution-intent.json`.
   - It holds the design sha256, the plan sha256, and the sha256 and full
     text of every session-253 path in the table above. `members.rs` is
     recorded as absent.
   - Re-read and hash-check each file before every edit. On drift, stop and
     reconcile from the current file.
3. Reproduce. Run named-tests command 1 below with its output redirected to
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/reproduce.log`. Record
   the exit status. Exit 100 with five failures is expected.
4. Implement TASK-005, then TASK-006, then TASK-007. After each task, run
   `CARGO_TERM_QUIET=true cargo build` and keep its log under
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/`.
5. Run every verification command below in order, in the foreground.
6. Write the receipt `tmp/song-mode-riela/session249-resolution-receipt.json`.
   It records:
   - the Clippy disposition table (D-, `RES-`, `SW-` and `ST-` rows);
   - `densityIndex`;
   - `jointGeometryFixture`;
   - `seams` (whether `configuration.rs` and `density/index.rs` were
     edited);
   - the cohort count (953);
   - the line counts;
   - the `git diff --quiet` result for the unowned paths;
   - `evidenceFingerprint`, the sha256 of the receipt.

   The fingerprint must differ from `04db7146...`, `4945fec7...`,
   `15c388bd...`, `0493996e...` and
   `3c6c504cf4e66accaf04cfedecab68895eb0e40dc149be59dbd60b203f277db6`.
7. Add one progress-log entry to this plan listing every command, exit status
   and log path. Edit no other plan.

### Session 253 verification (foreground; record the exit status and full log path)

1. Named tests. This command starts the focused log:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib cached_nested_slice_events_resolve_from_issued_transcript issued_joint_geometry_resolves_where_legacy_keeps_its_barrier distinct_equal_handle_invocations_are_all_resolved partitioned_nested_issued_queries_equal_the_full_route_set discarded_augmented_source_origins_are_resolved actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct distinct_sites_sharing_one_execution_are_retained_separately > tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   It must exit 0 with 7 tests passed.
2. Occupancy, legacy fixtures and Route8:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/snapshot::occupancy|routing::prepared/)' >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   It must exit 0.
3. Resolver filter:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/routing::(source|nested)::issued|prepared::tests/)' >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   It must exit 0 with 0 failed.
4. Legacy binaries:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   It must exit 0.
5. `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-resolution-build.log 2>&1`
   must exit 0.
6. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-resolution-clippy.log 2>&1`
   may exit nonzero only for diagnostics that each map to a disposition row.
   In this plan's paths, only `dead_code`/`unused_imports` on listed items are
   allowed.
7. Full suite:
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run > tmp/song-mode-riela/session249-resolution-nextest-full.log 2>&1`
   It must exit 0 and run at least 425 distinct tests.
8. `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-resolution-wasm.log 2>&1`
   must exit 0.
9. `rustfmt --edition 2021 --check src/song/routing/nested/issued.rs src/song/routing/nested/issued/members.rs src/song/snapshot/occupancy/lookup/authority.rs src/song/snapshot/occupancy/lookup.rs src/song/routing/configuration/index/canonical.rs src/song/routing/configuration.rs src/song/routing/source.rs src/song/routing/source/issued.rs src/song/routing/nested.rs src/song/routing/prepared.rs src/song/snapshot/occupancy.rs src/song/snapshot/occupancy/tests.rs src/song/snapshot/occupancy/route_view.rs > tmp/song-mode-riela/session249-resolution-fmt.log 2>&1`
   passes when no `Diff in` line names one of these paths. Add
   `src/song/routing/density/index.rs` to the list if it was edited.
10. `wc -l` on the same paths plus `src/song/routing/density/index.rs`:
    - each file is below 1000 lines;
    - `nested/issued.rs` is at most 949;
    - `source.rs` is at most 993.
11. `git diff --quiet 6543273 -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs`
    must exit 0.
12. `git ls-files -co --exclude-standard -- '*.rs' Cargo.toml Cargo.lock | wc -l`
    prints 953.

### Session 253 done criteria (mechanically checkable)

- [x] `grep -n "fn bind_issued_member\|struct IssuedMemberQuery\|fn selected_policy_in" src/song/snapshot/occupancy/lookup/authority.rs`
  shows all three.
- [x] `grep -c "bind_member" src/song/routing/configuration/index/canonical.rs`
  is 2: the import and the legacy call (formerly line 323). No issued call
  remains.
- [x] `git diff 6543273 -- src/song/snapshot/occupancy/lookup.rs` touches only
  `selected_policy`, whose body is replaced by the delegation.
- [ ] `grep -n "issued Index event has no canonical component" src/song/routing/nested/issued.rs`
  matches an `ok_or_else` or `Err` path, and no `else { continue; }` follows
  `issued_index_configuration`.
- [x] `grep -n "lookup_owner" src/song/routing/nested/issued.rs src/song/routing/nested/issued/members.rs`
  shows the owner-frame filter in the seal loop.
- [x] `grep -n "mod members;" src/song/routing/nested/issued.rs` matches.
- [x] The test hunks of `git diff 1ac457f -- src/song/routing/nested/issued.rs`
  show only the classified TASK-007 changes, and
  `grep -n "sampled context requires joint mapping geometry" src/song/routing/nested/issued.rs`
  matches inside the joint-geometry test.
- [ ] Verification commands 1-5, 7, 8, 11 and 12 exit 0 or print the required
  value. Command 6 is fully dispositioned, and commands 9 and 10 meet their
  rules.
- [x] No new `#[allow` or `#[expect` attribute:
  `git diff 6543273 | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
- [x] The receipt holds `jointGeometryFixture`, `densityIndex`, `seams`,
  cohort 953, and a fingerprint distinct from the five prior values.
- [x] One progress-log entry is added.

### Session 253 progress log

**Status**: Incomplete. The TASK-005 binder and TASK-006 owner-frame/member
path are implemented, and TASK-007 fixture assertions were restored. The
assigned resolver acceptance tests still fail. The owner-frame diagnostic
shows the specified four-field predicate has no match among the fresh seals
for the failing fixtures. The restored equal-handle fixture reaches the
first-structure refusal, and the static discarded-origin fixture does not
retain an augmented origin. These are explicit stop conditions in the
session-253 amendment; no guard or assertion was weakened.

**Changed paths**: `src/song/snapshot/occupancy/lookup/authority.rs`,
`src/song/snapshot/occupancy/lookup.rs`,
`src/song/routing/configuration/index/canonical.rs`,
`src/song/routing/nested/issued.rs`, and new
`src/song/routing/nested/issued/members.rs`. `configuration.rs` and
`density/index.rs` were authorized but unedited. Receipt:
`tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/receipt-final.json`
(fingerprint `245cd7f6e538be8d03cd07ec9c348978fc1e6eb46309d28464b04cae8acd9965`).

**Commands, exits and logs**:

- Named resolver and occupancy tests: exit 100; 7 run, 2 passed, 5 failed;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/final-focused-post-helper.log`.
- Resolver filter: exit 100; 11 of 17 selected tests ran, 6 passed, 5 failed;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/resolver-filter.log`.
- Occupancy/PreparedRoutes filter: exit 100; one failure matches the recorded
  session-252 baseline failure;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/occupancy-route8.log`.
- Legacy binaries (`song_route_preparation`, `song_source_routes`,
  `song_end_to_end`, `song_checker`): exit 0; 105 passed;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/legacy-binaries.log`.
- `CARGO_TERM_QUIET=true cargo build`: exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/build-post-integrity-helper.log`.
- `CARGO_TERM_QUIET=true cargo check`: exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/check-after-modify-retry.log`.
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`: exit 101;
  all 63 diagnostics are recorded by file, line, message and disposition in
  the receipt, including the concurrent `SW-let_and_return`;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/clippy-final.log`.
- Full nextest: exit 100; 1,229 ran, 1,224 passed, 5 failed, 3 skipped;
  failures are concurrent structural-clock tests;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/full-nextest-post-helper.log`.
- WASM build: exit 0; final-source rerun also exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/wasm.log` and
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/wasm-final-current.log`.
- Scoped `rustfmt --edition 2021 --check` on declared resolution paths: exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/fmt-final-current.log`.
- Declared path line limits: exit 0; all below 1,000, `nested/issued.rs` 762,
  `source.rs` 991; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/line-counts.log`.
- Unowned path comparison against `6543273`: exit 0; cohort count: 953;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/cohort-count.log`.

The remaining required focused/full test and strict-Clippy gates do not pass.
Do not mark this plan complete until the owner-frame/fixture mismatch and
current-source verification failures are reconciled and all assigned resolver
tests pass.

## Session 254 amendment (runs SECOND, after SONG-STRUCTURAL-CLOCK is accepted; serial)

The source of truth is the design section "Session 254 resume amendments
(2026-10-04)", subsection "Issued resolution (2a) decisions", together with
`tmp/song-mode-riela/session253-root-cause-diagnosis.md`. This section
supersedes these session 253 items:

- the `owner_matches` predicate (TASK-006 item 2);
- the TASK-007 bullets for `issued_joint_geometry`, `distinct_equal` and
  `discarded_augmented`;
- the matching test expectations.

Everything else from session 249-253 still applies. TASK-005 and the
TASK-006 wiring stay as they are at `a8b8ed2`; do not redo them.

### Intent and context

At `a8b8ed2` five resolver tests fail. The diagnosis gives three causes.

- **(a) Owner predicate.** `members.rs:5-10` compares `root` from the stage
  OUTPUT owner with `track`, `revision` and `placement` from `stage.handle`.
  `stage.handle` is the stage INPUT event (`issued.rs:247-262`), so no seal
  ever matches.
  - Once fixed, `issued_joint_geometry` fails only because it evaluates the
    program twice, so its revisions differ.
  - The nested tests then hit `required issued execution is not retained at
    site` (`authority.rs:505`). The fresh seal's execution is `Rc`-retained
    only by the enclosing record (scope 4), while the inner-site records
    (scopes 2 and 3) hold separate executions.
- **(b)** `distinct_equal` uses `slice p 2` with a structured subject.
  `slice_index_root` (`src/song/routing/index.rs:172-173`) correctly refuses
  it.
- **(c)** The `discarded_augmented` handle predicate cannot be satisfied.
  Every frozen contribution is value-equal to the descriptor origin.

### Non-goals

- Same as session 253: no legacy behavior change, no relaxed first-structure
  rule or legacy barrier, no SM4 dynamic admission, and no edits to 2b, wave-3
  or unowned paths.
- No relaxation of `Rc` identity anywhere. A value, handle or scalar match
  never stands in for `transcript.authentic_retained_invocation`.
- No new public API, no new file and no new test helper file.
- No edits to `src/pattern/eval/song_clock*`, `src/pattern/combinators/*` or
  `geometry_tests/domains.rs`. Those belong to SONG-STRUCTURAL-CLOCK, which is
  already accepted by then.

### Owned paths for session 254

| Path | Kind | Allowed edit |
|---|---|---|
| `src/song/routing/nested/issued/members.rs` | writePath | TASK-008: output-handle owner predicate and its threading |
| `src/song/routing/nested/issued.rs` | writePath | TASK-008: pass `&event.handle` at the call site (line 373). TASK-010: test fixtures. TASK-011: only if the investigation proves a scope defect. |
| `src/song/snapshot/occupancy/lookup/authority.rs` | writePath | TASK-009: the two-fact split in `bind_issued_owner_if_matching` |
| `src/song/routing/prepared.rs` | writePath | Unchanged unless a compile error caused by TASK-009 forces an edit. Record any such edit. |

Every other 2a path from earlier sessions stays declared but should not need
an edit. `src/song/routing/density/index.rs` stays unedited; the diagnosis
shows the static `[0]` fixture passes preparation. Record it as "authorized,
unedited".

### TASK-008: Owner-frame predicate on the stage OUTPUT handle

**Status**: Not started
**Parallelizable**: No
**Deliverables**: `members.rs`, plus one call-site line in `nested/issued.rs`

- Change `owner_matches` to take the stage output handle explicitly. Use the
  handle type of `Stage::handle`:
  `pub(super) fn owner_matches(frame: &CanonicalOwnerFrame, stage: &Stage<'_>, output_handle: &<Stage::handle type>) -> bool`.
  It returns true if and only if all four hold:
  - `frame.root == stage.output_owner.payload().id`;
  - `frame.track == output_handle.track()`;
  - `frame.revision == output_handle.revision()`;
  - `frame.placement == *output_handle.placement()`.
- `stage_owner_frame` gains the same `output_handle` parameter and passes it
  through.
- `issued_index_stage_configuration` gains one parameter,
  `event_handle: &<same type>`, which makes 7 parameters (the limit is 7).
  Inside the `for (stage_index, stage)` loop, compute
  `output_handle = if stage_index == 0 { event_handle } else { stages[stage_index - 1].handle }`.
  Use `.get(stage_index - 1).ok_or_else(|| invalid("issued enclosing stage missing"))`,
  never raw indexing. Use `output_handle` in both `stage_owner_frame` and the
  seal-loop `owner_matches` call.
- At the call site, `nested/issued.rs:373` passes `&event.handle`.
- Pitfalls:
  - Do not drop any of the four fields.
  - Do not compare against `stage.handle`, which is the input.
  - Leave `bind_slice_operands(..., stage.handle, ...)` unchanged. That call
    correctly uses the input handle.
  - Do not touch `parent_use_policy` or `bind_timing_member`.

### TASK-009: Retained execution versus site request (`bind_issued_owner_if_matching`)

**Status**: Not started
**Parallelizable**: No (after TASK-008)
**Deliverable**: `src/song/snapshot/occupancy/lookup/authority.rs`

The rule (design D6): two facts are checked separately, and both must hold.

1. **Site request exists.** Keep the existing first scan unchanged:
   - same per-record charges;
   - the same site, scope, track, issuer, prefix and window filter;
   - `authenticate_authority`;
   - the `original_payload` pointer check.

   Collect the site-matching requests in record order into
   `site_requests: Vec<&CanonicalIndexRequest>`, next to the existing
   `candidates`. If none matched, return `Ok(None)` as today.
2. **Execution authentically retained.** Run the existing identity loop over
   `candidates`. If it sets `found`, behavior is exactly as today, and the
   site record's own request is bound.

   Only if `found` is `None`, run one fallback scan:
   - Inside `with_work` with a fresh `ProjectionBudget` (same pattern as the
     first scan), call `budget.enter(depth)`.
   - Charge `view.records().len() + 1`.
   - For every record in `view.records()`:
     - charge `request.prefix.len() + 1`;
     - call `authenticate_authority(authority, &record.request, depth, &mut budget)?`;
     - for each retained invocation, charge the same placement, step and entry
       sum as the first scan. Keep it only if
       `old == fresh && seed == seed && entry == entry` (the same comparisons
       as the first scan), and count it with `limits.check_events`.
   - Outside `with_work`, for each kept invocation in order, call
     `transcript.authentic_retained_invocation(view.original(), seal, retained, work, depth)?`.
     On the first `true`:
     - apply the same `lookup_depth() > max_depth` `DepthExceeded` check;
     - set `found = Some(site_requests[0])`, the first site-matching request
       in record order;
     - stop.
   - If there is still no `true`, return the existing error
     `required issued execution is not retained at site`.
3. Leave unchanged:
   - the `retained_sources` precharge after binding;
   - the returned `RetainedOwnerAddress { authority, request, invocation: actual }`;
   - `bind_issued_owner`, `bind_issued_index` and every legacy function in
     `lookup.rs`.

Pitfalls:

- Non-site records supply identity evidence only. Never take `request`,
  `prefix`, `window` or `scope` from them.
- Never grant retention by `==` on executions, handles or owner frames alone.
  The only gate is `authentic_retained_invocation`, which uses `Rc::ptr_eq`.
- Do not hold a `RefCell` borrow of `work` across `authentic_retained_invocation`.
- Do not run the fallback when `found` is already set. The current charges
  on passing paths must stay byte-identical.
- The fallback must be charged: the test
  `retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits`
  in `prepared.rs` must keep passing unchanged.
- `authority.rs` is 540 lines. Keep it below 1000. A private helper
  `fn retained_in_any_record(...)` is allowed, with at most 7 parameters.

### TASK-010: Fixture repairs (`nested/issued.rs` tests)

**Status**: Not started
**Parallelizable**: No (verify with TASK-008 and TASK-009)

Diff the tests module against `a8b8ed2` before and after the edits, and
classify every changed line in the receipt.

- **`issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`.**
  - Keep the Euclid program exactly as it is.
  - Build everything from one `PreparedSong`, imitating
    `partitioned_nested_issued_queries_equal_the_full_route_set` (`:732-760`):
    - `let mut song = prepared_song(program)?`;
    - the legacy plan from `prepare_routes(song.snapshot(), ...)`;
    - `legacy_event` from `song.query(window, &policy)?`, first event;
    - `authority = song.issue_retained_route_authority(policy, &mut remaining, 0)?`;
    - `routes = prepare(authority.clone())?`;
    - `work = attached_work(&authority)?`;
    - `batch = song.query_issued_with_work(window, &work, 0)?`;
    - `issued_event = batch.events().first()`.
  - Remove the `capture(program)` call. Keep all four assertions verbatim:
    1. legacy `prepare_routes` is `Ok`;
    2. the legacy `resolve_route` error contains
       `sampled context requires joint mapping geometry`;
    3. `resolve_issued_event(&batch, 0, &work, 0)` is `Ok`;
    4. `legacy_event.handle == issued_event.descriptor().handle`.
  - If ordering matters (for example, the legacy query must run before
    retention), use the order that keeps all four assertions and record it.
- **`distinct_equal_handle_invocations_are_all_resolved`.** Replace
  `slice p 2 [0 nil]` with `slice {beat -> p} 2 [0 nil]` in `inner` and in
  both `outer` stack arms. Make no other change: the distinct-seal `find`, the
  `len() >= 2` assertion and the seal-debit assertion stay. If no event has
  two or more distinct seals, stop and report.
- **`discarded_augmented_source_origins_are_resolved`.** This is a declared
  assertion change (design D10). You may rename the test, for example to
  `slice_timed_contribution_union_is_resolved_per_contribution`. Record both
  names.
  - Keep the program (static `[0]`) byte-identical.
  - Select the first event index where at least 2 of its
    `source_contributions()` carry slice timing. Use the existing
    `carries_slice_timing` expression.
  - Assert that such an event exists, with the message
    `fixture has an equal-handle union with two or more slice-timed contributions`.
  - Read `work.borrow().remaining()` before and after
    `routes.resolve_issued_event(&batch, index, &work, 0)?`.
  - Assert that the debit is at least
    `u32::try_from(event.source_contributions().len())`. Imitate the
    seal-debit lines in `distinct_equal` (`:719-727`).
  - Remove only the `survives_descriptor` / `found_discarded_augmented`
    predicate and its assertion.
  - Add one comment line naming the replacement identity evidence,
    `snapshot::issued::...::fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority`.
    That snapshot test stays unchanged.
  - If no event qualifies, or the debit assertion fails, stop and report. Do
    not lower the bound.
- **`cached_nested` and `partitioned_nested`:** no change.
- No other test line changes.

### TASK-011: Duplicate frozen `inside` investigation

**Status**: Not started
**Parallelizable**: No (after TASK-008 and TASK-009)

In `cached_nested` and `partitioned_nested`, record in the receipt under
`insideDuplicateInvestigation` the following facts for the failing or
resolved stage:

- the selector `site.scope()`;
- the `output_scopes` per stage;
- the `selected.root_part` of each stage;
- the owner frames (root, revision, placement) of the fresh seals and of the
  record that retains the execution.

Gather them with a temporary local `eprintln!` or a debugger run, and remove
any instrumentation before the gates.

Classify the result as one of:

- `separate-placements`: parts 2 and 3 are distinct placements of the reusable
  function. Each owns its own frozen copy. No code change.
- `scope-defect`: `output_scopes` names the wrong part. Fix it in
  `nested/issued.rs` or `members.rs` only, and add the fix to the receipt.

Never merge copies by value.

### Invariants

- Legacy APIs and their outputs are unchanged. Legacy fixtures and the four
  binaries pass with zero edits.
- The only membership and retention grants are allocation identity
  (`Rc::ptr_eq` / `std::ptr::eq`).
- Every contributor authenticates before coalescing. The hard error
  `issued Index event has no canonical component` stays.
- Work comes only from the caller's `SharedIndexWork`.
- Files stay below 1000 lines. `nested/issued.rs` must be at most 990 (the 2c
  witness budget is gone). `source.rs` must be at most 993.
- `git diff --quiet <2c join commit> -- <three unowned paths>` exits 0.
- `.agents/settings.local.json` is untouched.

### Session 254 tests (input -> expected outcome)

- `cached_nested_slice_events_resolve_from_issued_transcript` -> passes.
- `partitioned_nested_issued_queries_equal_the_full_route_set` -> passes. The
  full and partitioned route sets are equal.
- `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` (one
  `PreparedSong`) -> all four assertions pass.
- `distinct_equal_handle_invocations_are_all_resolved` (`slice {beat -> p}`)
  -> passes, with at least 2 distinct seals and the seal debit met.
- `discarded_augmented` (renamed or not) -> an event with at least 2
  slice-timed contributions resolves, and its debit is at least the
  contribution count.
- `retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits`
  -> unchanged and passing. The foreign transcript still fails with a debit.
- `actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct`,
  `distinct_sites_sharing_one_execution_are_retained_separately`, the
  `snapshot::occupancy` legacy groups and
  `fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority`
  -> unchanged and passing.

### Session 254 execution steps (in order)

1. Copy `tmp/song-mode-riela/session249-resolution-*` to
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session253/` with sha256
   values.
2. Rewrite the intent JSON with the design and plan sha256 values and the
   sha256 and full text of `members.rs`, `nested/issued.rs` and
   `lookup/authority.rs`. Hash-check before every edit.
3. Reproduce with named-tests command 1 into
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/session254/reproduce.log`. The
   expected result is exit 100 with five failures.
4. Do TASK-008, then TASK-009, then TASK-010, then TASK-011. Run
   `CARGO_TERM_QUIET=true cargo build` after each task, with logs in
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/session254/`.
5. Run the verification below.
6. Write the receipt with:
   - the clippy disposition;
   - `densityIndex: "authorized, unedited"`;
   - `jointGeometryFixture: "euclid"`;
   - `insideDuplicateInvestigation`;
   - the TASK-010 line classification and the test names;
   - cohort 953;
   - the line counts;
   - the unowned-path result;
   - `evidenceFingerprint`. It must differ from `04db7146...`,
     `4945fec7...`, `15c388bd...`, `0493996e...`, `3c6c504c...` and
     `245cd7f6e538be8d03cd07ec9c348978fc1e6eb46309d28464b04cae8acd9965`.
7. Add one progress-log entry.

### Session 254 verification (foreground; record the exit status and full log path)

Commands 1-12 of "Session 253 verification" apply, with these changes:

- Command 1 replaces `discarded_augmented_source_origins_are_resolved` with
  the renamed test if it was renamed, and adds
  `retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits`.
  It must exit 0 with 8 tests passed.
- Command 6 (clippy): no diagnostic may name a 2a path except
  `dead_code`/`unused_imports` on listed `RES-` items. `SW-let_and_return`
  is still expected.
- Command 7 (full suite) adds `--no-fail-fast`. It must exit 0 with at least
  425 distinct tests passed and every test run.
- Command 10: `nested/issued.rs` is at most 990 lines.
- Command 11 uses the 2c join commit as the base instead of `6543273`.

### Session 254 done criteria (mechanically checkable)

- [ ] `grep -n "stage.handle" src/song/routing/nested/issued/members.rs`
  shows no `owner_matches` comparison. `grep -n "output_handle" src/song/routing/nested/issued/members.rs`
  shows the predicate and the per-stage selection.
- [ ] `grep -n "event.handle\|&event.handle" src/song/routing/nested/issued.rs`
  shows the argument at the `issued_index_stage_configuration` call.
- [ ] `grep -c "authentic_retained_invocation" src/song/snapshot/occupancy/lookup/authority.rs`
  is at least 2. `git diff <2c join> -- src/song/snapshot/occupancy/lookup/authority.rs`
  touches only `bind_issued_owner_if_matching` and an optional private helper.
- [ ] `grep -n "slice p 2" src/song/routing/nested/issued.rs` prints nothing.
- [ ] `grep -n "capture(program)" src/song/routing/nested/issued.rs` does not
  match inside the joint-geometry test.
- [ ] The receipt contains the TASK-010 classification and
  `insideDuplicateInvestigation`.
- [ ] Verification commands pass under the rules above, with no new
  `allow`/`expect`.
- [ ] The progress-log entry is added.

## Session 255 amendment (runs SECOND, after SONG-STRUCTURAL-CLOCK is joined; serial)

The source of truth is the design section "Session 255 resume amendments
(2026-10-04)": "Implementer authority" and "Remaining waves" > 2a. The session
254 decisions and TASK-008 to TASK-011 stand unchanged. TASK-008 to TASK-011
were not started in session 254, because 2c blocked first. This amendment
changes only three things: the stop rules, the base commit and the evidence
locations. The diff base is the 2c join commit, written `<2c-join>`.

### Intent and context

- At `c7083fb`, the five resolver tests fail for the three causes listed in
  the session 254 intent: the owner predicate is compared against the input
  handle, the joint-geometry fixture evaluates the program twice, and site
  request and retained execution are conflated.
- TASK-008 to TASK-011 repair them. The issued path must then:
  - authenticate every contributor, and each source contribution's augmented
    origin and member slots, before coalescing;
  - use fresh rebound clocks, the original source START, full configuration
    groups and connected uncut wholes before clipping;
  - resolve where legacy keeps its `NeedsJointGeometry` barrier
    (`sampled context requires joint mapping geometry`).
- Legacy `resolve_route`, `bind_member` and `prepare_routes` stay unchanged.

### Implementer authority (replaces the stop clauses listed below)

- Diagnose each failing gate and fix it inside this plan's writePaths and
  sharedPaths. Rerun the named-tests command until all of it passes, then run
  every gate. Record each fix in the receipt under `fixes[]` with: the test,
  the exact message, the root cause, the edited paths, and the class
  (`production-defect` or `own-test-corrected`, naming the design rule).
- Stop only for:
  1. a fix that needs a path outside the manifest below;
  2. a change to an assertion of a test that exists at `37ea3e8` and is not
     covered by a design decision (the `discarded_augmented` replacement is
     covered, by session 254 D10);
  3. relaxing an identity or security check: `Rc::ptr_eq` /
     `authentic_retained_invocation`, `authenticate_authority`, the
     owner/seed/entry/site comparisons, the hard error `issued Index event
     has no canonical component`, any production depth check, or the
     first-structure rule in `slice_index_root`.
- **Earlier clauses that become "diagnose and fix":**
  - TASK-010 `distinct_equal`, "If no event has two or more distinct seals,
    stop and report". The test is an own test (added at `1ac457f`).
    - If coalescing or binding in a 2a path loses a seal, that is a
      production defect: fix it.
    - If the fixture itself emits only one seal, adjust the fixture inside
      the accepted shape until some event has at least two distinct seals.
      The accepted shape is: Slice subject lambda `{beat -> p}`, a static
      Index list, the first-structure rule, and an equal-handle `stack`.
      Every assertion stays, including `len() >= 2` and the seal debit.
  - TASK-010 `discarded_augmented`, "If no event qualifies, or the debit
    assertion fails, stop and report".
    - A debit below the contribution count means per-contribution
      authentication or charging is missing in a 2a path. Fix the production
      code; never lower the bound.
    - If no event qualifies, adjust the own fixture, keeping a static Index
      list, until an equal-handle union with at least two slice-timed
      contributions exists.
  - TASK-010 joint-geometry: use the call order that keeps all four
    assertions (already allowed).
- **Clauses kept as stops:**
  - the session 252 legacy-invariance rule: any changed existing
    `snapshot::occupancy` assertion stops (stop condition 2);
  - the session 253 rule: an owner-matching bound seal with no canonical
    component never gets `continue` restored (stop condition 3);
  - a TASK-011 `scope-defect` fix only in `nested/issued.rs` or `members.rs`.

### Owned paths for session 255

```json
{
  "planId": "SONG-ISSUED-RESOLUTION",
  "planPath": "impl-plans/active/song-mode-issued-route-resolution.md",
  "dependsOn": ["SONG-ROUTE8", "SONG-STRUCTURAL-CLOCK"],
  "writePaths": [
    "src/song/routing/nested/issued/members.rs",
    "src/song/routing/nested/issued.rs",
    "src/song/snapshot/occupancy/lookup/authority.rs",
    "src/song/routing/prepared.rs",
    "src/song/routing/source.rs",
    "src/song/routing/source/issued.rs",
    "src/song/routing/nested.rs",
    "src/song/routing/configuration.rs",
    "src/song/routing/configuration/index/canonical.rs",
    "src/song/routing/density/index.rs",
    "src/song/snapshot/occupancy.rs",
    "src/song/snapshot/occupancy/tests.rs",
    "impl-plans/active/song-mode-issued-route-resolution.md",
    "tmp/song-mode-riela/session249-resolution-intent.json",
    "tmp/song-mode-riela/session249-resolution-receipt.json",
    "tmp/song-mode-riela/session249-resolution-build.log",
    "tmp/song-mode-riela/session249-resolution-clippy.log",
    "tmp/song-mode-riela/session249-resolution-nextest-focused.log",
    "tmp/song-mode-riela/session249-resolution-nextest-full.log",
    "tmp/song-mode-riela/session249-resolution-wasm.log",
    "tmp/song-mode-riela/session249-resolution-fmt.log"
  ],
  "sharedPaths": [
    "src/song/snapshot/occupancy/lookup.rs",
    "src/song/snapshot/occupancy/route_view.rs"
  ]
}
```

- The expected edits are in `members.rs`, `nested/issued.rs` (one call-site
  argument and tests) and `lookup/authority.rs`, per TASK-008 to TASK-010.
  The other writePaths are declared so that a proven defect does not end the
  run. Each edit to them is recorded in `fixes[]`.
- `src/song/routing/source.rs` is 991 lines and may reach at most 993. Any
  larger issued-path growth goes into `src/song/routing/source/issued.rs`.
- `src/song/routing/density/index.rs` is expected to stay unedited. The
  receipt then records `densityIndex: "authorized, unedited"`.
- `lookup.rs` and `route_view.rs` are sharedPaths, editable only under the
  session 252/253 limits (alias skip, alias marker, `selected_policy`
  delegation). Legacy outputs stay byte-identical.
- Never run rustfmt in write mode on `occupancy.rs` or `lookup.rs`: they have
  unowned children. Hand-format them.
- Frozen: `src/pattern/eval/song_clock*`, `src/pattern/combinators/*`,
  `geometry_tests/domains.rs`, `src/song/routing/index.rs`, every 2b and
  wave-3 path, the three unowned paths, and `.agents/settings.local.json`.

### Session 255 execution steps (in order)

1. Copy `tmp/song-mode-riela/session249-resolution-*` to
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session254/` and record
   their sha256 values. Scratch logs go under
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/session255/`.
2. Rewrite the intent JSON with the design and plan sha256 values and the
   sha256 and full text of every file you will edit. Hash-check before every
   edit. On drift, reconcile from the current file.
3. Reproduce with command 1 below into `.../session255/reproduce.log`. The
   expected result is exit 100 with the five resolver tests failing.
4. Do TASK-008, TASK-009, TASK-010 and TASK-011 from the session 254
   amendment, in that order. Run `CARGO_TERM_QUIET=true cargo build` after
   each into `.../session255/build-<task>.log`. Fix failures under the
   authority rule.
5. Run verification 1-12.
6. Write the receipt `tmp/song-mode-riela/session249-resolution-receipt.json`
   with:
   - `fixes[]`;
   - the TASK-010 line classification against `<2c-join>`;
   - `insideDuplicateInvestigation`;
   - `jointGeometryFixture: "euclid"`;
   - `densityIndex`;
   - the clippy disposition;
   - the cohort (953);
   - the line counts;
   - the unowned-path result;
   - `evidenceFingerprint`. It must differ from every hash in
     `tmp/song-s249/SONG-ISSUED-RESOLUTION/` and from the values listed in
     session 254 step 6.
7. Add one progress-log entry.

### Session 255 tests (input -> expected outcome)

These are the session 254 tests, unchanged:

- `cached_nested_slice_events_resolve_from_issued_transcript` -> passes.
- `partitioned_nested_issued_queries_equal_the_full_route_set` -> passes.
- `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` -> all four
  assertions pass from one `PreparedSong`.
- `distinct_equal_handle_invocations_are_all_resolved` -> at least 2 distinct
  seals, and the seal debit is met.
- `discarded_augmented_source_origins_are_resolved` (or its renamed form) ->
  an event with at least 2 slice-timed contributions resolves with a debit of
  at least the contribution count.
- `retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits`,
  `actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct`,
  `distinct_sites_sharing_one_execution_are_retained_separately` and
  `fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority`
  -> unchanged and passing.
- The 2c tests (`song_clock::structural_tests`, `song_clock::tests`,
  `geometry_tests::domains`) -> still passing.

### Session 255 verification (foreground; record the exit status and full log path)

1. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib cached_nested_slice_events_resolve_from_issued_transcript issued_joint_geometry_resolves_where_legacy_keeps_its_barrier distinct_equal_handle_invocations_are_all_resolved partitioned_nested_issued_queries_equal_the_full_route_set <discarded_augmented test name> actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct distinct_sites_sharing_one_execution_are_retained_separately retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits > tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   must exit 0 with 8 tests passed.
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/snapshot::occupancy|routing::prepared/)' >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   must exit 0.
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/routing::(source|nested)::issued|prepared::tests/)' >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   must exit 0.
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --test song_route_preparation --test song_source_routes --test song_end_to_end --test song_checker >> tmp/song-mode-riela/session249-resolution-nextest-focused.log 2>&1`
   must exit 0.
5. `CARGO_TERM_QUIET=true cargo build > tmp/song-mode-riela/session249-resolution-build.log 2>&1`
   must exit 0.
6. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/song-mode-riela/session249-resolution-clippy.log 2>&1`.
   In 2a paths, only `dead_code`/`unused_imports` on listed `RES-` items whose
   consumer is wave 3 are allowed. `SW-let_and_return` is expected, and every
   diagnostic is mapped.
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-mode-riela/session249-resolution-nextest-full.log 2>&1`
   must exit 0, with every test run and at least 425 distinct tests passed.
   No failure is allowed.
8. `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/song-mode-riela/session249-resolution-wasm.log 2>&1`
   must exit 0.
9. The session 253 command 9 `rustfmt --edition 2021 --check` list (13
   paths), plus `src/song/routing/density/index.rs` if it was edited. It
   passes when no `Diff in` hunk covers a line changed since `<2c-join>`.
10. `wc -l` on the same paths:
    - each is below 1000;
    - `nested/issued.rs` is at most 990;
    - `source.rs` is at most 993.
11. `git diff --quiet <2c-join> -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs src/song/routing/index.rs src/pattern/eval/song_clock.rs src/pattern/eval/song_clock/tests.rs src/pattern/eval/song_clock/structural_tests.rs`
    must exit 0.
12. `git ls-files -co --exclude-standard -- '*.rs' Cargo.toml Cargo.lock | wc -l`
    prints 953.

### Session 255 done criteria (mechanically checkable)

- [ ] Every session 254 done criterion holds, with `<2c-join>` as the base.
- [ ] Verification 1-5, 7, 8, 11 and 12 exit 0 (or print 953). Verification 6
  is fully dispositioned. Verification 9 and 10 meet their rules.
- [ ] `grep -n "issued Index event has no canonical component" src/song/routing/nested/issued.rs src/song/routing/nested/issued/members.rs`
  matches a hard-error path.
- [ ] `git diff <2c-join> | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
- [ ] The receipt has `fixes[]` and a new fingerprint, and one progress-log
  entry is added.

## Session 256 amendment (runs SECOND, after SONG-STRUCTURAL-CLOCK is accepted; serial)

The source of truth is the design section "Session 256 resume amendments
(2026-10-04)" > "2a: source fixture correction". Everything in the session 254
and 255 amendments stays in force: TASK-008 to TASK-011, the owned paths, the
implementer authority, the frozen paths and the verification commands. This
amendment adds TASK-012, adds one test to verification 1, and moves the
evidence locations. The diff base is still `<2c-join>`, the commit that
records 2c acceptance on `wf/route-authority`.

### Intent and context

- `song::routing::source::issued::tests::genuine_source_contributions_reject_a_foreign_transcript`
  fails with `issued route source: Slice fixture produced no source
  contribution`. It fails identically at `1ac457f` and `c7083fb`.
- Root cause (`tmp/song-mode-riela/session255-source-fixture-diagnosis.md`):
  the `SLICE` constant (`src/song/routing/source/issued.rs:241`) is a fixture
  defect. Its `indexed` subject never uses the selected source `p`, so `base`
  is never queried, and every event has empty `source_contributions()`
  (`src/song/snapshot/issued.rs:28`, `:175-191`, `:481-484`). The guard at
  `source/issued.rs:312-313` is correct.
- `source/issued.rs` is absent at `37ea3e8` and first appears in `1ac457f`.
  This test is therefore an own test, and the fix is class
  `own-test-corrected`, not a baseline-assertion change.
- Combined with TASK-008 to TASK-011, 2a must end with full nextest at zero
  failures.

### Non-goals

- No production change for this test. The guard, `authenticate_contributions`
  and the issued-path authentication of every contributor and of each source
  contribution's augmented origin and member slots stay exactly as they are.
- Do not touch `PLAIN`, `genuine_plain_event_matches_legacy_route`,
  `exact_work_succeeds_and_one_less_returns_no_route` or any assertion in the
  test module.
- Do not use `[0 nil]`. The design pins `[cut nil]`.

### TASK-012: SLICE fixture correction in `src/song/routing/source/issued.rs`

**Status**: Not Started
**Parallelizable**: No. It runs in the same serial 2a sub-wave and may go
before or after TASK-008 to TASK-011. Running it first gives early evidence.
**Deliverable**: one changed constant in the `#[cfg(test)]` module of
`src/song/routing/source/issued.rs`.

- In the `SLICE` constant, change exactly two lines of the embedded program:
  - `\tslice {beat -> s :analog > chord [:c :five]} 2 [cut nil]` becomes
    `\tslice {beat -> p} 2 [cut nil]`;
  - `let base {part [drums: {s :analog}] duration: 2}` becomes
    `let base {part [drums: {s :analog > chord [:c :five]}] duration: 2}`.
- `fn cut beat:` / `\tfirst [0]`, `fn indexed p:`, the `selected`
  `transform-instrument` line and the `song selected tail-seconds: 0 >
  play-song` line stay byte-identical. The constant stays a single string
  literal with `\n`/`\t` escapes, like `PLAIN` beside it.
- This is the nested resolver fixture shape (subject lambda `{beat -> p}`),
  which satisfies the first-structure rule in `slice_index_root`
  (`src/song/routing/index.rs`). Do not edit `index.rs`.
- Before the edit, re-read the file and hash-check it against the intent JSON.
  Edit by hand. Never run rustfmt in write mode on `source/issued.rs` or
  `source.rs`.
- Pitfalls:
  - Do not weaken the test to tolerate empty contributions, and do not remove
    the `if event.source_contributions().is_empty()` guard.
  - Do not pick a different event than `batch.events().first()`. If the first
    event has no contributions after the fix, diagnose. A production defect
    goes in a 2a path. A fixture defect stays within the accepted shape
    (subject `{beat -> p}`, static list `[cut nil]`, chord in `base`). Record
    either in `fixes[]`.
  - The foreign batch comes from a second `capture(SLICE)`. Keep that call; it
    makes a genuinely foreign transcript with the same program text.

Tests (input -> expected outcome):

- `genuine_source_contributions_reject_a_foreign_transcript` with the new
  SLICE -> the first event has at least one source contribution, the genuine
  transcript authenticates, and the foreign transcript returns `Err`.
- `genuine_plain_event_matches_legacy_route` -> unchanged and passing.
- `exact_work_succeeds_and_one_less_returns_no_route` -> unchanged and
  passing.

### Session 256 changes to the session 255 execution steps and verification

- Step 1: copy `tmp/song-mode-riela/session249-resolution-*` to
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session255/` with
  `sha256.txt`. Scratch logs go to
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session256/`. Never delete earlier
  attempt directories.
- Step 3 (reproduce): the expected result is exit 100 with the five resolver
  tests failing. Also run
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --lib --no-fail-fast -E 'test(/routing::source::issued/)' > tmp/song-s249/SONG-ISSUED-RESOLUTION/session256/reproduce-source.log 2>&1`.
  The expected result is exit 100 with exactly
  `genuine_source_contributions_reject_a_foreign_transcript` failing.
- Step 4: do TASK-012 as well as TASK-008 to TASK-011.
- Verification 1: append `genuine_source_contributions_reject_a_foreign_transcript`
  to the test-name list. It must exit 0 with 9 tests passed.
- Verification 3 (`-E 'test(/routing::(source|nested)::issued|prepared::tests/)'`):
  must exit 0, and all three `routing::source::issued::tests` must have run
  and passed.
- Verification 7 (full, `--no-fail-fast`): exit 0, with zero failures and a
  `Summary` line. No allowed-failure list remains.
- The receipt also records:
  - a `fixes[]` entry for TASK-012 with class `own-test-corrected` and design
    rule "Session 256 > 2a: source fixture correction";
  - `sourceFixtureDiff`, the output of
    `git diff <2c-join> -- src/song/routing/source/issued.rs`.

  The fingerprint must also differ from every hash in
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session255/`.

### Session 256 done criteria (mechanically checkable)

- [ ] Every session 255 done criterion holds.
- [ ] `grep -c 'slice {beat -> p} 2 \[cut nil\]' src/song/routing/source/issued.rs`
  prints at least 1, and
  `grep -c 'slice {beat -> s :analog > chord' src/song/routing/source/issued.rs`
  prints 0.
- [ ] `grep -n 'Slice fixture produced no source contribution' src/song/routing/source/issued.rs`
  still matches (the guard is kept).
- [ ] `grep -c 'foreign_batch.transcript(), &work, 0).is_err()' src/song/routing/source/issued.rs`
  prints 1.
- [ ] Verification 1 passes 9 tests. Verification 7 exits 0 with zero failures
  and a `Summary` line.
- [ ] `wc -l src/song/routing/source.rs` prints at most 993, and every 2a path
  is below 1000.
- [ ] The receipt has the TASK-012 `fixes[]` entry and a new fingerprint, and
  one progress-log entry is added.

## Session 257 amendment (runs SECOND, after SONG-STRUCTURAL-CLOCK is accepted; serial)

The source of truth is the design section "Session 257 resume amendments
(2026-10-04)" > "Later waves". The session 253-256 amendments stay in force
unchanged: TASK-008 to TASK-012, the owned paths and sharedPaths, the
implementer authority, the frozen paths, the verification commands and the
done criteria. This includes the TASK-012 SLICE correction
(`slice {beat -> p} 2 [cut nil]`, with the chord moved into
`let base {part [drums: {s :analog > chord [:c :five]}] duration: 2}`) and the
unchanged authentication assertions. The diff base is still `<2c-join>`, the
commit that records 2c acceptance. In session 257 that commit includes the 2c
harness fix in `tests/song_export.rs` and `tests/song_cli.rs`.

Only these items change:

- **Evidence locations.** Step 1 copies every existing
  `tmp/song-mode-riela/session249-resolution-*` file to
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session256/` and writes
  `sha256.txt` there. Do not create, delete or overwrite earlier attempt
  directories. Scratch logs, including `reproduce-source.log`, go to
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session257/`. The fingerprint must
  differ from every hash under `tmp/song-s249/SONG-ISSUED-RESOLUTION/`.
- **Reproduce expectation.** On `<2c-join>`, the full-suite failures are a
  subset of the six session 256 IDs, and no `song_export` or `song_cli` test
  fails. A `song_export` or `song_cli` failure here is stop condition 1. Its
  owner is SONG-STRUCTURAL-CLOCK, and 2a must not edit those files.
- **Not 2a paths.** `tests/song_export.rs`, `tests/song_cli.rs` and every 2c
  path are outside this plan.

### Session 257 done criteria

- [ ] Every session 256 done criterion holds. Verification 1 passes 9 tests,
  and verification 7 (full, `--no-fail-fast`) exits 0 with zero failures and
  a `Summary` line.
- [ ] `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session256/sha256.txt`
  exists, and the receipt fingerprint is new.
- [ ] One progress-log entry is added.

## Session 258 amendment (runs FIRST, on base e71d726; serial, concurrency 1)

The source of truth is the design section "Session 258 resume amendments
(2026-10-04)": "Serial order and dependency edge" and "2a: verified current
state and fix map". This section wins over every earlier amendment of this
plan where they conflict. The session 253 to 257 amendments otherwise stay in
force unchanged: TASK-008 to TASK-012, the owned paths and sharedPaths of the
session 255 manifest, the implementer authority, the frozen paths, the tests
and the done criteria.

### Intent and context

- The operator removed the `SONG-ISSUED-RESOLUTION` dependency on
  `SONG-STRUCTURAL-CLOCK`. `dependsOn` is now `["SONG-ROUTE8"]`. 2a runs
  first, because the workflow progress gate rejects any non-zero full-suite
  exit, and all six remaining failures are owned by 2a.
- The 2c code is already committed at `e71d726`, including the Euclid
  instrumentation used by `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`.
  2a builds on it and never edits it.
- **Base.** Every `<2c-join>` in the session 255 to 257 amendments of this
  plan now means `e71d726`. That covers verification 9 and 11, the TASK-010
  line classification and `sourceFixtureDiff`.
- **State verified at `e71d726`.** The session 253 code is present:
  `members.rs`, `mod members;` at `nested/issued.rs:2`, the hard error at
  `nested/issued.rs:476`, and two `bind_member` matches in `canonical.rs`.
  TASK-008 to TASK-012 are NOT applied yet:
  - `members.rs:5-10` `owner_matches` still reads `track`, `revision` and
    `placement` from `stage.handle` (the stage input);
  - `authority.rs` (540 lines) calls `authentic_retained_invocation` once (line
    493), so there is no fallback scan yet;
  - `nested/issued.rs:699` still uses `slice p 2 [0 nil]`;
  - `nested/issued.rs:630-655` still uses the handle-based
    `found_discarded_augmented` predicate;
  - `source/issued.rs:241` `SLICE` still has the old subject.
- **Failure-to-task map.** The latest full run is
  `tmp/song-mode-riela/session249-structural-nextest-full.log` (2793 run,
  2787 passed, 6 failed, 3 skipped):

  | Exact message | Tests | Tasks |
  |---|---|---|
  | `song route: issued Index timing has no matching fresh invocation` | `cached_nested_slice_events_resolve_from_issued_transcript`, `partitioned_nested_issued_queries_equal_the_full_route_set`, `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` | TASK-008, then TASK-009; TASK-010 for the joint-geometry fixture; TASK-011 investigation |
  | `song route: Index address contradicts first-structure rule` | `distinct_equal_handle_invocations_are_all_resolved` | TASK-010 (`slice {beat -> p} 2 [0 nil]`) |
  | panic at `src/song/routing/nested/issued.rs:654` | `discarded_augmented_source_origins_are_resolved` | TASK-010 (declared replacement evidence) |
  | `issued route source: Slice fixture produced no source contribution` | `genuine_source_contributions_reject_a_foreign_transcript` | TASK-012 |

  Once TASK-008 lands, the three nested tests are expected to move on to
  `required issued execution is not retained at site`
  (`authority.rs:505`). TASK-009 fixes that. This intermediate message is
  expected and is not a stop condition.

### Non-goals

- No edit to any 2c path: `src/pattern/eval/song_clock/structural_tests.rs`,
  `src/pattern/eval/song_clock/tests.rs`,
  `src/song/snapshot/occupancy/geometry_tests/domains.rs`,
  `src/pattern/combinators/region.rs`, `src/pattern/combinators/music.rs`,
  `tests/song_export.rs` and `tests/song_cli.rs`. Do not fix the two
  `structural_tests.rs` rustfmt hunks; 2c fixes them after 2a. A fix that needs
  a 2c path is stop condition 1.
- No edit to `src/song/snapshot/issued.rs` (its `let_and_return` belongs to
  2b), `src/pattern/eval/song_replay.rs`, `src/sched/**`, `src/host/**` or
  `src/song/routing/index.rs`.
- No `--retries` and no rerunning the full suite until it happens to pass.
- No relaxation of `Rc` identity, `authenticate_authority`, the
  owner/seed/entry/site comparisons, the hard `issued Index event has no
  canonical component` error, any production depth check or the
  first-structure rule.

### Owned paths for session 258

These are the session 255 manifest paths, unchanged, with the new
`dependsOn`:

- `dependsOn`: `["SONG-ROUTE8"]`.
- writePaths: `src/song/routing/nested/issued/members.rs`,
  `src/song/routing/nested/issued.rs`,
  `src/song/snapshot/occupancy/lookup/authority.rs`,
  `src/song/routing/prepared.rs`, `src/song/routing/source.rs`,
  `src/song/routing/source/issued.rs`, `src/song/routing/nested.rs`,
  `src/song/routing/configuration.rs`,
  `src/song/routing/configuration/index/canonical.rs`,
  `src/song/routing/density/index.rs`, `src/song/snapshot/occupancy.rs`,
  `src/song/snapshot/occupancy/tests.rs`, this plan file, and the nine
  `tmp/song-mode-riela/session249-resolution-*` evidence files.
- sharedPaths: `src/song/snapshot/occupancy/lookup.rs` and
  `src/song/snapshot/occupancy/route_view.rs`, under the session 252/253
  limits.

Expected edits are in `members.rs`, `nested/issued.rs` (one call-site argument
plus test fixtures), `lookup/authority.rs` and the `source/issued.rs` test
module. Any other edit must be recorded in `fixes[]`.

### Session 258 execution steps (in order)

1. Copy every `tmp/song-mode-riela/session249-resolution-*` file to
   `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session257/` and write their
   sha256 values to `sha256.txt` in that directory. Never delete or overwrite
   `session251/`, `session252/`, `session253/` or any other existing
   directory. Scratch logs go to `tmp/song-s249/SONG-ISSUED-RESOLUTION/session258/`.
2. Record `git rev-parse HEAD` and `git status --porcelain=v1` in
   `.../session258/tree.log`. The status must be clean apart from `tmp/` and
   the excluded `.agents/settings.local.json`.
3. Rewrite `tmp/song-mode-riela/session249-resolution-intent.json` with:
   - the sha256 of the design doc and of this plan;
   - the sha256 and full text of every file to be edited.

   Hash-check each file again right before each edit. On drift, re-read the
   file, reconcile against the current content and record it.
4. Reproduce. Run session 255 command 1, with
   `genuine_source_contributions_reject_a_foreign_transcript` appended, into
   `.../session258/reproduce.log`. It must exit 100 with exactly the six tests
   above failing, with the exact messages in the table. If the failure set or
   the messages differ, record the difference in the receipt under
   `reproduceDelta` and diagnose before editing.
5. Do TASK-012 first (early evidence), then TASK-008, TASK-009, TASK-010 and
   TASK-011, exactly as specified in the session 254 and 256 amendments. After
   each task, run `CARGO_TERM_QUIET=true cargo build > tmp/song-s249/SONG-ISSUED-RESOLUTION/session258/build-<task>.log 2>&1`
   and session 255 command 1 into `.../session258/named-<task>.log`. Fix any
   remaining failure under the session 255 implementer authority.
6. Run session 255 verification 1 to 12, with the session 256 changes and the
   changes below. Every command runs in the foreground, and its exit status and
   full log path are recorded.
7. Write `tmp/song-mode-riela/session249-resolution-receipt.json` with every
   field required by sessions 254 to 256, plus:
   - `session: 258` and `baseCommit: "e71d726a027bfdee20675f2b3cc3b4f22a018008"`;
   - `reviewDiffRange`: `git diff 1ac457f <final tree> -- <2a writePaths and edited sharedPaths>`.
     The reviews cover the session 251 to 253 2a work as well as this
     session's edits;
   - `fullSuite` with the `Summary` counts and `failures: []`;
   - `evidenceFingerprint`: the sha256 of the receipt with this field empty.
     It must differ from every hash under `tmp/song-s249/SONG-ISSUED-RESOLUTION/`
     and from the values listed in session 254 step 6.
8. Set TASK-008 to TASK-012 to Complete only if every done criterion passes.
   Add one progress-log entry with the commands, exit codes and log paths.

### Session 258 changes to verification

- Verification 1 (named tests, including the session 256 addition) must exit
  0 with 9 tests passed.
- Verification 7: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast > tmp/song-mode-riela/session249-resolution-nextest-full.log 2>&1`
  must exit 0. The log must have a `Summary` line showing at least 2793 run,
  0 failed, and every test run. Run
  `grep -cE '^\s+FAIL' tmp/song-mode-riela/session249-resolution-nextest-full.log`;
  it must print 0. The suite takes about 13 minutes. Run it in the foreground
  and poll it until it exits. A log without a `Summary` line is a failure.
- Verification 9: rustfmt `--check` passes when no `Diff in` hunk covers a
  line changed since `e71d726`.
- Verification 11 becomes
  `git diff --quiet e71d726 -- src/song/snapshot/resources.rs src/song/snapshot/reservations_tests.rs src/sched/runtime/song/clock_tests.rs src/song/routing/index.rs src/pattern/eval/song_clock.rs src/pattern/eval/song_clock/tests.rs src/pattern/eval/song_clock/structural_tests.rs src/song/snapshot/occupancy/geometry_tests/domains.rs src/pattern/combinators/region.rs src/pattern/combinators/music.rs tests/song_export.rs tests/song_cli.rs src/song/snapshot/issued.rs src/pattern/eval/song_replay.rs`
  and must exit 0.
- Verification 12 prints 953.
- S258-V1 (no new suppression):
  `git diff e71d726 | grep -E '^\+.*#\[(allow|expect)'` prints nothing.
- S258-V2 (line caps): `wc -l src/song/routing/source.rs src/song/routing/nested/issued.rs src/song/snapshot/occupancy/lookup/authority.rs src/song/routing/nested/issued/members.rs`
  shows `source.rs` at most 993, `nested/issued.rs` at most 990 and every file
  below 1000.

### Session 258 tests (input -> expected outcome)

The session 254 and 256 tests stand unchanged:

- `cached_nested_slice_events_resolve_from_issued_transcript` -> passes.
- `partitioned_nested_issued_queries_equal_the_full_route_set` -> passes, and
  the full and partitioned route sets are equal.
- `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` -> one
  `PreparedSong` and all four assertions pass. Legacy `resolve_route` still
  fails with `sampled context requires joint mapping geometry`.
- `distinct_equal_handle_invocations_are_all_resolved` -> at least 2 distinct
  seals, and the seal debit is met.
- `discarded_augmented_source_origins_are_resolved` (or its renamed form) ->
  an event with at least 2 slice-timed contributions resolves, with a debit of
  at least the contribution count.
- `genuine_source_contributions_reject_a_foreign_transcript` -> the first
  event has a contribution, the genuine transcript authenticates, and the
  foreign transcript errors.
- `retained_issued_owner_bridge_preserves_success_and_foreign_failure_debits`,
  `actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct`,
  `distinct_sites_sharing_one_execution_are_retained_separately`,
  `fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority`,
  the `snapshot::occupancy` legacy groups, the 2c tests and the four legacy
  binaries -> unchanged and passing.

### Session 258 done criteria (mechanically checkable)

- [ ] Every session 254, 255 and 256 done criterion holds, with `e71d726`
  replacing `<2c-join>`.
- [ ] `.../session258/reproduce.log` exists and shows the six failures (or
  `reproduceDelta` is recorded).
- [ ] Verification 1 passes 9 tests. Verification 7 exits 0 with a `Summary`
  line and 0 failures.
- [ ] Verification 11 exits 0, S258-V1 prints nothing, and S258-V2 meets the
  caps. Verification 12 prints 953.
- [ ] `tmp/song-s249/SONG-ISSUED-RESOLUTION/attempt-session257/sha256.txt`
  exists. The receipt has `session: 258`, `baseCommit`, `reviewDiffRange`,
  `fixes[]` and a new fingerprint.
- [ ] One progress-log entry is added. Test-integrity, adversarial and
  integration review follow; the workflow owns them.

### Session 259 — Resolver repair and final-source verification

Continued the assigned resolver work on base `e71d726` with the accepted
`SONG-ROUTE8` dependency. Updated nested issued resolution to use authenticated
descriptor-stage anchors and per-stage output owners, retained matching parent
policy context, and unioned authentic invocation seals before resolution.
Updated source branch selection to continue only on the two exact topology or
fresh-invocation mismatch errors; authentication and conflict failures remain
fatal. The discarded-origin fixture now checks an event with multiple
slice-timed contributions and confirms charged resolution. Eight of the nine
named tests pass, but `discarded_augmented_source_origins_are_resolved` still
fails with `event does not match admitted route topology`; do not mark the
resolver tasks complete.

Foreground verification on the final source:

- `CARGO_TERM_QUIET=true cargo build` — exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/build-final-candidate.log`.
- `CARGO_TERM_QUIET=true cargo check` — exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/check-final.log`.
- Focused issued resolver nextest — exit 100; 9 tests, 8 passed, 1 failed;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/focused-current-source.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast`
  — exit 100; 2793 run, 2792 passed, 1 failed, 3 skipped; the sole failure is
  `discarded_augmented_source_origins_are_resolved`;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/full-final-source.log`.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  — exit 0; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/wasm-final.log`.
- Scoped `rustfmt --edition 2021 --check` on the plan's touched Rust paths —
  exit 0; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/fmt-final.log`.
- Strict all-target Clippy — exit 101; the diagnostics are disposition-limited
  dead-code/unused-import warnings plus `clippy::let_and_return` in the
  downstream shared-work path;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/agent-clippy.log`.
- Unowned-path equality and no-new-suppression checks pass. The Rust cohort is
  953, and all touched Rust files are below 1000 lines (`source.rs`: 991).

The complete suite remains red, so implementation acceptance is incomplete.
The remaining fixture diagnosis is an exact owner mismatch: the sequence-root
descriptor is Rev4 at placement `[1,1,5,2]`, while every matching fresh owner
frame is Rev2 at the same root and placement. Session 254 requires the owner
predicate to compare the stage output handle's revision and placement, and
TASK-010 requires this static-`[0]` program to remain byte-identical. A local
candidate that removes the intro sequence would reconcile those identities,
but it violates that accepted fixture constraint, so it was not applied. No
identity check was relaxed. Resume after an accepted TASK-010 fixture amendment
or an explicitly compatible output-owner interpretation, then rerun the
discarded-origin focused test and full nextest. Formal review and workflow
finalization remain downstream.

Diagnostic attempts are in `session259/discarded-owner-diagnostic.log`,
`session259/discarded-anchor-debug.log` and
`session259/discarded-branch-debug.log`. Temporary diagnostic output was
removed from source before handoff. The latest full-suite evidence above is
from before partial descriptor-anchor retention; it remains a failure and is
not claimed as final-source passing evidence.

### Session 259 final-source verification addendum

The following gates ran after descriptor-anchor retention and after all
temporary diagnostics were removed:

- `CARGO_TERM_QUIET=true cargo check` — exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/check-post-diagnostics.log`.
- `CARGO_TERM_QUIET=true cargo build` — exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/build-post-diagnostics.log`.
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  — exit 0; `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/wasm-post-diagnostics.log`.
- Focused resolver command — exit 100; 9 run, 8 passed, 1 failed;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/focused-final-source.log`.
- Full nextest `--no-fail-fast` — exit 100; 2793 run, 2792 passed, 1 failed,
  3 skipped; `discarded_augmented_source_origins_are_resolved` is the only
  failure. Log: `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/full-post-diagnostics.log`.
- Strict all-target Clippy — exit 101; 67 dead-code/unused-import errors and
  one `let_and_return` in the downstream shared-work path;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/clippy-post-diagnostics.log`.
- Scoped `rustfmt --edition 2021 --check` — exit 0;
  `tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/fmt-post-diagnostics.log`.
- Line caps pass (`source.rs` 991, `nested/issued.rs` 901); cohort is 953;
  unowned-path equality and no-new-suppression checks pass. Logs:
  `line-counts-final.log`, `cohort-files-final.log`,
  `unowned-paths-final.log`, `suppressions-final2.log` in the same session259
  directory.

This final-source suite result supersedes the earlier pre-anchor full-suite
record. The TASK-010/TASK-008 fixture-owner conflict remains unresolved, so all
resolver completion criteria stay unchecked.
