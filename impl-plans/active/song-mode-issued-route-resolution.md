# Authenticated issued route resolution

**Status**: In Progress (wave 2a, session 252, serial; see "Session 252 amendment")
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

**Status**: Not Started
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
